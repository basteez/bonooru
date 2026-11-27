//! C.21: the bridge discovery rules (Story 4.4 AC 3, 4; AD-13, AD-18).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use ocb_core::device::{
    BRIDGE_PID, BRIDGE_PRODUCT, BRIDGE_VID, Class, PortInfo, Rules, UsbIds, choose, classify,
    name_order, ranked,
};

fn usb(name: &str, vid: u16, pid: u16, serial: Option<&str>, product: Option<&str>) -> PortInfo {
    PortInfo {
        name: name.to_owned(),
        usb: Some(UsbIds {
            vid,
            pid,
            serial: serial.map(str::to_owned),
            manufacturer: Some("Bonooru".to_owned()),
            product: product.map(str::to_owned),
        }),
    }
}

fn bridge(name: &str, serial: &str, product: Option<&str>) -> PortInfo {
    usb(name, BRIDGE_VID, BRIDGE_PID, Some(serial), product)
}

const ALL: [Rules; 3] = [Rules::MACOS, Rules::WINDOWS, Rules::LINUX];

#[test]
fn c21_constants() {
    assert_eq!((BRIDGE_VID, BRIDGE_PID), (0x2E8A, 0x000A));
    assert_eq!(BRIDGE_PRODUCT, "Bonooru Bridge");
}

#[test]
fn c21_linux_classes() {
    let r = Rules::LINUX;
    let p = |product| classify(&bridge("/dev/ttyACM0", "S", product), r);
    assert_eq!(p(Some("Bonooru Bridge")), Class::Ours);
    assert_eq!(p(Some("  Bonooru Bridge ")), Class::Ours, "trimmed");
    assert_eq!(
        p(Some("2wiCC COM")),
        Class::Foreign {
            product: "2wiCC COM".to_owned()
        }
    );
    assert_eq!(
        p(Some("bonooru bridge")),
        Class::Foreign {
            product: "bonooru bridge".to_owned()
        },
        "case-sensitive"
    );
    assert_eq!(p(None), Class::Unconfirmed);
    assert_eq!(p(Some("")), Class::Unconfirmed, "empty = not given");
    assert_eq!(p(Some("   ")), Class::Unconfirmed, "blank = not given");
}

#[test]
fn c21_macos_callout_only() {
    let r = Rules::MACOS;
    assert_eq!(
        classify(
            &bridge("/dev/cu.usbmodem101", "S", Some("Bonooru Bridge")),
            r
        ),
        Class::Ours
    );
    assert_eq!(
        classify(
            &bridge("/dev/tty.usbmodem101", "S", Some("Bonooru Bridge")),
            r
        ),
        Class::Ignored
    );
    assert_eq!(
        classify(&bridge("/dev/cu.usbmodem101", "S", None), r),
        Class::Unconfirmed
    );
    assert_eq!(
        classify(&bridge("/dev/cu.usbmodem101", "S", Some("2wiCC COM")), r),
        Class::Foreign {
            product: "2wiCC COM".to_owned()
        }
    );
    // Linux and Windows names are not filtered by the callout rule.
    assert_eq!(
        classify(
            &bridge("/dev/tty.usbmodem101", "S", Some("Bonooru Bridge")),
            Rules::LINUX
        ),
        Class::Ours
    );
}

#[test]
fn c21_windows_product_never_compared() {
    let r = Rules::WINDOWS;
    for product in [
        Some("USB Serial Device (COM5)"),
        Some("Bonooru Bridge"),
        Some("2wiCC COM"),
        None,
    ] {
        assert_eq!(
            classify(&bridge("COM5", "S", product), r),
            Class::Unconfirmed,
            "{product:?}"
        );
    }
}

#[test]
fn c21_other_ports_ignored_everywhere() {
    for r in ALL {
        let name = if r.callout_only { "/dev/cu.x" } else { "COM3" };
        // BOOTSEL (2E8A:0003), a Pro Controller (057E:2009), non-USB.
        assert_eq!(
            classify(&usb(name, 0x2E8A, 0x0003, Some("S"), Some("RP2 Boot")), r),
            Class::Ignored
        );
        assert_eq!(
            classify(&usb(name, 0x057E, 0x2009, None, Some("Pro Controller")), r),
            Class::Ignored
        );
        assert_eq!(
            classify(
                &PortInfo {
                    name: name.to_owned(),
                    usb: None
                },
                r
            ),
            Class::Ignored
        );
    }
}

#[test]
fn c21_candidates_and_tags() {
    assert!(Class::Ours.is_candidate() && Class::Unconfirmed.is_candidate());
    assert!(!Class::Ignored.is_candidate());
    assert!(
        !Class::Foreign {
            product: String::new()
        }
        .is_candidate()
    );
    assert_eq!(Class::Ours.tag(), "ours");
    assert_eq!(Class::Unconfirmed.tag(), "unconfirmed");
    assert_eq!(Class::Ignored.tag(), "ignored");
}

fn classified(ports: Vec<PortInfo>, r: Rules) -> Vec<(PortInfo, Class)> {
    ports
        .into_iter()
        .map(|p| {
            let c = classify(&p, r);
            (p, c)
        })
        .collect()
}

fn names(v: Vec<&PortInfo>) -> Vec<&str> {
    v.into_iter().map(|p| p.name.as_str()).collect()
}

#[test]
fn c21_choice_order() {
    let r = Rules::LINUX;
    let ports = classified(
        vec![
            bridge("/dev/ttyACM2", "C", None),
            bridge("/dev/ttyACM1", "B", Some("Bonooru Bridge")),
            bridge("/dev/ttyACM0", "A", None),
            bridge("/dev/ttyACM3", "D", Some("2wiCC COM")),
            usb("/dev/ttyUSB0", 0x0403, 0x6001, Some("F"), Some("FT232R")),
            bridge("/dev/ttyACM9", "E", Some("Bonooru Bridge")),
        ],
        r,
    );
    // Ours first (lowest name), then unconfirmed; foreign and ignored never.
    assert_eq!(
        names(ranked(&ports, None)),
        [
            "/dev/ttyACM1",
            "/dev/ttyACM9",
            "/dev/ttyACM0",
            "/dev/ttyACM2"
        ]
    );
    // The last connected serial first within its class.
    assert_eq!(
        names(ranked(&ports, Some("E"))),
        [
            "/dev/ttyACM9",
            "/dev/ttyACM1",
            "/dev/ttyACM0",
            "/dev/ttyACM2"
        ]
    );
    assert_eq!(
        names(ranked(&ports, Some("C"))),
        [
            "/dev/ttyACM1",
            "/dev/ttyACM9",
            "/dev/ttyACM2",
            "/dev/ttyACM0"
        ],
        "ours still before unconfirmed"
    );
    assert_eq!(choose(&ports, None).unwrap().name, "/dev/ttyACM1");
    assert!(choose(&classified(vec![], r), None).is_none());
}

#[test]
fn c21_com_ports_by_number() {
    let r = Rules::WINDOWS;
    let ports = classified(
        vec![
            bridge("COM10", "A", None),
            bridge("COM9", "B", None),
            bridge("COM3", "C", None),
        ],
        r,
    );
    assert_eq!(names(ranked(&ports, None)), ["COM3", "COM9", "COM10"]);
    assert_eq!(names(ranked(&ports, Some("A"))), ["COM10", "COM3", "COM9"]);
    assert_eq!(name_order("COM9", "COM10"), std::cmp::Ordering::Less);
    assert_eq!(name_order("COM10", "COM9"), std::cmp::Ordering::Greater);
    assert_eq!(
        name_order("/dev/cu.usbmodem101", "/dev/cu.usbmodem1101"),
        std::cmp::Ordering::Less
    );
}
