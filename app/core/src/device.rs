//! Bridge discovery rules (AD-13, AD-18, Story 4.4): which listed serial
//! ports are the Bonooru bridge, which may be, which belong to someone else,
//! and which one to open first. Pure: the caller lists the ports and passes
//! the platform's [`Rules`]; nothing here touches a port.

use std::cmp::Ordering;

/// The bridge's USB vendor id (Raspberry Pi, Story 3.1).
pub const BRIDGE_VID: u16 = 0x2E8A;
/// The bridge's USB product id (Pico SDK CDC, Story 3.1).
pub const BRIDGE_PID: u16 = 0x000A;
/// The bridge's USB product string (Story 3.1 descriptors).
pub const BRIDGE_PRODUCT: &str = "Bonooru Bridge";

/// The USB identity of a port, as the platform reports it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UsbIds {
    /// USB vendor id.
    pub vid: u16,
    /// USB product id.
    pub pid: u16,
    /// The serial number (the bridge's flash unique ID).
    pub serial: Option<String>,
    /// The manufacturer string (kept for the log only).
    pub manufacturer: Option<String>,
    /// The product string; `None` where the platform gives none (Windows).
    pub product: Option<String>,
}

/// One listed serial port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortInfo {
    /// The name to open (`/dev/cu.usbmodem101`, `COM5`, `/dev/ttyACM0`).
    pub name: String,
    /// `None` for a port that is not USB.
    pub usb: Option<UsbIds>,
}

/// What differs between platforms, passed in by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// The platform reports the USB product string (false on Windows,
    /// where the reported string is the driver's display name).
    pub product_reported: bool,
    /// Only `/dev/cu.*` names are ports (macOS: the `/dev/tty.*` twin
    /// waits for carrier detect).
    pub callout_only: bool,
}

impl Rules {
    /// macOS.
    pub const MACOS: Self = Self {
        product_reported: true,
        callout_only: true,
    };
    /// Windows.
    pub const WINDOWS: Self = Self {
        product_reported: false,
        callout_only: false,
    };
    /// Linux.
    pub const LINUX: Self = Self {
        product_reported: true,
        callout_only: false,
    };
}

/// The class of a listed port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// USB `2E8A:000A` with the product `Bonooru Bridge`.
    Ours,
    /// USB `2E8A:000A` with no usable product string (always on Windows).
    Unconfirmed,
    /// USB `2E8A:000A` with another product string: never opened.
    Foreign {
        /// The product string it reports.
        product: String,
    },
    /// Any other port: never opened.
    Ignored,
}

impl Class {
    /// A port the app may open and probe with HELLO.
    #[must_use]
    pub fn is_candidate(&self) -> bool {
        matches!(self, Self::Ours | Self::Unconfirmed)
    }

    /// The short lower-case name used in logs and probe lines.
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Ours => "ours",
            Self::Unconfirmed => "unconfirmed",
            Self::Foreign { .. } => "foreign",
            Self::Ignored => "ignored",
        }
    }
}

/// Classifies one listed port.
#[must_use]
pub fn classify(port: &PortInfo, rules: Rules) -> Class {
    if rules.callout_only && !port.name.starts_with("/dev/cu.") {
        return Class::Ignored;
    }
    let Some(usb) = &port.usb else {
        return Class::Ignored;
    };
    if usb.vid != BRIDGE_VID || usb.pid != BRIDGE_PID {
        return Class::Ignored;
    }
    if !rules.product_reported {
        return Class::Unconfirmed;
    }
    match usb.product.as_deref().map(str::trim) {
        None | Some("") => Class::Unconfirmed,
        Some(p) if p == BRIDGE_PRODUCT => Class::Ours,
        Some(p) => Class::Foreign {
            product: p.to_owned(),
        },
    }
}

/// Port names in natural order: the text before the trailing number, then
/// the number (`COM9` before `COM10`), then the whole name.
#[must_use]
pub fn name_order(a: &str, b: &str) -> Ordering {
    fn split(s: &str) -> (&str, Option<u64>) {
        let digits = s.bytes().rev().take_while(u8::is_ascii_digit).count();
        let (head, tail) = s.split_at(s.len() - digits);
        (head, tail.parse().ok())
    }
    let (ha, na) = split(a);
    let (hb, nb) = split(b);
    ha.cmp(hb).then(na.cmp(&nb)).then(a.cmp(b))
}

/// The candidates (`Ours`, `Unconfirmed`) in the order they are tried
/// (AD-13, Story 4.4 AC 4): `Ours` before `Unconfirmed`; within a class
/// the last connected bridge's serial first, then the lowest port name.
#[must_use]
pub fn ranked<'a>(ports: &'a [(PortInfo, Class)], last_serial: Option<&str>) -> Vec<&'a PortInfo> {
    let mut out: Vec<&(PortInfo, Class)> = ports.iter().filter(|p| p.1.is_candidate()).collect();
    let key = |p: &(PortInfo, Class)| {
        let class = u8::from(p.1 != Class::Ours);
        let serial = p.0.usb.as_ref().and_then(|u| u.serial.as_deref());
        let not_last = u8::from(last_serial.is_none() || serial != last_serial);
        (class, not_last)
    };
    out.sort_by(|a, b| key(a).cmp(&key(b)).then(name_order(&a.0.name, &b.0.name)));
    out.into_iter().map(|p| &p.0).collect()
}

/// The port to open first, if any (the head of [`ranked`]).
#[must_use]
pub fn choose<'a>(
    ports: &'a [(PortInfo, Class)],
    last_serial: Option<&str>,
) -> Option<&'a PortInfo> {
    ranked(ports, last_serial).into_iter().next()
}
