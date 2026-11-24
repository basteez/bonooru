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
        Ok(Self {
            port,
            t0: Instant::now(),
            tmp: [0; 4096],
        })
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
