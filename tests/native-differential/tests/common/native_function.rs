//! Link validated function bytes with a host C caller; this does not emit instructions.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn assert_c_driver(layout: &StagedOptimizedResolvedSelectedFormLayout, driver: &str) {
    let bytes = function_bytes(layout);
    assert_c_text(&bytes, 0, driver);
}

/// Link already framed and internally relocated compiler text without rewriting it.
pub(crate) fn assert_c_text(bytes: &[u8], entry_offset: usize, driver: &str) {
    assert!(entry_offset < bytes.len());
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let symbol = if cfg!(target_os = "macos") {
        "_omega_entry"
    } else {
        "omega_entry"
    };
    let mut assembly = format!(
        ".text\n.p2align 4\n.Lomega_text:\n.byte {bytes}\n.globl {symbol}\n.set {symbol}, .Lomega_text + {entry_offset}\n"
    );
    if !cfg!(target_os = "macos") {
        assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    }
    compile_and_run(&assembly, driver);
}

fn compile_and_run(assembly: &str, driver: &str) {
    let directory = fresh_scratch_directory();
    let cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    std::fs::write(&assembly_path, assembly).expect("write native function assembly");
    std::fs::write(&driver_path, driver).expect("write native function caller");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host linker for native function");
    assert!(
        link.status.success(),
        "host linker rejected native function:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    let status = Command::new(&executable_path)
        .status()
        .expect("execute native function");
    assert_eq!(status.code(), Some(0), "native value mismatch");
    drop(cleanup);
}

fn function_bytes(layout: &StagedOptimizedResolvedSelectedFormLayout) -> Vec<u8> {
    let [function] = layout.functions() else {
        panic!("host corpus requires exactly one scalar function")
    };
    let byte_count = usize::try_from(function.byte_count).expect("function byte count fits usize");
    let mut bytes = vec![0_u8; byte_count];
    let mut written = vec![false; byte_count];
    for row in function.blocks.iter().flat_map(|block| &block.instructions) {
        let start = usize::try_from(row.offset).expect("row offset fits usize");
        let end = start
            .checked_add(row.bytes.len())
            .expect("row end fits usize");
        assert!(end <= bytes.len(), "row exceeds function layout");
        assert!(
            written[start..end].iter().all(|covered| !covered),
            "resolved rows overlap"
        );
        bytes[start..end].copy_from_slice(&row.bytes);
        written[start..end].fill(true);
    }
    assert!(
        written.iter().all(|covered| *covered),
        "layout contains a gap"
    );
    bytes
}

fn fresh_scratch_directory() -> PathBuf {
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "omega-native-function-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&path).expect("create native function scratch directory");
    path
}

struct ScratchDirectory(PathBuf);

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = std::fs::remove_dir_all(Path::new(&self.0));
    }
}
