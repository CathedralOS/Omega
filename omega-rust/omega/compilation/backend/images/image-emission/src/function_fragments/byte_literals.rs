//! Place every addressed byte literal's octets in the image's read-only
//! mapping and bind each literal address field to them.
//!
//! A quoted literal's view denotes immutable image storage. Text placement
//! leaves each literal address field zero and names the literal by its
//! establishing Terminal operation and place; the octets themselves are
//! rejoined here from the retained abstract operation, never from selected
//! code. Identical octets share one pool entry. The pool is a single
//! object-kind text symbol after the program and compiler-private code, so the
//! final image maps it read-only beside the code on every container format.
//! It is padded to a 16-byte boundary so text appended later stays aligned.
//!
//! Replay recomputes the pool from the same retained source and compares the
//! symbol, bytes, carriers and relocation rows exactly.

use super::{Error, host, source};
use crate::{ObjectArtifact, ObjectByteLiteralAddress, ObjectByteLiteralPool, ObjectFunction};
use abstract_operations::AbstractOperation;
use machine_code::{ImageAddressFields, PlacedImageAddress};
use object_file::{
    ObjectPlan, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord, SectionKind,
    StagedOptimizedRelocationFreeObjectContainer, SymbolKind, SymbolPlan, SymbolSection,
};
use std::collections::BTreeMap;

pub(super) const POOL_SYMBOL_NAME: &str = "omega_byte_literal_pool";
const POOL_ALIGNMENT: usize = 16;

/// Append the pool to `text_bytes` and declare its symbol. Returns `None`
/// when the program addresses no literal.
pub(super) fn place(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    object: &mut ObjectPlan,
    text_bytes: &mut Vec<u8>,
) -> Result<Option<ObjectByteLiteralPool>, Error> {
    let Some((pool_bytes, addresses)) = pool_contents(source, text_bytes.len())? else {
        return Ok(None);
    };
    let text_offset = text_bytes.len();
    if object
        .layout
        .symbols
        .iter()
        .any(|(_, symbol)| symbol.name == POOL_SYMBOL_NAME)
    {
        return Err(Error::Mismatch("byte-literal pool symbol name collision"));
    }
    let symbol = object.layout.symbols.insert(SymbolPlan {
        name: POOL_SYMBOL_NAME.to_owned(),
        section: SymbolSection::Section(SectionKind::Text),
        offset: text_offset,
        size: pool_bytes.len(),
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    text_bytes.extend_from_slice(&pool_bytes);
    Ok(Some(ObjectByteLiteralPool {
        symbol,
        text_offset,
        byte_count: pool_bytes.len(),
        addresses,
    }))
}

/// One relocation per address field, bound to the pool symbol with the
/// literal's pool offset as addend. Appended after the import rows.
pub(super) fn publish_relocations(
    pool: Option<&ObjectByteLiteralPool>,
    functions: &[ObjectFunction],
    relocations: &mut RelocationPlan,
) -> Result<(), Error> {
    let Some(pool) = pool else {
        return Ok(());
    };
    for address in &pool.addresses {
        for record in expected_records(pool, address, functions)? {
            relocations.push_record(record);
        }
    }
    Ok(())
}

/// Exact replay of the pool: bytes after the code, its one symbol, the
/// carrier rows and every relocation record in order.
pub(super) fn validate(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    artifact: &ObjectArtifact,
    pool_text_offset: usize,
) -> Result<(), Error> {
    let expected = pool_contents(source, pool_text_offset)?;
    let (pool, (pool_bytes, addresses)) = match (&artifact.byte_literal_pool, expected) {
        (None, None) => {
            if artifact.text_bytes.len() != pool_text_offset {
                return Err(Error::Mismatch("byte-literal pool bytes without literals"));
            }
            return Ok(());
        }
        (Some(pool), Some(expected)) => (pool, expected),
        _ => return Err(Error::Mismatch("byte-literal pool roster changed")),
    };
    let layout = &artifact.object.layout;
    let symbol = layout
        .symbols
        .is_valid(pool.symbol)
        .then(|| layout.symbols.get(pool.symbol))
        .ok_or(Error::Mismatch("byte-literal pool symbol is missing"))?;
    if pool.text_offset != pool_text_offset
        || pool.byte_count != pool_bytes.len()
        || pool.addresses != addresses
        || artifact.text_bytes.get(pool_text_offset..) != Some(pool_bytes.as_slice())
        || symbol.name != POOL_SYMBOL_NAME
        || symbol.section != SymbolSection::Section(SectionKind::Text)
        || symbol.offset != pool_text_offset
        || symbol.size != pool_bytes.len()
        || symbol.kind != SymbolKind::Object
        || !symbol.import_library.is_empty()
    {
        return Err(Error::Mismatch(
            "byte-literal pool differs from its literals",
        ));
    }
    let expected_records = pool
        .addresses
        .iter()
        .map(|address| expected_records(pool, address, &artifact.functions))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let actual = artifact
        .relocations
        .records()
        .filter(|(_, record)| record.symbol_handle == pool.symbol)
        .map(|(_, record)| record.clone())
        .collect::<Vec<_>>();
    if actual != expected_records {
        return Err(Error::Mismatch(
            "byte-literal relocation differs from its placed address",
        ));
    }
    Ok(())
}

/// The number of relocation records the pool contributes.
pub(super) fn relocation_count(pool: Option<&ObjectByteLiteralPool>) -> usize {
    pool.map_or(0, |pool| {
        pool.addresses
            .iter()
            .map(|address| match address.fields {
                ImageAddressFields::X86_64Relative32 { .. } => 1,
                ImageAddressFields::Aarch64PageAddress { .. } => 2,
            })
            .sum()
    })
}

fn expected_records(
    pool: &ObjectByteLiteralPool,
    address: &ObjectByteLiteralAddress,
    functions: &[ObjectFunction],
) -> Result<Vec<RelocationRecord>, Error> {
    let function = crate::object_artifact::find_object_function(functions, address.machine)
        .ok_or(Error::Mismatch("byte-literal address owner is absent"))?;
    let origin = RelocationOrigin::SemanticOperation {
        function_symbol_handle: function.symbol,
        operation_identity: address.literal.operation.get(),
    };
    let addend = i64::try_from(address.pool_offset).map_err(|_| Error::Overflow)?;
    let record = |offset: u64, kind: RelocationKind| -> Result<RelocationRecord, Error> {
        Ok(RelocationRecord {
            origin,
            section: SectionKind::Text,
            offset: host(offset)?,
            byte_width: 4,
            symbol_handle: pool.symbol,
            addend,
            kind,
        })
    };
    match address.fields {
        ImageAddressFields::X86_64Relative32 { patch_offset } => Ok(vec![record(
            patch_offset,
            RelocationKind::X86_64Relative32,
        )?]),
        ImageAddressFields::Aarch64PageAddress {
            page_patch_offset,
            page_offset_patch_offset,
        } => Ok(vec![
            record(page_patch_offset, RelocationKind::Aarch64Page21)?,
            record(
                page_offset_patch_offset,
                RelocationKind::Aarch64PageOffset12,
            )?,
        ]),
    }
}

/// The padded pool bytes and one carrier row per placed address, in text
/// order. `text_offset` is where the pool begins; padding brings its end to
/// the next 16-byte boundary and a pool of only empty literals still owns
/// one aligned granule, so the region is never zero-width.
#[allow(clippy::type_complexity)]
fn pool_contents(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    text_offset: usize,
) -> Result<Option<(Vec<u8>, Vec<ObjectByteLiteralAddress>)>, Error> {
    let placed = source
        .source()
        .text_section()
        .unresolved_image_addresses
        .iter()
        .filter_map(|row| row.byte_literal().map(|literal| (row, literal)))
        .collect::<Vec<_>>();
    if placed.is_empty() {
        return Ok(None);
    }
    let literals = placed
        .iter()
        .map(|(row, _)| literal_octets(source, row))
        .collect::<Result<Vec<_>, _>>()?;
    let (pool, offsets) = pack_octets(&literals, text_offset)?;
    let addresses = placed
        .iter()
        .zip(literals.iter().zip(offsets))
        .map(
            |((row, literal), (octets, pool_offset))| ObjectByteLiteralAddress {
                machine: row.machine,
                literal: *literal,
                fields: row.fields,
                pool_offset,
                byte_count: octets.len(),
            },
        )
        .collect();
    Ok(Some((pool, addresses)))
}

/// Pack literal octets into one pool starting at `text_offset`: identical
/// octets share an entry, the end is zero-padded to the next 16-byte
/// boundary, and a pool of only empty literals still owns one granule.
/// Returns the pool bytes and each literal's offset from the pool start.
fn pack_octets(literals: &[&[u8]], text_offset: usize) -> Result<(Vec<u8>, Vec<usize>), Error> {
    let mut pool = Vec::new();
    // Construction scratch only: identical octets share one pool entry.
    let mut entries = BTreeMap::<&[u8], usize>::new();
    let mut offsets = Vec::with_capacity(literals.len());
    for octets in literals {
        let offset = *entries.entry(*octets).or_insert_with(|| {
            let offset = pool.len();
            pool.extend_from_slice(octets);
            offset
        });
        offsets.push(offset);
    }
    let end = text_offset
        .checked_add(pool.len().max(1))
        .and_then(|end| end.checked_next_multiple_of(POOL_ALIGNMENT))
        .ok_or(Error::Overflow)?;
    pool.resize(end - text_offset, 0);
    Ok((pool, offsets))
}

/// The exact octets of the one retained literal establishment the row names.
fn literal_octets<'a>(
    source: &'a StagedOptimizedRelocationFreeObjectContainer,
    row: &PlacedImageAddress,
) -> Result<&'a [u8], Error> {
    let literal = row
        .byte_literal()
        .ok_or(Error::Mismatch("non-literal address in literal pool"))?;
    if row.operation != literal.operation {
        return Err(Error::Mismatch("literal address operation changed"));
    }
    let (abstracted, _) = source::function(source, row.machine)?;
    let mut matches = abstracted
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                bytes,
                ..
            } if *psi_operation == literal.operation && place.id == literal.place => {
                Some(bytes.as_slice())
            }
            _ => None,
        });
    match (matches.next(), matches.next()) {
        (Some(bytes), None) => Ok(bytes),
        _ => Err(Error::Mismatch(
            "byte-literal address names no unique retained literal",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::pack_octets;

    #[test]
    fn identical_octets_share_one_entry_and_the_pool_ends_aligned() {
        let (pool, offsets) = pack_octets(
            &[b"Door".as_slice(), b"Gate".as_slice(), b"Door".as_slice()],
            20,
        )
        .unwrap();
        assert_eq!(offsets, [0, 4, 0]);
        assert_eq!(&pool[..8], b"DoorGate");
        assert_eq!((20 + pool.len()) % 16, 0);
        assert!(pool[8..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn empty_literals_still_own_one_aligned_granule() {
        let (pool, offsets) = pack_octets(&[b"".as_slice()], 32).unwrap();
        assert_eq!(offsets, [0]);
        assert_eq!(pool, [0; 16]);
        let (pool, _) = pack_octets(&[b"".as_slice()], 33).unwrap();
        assert_eq!(pool.len(), 15);
    }
}
