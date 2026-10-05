//! Process-wide accounting of host reads of source-file bytes.
//!
//! One compilation consumes a fixed set of source files, and the honest cost of
//! loading them is one host read each. A stage that re-reads a file to compare
//! it against bytes the same compilation already holds spends a second read on
//! a question its own loader already answered, and nothing in the compile's
//! output shows that it happened. [`record_source_file_read`] is the single
//! event every reading site reports, so the count divided by the consumed-file
//! count is the compilation's read multiplier, which is the figure `TASKS.md`'s
//! source-read acceptance is stated in.
//!
//! The total is one relaxed `AtomicU64`: this is counting, not synchronizing,
//! and a load taken while another thread reads is not a coherent instant. It is
//! monotonic for the process, so a per-invocation figure is the difference
//! between two observations rather than the raw total.
//!
//! Setting `OMEGA_SOURCE_READS` additionally prints one
//! `source-read site=<site> path=<path> bytes=<count>` line per event to
//! stderr -- the same opt-in observation convention as `OMEGA_REVIEW_TIMINGS`
//! -- so a duplicate attributes to both its path and the operation that spent
//! it. An ordinary invocation pays one cached environment lookup for the whole
//! process and one relaxed increment per read.

use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

static SOURCE_FILE_READS: AtomicU64 = AtomicU64::new(0);

/// Monotonic host reads of source-file bytes since this process started.
pub fn source_file_read_count() -> u64 {
    SOURCE_FILE_READS.load(Ordering::Relaxed)
}

/// Account one host read of one source file's bytes. `site` names the reading
/// operation rather than the file's role, so a duplicate names the stage that
/// spent it instead of only the path that paid.
pub fn record_source_file_read(site: &'static str, path: &Path, bytes: u64) {
    SOURCE_FILE_READS.fetch_add(1, Ordering::Relaxed);
    if *reads_reported() {
        eprintln!(
            "source-read site={site} path={} bytes={bytes}",
            path.display()
        );
    }
}

fn reads_reported() -> &'static bool {
    static REPORTED: OnceLock<bool> = OnceLock::new();
    REPORTED.get_or_init(|| {
        std::env::var_os("OMEGA_SOURCE_READS").is_some_and(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_recorded_read_raises_the_total_by_one() {
        let before = source_file_read_count();
        record_source_file_read("accounting_test", Path::new("main.omg"), 11);
        record_source_file_read("accounting_test", Path::new("main.omg"), 11);
        assert_eq!(source_file_read_count(), before + 2);
    }
}
