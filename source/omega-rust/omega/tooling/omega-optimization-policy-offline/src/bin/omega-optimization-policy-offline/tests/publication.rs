use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::publication::publish_new;

static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

fn temporary_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "omega-offline-policy-{name}-{}-{}",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed),
    ))
}

#[test]
fn publishes_exact_bytes_once() {
    let path = temporary_path("publish");
    publish_new(&path, b"canonical artifact").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"canonical artifact");
    assert!(publish_new(&path, b"replacement").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"canonical artifact");
    fs::remove_file(path).unwrap();
}
