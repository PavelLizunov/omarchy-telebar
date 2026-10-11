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
    let mut child = Owned {
        child: command.spawn()?,
        reaped: false,
    };
    let mut stdin = child.child.stdin.take();
    let mut stdout = child
        .child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("helper stdout missing"))?;
    let mut stderr = child
        .child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("helper stderr missing"))?;
    nonblocking(stdout.as_raw_fd())?;
    nonblocking(stderr.as_raw_fd())?;
    if let Some(pipe) = &stdin {
        nonblocking(pipe.as_raw_fd())?;
    }
    let deadline = Instant::now() + timeout;
    let mut out = Output {
        code: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
        timed_out: false,
        truncated: false,
    };
    let mut sent = 0;
    loop {
        out.truncated |= drain(&mut stdout, &mut out.stdout, limit)?;
        out.truncated |= drain(&mut stderr, &mut out.stderr, limit)?;
        if out.truncated
            || Instant::now() >= deadline
            || cancel.load(std::sync::atomic::Ordering::Relaxed)
        {
            out.timed_out = !out.truncated;
            child.stop()?;
            break;
        }
        if let Some(pipe) = &mut stdin {
            if sent == input.len() {
                stdin = None;
            } else {
                match pipe.write(&input[sent..]) {
                    Ok(n) => sent += n,
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) if e.kind() == io::ErrorKind::BrokenPipe => stdin = None,
                    Err(e) => return Err(e),
                }
            }
        }
        if child.exited()? {
            out.code = child.stop()?;
            out.truncated |= drain(&mut stdout, &mut out.stdout, limit)?;
            out.truncated |= drain(&mut stderr, &mut out.stderr, limit)?;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(out)
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
