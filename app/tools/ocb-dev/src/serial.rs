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
    port: Box<dyn SerialPort>,
    t0: Instant,
    tmp: [u8; 4096],
}

impl std::fmt::Debug for SerialIo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialIo")
            .field("port", &self.port.name())
            .finish()
    }
}

impl SerialIo {
    /// Opens `path` and asserts DTR.
    pub fn open(path: &str) -> Result<Self, String> {
        Ok(Self {
            path: path.to_owned(),
            port: Self::open_port(path)?,
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
        match self.port.read(&mut self.tmp) {
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
            Err(e) => Err(format!("serial read: {e} (bridge unplugged?)")),
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
        match Self::open_port(&self.path) {
            Ok(p) => {
                self.port = p;
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
        // The port timeout (1 ms) also bounds each write call: a full USB
        // buffer returns TimedOut, so retry until the whole chunk is out or
        // 1 s has passed (a stuck port, not back-pressure).
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut rest = bytes;
        while !rest.is_empty() {
            match self.port.write(rest) {
                Ok(0) => {}
                Ok(n) => rest = rest.get(n..).unwrap_or(&[]),
                Err(e) if e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock => {}
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(format!("serial write: {e} (bridge unplugged?)")),
            }
            if !rest.is_empty() && Instant::now() >= deadline {
                return Err("serial write: no progress for 1 s (bridge stuck?)".to_owned());
            }
        }
        Ok(())
    }
}

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
