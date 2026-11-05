//! Owner-only Linux Unix socket transport and bounded monotonic framing.

use crate::{
    canonical_json,
    error::{Error, Result},
    interface::{API_VERSION, Command, MAX_LINE_BYTES, Response, Service, WireError, strict_json},
    safe_fs::{Directory, validate_path},
};
use nix::{
    sys::{
        socket::{getsockopt, sockopt::PeerCredentials},
        stat::{FchmodatFlags, Mode, fchmodat},
    },
    unistd::getuid,
};
use serde_json::Value;
use std::{
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;

const SOCKET: &str = "ipc-v2.sock";
#[derive(Clone, Copy)]
pub(crate) struct IoLimits {
    pub stall: Duration,
    pub total: Duration,
    pub max: usize,
}
impl Default for IoLimits {
    fn default() -> Self {
        Self {
            stall: Duration::from_secs(5),
            total: Duration::from_secs(30),
            max: MAX_LINE_BYTES,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameError {
    Timeout,
    Unavailable,
    TooLarge,
}
fn timeout(
    limits: IoLimits,
    start: Instant,
    last: Instant,
) -> std::result::Result<Duration, FrameError> {
    let now = Instant::now();
    let total = limits
        .total
        .checked_sub(now.duration_since(start))
        .filter(|t| !t.is_zero())
        .ok_or(FrameError::Timeout)?;
    let stall = limits
        .stall
        .checked_sub(now.duration_since(last))
        .filter(|t| !t.is_zero())
        .ok_or(FrameError::Timeout)?;
    Ok(total.min(stall))
}
fn io_error(error: &io::Error) -> FrameError {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        FrameError::Timeout
    } else {
        FrameError::Unavailable
    }
}
pub(crate) fn read_frame(
    stream: &mut UnixStream,
    limits: IoLimits,
) -> std::result::Result<Option<Vec<u8>>, FrameError> {
    let start = Instant::now();
    let mut last = start;
    let mut line = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        stream
            .set_read_timeout(Some(timeout(limits, start, last)?))
            .map_err(|e| io_error(&e))?;
        let count = match stream.read(&mut buffer) {
            Ok(0) => return Ok(None),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_error(&e)),
        };
        timeout(limits, start, last)?;
        last = Instant::now();
        let end = buffer[..count].iter().position(|b| *b == b'\n');
        let used = end.map_or(count, |i| i + 1);
        if line.len() + used > limits.max {
            return Err(FrameError::TooLarge);
        }
        line.extend_from_slice(&buffer[..used]);
        if end.is_some() {
            return Ok(Some(line));
        }
        if line.len() == limits.max {
            return Err(FrameError::TooLarge);
        }
    }
}
pub(crate) fn write_frame(
    stream: &mut UnixStream,
    bytes: &[u8],
    limits: IoLimits,
) -> std::result::Result<(), FrameError> {
    if bytes.len() > limits.max {
        return Err(FrameError::TooLarge);
    }
    let start = Instant::now();
    let mut last = start;
    let mut offset = 0;
    while offset < bytes.len() {
        stream
            .set_write_timeout(Some(timeout(limits, start, last)?))
            .map_err(|e| io_error(&e))?;
        match stream.write(&bytes[offset..]) {
            Ok(0) => return Err(FrameError::Unavailable),
            Ok(n) => {
                timeout(limits, start, last)?;
                offset += n;
                last = Instant::now();
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
            Err(e) => return Err(io_error(&e)),
        }
    }
    Ok(())
}
fn owner_directory(path: &Path) -> Result<Directory> {
    use std::os::unix::fs::MetadataExt;
    let dir = Directory::open(path)?;
    let m = dir.0.metadata()?;
    if m.uid() != getuid().as_raw() || m.mode() & 0o7777 != 0o700 {
        return Err(Error("filesystem.unavailable"));
    }
    Ok(dir)
}
fn address(dir: &Directory) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}/{SOCKET}", dir.0.as_raw_fd()))
}
fn validate_socket(dir: &Directory) -> Result<nix::sys::stat::FileStat> {
    let s = dir.stat(Path::new(SOCKET))?;
    if s.st_mode & nix::libc::S_IFMT != nix::libc::S_IFSOCK
        || s.st_mode & 0o7777 != 0o600
        || s.st_uid != getuid().as_raw()
    {
        return Err(Error("filesystem.unavailable"));
    }
    Ok(s)
}
fn peer_is_owner(stream: &UnixStream, uid: u32) -> bool {
    getsockopt(stream, PeerCredentials).is_ok_and(|c| c.uid() == uid)
}
pub struct Server {
    listener: UnixListener,
    dir: Directory,
    identity: (u64, u64),
    service: Arc<Service>,
    limits: IoLimits,
}
impl Server {
    pub fn bind(runtime: &Path, service: Service) -> Result<Self> {
        validate_path(runtime)?;
        if runtime.join(SOCKET).as_os_str().as_encoded_bytes().len() > 107 {
            return Err(Error("validation.invalid_request"));
        }
        let dir = owner_directory(runtime)?;
        // Never unlink an existing entry, even a socket: explicit stale-socket repair only.
        if dir.exists(Path::new(SOCKET))? {
            return Err(Error("conflict.socket_exists"));
        }
        let listener = UnixListener::bind(address(&dir))?;
        let stat = dir.stat(Path::new(SOCKET))?;
        let server = Self {
            listener,
            dir,
            identity: (stat.st_dev, stat.st_ino),
            service: Arc::new(service),
            limits: IoLimits::default(),
        };
        fchmodat(
            &server.dir.0,
            SOCKET,
            Mode::S_IRUSR | Mode::S_IWUSR,
            FchmodatFlags::NoFollowSymlink,
        )?;
        validate_socket(&server.dir)?;
        server.listener.set_nonblocking(true)?;
        Ok(server)
    }
    pub fn serve_until(&self, stop: &AtomicBool) -> Result<()> {
        let mut workers: Vec<JoinHandle<()>> = Vec::new();
        let mut outcome = Ok(());
        while !stop.load(Ordering::Acquire) {
            let mut i = 0;
            while i < workers.len() {
                if workers[i].is_finished() {
                    let _ = workers.swap_remove(i).join();
                } else {
                    i += 1;
                }
            }
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if workers.len() >= 32 {
                        drop(stream);
                        continue;
                    }
                    let service = Arc::clone(&self.service);
                    let limits = self.limits;
                    if let Ok(worker) = thread::Builder::new()
                        .name("session-manager-request".into())
                        .spawn(move || {
                            serve_connection(stream, &service, getuid().as_raw(), limits);
                        })
                    {
                        workers.push(worker);
                    } else {
                        outcome = Err(Error("internal.error"));
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(_) => {
                    outcome = Err(Error("filesystem.unavailable"));
                    break;
                }
            }
        }
        for worker in workers {
            let _ = worker.join();
        }
        outcome
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if self.dir.stat(Path::new(SOCKET)).is_ok_and(|s| {
            (s.st_dev, s.st_ino) == self.identity
                && s.st_mode & nix::libc::S_IFMT == nix::libc::S_IFSOCK
        }) {
            let _ = self.dir.unlink(Path::new(SOCKET));
        }
    }
}
pub(crate) fn serve_connection(
    mut stream: UnixStream,
    service: &Service,
    uid: u32,
    limits: IoLimits,
) {
    if !peer_is_owner(&stream, uid) {
        return;
    }
    let response = match read_frame(&mut stream, limits) {
        Ok(Some(line)) => service.handle(&line),
        Err(FrameError::TooLarge) => Response::failure(None, Error("protocol.message_too_large")),
        Ok(None) | Err(_) => return,
    };
    if let Ok(bytes) = response.encode() {
        let _ = write_frame(&mut stream, &bytes, limits);
    }
}

pub fn call(runtime: &Path, command: &Command) -> std::result::Result<Value, WireError> {
    let id = Uuid::new_v4();
    let bytes = canonical_json::encode_line(&command.request(id))
        .map_err(|_| WireError::from(Error("internal.error")))?;
    if bytes.len() > MAX_LINE_BYTES {
        return Err(Error("protocol.message_too_large").into());
    }
    let mut stream =
        connect(runtime).map_err(|_| WireError::from(Error("transport.unavailable")))?;
    let result = (|| {
        write_frame(&mut stream, &bytes, IoLimits::default()).map_err(frame_code)?;
        let line = read_frame(&mut stream, IoLimits::default())
            .map_err(frame_code)?
            .ok_or(Error("transport.invalid_response"))?;
        Ok(line)
    })();
    let line = result.map_err(|e: Error| {
        WireError::from(if command.mutates() {
            Error("transport.uncertain")
        } else {
            e
        })
    })?;
    decode_response(&line, id).map_err(|error| {
        if command.mutates() && error.code == "transport.invalid_response" {
            Error("transport.uncertain").into()
        } else {
            error
        }
    })
}
fn frame_code(error: FrameError) -> Error {
    match error {
        FrameError::Timeout => Error("transport.timeout"),
        FrameError::TooLarge => Error("transport.invalid_response"),
        FrameError::Unavailable => Error("transport.unavailable"),
    }
}
fn connect_bounded(dir: &Directory) -> Result<UnixStream> {
    use nix::{
        errno::Errno,
        sys::socket::{
            AddressFamily, SockFlag, SockType, UnixAddr, connect as unix_connect, socket,
        },
    };
    let fd = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
        None,
    )?;
    let addr = UnixAddr::new(&address(dir))?;
    let start = Instant::now();
    loop {
        match unix_connect(fd.as_raw_fd(), &addr) {
            Ok(()) => break,
            Err(Errno::EAGAIN | Errno::EINTR) if start.elapsed() < Duration::from_secs(5) => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return Err(Error("transport.unavailable")),
        }
    }
    let stream = UnixStream::from(fd);
    stream.set_nonblocking(false)?;
    Ok(stream)
}
fn connect(runtime: &Path) -> Result<UnixStream> {
    let dir = owner_directory(runtime)?;
    let before = validate_socket(&dir)?;
    let stream = connect_bounded(&dir)?;
    let after = validate_socket(&dir)?;
    if !peer_is_owner(&stream, getuid().as_raw())
        || (before.st_dev, before.st_ino) != (after.st_dev, after.st_ino)
    {
        return Err(Error("transport.unavailable"));
    }
    Ok(stream)
}
pub(crate) fn decode_response(line: &[u8], id: Uuid) -> std::result::Result<Value, WireError> {
    let invalid = || WireError::from(Error("transport.invalid_response"));
    let value = strict_json(line).map_err(|_| invalid())?;
    let map = value
        .as_object()
        .filter(|m| m.len() == 4)
        .ok_or_else(invalid)?;
    if map.get("protocol_version").and_then(Value::as_u64) != Some(API_VERSION)
        || map.get("type").and_then(Value::as_str) != Some("response")
        || map.get("request_id").and_then(Value::as_str) != Some(id.to_string().as_str())
    {
        return Err(invalid());
    }
    if let Some(result) = map.get("result").filter(|v| v.is_object()) {
        return Ok(result.clone());
    }
    let error: WireError = serde_json::from_value(map.get("error").cloned().ok_or_else(invalid)?)
        .map_err(|_| invalid())?;
    if !error.valid() {
        return Err(invalid());
    }
    Err(error)
}
