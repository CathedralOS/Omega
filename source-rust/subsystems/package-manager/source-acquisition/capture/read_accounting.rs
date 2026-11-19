use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

static SOURCE_FILE_READS: AtomicU64 = AtomicU64::new(0);

pub fn source_file_read_count() -> u64 {
    SOURCE_FILE_READS.load(Ordering::Relaxed)
}

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
    fn recorded_reads_are_included_in_the_total() {
        let before = source_file_read_count();
        record_source_file_read("accounting_test", Path::new("main.omg"), 11);
        record_source_file_read("accounting_test", Path::new("main.omg"), 11);
        assert!(source_file_read_count() >= before + 2);
    }
}
