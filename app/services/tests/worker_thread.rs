//! C.23 (real thread): the worker thread ends quickly on `Shutdown` and
//! closes the port, calls the waker after publishing, and its snapshot
//! channel never grows (Story 4.4 AC 1, 2). Fake backend only.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use ocb_core::device::Rules;
use ocb_services::log::LogHandle;
use ocb_services::sim::{FakeBackend, FakePort};
use ocb_services::worker::{FakeClock, RealClock, WorkerEvent, spawn_with};

const CU: &str = "/dev/cu.usbmodem101";

fn backend() -> FakeBackend {
    let b = FakeBackend::new(FakeClock::at(0))
        .with(FakePort::bridge(CU, "S", Some("Bonooru Bridge")).silent());
    b.state().read_sleep = Some(Duration::from_millis(1));
    b
}

#[test]
fn c23_shutdown_joins_quickly() {
    let b = backend();
    let mut h = spawn_with(
        b.clone(),
        RealClock::new(),
        Rules::MACOS,
        LogHandle::disabled(),
        Box::new(|| {}),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let e = h.events.recv_timeout(Duration::from_secs(5)).unwrap();
        if matches!(e.event, WorkerEvent::PortOpened { .. }) {
            break;
        }
        assert!(Instant::now() < deadline, "no PortOpened");
    }
    assert!(b.state().dtr_of(CU));
    let t0 = Instant::now();
    assert!(h.shutdown(Duration::from_millis(500)), "joined");
    let took = t0.elapsed();
    assert!(took <= Duration::from_millis(500), "{took:?}");
    assert!(h.is_finished());
    let st = b.state();
    assert_eq!(st.closes.len(), 1, "the port is closed");
    assert!(!st.dtr_of(CU), "DTR falls");
}

#[test]
fn c23_waker_called_and_snapshots_bounded() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&calls);
    let mut h = spawn_with(
        backend(),
        RealClock::new(),
        Rules::MACOS,
        LogHandle::disabled(),
        Box::new(move || {
            c.fetch_add(1, Ordering::SeqCst);
        }),
    )
    .unwrap();
    // Nobody reads for over a scan period: the channel stays at one.
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1200) {
        assert!(h.snapshots.len() <= 1);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        calls.load(Ordering::SeqCst) >= 2,
        "waker after each publish"
    );
    let s = h.snapshots.try_recv().unwrap();
    assert!(s.scans >= 1);
    assert!(h.shutdown(Duration::from_millis(500)));
}
