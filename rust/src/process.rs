//! Finite subprocess owner. No shell, inherited credentials or unmanaged jobs.
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub struct Output {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
    pub truncated: bool,
}
impl Output {
    pub fn ok(&self) -> bool {
        self.code == Some(0) && !self.timed_out && !self.truncated
    }
}
struct Owned {
    child: Child,
    reaped: bool,
}
impl Owned {
    fn exited(&self) -> io::Result<bool> {
        // SAFETY: waitid writes the initialized siginfo; WNOWAIT deliberately
        // retains our exact child's PID/group identity even after leader exit.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: si_pid reads the child-status union set by waitid.
        Ok(unsafe { info.si_pid() } != 0)
    }
    fn signal(&self, signal: i32) -> io::Result<()> {
        if self.reaped {
            return Ok(());
        }
        let pid =
            i32::try_from(self.child.id()).map_err(|_| io::Error::other("invalid helper pid"))?;
        // SAFETY: unreaped owned leader pins group identity until final cleanup.
        if unsafe { libc::kill(-pid, signal) } != 0
            && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    fn stop(&mut self) -> io::Result<Option<i32>> {
        if self.reaped {
            return Ok(None);
        }
        let pid =
            i32::try_from(self.child.id()).map_err(|_| io::Error::other("invalid helper pid"))?;
        // SAFETY: the leader remains unreaped, so this exact task-owned group
        // cannot be reused. Retire descendants even when the leader exited.
        let killed = unsafe { libc::kill(-pid, libc::SIGKILL) };
        if killed != 0 && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            return Err(io::Error::last_os_error());
        }
        let status = self.child.wait()?;
        self.reaped = true;
        Ok(status.code())
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
fn nonblocking(fd: i32) -> io::Result<()> {
    // SAFETY: caller owns this open pipe; fcntl has no borrowed pointers.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn drain<T: Read>(pipe: &mut T, bytes: &mut Vec<u8>, limit: usize) -> io::Result<bool> {
    loop {
        let mut chunk = [0; 8192];
        match pipe.read(&mut chunk) {
            Ok(0) => return Ok(false),
            Ok(n) => {
                let free = limit.saturating_sub(bytes.len());
                bytes.extend_from_slice(&chunk[..n.min(free)]);
                if n > free {
                    return Ok(true);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}
/// Absolute verified executable, explicit argv/environment and bounded stdin.
/// Sensitive bytes belong only to pipes, never argv or diagnostic messages.
pub fn run(
    executable: &str,
    args: &[&str],
    environment: &BTreeMap<String, String>,
    input: &[u8],
    timeout: Duration,
    limit: usize,
) -> io::Result<Output> {
    run_cancel(
        executable,
        args,
        environment,
        input,
        timeout,
        limit,
        &std::sync::atomic::AtomicBool::new(false),
    )
}
pub fn run_cancel(
    executable: &str,
    args: &[&str],
    environment: &BTreeMap<String, String>,
    input: &[u8],
    timeout: Duration,
    limit: usize,
    cancel: &std::sync::atomic::AtomicBool,
) -> io::Result<Output> {
    if !executable.starts_with('/')
        || timeout.is_zero()
        || timeout > Duration::from_secs(3600)
        || limit > 32 * 1024 * 1024
        || input.len() > 32 * 1024 * 1024
    {
        return Err(io::Error::other("invalid helper bounds"));
    }
    // A worker retired before dispatch must not start even a short-lived
    // helper that can mutate an external service (for example Secret Service).
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(Output {
            code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            timed_out: true,
            truncated: false,
        });
    }
    let mut child = Managed::start(executable, args, environment, input, timeout, limit)?;
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return child.cancel();
        }
        if let Some(output) = child.poll()? {
            return Ok(output);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Pollable foreground child, owned by the native loop or a finite worker.
/// Finite helpers have a deadline; a persistent selection ends with its owner.
/// No auto-fork, detached child or restart loop is accepted by this owner.
pub struct Managed {
    child: Owned,
    stdin: Option<std::process::ChildStdin>,
    stdout: std::process::ChildStdout,
    stderr: std::process::ChildStderr,
    input: Vec<u8>,
    sent: usize,
    limit: usize,
    deadline: Option<Instant>,
    output: Output,
    finished: bool,
    input_complete: bool,
    keep_stdin: bool,
}
impl Managed {
    pub fn start(
        executable: &str,
        args: &[&str],
        environment: &BTreeMap<String, String>,
        input: &[u8],
        timeout: Duration,
        limit: usize,
    ) -> io::Result<Self> {
        if !executable.starts_with('/')
            || timeout.is_zero()
            || timeout > Duration::from_secs(3600)
            || limit > 32 * 1024 * 1024
            || input.len() > 32 * 1024 * 1024
        {
            return Err(io::Error::other("invalid helper bounds"));
        }
        Self::start_until(
            executable,
            args,
            environment,
            input,
            Some(Instant::now() + timeout),
            limit,
        )
    }
    /// One foreground selection service, retired on replacement/owner shutdown.
    /// This is not a detached process or a permission to run an unbounded worker.
    pub fn start_owned(
        executable: &str,
        args: &[&str],
        environment: &BTreeMap<String, String>,
        input: &[u8],
        limit: usize,
    ) -> io::Result<Self> {
        Self::start_until(executable, args, environment, input, None, limit)
    }
    fn start_until(
        executable: &str,
        args: &[&str],
        environment: &BTreeMap<String, String>,
        input: &[u8],
        deadline: Option<Instant>,
        limit: usize,
    ) -> io::Result<Self> {
        if !executable.starts_with('/')
            || limit > 32 * 1024 * 1024
            || input.len() > 32 * 1024 * 1024
        {
            return Err(io::Error::other("invalid helper bounds"));
        }
        let executable = crate::executable::checked(std::path::Path::new(executable))?;
        let mut command = Command::new(executable);
        command
            .args(args)
            .env_clear()
            .envs(environment)
            .env("PATH", "/usr/bin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        Self::spawn_command(command, input, deadline, limit)
    }
    fn spawn_command(
        mut command: Command,
        input: &[u8],
        deadline: Option<Instant>,
        limit: usize,
    ) -> io::Result<Self> {
        let mut child = Owned {
            child: command.spawn()?,
            reaped: false,
        };
        let stdin = child.child.stdin.take();
        let stdout = child
            .child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("helper stdout missing"))?;
        let stderr = child
            .child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("helper stderr missing"))?;
        nonblocking(stdout.as_raw_fd())?;
        nonblocking(stderr.as_raw_fd())?;
        if let Some(pipe) = &stdin {
            nonblocking(pipe.as_raw_fd())?;
        }
        Ok(Self {
            child,
            stdin,
            stdout,
            stderr,
            input: input.to_vec(),
            sent: 0,
            limit,
            deadline,
            output: Output {
                code: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                timed_out: false,
                truncated: false,
            },
            finished: false,
            input_complete: input.is_empty(),
            keep_stdin: false,
        })
    }
    /// Keep stdin as an owner-lifetime pipe after a bounded startup config.
    pub fn keep_stdin_open(&mut self) {
        self.keep_stdin = true;
    }
    pub fn output_snapshot(&self) -> (&[u8], &[u8]) {
        (&self.output.stdout, &self.output.stderr)
    }
    pub fn take_stdout(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output.stdout)
    }
    pub fn take_stderr(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output.stderr)
    }
    pub fn pid(&self) -> u32 {
        self.child.child.id()
    }
    pub fn input_sent(&self) -> bool {
        self.input_complete
    }
    pub fn signal_stop(&mut self, signal: i32, grace: Duration) -> io::Result<()> {
        if !matches!(signal, libc::SIGINT | libc::SIGTERM) || grace > Duration::from_secs(5) {
            return Err(io::Error::other("invalid helper stop"));
        }
        if self.finished {
            return Ok(());
        }
        self.child.signal(signal)?;
        let until = Instant::now() + grace;
        self.deadline = Some(self.deadline.map_or(until, |deadline| deadline.min(until)));
        Ok(())
    }
    fn finish(&mut self) -> io::Result<Output> {
        self.output.code = self.child.stop()?;
        self.stdin = None;
        self.input.clear();
        self.finished = true;
        self.output.truncated |= drain(&mut self.stdout, &mut self.output.stdout, self.limit)?;
        self.output.truncated |= drain(&mut self.stderr, &mut self.output.stderr, self.limit)?;
        Ok(std::mem::replace(
            &mut self.output,
            Output {
                code: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                timed_out: false,
                truncated: false,
            },
        ))
    }
    pub fn cancel(&mut self) -> io::Result<Output> {
        if self.finished {
            return Err(io::Error::other("helper already retired"));
        }
        self.output.timed_out = true;
        self.finish()
    }
    pub fn poll(&mut self) -> io::Result<Option<Output>> {
        if self.finished {
            return Ok(None);
        }
        self.output.truncated |= drain(&mut self.stdout, &mut self.output.stdout, self.limit)?;
        self.output.truncated |= drain(&mut self.stderr, &mut self.output.stderr, self.limit)?;
        if self.output.truncated
            || self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.output.timed_out = !self.output.truncated;
            return self.finish().map(Some);
        }
        if let Some(pipe) = &mut self.stdin {
            if self.sent == self.input.len() {
                if !self.keep_stdin {
                    self.stdin = None;
                }
                self.input.clear();
                self.sent = 0;
            } else {
                match pipe.write(&self.input[self.sent..]) {
                    Ok(n) => {
                        self.sent += n;
                        self.input_complete = self.sent == self.input.len();
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) if e.kind() == io::ErrorKind::BrokenPipe => self.stdin = None,
                    Err(e) => return Err(e),
                }
            }
        }
        if self.child.exited()? {
            return self.finish().map(Some);
        }
        Ok(None)
    }
}

/// Finite long build invocation with a tail-only private log. Command was
/// constructed by the pinned helper, never from a request-provided executable.
pub fn run_command_logged(
    command: Command,
    timeout: Duration,
    home: &crate::storage::Storage,
    path: &str,
    limit: usize,
    argv: &[&str],
) -> io::Result<bool> {
    if timeout.is_zero() || timeout > Duration::from_secs(4 * 3600) || limit > 8 * 1024 * 1024 {
        return Err(io::Error::other("invalid build bounds"));
    }
    let mut helper = Managed::spawn_command(
        command,
        &[],
        Some(Instant::now() + timeout),
        64 * 1024 * 1024,
    )?;
    let mut tail = home.read(path, limit).ok().flatten().unwrap_or_default();
    tail.extend_from_slice(format!("\n$ {}\n", argv.join(" ")).as_bytes());
    let mut total = 0usize;
    let completed = loop {
        let done = helper.poll()?;
        let (out, err) = if let Some(done) = &done {
            (done.stdout.clone(), done.stderr.clone())
        } else {
            (helper.take_stdout(), helper.take_stderr())
        };
        total = total.saturating_add(out.len() + err.len());
        tail.extend(out);
        tail.extend(err);
        if tail.len() > limit {
            tail.drain(..tail.len() - limit);
        }
        if total > 64 * 1024 * 1024 {
            break false;
        }
        if let Some(done) = done {
            break done.ok();
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    home.write(path, &tail, 0o600)?;
    Ok(completed)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_reaps_the_exact_helper_without_waiting_for_deadline() {
        let cancel = std::sync::atomic::AtomicBool::new(true);
        // Missing executable proves a pre-cancelled request never spawns.
        let missing = run_cancel(
            "/fixture/nonexistent-helper",
            &[],
            &BTreeMap::new(),
            &[],
            Duration::from_secs(5),
            4096,
            &cancel,
        )
        .unwrap();
        assert!(missing.timed_out && missing.code.is_none());
        let at = Instant::now();
        let output = run_cancel(
            "/usr/bin/sleep",
            &["5"],
            &BTreeMap::new(),
            &[],
            Duration::from_secs(10),
            64,
            &cancel,
        )
        .unwrap();
        assert!(!output.ok());
        assert!(at.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn pipes_are_bounded_and_owned_helpers_are_reaped() {
        let env = BTreeMap::new();
        let output = run(
            "/usr/bin/cat",
            &[],
            &env,
            b"synthetic",
            Duration::from_secs(2),
            100,
        )
        .unwrap();
        assert!(output.ok());
        assert_eq!(output.stdout, b"synthetic");
        let output = run("/usr/bin/yes", &[], &env, &[], Duration::from_secs(2), 128).unwrap();
        assert!(output.truncated);
        assert_eq!(output.stdout.len(), 128);
        let output = run(
            "/usr/bin/sleep",
            &["10"],
            &env,
            &[],
            Duration::from_millis(50),
            128,
        )
        .unwrap();
        assert!(output.timed_out && !output.ok());
        assert!(run("cat", &[], &env, &[], Duration::from_secs(1), 100).is_err());
    }
}
