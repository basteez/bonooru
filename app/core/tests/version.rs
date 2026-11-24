//! C.5 (build side): the version `build.rs` computed is consistent. The
//! parsing rule itself is unit-tested in `src/version_parse.rs`. CI prints
//! the line below into the job summary.

use ocb_core::version::{APP_VERSION, APP_VERSION_TRIPLE};

#[test]
fn c5_version_prints() {
    println!("OCB_APP_VERSION={APP_VERSION}");
    assert!(!APP_VERSION.is_empty());
    let [x, y, z] = APP_VERSION_TRIPLE;
    if APP_VERSION == "0.0.0-dev" {
        assert_eq!(APP_VERSION_TRIPLE, [0, 0, 0]);
    } else {
        assert!(
            APP_VERSION.starts_with(&format!("{x}.{y}.{z}")),
            "{APP_VERSION}"
        );
    }
}
