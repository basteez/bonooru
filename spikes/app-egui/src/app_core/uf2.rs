//! UF2 flasher logic: find a mounted RP2040 bootloader volume (`RPI-RP2`), validate a UF2,
//! copy it, wait for the volume to go away. GUI-free.
//!
//! This spike never targets a real board on purpose (it would erase working firmware). The UI
//! only exposes a "simulated drive" directory; `list_candidate_volumes` is shown read-only.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const VOLUME_LABEL: &str = "RPI-RP2";
pub const INFO_FILE: &str = "INFO_UF2.TXT";
pub const RP2040_FAMILY: u32 = 0xE48B_FF56;
const MAGIC0: u32 = 0x0A32_4655;
const MAGIC1: u32 = 0x9E5D_5157;
const MAGIC_END: u32 = 0x0AB1_6F30;
const FLAG_FAMILY: u32 = 0x0000_2000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootVolume {
    pub mount_point: PathBuf,
    pub label: String,
    pub model: Option<String>,
    pub board_id: Option<String>,
}

/// Decide whether a mounted volume is an RP2040 bootloader drive.
///
/// `label` is whatever the OS reports: the volume name on macOS/Windows, but on Linux
/// `sysinfo` reports the device (`/dev/sdb1`), so we also accept a mount point whose last
/// component is `RPI-RP2` (udisks mounts at `/media/$USER/RPI-RP2`). In all cases the volume
/// must contain `INFO_UF2.TXT`.
pub fn probe(mount_point: &Path, label: &str) -> Option<BootVolume> {
    let label_ok = label.eq_ignore_ascii_case(VOLUME_LABEL)
        || mount_point
            .file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(VOLUME_LABEL));
    let info = fs::read_to_string(mount_point.join(INFO_FILE)).ok()?;
    let (model, board_id) = parse_info(&info);
    // Accept a mislabelled volume only if INFO_UF2.TXT itself says RP2.
    let info_ok = board_id
        .as_deref()
        .is_some_and(|b| b.starts_with("RPI-RP2"))
        || model.as_deref().is_some_and(|m| m.contains("RP2"));
    (label_ok || info_ok).then(|| BootVolume {
        mount_point: mount_point.to_path_buf(),
        label: label.to_string(),
        model,
        board_id,
    })
}

pub fn parse_info(text: &str) -> (Option<String>, Option<String>) {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_string()))
    };
    (field("Model:"), field("Board-ID:"))
}

/// All mounted volumes as `(mount_point, label)`, via `sysinfo`.
pub fn mounted_volumes() -> Vec<(PathBuf, String)> {
    let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else {
        return Vec::new();
    };
    disks
        .iter()
        .map(|d| {
            (
                d.mount_point().to_path_buf(),
                d.name().to_string_lossy().into_owned(),
            )
        })
        .collect()
}

/// Mounted RP2040 boot volumes, plus an optional simulated one (a plain directory).
pub fn find_boot_volumes(simulated: Option<&Path>) -> Vec<BootVolume> {
    let mut out: Vec<BootVolume> = mounted_volumes()
        .iter()
        .filter_map(|(m, l)| probe(m, l))
        .collect();
    if let Some(dir) = simulated
        && let Some(v) = probe(dir, VOLUME_LABEL)
    {
        out.push(v);
    }
    out
}

#[derive(Debug, PartialEq, Eq)]
pub struct Uf2Summary {
    pub blocks: u32,
    pub family: Option<u32>,
    pub bytes: usize,
}

/// Validate every 512-byte block: magic numbers, block numbering and a single family ID.
pub fn validate_uf2(data: &[u8]) -> Result<Uf2Summary, String> {
    if data.is_empty() || !data.len().is_multiple_of(512) {
        return Err(format!("size {} is not a multiple of 512", data.len()));
    }
    let le = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let mut total = None;
    let mut family = None;
    for (i, b) in data.as_chunks::<512>().0.iter().enumerate() {
        if le(b, 0) != MAGIC0 || le(b, 4) != MAGIC1 || le(b, 508) != MAGIC_END {
            return Err(format!("block {i}: bad magic"));
        }
        let (flags, block_no, num_blocks) = (le(b, 8), le(b, 20), le(b, 24));
        if block_no as usize != i {
            return Err(format!("block {i}: block number {block_no}"));
        }
        if *total.get_or_insert(num_blocks) != num_blocks {
            return Err(format!("block {i}: inconsistent block count"));
        }
        if flags & FLAG_FAMILY != 0 {
            let f = le(b, 28);
            if *family.get_or_insert(f) != f {
                return Err(format!("block {i}: mixed family IDs"));
            }
        }
    }
    let blocks = total.unwrap_or(0);
    if blocks as usize != data.len() / 512 {
        return Err(format!(
            "header says {blocks} blocks, file has {}",
            data.len() / 512
        ));
    }
    Ok(Uf2Summary {
        blocks,
        family,
        bytes: data.len(),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    Copying { written: usize, total: usize },
    Copied,
    WaitingForReboot,
    Done { reboot_after: Duration },
}

#[derive(Debug, PartialEq, Eq)]
pub enum FlashError {
    Invalid(String),
    WrongFamily(u32),
    Io(String),
    Timeout,
}

impl std::fmt::Display for FlashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlashError::Invalid(e) => write!(f, "invalid UF2: {e}"),
            FlashError::WrongFamily(x) => write!(f, "UF2 family {x:#010x} is not RP2040"),
            FlashError::Io(e) => write!(f, "I/O error: {e}"),
            FlashError::Timeout => write!(f, "volume did not disappear (board did not reboot?)"),
        }
    }
}

fn io_err(e: io::Error) -> FlashError {
    FlashError::Io(e.to_string())
}

/// Copy `uf2` onto the boot volume and wait until it disappears (the board reboots after the
/// last block). Uses a plain read/write loop, not `fs::copy`: on macOS `fs::copy` uses
/// `fcopyfile`, which also tries to copy extended attributes, a known failure mode on
/// `RPI-RP2` (the "cp: could not copy extended attributes" error).
pub fn flash(
    uf2: &Path,
    volume: &Path,
    timeout: Duration,
    poll: Duration,
    mut progress: impl FnMut(Progress),
) -> Result<(), FlashError> {
    let mut data = Vec::new();
    fs::File::open(uf2)
        .and_then(|mut f| f.read_to_end(&mut data))
        .map_err(io_err)?;
    let summary = validate_uf2(&data).map_err(FlashError::Invalid)?;
    if let Some(f) = summary.family
        && f != RP2040_FAMILY
    {
        return Err(FlashError::WrongFamily(f));
    }
    let name = uf2.file_name().unwrap_or("firmware.uf2".as_ref());
    let mut out = fs::File::create(volume.join(name)).map_err(io_err)?;
    for (i, chunk) in data.chunks(64 * 1024).enumerate() {
        out.write_all(chunk).map_err(io_err)?;
        progress(Progress::Copying {
            written: (i * 64 * 1024 + chunk.len()).min(data.len()),
            total: data.len(),
        });
    }
    // The volume may vanish during sync on a real board; that is success, not an error.
    let _ = out.sync_all();
    drop(out);
    progress(Progress::Copied);
    progress(Progress::WaitingForReboot);
    let t0 = Instant::now();
    while volume.join(INFO_FILE).exists() {
        if t0.elapsed() > timeout {
            return Err(FlashError::Timeout);
        }
        std::thread::sleep(poll);
    }
    progress(Progress::Done {
        reboot_after: t0.elapsed(),
    });
    Ok(())
}

/// Build a syntactically valid RP2040 UF2 with `blocks` blocks of zero payload (for tests and
/// the simulated drive). It is never written to a real board.
pub fn dummy_uf2(blocks: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(blocks as usize * 512);
    for i in 0..blocks {
        let mut b = [0u8; 512];
        let mut put = |o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        put(0, MAGIC0);
        put(4, MAGIC1);
        put(8, FLAG_FAMILY);
        put(12, 0x1000_0000 + i * 256);
        put(16, 256);
        put(20, i);
        put(24, blocks);
        put(28, RP2040_FAMILY);
        put(508, MAGIC_END);
        out.extend_from_slice(&b);
    }
    out
}

/// Create a fake `RPI-RP2` directory with `INFO_UF2.TXT` and a dummy UF2 next to it.
/// Returns `(drive_dir, uf2_path)`.
pub fn make_simulated_drive(root: &Path) -> io::Result<(PathBuf, PathBuf)> {
    let drive = root.join(VOLUME_LABEL);
    fs::create_dir_all(&drive)?;
    fs::write(
        drive.join(INFO_FILE),
        "UF2 Bootloader v3.0\nModel: Raspberry Pi RP2\nBoard-ID: RPI-RP2\n",
    )?;
    let uf2 = root.join("dummy-bridge.uf2");
    fs::write(&uf2, dummy_uf2(64))?;
    Ok((drive, uf2))
}

/// Mimic the board: once a `.uf2` appears in `drive`, wait `delay` and "reboot" by removing
/// `INFO_UF2.TXT` and the copied file. Runs on its own thread.
pub fn spawn_fake_bootloader(drive: PathBuf, delay: Duration) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(30) {
            let uf2 = fs::read_dir(&drive).ok().and_then(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("uf2")))
            });
            if let Some(p) = uf2 {
                std::thread::sleep(delay);
                let _ = fs::remove_file(p);
                let _ = fs::remove_file(drive.join(INFO_FILE));
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("uf2-spike-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn parse_info_file() {
        let (m, b) =
            parse_info("UF2 Bootloader v3.0\nModel: Raspberry Pi RP2\nBoard-ID: RPI-RP2\n");
        assert_eq!(m.as_deref(), Some("Raspberry Pi RP2"));
        assert_eq!(b.as_deref(), Some("RPI-RP2"));
    }

    #[test]
    fn probe_rules() {
        let root = tmp("probe");
        let (drive, _) = make_simulated_drive(&root).unwrap();
        // macOS/Windows: label match
        assert!(probe(&drive, "RPI-RP2").is_some());
        // Linux: label is a device path, mount dir name matches
        assert!(probe(&drive, "/dev/sdb1").is_some());
        // no INFO file -> never a boot volume
        let empty = root.join("RPI-RP2-empty");
        fs::create_dir_all(&empty).unwrap();
        assert!(probe(&empty, "RPI-RP2").is_none());
        // a random USB stick with a random INFO file
        let other = root.join("STICK");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join(INFO_FILE), "Model: Adafruit\nBoard-ID: SAMD21\n").unwrap();
        assert!(probe(&other, "STICK").is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validate_dummy_and_reject_bad() {
        let ok = dummy_uf2(8);
        let s = validate_uf2(&ok).unwrap();
        assert_eq!(s.blocks, 8);
        assert_eq!(s.family, Some(RP2040_FAMILY));
        assert!(validate_uf2(&ok[..511]).is_err());
        let mut bad = ok.clone();
        bad[0] = 0;
        assert!(validate_uf2(&bad).is_err());
        let mut renum = ok.clone();
        renum[512 + 20] = 5;
        assert!(validate_uf2(&renum).is_err());
    }

    #[test]
    fn flash_simulated_drive_end_to_end() {
        let root = tmp("flash");
        let (drive, uf2) = make_simulated_drive(&root).unwrap();
        let fake = spawn_fake_bootloader(drive.clone(), Duration::from_millis(50));
        let mut seen = vec![];
        flash(
            &uf2,
            &drive,
            Duration::from_secs(5),
            Duration::from_millis(10),
            |p| seen.push(p),
        )
        .unwrap();
        fake.join().unwrap();
        assert!(matches!(seen.last(), Some(Progress::Done { .. })));
        assert!(seen.contains(&Progress::Copied));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn flash_times_out_if_volume_stays() {
        let root = tmp("timeout");
        let (drive, uf2) = make_simulated_drive(&root).unwrap();
        let r = flash(
            &uf2,
            &drive,
            Duration::from_millis(100),
            Duration::from_millis(10),
            |_| {},
        );
        assert_eq!(r, Err(FlashError::Timeout));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn flash_rejects_invalid_file() {
        let root = tmp("invalid");
        let (drive, _) = make_simulated_drive(&root).unwrap();
        let bad = root.join("bad.uf2");
        fs::write(&bad, b"not a uf2").unwrap();
        let r = flash(
            &bad,
            &drive,
            Duration::from_millis(100),
            Duration::from_millis(10),
            |_| {},
        );
        assert!(matches!(r, Err(FlashError::Invalid(_))));
        assert!(
            !drive.join("bad.uf2").exists(),
            "nothing copied on validation failure"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
