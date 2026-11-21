//! Serial port discovery and a background read-only port worker. GUI-free.
//!
//! SAFETY RULE FOR THIS SPIKE: nothing in this module writes to a serial port and nothing
//! ever sets 1200 baud (that would reboot a board into BOOTSEL). The worker only reads.

use crossbeam_channel::{Receiver, Sender, select, tick, unbounded};
use std::io::{self, Read};
use std::thread;
use std::time::Duration;

pub const RP2040_VID: u16 = 0x2E8A;
/// Baud used when opening read-only. USB CDC ignores it, but it must never be 1200.
pub const READ_BAUD: u32 = 115_200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortInfo {
    pub name: String,
    pub usb: Option<UsbInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbInfo {
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Match {
    None,
    Rp2040,
    /// Product string matches the configured filter (e.g. "Bonooru Bridge").
    Product,
}

/// Classify a port. Product match wins over plain RP2040 VID match.
pub fn classify(port: &PortInfo, product_filter: &str) -> Match {
    let Some(usb) = &port.usb else {
        return Match::None;
    };
    let filter = product_filter.trim().to_lowercase();
    if !filter.is_empty()
        && usb
            .product
            .as_deref()
            .is_some_and(|p| p.to_lowercase().contains(&filter))
    {
        return Match::Product;
    }
    if usb.vid == RP2040_VID {
        return Match::Rp2040;
    }
    Match::None
}

/// Enumerate serial ports via the `serialport` crate.
pub fn list_ports() -> Result<Vec<PortInfo>, String> {
    let mut ports: Vec<PortInfo> = serialport::available_ports()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|p| PortInfo {
            usb: match p.port_type {
                serialport::SerialPortType::UsbPort(u) => Some(UsbInfo {
                    vid: u.vid,
                    pid: u.pid,
                    serial: u.serial_number,
                    manufacturer: u.manufacturer,
                    product: u.product,
                }),
                _ => None,
            },
            name: p.port_name,
        })
        .collect();
    ports.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ports)
}

/// Open a port for reading only. DTR is left untouched where the OS allows it.
pub fn open_read_only(name: &str) -> io::Result<Box<dyn Read + Send>> {
    debug_assert_ne!(READ_BAUD, 1200);
    let port = serialport::new(name, READ_BAUD)
        .timeout(Duration::from_millis(100))
        .preserve_dtr_on_open()
        .open()
        .map_err(io::Error::other)?;
    Ok(Box::new(port))
}

pub enum Command {
    Rescan,
    Open(String),
    Close,
    Shutdown,
}

#[derive(Debug)]
pub enum Event {
    Ports(Vec<PortInfo>),
    Opened(String),
    Rx { total: usize, preview: String },
    Closed(String),
    Error(String),
}

pub struct WorkerHandle {
    pub tx: Sender<Command>,
    pub rx: Receiver<Event>,
    join: Option<thread::JoinHandle<()>>,
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

type Lister = Box<dyn Fn() -> Result<Vec<PortInfo>, String> + Send>;
type Opener = Box<dyn Fn(&str) -> io::Result<Box<dyn Read + Send>> + Send>;
type Waker = Box<dyn Fn() + Send>;

/// Spawn the worker with the real serialport backend.
pub fn spawn(rescan_every: Duration, wake: impl Fn() + Send + 'static) -> WorkerHandle {
    spawn_with(
        rescan_every,
        Box::new(list_ports),
        Box::new(open_read_only),
        Box::new(wake),
    )
}

/// Spawn with injectable backends (used by tests and by a "fake device" mode).
pub fn spawn_with(
    rescan_every: Duration,
    lister: Lister,
    opener: Opener,
    wake: Waker,
) -> WorkerHandle {
    let (cmd_tx, cmd_rx) = unbounded::<Command>();
    let (ev_tx, ev_rx) = unbounded::<Event>();
    let join = thread::Builder::new()
        .name("serial-worker".into())
        .spawn(move || worker_loop(rescan_every, lister, opener, wake, cmd_rx, ev_tx))
        .expect("spawn serial worker");
    WorkerHandle {
        tx: cmd_tx,
        rx: ev_rx,
        join: Some(join),
    }
}

fn worker_loop(
    rescan_every: Duration,
    lister: Lister,
    opener: Opener,
    wake: Waker,
    cmd_rx: Receiver<Command>,
    ev_tx: Sender<Event>,
) {
    let send = |e: Event| {
        let _ = ev_tx.send(e);
        wake();
    };
    let mut last_ports: Option<Vec<PortInfo>> = None;
    let mut rescan = |force: bool| match lister() {
        Ok(p) => {
            if force || last_ports.as_ref() != Some(&p) {
                last_ports = Some(p.clone());
                send(Event::Ports(p));
            }
        }
        Err(e) => send(Event::Error(e)),
    };
    rescan(true);
    let ticker = tick(rescan_every);
    let mut port: Option<Box<dyn Read + Send>> = None;
    let mut total = 0usize;
    let mut buf = [0u8; 256];
    loop {
        if let Some(p) = port.as_mut() {
            // Blocking read with a short timeout, then poll commands without blocking.
            match p.read(&mut buf) {
                Ok(0) => {
                    port = None;
                    send(Event::Closed("EOF".into()));
                }
                Ok(n) => {
                    total += n;
                    let preview = buf[..n.min(16)]
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    send(Event::Rx { total, preview });
                }
                Err(e) if e.kind() == io::ErrorKind::TimedOut => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    port = None;
                    send(Event::Closed(format!("read error: {e}")));
                }
            }
            if ticker.try_recv().is_ok() {
                rescan(false);
            }
            match cmd_rx.try_recv() {
                Ok(c) => {
                    if handle(c, &mut port, &mut total, &opener, &send, &mut rescan) {
                        return;
                    }
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => return,
                Err(_) => {}
            }
        } else {
            select! {
                recv(cmd_rx) -> c => match c {
                    Ok(c) => if handle(c, &mut port, &mut total, &opener, &send, &mut rescan) { return },
                    Err(_) => return,
                },
                recv(ticker) -> _ => rescan(false),
            }
        }
    }
}

/// Returns true on shutdown.
fn handle(
    c: Command,
    port: &mut Option<Box<dyn Read + Send>>,
    total: &mut usize,
    opener: &Opener,
    send: &dyn Fn(Event),
    rescan: &mut dyn FnMut(bool),
) -> bool {
    match c {
        Command::Shutdown => return true,
        Command::Rescan => rescan(true),
        Command::Close => {
            if port.take().is_some() {
                send(Event::Closed("closed by user".into()));
            }
        }
        Command::Open(name) => {
            *port = None;
            *total = 0;
            match opener(&name) {
                Ok(p) => {
                    *port = Some(p);
                    send(Event::Opened(name));
                }
                Err(e) => send(Event::Error(format!("open {name}: {e}"))),
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn usb(vid: u16, product: &str) -> PortInfo {
        PortInfo {
            name: "/dev/x".into(),
            usb: Some(UsbInfo {
                vid,
                pid: 0x000a,
                serial: Some("E66".into()),
                manufacturer: None,
                product: Some(product.into()),
            }),
        }
    }

    #[test]
    fn classify_ports() {
        assert_eq!(
            classify(&usb(0x2E8A, "Bonooru Bridge"), "bonooru bridge"),
            Match::Product
        );
        assert_eq!(
            classify(&usb(0x2E8A, "RiiDraw"), "Bonooru Bridge"),
            Match::Rp2040
        );
        assert_eq!(
            classify(&usb(0x1234, "Bonooru Bridge"), "Bonooru"),
            Match::Product
        );
        assert_eq!(classify(&usb(0x1234, "Other"), "Bonooru"), Match::None);
        assert_eq!(classify(&usb(0x1234, "Other"), ""), Match::None);
        let plain = PortInfo {
            name: "/dev/ttyS0".into(),
            usb: None,
        };
        assert_eq!(classify(&plain, "x"), Match::None);
    }

    #[test]
    fn worker_reports_ports_and_reads_fake_device() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let w2 = wakes.clone();
        let h = spawn_with(
            Duration::from_millis(20),
            Box::new(|| Ok(vec![usb(0x2E8A, "Bonooru Bridge")])),
            Box::new(|_name| {
                Ok(Box::new(io::Cursor::new(vec![0x81u8, 0x01, 0x02])) as Box<dyn Read + Send>)
            }),
            Box::new(move || {
                w2.fetch_add(1, Ordering::SeqCst);
            }),
        );
        let first = h.rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(first, Event::Ports(ref p) if p.len() == 1));
        h.tx.send(Command::Open("/dev/x".into())).unwrap();
        let mut got_rx = false;
        let mut got_closed = false;
        while let Ok(ev) = h.rx.recv_timeout(Duration::from_secs(2)) {
            match ev {
                Event::Rx { total, preview } => {
                    assert_eq!(total, 3);
                    assert_eq!(preview, "81 01 02");
                    got_rx = true;
                }
                Event::Closed(_) => {
                    got_closed = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(got_rx && got_closed);
        assert!(
            wakes.load(Ordering::SeqCst) >= 3,
            "UI must be woken per event"
        );
    }
}
