//! The port adapter (Story 4.4 AC 4, 6, 7): the traits the worker talks to,
//! the real `serialport` backend, and the write and read rules carried over
//! from `ocb-dev`'s `SerialIo` (Stories 3.1, 3.4, 3.5).
//!
//! The worker never sees `serialport`: it lists and opens through
//! [`Backend`] and talks to an open port through [`OpenPort`], which wraps a
//! [`Port`]. Tests use the fakes of `crate::sim` (feature `sim`).

use std::fmt;
use std::io::{self, ErrorKind, Read as _, Write as _};
use std::time::Duration;

pub use ocb_core::device::{PortInfo, UsbIds};

use crate::worker::Clock;

/// The link's line rate (AD-4).
pub const BAUD: u32 = 460_800;

/// More bytes than this waiting in the OS output queue: the bridge is not
/// reading (about 90 ms of the line at 460800 baud). The chunk is dropped.
pub const OUT_QUEUE_LIMIT: u32 = 4096;

/// A chunk that makes no progress for this long is dropped, µs.
pub const WRITE_STALL_US: u64 = 1_000_000;

/// A serial port, as small as the worker needs (the `serialport::SerialPort`
/// calls it uses). Reads and writes return at most after the port timeout
/// (1 ms for the real port).
pub trait Port: Send {
    /// One read of at most the port timeout.
    ///
    /// # Errors
    /// The OS error; `TimedOut`, `WouldBlock` and `Interrupted` mean "no
    /// bytes yet".
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;

    /// One write call of at most the port timeout.
    ///
    /// # Errors
    /// The OS error; `TimedOut`, `WouldBlock` and `Interrupted` mean "no
    /// progress yet".
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize>;

    /// Bytes waiting in the OS output queue.
    ///
    /// # Errors
    /// The OS error.
    fn bytes_to_write(&self) -> io::Result<u32>;

    /// Sets the DTR line (AD-7: asserted while the session is open).
    ///
    /// # Errors
    /// The OS error.
    fn set_dtr(&mut self, on: bool) -> io::Result<()>;

    /// Drops what the OS has queued for reading.
    ///
    /// # Errors
    /// The OS error.
    fn clear_input(&mut self) -> io::Result<()>;
}

/// Why a port could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The OS refused access (Linux: the user is not in the node's group).
    PermissionDenied,
    /// Another program holds the port (it is still listed).
    Busy,
    /// The port is no longer there.
    Gone,
    /// Anything else.
    Other(String),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied => f.write_str("permission denied"),
            Self::Busy => f.write_str("port busy (held by another program)"),
            Self::Gone => f.write_str("port gone"),
            Self::Other(e) => f.write_str(e),
        }
    }
}

/// Lists and opens ports.
pub trait Backend: Send {
    /// Every serial port the OS lists now.
    ///
    /// # Errors
    /// The enumeration error, as text.
    fn list(&mut self) -> Result<Vec<PortInfo>, String>;

    /// Opens `name` at 460800 8N1, no flow control, 1 ms timeout. DTR and
    /// the input buffer are the worker's (it asserts DTR, then clears).
    ///
    /// # Errors
    /// [`OpenError`].
    fn open(&mut self, name: &str) -> Result<Box<dyn Port>, OpenError>;

    /// The group that owns the device node (Linux), for the permission
    /// message; `None` when it cannot be read.
    fn node_group(&self, name: &str) -> Option<String> {
        let _ = name;
        None
    }
}

/// A read or write ended the port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortLost(pub String);

/// Time [`OpenPort::flush`] may spend writing in one worker pass, µs. Each
/// write call is bounded by the port timeout (1 ms), and a call that makes
/// no progress ends the flush, so a port that does not drain never holds
/// the worker for more than about this long per pass (scans, shutdown and
/// the log stay on time).
pub const WRITE_BUDGET_US: u64 = 2_000;

/// An open port with the `SerialIo` rules (E3-A5): any hard error drops
/// the handle at once, writes never block (an own bounded queue, flushed a
/// little per pass), reads take at most the port timeout.
pub struct OpenPort {
    /// `None` after a hard error: the descriptor is dropped at once, so its
    /// exclusive lock never blocks the re-open (Story 3.5 review) and Linux
    /// does not renumber the replugged port.
    port: Option<Box<dyn Port>>,
    /// Bytes handed over and not yet accepted by the OS.
    pending: Vec<u8>,
    /// When the OS last accepted a byte while `pending` was not empty.
    last_progress_us: Option<u64>,
    /// Chunks not queued (own queue over [`OUT_QUEUE_LIMIT`], or the OS
    /// queue reported over it) or abandoned after [`WRITE_STALL_US`]
    /// without progress.
    pub dropped_chunks: u64,
}

impl fmt::Debug for OpenPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenPort")
            .field("open", &self.port.is_some())
            .field("pending", &self.pending.len())
            .field("dropped_chunks", &self.dropped_chunks)
            .finish()
    }
}

fn soft(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted
    )
}

impl OpenPort {
    /// Wraps an open port.
    #[must_use]
    pub fn new(port: Box<dyn Port>) -> Self {
        Self {
            port: Some(port),
            pending: Vec::new(),
            last_progress_us: None,
            dropped_chunks: 0,
        }
    }

    /// The handle is still held.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.port.is_some()
    }

    /// Bytes waiting in the own queue.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Lowers DTR (best effort: AD-7, the controller sees the session end
    /// even where the OS keeps DTR on close), then drops the handle.
    pub fn close(&mut self) {
        if let Some(mut p) = self.port.take() {
            let _ = p.set_dtr(false);
        }
        self.pending.clear();
    }

    fn lost(&mut self, what: &str, e: &io::Error) -> PortLost {
        self.port = None;
        self.pending.clear();
        PortLost(format!("{what}: {e}"))
    }

    /// Asserts DTR, then clears the OS input buffer: a STATUS queued from a
    /// controller since unplugged is not this session's (Story 3.5 H.3).
    ///
    /// # Errors
    /// [`PortLost`] if DTR cannot be set (the clear is best effort).
    pub fn start(&mut self) -> Result<(), PortLost> {
        let Some(port) = self.port.as_mut() else {
            return Err(PortLost("the port is closed".to_owned()));
        };
        if let Err(e) = port.set_dtr(true) {
            return Err(self.lost("DTR", &e));
        }
        let _ = port.clear_input();
        Ok(())
    }

    /// One read of at most the port timeout into `buf`; 0 when nothing came.
    ///
    /// # Errors
    /// [`PortLost`] on any error other than timeout, would-block or
    /// interrupted (the handle is dropped first).
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, PortLost> {
        let Some(port) = self.port.as_mut() else {
            return Err(PortLost("the port is closed".to_owned()));
        };
        match port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if soft(&e) => Ok(0),
            Err(e) => Err(self.lost("read", &e)),
        }
    }

    /// Queues one chunk (a frame with its `00`, or a lone `00`) for
    /// [`OpenPort::flush`]. Returns `false` when the chunk was dropped:
    /// the own queue would pass [`OUT_QUEUE_LIMIT`], or the OS reports more
    /// than that queued. The own queue is what counts: Linux cdc-acm and
    /// Windows usbser do not report a real OS queue. The session resends
    /// what matters (HELLO, STOP, go-back-N), so a dropped chunk is never
    /// waited for: while the bridge has no controller link it stops reading
    /// USB (Story 3.1, Story 3.5 review).
    pub fn queue(&mut self, bytes: &[u8]) -> bool {
        let Some(port) = self.port.as_ref() else {
            return false;
        };
        let os_full = port.bytes_to_write().is_ok_and(|q| q > OUT_QUEUE_LIMIT);
        if os_full || self.pending.len() + bytes.len() > OUT_QUEUE_LIMIT as usize {
            self.dropped_chunks += 1;
            return false;
        }
        self.pending.extend_from_slice(bytes);
        true
    }

    /// Writes from the own queue for at most [`WRITE_BUDGET_US`] of
    /// `clock` (a write call that makes no progress ends it at once). When
    /// the queue has made no progress for [`WRITE_STALL_US`], measured
    /// across passes, what is left is abandoned and counted as one dropped
    /// chunk; a lone `00` is queued next, so a frame cut short is closed
    /// and the receiver discards it.
    ///
    /// # Errors
    /// [`PortLost`] on a hard error (the handle is dropped first).
    pub fn flush(&mut self, clock: &dyn Clock) -> Result<(), PortLost> {
        if self.pending.is_empty() {
            self.last_progress_us = None;
            return Ok(());
        }
        let start = clock.now_us();
        let mut since = *self.last_progress_us.get_or_insert(start);
        while !self.pending.is_empty() {
            let Some(port) = self.port.as_mut() else {
                return Err(PortLost("the port is closed".to_owned()));
            };
            match port.write(&self.pending) {
                Ok(n) if n > 0 => {
                    self.pending.drain(..n.min(self.pending.len()));
                    since = clock.now_us();
                }
                Ok(_) => break,
                Err(e) if soft(&e) => break,
                Err(e) => return Err(self.lost("write", &e)),
            }
            if clock.now_us().saturating_sub(start) >= WRITE_BUDGET_US {
                break;
            }
        }
        if self.pending.is_empty() {
            self.last_progress_us = None;
            return Ok(());
        }
        self.last_progress_us = Some(since);
        if clock.now_us().saturating_sub(since) >= WRITE_STALL_US {
            self.pending.clear();
            self.pending.push(0);
            self.last_progress_us = None;
            self.dropped_chunks += 1;
        }
        Ok(())
    }
}

/// The message of a Linux permission denial (UX-DR65, Story 4.4 AC 7).
#[must_use]
pub fn permission_message(port: &str, group: &str) -> String {
    format!(
        "Bonooru can't open {port}: your user is not in the {group} group. \
         Run: sudo usermod -aG {group} $USER — then log out and back in."
    )
}

/// The group used when the node's group cannot be read (Debian, Ubuntu,
/// Fedora; Arch uses `uucp`).
pub const DEFAULT_SERIAL_GROUP: &str = "dialout";

/// The name of group `gid` in an `/etc/group` text (`name:x:gid:members`).
#[must_use]
pub fn group_name(etc_group: &str, gid: u32) -> Option<String> {
    etc_group.lines().find_map(|line| {
        let mut f = line.split(':');
        let name = f.next()?.trim();
        let _password = f.next()?;
        let id: u32 = f.next()?.trim().parse().ok()?;
        (id == gid && !name.is_empty() && !name.starts_with('#')).then(|| name.to_owned())
    })
}

/// The group owning a device node (Linux only; `None` elsewhere).
#[must_use]
pub fn node_group(path: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt as _;
        let gid = std::fs::metadata(path).ok()?.gid();
        group_name(&std::fs::read_to_string("/etc/group").ok()?, gid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}

/// The real backend: `serialport` 4.10.1.
#[derive(Debug, Default)]
pub struct SerialBackend {
    /// Names of the last listing (an open failing with `NoDevice` on a
    /// listed port is "busy", on an unlisted one "gone").
    last_names: Vec<String>,
}

/// One listed port and, on Windows, the product string `serialport`
/// reported before the adapter dropped it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawListing {
    /// The port as the worker sees it.
    pub info: PortInfo,
    /// What the OS reported as the product, where the adapter ignores it
    /// (Windows: the driver's display name, e.g. "USB Serial Device
    /// (COM5)"); `None` elsewhere.
    pub product_raw: Option<String>,
}

struct SerialPortBox(Box<dyn serialport::SerialPort>);

fn io_err(e: serialport::Error) -> io::Error {
    match e.kind {
        serialport::ErrorKind::Io(kind) => io::Error::new(kind, e.description),
        _ => io::Error::other(e.description),
    }
}

impl Port for SerialPortBox {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }

    fn bytes_to_write(&self) -> io::Result<u32> {
        self.0.bytes_to_write().map_err(io_err)
    }

    fn set_dtr(&mut self, on: bool) -> io::Result<()> {
        self.0.write_data_terminal_ready(on).map_err(io_err)
    }

    fn clear_input(&mut self) -> io::Result<()> {
        self.0.clear(serialport::ClearBuffer::Input).map_err(io_err)
    }
}

impl SerialBackend {
    /// A backend with no listing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lists the ports with the raw product string kept (for `ocb-probe
    /// list`, H.28).
    ///
    /// # Errors
    /// The enumeration error, as text.
    pub fn list_raw(&mut self) -> Result<Vec<RawListing>, String> {
        let ports = serialport::available_ports().map_err(|e| e.to_string())?;
        let out: Vec<RawListing> = ports.into_iter().map(listing).collect();
        self.last_names = out.iter().map(|l| l.info.name.clone()).collect();
        Ok(out)
    }
}

fn listing(p: serialport::SerialPortInfo) -> RawListing {
    match p.port_type {
        serialport::SerialPortType::UsbPort(u) => {
            // Windows: `serialport` fills `product` with SPDRP_FRIENDLYNAME,
            // the driver's display name ("USB Serial Device (COM5)" with the
            // inbox usbser.sys), not the USB iProduct string (Story 4.4 Dev
            // Notes › Cross-OS discovery, D3). It is never compared.
            let (product, product_raw) = if cfg!(windows) {
                (None, u.product)
            } else {
                (u.product, None)
            };
            RawListing {
                info: PortInfo {
                    name: p.port_name,
                    usb: Some(UsbIds {
                        vid: u.vid,
                        pid: u.pid,
                        serial: u.serial_number,
                        manufacturer: u.manufacturer,
                        product,
                    }),
                },
                product_raw,
            }
        }
        _ => RawListing {
            info: PortInfo {
                name: p.port_name,
                usb: None,
            },
            product_raw: None,
        },
    }
}

/// Whether `name` is still listed: a fresh listing when there is one,
/// else the last scan's.
#[must_use]
pub fn still_listed(fresh: Option<&[String]>, last: &[String], name: &str) -> bool {
    fresh.unwrap_or(last).iter().any(|n| n == name)
}

/// Maps a `serialport` open error (Story 4.4 Dev Notes › Cross-OS
/// discovery): `EACCES` is a permission denial; `NoDevice` (Linux/macOS
/// `EBUSY` from `TIOCEXCL` or a held `flock`, Windows
/// `ERROR_ACCESS_DENIED`) is "busy" while the port is listed and "gone"
/// otherwise.
#[must_use]
pub fn map_open_error(e: &serialport::Error, listed: bool) -> OpenError {
    match e.kind {
        serialport::ErrorKind::Io(ErrorKind::PermissionDenied) => OpenError::PermissionDenied,
        serialport::ErrorKind::NoDevice if listed => OpenError::Busy,
        serialport::ErrorKind::NoDevice | serialport::ErrorKind::Io(ErrorKind::NotFound) => {
            OpenError::Gone
        }
        _ => OpenError::Other(e.description.clone()),
    }
}

impl Backend for SerialBackend {
    fn list(&mut self) -> Result<Vec<PortInfo>, String> {
        Ok(self.list_raw()?.into_iter().map(|l| l.info).collect())
    }

    fn open(&mut self, name: &str) -> Result<Box<dyn Port>, OpenError> {
        let port = serialport::new(name, BAUD)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(1))
            .open()
            .map_err(|e| {
                // Windows maps ERROR_FILE_NOT_FOUND (unplugged) and
                // ERROR_ACCESS_DENIED (held) both to `NoDevice`: only a
                // fresh listing tells them apart. POSIX `NoDevice` is
                // EBUSY/flock (held); a missing node is `Io(NotFound)`.
                let fresh = (cfg!(windows) && e.kind == serialport::ErrorKind::NoDevice)
                    .then(|| {
                        serialport::available_ports()
                            .ok()
                            .map(|l| l.into_iter().map(|p| p.port_name).collect::<Vec<_>>())
                    })
                    .flatten();
                map_open_error(&e, still_listed(fresh.as_deref(), &self.last_names, name))
            })?;
        Ok(Box::new(SerialPortBox(port)))
    }

    fn node_group(&self, name: &str) -> Option<String> {
        node_group(name)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::worker::FakeClock;

    /// A `SerialPort`-like fake: a fixed OS queue figure, writes that
    /// accept `accept` bytes per call (0: no progress), each call taking
    /// 1 ms of fake time like the real 1 ms timeout.
    struct QueuePort {
        queued: u32,
        accept: usize,
        clock: FakeClock,
        calls: Arc<Mutex<Vec<usize>>>,
        dtr: Arc<Mutex<Vec<bool>>>,
        fail: Option<ErrorKind>,
    }

    impl Port for QueuePort {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            self.clock.advance(1_000);
            match self.fail {
                Some(k) => Err(io::Error::from(k)),
                None => Ok(0),
            }
        }
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.clock.advance(1_000);
            self.calls.lock().unwrap().push(bytes.len());
            if let Some(k) = self.fail {
                return Err(io::Error::from(k));
            }
            if self.accept == 0 {
                return Err(io::Error::from(ErrorKind::TimedOut));
            }
            Ok(self.accept.min(bytes.len()))
        }
        fn bytes_to_write(&self) -> io::Result<u32> {
            Ok(self.queued)
        }
        fn set_dtr(&mut self, on: bool) -> io::Result<()> {
            self.dtr.lock().unwrap().push(on);
            Ok(())
        }
        fn clear_input(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct Rig {
        p: OpenPort,
        clock: FakeClock,
        calls: Arc<Mutex<Vec<usize>>>,
        dtr: Arc<Mutex<Vec<bool>>>,
    }

    fn port(queued: u32, accept: usize, fail: Option<ErrorKind>) -> Rig {
        let clock = FakeClock::at(0);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dtr = Arc::new(Mutex::new(Vec::new()));
        let p = OpenPort::new(Box::new(QueuePort {
            queued,
            accept,
            clock: clock.clone(),
            calls: Arc::clone(&calls),
            dtr: Arc::clone(&dtr),
            fail,
        }));
        Rig {
            p,
            clock,
            calls,
            dtr,
        }
    }

    #[test]
    fn os_queue_over_limit_drops_without_writing() {
        let mut r = port(OUT_QUEUE_LIMIT + 1, 64, None);
        assert!(!r.p.queue(&[1, 2, 3, 0]));
        assert_eq!(r.p.dropped_chunks, 1);
        r.p.flush(&r.clock).unwrap();
        assert!(r.calls.lock().unwrap().is_empty(), "no write call");
        let mut r = port(OUT_QUEUE_LIMIT, 64, None);
        assert!(r.p.queue(&[1, 2, 3, 0]));
        r.p.flush(&r.clock).unwrap();
        assert_eq!(*r.calls.lock().unwrap(), [4]);
    }

    #[test]
    fn own_queue_bounds_writes_when_the_os_reports_nothing() {
        // Linux cdc-acm / Windows usbser: the OS figure stays 0 while the
        // device does not read. The own queue fills and then drops.
        let mut r = port(0, 0, None);
        let chunk = [7u8; 100];
        let mut queued = 0;
        for _ in 0..50 {
            if r.p.queue(&chunk) {
                queued += 1;
            }
            r.p.flush(&r.clock).unwrap();
        }
        assert_eq!(queued, 40, "4096 B / 100 B");
        assert_eq!(r.p.dropped_chunks, 10);
        assert!(r.p.pending() <= OUT_QUEUE_LIMIT as usize);
    }

    #[test]
    fn flush_never_takes_more_than_its_budget() {
        // A port that accepts 1 byte per 1 ms call.
        let mut r = port(0, 1, None);
        assert!(r.p.queue(&[9u8; 1000]));
        let t0 = r.clock.now_us();
        r.p.flush(&r.clock).unwrap();
        assert!(
            r.clock.now_us() - t0 <= WRITE_BUDGET_US,
            "{}",
            r.clock.now_us() - t0
        );
        assert_eq!(r.p.pending(), 998);
        // A port that makes no progress: one call, then the pass goes on.
        let mut r = port(0, 0, None);
        assert!(r.p.queue(&[1, 2, 3, 0]));
        let t0 = r.clock.now_us();
        r.p.flush(&r.clock).unwrap();
        assert_eq!(r.clock.now_us() - t0, 1_000);
        assert_eq!(r.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn stall_measured_across_passes_drops_after_one_second() {
        let mut r = port(0, 0, None);
        assert!(r.p.queue(&[1, 2, 3, 0]));
        let mut passes = 0;
        while r.p.dropped_chunks == 0 {
            r.p.flush(&r.clock).unwrap();
            passes += 1;
            assert!(passes < 2000, "never dropped");
        }
        assert_eq!(
            r.clock.now_us(),
            WRITE_STALL_US,
            "dropped after 1 s of no progress"
        );
        assert_eq!(passes, 1000, "1 ms per pass");
        assert_eq!(r.p.pending(), 1, "a lone 00 closes the cut frame");
        assert!(r.p.is_open(), "a stall is not a loss");
    }

    #[test]
    fn partial_writes_complete_over_passes() {
        let mut r = port(0, 2, None);
        assert!(r.p.queue(&[1, 2, 3, 4, 0]));
        r.p.flush(&r.clock).unwrap();
        r.p.flush(&r.clock).unwrap();
        assert_eq!(*r.calls.lock().unwrap(), [5, 3, 1]);
        assert_eq!(r.p.pending(), 0);
    }

    #[test]
    fn close_lowers_dtr_then_drops() {
        let mut r = port(0, 64, None);
        r.p.start().unwrap();
        r.p.close();
        assert_eq!(*r.dtr.lock().unwrap(), [true, false]);
        assert!(!r.p.is_open());
    }

    #[test]
    fn hard_errors_drop_the_handle() {
        let mut r = port(0, 64, Some(ErrorKind::BrokenPipe));
        assert!(r.p.queue(&[1, 0]));
        assert!(r.p.flush(&r.clock).is_err());
        assert!(!r.p.is_open(), "handle dropped at once");
        assert!(r.p.read(&mut [0; 8]).is_err());
        let mut r = port(0, 64, Some(ErrorKind::Other));
        assert!(r.p.read(&mut [0; 8]).is_err());
        assert!(!r.p.is_open());
    }

    #[test]
    fn soft_read_errors_are_no_bytes() {
        for k in [
            ErrorKind::TimedOut,
            ErrorKind::WouldBlock,
            ErrorKind::Interrupted,
        ] {
            let mut r = port(0, 64, Some(k));
            assert_eq!(r.p.read(&mut [0; 8]), Ok(0));
            assert!(r.p.is_open());
        }
    }

    #[test]
    fn permission_message() {
        assert_eq!(
            super::permission_message("/dev/ttyACM0", "dialout"),
            "Bonooru can't open /dev/ttyACM0: your user is not in the dialout group. \
             Run: sudo usermod -aG dialout $USER — then log out and back in."
        );
        assert_eq!(
            super::permission_message("/dev/ttyACM0", "uucp"),
            "Bonooru can't open /dev/ttyACM0: your user is not in the uucp group. \
             Run: sudo usermod -aG uucp $USER — then log out and back in."
        );
        assert_eq!(DEFAULT_SERIAL_GROUP, "dialout");
    }

    #[test]
    fn group_lookup() {
        let etc = "root:x:0:\n# comment:x:5:\ndialout:x:20:alice\nuucp:x:14:\nbad line\n";
        assert_eq!(group_name(etc, 20).as_deref(), Some("dialout"));
        assert_eq!(group_name(etc, 14).as_deref(), Some("uucp"));
        assert_eq!(group_name(etc, 5), None);
        assert_eq!(group_name(etc, 99), None);
    }

    #[test]
    fn fresh_listing_decides_busy_or_gone() {
        let last = vec!["COM5".to_owned()];
        let fresh_without: Vec<String> = vec!["COM3".to_owned()];
        assert!(
            !still_listed(Some(&fresh_without), &last, "COM5"),
            "unplugged"
        );
        assert!(
            still_listed(None, &last, "COM5"),
            "no fresh listing: the last scan"
        );
        assert!(still_listed(Some(&last), &[], "COM5"));
    }

    #[test]
    fn open_error_mapping() {
        let e = |kind| serialport::Error::new(kind, "x");
        assert_eq!(
            map_open_error(
                &e(serialport::ErrorKind::Io(ErrorKind::PermissionDenied)),
                true
            ),
            OpenError::PermissionDenied
        );
        assert_eq!(
            map_open_error(&e(serialport::ErrorKind::NoDevice), true),
            OpenError::Busy
        );
        assert_eq!(
            map_open_error(&e(serialport::ErrorKind::NoDevice), false),
            OpenError::Gone
        );
        assert_eq!(
            map_open_error(&e(serialport::ErrorKind::Io(ErrorKind::NotFound)), true),
            OpenError::Gone
        );
        assert_eq!(
            map_open_error(&e(serialport::ErrorKind::Unknown), true),
            OpenError::Other("x".to_owned())
        );
    }
}
