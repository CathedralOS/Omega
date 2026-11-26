use super::{Error, ObjectFunctionLocation};
use crate::{
    ObjectByteLiteralAddress, ObjectByteLiteralPool, ObjectPlan, ObjectPublication,
    RelocationOrigin, RelocationRecord, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
};
use machine_code::PlacedImageAddress;
use std::{borrow::Cow, collections::BTreeMap};
pub const POOL_SYMBOL_NAME: &str = "omega_byte_literal_pool";
const POOL_ALIGNMENT: usize = 16;

pub(super) fn place(
    source: &ObjectPublication,
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

pub fn records(
    pool: &ObjectByteLiteralPool,
    functions: &[impl AsRef<ObjectFunctionLocation>],
) -> Result<Vec<RelocationRecord>, Error> {
    let mut rows = Vec::new();
    for address in &pool.addresses {
        rows.extend(expected_records(pool, address, functions)?);
    }
    Ok(rows)
}

fn expected_records(
    pool: &ObjectByteLiteralPool,
    address: &ObjectByteLiteralAddress,
    functions: &[impl AsRef<ObjectFunctionLocation>],
) -> Result<Vec<RelocationRecord>, Error> {
    let function = super::find_object_function(functions, address.machine)
        .ok_or(Error::Mismatch("byte-literal address owner is absent"))?;
    let origin = RelocationOrigin::SemanticOperation {
        function_symbol_handle: function.symbol,
        operation_identity: address.literal.operation.get(),
    };
    let addend = i64::try_from(address.pool_offset).map_err(|_| Error::Overflow)?;
    super::image_addresses::records(address.fields, origin, pool.symbol, addend)
}

#[allow(clippy::type_complexity)]
pub fn pool_contents(
    source: &ObjectPublication,
    text_offset: usize,
) -> Result<Option<(Vec<u8>, Vec<ObjectByteLiteralAddress>)>, Error> {
    let placed = source
        .text()
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
    let entries = literals
        .iter()
        .map(|(octets, alignment)| (octets.as_ref(), *alignment))
        .collect::<Vec<_>>();
    let (pool, offsets) = pack_octets(&entries, text_offset)?;
    let addresses = placed
        .iter()
        .zip(literals.iter().zip(offsets))
        .map(
            |((row, literal), ((octets, _), pool_offset))| ObjectByteLiteralAddress {
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

/// Packs each literal at an image position aligned to its element alignment (at most the
/// pool's own alignment, which the text section also honors). Identical octets share an
/// entry when that entry is already aligned enough.
fn pack_octets(
    literals: &[(&[u8], usize)],
    text_offset: usize,
) -> Result<(Vec<u8>, Vec<usize>), Error> {
    let mut pool = Vec::new();
    let mut entries = BTreeMap::<&[u8], usize>::new();
    let mut offsets = Vec::with_capacity(literals.len());
    for (octets, alignment) in literals {
        if !alignment.is_power_of_two() || *alignment > POOL_ALIGNMENT {
            return Err(Error::Mismatch("literal alignment exceeds the pool's"));
        }
        let aligned = |offset: usize| (text_offset + offset).is_multiple_of(*alignment);
        let offset = match entries.get(octets) {
            Some(offset) if aligned(*offset) => *offset,
            _ => {
                let offset = (text_offset + pool.len())
                    .checked_next_multiple_of(*alignment)
                    .ok_or(Error::Overflow)?
                    - text_offset;
                pool.resize(offset, 0);
                pool.extend_from_slice(octets);
                entries.insert(octets, offset);
                offset
            }
        };
        offsets.push(offset);
    }
    let end = text_offset
        .checked_add(pool.len().max(1))
        .and_then(|end| end.checked_next_multiple_of(POOL_ALIGNMENT))
        .ok_or(Error::Overflow)?;
    pool.resize(end - text_offset, 0);
    Ok((pool, offsets))
}

/// The exact quoted bytes named by a retained literal address.
fn literal_octets<'a>(
    source: &'a ObjectPublication,
    row: &PlacedImageAddress,
) -> Result<(Cow<'a, [u8]>, usize), Error> {
    let literal = row
        .byte_literal()
        .ok_or(Error::Mismatch("non-literal address in literal pool"))?;
    if row.operation != literal.operation {
        return Err(Error::Mismatch("literal address operation changed"));
    }
    let current = source
        .emission_facts()
        .physical
        .contracts
        .publication()
        .function(row.machine)
        .ok_or(Error::Mismatch("current function has no publication facts"))?;
    Ok((
        Cow::Borrowed(quoted_octets(current, literal.operation, literal.place)?),
        1,
    ))
}

fn quoted_octets(
    current: &target_vocabulary::NativeFunctionPublication,
    operation: semantic_vocabulary::OperationId,
    place: semantic_vocabulary::PlaceId,
) -> Result<&[u8], Error> {
    let quoted = current
        .byte_literal(operation)
        .filter(|quoted| quoted.place == place)
        .ok_or(Error::Mismatch(
            "byte-literal address names no unique retained literal",
        ))?;
    Ok(quoted.bytes.as_ref())
}

#[cfg(test)]
mod tests {
    use super::{pack_octets, quoted_octets};

    #[test]
    fn current_literal_lookup_keeps_exact_bytes_and_refuses_another_place_or_operation() {
        use machine_code::SemanticCodeSite;
        use semantic_vocabulary::{MachineId, OperationId, PlaceId};
        use target_vocabulary::{
            NativeByteLiteral, NativeFunctionPublication, NativePublicationFacts,
        };
        let operation = OperationId::new(1).unwrap();
        let place = PlaceId::new(1).unwrap();
        let facts = NativePublicationFacts::seal(
            vec![NativeFunctionPublication {
                call_targets: Vec::new(),
                machine: MachineId::new(1).unwrap(),
                operation_count: 1,
                sites: vec![(SemanticCodeSite::Operation(operation), 0)],
                byte_literals: vec![NativeByteLiteral {
                    operation,
                    place,
                    bytes: std::sync::Arc::from(b"exact\0octets".as_slice()),
                }],
            }],
            Default::default(),
        )
        .unwrap();
        let function = &facts.functions()[0];
        assert_eq!(
            quoted_octets(function, operation, place).unwrap(),
            b"exact\0octets"
        );
        assert!(quoted_octets(function, OperationId::new(2).unwrap(), place).is_err());
        assert!(quoted_octets(function, operation, PlaceId::new(2).unwrap()).is_err());
        let mut empty = function.clone();
        empty.byte_literals[0].bytes = std::sync::Arc::from([]);
        let empty = NativePublicationFacts::seal(vec![empty], Default::default()).unwrap();
        assert_eq!(
            quoted_octets(&empty.functions()[0], operation, place).unwrap(),
            b""
        );
    }

    #[test]
    fn identical_octets_share_one_entry_and_the_pool_ends_aligned() {
        let (pool, offsets) = pack_octets(
            &[
                (b"Door".as_slice(), 1),
                (b"Gate".as_slice(), 1),
                (b"Door".as_slice(), 1),
            ],
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
        let (pool, offsets) = pack_octets(&[(b"".as_slice(), 1)], 32).unwrap();
        assert_eq!(offsets, [0]);
        assert_eq!(pool, [0; 16]);
        let (pool, _) = pack_octets(&[(b"".as_slice(), 1)], 33).unwrap();
        assert_eq!(pool.len(), 15);
    }

    #[test]
    fn an_element_roster_starts_at_its_alignment_in_the_image() {
        let roster = 11_u32.to_le_bytes();
        let (pool, offsets) =
            pack_octets(&[(b"Gate".as_slice(), 1), (roster.as_slice(), 4)], 21).unwrap();
        assert_eq!(offsets, [0, 7]);
        assert_eq!((21 + offsets[1]) % 4, 0);
        assert_eq!(&pool[7..11], roster);
        assert!(pack_octets(&[(roster.as_slice(), 32)], 0).is_err());
    }
}
