//! The app version (AD-14), computed at build time by `build.rs` from
//! `git describe`, with the fallback `0.0.0-dev`. Story 3.3 compares
//! [`APP_VERSION_TRIPLE`] with `INFO.fw_ver` and `INFO.bridge_ver` (AD-13).

/// `X.Y.Z`, `X.Y.Z-N-gHASH`, either with `-dirty`, or `0.0.0-dev`.
pub const APP_VERSION: &str = env!("OCB_APP_VERSION");

/// `[X, Y, Z]` of [`APP_VERSION`] (zeros for the fallback).
pub const APP_VERSION_TRIPLE: [u8; 3] = [
    parse_u8(env!("OCB_APP_VERSION_MAJOR")),
    parse_u8(env!("OCB_APP_VERSION_MINOR")),
    parse_u8(env!("OCB_APP_VERSION_PATCH")),
];

const fn parse_u8(s: &str) -> u8 {
    let mut rest = s.as_bytes();
    let mut v: u32 = 0;
    while let [d, tail @ ..] = rest {
        if !d.is_ascii_digit() {
            return 0;
        }
        v = v * 10 + (*d - b'0') as u32;
        if v > 255 {
            return 0;
        }
        rest = tail;
    }
    v as u8
}
