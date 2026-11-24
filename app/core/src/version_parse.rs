// The app version rule (AD-14), shared by `build.rs` (through `#[path]`) and
// the crate's tests. Same rule as `firmware/cmake/ocb_version.cmake`:
// `git describe --tags --match "v[0-9]*.[0-9]*.[0-9]*" --long --dirty` output
// `vX.Y.Z-N-gHASH[-dirty]` gives `X.Y.Z`, plus `-N-gHASH` when N > 0, plus
// `-dirty`; anything else gives the fallback `0.0.0-dev`. X, Y and Z must fit
// in a u8 (versions are `[u8; 3]` on the wire).

/// A parsed app version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVersion {
    /// `X.Y.Z[-N-gHASH][-dirty]` or `0.0.0-dev`.
    pub string: String,
    /// `[X, Y, Z]`, zeros for the fallback.
    pub triple: [u8; 3],
}

/// The fallback version.
pub const FALLBACK: &str = "0.0.0-dev";

fn fallback() -> ParsedVersion {
    ParsedVersion {
        string: FALLBACK.to_owned(),
        triple: [0, 0, 0],
    }
}

fn digits(s: &str) -> Option<&str> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then_some(s)
}

/// Parses one `git describe` output line; `None` input means git failed.
/// Like CMake's `OUTPUT_STRIP_TRAILING_WHITESPACE`, only trailing
/// whitespace is removed.
#[must_use]
pub fn parse_describe(describe: Option<&str>) -> ParsedVersion {
    parse_inner(describe.map(str::trim_end)).unwrap_or_else(fallback)
}

fn parse_inner(describe: Option<&str>) -> Option<ParsedVersion> {
    let s = describe?.strip_prefix('v')?;
    let (s, dirty) = match s.strip_suffix("-dirty") {
        Some(rest) => (rest, true),
        None => (s, false),
    };
    // X.Y.Z-N-gHASH: split from the right so the hash and N are exact.
    let (rest, hash) = s.rsplit_once('-')?;
    let hex = hash.strip_prefix('g')?;
    if hex.is_empty() || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let (xyz, n) = rest.rsplit_once('-')?;
    let n = digits(n)?;
    let mut parts = xyz.split('.');
    let mut text = [""; 3];
    let mut triple = [0u8; 3];
    for (slot, num) in text.iter_mut().zip(triple.iter_mut()) {
        *slot = digits(parts.next()?)?;
        *num = slot.parse::<u8>().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    // The captured text, as ocb_version.cmake does (`v01.2.3` stays `01.2.3`).
    let [x, y, z] = text;
    let mut string = format!("{x}.{y}.{z}");
    if n != "0" {
        string.push_str(&format!("-{n}-{hash}"));
    }
    if dirty {
        string.push_str("-dirty");
    }
    Some(ParsedVersion { string, triple })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> (String, [u8; 3]) {
        let v = parse_describe(Some(s));
        (v.string, v.triple)
    }

    #[test]
    fn describe_forms() {
        assert_eq!(p("v0.2.3-0-gabc1234"), ("0.2.3".into(), [0, 2, 3]));
        assert_eq!(
            p("v0.2.3-5-gabc1234"),
            ("0.2.3-5-gabc1234".into(), [0, 2, 3])
        );
        assert_eq!(
            p("v0.2.3-0-gabc1234-dirty"),
            ("0.2.3-dirty".into(), [0, 2, 3])
        );
        assert_eq!(
            p("v0.2.3-12-g0f-dirty"),
            ("0.2.3-12-g0f-dirty".into(), [0, 2, 3])
        );
        assert_eq!(p("v255.0.10-0-gdead\n"), ("255.0.10".into(), [255, 0, 10]));
        // Same strings as ocb_version.cmake: captured text, N compared to "0".
        assert_eq!(p("v01.2.3-0-gabc"), ("01.2.3".into(), [1, 2, 3]));
        assert_eq!(p("v1.2.3-00-gabc"), ("1.2.3-00-gabc".into(), [1, 2, 3]));
    }

    #[test]
    fn fallback_forms() {
        for bad in [
            "",
            "v1.2-0-gabc",
            "0.2.3-0-gabc",
            "v1.2.3-x-gabc",
            "v1.2.3-0-abc",
            "v1.2.3-0-g",
            "v1.2.3-0-gABC",
            "v1.2.3.4-0-gabc",
            "v256.0.0-0-gabc",
            "v1.2.3",
            "b5e0da0",
            "v1.-2.3-0-gabc",
            " v1.2.3-0-gabc",
            "v1.2.3-rc1-0-gabc",
        ] {
            assert_eq!(p(bad), (FALLBACK.into(), [0, 0, 0]), "{bad:?}");
        }
        assert_eq!(parse_describe(None).string, FALLBACK);
    }
}
