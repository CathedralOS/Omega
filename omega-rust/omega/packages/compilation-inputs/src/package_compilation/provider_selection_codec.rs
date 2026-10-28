//! Canonical retained selection payloads for the existing dependency handoff.
//!
//! Sources are interned once per payload, not repeated for each Build row. The
//! cache encoding retains physical custody; the comparison digest uses package
//! identity and relative source coordinates so moving a checkout changes no
//! semantic choice. Decoding supplies inert anchors, never execution authority.

use build_time_evaluation::provider_declarations::RetainedProviderSelection;
use filesystem_authority::observation_codec::{ObservationCursor, push_framed};
use language_semantics::declaration_selection::{
    BuildSelectionOrigin, CompositionMode, ProductTarget,
};
use semantic_vocabulary::PackageKeyIdentity;
use source::{DependencyScope, SourceFile, SourceId, SourceOrigin, SourceResolutionStratum, Span};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use symbols::{RetainedDeclaration, SymbolKind};

const MAGIC: &[u8] = b"OMG-BUILD-PROVIDER-CHOICES\x01";
const MAXIMUM_ROWS: u64 = 1 << 20;

pub(super) struct SelectionSources {
    pub(super) sources: Vec<SourceFile>,
}

impl SelectionSources {
    fn collect(rows: &[RetainedProviderSelection]) -> Self {
        let mut sources = Vec::new();
        for row in rows {
            for anchor in [
                &row.subject,
                &row.provider,
                row.origin.activation(),
                row.origin.author(),
            ] {
                if !sources.contains(anchor.source()) {
                    sources.push(anchor.source().clone());
                }
            }
        }
        Self { sources }
    }

    pub(super) fn anchor(&self, anchor: &RetainedDeclaration, output: &mut Vec<u8>) {
        let source_position = self
            .sources
            .iter()
            .position(|source| source == anchor.source())
            .expect("selection source pool contains every retained anchor");
        integer(output, source_position as u64);
        integer(output, anchor.span().start as u64);
        integer(output, anchor.span().end as u64);
        output.push(kind_tag(anchor.kind()));
    }

    fn rows(&self, rows: &[RetainedProviderSelection], observation: Option<[u8; 32]>) -> Vec<u8> {
        let mut output = Vec::new();
        match observation {
            None => output.push(0),
            Some(digest) => {
                output.push(1);
                output.extend_from_slice(&digest);
            }
        }
        integer(&mut output, rows.len() as u64);
        for row in rows {
            self.anchor(&row.subject, &mut output);
            push_framed(&mut output, row.subject_identity.as_bytes());
            self.anchor(&row.provider, &mut output);
            push_framed(&mut output, row.provider_identity.as_bytes());
            integer(&mut output, row.target.code());
            output.push(match row.composition_mode {
                CompositionMode::Fused => 0,
                CompositionMode::Independent => 1,
            });
            self.anchor(row.origin.activation(), &mut output);
            self.anchor(row.origin.author(), &mut output);
            integer(&mut output, row.origin.occurrence().start as u64);
            integer(&mut output, row.origin.occurrence().end as u64);
            match row.origin.event_ordinal() {
                None => output.push(0),
                Some(ordinal) => {
                    output.push(1);
                    integer(&mut output, ordinal);
                }
            }
        }
        output
    }
}

pub fn encode_package_provider_selections(
    rows: &[RetainedProviderSelection],
    observation: Option<[u8; 32]>,
) -> Vec<u8> {
    let pool = SelectionSources::collect(rows);
    let mut output = MAGIC.to_vec();
    encode_sources(&pool.sources, &mut output);
    output.extend_from_slice(&pool.rows(rows, observation));
    output
}

pub fn package_provider_selections_digest(
    rows: &[RetainedProviderSelection],
    observation: Option<[u8; 32]>,
) -> [u8; 32] {
    let pool = SelectionSources::collect(rows);
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-package-provider-choices-v1\0");
    digest.u64(pool.sources.len() as u64);
    for source in &pool.sources {
        let relative = source
            .path
            .strip_prefix(&source.package_root)
            .unwrap_or(&source.path);
        digest.bytes(relative.as_os_str().as_encoded_bytes());
        let mut owner = Vec::new();
        source_owner(source, &mut owner);
        digest.bytes(&owner);
        digest.raw(&crate::digest_encoder::content_identity(
            source.source.as_bytes(),
        ));
    }
    digest.bytes(&pool.rows(rows, observation));
    digest.finish()
}

pub fn decode_package_provider_selections(
    bytes: &[u8],
) -> Option<(Vec<RetainedProviderSelection>, Option<[u8; 32]>)> {
    let mut cursor = ObservationCursor::new(bytes);
    if cursor.bytes(MAGIC.len())? != MAGIC {
        return None;
    }
    let sources = decode_sources(&mut cursor)?;
    let observation = match cursor.u8()? {
        0 => None,
        1 => Some(cursor.bytes(32)?.try_into().ok()?),
        _ => return None,
    };
    let count = sequence_length(&mut cursor)?;
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        let subject = decode_anchor(&mut cursor, &sources)?;
        let subject_identity = std::str::from_utf8(cursor.framed()?).ok()?.to_owned();
        let provider = decode_anchor(&mut cursor, &sources)?;
        let provider_identity = std::str::from_utf8(cursor.framed()?).ok()?.to_owned();
        let target = ProductTarget::from_code(cursor.u64()?)?;
        let composition_mode = match cursor.u8()? {
            0 => CompositionMode::Fused,
            1 => CompositionMode::Independent,
            _ => return None,
        };
        let activation = decode_anchor(&mut cursor, &sources)?;
        let author = decode_anchor(&mut cursor, &sources)?;
        let occurrence = decode_span(&mut cursor)?;
        let ordinal = match cursor.u8()? {
            0 => None,
            1 => Some(cursor.u64()?),
            _ => return None,
        };
        let origin =
            BuildSelectionOrigin::from_retained_parts(activation, author, occurrence, ordinal)
                .ok()?;
        rows.push(RetainedProviderSelection {
            subject,
            subject_identity,
            provider,
            provider_identity,
            target,
            composition_mode,
            origin,
        });
    }
    cursor.exhausted().then_some((rows, observation))
}

pub(super) fn integer(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}
pub(super) fn sequence_length(cursor: &mut ObservationCursor<'_>) -> Option<usize> {
    let count = cursor.u64()?;
    (count <= MAXIMUM_ROWS)
        .then(|| usize::try_from(count).ok())
        .flatten()
}
pub(super) fn decode_span(cursor: &mut ObservationCursor<'_>) -> Option<Span> {
    Some(Span::new(
        usize::try_from(cursor.u64()?).ok()?,
        usize::try_from(cursor.u64()?).ok()?,
    ))
}
pub(super) fn decode_anchor(
    cursor: &mut ObservationCursor<'_>,
    sources: &[SourceFile],
) -> Option<RetainedDeclaration> {
    let source = sources.get(usize::try_from(cursor.u64()?).ok()?)?.clone();
    let span = decode_span(cursor)?;
    let kind = match cursor.u8()? {
        0 => SymbolKind::Machine,
        1 => SymbolKind::Trait,
        2 => SymbolKind::Conformance,
        3 => SymbolKind::Data,
        4 => SymbolKind::Operator,
        5 => SymbolKind::State,
        6 => SymbolKind::Field,
        _ => return None,
    };
    RetainedDeclaration::from_retained_parts(source, span, kind).ok()
}
fn kind_tag(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Machine => 0,
        SymbolKind::Trait => 1,
        SymbolKind::Conformance => 2,
        SymbolKind::Data => 3,
        SymbolKind::Operator => 4,
        SymbolKind::State => 5,
        SymbolKind::Field => 6,
        _ => unreachable!("provider selection anchors are admitted declaration roles"),
    }
}
pub(super) fn source_owner(source: &SourceFile, output: &mut Vec<u8>) {
    match source.package_identity {
        None => output.push(0),
        Some(owner) => {
            output.push(1);
            output.extend_from_slice(&owner.digest());
        }
    }
    output.push(match source.dependency_scope {
        DependencyScope::Product => 0,
        DependencyScope::Build => 1,
    });
    output.push(match source.origin {
        SourceOrigin::User => 0,
        SourceOrigin::Toolchain => 1,
    });
    output.push(match source.resolution_stratum {
        SourceResolutionStratum::Base => 0,
        SourceResolutionStratum::CurrentActivationExtension => 1,
        SourceResolutionStratum::DependencyActivationExtension => 2,
    });
}

#[cfg(unix)]
pub(super) fn encode_path(path: &Path, output: &mut Vec<u8>) {
    use std::os::unix::ffi::OsStrExt;
    output.push(0);
    push_framed(output, path.as_os_str().as_bytes());
}
#[cfg(unix)]
pub(super) fn decode_path(cursor: &mut ObservationCursor<'_>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    if cursor.u8()? != 0 {
        return None;
    }
    Some(std::ffi::OsString::from_vec(cursor.framed()?.to_vec()).into())
}
#[cfg(windows)]
pub(super) fn encode_path(path: &Path, output: &mut Vec<u8>) {
    use std::os::windows::ffi::OsStrExt;
    output.push(1);
    let bytes: Vec<_> = path
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect();
    push_framed(output, &bytes);
}
#[cfg(windows)]
pub(super) fn decode_path(cursor: &mut ObservationCursor<'_>) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    if cursor.u8()? != 1 {
        return None;
    }
    let bytes = cursor.framed()?;
    if bytes.len() % 2 != 0 {
        return None;
    }
    let words: Vec<_> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Some(std::ffi::OsString::from_wide(&words).into())
}

pub(super) fn encode_sources(sources: &[SourceFile], output: &mut Vec<u8>) {
    integer(output, sources.len() as u64);
    for source in sources {
        encode_path(&source.package_root, output);
        encode_path(&source.path, output);
        source_owner(source, output);
        push_framed(output, source.source.as_bytes());
    }
}

pub(super) fn decode_sources(cursor: &mut ObservationCursor<'_>) -> Option<Vec<SourceFile>> {
    let count = sequence_length(cursor)?;
    let mut sources = Vec::with_capacity(count);
    for _ in 0..count {
        let package_root = decode_path(cursor)?;
        let path = decode_path(cursor)?;
        let package_identity = match cursor.u8()? {
            0 => None,
            1 => Some(PackageKeyIdentity::from_digest(
                cursor.bytes(32)?.try_into().ok()?,
            )?),
            _ => return None,
        };
        let dependency_scope = match cursor.u8()? {
            0 => DependencyScope::Product,
            1 => DependencyScope::Build,
            _ => return None,
        };
        let origin = match cursor.u8()? {
            0 => SourceOrigin::User,
            1 => SourceOrigin::Toolchain,
            _ => return None,
        };
        let resolution_stratum = match cursor.u8()? {
            0 => SourceResolutionStratum::Base,
            1 => SourceResolutionStratum::CurrentActivationExtension,
            2 => SourceResolutionStratum::DependencyActivationExtension,
            _ => return None,
        };
        let source = Arc::from(std::str::from_utf8(cursor.framed()?).ok()?);
        sources.push(SourceFile {
            source_id: SourceId(0),
            package_root,
            path,
            package_identity,
            dependency_scope,
            origin,
            resolution_stratum,
            source,
        });
    }
    Some(sources)
}
