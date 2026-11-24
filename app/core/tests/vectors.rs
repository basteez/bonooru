//! C.1: `ocb-core` against the language-neutral conformance vectors in
//! `firmware/common/proto/vectors/` (AD-14; format in `vectors/README.md`).
//! The Rust twin of the firmware's F.9 (`firmware/tests/test_vectors.c`):
//! same file list, same minimum case counts, constants checked in both
//! directions, every message type needs an ok case, every `proto_version`
//! header must equal `PROTO_VERSION`. Failures are collected and reported as
//! `file:line: case: message`, so one run shows every problem.
//!
//! The negative tests at the end copy the vectors to a temporary directory,
//! break one thing and check that the run fails (red proof).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use ocb_core::protocol::consts::{self, PROTO_VERSION};
use ocb_core::protocol::{
    Ack, Event, EventCode, Frame, FrameError, Heartbeat, Hello, HidProfile, Info, Message, Mode,
    MsgError, NEUTRAL, Nak, NakReason, QueueEvent, RxEvent, RxSide, SetState, Splitter, Start,
    State, Status, cobs, crc16, decode_frame, encode_frame,
};

/// The vector files and their minimum case counts (lines for constants):
/// the same list as `kFiles[]` in `firmware/tests/test_vectors.c`.
const FILES: &[(&str, usize)] = &[
    ("constants.txt", 100),
    ("cobs.txt", 14),
    ("crc16.txt", 5),
    ("frames.txt", 18),
    ("messages.txt", 18),
    ("streams.txt", 10),
    ("state.txt", 17),
];

fn vectors_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../firmware/common/proto/vectors")
}

// ------------------------------------------------------------------ reporting

#[derive(Default)]
struct Report {
    failures: Vec<String>,
    cases: usize,
}

impl Report {
    fn fail(&mut self, file: &str, line: usize, case: &str, msg: impl AsRef<str>) {
        let case = if case.is_empty() { "-" } else { case };
        self.failures
            .push(format!("{file}:{line}: {case}: {}", msg.as_ref()));
    }
}

fn hex(b: &[u8]) -> String {
    let mut s = String::new();
    for (i, x) in b.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        write!(s, "{x:02x}").unwrap();
    }
    s
}

// ------------------------------------------------------------------ parsing

struct Kv {
    line: usize,
    key: String,
    value: String,
}

/// Reads a file into key/value lines after the `proto_version` header.
fn read_kv(report: &mut Report, dir: &Path, file: &str) -> Option<Vec<Kv>> {
    let text = match fs::read_to_string(dir.join(file)) {
        Ok(t) => t,
        Err(e) => {
            report.fail(file, 0, "", format!("cannot read: {e}"));
            return None;
        }
    };
    let mut out = Vec::new();
    let mut header_seen = false;
    for (i, raw) in text.split('\n').enumerate() {
        let line = i + 1;
        if let Some(bad) = raw.chars().find(|c| !(' '..='~').contains(c)) {
            report.fail(
                file,
                line,
                "",
                format!("character {bad:?} outside printable ASCII (README rule 1)"),
            );
            continue;
        }
        let t = raw.trim_matches(' ');
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some((k, v)) = t.split_once(':') else {
            report.fail(file, line, "", "not `key: value`");
            continue;
        };
        let (key, value) = (
            k.trim_matches(' ').to_owned(),
            v.trim_matches(' ').to_owned(),
        );
        if value.contains('#') {
            report.fail(file, line, "", "`#` inside a value");
            continue;
        }
        if !header_seen {
            header_seen = true;
            if key != "proto_version" {
                report.fail(file, line, "", "first key must be proto_version");
                return None;
            }
            match parse_hex(&value).as_deref() {
                Some([v]) if *v == PROTO_VERSION && value.len() == 2 => {}
                _ => report.fail(
                    file,
                    line,
                    "",
                    format!("proto_version {value} != PROTO_VERSION {PROTO_VERSION:02x}"),
                ),
            }
            continue;
        }
        out.push(Kv { line, key, value });
    }
    if !header_seen {
        report.fail(file, 0, "", "no proto_version header");
        return None;
    }
    Some(out)
}

/// Hex bytes in wire order: pairs of hex digits, optional single spaces.
fn parse_hex(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for tok in s.split(' ') {
        if tok.is_empty() || tok.len() % 2 != 0 || !tok.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        for i in (0..tok.len()).step_by(2) {
            out.push(u8::from_str_radix(&tok[i..i + 2], 16).ok()?);
        }
    }
    Some(out)
}

struct Case {
    name: String,
    line: usize,
    fields: BTreeMap<String, (usize, String)>,
}

impl Case {
    fn get(&self, f: &str) -> Option<&str> {
        self.fields.get(f).map(|(_, v)| v.as_str())
    }
}

/// Groups consecutive `<case>.<field>` lines into cases (README rule 4).
fn group_cases(report: &mut Report, file: &str, kvs: Vec<Kv>, allowed: &[&str]) -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    let mut seen = BTreeSet::new();
    for kv in kvs {
        let Some((case, field)) = kv.key.rsplit_once('.') else {
            report.fail(file, kv.line, "", format!("key `{}` has no `.`", kv.key));
            continue;
        };
        if case.is_empty()
            || !case
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            report.fail(file, kv.line, case, "bad case name");
            continue;
        }
        if !allowed.contains(&field) {
            report.fail(file, kv.line, case, format!("unknown field `{field}`"));
            continue;
        }
        let same = cases.last().is_some_and(|c| c.name == case);
        if !same {
            if !seen.insert(case.to_owned()) {
                report.fail(file, kv.line, case, "duplicate case name");
                continue;
            }
            cases.push(Case {
                name: case.to_owned(),
                line: kv.line,
                fields: BTreeMap::new(),
            });
        }
        let c = cases.last_mut().unwrap();
        if c.fields
            .insert(field.to_owned(), (kv.line, kv.value))
            .is_some()
        {
            report.fail(file, kv.line, case, format!("duplicate field `{field}`"));
        }
    }
    cases
}

/// Required, then hex-decoded field; reports a failure if missing or bad.
fn need_hex(report: &mut Report, file: &str, c: &Case, f: &str) -> Option<Vec<u8>> {
    let Some(v) = c.get(f) else {
        report.fail(file, c.line, &c.name, format!("missing field `{f}`"));
        return None;
    };
    let b = parse_hex(v);
    if b.is_none() {
        report.fail(file, c.line, &c.name, format!("bad hex in `{f}`"));
    }
    b
}

fn rx_side(report: &mut Report, file: &str, c: &Case) -> Option<RxSide> {
    match c.get("rx") {
        None | Some("CONTROLLER") => Some(RxSide::Controller),
        Some("APP") => Some(RxSide::App),
        Some(other) => {
            report.fail(file, c.line, &c.name, format!("bad rx `{other}`"));
            None
        }
    }
}

/// `name=hex` pairs in order (README rule 6): values are hex without
/// spaces, names unique.
fn parse_fields(s: &str) -> Option<FieldList> {
    let mut out: FieldList = Vec::new();
    if s.is_empty() {
        return Some(out);
    }
    for tok in s.split(' ') {
        let (k, v) = tok.split_once('=')?;
        if k.is_empty() || v.is_empty() || out.iter().any(|(n, _)| n == k) {
            return None;
        }
        out.push((k.to_owned(), parse_hex(v)?));
    }
    Some(out)
}

fn to_map(list: &FieldList) -> Fields {
    list.iter().cloned().collect()
}

// ------------------------------------------------------------------ areas

fn area_constants(report: &mut Report, file: &str, kvs: Vec<Kv>) -> usize {
    let rust: BTreeMap<&str, u32> = consts::ALL.iter().copied().collect();
    assert_eq!(
        rust.len(),
        consts::ALL.len(),
        "duplicate name in consts::ALL"
    );
    let mut seen = BTreeSet::new();
    let n = kvs.len();
    for kv in kvs {
        if kv.key.contains('.') {
            report.fail(file, kv.line, &kv.key, "constants have no dots");
            continue;
        }
        if !seen.insert(kv.key.clone()) {
            report.fail(file, kv.line, &kv.key, "duplicate constant");
            continue;
        }
        let digits_ok = (2..=8).contains(&kv.value.len())
            && kv.value.len() % 2 == 0
            && kv.value.bytes().all(|b| b.is_ascii_hexdigit());
        let Some(bytes) = parse_hex(&kv.value).filter(|_| digits_ok) else {
            report.fail(file, kv.line, &kv.key, "bad hex number");
            continue;
        };
        let v = bytes.iter().fold(0u32, |a, &b| (a << 8) | u32::from(b));
        match rust.get(kv.key.as_str()) {
            None => report.fail(file, kv.line, &kv.key, "no Rust constant of that name"),
            Some(&r) if r != v => report.fail(
                file,
                kv.line,
                &kv.key,
                format!("vector {v:#x} != Rust {r:#x}"),
            ),
            Some(_) => {}
        }
    }
    for name in rust.keys() {
        if !seen.contains(*name) {
            report.fail(file, 0, name, "Rust constant missing from constants.txt");
        }
    }
    n
}

fn area_cobs(report: &mut Report, file: &str, cases: &[Case]) {
    for c in cases {
        let Some(enc) = need_hex(report, file, c, "enc") else {
            continue;
        };
        match (c.get("raw"), c.get("err")) {
            (Some(_), None) => {
                let Some(raw) = need_hex(report, file, c, "raw") else {
                    continue;
                };
                let got = cobs::encode(&raw);
                if enc.len() > raw.len() + raw.len() / 254 + 1 {
                    report.fail(file, c.line, &c.name, "enc longer than the COBS bound");
                }
                if got != enc {
                    report.fail(
                        file,
                        c.line,
                        &c.name,
                        format!("encode: got {} expected {}", hex(&got), hex(&enc)),
                    );
                }
                match cobs::decode(&enc) {
                    Ok(d) if d == raw => {}
                    other => report.fail(file, c.line, &c.name, format!("decode: {other:?}")),
                }
            }
            (None, Some("COBS")) => {
                if let Ok(d) = cobs::decode(&enc) {
                    report.fail(
                        file,
                        c.line,
                        &c.name,
                        format!("decode should fail, got {}", hex(&d)),
                    );
                }
            }
            _ => report.fail(file, c.line, &c.name, "needs raw+enc or enc+err COBS"),
        }
    }
}

fn area_crc16(report: &mut Report, file: &str, cases: &[Case]) {
    for c in cases {
        let (Some(data), Some(crc)) = (
            need_hex(report, file, c, "data"),
            need_hex(report, file, c, "crc"),
        ) else {
            continue;
        };
        let got = crc16::crc16(&data).to_le_bytes();
        if got[..] != crc[..] {
            report.fail(
                file,
                c.line,
                &c.name,
                format!("crc got {} expected {}", hex(&got), hex(&crc)),
            );
        }
    }
}

fn frame_err_name(e: FrameError) -> &'static str {
    e.name()
}

fn area_frames(report: &mut Report, file: &str, cases: &[Case]) {
    for c in cases {
        let Some(wire) = need_hex(report, file, c, "wire") else {
            continue;
        };
        let Some(side) = rx_side(report, file, c) else {
            continue;
        };
        let payload = match c.get("payload") {
            Some(_) => match need_hex(report, file, c, "payload") {
                Some(p) if p.len() >= 2 => Some(p),
                _ => {
                    report.fail(file, c.line, &c.name, "payload needs type and seq");
                    continue;
                }
            },
            None => None,
        };
        if let Some(p) = &payload {
            let mut want = wire.clone();
            want.push(0);
            match encode_frame(p[0], p[1], &p[2..]) {
                Ok(got) if got == want => {}
                other => report.fail(
                    file,
                    c.line,
                    &c.name,
                    format!("encode: {other:?}, expected {}", hex(&want)),
                ),
            }
        }
        match c.get("err") {
            None => {
                let Some(p) = payload else {
                    report.fail(file, c.line, &c.name, "ok case needs payload");
                    continue;
                };
                let want = Frame {
                    ty: p[0],
                    seq: p[1],
                    body: p[2..].to_vec(),
                };
                match decode_frame(side, &wire) {
                    Ok(f) if f == want => {}
                    other => report.fail(file, c.line, &c.name, format!("decode: {other:?}")),
                }
            }
            Some(err) => {
                if !["COBS", "CRC", "RANGE", "EMPTY", "OVERLONG"].contains(&err) {
                    report.fail(file, c.line, &c.name, format!("bad err `{err}`"));
                    continue;
                }
                match decode_frame(side, &wire) {
                    Err(e) if frame_err_name(e) == err => {}
                    other => report.fail(
                        file,
                        c.line,
                        &c.name,
                        format!("decode: {other:?}, expected {err}"),
                    ),
                }
            }
        }
    }
}

// Message fields <-> typed messages. Numbers are big-endian bytes of their
// full width, arrays and the state are in wire order (README rule 6).

type Fields = BTreeMap<String, Vec<u8>>;
type FieldList = Vec<(String, Vec<u8>)>;

/// The fields of a message in wire order (the order the vectors use).
fn fields_of(m: &Message) -> FieldList {
    let mut f = FieldList::new();
    let mut put = |k: &str, v: &[u8]| {
        f.push((k.to_owned(), v.to_vec()));
    };
    match m {
        Message::Hello(h) => put("proto_ver", &[h.proto_ver]),
        Message::Info(i) => {
            put("proto_ver", &[i.proto_ver]);
            put("fw_ver", &i.fw_ver);
            put("bridge_ver", &i.bridge_ver);
            put("hid_profile", &[i.hid_profile.to_u8()]);
            put("queue_cap", &i.queue_cap.to_be_bytes());
            put("app_timeout_ms", &i.app_timeout_ms.to_be_bytes());
            put("report_period_us", &i.report_period_us.to_be_bytes());
        }
        Message::SetState(s) => put("state", &s.state.to_bytes()),
        Message::QueueEvent(q) => {
            put("t_us", &q.t_us.to_be_bytes());
            put("state", &q.state.to_bytes());
        }
        Message::Start(s) => put("lead_us", &s.lead_us.to_be_bytes()),
        Message::Stop | Message::Ping => {}
        Message::Heartbeat(h) => {
            put("bridge_fw_ver", &h.bridge_fw_ver);
            put("flags", &[h.flags]);
        }
        Message::Status(s) => {
            put("free", &s.free.to_be_bytes());
            put("now_us", &s.now_us.to_be_bytes());
            put("last_seq", &[s.last_seq]);
            put("mode", &[s.mode.to_u8()]);
            put("abort_count", &[s.abort_count]);
            put("last_event", &[s.last_event.to_u8()]);
            put("flags", &[s.flags]);
            if !s.extra.is_empty() {
                put("extra", &s.extra);
            }
        }
        Message::Ack(a) => put("seq", &[a.seq]),
        Message::Nak(n) => {
            put("reason", &[n.reason.to_u8()]);
            put("expected", &[n.expected]);
            put("got", &[n.got]);
        }
        Message::Event(e) => put("code", &[e.code.to_u8()]),
    }
    f
}

fn take<const N: usize>(f: &Fields, k: &str) -> Option<[u8; N]> {
    f.get(k)?.as_slice().try_into().ok()
}

fn u8_of(f: &Fields, k: &str) -> Option<u8> {
    take::<1>(f, k).map(|[b]| b)
}

fn u16_of(f: &Fields, k: &str) -> Option<u16> {
    take::<2>(f, k).map(u16::from_be_bytes)
}

fn u32_of(f: &Fields, k: &str) -> Option<u32> {
    take::<4>(f, k).map(u32::from_be_bytes)
}

fn state_of(f: &Fields) -> Option<State> {
    take::<8>(f, "state").map(|b| State::from_bytes(&b))
}

fn message_from_fields(ty: u8, f: &Fields) -> Option<Message> {
    Some(match ty {
        consts::MSG_HELLO => Message::Hello(Hello {
            proto_ver: u8_of(f, "proto_ver")?,
        }),
        consts::MSG_INFO => Message::Info(Info {
            proto_ver: u8_of(f, "proto_ver")?,
            fw_ver: take(f, "fw_ver")?,
            bridge_ver: take(f, "bridge_ver")?,
            hid_profile: HidProfile::from_u8(u8_of(f, "hid_profile")?),
            queue_cap: u16_of(f, "queue_cap")?,
            app_timeout_ms: u16_of(f, "app_timeout_ms")?,
            report_period_us: u16_of(f, "report_period_us")?,
        }),
        consts::MSG_SET_STATE => Message::SetState(SetState {
            state: state_of(f)?,
        }),
        consts::MSG_QUEUE_EVENT => Message::QueueEvent(QueueEvent {
            t_us: u32_of(f, "t_us")?,
            state: state_of(f)?,
        }),
        consts::MSG_START => Message::Start(Start {
            lead_us: u32_of(f, "lead_us")?,
        }),
        consts::MSG_STOP => Message::Stop,
        consts::MSG_PING => Message::Ping,
        consts::MSG_HEARTBEAT => Message::Heartbeat(Heartbeat {
            bridge_fw_ver: take(f, "bridge_fw_ver")?,
            flags: u8_of(f, "flags")?,
        }),
        consts::MSG_STATUS => Message::Status(Status {
            free: u16_of(f, "free")?,
            now_us: u32_of(f, "now_us")?,
            last_seq: u8_of(f, "last_seq")?,
            mode: Mode::from_u8(u8_of(f, "mode")?),
            abort_count: u8_of(f, "abort_count")?,
            last_event: EventCode::from_u8(u8_of(f, "last_event")?),
            flags: u8_of(f, "flags")?,
            extra: f.get("extra").cloned().unwrap_or_default(),
        }),
        consts::MSG_ACK => Message::Ack(Ack {
            seq: u8_of(f, "seq")?,
        }),
        consts::MSG_NAK => Message::Nak(Nak {
            reason: NakReason::from_u8(u8_of(f, "reason")?),
            expected: u8_of(f, "expected")?,
            got: u8_of(f, "got")?,
        }),
        consts::MSG_EVENT => Message::Event(Event {
            code: EventCode::from_u8(u8_of(f, "code")?),
        }),
        _ => return None,
    })
}

const MESSAGE_TYPES: &[(&str, u8)] = &[
    ("HELLO", consts::MSG_HELLO),
    ("SET_STATE", consts::MSG_SET_STATE),
    ("QUEUE_EVENT", consts::MSG_QUEUE_EVENT),
    ("START", consts::MSG_START),
    ("STOP", consts::MSG_STOP),
    ("PING", consts::MSG_PING),
    ("HEARTBEAT", consts::MSG_HEARTBEAT),
    ("INFO", consts::MSG_INFO),
    ("STATUS", consts::MSG_STATUS),
    ("ACK", consts::MSG_ACK),
    ("NAK", consts::MSG_NAK),
    ("EVENT", consts::MSG_EVENT),
];

fn area_messages(report: &mut Report, file: &str, cases: &[Case]) {
    let mut ok_types = BTreeSet::new();
    for c in cases {
        let Some(ty) = need_hex(report, file, c, "type") else {
            continue;
        };
        let [ty] = ty[..] else {
            report.fail(file, c.line, &c.name, "type is one byte");
            continue;
        };
        let Some(body) = need_hex(report, file, c, "body") else {
            continue;
        };
        if let Some(err) = c.get("err") {
            if let Some(extra) = c
                .fields
                .keys()
                .find(|k| !["type", "body", "err"].contains(&k.as_str()))
            {
                report.fail(
                    file,
                    c.line,
                    &c.name,
                    format!("error case with field `{extra}`"),
                );
            }
            if err != "BODY_LEN" {
                report.fail(file, c.line, &c.name, format!("bad err `{err}`"));
            } else if Message::decode(ty, &body) != Err(MsgError::BodyLen) {
                report.fail(file, c.line, &c.name, "decode should fail with BODY_LEN");
            }
            continue;
        }
        let Some(seq) = need_hex(report, file, c, "seq") else {
            continue;
        };
        let [seq] = seq[..] else {
            report.fail(file, c.line, &c.name, "seq must be one byte");
            continue;
        };
        let Some(wire) = need_hex(report, file, c, "wire") else {
            continue;
        };
        let trailing = match c.get("trailing") {
            Some(_) => match need_hex(report, file, c, "trailing") {
                Some(t) => t,
                None => continue,
            },
            None => Vec::new(),
        };
        let Some(fields) = c.get("fields").and_then(parse_fields) else {
            report.fail(file, c.line, &c.name, "missing or bad `fields`");
            continue;
        };
        // decode(body) == fields
        match Message::decode(ty, &body) {
            Ok(m) => {
                let got = fields_of(&m);
                if got != fields {
                    report.fail(
                        file,
                        c.line,
                        &c.name,
                        format!("decode fields {got:?} != {fields:?}"),
                    );
                }
            }
            Err(e) => report.fail(file, c.line, &c.name, format!("decode: {e}")),
        }
        // encode(fields) + trailing == body
        match message_from_fields(ty, &to_map(&fields)) {
            Some(m) => match m.encode_body() {
                Ok(mut enc) => {
                    enc.extend_from_slice(&trailing);
                    if enc != body {
                        report.fail(
                            file,
                            c.line,
                            &c.name,
                            format!("encode: {} != body {}", hex(&enc), hex(&body)),
                        );
                    }
                }
                Err(e) => report.fail(file, c.line, &c.name, format!("encode: {e}")),
            },
            None => report.fail(file, c.line, &c.name, "fields do not build the message"),
        }
        // encode_frame(type, seq, body) == wire + 00
        let mut want = wire.clone();
        want.push(0);
        match encode_frame(ty, seq, &body) {
            Ok(got) if got == want => {}
            other => report.fail(
                file,
                c.line,
                &c.name,
                format!("frame: {other:?}, expected {}", hex(&want)),
            ),
        }
        // decode_frame(wire) on the receiving side == (type, seq, body)
        let side = if ty >= consts::TYPE_CTRL_MIN {
            RxSide::App
        } else {
            RxSide::Controller
        };
        match decode_frame(side, &wire) {
            Ok(f)
                if f == (Frame {
                    ty,
                    seq,
                    body: body.clone(),
                }) => {}
            other => report.fail(file, c.line, &c.name, format!("wire decode: {other:?}")),
        }
        ok_types.insert(ty);
    }
    for (name, ty) in MESSAGE_TYPES {
        if !ok_types.contains(ty) {
            report.fail(file, 0, name, format!("no ok case for message {name}"));
        }
    }
}

fn area_streams(report: &mut Report, file: &str, cases: &[Case]) {
    for c in cases {
        let Some(input) = need_hex(report, file, c, "in") else {
            continue;
        };
        let Some(side) = rx_side(report, file, c) else {
            continue;
        };
        let Some(frames) = c.get("frames") else {
            report.fail(file, c.line, &c.name, "missing field `frames`");
            continue;
        };
        let mut want_frames = Vec::new();
        let mut bad = false;
        if !frames.is_empty() {
            for p in frames.split(';') {
                match parse_hex(p.trim()) {
                    Some(b) if b.len() >= 2 => want_frames.push(Frame {
                        ty: b[0],
                        seq: b[1],
                        body: b[2..].to_vec(),
                    }),
                    _ => bad = true,
                }
            }
        }
        let want_stats = c.get("stats").and_then(parse_stats);
        if bad || want_stats.is_none() {
            report.fail(file, c.line, &c.name, "bad `frames` or `stats`");
            continue;
        }
        let mut sp = Splitter::new(side);
        let mut got_frames = Vec::new();
        for &b in &input {
            if let RxEvent::Frame(f) = sp.push(b) {
                got_frames.push(f);
            }
        }
        if got_frames != want_frames {
            report.fail(
                file,
                c.line,
                &c.name,
                format!("frames {got_frames:?} != {want_frames:?}"),
            );
        }
        let s = sp.stats();
        let got = [s.ok, s.empty, s.cobs, s.crc, s.range, s.overlong];
        if Some(got) != want_stats {
            report.fail(
                file,
                c.line,
                &c.name,
                format!("stats {got:?} != {want_stats:?}"),
            );
        }
    }
}

/// `ok=N empty=N cobs=N crc=N range=N overlong=N`, decimal, in this order.
fn parse_stats(s: &str) -> Option<[u32; 6]> {
    const NAMES: [&str; 6] = ["ok", "empty", "cobs", "crc", "range", "overlong"];
    let toks: Vec<&str> = s.split(' ').collect();
    if toks.len() != 6 {
        return None;
    }
    let mut out = [0u32; 6];
    for (i, t) in toks.iter().enumerate() {
        let (k, v) = t.split_once('=')?;
        if k != NAMES[i] || v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        out[i] = v.parse().ok()?;
    }
    Some(out)
}

fn area_state(report: &mut Report, file: &str, cases: &[Case]) {
    let mut saw_neutral = false;
    for c in cases {
        let Some(bytes) = need_hex(report, file, c, "bytes") else {
            continue;
        };
        let Ok(arr) = <[u8; 8]>::try_from(bytes.as_slice()) else {
            report.fail(file, c.line, &c.name, "bytes must be 8 bytes");
            continue;
        };
        let Some(f) = c.get("fields").and_then(parse_fields) else {
            report.fail(file, c.line, &c.name, "missing or bad `fields`");
            continue;
        };
        const ORDER: [(&str, usize); 7] = [
            ("buttons", 2),
            ("hat", 1),
            ("lx", 1),
            ("ly", 1),
            ("rx", 1),
            ("ry", 1),
            ("flags", 1),
        ];
        let shape_ok = f.len() == ORDER.len()
            && f.iter()
                .zip(ORDER)
                .all(|((n, v), (w, len))| n == w && v.len() == len);
        let m = to_map(&f);
        let fields_state = shape_ok
            .then(|| -> Option<State> {
                Some(State {
                    buttons: u16_of(&m, "buttons")?,
                    hat: u8_of(&m, "hat")?,
                    lx: u8_of(&m, "lx")?,
                    ly: u8_of(&m, "ly")?,
                    rx: u8_of(&m, "rx")?,
                    ry: u8_of(&m, "ry")?,
                    flags: u8_of(&m, "flags")?,
                })
            })
            .flatten();
        let Some(want) = fields_state else {
            report.fail(
                file,
                c.line,
                &c.name,
                "fields must be buttons hat lx ly rx ry flags, in order and full width",
            );
            continue;
        };
        if State::from_bytes(&arr) != want {
            report.fail(file, c.line, &c.name, "decode(bytes) != fields");
        }
        if want.to_bytes() != arr {
            report.fail(file, c.line, &c.name, "encode(fields) != bytes");
        }
        if c.name == "neutral" {
            saw_neutral = true;
            if want != NEUTRAL {
                report.fail(file, c.line, &c.name, "neutral case != NEUTRAL");
            }
        }
    }
    if !saw_neutral {
        report.fail(file, 0, "neutral", "no `neutral` case");
    }
}

// ------------------------------------------------------------------ driver

fn run(dir: &Path) -> Report {
    let mut report = Report::default();

    // Every .txt in the directory must be on the list (Story 2.2 D16).
    match fs::read_dir(dir) {
        Ok(entries) => {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.ends_with(".txt") && !FILES.iter().any(|(f, _)| *f == name) {
                    report.fail(&name, 0, "", "vector file not in the Rust test's list");
                }
            }
        }
        Err(e) => {
            report.fail(
                &dir.display().to_string(),
                0,
                "",
                format!("cannot read dir: {e}"),
            );
            return report;
        }
    }

    for &(file, min) in FILES {
        let Some(kvs) = read_kv(&mut report, dir, file) else {
            continue;
        };
        let n = if file == "constants.txt" {
            area_constants(&mut report, file, kvs)
        } else {
            let allowed: &[&str] = match file {
                "cobs.txt" => &["raw", "enc", "err"],
                "crc16.txt" => &["data", "crc"],
                "frames.txt" => &["payload", "wire", "rx", "err"],
                "messages.txt" => &["type", "seq", "fields", "body", "wire", "trailing", "err"],
                "streams.txt" => &["in", "rx", "frames", "stats"],
                "state.txt" => &["bytes", "fields"],
                _ => unreachable!(),
            };
            let cases = group_cases(&mut report, file, kvs, allowed);
            match file {
                "cobs.txt" => area_cobs(&mut report, file, &cases),
                "crc16.txt" => area_crc16(&mut report, file, &cases),
                "frames.txt" => area_frames(&mut report, file, &cases),
                "messages.txt" => area_messages(&mut report, file, &cases),
                "streams.txt" => area_streams(&mut report, file, &cases),
                "state.txt" => area_state(&mut report, file, &cases),
                _ => unreachable!(),
            }
            cases.len()
        };
        if n < min {
            report.fail(file, 0, "", format!("{n} cases, at least {min} required"));
        }
        report.cases += n;
    }
    report
}

#[test]
fn c1_conformance_vectors() {
    let dir = vectors_dir();
    assert!(dir.is_dir(), "vector directory missing: {}", dir.display());
    let r = run(&dir);
    if !r.failures.is_empty() {
        panic!(
            "{} vector failure(s):\n{}",
            r.failures.len(),
            r.failures.join("\n")
        );
    }
    println!(
        "vectors: {} cases in {} files ok (proto_version {PROTO_VERSION:02x})",
        r.cases,
        FILES.len()
    );
}

// ------------------------------------------------------------------ red proof

/// Copies the vectors to a fresh temp dir and applies `edit` to one file.
fn broken_copy(tag: &str, file: &str, edit: impl Fn(&str) -> String) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ocb-vectors-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for e in fs::read_dir(vectors_dir()).unwrap().flatten() {
        if e.path().is_file() {
            fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        }
    }
    let p = dir.join(file);
    let text = fs::read_to_string(&p).unwrap();
    let new = edit(&text);
    assert_ne!(new, text, "the edit changed nothing");
    fs::write(&p, new).unwrap();
    dir
}

fn expect_failure(dir: &Path, needle: &str) {
    let r = run(dir);
    let _ = fs::remove_dir_all(dir);
    assert!(
        r.failures.iter().any(|f| f.contains(needle)),
        "expected a failure containing {needle:?}, got {:#?}",
        r.failures
    );
}

#[test]
fn c1_red_wrong_crc_vector() {
    let d = broken_copy("crc", "crc16.txt", |t| {
        t.replace("check.crc: b1 29", "check.crc: b1 28")
    });
    expect_failure(&d, "crc16.txt:9: check: crc got b1 29 expected b1 28");
}

#[test]
fn c1_red_wrong_proto_version_header() {
    let d = broken_copy("pv", "state.txt", |t| {
        t.replacen("proto_version: 01", "proto_version: 02", 1)
    });
    expect_failure(&d, "proto_version 02 != PROTO_VERSION 01");
}

#[test]
fn c1_red_constant_differs() {
    let d = broken_copy("const", "constants.txt", |t| {
        t.replace("LINK_TIMEOUT_MS: 50", "LINK_TIMEOUT_MS: 51")
    });
    expect_failure(&d, "LINK_TIMEOUT_MS: vector 0x51 != Rust 0x50");
}

#[test]
fn c1_red_constant_missing() {
    let d = broken_copy("constmiss", "constants.txt", |t| {
        t.replace("ACK_WINDOW: 40\n", "")
    });
    expect_failure(&d, "ACK_WINDOW: Rust constant missing");
}

#[test]
fn c1_red_unknown_field() {
    // Right after the case's last line, so only the unknown-field rule fires.
    let d = broken_copy("field", "cobs.txt", |t| {
        t.replacen("empty.enc: 01\n", "empty.enc: 01\nempty.extra: 00\n", 1)
    });
    expect_failure(&d, "cobs.txt:11: empty: unknown field `extra`");
}

#[test]
fn c1_red_unlisted_file() {
    let d = broken_copy("file", "cobs.txt", |t| format!("{t}\n"));
    fs::write(d.join("new_area.txt"), "proto_version: 01\n").unwrap();
    expect_failure(
        &d,
        "new_area.txt:0: -: vector file not in the Rust test's list",
    );
}

#[test]
fn c1_red_bad_seq() {
    let d = broken_copy("seq", "messages.txt", |t| {
        t.replacen("hello.seq: 00", "hello.seq: 00 00", 1)
    });
    expect_failure(&d, "hello: seq must be one byte");
}

#[test]
fn c1_red_non_ascii() {
    let d = broken_copy("ascii", "cobs.txt", |t| {
        t.replacen("one_zero.enc: 01 01", "one_zero.enc: 01 01\u{a0}", 1)
    });
    expect_failure(&d, "outside printable ASCII");
}

#[test]
fn c1_red_spaced_constant() {
    let d = broken_copy("cspace", "constants.txt", |t| {
        t.replacen("MAX_RAW_FRAME: 0104", "MAX_RAW_FRAME: 01 04", 1)
    });
    expect_failure(&d, "MAX_RAW_FRAME: bad hex number");
}

#[test]
fn c1_red_state_field_order() {
    let d = broken_copy("sorder", "state.txt", |t| {
        t.replacen(
            "neutral.fields: buttons=0000 hat=08 lx=80 ly=80 rx=80 ry=80 flags=00",
            "neutral.fields: flags=00 ry=80 rx=80 ly=80 lx=80 hat=08 buttons=0000",
            1,
        )
    });
    expect_failure(&d, "neutral: fields must be buttons hat lx ly rx ry flags");
}

#[test]
fn c1_red_message_field_order() {
    let d = broken_copy("morder", "messages.txt", |t| {
        t.replacen(
            "nak_seq.fields: reason=02 expected=05 got=07",
            "nak_seq.fields: got=07 expected=05 reason=02",
            1,
        )
    });
    expect_failure(&d, "nak_seq: decode fields");
}

#[test]
fn c1_red_error_case_with_extra_field() {
    let d = broken_copy("errx", "messages.txt", |t| {
        t.replacen(
            "set_state_short.err: BODY_LEN",
            "set_state_short.err: BODY_LEN\nset_state_short.seq: 00",
            1,
        )
    });
    expect_failure(&d, "set_state_short: error case with field `seq`");
}

#[test]
fn c1_red_wrong_message_wire() {
    let d = broken_copy("msg", "messages.txt", |t| {
        t.replace(
            "hello.wire: 02 01 05 01 01 64 d1",
            "hello.wire: 02 01 05 01 01 64 d0",
        )
    });
    expect_failure(&d, "messages.txt:11: hello: frame:");
}
