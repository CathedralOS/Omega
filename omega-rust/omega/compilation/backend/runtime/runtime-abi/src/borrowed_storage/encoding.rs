//! Shares readable geometry with the canonical bytes that bind it.

use super::*;
use semantic_vocabulary::{
    identity_encoding::{encode_integer_type, encode_scalar_type},
    structural_path_encoding::encode_canonical_structural_path_segment,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

mod decoding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedStorageEncodingError {
    pub layout: usize,
    pub reason: &'static str,
}

impl core::fmt::Display for BorrowedStorageEncodingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "borrowed storage layout {}: {}",
            self.layout, self.reason
        )
    }
}

impl std::error::Error for BorrowedStorageEncodingError {}

/// A checked, immutable roster, shared by current native forms. Its encoding
/// binds geometry only: it grants neither access nor ownership of the storage.
#[derive(Debug, Clone)]
pub struct NativeBorrowedStorageLayouts(Arc<SealedLayouts>);

#[derive(Debug)]
struct SealedLayouts {
    layouts: Vec<NativeBorrowedStorageLayout>,
    canonical: Vec<u8>,
    identity: [u8; 32],
}

impl NativeBorrowedStorageLayouts {
    /// Refuses unreadable arena coordinates and missing discriminator rows
    /// instead of encoding them as an absent field or case.
    pub fn seal(
        layouts: Vec<NativeBorrowedStorageLayout>,
    ) -> Result<Self, BorrowedStorageEncodingError> {
        let mut canonical = b"omega.native-borrowed-storage.v1\0".to_vec();
        count(&mut canonical, layouts.len());
        let mut identities: Vec<_> = layouts
            .iter()
            .enumerate()
            .map(|(index, layout)| (layout.structural_type, index))
            .collect();
        identities.sort_unstable();
        if let Some(duplicate) = identities.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(BorrowedStorageEncodingError {
                layout: duplicate[1].1,
                reason: "duplicate structural type",
            });
        }
        for (index, layout) in layouts.iter().enumerate() {
            encode_layout(&mut canonical, layout).map_err(|reason| {
                BorrowedStorageEncodingError {
                    layout: index,
                    reason,
                }
            })?;
        }
        let identity = Sha256::digest(&canonical).into();
        Ok(Self(Arc::new(SealedLayouts {
            layouts,
            canonical,
            identity,
        })))
    }

    pub fn layouts(&self) -> &[NativeBorrowedStorageLayout] {
        &self.0.layouts
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.0.canonical
    }

    /// Computed once from the complete, domain-separated canonical roster.
    pub fn identity_bytes(&self) -> [u8; 32] {
        self.0.identity
    }

    /// Decodes a foreign roster and checks its complete canonical encoding.
    pub fn decode(bytes: &[u8]) -> Result<Self, BorrowedStorageEncodingError> {
        decoding::decode(bytes)
    }
}

impl Default for NativeBorrowedStorageLayouts {
    fn default() -> Self {
        Self::seal(Vec::new()).expect("the empty layout roster is readable")
    }
}

impl PartialEq for NativeBorrowedStorageLayouts {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.canonical_bytes() == other.canonical_bytes()
    }
}

impl Eq for NativeBorrowedStorageLayouts {}

impl core::ops::Deref for NativeBorrowedStorageLayouts {
    type Target = [NativeBorrowedStorageLayout];

    fn deref(&self) -> &Self::Target {
        self.layouts()
    }
}

fn count(bytes: &mut Vec<u8>, length: usize) {
    // Rust's supported address spaces fit the wire's unsigned 64-bit length.
    bytes.extend_from_slice(&(length as u64).to_le_bytes());
}

fn rows<T: Default>(arena: &Arena<T>, span: HandleSpan<T>) -> Result<&[T], &'static str> {
    arena.span(span).ok_or("unreadable arena span")
}

fn path(
    bytes: &mut Vec<u8>,
    layout: &NativeBorrowedStorageLayout,
    span: HandleSpan<NativeStoragePathStep>,
    repetitions: HandleSpan<NativeStorageRepetition>,
) -> Result<(), &'static str> {
    let path = rows(&layout.paths, span)?;
    count(bytes, path.len());
    for step in path {
        encode_canonical_structural_path_segment(bytes, step.0);
    }
    let repetitions = rows(&layout.repetitions, repetitions)?;
    if repetitions
        .windows(2)
        .any(|pair| pair[0].path_position >= pair[1].path_position)
    {
        return Err("repeated or unordered array dimensions");
    }
    count(bytes, repetitions.len());
    for repetition in repetitions {
        if !matches!(
            path.get(repetition.path_position),
            Some(NativeStoragePathStep(
                CanonicalStructuralPathSegment::FixedIndex(_)
            ))
        ) {
            return Err("repetition does not name an array path segment");
        }
        count(bytes, repetition.path_position);
        bytes.extend_from_slice(&repetition.length.to_le_bytes());
        bytes.extend_from_slice(&repetition.byte_stride.to_le_bytes());
    }
    Ok(())
}

fn scalar(bytes: &mut Vec<u8>, scalar: ScalarType) -> Result<(), &'static str> {
    semantic_vocabulary::ClosedScalarType::try_from(scalar).map_err(|_| "unclosed scalar type")?;
    encode_scalar_type(bytes, scalar, Vec::push, encode_integer_type);
    Ok(())
}

fn unique_paths(
    layout: &NativeBorrowedStorageLayout,
    spans: impl Iterator<Item = HandleSpan<NativeStoragePathStep>>,
) -> Result<(), &'static str> {
    let mut paths = spans
        .map(|span| rows(&layout.paths, span))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort_unstable();
    if paths.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("duplicate storage path");
    }
    Ok(())
}

fn encode_layout(
    bytes: &mut Vec<u8>,
    layout: &NativeBorrowedStorageLayout,
) -> Result<(), &'static str> {
    bytes.extend_from_slice(&layout.structural_type.get().to_le_bytes());
    bytes.extend_from_slice(&layout.byte_size.to_le_bytes());
    bytes.extend_from_slice(&layout.alignment.to_le_bytes());
    let fields = rows(&layout.storage, layout.fields)?;
    unique_paths(layout, fields.iter().map(|field| field.path))?;
    count(bytes, fields.len());
    for field in fields {
        path(bytes, layout, field.path, field.repetitions)?;
        bytes.extend_from_slice(&field.byte_offset.to_le_bytes());
        scalar(bytes, field.logical_scalar)?;
        scalar(bytes, field.stored_scalar)?;
    }
    let discriminators = rows(&layout.discriminator_storage, layout.discriminators)?;
    let mut discriminator_paths = Vec::with_capacity(discriminators.len());
    count(bytes, discriminators.len());
    for slot in discriminators {
        let discriminator = slot.0.as_ref().ok_or("missing discriminator row")?;
        discriminator_paths.push(discriminator.path);
        path(bytes, layout, discriminator.path, discriminator.repetitions)?;
        bytes.extend_from_slice(&discriminator.structural_type.get().to_le_bytes());
        bytes.extend_from_slice(&discriminator.byte_offset.to_le_bytes());
        let cases = rows(&layout.cases, discriminator.cases)?;
        let mut mappings = cases
            .iter()
            .map(|slot| slot.0.ok_or("missing discriminator case row"))
            .collect::<Result<Vec<_>, _>>()?;
        mappings.sort_unstable_by_key(|mapping| mapping.case);
        if mappings.windows(2).any(|pair| pair[0].case == pair[1].case) {
            return Err("duplicate discriminator case");
        }
        mappings.sort_unstable_by_key(|mapping| mapping.native_tag);
        if mappings
            .windows(2)
            .any(|pair| pair[0].native_tag == pair[1].native_tag)
        {
            return Err("duplicate native discriminator tag");
        }
        count(bytes, cases.len());
        for slot in cases {
            let case = slot.0.as_ref().ok_or("missing discriminator case row")?;
            bytes.extend_from_slice(&case.case.get().to_le_bytes());
            bytes.extend_from_slice(&case.native_tag.to_le_bytes());
        }
    }
    unique_paths(layout, discriminator_paths.into_iter())?;
    let regions = rows(&layout.opaque_storage, layout.opaque_regions)?;
    unique_paths(layout, regions.iter().map(|region| region.path))?;
    count(bytes, regions.len());
    for region in regions {
        path(bytes, layout, region.path, region.repetitions)?;
        bytes.extend_from_slice(&region.byte_offset.to_le_bytes());
        bytes.extend_from_slice(&region.byte_size.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
