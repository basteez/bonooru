//! C.14: on macOS only `/dev/cu.*` (AD-18).
#![allow(clippy::panic)]
use ocb_dev::port_rule::port_rule;

#[test]
fn macos_opens_only_callout_devices() {
    assert!(port_rule("/dev/cu.usbmodem101", true).is_ok());
    let Err(e) = port_rule("/dev/tty.usbmodem101", true) else {
        panic!("tty refused");
    };
    assert!(e.contains("/dev/cu.usbmodem101"), "{e}");
    assert!(port_rule("/dev/cu.", true).is_err());
    assert!(port_rule("usbmodem101", true).is_err());
    assert!(port_rule("COM5", true).is_err());
}

#[test]
fn elsewhere_any_path() {
    assert!(port_rule("COM5", false).is_ok());
    assert!(port_rule("/dev/ttyACM0", false).is_ok());
    assert!(port_rule("/dev/tty.usbmodem101", false).is_ok());
    assert!(port_rule("", false).is_err());
}
