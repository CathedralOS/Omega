/*
We build and encode the ELF64 dynamic linking tables shared by dynamically
linked executables and `ET_DYN` shared components: `.interp`, `.dynstr`,
`.dynsym`, `.hash` (System V), `.gnu.hash`, `.gnu.version`, `.gnu.version_r`,
and the `.dynamic` tag table.

Imports are sorted canonically by `(symbol, object, version, identity)` and
deduplicated across string and version tables so output bytes do not depend on
arena insertion order.
*/

use crate::bytes::{write_u16, write_u32, write_u64};
use crate::constants::{ELF64_RELA_SIZE, ELF64_SYMBOL_SIZE};
use crate::dynamic_layout::{checked_product, checked_sum, checked_u32, checked_u64};
use crate::imports::{ElfImportLocator, ElfImportRequest};
use diagnostics::Diagnostic;

const STB_GLOBAL: u8 = 1;
const STT_FUNC: u8 = 2;
const SHN_UNDEF: u16 = 0;
const VER_NEED_CURRENT: u16 = 1;
const FIRST_REQUIRED_VERSION: u16 = 2;
const VERSION_INDEX_MASK: u16 = 0x7fff;

const GNU_HASH_BUCKET_COUNT: u32 = 1;
const GNU_HASH_SYMBOL_OFFSET: u32 = 1;
const GNU_HASH_BLOOM_COUNT: u32 = 1;
const GNU_HASH_BLOOM_SHIFT: u32 = 5;
const GNU_HASH_WORD_BITS: u32 = u64::BITS;

const ELF64_VERSION_NEED_SIZE: usize = 16;
const ELF64_VERSION_AUXILIARY_SIZE: usize = 16;
const DF_BIND_NOW: u64 = 0x8;

const DT_NULL: i64 = 0;
const DT_NEEDED: i64 = 1;
const DT_PLTRELSZ: i64 = 2;
const DT_PLTGOT: i64 = 3;
const DT_HASH: i64 = 4;
const DT_STRTAB: i64 = 5;
const DT_SYMTAB: i64 = 6;
const DT_RELA: i64 = 7;
const DT_RELASZ: i64 = 8;
const DT_RELAENT: i64 = 9;
const DT_STRSZ: i64 = 10;
const DT_SYMENT: i64 = 11;
const DT_PLTREL: i64 = 20;
const DT_JMPREL: i64 = 23;
const DT_FLAGS: i64 = 30;
const DT_GNU_HASH: i64 = 0x6fff_fef5;
const DT_VERSYM: i64 = 0x6fff_fff0;
const DT_VERNEED: i64 = 0x6fff_fffe;
const DT_VERNEEDNUM: i64 = 0x6fff_ffff;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfDynamicSectionContents {
    pub(crate) interpreter: Vec<u8>,
    pub(crate) dynstr: Vec<u8>,
    pub(crate) dynsym: Vec<ElfDynamicSymbol>,
    pub(crate) sysv_hash: ElfSysvHash,
    pub(crate) gnu_hash: ElfGnuHash,
    pub(crate) versym: Vec<u16>,
    pub(crate) verneed: Vec<ElfVersionNeed>,
    pub(crate) needed: Vec<u32>,
    pub(crate) bindings: Vec<ElfDynamicImportBinding>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ElfDynamicSymbol {
    pub(crate) name: u32,
    pub(crate) info: u8,
    pub(crate) other: u8,
    pub(crate) section_index: u16,
    pub(crate) value: u64,
    pub(crate) size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfSysvHash {
    pub(crate) bucket_count: u32,
    pub(crate) chain_count: u32,
    pub(crate) buckets: Vec<u32>,
    pub(crate) chains: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfGnuHash {
    pub(crate) bucket_count: u32,
    pub(crate) symbol_offset: u32,
    pub(crate) bloom_count: u32,
    pub(crate) bloom_shift: u32,
    pub(crate) bloom: Vec<u64>,
    pub(crate) buckets: Vec<u32>,
    pub(crate) chains: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfVersionNeed {
    pub(crate) version: u16,
    pub(crate) count: u16,
    pub(crate) file: u32,
    pub(crate) auxiliary_offset: u32,
    pub(crate) next_offset: u32,
    pub(crate) auxiliaries: Vec<ElfVersionNeedAuxiliary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfVersionNeedAuxiliary {
    pub(crate) hash: u32,
    pub(crate) flags: u16,
    pub(crate) other: u16,
    pub(crate) name: u32,
    pub(crate) next_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfDynamicImportBinding {
    pub(crate) request_index: usize,
    pub(crate) compatibility_report_identity: u64,
    pub(crate) dynamic_symbol_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodedImportTables {
    pub(crate) contents: ElfDynamicSectionContents,
    pub(crate) interpreter: Vec<u8>,
    pub(crate) dynstr: Vec<u8>,
    pub(crate) dynsym: Vec<u8>,
    pub(crate) sysv_hash: Vec<u8>,
    pub(crate) gnu_hash: Vec<u8>,
    pub(crate) versym: Vec<u8>,
    pub(crate) verneed: Vec<u8>,
}

struct OrderedImport<'a> {
    request_index: usize,
    compatibility_report_identity: u64,
    object: &'a [u8],
    symbol: &'a [u8],
    version: &'a [u8],
}

/// Build and serialize the ELF64 dynamic import tables for a dynamic executable.
pub(crate) fn build_dynamic_import_tables(
    imports: &[ElfImportRequest],
    interpreter_path: &[u8],
) -> Result<EncodedImportTables, Diagnostic> {
    if imports.is_empty() {
        return Err(Diagnostic::error(
            "ELF dynamic-section planning requires at least one referenced import",
        ));
    }
    let ordered = ordered_import_requests(imports);
    let mut interpreter = interpreter_path.to_vec();
    interpreter.push(0);
    let contents = derive_import_contents(&ordered, interpreter)?;
    encode_import_tables(contents)
}

/// Build address-free dynamic import contents for `ET_DYN` shared components.
pub(crate) fn derive_shared_import_contents(
    imports: &[ElfImportRequest],
) -> Result<ElfDynamicSectionContents, Diagnostic> {
    let ordered = ordered_import_requests(imports);
    derive_import_contents(&ordered, Vec::new())
}

fn derive_import_contents(
    ordered: &[OrderedImport<'_>],
    interpreter: Vec<u8>,
) -> Result<ElfDynamicSectionContents, Diagnostic> {
    let strings = canonical_strings(ordered);
    let (dynstr, string_offsets) = build_dynstr(&strings)?;
    let version_pairs = canonical_version_pairs(ordered);
    let version_indexes = assign_version_indexes(&version_pairs)?;

    let mut dynsym = vec![ElfDynamicSymbol::default()];
    let mut versym = vec![0];
    let mut bindings = Vec::with_capacity(ordered.len());
    for import in ordered {
        let dynamic_symbol_index = checked_u32(dynsym.len(), "dynamic symbol index")?;
        let version_index = version_index(&version_indexes, import.object, import.version)?;
        dynsym.push(ElfDynamicSymbol {
            name: string_offset(&string_offsets, import.symbol)?,
            info: (STB_GLOBAL << 4) | STT_FUNC,
            other: 0,
            section_index: SHN_UNDEF,
            value: 0,
            size: 0,
        });
        versym.push(version_index);
        bindings.push(ElfDynamicImportBinding {
            request_index: import.request_index,
            compatibility_report_identity: import.compatibility_report_identity,
            dynamic_symbol_index,
        });
    }

    let symbol_names = ordered
        .iter()
        .map(|import| import.symbol)
        .collect::<Vec<_>>();
    let sysv_hash = build_sysv_hash(&symbol_names)?;
    let gnu_hash = build_gnu_hash(&symbol_names)?;
    let objects = canonical_objects(ordered);
    let needed = objects
        .iter()
        .map(|object| string_offset(&string_offsets, object))
        .collect::<Result<Vec<_>, _>>()?;
    let verneed = build_version_needs(&objects, &version_pairs, &version_indexes, &string_offsets)?;

    Ok(ElfDynamicSectionContents {
        interpreter,
        dynstr,
        dynsym,
        sysv_hash,
        gnu_hash,
        versym,
        verneed,
        needed,
        bindings,
    })
}

fn encode_import_tables(
    contents: ElfDynamicSectionContents,
) -> Result<EncodedImportTables, Diagnostic> {
    let mut dynsym = Vec::with_capacity(checked_product(
        contents.dynsym.len(),
        ELF64_SYMBOL_SIZE,
        "ELF64 dynamic symbol payload size",
    )?);
    for symbol in &contents.dynsym {
        write_u32(&mut dynsym, symbol.name);
        dynsym.push(symbol.info);
        dynsym.push(symbol.other);
        write_u16(&mut dynsym, symbol.section_index);
        write_u64(&mut dynsym, symbol.value);
        write_u64(&mut dynsym, symbol.size);
    }

    let hash_word_count = 2usize
        .checked_add(contents.sysv_hash.buckets.len())
        .and_then(|count| count.checked_add(contents.sysv_hash.chains.len()))
        .ok_or_else(|| Diagnostic::error("System V hash payload word count overflow"))?;
    let mut sysv_hash = Vec::with_capacity(checked_product(
        hash_word_count,
        4,
        "System V hash payload size",
    )?);
    write_u32(&mut sysv_hash, contents.sysv_hash.bucket_count);
    write_u32(&mut sysv_hash, contents.sysv_hash.chain_count);
    for word in contents
        .sysv_hash
        .buckets
        .iter()
        .chain(&contents.sysv_hash.chains)
    {
        write_u32(&mut sysv_hash, *word);
    }

    let gnu_hash_size = checked_sum(
        16,
        checked_sum(
            checked_product(contents.gnu_hash.bloom.len(), 8, "GNU hash bloom size")?,
            checked_sum(
                checked_product(contents.gnu_hash.buckets.len(), 4, "GNU hash bucket size")?,
                checked_product(contents.gnu_hash.chains.len(), 4, "GNU hash chain size")?,
                "GNU hash bucket/chain size",
            )?,
            "GNU hash tables size",
        )?,
        "GNU hash payload size",
    )?;
    let mut gnu_hash = Vec::with_capacity(gnu_hash_size);
    write_u32(&mut gnu_hash, contents.gnu_hash.bucket_count);
    write_u32(&mut gnu_hash, contents.gnu_hash.symbol_offset);
    write_u32(&mut gnu_hash, contents.gnu_hash.bloom_count);
    write_u32(&mut gnu_hash, contents.gnu_hash.bloom_shift);
    for word in &contents.gnu_hash.bloom {
        write_u64(&mut gnu_hash, *word);
    }
    for word in contents
        .gnu_hash
        .buckets
        .iter()
        .chain(&contents.gnu_hash.chains)
    {
        write_u32(&mut gnu_hash, *word);
    }

    let mut versym = Vec::with_capacity(checked_product(
        contents.versym.len(),
        2,
        "GNU symbol-version payload size",
    )?);
    for version in &contents.versym {
        write_u16(&mut versym, *version);
    }

    let verneed_size = contents.verneed.iter().try_fold(0usize, |size, need| {
        let auxiliaries = checked_product(
            need.auxiliaries.len(),
            ELF64_VERSION_AUXILIARY_SIZE,
            "GNU version auxiliary payload size",
        )?;
        size.checked_add(ELF64_VERSION_NEED_SIZE)
            .and_then(|size| size.checked_add(auxiliaries))
            .ok_or_else(|| Diagnostic::error("GNU version requirement payload size overflow"))
    })?;
    let mut verneed = Vec::with_capacity(verneed_size);
    for need in &contents.verneed {
        write_u16(&mut verneed, need.version);
        write_u16(&mut verneed, need.count);
        write_u32(&mut verneed, need.file);
        write_u32(&mut verneed, need.auxiliary_offset);
        write_u32(&mut verneed, need.next_offset);
        for auxiliary in &need.auxiliaries {
            write_u32(&mut verneed, auxiliary.hash);
            write_u16(&mut verneed, auxiliary.flags);
            write_u16(&mut verneed, auxiliary.other);
            write_u32(&mut verneed, auxiliary.name);
            write_u32(&mut verneed, auxiliary.next_offset);
        }
    }

    let interpreter = contents.interpreter.clone();
    let dynstr = contents.dynstr.clone();
    Ok(EncodedImportTables {
        contents,
        interpreter,
        dynstr,
        dynsym,
        sysv_hash,
        gnu_hash,
        versym,
        verneed,
    })
}

/// Compute the byte size of the `.dynamic` section before addresses are known.
pub(crate) fn dynamic_table_byte_size(
    contents: &ElfDynamicSectionContents,
) -> Result<usize, Diagnostic> {
    let fixed_rows = 15 + 3 * usize::from(!contents.verneed.is_empty());
    let row_count = checked_sum(contents.needed.len(), fixed_rows, "ELF dynamic row count")?;
    checked_product(row_count, 16, "ELF64 dynamic payload size")
}

/// Virtual addresses of the sections referenced by `.dynamic` entries.
pub(crate) struct DynamicTableAddresses {
    pub(crate) got_plt: u64,
    pub(crate) sysv_hash: u64,
    pub(crate) gnu_hash: u64,
    pub(crate) dynstr: u64,
    pub(crate) dynsym: u64,
    pub(crate) rela_plt: u64,
    pub(crate) rela_dyn: u64,
    pub(crate) versym: u64,
    pub(crate) verneed: u64,
}

/// Encode the `.dynamic` (`Elf64_Dyn`) table with resolved virtual addresses.
pub(crate) fn encode_dynamic_table(
    tables: &EncodedImportTables,
    rela_plt_size: usize,
    rela_dyn_size: usize,
    addresses: &DynamicTableAddresses,
) -> Result<Vec<u8>, Diagnostic> {
    let contents = &tables.contents;
    let total_bytes = dynamic_table_byte_size(contents)?;
    let mut bytes = Vec::with_capacity(total_bytes);

    let push_row = |bytes: &mut Vec<u8>, tag: i64, value: u64| {
        write_u64(bytes, tag as u64);
        write_u64(bytes, value);
    };

    for needed in &contents.needed {
        push_row(&mut bytes, DT_NEEDED, u64::from(*needed));
    }
    push_row(
        &mut bytes,
        DT_PLTRELSZ,
        checked_u64(rela_plt_size, "procedure relocation byte count")?,
    );
    push_row(&mut bytes, DT_PLTGOT, addresses.got_plt);
    push_row(&mut bytes, DT_HASH, addresses.sysv_hash);
    push_row(&mut bytes, DT_GNU_HASH, addresses.gnu_hash);
    push_row(&mut bytes, DT_STRTAB, addresses.dynstr);
    push_row(&mut bytes, DT_SYMTAB, addresses.dynsym);
    push_row(
        &mut bytes,
        DT_STRSZ,
        checked_u64(tables.dynstr.len(), "dynamic string byte count")?,
    );
    push_row(&mut bytes, DT_SYMENT, ELF64_SYMBOL_SIZE as u64);
    push_row(&mut bytes, DT_PLTREL, DT_RELA as u64);
    push_row(&mut bytes, DT_JMPREL, addresses.rela_plt);
    push_row(&mut bytes, DT_RELA, addresses.rela_dyn);
    push_row(
        &mut bytes,
        DT_RELASZ,
        checked_u64(rela_dyn_size, "general relocation byte count")?,
    );
    push_row(&mut bytes, DT_RELAENT, ELF64_RELA_SIZE as u64);

    if !contents.verneed.is_empty() {
        push_row(&mut bytes, DT_VERSYM, addresses.versym);
        push_row(&mut bytes, DT_VERNEED, addresses.verneed);
        push_row(
            &mut bytes,
            DT_VERNEEDNUM,
            checked_u64(
                contents.verneed.len(),
                "GNU version-requirement record count",
            )?,
        );
    }

    push_row(&mut bytes, DT_FLAGS, DF_BIND_NOW);
    push_row(&mut bytes, DT_NULL, 0);

    debug_assert_eq!(bytes.len(), total_bytes);
    Ok(bytes)
}

fn ordered_import_requests(imports: &[ElfImportRequest]) -> Vec<OrderedImport<'_>> {
    let mut ordered = imports
        .iter()
        .enumerate()
        .map(|(request_index, request)| match &request.locator {
            ElfImportLocator::Versioned {
                compatibility_report_identity,
                object,
                symbol,
                version,
                ..
            } => OrderedImport {
                request_index,
                compatibility_report_identity: *compatibility_report_identity,
                object,
                symbol,
                version,
            },
            ElfImportLocator::Component {
                locator,
                object,
                symbol,
            } => OrderedImport {
                request_index,
                compatibility_report_identity: locator
                    .non_authoritative_compatibility_fingerprint(),
                object,
                symbol,
                version: &[],
            },
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        (
            left.symbol,
            left.object,
            left.version,
            left.compatibility_report_identity,
        )
            .cmp(&(
                right.symbol,
                right.object,
                right.version,
                right.compatibility_report_identity,
            ))
    });
    ordered
}

fn canonical_strings(ordered: &[OrderedImport<'_>]) -> Vec<Vec<u8>> {
    let mut strings = ordered
        .iter()
        .flat_map(|import| [import.object, import.symbol, import.version])
        .filter(|string| !string.is_empty())
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    strings.sort();
    strings.dedup();
    strings
}

fn canonical_objects(ordered: &[OrderedImport<'_>]) -> Vec<Vec<u8>> {
    let mut objects = ordered
        .iter()
        .map(|import| import.object.to_vec())
        .collect::<Vec<_>>();
    objects.sort();
    objects.dedup();
    objects
}

fn canonical_version_pairs(ordered: &[OrderedImport<'_>]) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut pairs = ordered
        .iter()
        .filter(|import| !import.version.is_empty())
        .map(|import| (import.object.to_vec(), import.version.to_vec()))
        .collect::<Vec<_>>();
    pairs.sort();
    pairs.dedup();
    pairs
}

fn build_dynstr(strings: &[Vec<u8>]) -> Result<(Vec<u8>, Vec<(Vec<u8>, u32)>), Diagnostic> {
    let mut dynstr = vec![0];
    let mut offsets = Vec::with_capacity(strings.len());
    for string in strings {
        let offset = checked_u32(dynstr.len(), "dynamic string offset")?;
        offsets.push((string.clone(), offset));
        dynstr.extend(string);
        dynstr.push(0);
    }
    Ok((dynstr, offsets))
}

fn string_offset(offsets: &[(Vec<u8>, u32)], string: &[u8]) -> Result<u32, Diagnostic> {
    offsets
        .binary_search_by(|(candidate, _)| candidate.as_slice().cmp(string))
        .map(|index| offsets[index].1)
        .map_err(|_| Diagnostic::error("ELF dynamic string is absent from the canonical table"))
}

fn assign_version_indexes(
    pairs: &[(Vec<u8>, Vec<u8>)],
) -> Result<Vec<((Vec<u8>, Vec<u8>), u16)>, Diagnostic> {
    pairs
        .iter()
        .enumerate()
        .map(|(index, pair)| {
            let index = u16::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(FIRST_REQUIRED_VERSION))
                .filter(|index| *index <= VERSION_INDEX_MASK)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "ELF required-version count exceeds the unhidden version-index domain",
                    )
                })?;
            Ok((pair.clone(), index))
        })
        .collect()
}

fn version_index(
    indexes: &[((Vec<u8>, Vec<u8>), u16)],
    object: &[u8],
    version: &[u8],
) -> Result<u16, Diagnostic> {
    if version.is_empty() {
        return Ok(1);
    }
    indexes
        .binary_search_by(|((candidate_object, candidate_version), _)| {
            (candidate_object.as_slice(), candidate_version.as_slice()).cmp(&(object, version))
        })
        .map(|index| indexes[index].1)
        .map_err(|_| Diagnostic::error("ELF import lacks a canonical required-version index"))
}

/// Build the System V ELF `.hash` table over ordered symbol names.
pub(crate) fn build_sysv_hash(symbol_names: &[&[u8]]) -> Result<ElfSysvHash, Diagnostic> {
    let bucket_count = checked_u32(symbol_names.len().max(1), "System V hash bucket count")?;
    let chain_count = checked_u32(symbol_names.len() + 1, "System V hash chain count")?;
    let mut buckets = vec![0; bucket_count as usize];
    let mut chains = vec![0; chain_count as usize];
    for (zero_based_index, name) in symbol_names.iter().enumerate() {
        let symbol_index = checked_u32(zero_based_index + 1, "System V hash symbol index")?;
        let bucket_index = (elf_hash(name) % bucket_count) as usize;
        if buckets[bucket_index] == 0 {
            buckets[bucket_index] = symbol_index;
            continue;
        }
        let mut chain_index = buckets[bucket_index] as usize;
        while chains[chain_index] != 0 {
            chain_index = chains[chain_index] as usize;
        }
        chains[chain_index] = symbol_index;
    }
    Ok(ElfSysvHash {
        bucket_count,
        chain_count,
        buckets,
        chains,
    })
}

fn build_gnu_hash(symbol_names: &[&[u8]]) -> Result<ElfGnuHash, Diagnostic> {
    let mut bloom = vec![0u64; GNU_HASH_BLOOM_COUNT as usize];
    let mut chains = Vec::with_capacity(symbol_names.len());
    for name in symbol_names {
        let hash = gnu_hash(name);
        let first_bit = hash % GNU_HASH_WORD_BITS;
        let second_bit = (hash >> GNU_HASH_BLOOM_SHIFT) % GNU_HASH_WORD_BITS;
        bloom[0] |= (1u64 << first_bit) | (1u64 << second_bit);
        chains.push(hash & !1);
    }
    if let Some(last) = chains.last_mut() {
        *last |= 1;
    }
    Ok(ElfGnuHash {
        bucket_count: GNU_HASH_BUCKET_COUNT,
        symbol_offset: GNU_HASH_SYMBOL_OFFSET,
        bloom_count: GNU_HASH_BLOOM_COUNT,
        bloom_shift: GNU_HASH_BLOOM_SHIFT,
        bloom,
        buckets: vec![if symbol_names.is_empty() {
            0
        } else {
            GNU_HASH_SYMBOL_OFFSET
        }],
        chains,
    })
}

fn build_version_needs(
    objects: &[Vec<u8>],
    pairs: &[(Vec<u8>, Vec<u8>)],
    indexes: &[((Vec<u8>, Vec<u8>), u16)],
    strings: &[(Vec<u8>, u32)],
) -> Result<Vec<ElfVersionNeed>, Diagnostic> {
    let mut needs = Vec::with_capacity(objects.len());
    let objects = objects
        .iter()
        .filter(|object| pairs.iter().any(|(candidate, _)| candidate == *object))
        .collect::<Vec<_>>();
    for (object_index, object) in objects.iter().copied().enumerate() {
        let versions = pairs
            .iter()
            .filter(|(candidate, _)| candidate == object)
            .map(|(_, version)| version)
            .collect::<Vec<_>>();
        let count = u16::try_from(versions.len()).map_err(|_| {
            Diagnostic::error("ELF object version requirement count exceeds Elf64_Half")
        })?;
        let mut auxiliaries = Vec::with_capacity(versions.len());
        for (version_index_in_object, version) in versions.iter().enumerate() {
            auxiliaries.push(ElfVersionNeedAuxiliary {
                hash: elf_hash(version),
                flags: 0,
                other: version_index(indexes, object, version)?,
                name: string_offset(strings, version)?,
                next_offset: if version_index_in_object + 1 == versions.len() {
                    0
                } else {
                    16
                },
            });
        }
        let group_size = 16usize
            .checked_add(16usize.checked_mul(auxiliaries.len()).ok_or_else(|| {
                Diagnostic::error("ELF version-need auxiliary byte count overflow")
            })?)
            .ok_or_else(|| Diagnostic::error("ELF version-need group byte count overflow"))?;
        needs.push(ElfVersionNeed {
            version: VER_NEED_CURRENT,
            count,
            file: string_offset(strings, object)?,
            auxiliary_offset: 16,
            next_offset: if object_index + 1 == objects.len() {
                0
            } else {
                checked_u32(group_size, "ELF next version-need offset")?
            },
            auxiliaries,
        });
    }
    Ok(needs)
}

fn elf_hash(bytes: &[u8]) -> u32 {
    let mut hash = 0u32;
    for byte in bytes {
        hash = (hash << 4).wrapping_add(u32::from(*byte));
        let high = hash & 0xf000_0000;
        if high != 0 {
            hash ^= high >> 24;
        }
        hash &= !high;
    }
    hash
}

fn gnu_hash(bytes: &[u8]) -> u32 {
    bytes.iter().fold(5381u32, |hash, byte| {
        hash.wrapping_mul(33).wrapping_add(u32::from(*byte))
    })
}

#[cfg(test)]
mod tests {
    use super::{build_dynamic_import_tables, derive_shared_import_contents, gnu_hash};
    use crate::imports::canonical_referenced_imports;
    use crate::test_support::{IMPORTS, dynamic_import_inputs, import_with_text_relocations};
    use target::TargetProfile;

    #[test]
    fn gnu_hash_matches_known_vectors() {
        assert_eq!(gnu_hash(b""), 0x0000_1505);
        assert_eq!(gnu_hash(b"printf"), 0x156b_2bb8);
        assert_eq!(gnu_hash(b"exit"), 0x7c96_7e3f);
    }

    #[test]
    fn pure_component_import_tables_have_no_version_needs_and_use_unversioned_index() {
        let profile = TargetProfile::LinuxX64;
        let mut image = crate::test_support::import_image(profile, 8, 2);
        let locator =
            target::NormalizedComponentLocator::for_image(profile, b"component bytes", [7; 32])
                .expect("exact component locator");
        image.memory.text[0] = 0xe8;
        import_with_text_relocations(
            &mut image,
            "__omega_component",
            image::FinalImageImportPlan::Normalized(locator.into()),
            &[1],
        );
        let imports = canonical_referenced_imports(&image).unwrap();
        let tables = build_dynamic_import_tables(&imports, b"/lib64/ld-linux-x86-64.so.2").unwrap();
        assert!(tables.contents.verneed.is_empty());
        assert_eq!(tables.contents.versym, [0, 1]);
        assert_eq!(tables.contents.needed.len(), 1);
    }

    #[test]
    fn dynamic_tables_deduplicate_and_ignore_import_insertion_order() {
        let profile = TargetProfile::LinuxX64;
        let (forward_img, forward_interp, forward_imports) =
            dynamic_import_inputs(profile, &IMPORTS, "__omega_fwd_", "valid", "valid");
        let reversed = [IMPORTS[2], IMPORTS[1], IMPORTS[0]];
        let (reverse_img, reverse_interp, reverse_imports) =
            dynamic_import_inputs(profile, &reversed, "__omega_rev_", "valid", "valid");
        let t1 = build_dynamic_import_tables(&forward_imports, forward_interp.interpreter_path())
            .unwrap();
        let t2 = build_dynamic_import_tables(&reverse_imports, reverse_interp.interpreter_path())
            .unwrap();
        assert_eq!(t1.dynstr, t2.dynstr);
        assert_eq!(t1.dynsym, t2.dynsym);
        assert_eq!(t1.sysv_hash, t2.sysv_hash);
        assert_eq!(t1.gnu_hash, t2.gnu_hash);
        assert_eq!(t1.versym, t2.versym);
        assert_eq!(t1.verneed, t2.verneed);
        assert_eq!(t1.contents.needed, t2.contents.needed);
        assert_eq!(t1.contents.verneed.len(), 2);
        assert_eq!(t1.contents.needed.len(), 2);
        let _ = (forward_img, reverse_img);
    }

    #[test]
    fn shared_import_contents_handles_empty_imports() {
        let contents = derive_shared_import_contents(&[]).unwrap();
        assert_eq!(contents.dynstr, [0]);
        assert_eq!(contents.dynsym.len(), 1);
        assert_eq!(contents.versym, [0]);
        assert!(contents.verneed.is_empty());
        assert!(contents.needed.is_empty());
        assert_eq!(contents.sysv_hash.bucket_count, 1);
        assert_eq!(contents.sysv_hash.chain_count, 1);
    }
}
