/*
The local directory that keeps checked dependency slices between compiles.

Corpus fixtures and ordinary builds that depend on the bundled library check
the same library Build programs again in every process. The Psi checker keys
each such check exactly and frames the record (`psi_compiler::CheckedSliceStore`);
this module only decides where records live and writes them so a reader never
sees a partial file. Records are written once and never rewritten: a key names
one exact input, so an existing record is already the right one. The directory
holds at most `MAX_BYTES`; publishing past that removes the oldest records
first. A record removed while another process reads it is only a miss there.

The directory is `CathedralOS/Omega/checked-slices/v1` beneath the per-user
cache base. `OMEGA_CHECKED_SLICE_CACHE=off` turns the store off for one
process, and an absolute path replaces the directory. An unusable directory
also turns it off; every check then runs as before.
*/

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

const VARIABLE: &str = "OMEGA_CHECKED_SLICE_CACHE";
const COMPONENTS: &[&str] = &["CathedralOS", "Omega", "checked-slices", "v1"];
const EXTENSION: &str = "rec";
const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;

static STORE: OnceLock<Option<CheckedSliceDirectory>> = OnceLock::new();
static NEXT_WRITE: AtomicU64 = AtomicU64::new(0);

struct CheckedSliceDirectory {
    directory: PathBuf,
    producer: [u8; 32],
}

/// The process's store, or `None` when it is turned off or unavailable.
pub(crate) fn shared() -> Option<&'static dyn psi_compiler::CheckedSliceStore> {
    STORE
        .get_or_init(CheckedSliceDirectory::open)
        .as_ref()
        .map(|store| store as &dyn psi_compiler::CheckedSliceStore)
}

impl CheckedSliceDirectory {
    fn open() -> Option<Self> {
        let directory = match std::env::var_os(VARIABLE) {
            Some(value) if value == "off" => return None,
            Some(value) => {
                let path = PathBuf::from(value);
                path.is_absolute().then_some(path)?
            }
            None => {
                let mut path = package_source::storage::current_user_cache_base().ok()?;
                path.extend(COMPONENTS);
                path
            }
        };
        std::fs::create_dir_all(&directory).ok()?;
        Some(Self {
            directory,
            producer: crate::compiler_input_identity().ok()?,
        })
    }

    fn path(&self, key: &[u8; 32]) -> PathBuf {
        let mut name = String::with_capacity(68);
        for byte in key {
            use std::fmt::Write as _;
            let _ = write!(name, "{byte:02x}");
        }
        name.push('.');
        name.push_str(EXTENSION);
        self.directory.join(name)
    }

    /*
        Removes the oldest records until `incoming` more bytes fit the budget.
        A record larger than the whole budget is never kept.
    */
    fn make_room(&self, incoming: u64) -> bool {
        if incoming > MAX_BYTES {
            return false;
        }
        let Ok(entries) = std::fs::read_dir(&self.directory) else {
            return false;
        };
        let mut records: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == EXTENSION)
            })
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                Some((metadata.modified().ok()?, metadata.len(), entry.path()))
            })
            .collect();
        let mut total: u64 = records.iter().map(|(_, length, _)| length).sum();
        records.sort();
        for (_, length, path) in records {
            if total + incoming <= MAX_BYTES {
                break;
            }
            if std::fs::remove_file(path).is_ok() {
                total -= length;
            }
        }
        total + incoming <= MAX_BYTES
    }
}

impl psi_compiler::CheckedSliceStore for CheckedSliceDirectory {
    fn producer_identity(&self) -> [u8; 32] {
        self.producer
    }

    fn load(&self, key: &[u8; 32]) -> Option<Vec<u8>> {
        std::fs::read(self.path(key)).ok()
    }

    fn save(&self, key: &[u8; 32], record: &[u8]) {
        let path = self.path(key);
        if path.exists() || !self.make_room(record.len() as u64) {
            return;
        }
        let temporary = self.directory.join(format!(
            ".{}.{}.tmp",
            std::process::id(),
            NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
        ));
        let written = std::fs::File::create(&temporary)
            .and_then(|mut file| file.write_all(record).and_then(|()| file.sync_all()));
        if written.is_err() || std::fs::rename(&temporary, &path).is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CheckedSliceDirectory;
    use psi_compiler::CheckedSliceStore;

    #[test]
    fn a_record_is_published_whole_and_never_replaced() {
        let directory = std::env::temp_dir().join(format!(
            "omega-checked-slices-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let store = CheckedSliceDirectory {
            directory: directory.clone(),
            producer: [0; 32],
        };
        assert_eq!(store.load(&[1; 32]), None);
        store.save(&[1; 32], b"first");
        store.save(&[1; 32], b"second");
        assert_eq!(store.load(&[1; 32]).as_deref(), Some(&b"first"[..]));
        let names: Vec<_> = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names.len(), 1, "no temporary file survives: {names:?}");
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
