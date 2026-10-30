//! One private source/check owner graph, never a portable IR or acceptance row.
//!
//! The compiler/library and first canonical root locate one invocation's whole
//! candidate, including its dependents. This avoids restoring unrelated projects.
//! Fresh per-request captures, package inputs, frontend keys and
//! complete preparation admission decide reuse later. Installing the whole
//! coordinate lineage before parsing keeps all declaration/body correspondences.
//! Live Build returns, resources and authority are absent from this record.
use super::PreparedSource;
use compilation_products::{RetainedCompilationChecks, SourcePreparation};
use compiler_storage::{Record, RecordLimits, RecordWriter};
use optimization_core::digest_encoder::{DomainDigestEncoder, content_identity, hex};
use package_source::storage::RetainedStorageLane;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

const MAGIC: &[u8; 8] = b"OMGSG01\0";
const MAX_BYTES: usize = 512 * 1024 * 1024;
const HEADER_BYTES: usize = 80;
static NEXT_WRITE: AtomicU64 = AtomicU64::new(0);

#[derive(compiler_storage::Record)]
pub(super) struct SourceGraph {
    pub preparation: SourcePreparation,
    pub sources: Vec<PreparedSource>,
    pub checks: RetainedCompilationChecks,
}

pub(super) struct SourceGraphCache {
    storage: RetainedStorageLane,
    compiler: [u8; 32],
    name: String,
}
impl SourceGraphCache {
    pub fn new(storage: RetainedStorageLane) -> Result<Self, String> {
        let storage = storage
            .retain_child("source-graphs")
            .map_err(|reason| reason.to_string())?;
        let compiler = crate::compiler_input_identity()?;
        let name = format!("{}.rec", hex(&compiler));
        Ok(Self {
            storage,
            compiler,
            name,
        })
    }
    pub fn scope_to_root(&mut self, root: &Path) {
        // A private lookup coordinate, never source identity or admission.
        // The session binds this once, before installing any source coordinates.
        let mut digest = DomainDigestEncoder::new(b"omega-source-graph-root-v1\0");
        digest.raw(&self.compiler);
        digest.bytes(root.as_os_str().as_encoded_bytes());
        self.name = format!("{}.rec", hex(&digest.finish()));
        if phase_profile::enabled() {
            eprintln!(
                "compiler source record: {}",
                self.storage.path().join(&self.name).display()
            );
        }
    }
    fn require_current(&self) -> Result<(), String> {
        self.storage
            .verify_path_identity()
            .map_err(|reason| reason.to_string())?;
        if crate::compiler_input_identity()? != self.compiler {
            return Err("compiler or selected library changed during source retention".to_owned());
        }
        Ok(())
    }
    pub fn load(&self) -> Result<Option<SourceGraph>, String> {
        phase_profile::scope("source.graph.load.require_before", || {
            self.require_current()
        })?;
        let file = match self.storage.directory().open(&self.name) {
            Ok(file) => file,
            Err(reason) if reason.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(reason) => return Err(format!("open retained source graph: {reason}")),
        };
        let read_limit = (MAX_BYTES + HEADER_BYTES + 1) as u64;
        let capacity = file
            .metadata()
            .map_err(|reason| format!("inspect retained source graph: {reason}"))?
            .len()
            .saturating_add(1)
            .min(read_limit) as usize;
        let mut bytes = Vec::new();
        // Keep one spare byte for the EOF probe without growing the buffer.
        bytes
            .try_reserve_exact(capacity)
            .map_err(|reason| format!("reserve retained source graph: {reason}"))?;
        phase_profile::scope("source.graph.load.read", || {
            file.take(read_limit)
                .read_to_end(&mut bytes)
                .map_err(|reason| format!("read retained source graph: {reason}"))
        })?;
        if bytes.len() < HEADER_BYTES
            || bytes.len() > MAX_BYTES + HEADER_BYTES
            || &bytes[..8] != MAGIC
            || bytes[8..40] != self.compiler
        {
            return Err(
                "retained source graph has a foreign schema/compiler or invalid size".to_owned(),
            );
        }
        let length = u64::from_le_bytes(
            bytes[72..80]
                .try_into()
                .map_err(|reason| format!("retained source graph length: {reason}"))?,
        );
        if length != (bytes.len() - HEADER_BYTES) as u64
            || bytes[40..72]
                != phase_profile::scope("source.graph.load.digest", || {
                    content_identity(&bytes[HEADER_BYTES..])
                })
        {
            return Err(
                "retained source graph is truncated or has a different payload digest".to_owned(),
            );
        }
        let graph = phase_profile::scope("source.graph.load.decode", || {
            compiler_storage::decode(&bytes[HEADER_BYTES..], limits())
                .map_err(|reason| format!("decode retained source graph: {reason}"))
        })?;
        phase_profile::scope("source.graph.load.require_after", || self.require_current())?;
        Ok(Some(graph))
    }
    fn prune(&self) -> Result<(), String> {
        let directory = self.storage.directory();
        let mut records = Vec::new();
        let mut total_bytes = 0u64;
        for entry in directory
            .entries()
            .map_err(|reason| format!("inspect source graph storage: {reason}"))?
        {
            let entry = entry.map_err(|reason| format!("inspect source graph entry: {reason}"))?;
            let name = entry.file_name();
            let Some(text) = name.to_str() else {
                continue;
            };
            if text.len() != 68
                || !text.ends_with(".rec")
                || !text[..64].bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                continue;
            }
            let metadata = entry
                .metadata()
                .map_err(|reason| format!("inspect source graph metadata: {reason}"))?;
            if !metadata.is_file() {
                continue;
            }
            total_bytes = total_bytes.saturating_add(metadata.len());
            records.push((metadata.modified().ok(), name, metadata.len()));
        }
        records.sort_unstable();
        let mut count = records.len();
        for (_, name, length) in records {
            // Root-scoped records share the existing byte budget. Keep a bounded
            // number of small projects instead of evicting after four roots.
            if count <= 64 && total_bytes <= MAX_BYTES as u64 {
                break;
            }
            if name == std::ffi::OsStr::new(&self.name) {
                continue;
            }
            directory
                .remove_file(name)
                .map_err(|reason| format!("prune source graph storage: {reason}"))?;
            count -= 1;
            total_bytes = total_bytes.saturating_sub(length);
        }
        Ok(())
    }
    pub fn store(
        &self,
        preparation: &SourcePreparation,
        sources: &[PreparedSource],
        checks: &RetainedCompilationChecks,
    ) -> Result<(), String> {
        // Borrow the complete owner rather than cloning its source/checked
        // arenas merely to encode. Field order is SourceGraph's record schema.
        let mut output = RecordWriter::new(limits());
        output
            .record(|output| {
                preparation.write(output)?;
                let preparation_bytes = output.bytes_written();
                phase_profile::count("source.graph.preparation.bytes", preparation_bytes as u64);
                output.record(|output| {
                    output.sequence(sources.len())?;
                    for source in sources {
                        source.write(output)?;
                    }
                    Ok(())
                })?;
                let source_bytes = output.bytes_written();
                phase_profile::count(
                    "source.graph.sources.bytes",
                    (source_bytes - preparation_bytes) as u64,
                );
                checks.write(output)?;
                phase_profile::count(
                    "source.graph.checks.bytes",
                    (output.bytes_written() - source_bytes) as u64,
                );
                Ok(())
            })
            .map_err(|reason| format!("encode retained source graph: {reason}"))?;
        let payload = output.finish();
        self.require_current()?;
        let temporary = format!(
            ".{}-{}.tmp",
            std::process::id(),
            NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
        );
        let directory = self.storage.directory();
        let write = (|| {
            let mut options = cap_std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            let mut file = directory
                .open_with(&temporary, &options)
                .map_err(|reason| format!("stage retained source graph: {reason}"))?;
            file.write_all(MAGIC)
                .and_then(|_| file.write_all(&self.compiler))
                .and_then(|_| file.write_all(&content_identity(&payload)))
                .and_then(|_| file.write_all(&(payload.len() as u64).to_le_bytes()))
                .and_then(|_| file.write_all(&payload))
                .map_err(|reason| format!("write retained source graph: {reason}"))?;
            drop(file);
            self.require_current()?;
            match directory.rename(&temporary, directory, &self.name) {
                Ok(()) => Ok(()),
                Err(reason) if reason.kind() == std::io::ErrorKind::AlreadyExists => {
                    directory
                        .remove_file(&self.name)
                        .map_err(|reason| format!("replace retained source graph: {reason}"))?;
                    directory
                        .rename(&temporary, directory, &self.name)
                        .map_err(|reason| format!("publish retained source graph: {reason}"))
                }
                Err(reason) => Err(format!("publish retained source graph: {reason}")),
            }
        })();
        if let Err(reason) = directory.remove_file(&temporary)
            && reason.kind() != std::io::ErrorKind::NotFound
        {
            report_miss(&format!("remove retained source graph temporary: {reason}"));
        }
        write?;
        self.prune()?;
        phase_profile::count("source.graph.retained", 1);
        Ok(())
    }
}
fn limits() -> RecordLimits {
    RecordLimits {
        bytes: MAX_BYTES,
        allocation: 4 * 1024 * 1024 * 1024,
        elements: 64 * 1024 * 1024,
        depth: 256,
    }
}
pub(super) fn report_miss(reason: &str) {
    phase_profile::count("source.graph.invalid", 1);
    if phase_profile::enabled() {
        eprintln!("retained source graph ignored: {reason}");
    }
}
