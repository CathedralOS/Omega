//! Host execution oracle for one complete optimized function body.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use omega_selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn assert_u64_result(layout: &StagedOptimizedResolvedSelectedFormLayout, expected: u64) {
    let bytes = function_bytes(layout);
    let directory = fresh_scratch_directory();
    let cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    std::fs::write(&assembly_path, assembly(&bytes)).expect("write optimizer corpus assembly");
    std::fs::write(
        &driver_path,
        format!(
            "#include <stdint.h>\nextern uint64_t omega_entry(uint8_t);\nint main(void) {{ return omega_entry(0) == {expected}ULL && omega_entry(1) == {expected}ULL ? 0 : 1; }}\n"
        ),
    )
    .expect("write optimizer corpus driver");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host linker for optimizer corpus");
    assert!(
        link.status.success(),
        "host linker rejected optimized corpus function:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    let status = Command::new(&executable_path)
        .status()
        .expect("execute optimized corpus function");
    assert_eq!(status.code(), Some(0), "optimized corpus value mismatch");
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

fn assembly(bytes: &[u8]) -> String {
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    if cfg!(target_os = "macos") {
        format!(".text\n.globl _omega_entry\n.p2align 2\n_omega_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl omega_entry\n.type omega_entry,@function\nomega_entry:\n.byte {bytes}\n.size omega_entry, .-omega_entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    }
}

fn fresh_scratch_directory() -> PathBuf {
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "omega-optimizer-corpus-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&path).expect("create optimizer corpus scratch directory");
    path
}

struct ScratchDirectory(PathBuf);

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = std::fs::remove_dir_all(Path::new(&self.0));
    }
}
