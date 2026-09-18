//! Canonical ELF64 little-endian serialization of validated dynamic contents.
//!
//! This module serializes only address-free section payloads. The primary
//! contracts are the System V ABI [64-bit data types], [data encoding],
//! [Elf64_Sym], and [System V symbol hash] layout, the original GNU
//! [`DT_GNU_HASH` implementation], plus the LSB [symbol versioning]
//! structures.
//!
//! [64-bit data types]: https://gabi.xinuos.com/elf/01-intro.html#sixty-four-bit-data-types
//! [data encoding]: https://gabi.xinuos.com/elf/02-eheader.html#data-encoding
//! [Elf64_Sym]: https://gabi.xinuos.com/elf/05-symtab.html#symbol-table-entry
//! [System V symbol hash]: https://gabi.xinuos.com/elf/08-dynamic.html#hash-table
//! [`DT_GNU_HASH` implementation]: https://sourceware.org/pipermail/binutils/2006-July/048074.html
//! [symbol versioning]: https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/symversion.html

use crate::bytes::{write_u16, write_u32, write_u64};
use crate::dynamic_executable::checked::require;
use crate::dynamic_executable::import_sections::dynamic_sections::{
    ElfDynamicSectionContents, ElfDynamicSymbol, ElfGnuHash, ElfSysvHash, ElfVersionNeed,
    ElfVersionNeedAuxiliary, ValidatedElfDynamicSectionPlan,
};
use diagnostics::Diagnostic;

const ELF64_SYMBOL_SIZE: usize = 24;
const ELF64_VERSION_NEED_SIZE: usize = 16;
const ELF64_VERSION_AUXILIARY_SIZE: usize = 16;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently decoded and replayed ELF64-LSB dynamic section payloads.
///
/// The source structural plan remains owned by this non-clone carrier. These
/// bytes have no addresses, section indexes, program headers, relocation
/// effects, publication state, or runnable-image authority.
#[derive(Debug)]
#[must_use = "validated ELF payloads retain the exact structural section plan"]
pub struct ValidatedElfDynamicSectionPayloads {
    plan: ValidatedElfDynamicSectionPlan,
    payloads: ElfDynamicSectionPayloadBytes,
    non_authoritative_payload_compatibility_fingerprint: u64,
}

impl ValidatedElfDynamicSectionPayloads {
    pub const fn plan(&self) -> &ValidatedElfDynamicSectionPlan {
        &self.plan
    }

    pub fn interpreter_byte_count(&self) -> usize {
        self.payloads.interpreter.len()
    }

    pub fn dynamic_string_byte_count(&self) -> usize {
        self.payloads.dynstr.len()
    }

    pub fn dynamic_symbol_byte_count(&self) -> usize {
        self.payloads.dynsym.len()
    }

    pub fn system_v_hash_byte_count(&self) -> usize {
        self.payloads.sysv_hash.len()
    }

    pub fn gnu_hash_byte_count(&self) -> usize {
        self.payloads.gnu_hash.len()
    }

    pub fn symbol_version_byte_count(&self) -> usize {
        self.payloads.versym.len()
    }

    pub fn version_requirement_byte_count(&self) -> usize {
        self.payloads.verneed.len()
    }

    /// Compatibility fingerprint of the exact source content identity,
    /// ELF64-LSB encoding selection, section-kind boundaries, and serialized
    /// bytes. This is a content compatibility coordinate, not image or loader authority.
    pub const fn non_authoritative_payload_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_payload_compatibility_fingerprint
    }

    pub(crate) const fn payloads(&self) -> &ElfDynamicSectionPayloadBytes {
        &self.payloads
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfDynamicSectionPlan,
        ElfDynamicSectionPayloadBytes,
    ) {
        (self.plan, self.payloads)
    }
}

/// Rejected ELF dynamic payload serialization with exact plan custody.
#[derive(Debug)]
#[must_use = "ELF payload serialization rejection retains the validated structural plan"]
pub struct ElfDynamicSectionSerializationError {
    plan: ValidatedElfDynamicSectionPlan,
    diagnostic: Diagnostic,
}

impl ElfDynamicSectionSerializationError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicSectionPlan, Diagnostic) {
        (self.plan, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicSectionSerializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicSectionSerializationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfDynamicSectionPayloadBytes {
    pub(crate) interpreter: Vec<u8>,
    pub(crate) dynstr: Vec<u8>,
    pub(crate) dynsym: Vec<u8>,
    pub(crate) sysv_hash: Vec<u8>,
    pub(crate) gnu_hash: Vec<u8>,
    pub(crate) versym: Vec<u8>,
    pub(crate) verneed: Vec<u8>,
}

struct Candidate {
    plan: ValidatedElfDynamicSectionPlan,
    payloads: ElfDynamicSectionPayloadBytes,
    non_authoritative_payload_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Serialize all currently validated address-free dynamic contents as exact
/// ELF64 little-endian section payloads, then independently decode and replay
/// them before sealing success.
///
/// The `DT_NEEDED` string-index roster deliberately remains typed data in the
/// structural plan. Its `Elf64_Dyn` encoding belongs to the later complete
/// `.dynamic` plan alongside address-bearing tags. This function does not
/// place sections, write headers, lower relocations, or mutate image bytes.
pub fn serialize_elf_dynamic_sections(
    plan: ValidatedElfDynamicSectionPlan,
) -> Result<ValidatedElfDynamicSectionPayloads, Box<ElfDynamicSectionSerializationError>> {
    let payloads = match encode_payloads(plan.contents()) {
        Ok(payloads) => payloads,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicSectionSerializationError {
                plan,
                diagnostic,
            }));
        }
    };
    let non_authoritative_payload_compatibility_fingerprint =
        non_authoritative_payload_compatibility_fingerprint(&plan, &payloads);
    let candidate = Candidate {
        plan,
        payloads,
        non_authoritative_payload_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfDynamicSectionSerializationError {
            plan: error.candidate.plan,
            diagnostic: error.diagnostic,
        })),
    }
}

fn encode_payloads(
    contents: &ElfDynamicSectionContents,
) -> Result<ElfDynamicSectionPayloadBytes, Diagnostic> {
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

    let gnu_hash_bloom_size = checked_product(
        contents.gnu_hash.bloom.len(),
        8,
        "GNU hash bloom payload size",
    )?;
    let gnu_hash_bucket_size = checked_product(
        contents.gnu_hash.buckets.len(),
        4,
        "GNU hash bucket payload size",
    )?;
    let gnu_hash_chain_size = checked_product(
        contents.gnu_hash.chains.len(),
        4,
        "GNU hash chain payload size",
    )?;
    let gnu_hash_size = checked_add(16, gnu_hash_bloom_size, "GNU hash bloom payload end")?;
    let gnu_hash_size = checked_add(
        gnu_hash_size,
        gnu_hash_bucket_size,
        "GNU hash bucket payload end",
    )?;
    let gnu_hash_size = checked_add(
        gnu_hash_size,
        gnu_hash_chain_size,
        "GNU hash chain payload end",
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

    Ok(ElfDynamicSectionPayloadBytes {
        interpreter: contents.interpreter.clone(),
        dynstr: contents.dynstr.clone(),
        dynsym,
        sysv_hash,
        gnu_hash,
        versym,
        verneed,
    })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicSectionPayloads, CandidateValidationError> {
    if let Err(diagnostic) = validate_payloads(&candidate.plan, &candidate.payloads) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_payload_compatibility_fingerprint
        != non_authoritative_payload_compatibility_fingerprint(&candidate.plan, &candidate.payloads)
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF dynamic payload compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfDynamicSectionPayloads {
        plan: candidate.plan,
        payloads: candidate.payloads,
        non_authoritative_payload_compatibility_fingerprint: candidate
            .non_authoritative_payload_compatibility_fingerprint,
    })
}

fn validate_payloads(
    plan: &ValidatedElfDynamicSectionPlan,
    payloads: &ElfDynamicSectionPayloadBytes,
) -> Result<(), Diagnostic> {
    let contents = plan.contents();
    require(
        payloads.interpreter == contents.interpreter
            && payloads.interpreter.last() == Some(&0)
            && payloads
                .interpreter
                .iter()
                .filter(|byte| **byte == 0)
                .count()
                == 1,
        "serialized PT_INTERP payload does not preserve one exact terminated pathname",
    )?;
    require(
        payloads.dynstr == contents.dynstr
            && payloads.dynstr.first() == Some(&0)
            && payloads.dynstr.last() == Some(&0),
        "serialized dynamic string payload is not exact or NUL framed",
    )?;

    let dynsym = decode_dynsym(&payloads.dynsym, contents.dynsym.len())?;
    require(
        dynsym == contents.dynsym,
        "decoded ELF64 dynamic symbols drifted from the validated plan",
    )?;
    let sysv_hash = decode_sysv_hash(&payloads.sysv_hash)?;
    require(
        sysv_hash == contents.sysv_hash && sysv_hash.chain_count as usize == dynsym.len(),
        "decoded System V hash drifted from the validated dynamic symbols",
    )?;
    let gnu_hash = decode_gnu_hash(&payloads.gnu_hash, dynsym.len())?;
    require(
        gnu_hash == contents.gnu_hash,
        "decoded GNU hash drifted from the validated dynamic symbols",
    )?;
    let versym = decode_versym(&payloads.versym, contents.versym.len())?;
    require(
        versym == contents.versym && versym.len() == dynsym.len(),
        "decoded GNU symbol-version rows drifted from the dynamic symbols",
    )?;
    let verneed = decode_verneed(&payloads.verneed, contents.verneed.len())?;
    require(
        verneed == contents.verneed,
        "decoded GNU version requirements drifted from the validated plan",
    )?;

    for symbol in dynsym.iter().skip(1) {
        referenced_string(&payloads.dynstr, symbol.name)?;
    }
    for offset in &contents.needed {
        referenced_string(&payloads.dynstr, *offset)?;
    }
    for need in &verneed {
        referenced_string(&payloads.dynstr, need.file)?;
        for auxiliary in &need.auxiliaries {
            referenced_string(&payloads.dynstr, auxiliary.name)?;
        }
    }
    Ok(())
}

fn decode_dynsym(bytes: &[u8], symbol_count: usize) -> Result<Vec<ElfDynamicSymbol>, Diagnostic> {
    let expected_size = checked_product(
        symbol_count,
        ELF64_SYMBOL_SIZE,
        "decoded ELF64 dynamic symbol payload size",
    )?;
    require(
        bytes.len() == expected_size,
        "ELF64 dynamic symbol payload has a truncated row or trailing bytes",
    )?;
    let mut symbols = Vec::with_capacity(symbol_count);
    for index in 0..symbol_count {
        let offset = checked_product(index, ELF64_SYMBOL_SIZE, "ELF64 symbol row offset")?;
        symbols.push(ElfDynamicSymbol {
            name: read_u32(bytes, offset, "Elf64_Sym.st_name")?,
            info: read_u8(bytes, offset + 4, "Elf64_Sym.st_info")?,
            other: read_u8(bytes, offset + 5, "Elf64_Sym.st_other")?,
            section_index: read_u16(bytes, offset + 6, "Elf64_Sym.st_shndx")?,
            value: read_u64(bytes, offset + 8, "Elf64_Sym.st_value")?,
            size: read_u64(bytes, offset + 16, "Elf64_Sym.st_size")?,
        });
    }
    require(
        symbols.first() == Some(&ElfDynamicSymbol::default()),
        "decoded ELF64 dynamic symbols lack the reserved zero row",
    )?;
    Ok(symbols)
}

fn decode_sysv_hash(bytes: &[u8]) -> Result<ElfSysvHash, Diagnostic> {
    let bucket_count = read_u32(bytes, 0, "System V nbucket")?;
    let chain_count = read_u32(bytes, 4, "System V nchain")?;
    require(
        bucket_count > 0 && chain_count > 0,
        "System V hash payload has a zero table count",
    )?;
    let word_count = usize::try_from(bucket_count)
        .ok()
        .and_then(|buckets| buckets.checked_add(chain_count as usize))
        .ok_or_else(|| Diagnostic::error("System V hash decoded word count overflow"))?;
    let expected_size = checked_product(word_count, 4, "System V decoded word byte count")?
        .checked_add(8)
        .ok_or_else(|| Diagnostic::error("System V decoded payload size overflow"))?;
    require(
        bytes.len() == expected_size,
        "System V hash counts do not consume the exact payload bytes",
    )?;

    let mut offset = 8usize;
    let mut buckets = Vec::with_capacity(bucket_count as usize);
    for _ in 0..bucket_count {
        buckets.push(read_u32(bytes, offset, "System V hash bucket")?);
        offset += 4;
    }
    let mut chains = Vec::with_capacity(chain_count as usize);
    for _ in 0..chain_count {
        chains.push(read_u32(bytes, offset, "System V hash chain")?);
        offset += 4;
    }
    require(
        buckets
            .iter()
            .chain(&chains)
            .all(|index| *index < chain_count)
            && chains.first() == Some(&0),
        "System V hash payload contains an out-of-range symbol index",
    )?;
    for bucket in &buckets {
        let mut index = *bucket;
        let mut steps = 0u32;
        while index != 0 {
            index = chains[index as usize];
            steps = steps.saturating_add(1);
            require(
                steps <= chain_count,
                "System V hash payload contains a chain cycle",
            )?;
        }
    }
    Ok(ElfSysvHash {
        bucket_count,
        chain_count,
        buckets,
        chains,
    })
}

fn decode_gnu_hash(bytes: &[u8], dynamic_symbol_count: usize) -> Result<ElfGnuHash, Diagnostic> {
    let bucket_count = read_u32(bytes, 0, "GNU hash nbuckets")?;
    let symbol_offset = read_u32(bytes, 4, "GNU hash symoffset")?;
    let bloom_count = read_u32(bytes, 8, "GNU hash bloom_size")?;
    let bloom_shift = read_u32(bytes, 12, "GNU hash bloom_shift")?;
    require(
        bucket_count > 0 && bloom_count > 0 && symbol_offset > 0,
        "GNU hash payload has a zero table count or symbol offset",
    )?;
    let symbol_offset = symbol_offset as usize;
    require(
        symbol_offset <= dynamic_symbol_count,
        "GNU hash symbol offset exceeds the dynamic symbol count",
    )?;
    let chain_count = dynamic_symbol_count - symbol_offset;
    let bloom_bytes =
        checked_product(bloom_count as usize, 8, "GNU hash decoded bloom byte count")?;
    let bucket_bytes = checked_product(
        bucket_count as usize,
        4,
        "GNU hash decoded bucket byte count",
    )?;
    let chain_bytes = checked_product(chain_count, 4, "GNU hash decoded chain byte count")?;
    let expected_size = checked_add(16, bloom_bytes, "GNU hash decoded bloom end")?
        .checked_add(bucket_bytes)
        .and_then(|size| size.checked_add(chain_bytes))
        .ok_or_else(|| Diagnostic::error("GNU hash decoded payload size overflow"))?;
    require(
        bytes.len() == expected_size,
        "GNU hash counts do not consume the exact payload bytes",
    )?;

    let mut offset = 16usize;
    let mut bloom = Vec::with_capacity(bloom_count as usize);
    for _ in 0..bloom_count {
        bloom.push(read_u64(bytes, offset, "GNU hash bloom word")?);
        offset += 8;
    }
    let mut buckets = Vec::with_capacity(bucket_count as usize);
    for _ in 0..bucket_count {
        buckets.push(read_u32(bytes, offset, "GNU hash bucket")?);
        offset += 4;
    }
    let mut chains = Vec::with_capacity(chain_count);
    for _ in 0..chain_count {
        chains.push(read_u32(bytes, offset, "GNU hash chain")?);
        offset += 4;
    }
    require(
        buckets.iter().all(|index| {
            *index == 0
                || ((*index as usize) >= symbol_offset && (*index as usize) < dynamic_symbol_count)
        }),
        "GNU hash bucket contains an out-of-range dynamic symbol index",
    )?;
    for bucket in buckets.iter().copied().filter(|bucket| *bucket != 0) {
        let mut symbol_index = bucket as usize;
        let mut steps = 0usize;
        loop {
            let chain_index = symbol_index - symbol_offset;
            let chain = *chains.get(chain_index).ok_or_else(|| {
                Diagnostic::error("GNU hash chain exceeds the dynamic symbol domain")
            })?;
            steps += 1;
            require(
                steps <= chains.len(),
                "GNU hash chain lacks a bounded terminator",
            )?;
            if chain & 1 != 0 {
                break;
            }
            symbol_index = symbol_index
                .checked_add(1)
                .ok_or_else(|| Diagnostic::error("GNU hash dynamic symbol index overflow"))?;
            require(
                symbol_index < dynamic_symbol_count,
                "GNU hash chain lacks a terminator before the symbol-table end",
            )?;
        }
    }
    Ok(ElfGnuHash {
        bucket_count,
        symbol_offset: symbol_offset as u32,
        bloom_count,
        bloom_shift,
        bloom,
        buckets,
        chains,
    })
}

fn decode_versym(bytes: &[u8], symbol_count: usize) -> Result<Vec<u16>, Diagnostic> {
    let expected_size =
        checked_product(symbol_count, 2, "decoded GNU symbol-version payload size")?;
    require(
        bytes.len() == expected_size,
        "GNU symbol-version payload has a truncated row or trailing bytes",
    )?;
    (0..symbol_count)
        .map(|index| read_u16(bytes, index * 2, "GNU symbol-version row"))
        .collect()
}

fn decode_verneed(bytes: &[u8], need_count: usize) -> Result<Vec<ElfVersionNeed>, Diagnostic> {
    if need_count == 0 {
        require(
            bytes.is_empty(),
            "GNU version requirement payload has rows without a required object",
        )?;
        return Ok(Vec::new());
    }
    let mut needs = Vec::with_capacity(need_count);
    let mut need_offset = 0usize;
    let mut consumed_end = 0usize;
    for need_index in 0..need_count {
        let version = read_u16(bytes, need_offset, "Elf64_Verneed.vn_version")?;
        let count = read_u16(bytes, need_offset + 2, "Elf64_Verneed.vn_cnt")?;
        let file = read_u32(bytes, need_offset + 4, "Elf64_Verneed.vn_file")?;
        let auxiliary_offset = read_u32(bytes, need_offset + 8, "Elf64_Verneed.vn_aux")?;
        let next_offset = read_u32(bytes, need_offset + 12, "Elf64_Verneed.vn_next")?;
        require(
            count > 0 && auxiliary_offset == ELF64_VERSION_NEED_SIZE as u32,
            "GNU version requirement has an empty or noncanonical auxiliary chain",
        )?;
        let canonical_group_size = checked_product(
            count as usize,
            ELF64_VERSION_AUXILIARY_SIZE,
            "GNU version requirement group auxiliary size",
        )?
        .checked_add(ELF64_VERSION_NEED_SIZE)
        .ok_or_else(|| Diagnostic::error("GNU version requirement group size overflow"))?;
        let group_end = checked_add(
            need_offset,
            canonical_group_size,
            "GNU version requirement group end",
        )?;
        require(
            group_end <= bytes.len(),
            "GNU version requirement count exceeds the remaining payload bytes",
        )?;
        let mut auxiliary_cursor = checked_add(
            need_offset,
            auxiliary_offset as usize,
            "GNU version auxiliary offset",
        )?;
        let mut auxiliaries = Vec::with_capacity(count as usize);
        for auxiliary_index in 0..count {
            let hash = read_u32(bytes, auxiliary_cursor, "Elf64_Vernaux.vna_hash")?;
            let flags = read_u16(bytes, auxiliary_cursor + 4, "Elf64_Vernaux.vna_flags")?;
            let other = read_u16(bytes, auxiliary_cursor + 6, "Elf64_Vernaux.vna_other")?;
            let name = read_u32(bytes, auxiliary_cursor + 8, "Elf64_Vernaux.vna_name")?;
            let auxiliary_next = read_u32(bytes, auxiliary_cursor + 12, "Elf64_Vernaux.vna_next")?;
            let last_auxiliary = auxiliary_index + 1 == count;
            require(
                (last_auxiliary && auxiliary_next == 0)
                    || (!last_auxiliary && auxiliary_next == ELF64_VERSION_AUXILIARY_SIZE as u32),
                "GNU version auxiliary chain has a noncanonical next offset",
            )?;
            auxiliaries.push(ElfVersionNeedAuxiliary {
                hash,
                flags,
                other,
                name,
                next_offset: auxiliary_next,
            });
            if !last_auxiliary {
                auxiliary_cursor = checked_add(
                    auxiliary_cursor,
                    auxiliary_next as usize,
                    "GNU next version auxiliary offset",
                )?;
            }
        }
        consumed_end = checked_add(
            auxiliary_cursor,
            ELF64_VERSION_AUXILIARY_SIZE,
            "GNU version requirement consumed byte count",
        )?;
        let last_need = need_index + 1 == need_count;
        require(
            (last_need && next_offset == 0)
                || (!last_need && next_offset as usize == canonical_group_size),
            "GNU version requirement chain has a noncanonical next offset",
        )?;
        needs.push(ElfVersionNeed {
            version,
            count,
            file,
            auxiliary_offset,
            next_offset,
            auxiliaries,
        });
        if !last_need {
            need_offset = checked_add(
                need_offset,
                next_offset as usize,
                "GNU next version requirement offset",
            )?;
        }
    }
    require(
        consumed_end == bytes.len(),
        "GNU version requirement chain does not consume the exact payload bytes",
    )?;
    Ok(needs)
}

fn referenced_string(bytes: &[u8], offset: u32) -> Result<&[u8], Diagnostic> {
    let offset = offset as usize;
    let suffix = bytes.get(offset..).ok_or_else(|| {
        Diagnostic::error("ELF dynamic string reference exceeds the serialized payload")
    })?;
    let end = suffix
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| Diagnostic::error("ELF dynamic string reference lacks a NUL terminator"))?;
    Ok(&suffix[..end])
}

fn read_u8(bytes: &[u8], offset: usize, context: &'static str) -> Result<u8, Diagnostic> {
    bytes
        .get(offset)
        .copied()
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))
}

fn read_u16(bytes: &[u8], offset: usize, context: &'static str) -> Result<u16, Diagnostic> {
    Ok(u16::from_le_bytes(read_array(bytes, offset, context)?))
}

fn read_u32(bytes: &[u8], offset: usize, context: &'static str) -> Result<u32, Diagnostic> {
    Ok(u32::from_le_bytes(read_array(bytes, offset, context)?))
}

fn read_u64(bytes: &[u8], offset: usize, context: &'static str) -> Result<u64, Diagnostic> {
    Ok(u64::from_le_bytes(read_array(bytes, offset, context)?))
}

fn read_array<const N: usize>(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<[u8; N], Diagnostic> {
    let end = checked_add(offset, N, context)?;
    bytes
        .get(offset..end)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))
}

fn checked_product(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflow")))
}

fn checked_add(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflow")))
}

fn non_authoritative_payload_compatibility_fingerprint(
    plan: &ValidatedElfDynamicSectionPlan,
    payloads: &ElfDynamicSectionPayloadBytes,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-section-payloads.v2");
    hash.bytes(b"ELFCLASS64");
    hash.bytes(b"ELFDATA2LSB");
    hash.bytes(
        &plan
            .non_authoritative_content_compatibility_fingerprint()
            .to_le_bytes(),
    );
    for (kind, bytes) in [
        (b".interp".as_slice(), payloads.interpreter.as_slice()),
        (b".dynstr".as_slice(), payloads.dynstr.as_slice()),
        (b".dynsym".as_slice(), payloads.dynsym.as_slice()),
        (b".hash".as_slice(), payloads.sysv_hash.as_slice()),
        (b".gnu.hash".as_slice(), payloads.gnu_hash.as_slice()),
        (b".gnu.version".as_slice(), payloads.versym.as_slice()),
        (b".gnu.version_r".as_slice(), payloads.verneed.as_slice()),
    ] {
        hash.bytes(kind);
        hash.bytes(bytes);
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
