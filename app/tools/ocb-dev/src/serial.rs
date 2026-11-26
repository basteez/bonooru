//! The real port (the only `serialport` user): 460800 8N1, no flow control,
//! 1 ms read timeout, DTR asserted on open (AD-7), and a monotonic clock.

use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

use crate::runner::Io;

/// The link's line rate (AD-4; 921600 is a build-constant change, M4).
pub const BAUD: u32 = 460_800;

/// An open serial port and the clock.
pub struct SerialIo {
    path: String,
    /// `None` after a port failure: the old descriptor is dropped at once,
    /// so its exclusive lock never blocks the re-open (Story 3.5 review).
    port: Option<Box<dyn SerialPort>>,
    /// Chunks not written because the OS output queue was full (the bridge
    /// stops reading USB while it has no controller link).
    pub dropped_chunks: u64,
    t0: Instant,
    tmp: [u8; 4096],
}

impl std::fmt::Debug for SerialIo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialIo")
            .field("path", &self.path)
            .finish()
    }
}

impl SerialIo {
    /// Opens `path` and asserts DTR.
    pub fn open(path: &str) -> Result<Self, String> {
        Ok(Self {
            path: path.to_owned(),
            port: Some(Self::open_port(path)?),
            dropped_chunks: 0,
            t0: Instant::now(),
            tmp: [0; 4096],
        })
    }

    fn open_port(path: &str) -> Result<Box<dyn SerialPort>, String> {
        let mut port = serialport::new(path, BAUD)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .flow_control(FlowControl::None)
            .timeout(Duration::from_millis(1))
            .open()
            .map_err(|e| format!("{path}: {e}"))?;
        port.write_data_terminal_ready(true)
            .map_err(|e| format!("{path}: DTR: {e}"))?;
        // Bytes the OS queued before this open (a STATUS of a controller
        // since unplugged) are not this session's (Story 3.5 H.3 bench).
        let _ = port.clear(serialport::ClearBuffer::Input);
        Ok(port)
    }
}

impl Io for SerialIo {
    fn now_us(&mut self) -> u64 {
        u64::try_from(self.t0.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    fn read(&mut self, until_us: u64, buf: &mut Vec<u8>) -> Result<(), String> {
        // One read of at most the port timeout (1 ms); when the caller's
        // deadline has passed, only take what is already there.
        let _ = until_us;
        let Some(port) = self.port.as_mut() else {
            return Err("serial read: the port is closed (bridge unplugged)".to_owned());
        };
        match port.read(&mut self.tmp) {
            Ok(n) => {
                buf.extend_from_slice(self.tmp.get(..n).unwrap_or(&[]));
                Ok(())
            }
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted
                ) =>
            {
                Ok(())
            }
            Err(e) => {
                self.port = None;
                Err(format!("serial read: {e} (bridge unplugged?)"))
            }
        }
    }

    fn reopen(&mut self) -> Result<bool, String> {
        if !std::path::Path::new(&self.path).exists() {
            // macOS names the port after the USB location, and the name
            // changes with the plug order (Story 3.1 H.1: usbmodem101 or
            // usbmodem1101). The controller has no CDC port, so a single
            // `/dev/cu.usbmodem*` is the bridge.
            match single_usbmodem(&self.path) {
                Some(p) => self.path = p,
                None => return Ok(false),
            }
        }
        // The node can appear before it opens: a failed open is "not yet".
        self.port = None;
        match Self::open_port(&self.path) {
            Ok(p) => {
                self.port = Some(p);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    fn idle_until(&mut self, t_us: u64) {
        let now = self.now_us();
        if t_us > now {
            std::thread::sleep(Duration::from_micros(t_us - now));
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(port) = self.port.as_mut() else {
            return Err("serial write: the port is closed (bridge unplugged)".to_owned());
        };
        // While the bridge has no controller link it stops reading USB
        // (Story 3.1: nothing leaves its UART), so the OS queue fills. The
        // session resends everything that matters (HELLO, STOP, go-back-N),
        // so a chunk that cannot be queued is dropped, not waited for: a
        // blocked write would stall the tool for nothing (Story 3.5 review).
        if port.bytes_to_write().is_ok_and(|q| q > OUT_QUEUE_LIMIT) {
            self.dropped_chunks += 1;
            return Ok(());
        }
        // The port timeout (1 ms) also bounds each write call; a chunk that
        // makes no progress for 1 s is dropped the same way (the bytes
        // already out end as a damaged frame the receiver discards).
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut rest = bytes;
        while !rest.is_empty() {
            match port.write(rest) {
                Ok(0) => {}
                Ok(n) => rest = rest.get(n..).unwrap_or(&[]),
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted
                    ) => {}
                Err(e) => {
                    self.port = None;
                    return Err(format!("serial write: {e} (bridge unplugged?)"));
                }
            }
            if !rest.is_empty() && Instant::now() >= deadline {
                self.dropped_chunks += 1;
                return Ok(());
            }
        }
        Ok(())
    }
}

/// More bytes than this waiting in the OS output queue: the bridge is not
/// reading (about 90 ms of the line at 460800 baud).
const OUT_QUEUE_LIMIT: u32 = 4096;

/// The only `/dev/cu.usbmodem*` node, when `old` was one too.
fn single_usbmodem(old: &str) -> Option<String> {
    const PREFIX: &str = "cu.usbmodem";
    if !old.starts_with("/dev/cu.usbmodem") {
        return None;
    }
    let mut found = std::fs::read_dir("/dev")
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(PREFIX));
    let first = found.next()?;
    if found.next().is_some() {
        return None;
    }
    Some(format!("/dev/{first}"))
}
