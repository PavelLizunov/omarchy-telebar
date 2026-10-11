use libloading::Library;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    ffi::{CString, c_char, c_double, c_int},
    io,
    marker::PhantomData,
    os::fd::AsRawFd,
    path::Path,
    rc::Rc,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

const RESPONSE_MAX: usize = 32 * 1024 * 1024;
// The official JSON ABI owns global receive/execute buffers. A process-wide
// guard serializes their use, including tests opening independent wrappers.
static NATIVE_OWNER: Mutex<()> = Mutex::new(());

type Create = unsafe extern "C" fn() -> c_int;
type Send = unsafe extern "C" fn(c_int, *const c_char);
type Receive = unsafe extern "C" fn(c_double) -> *const c_char;
type Execute = unsafe extern "C" fn(*const c_char) -> *const c_char;

pub struct Td {
    create: Create,
    send: Send,
    receive: Receive,
    execute: Execute,
    clients: BTreeSet<i32>,
    _library: Library,
    _owner: MutexGuard<'static, ()>,
    // TDLib calls and borrowed buffers stay on one owner thread. No unsafe
    // Send/Sync implementation is needed or permitted for this wrapper.
    _thread: PhantomData<Rc<()>>,
}

impl Td {
    pub fn open(path: &Path) -> io::Result<Self> {
        let owner = NATIVE_OWNER
            .try_lock()
            .map_err(|_| io::Error::other("TDLib already has a receiver owner"))?;
        let file = crate::paths::library_file(path)?;
        let anchored = format!("/proc/self/fd/{}", file.as_raw_fd());
        // SAFETY: loading executes trusted native code. The opened regular file
        // and parent ownership/mode checks establish the task's library trust
        // policy; /proc/self/fd keeps the checked inode stable during dlopen.
        // TDLib owns process-global native workers. Its JSON ABI has no global
        // shutdown/join function, even after all clients report Closed. Pin the
        // mapping until process exit: dlclose must never unload worker code or
        // thread-local destructors, including on symbol errors/close timeout.
        let library: Library = unsafe {
            libloading::os::unix::Library::open(
                Some(anchored),
                libc::RTLD_NOW | libc::RTLD_NODELETE,
            )
        }
        .map_err(|_| io::Error::other("TDLib could not be loaded"))?
        .into();
        // SAFETY: these exact four signatures are specified by the official
        // td_json_client.h ABI. Symbol values are retained only with Library.
        let (create, send, receive, execute) = unsafe {
            (
                *library
                    .get::<Create>(b"td_create_client_id\0")
                    .map_err(|_| io::Error::other("missing td_create_client_id"))?,
                *library
                    .get::<Send>(b"td_send\0")
                    .map_err(|_| io::Error::other("missing td_send"))?,
                *library
                    .get::<Receive>(b"td_receive\0")
                    .map_err(|_| io::Error::other("missing td_receive"))?,
                *library
                    .get::<Execute>(b"td_execute\0")
                    .map_err(|_| io::Error::other("missing td_execute"))?,
            )
        };
        let mut td = Self {
            create,
            send,
            receive,
            execute,
            clients: BTreeSet::new(),
            _library: library,
            _owner: owner,
            _thread: PhantomData,
        };
        td.execute(&json!({"@type":"setLogStream","log_stream":{"@type":"logStreamEmpty"}}))?;
        td.execute(&json!({"@type":"setLogVerbosityLevel","new_verbosity_level":0}))?;
        Ok(td)
    }

    pub fn create_client(&mut self) -> io::Result<i32> {
        // SAFETY: symbol and lifetime were verified at open; only this owner
        // can call the wrapper, and create has no pointer arguments.
        let id = unsafe { (self.create)() };
        if id <= 0 {
            return Err(io::Error::other("invalid native client id"));
        }
        self.clients.insert(id);
        Ok(id)
    }

    pub fn send(&mut self, id: i32, value: &Value) -> io::Result<()> {
        if !self.clients.contains(&id) {
            return Err(io::Error::other("unknown native client"));
        }
        let query = encode(value)?;
        // SAFETY: id belongs to this owner; query is NUL-terminated and remains
        // alive through td_send, which consumes/copies the request synchronously.
        unsafe { (self.send)(id, query.as_ptr()) };
        Ok(())
    }

    pub fn execute(&mut self, value: &Value) -> io::Result<Option<Value>> {
        let query = encode(value)?;
        // SAFETY: valid JSON CString lives through this call. Only this owner
        // invokes receive/execute and the result is copied before any next call.
        let ptr = unsafe { (self.execute)(query.as_ptr()) };
        // SAFETY: the trusted ABI returns null or a valid NUL-terminated buffer
        // valid until the next receive/execute. No intervening native call occurs.
        unsafe { copy_result(ptr) }
    }

    pub fn receive(&mut self, timeout: Duration) -> io::Result<Option<Value>> {
        // SAFETY: finite timeout and exclusive owner satisfy the official ABI.
        let ptr = unsafe { (self.receive)(timeout.as_secs_f64()) };
        // SAFETY: same ABI lifetime as execute; copy completes before reuse.
        let value = unsafe { copy_result(ptr) }?;
        if let Some(event) = &value
            && event["@type"] == "updateAuthorizationState"
            && event["authorization_state"]["@type"] == "authorizationStateClosed"
            && let Some(id) = event["@client_id"]
                .as_i64()
                .and_then(|v| i32::try_from(v).ok())
        {
            self.clients.remove(&id);
        }
        Ok(value)
    }

    pub fn close(&mut self, timeout: Duration) -> io::Result<()> {
        let ids: Vec<_> = self.clients.iter().copied().collect();
        for id in ids {
            self.send(id, &json!({"@type":"close"}))?;
        }
        let deadline = Instant::now() + timeout;
        while !self.clients.is_empty() && Instant::now() < deadline {
            self.receive(Duration::from_millis(50))?;
        }
        if !self.clients.is_empty() {
            return Err(io::Error::other(
                "native clients did not close before deadline",
            ));
        }
        Ok(())
    }
}

impl Drop for Td {
    fn drop(&mut self) {
        if !self.clients.is_empty() {
            let _ = self.close(Duration::from_secs(5));
        }
    }
}

fn encode(value: &Value) -> io::Result<CString> {
    CString::new(value.to_string()).map_err(|_| io::Error::other("invalid native JSON"))
}

unsafe fn copy_result(ptr: *const c_char) -> io::Result<Option<Value>> {
    if ptr.is_null() {
        return Ok(None);
    }
    // SAFETY: caller guarantees a valid native NUL-terminated allocation.
    // strnlen stops at NUL without reading beyond it and caps all copying.
    let len = unsafe { libc::strnlen(ptr, RESPONSE_MAX + 1) };
    if len > RESPONSE_MAX {
        return Err(io::Error::other("native JSON exceeds response limit"));
    }
    // SAFETY: strnlen established len initialized readable bytes; the native
    // owner has not called receive/execute again, so the allocation is live.
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
    let value =
        serde_json::from_slice(bytes).map_err(|_| io::Error::other("native JSON is invalid"))?;
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_native_bytes_survive_replacement() {
        let text = CString::new(r#"{"@type":"ok","value":"😀"}"#).unwrap();
        // SAFETY: CString owns a valid NUL-terminated buffer for this scope.
        let value = unsafe { copy_result(text.as_ptr()) }.unwrap().unwrap();
        drop(text);
        assert_eq!(value["value"], "😀");
        // SAFETY: null is explicitly accepted by copy_result.
        assert_eq!(unsafe { copy_result(std::ptr::null()) }.unwrap(), None);
    }
}
