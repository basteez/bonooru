//! Build script: the app version from `git describe` (AD-14, Story 3.2 D5),
//! same command and rule as `firmware/cmake/ocb_version.cmake`. Never fails
//! the build: any git problem gives the fallback `0.0.0-dev`.

use std::path::Path;
use std::process::Command;

#[path = "src/version_parse.rs"]
#[allow(dead_code)]
mod version_parse;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_owned());
    let repo = Path::new(&manifest).join("../..");

    let describe = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args([
            "describe",
            "--tags",
            "--match",
            "v[0-9]*.[0-9]*.[0-9]*",
            "--long",
            "--dirty",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());

    let v = version_parse::parse_describe(describe.as_deref());
    let [x, y, z] = v.triple;
    println!("cargo:rustc-env=OCB_APP_VERSION={}", v.string);
    println!("cargo:rustc-env=OCB_APP_VERSION_MAJOR={x}");
    println!("cargo:rustc-env=OCB_APP_VERSION_MINOR={y}");
    println!("cargo:rustc-env=OCB_APP_VERSION_PATCH={z}");

    // Best effort: rebuild when HEAD or the tags move (CI always builds fresh).
    for p in [
        ".git/HEAD",
        ".git/refs/tags",
        ".git/packed-refs",
        ".git/index",
    ] {
        let path = repo.join(p);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/version_parse.rs");
}
