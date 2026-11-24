//! C.13: the timeline file (Story 3.4 Task 4).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_core::protocol::consts::HAT_UP;
use ocb_dev::timeline::{neutral_end, parse};

#[test]
fn the_committed_melody_parses() {
    let tl = parse(&common::melody_text()).unwrap();
    assert_eq!(tl.lead_us, 500_000, "the Story 1.5 profile's start lead");
    assert_eq!(tl.events.len(), 158, "the events of testtl_data.c");
    assert_eq!(tl.events[1].0, 500_000);
    assert_eq!(tl.events[1].1.buttons, 0x0001, "NOTE 01 B5 on Y");
    assert_eq!(*tl.events.last().unwrap(), (44_166_667, neutral_end()));
    assert!(
        tl.events.iter().any(|e| e.1.hat == HAT_UP),
        "D-pad up is used"
    );
}

const HEAD: &str = "# start_lead_us: 500000\nt_us,state\n";
const END: &str = "2000,0000088080808001\n";

fn refused(body: &str, needle: &str) {
    let err = parse(body).expect_err(needle);
    assert!(err.contains(needle), "{err:?} lacks {needle:?}");
}

#[test]
fn refusals_name_the_line() {
    refused("t_us,state\n0,0000088080808001\n", "start_lead_us");
    refused(
        "# start_lead_us: x\nt_us,state\n",
        "line 1: bad start_lead_us",
    );
    refused(
        "# start_lead_us: 1\n# start_lead_us: 2\nt_us,state\n",
        "line 2: start_lead_us given twice",
    );
    refused(
        "# start_lead_us: 1\n0,0000088080808001\n",
        "line 2: expected the header",
    );
    refused(&format!("{HEAD}0\n{END}"), "line 3: expected t_us,state");
    refused(
        &format!("{HEAD}x,0000088080808000\n{END}"),
        "line 3: bad t_us",
    );
    refused(
        &format!("{HEAD}0,00000880808080\n{END}"),
        "line 3: state must be 16 hex digits",
    );
    refused(
        &format!("{HEAD}0,zz00088080808000\n{END}"),
        "line 3: state must be 16 hex digits",
    );
    refused(
        &format!("{HEAD}5,0000088080808000\n5,0100088080808000\n{END}"),
        "line 4: t_us 5 is not after the previous 5",
    );
    refused(
        &format!("{HEAD}2147483648,0000088080808001\n"),
        "line 3: t_us 2147483648 is not below 2^31",
    );
    // AD-9: B (0x0002), HOME (0x1000), ZL (0x0040), D-pad left (6), right stick, a reserved flag.
    for s in [
        "0200088080808000",
        "0010088080808000",
        "4000088080808000",
        "0000068080808000",
        "0000088080008000",
        "0000088080808002",
    ] {
        refused(&format!("{HEAD}0,{s}\n{END}"), "line 3: state");
    }
    refused(
        &format!("{HEAD}0,0100088080808001\n{END}"),
        "line 3: END on a state that is not neutral",
    );
    refused(
        &format!("{HEAD}0,0000088080808001\n{END}"),
        "line 4: a row after the END row (line 3)",
    );
    refused(
        &format!("{HEAD}0,0100088080808000\n"),
        "the last row must be NEUTRAL|END",
    );
    refused(HEAD, "no events");
    refused("# start_lead_us: 1\n", "no header");
    let many: String = (0..100_001u32)
        .map(|i| format!("{i},0000088080808000\n"))
        .collect();
    refused(
        &format!("{HEAD}{many}"),
        "line 100003: more than 100000 rows",
    );
}

#[test]
fn a_minimal_file_with_comments_and_blank_lines() {
    let tl = parse(&format!(
        "# a comment\n\n{HEAD}\n0,0400088000808000\n# mid comment\n{END}"
    ))
    .unwrap();
    assert_eq!(tl.events.len(), 2);
    assert_eq!(tl.events[0].1.buttons, 0x0004, "A");
    assert_eq!(tl.events[0].1.ly, 0x00, "left stick up is allowed");
}
