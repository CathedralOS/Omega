//! Address-free contents for the first complete ELF dynamic-section plan.
//!
//! The plan selects the System V symbol hash defined by the generic ABI, an
//! address-free GNU symbol hash, and the GNU version-requirement format
//! specified by the LSB. It deliberately
//! stops before section placement, program headers, dynamic addresses,
//! relocation lowering, image mutation, or runnable-image authority.
//!
//! Primary format contracts: [System V string tables], [System V symbol
//! tables], [System V dynamic hash tables], the original GNU
//! [`DT_GNU_HASH` implementation], and [LSB symbol versioning].
//!
//! [System V string tables]: https://gabi.xinuos.com/elf/04-strtab.html
//! [System V symbol tables]: https://gabi.xinuos.com/elf/05-symtab.html
//! [System V dynamic hash tables]: https://gabi.xinuos.com/elf/08-dynamic.html#hash-table
//! [`DT_GNU_HASH` implementation]: https://sourceware.org/pipermail/binutils/2006-July/048074.html
//! [LSB symbol versioning]: https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/symversion.html

use crate::dynamic_executable::checked::checked_u32;
use crate::dynamic_executable::import_sections::dynamic_link::PlannedElfDynamicLinkInputs;
use crate::imports::ElfImportLocator;
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
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently validated, address-free dynamic ELF section contents.
///
/// The exact preflight inputs remain owned by this non-clone plan. Public
/// observations expose only deterministic counts and content identity; the
/// private table rows remain in the ELF image owner until later layout and
/// relocation milestones consume them.
#[derive(Debug)]
#[must_use = "validated ELF dynamic sections retain the exact preflight inputs"]
pub struct ValidatedElfDynamicSectionPlan {
    inputs: PlannedElfDynamicLinkInputs,
    contents: ElfDynamicSectionContents,
}

impl ValidatedElfDynamicSectionPlan {
    pub const fn inputs(&self) -> &PlannedElfDynamicLinkInputs {
        &self.inputs
    }

    pub fn interpreter_byte_count(&self) -> usize {
        self.contents.interpreter.len()
    }

    pub fn dynamic_string_byte_count(&self) -> usize {
        self.contents.dynstr.len()
    }

    pub fn dynamic_symbol_count(&self) -> usize {
        self.contents.dynsym.len()
    }

    pub fn needed_object_count(&self) -> usize {
        self.contents.needed.len()
    }

    pub fn required_version_count(&self) -> usize {
        self.contents
            .verneed
            .iter()
            .map(|need| need.auxiliaries.len())
            .sum()
    }

    /// Compatibility fingerprint of the selected profile, exact interpreter
    /// contents, canonical table contents, normalized import identities, and
    /// their assigned dynamic-symbol/version indexes. This is deterministic
    /// artifact content identity, not admission or loader authority.
    pub const fn non_authoritative_content_compatibility_fingerprint(&self) -> u64 {
        self.contents
            .non_authoritative_content_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfDynamicSectionContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(self) -> (PlannedElfDynamicLinkInputs, ElfDynamicSectionContents) {
        (self.inputs, self.contents)
    }
}

/// Rejected address-free dynamic-section planning with exact input custody.
#[derive(Debug)]
#[must_use = "ELF dynamic-section rejection retains the exact preflight inputs"]
pub struct ElfDynamicSectionPlanningError {
    inputs: PlannedElfDynamicLinkInputs,
    diagnostic: Diagnostic,
}

impl ElfDynamicSectionPlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PlannedElfDynamicLinkInputs, Diagnostic) {
        (self.inputs, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicSectionPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicSectionPlanningError {}

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
    pub(crate) non_authoritative_content_compatibility_fingerprint: u64,
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
    pub(crate) version_index: u16,
}

struct OrderedImport<'a> {
    request_index: usize,
    compatibility_report_identity: u64,
    object: &'a [u8],
    symbol: &'a [u8],
    version: &'a [u8],
}

struct Candidate {
    inputs: PlannedElfDynamicLinkInputs,
    contents: ElfDynamicSectionContents,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume one exact dynamic-link preflight into a concrete, address-free ELF
/// table plan and independently replay every table relationship before sealing
/// it.
///
/// This chooses System V `.hash`, address-free GNU `.gnu.hash`, and GNU
/// `.gnu.version` / `.gnu.version_r` requirements. It does not encode
/// `.dynamic`, integrate GNU hash into the semantic section roster, lower
/// relocations, assign addresses, mutate image bytes, or
/// grant publication, admission, or runnable-image authority.
pub fn plan_elf_dynamic_sections(
    inputs: PlannedElfDynamicLinkInputs,
) -> Result<ValidatedElfDynamicSectionPlan, Box<ElfDynamicSectionPlanningError>> {
    let contents = match derive_contents(&inputs) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicSectionPlanningError {
                inputs,
                diagnostic,
            }));
        }
    };
    let candidate = Candidate { inputs, contents };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfDynamicSectionPlanningError {
            inputs: error.candidate.inputs,
            diagnostic: error.diagnostic,
        })),
    }
}

fn derive_contents(
    inputs: &PlannedElfDynamicLinkInputs,
) -> Result<ElfDynamicSectionContents, Diagnostic> {
    let ordered = ordered_imports(inputs)?;
    if ordered.is_empty() {
        return Err(Diagnostic::error(
            "ELF dynamic-section planning requires at least one referenced import",
        ));
    }

    let mut interpreter = inputs.interpreter().interpreter_path().to_vec();
    interpreter.push(0);

    let strings = canonical_strings(&ordered);
    let (dynstr, string_offsets) = build_dynstr(&strings)?;
    let version_pairs = canonical_version_pairs(&ordered);
    let version_indexes = assign_version_indexes(&version_pairs)?;

    let mut dynsym = vec![ElfDynamicSymbol::default()];
    let mut versym = vec![0];
    let mut bindings = Vec::with_capacity(ordered.len());
    for import in &ordered {
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
            version_index,
        });
    }

    let symbol_names = ordered
        .iter()
        .map(|import| import.symbol)
        .collect::<Vec<_>>();
    let sysv_hash = build_sysv_hash(&symbol_names)?;
    let gnu_hash = build_gnu_hash(&symbol_names)?;
    let objects = canonical_objects(&ordered);
    let needed = objects
        .iter()
        .map(|object| string_offset(&string_offsets, object))
        .collect::<Result<Vec<_>, _>>()?;
    let verneed = build_version_needs(&objects, &version_pairs, &version_indexes, &string_offsets)?;

    let mut contents = ElfDynamicSectionContents {
        interpreter,
        dynstr,
        dynsym,
        sysv_hash,
        gnu_hash,
        versym,
        verneed,
        needed,
        bindings,
        non_authoritative_content_compatibility_fingerprint: 0,
    };
    contents.non_authoritative_content_compatibility_fingerprint =
        non_authoritative_content_compatibility_fingerprint(inputs, &contents);
    Ok(contents)
}

fn ordered_imports(
    inputs: &PlannedElfDynamicLinkInputs,
) -> Result<Vec<OrderedImport<'_>>, Diagnostic> {
    let mut ordered = inputs
        .imports()
        .iter()
        .enumerate()
        .map(|(request_index, request)| match &request.locator {
            ElfImportLocator::Versioned {
                compatibility_report_identity,
                object,
                symbol,
                version,
                ..
            } => Ok(OrderedImport {
                request_index,
                compatibility_report_identity: *compatibility_report_identity,
                object,
                symbol,
                version,
            }),
            ElfImportLocator::StringBackedBootstrap { .. } => Err(Diagnostic::error(
                "string-backed ELF import reached normalized dynamic-section planning",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    Ok(ordered)
}

fn canonical_strings(ordered: &[OrderedImport<'_>]) -> Vec<Vec<u8>> {
    let mut strings = ordered
        .iter()
        .flat_map(|import| [import.object, import.symbol, import.version])
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
    indexes
        .binary_search_by(|((candidate_object, candidate_version), _)| {
            (candidate_object.as_slice(), candidate_version.as_slice()).cmp(&(object, version))
        })
        .map(|index| indexes[index].1)
        .map_err(|_| Diagnostic::error("ELF import lacks a canonical required-version index"))
}

fn build_sysv_hash(symbol_names: &[&[u8]]) -> Result<ElfSysvHash, Diagnostic> {
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
    require_equal(
        !symbol_names.is_empty(),
        "GNU hash planning requires at least one dynamic symbol after the reserved row",
    )?;
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
        buckets: vec![GNU_HASH_SYMBOL_OFFSET],
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
    for (object_index, object) in objects.iter().enumerate() {
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

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicSectionPlan, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.inputs, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    Ok(ValidatedElfDynamicSectionPlan {
        inputs: candidate.inputs,
        contents: candidate.contents,
    })
}

fn validate_contents(
    inputs: &PlannedElfDynamicLinkInputs,
    contents: &ElfDynamicSectionContents,
) -> Result<(), Diagnostic> {
    let ordered = ordered_imports(inputs)?;
    let mut expected_interpreter = inputs.interpreter().interpreter_path().to_vec();
    expected_interpreter.push(0);
    require_equal(
        contents.interpreter == expected_interpreter,
        "PT_INTERP contents do not exactly equal the selected path plus one NUL terminator",
    )?;

    let strings = canonical_strings(&ordered);
    let (expected_dynstr, offsets) = build_dynstr(&strings)?;
    require_equal(
        contents.dynstr == expected_dynstr
            && contents.dynstr.first() == Some(&0)
            && contents.dynstr.last() == Some(&0),
        "ELF dynamic string table is not canonical or NUL framed",
    )?;

    require_equal(
        contents.dynsym.len() == ordered.len() + 1
            && contents.dynsym.first() == Some(&ElfDynamicSymbol::default()),
        "ELF dynamic symbol table lacks its exact reserved undefined row",
    )?;
    let pairs = canonical_version_pairs(&ordered);
    let version_indexes = assign_version_indexes(&pairs)?;
    require_equal(
        contents.versym.len() == contents.dynsym.len()
            && contents.versym.first() == Some(&0)
            && contents.bindings.len() == ordered.len(),
        "ELF symbol-version or import-index table does not parallel the dynamic symbol table",
    )?;
    for (index, import) in ordered.iter().enumerate() {
        let dynamic_symbol_index = checked_u32(index + 1, "validated dynamic symbol index")?;
        let expected_version = version_index(&version_indexes, import.object, import.version)?;
        let expected_symbol = ElfDynamicSymbol {
            name: string_offset(&offsets, import.symbol)?,
            info: (STB_GLOBAL << 4) | STT_FUNC,
            other: 0,
            section_index: SHN_UNDEF,
            value: 0,
            size: 0,
        };
        let expected_binding = ElfDynamicImportBinding {
            request_index: import.request_index,
            compatibility_report_identity: import.compatibility_report_identity,
            dynamic_symbol_index,
            version_index: expected_version,
        };
        require_equal(
            contents.dynsym.get(index + 1) == Some(&expected_symbol)
                && contents.versym.get(index + 1) == Some(&expected_version)
                && contents.bindings.get(index) == Some(&expected_binding),
            "ELF dynamic symbol, version, and private import indexes drifted",
        )?;
    }

    let names = ordered
        .iter()
        .map(|import| import.symbol)
        .collect::<Vec<_>>();
    require_equal(
        contents.sysv_hash == build_sysv_hash(&names)?,
        "System V symbol hash buckets or chains are not canonical",
    )?;
    validate_hash_reachability(&contents.sysv_hash, &names)?;
    require_equal(
        contents.gnu_hash == build_gnu_hash(&names)?,
        "GNU symbol hash header, bloom, buckets, or chains are not canonical",
    )?;
    validate_gnu_hash_reachability(&contents.gnu_hash, &names)?;

    let objects = canonical_objects(&ordered);
    let expected_needed = objects
        .iter()
        .map(|object| string_offset(&offsets, object))
        .collect::<Result<Vec<_>, _>>()?;
    require_equal(
        contents.needed == expected_needed,
        "ELF DT_NEEDED string-index roster is not canonical",
    )?;
    require_equal(
        contents.verneed == build_version_needs(&objects, &pairs, &version_indexes, &offsets)?,
        "ELF GNU version requirements do not match the exact object/version rows",
    )?;
    require_equal(
        contents.non_authoritative_content_compatibility_fingerprint
            == non_authoritative_content_compatibility_fingerprint(inputs, contents),
        "ELF dynamic-section content compatibility fingerprint does not replay",
    )
}

fn validate_hash_reachability(hash: &ElfSysvHash, names: &[&[u8]]) -> Result<(), Diagnostic> {
    require_equal(
        hash.bucket_count > 0
            && hash.bucket_count as usize == hash.buckets.len()
            && hash.chain_count as usize == hash.chains.len()
            && hash.chain_count as usize == names.len() + 1,
        "System V hash counts do not match their tables or dynamic symbols",
    )?;
    for (name_index, name) in names.iter().enumerate() {
        let wanted = checked_u32(name_index + 1, "validated hash symbol index")?;
        let mut current = hash.buckets[(elf_hash(name) % hash.bucket_count) as usize];
        let mut steps = 0usize;
        while current != 0 && current != wanted {
            require_equal(
                (current as usize) < hash.chains.len(),
                "System V hash chain contains an out-of-range symbol index",
            )?;
            current = hash.chains[current as usize];
            steps += 1;
            require_equal(steps <= names.len(), "System V hash chain contains a cycle")?;
        }
        require_equal(
            current == wanted,
            "System V hash lookup cannot reach one exact dynamic symbol",
        )?;
    }
    Ok(())
}

fn validate_gnu_hash_reachability(hash: &ElfGnuHash, names: &[&[u8]]) -> Result<(), Diagnostic> {
    require_equal(
        hash.bucket_count == GNU_HASH_BUCKET_COUNT
            && hash.symbol_offset == GNU_HASH_SYMBOL_OFFSET
            && hash.bloom_count == GNU_HASH_BLOOM_COUNT
            && hash.bloom_shift == GNU_HASH_BLOOM_SHIFT
            && hash.bloom.len() == hash.bloom_count as usize
            && hash.buckets.len() == hash.bucket_count as usize
            && hash.chains.len() == names.len()
            && hash.buckets.first() == Some(&GNU_HASH_SYMBOL_OFFSET),
        "GNU hash counts or canonical one-bucket symbol domain drifted",
    )?;
    let bloom_word = hash.bloom[0];
    for (index, name) in names.iter().enumerate() {
        let symbol_hash = gnu_hash(name);
        let first_bit = symbol_hash % GNU_HASH_WORD_BITS;
        let second_bit = (symbol_hash >> hash.bloom_shift) % GNU_HASH_WORD_BITS;
        require_equal(
            bloom_word & (1u64 << first_bit) != 0 && bloom_word & (1u64 << second_bit) != 0,
            "GNU hash bloom filter cannot admit one exact dynamic symbol",
        )?;
        let is_last = index + 1 == names.len();
        let expected_chain = (symbol_hash & !1) | u32::from(is_last);
        require_equal(
            hash.chains.get(index) == Some(&expected_chain),
            "GNU hash chain does not preserve an exact symbol hash or terminator",
        )?;
    }
    Ok(())
}

fn require_equal(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

fn elf_hash(bytes: &[u8]) -> u32 {
    let mut hash = 0u32;
    for byte in bytes {
        hash = hash.wrapping_shl(4).wrapping_add(u32::from(*byte));
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

fn non_authoritative_content_compatibility_fingerprint(
    inputs: &PlannedElfDynamicLinkInputs,
    contents: &ElfDynamicSectionContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-section-plan.v2");
    hash.bytes(inputs.interpreter().target().target_name().as_bytes());
    hash.bytes(
        &inputs
            .interpreter()
            .non_authoritative_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.interpreter);
    hash.bytes(&contents.dynstr);
    for symbol in &contents.dynsym {
        hash.bytes(&symbol.name.to_le_bytes());
        hash.byte(symbol.info);
        hash.byte(symbol.other);
        hash.bytes(&symbol.section_index.to_le_bytes());
        hash.bytes(&symbol.value.to_le_bytes());
        hash.bytes(&symbol.size.to_le_bytes());
    }
    hash.bytes(&contents.sysv_hash.bucket_count.to_le_bytes());
    hash.bytes(&contents.sysv_hash.chain_count.to_le_bytes());
    for value in contents
        .sysv_hash
        .buckets
        .iter()
        .chain(&contents.sysv_hash.chains)
    {
        hash.bytes(&value.to_le_bytes());
    }
    hash.bytes(&contents.gnu_hash.bucket_count.to_le_bytes());
    hash.bytes(&contents.gnu_hash.symbol_offset.to_le_bytes());
    hash.bytes(&contents.gnu_hash.bloom_count.to_le_bytes());
    hash.bytes(&contents.gnu_hash.bloom_shift.to_le_bytes());
    for value in &contents.gnu_hash.bloom {
        hash.bytes(&value.to_le_bytes());
    }
    for value in contents
        .gnu_hash
        .buckets
        .iter()
        .chain(&contents.gnu_hash.chains)
    {
        hash.bytes(&value.to_le_bytes());
    }
    for value in &contents.versym {
        hash.bytes(&value.to_le_bytes());
    }
    for need in &contents.verneed {
        hash.bytes(&need.version.to_le_bytes());
        hash.bytes(&need.count.to_le_bytes());
        hash.bytes(&need.file.to_le_bytes());
        hash.bytes(&need.auxiliary_offset.to_le_bytes());
        hash.bytes(&need.next_offset.to_le_bytes());
        for auxiliary in &need.auxiliaries {
            hash.bytes(&auxiliary.hash.to_le_bytes());
            hash.bytes(&auxiliary.flags.to_le_bytes());
            hash.bytes(&auxiliary.other.to_le_bytes());
            hash.bytes(&auxiliary.name.to_le_bytes());
            hash.bytes(&auxiliary.next_offset.to_le_bytes());
        }
    }
    for needed in &contents.needed {
        hash.bytes(&needed.to_le_bytes());
    }
    for binding in &contents.bindings {
        hash.bytes(&binding.compatibility_report_identity.to_le_bytes());
        hash.bytes(&binding.dynamic_symbol_index.to_le_bytes());
        hash.bytes(&binding.version_index.to_le_bytes());
    }
    hash.finish()
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests;
