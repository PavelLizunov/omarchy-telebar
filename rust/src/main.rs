use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use telebar_runtime::{
    projection,
    td::Td,
    wire::{self, Request},
};

static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

fn main() {
    if run().is_err() {
        eprintln!("telebar-runtime: operation failed; no live account cutover performed");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [mode] if mode == "--project" => project_stream(),
        [mode, path] if mode == "--probe-tdlib" => {
            let mut td = Td::open(Path::new(path))?;
            let version = td.execute(&json!({"@type":"getOption","name":"version"}))?;
            let commit = td.execute(&json!({"@type":"getOption","name":"commit_hash"}))?;
            println!(
                "{}",
                json!({"version":version,"commit":commit,"profile_opened":false})
            );
            td.close(Duration::from_secs(5))
        }
        [mode, path] if mode == "--probe-lifecycle" => {
            let mut td = Td::open(Path::new(path))?;
            let client = td.create_client()?;
            td.send(
                client,
                &json!({"@type":"getOption","name":"version","@extra":1}),
            )?;
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut version = Value::Null;
            while Instant::now() < deadline {
                if let Some(event) = td.receive(Duration::from_millis(50))?
                    && event["@extra"] == 1
                {
                    version = event;
                    break;
                }
            }
            td.close(Duration::from_secs(5))?;
            println!(
                "{}",
                json!({"version":version,"closed":true,"parameters_sent":false,"profile_opened":false})
            );
            Ok(())
        }
        [mode, path] if mode == "--probe-driver" => {
            let mut driver = telebar_runtime::native_driver::Driver::open(
                Path::new(path),
                "default".into(),
                "en".into(),
            )?;
            driver.register("default".into(), "Inert driver probe".into(), vec![])?;
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline
                && driver.runtime.metadata["default"].auth["state"] != "needCredentials"
            {
                driver.tick(&mut |_| Ok(None))?;
                std::thread::sleep(Duration::from_millis(5));
            }
            let auth = driver.runtime.metadata["default"].auth.clone();
            driver.close(Duration::from_secs(5))?;
            if auth["state"] != "needCredentials" {
                return Err(io::Error::other("driver did not reach parameter boundary"));
            }
            println!(
                "{}",
                json!({"native_driver":true,"auth":auth,"closed":true,"parameters_sent":false,"profile_opened":false})
            );
            Ok(())
        }
        [mode, path] if mode == "--probe-runtime" => {
            let mut td = Td::open(Path::new(path))?;
            let client = td.create_client()?;
            let mut runtime = telebar_runtime::authorization::Runtime::new("default".into());
            let initial = runtime
                .register(
                    "default".into(),
                    "Inert lifecycle probe".into(),
                    client,
                    vec![],
                    Instant::now(),
                )
                .map_err(io::Error::other)?;
            for request in initial.native {
                td.send(request.client, &request.query)?;
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut waiting = false;
            while Instant::now() < deadline {
                if let Some(event) = td.receive(Duration::from_millis(50))? {
                    waiting |= event["authorization_state"]["@type"]
                        == "authorizationStateWaitTdlibParameters";
                    let effects =
                        runtime.receive(event, Instant::now(), &mut |_| Ok(None), &mut |query| {
                            td.execute(query).ok().flatten()
                        });
                    for request in effects.native {
                        if request.query["@type"] == "setTdlibParameters" {
                            return Err(io::Error::other(
                                "inert startup unexpectedly prepared parameters",
                            ));
                        }
                        td.send(request.client, &request.query)?;
                    }
                    if waiting && runtime.metadata["default"].auth["state"] == "needCredentials" {
                        break;
                    }
                }
            }
            runtime.stopping = true;
            td.close(Duration::from_secs(5))?;
            if !waiting {
                return Err(io::Error::other(
                    "native startup did not reach parameter boundary",
                ));
            }
            println!(
                "{}",
                json!({"runtime_owner":true,"parameter_boundary":true,"auth":runtime.metadata["default"].auth,"closed":true,"parameters_sent":false,"profile_opened":false})
            );
            Ok(())
        }
        [mode, directory] if mode == "--inert-socket" => {
            serve(Path::new(directory), false, None, false)
        }
        [mode, directory] if mode == "--inert-engine" => {
            serve(Path::new(directory), true, None, false)
        }
        [mode, directory, library] if mode == "--inert-native" => {
            serve(Path::new(directory), false, Some(Path::new(library)), false)
        }
        [mode, directory, library] if mode == "--inert-application" => {
            serve(Path::new(directory), false, Some(Path::new(library)), true)
        }
        _ => {
            eprintln!(
                "Usage: telebar-runtime --project | --probe-tdlib <verified-library> | --probe-lifecycle <verified-library> | --probe-runtime <verified-library> | --inert-socket <private-fixture-directory> | --inert-engine <private-fixture-directory>"
            );
            eprintln!(
                "Migration candidate: production daemon mode is not available until all commands/helpers pass parity."
            );
            Err(io::Error::other("invalid migration mode"))
        }
    }
}

fn project_stream() -> io::Result<()> {
    let mut input = io::stdin().lock();
    let mut line = Vec::new();
    loop {
        let mut byte = [0];
        if input.read(&mut byte)? == 0 {
            break;
        }
        if byte[0] != b'\n' {
            if line.len() >= 32 * 1024 * 1024 {
                return Err(io::Error::other("fixture exceeds limit"));
            }
            line.push(byte[0]);
            continue;
        }
        let value: Value =
            serde_json::from_slice(&line).map_err(|_| io::Error::other("invalid fixture"))?;
        let roots: Vec<_> = value["roots"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        let result = projection::project(
            value["operation"].as_str().unwrap_or(""),
            &value["value"],
            &roots,
        )
        .map_err(io::Error::other)?;
        println!("{result}");
        line.clear();
    }
    if !line.is_empty() {
        return Err(io::Error::other("unterminated fixture"));
    }
    Ok(())
}

struct Peer {
    id: u64,
    socket: UnixStream,
    inbox: Vec<u8>,
    outbox: Vec<u8>,
    written: usize,
}
impl Peer {
    fn queue(&mut self, value: Value) -> io::Result<()> {
        let data = value.to_string();
        if self
            .outbox
            .len()
            .saturating_sub(self.written)
            .saturating_add(data.len() + 1)
            > wire::OUTBOX_MAX
        {
            return Err(io::Error::other("slow consumer"));
        }
        if self.written > 0 {
            self.outbox.drain(..self.written);
            self.written = 0;
        }
        self.outbox.extend_from_slice(data.as_bytes());
        self.outbox.push(b'\n');
        Ok(())
    }

    fn pump(
        &mut self,
        handler: &mut dyn FnMut(Request) -> io::Result<Vec<Value>>,
    ) -> io::Result<()> {
        let mut read_bytes = 0;
        loop {
            // A chatty peer must yield to native events, deadlines and peers.
            if read_bytes >= 128 * 1024 {
                break;
            }
            let mut chunk = [0u8; 65536];
            match self.socket.read(&mut chunk) {
                Ok(0) => return Err(io::Error::other("peer closed")),
                Ok(n) => {
                    read_bytes += n;
                    self.inbox.extend_from_slice(&chunk[..n]);
                    while let Some(newline) = self.inbox.iter().position(|b| *b == b'\n') {
                        if newline > wire::LINE_MAX {
                            return Err(io::Error::other("oversized line"));
                        }
                        let responses = match Request::parse(&self.inbox[..newline]) {
                            Err(error) => vec![error],
                            Ok(request) => handler(request)?,
                        };
                        self.inbox.drain(..=newline);
                        for response in responses {
                            self.queue(response)?;
                        }
                    }
                    if self.inbox.len() > wire::LINE_MAX {
                        return Err(io::Error::other("oversized line"));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        while self.written < self.outbox.len() {
            match self.socket.write(&self.outbox[self.written..]) {
                Ok(0) => return Err(io::Error::other("peer closed")),
                Ok(n) => self.written += n,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        if self.written == self.outbox.len() {
            self.outbox.clear();
            self.written = 0;
        }
        Ok(())
    }
}

struct SocketCleanup<'a>(&'a Path);
impl Drop for SocketCleanup<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0);
    }
}

fn serve(
    directory: &Path,
    with_engine: bool,
    library: Option<&Path>,
    with_application: bool,
) -> io::Result<()> {
    // An inert fixture directory is explicit; this mode never derives a real
    // runtime/home, opens TDLib, reads keyring or loads accounts/settings.
    let meta = fs::symlink_metadata(directory)?;
    // SAFETY: getuid has no pointer arguments or mutable caller memory.
    let uid = unsafe { libc::getuid() };
    if !directory.is_absolute()
        || !meta.is_dir()
        || meta.uid() != uid
        || meta.mode() & 0o777 != 0o700
    {
        return Err(io::Error::other(
            "fixture directory must be owned mode 0700",
        ));
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(directory.join("migration.lock"))?;
    // SAFETY: flock operates on the valid owned open descriptor.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(io::Error::other("fixture already owned"));
    }
    let socket_path = directory.join("migration.sock");
    // Refuse any existing path rather than remove a potentially foreign socket.
    if fs::symlink_metadata(&socket_path).is_ok() {
        return Err(io::Error::other("fixture socket already exists"));
    }
    // SAFETY: setting umask is process-global; no worker threads exist here.
    let mask = unsafe { libc::umask(0o177) };
    let listener_result = UnixListener::bind(&socket_path);
    // SAFETY: restore the original process mask, before starting any workers.
    unsafe { libc::umask(mask) };
    let listener = listener_result?;
    let _cleanup = SocketCleanup(&socket_path);
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    // SAFETY: fixed signal handlers only store a lock-free atomic boolean;
    // signatures match libc signal and no memory/lifetime is passed to C.
    unsafe {
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, stop as *const () as libc::sighandler_t);
    }
    let mut driver = if let Some(library) = library {
        let mut driver =
            telebar_runtime::native_driver::Driver::open(library, "default".into(), "en".into())?;
        if !with_application {
            driver.register("default".into(), "Inert native socket".into(), vec![])?;
        }
        Some(driver)
    } else {
        None
    };
    let mut application = if with_application {
        let home = telebar_runtime::storage::Storage::open(directory)?;
        let mut app = telebar_runtime::application::Application::load(&home)?;
        app.diagnostics = telebar_runtime::diagnostics::Diagnostics::start(&home).ok();
        let mut databases = vec![directory.join(".local/share/omagram/database")];
        databases.extend(
            app.catalog["accounts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|a| a["id"].as_str())
                .filter(|id| *id != "default")
                .map(|id| directory.join(format!(".local/share/omagram/accounts/{id}/database"))),
        );
        app.upload_context = Some((directory.to_owned(), databases));
        let owner = driver
            .as_mut()
            .ok_or_else(|| io::Error::other("application fixture requires native owner"))?;
        for row in app.catalog["accounts"].as_array().into_iter().flatten() {
            let id = row["id"]
                .as_str()
                .ok_or_else(|| io::Error::other("invalid fixture account"))?;
            if !owner.runtime.engine.accounts.contains_key(id) {
                owner.register(id.into(), row["name"].as_str().unwrap_or(id).into(), vec![])?;
            }
            let meta = owner
                .runtime
                .metadata
                .get_mut(id)
                .expect("registered fixture account");
            meta.phone = row["phone"].as_str().unwrap_or("").into();
            owner
                .runtime
                .engine
                .accounts
                .get_mut(id)
                .expect("registered fixture account")
                .state
                .me_id = telebar_runtime::projection::integer(&row["meId"], 0);
        }
        owner.runtime.engine.active = app.catalog["active"].as_str().unwrap_or("default").into();
        Some((home, app))
    } else {
        None
    };
    println!("{}", json!({"event":"ready","production_profiles":false}));
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut engine = telebar_runtime::engine::Engine::new("default".into());
    if with_engine {
        for (id, client) in [("default", 1), ("other", 2)] {
            engine.accounts.insert(
                id.into(),
                telebar_runtime::engine::Account {
                    client,
                    generation: 1,
                    state: telebar_runtime::state::State::default(),
                    ready: true,
                },
            );
        }
    }
    let mut next_peer = 1;
    let mut peers: Vec<Peer> = Vec::new();
    while !STOP.load(Ordering::Relaxed) && Instant::now() < deadline {
        match listener.accept() {
            Ok((socket, _)) => {
                let mut credential = libc::ucred {
                    pid: 0,
                    uid: 0,
                    gid: 0,
                };
                let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
                // SAFETY: credential and size are initialized correctly-sized
                // output buffers alive for getsockopt; socket is an open fd.
                let accepted = unsafe {
                    libc::getsockopt(
                        socket.as_raw_fd(),
                        libc::SOL_SOCKET,
                        libc::SO_PEERCRED,
                        (&mut credential as *mut libc::ucred).cast(),
                        &mut size,
                    )
                } == 0;
                if accepted && credential.uid == uid && peers.len() < wire::CLIENTS_MAX {
                    socket.set_nonblocking(true)?;
                    peers.push(Peer {
                        id: next_peer,
                        socket,
                        inbox: Vec::new(),
                        outbox: Vec::new(),
                        written: 0,
                    });
                    if let Some((_, app)) = &mut application {
                        app.diagnostic_peers
                            .connect(next_peer, credential.pid as u32);
                    }
                    next_peer += 1;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
        if let Some((_, app)) = &mut application {
            app.diagnostic_runtime = json!({"event_queue":0,"task_queue":0,"recording":false,"oldest_td_ms":null,"td_idle_ms":null,"counts":{},"socket_in_bytes":peers.iter().map(|p|p.inbox.len()).sum::<usize>(),"socket_out_bytes":peers.iter().map(|p|p.outbox.len().saturating_sub(p.written)).sum::<usize>()});
            if let Some(log) = &app.diagnostics {
                log.progress(telebar_runtime::diagnostics::Phase::Ipc);
            }
        }
        let mut broadcasts = Vec::new();
        peers.retain_mut(|peer| {
            let id=peer.id;
            let result=peer.pump(&mut |request|{
                // Deliberately no compatible hello in either fixture mode: a
                // partial daemon must not impersonate the production backend.
                if request.command=="diagnostics.status" && application.is_none(){return Ok(vec![wire::success(request.id,json!({"runtime":"rust","migration_complete":false,"production_profiles":false}))]);}
                if request.command=="hello"{return Ok(vec![wire::failure(request.id,"migration command is not implemented",0)]);}
                if request.command=="fixture.version"&&let Some(driver)=&mut driver{return Ok(vec![wire::success(request.id,driver.version()?.unwrap_or(Value::Null))]);}
                if let Some(driver)=&mut driver{
                    let effects=if let Some((home,app))=&mut application{let mut request=request;if request.command=="fixture.snapshot"{request.command="hello".into();}driver.request_application(app,home,id,request)?}else{driver.request(id,request)?};
                    broadcasts.extend(effects.replies);
                    if effects.disconnect.is_some(){return Err(io::Error::other("duplicate live request id"));}
                    return Ok(Vec::new());
                }
                if !with_engine{return Ok(vec![wire::failure(request.id,"migration command is not implemented",0)]);}
                let mut effects=engine.request(id,request,Instant::now());
                if effects.disconnect.is_some(){broadcasts.extend(effects.replies);return Err(io::Error::other("duplicate live request id"));}
                for native in std::mem::take(&mut effects.native){
                    // Inert TD seam answers only synthetic getChatHistory. No
                    // TDLib load, profile, keyring or network exists in this mode.
                    if native.query["@type"]=="getChatHistory"{let done=engine.receive(json!({"@type":"messages","@client_id":native.client,"@extra":native.query["@extra"],"messages":[]}));effects.replies.extend(done.replies);}
                }
                Ok(effects.replies.into_iter().filter(|(peer,_)|*peer==id||*peer==0).map(|(_,v)|v).collect())
            });
            if result.is_err(){
                broadcasts.extend(engine.retire_peer(id).replies);
                if let Some(driver)=&mut driver{broadcasts.extend(driver.runtime.engine.retire_peer(id).replies);}
                if let Some((_,app))=&mut application{app.retire_peer(id);}
                false
            }else{true}
        });
        if let Some((_, app)) = &application
            && let Some(log) = &app.diagnostics
        {
            log.progress(telebar_runtime::diagnostics::Phase::Native);
        }
        let mut effects = if let Some(driver) = &mut driver {
            if let Some((home, app)) = &mut application {
                driver.tick_application(app, home, &mut |_| Ok(None))?
            } else {
                driver.tick(&mut |_| Ok(None))?
            }
        } else {
            engine.expire(Instant::now())
        };
        effects.replies.splice(0..0, broadcasts);
        let mut failed = std::collections::HashSet::new();
        for (id, value) in effects.replies {
            for peer in peers.iter_mut().filter(|peer| id == 0 || peer.id == id) {
                if peer.queue(value.clone()).is_err() {
                    failed.insert(peer.id);
                }
            }
        }
        let mut retired = Vec::new();
        peers.retain(|peer| {
            if failed.contains(&peer.id) {
                retired.extend(engine.retire_peer(peer.id).replies);
                if let Some((_, app)) = &mut application {
                    app.retire_peer(peer.id);
                }
                if let Some(driver) = &mut driver {
                    retired.extend(driver.runtime.engine.retire_peer(peer.id).replies);
                }
                false
            } else {
                true
            }
        });
        // Retirement can change retained-send state for surviving observers.
        for (id, value) in retired {
            for peer in peers.iter_mut().filter(|peer| id == 0 || peer.id == id) {
                let _ = peer.queue(value.clone());
            }
        }
        if application
            .as_ref()
            .is_some_and(|(_, app)| app.quit_deadline.is_some_and(|d| Instant::now() >= d))
        {
            break;
        }
        if let Some((_, app)) = &application
            && let Some(log) = &app.diagnostics
        {
            log.progress(telebar_runtime::diagnostics::Phase::Idle);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if let Some((_, app)) = &application
        && let Some(log) = &app.diagnostics
    {
        log.progress(telebar_runtime::diagnostics::Phase::Shutdown);
    }
    if let Some(driver) = &mut driver {
        driver.close(Duration::from_secs(5))?;
    }
    Ok(())
}
