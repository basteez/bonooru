//! Which serial paths the tool opens (AD-18): on macOS every device appears
//! as `/dev/cu.*` and `/dev/tty.*`, and only `cu.*` is opened (opening a
//! `tty.*` node waits for carrier detect).

/// `Ok` when `path` may be opened on this OS (`macos`: the host is macOS).
pub fn port_rule(path: &str, macos: bool) -> Result<(), String> {
    if !macos {
        return if path.is_empty() {
            Err("empty port name".to_owned())
        } else {
            Ok(())
        };
    }
    if let Some(rest) = path.strip_prefix("/dev/tty.") {
        return Err(format!(
            "{path}: on macOS open the callout device /dev/cu.{rest}, not /dev/tty.*"
        ));
    }
    match path.strip_prefix("/dev/cu.") {
        Some(rest) if !rest.is_empty() => Ok(()),
        _ => Err(format!(
            "{path}: on macOS only /dev/cu.* ports are opened (ls /dev/cu.usbmodem*)"
        )),
    }
}
