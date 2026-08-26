//! Canonical ELF64 little-endian serialization of validated dynamic contents.
//!
//! This module serializes only address-free section payloads. The primary
//! contracts are the System V ABI [64-bit data types], [data encoding],
//! [Elf64_Sym], and [symbol hash] layout plus the LSB [symbol versioning]
//! structures.
//!
//! [64-bit data types]: https://gabi.xinuos.com/elf/01-intro.html#sixty-four-bit-data-types
//! [data encoding]: https://gabi.xinuos.com/elf/02-eheader.html#data-encoding
//! [Elf64_Sym]: https://gabi.xinuos.com/elf/05-symtab.html#symbol-table-entry
//! [symbol hash]: https://gabi.xinuos.com/elf/08-dynamic.html#hash-table
//! [symbol versioning]: https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/symversion.html

use crate::bytes::{write_u16, write_u32, write_u64};
use crate::dynamic_sections::{
    ElfDynamicSectionContents, ElfDynamicSymbol, ElfSysvHash, ElfVersionNeed,
    ElfVersionNeedAuxiliary, ValidatedElfDynamicSectionPlan,
};
use psi_diagnostics::Diagnostic;

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
    payload_identity: u64,
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

    pub fn symbol_version_byte_count(&self) -> usize {
        self.payloads.versym.len()
    }

    pub fn version_requirement_byte_count(&self) -> usize {
        self.payloads.verneed.len()
    }

    /// Compatibility fingerprint of the exact source content identity,
    /// ELF64-LSB encoding selection, section-kind boundaries, and serialized
    /// bytes. This is content identity, not image or loader authority.
    pub const fn payload_identity(&self) -> u64 {
        self.payload_identity
    }

    #[allow(dead_code)]
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
    pub(crate) versym: Vec<u8>,
    pub(crate) verneed: Vec<u8>,
}

struct Candidate {
    plan: ValidatedElfDynamicSectionPlan,
    payloads: ElfDynamicSectionPayloadBytes,
    payload_identity: u64,
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
    let payload_identity = payload_identity(&plan, &payloads);
    let candidate = Candidate {
        plan,
        payloads,
        payload_identity,
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
    if candidate.payload_identity != payload_identity(&candidate.plan, &candidate.payloads) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error("ELF dynamic payload identity does not replay"),
        });
    }
    Ok(ValidatedElfDynamicSectionPayloads {
        plan: candidate.plan,
        payloads: candidate.payloads,
        payload_identity: candidate.payload_identity,
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

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

fn payload_identity(
    plan: &ValidatedElfDynamicSectionPlan,
    payloads: &ElfDynamicSectionPayloadBytes,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-section-payloads.v1");
    hash.bytes(b"ELFCLASS64");
    hash.bytes(b"ELFDATA2LSB");
    hash.bytes(&plan.content_identity().to_le_bytes());
    for (kind, bytes) in [
        (b".interp".as_slice(), payloads.interpreter.as_slice()),
        (b".dynstr".as_slice(), payloads.dynstr.as_slice()),
        (b".dynsym".as_slice(), payloads.dynsym.as_slice()),
        (b".hash".as_slice(), payloads.sysv_hash.as_slice()),
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
mod tests {
    use super::*;
    use crate::{plan_elf_dynamic_link_inputs, plan_elf_dynamic_sections};
    use omega_image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use omega_object_file::{RelocationKind, SymbolKind};
    use omega_target::{
        ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_elf_interpreter_plan,
        normalize_foreign_locator,
    };
    use psi_arena::Handle;

    #[derive(Clone, Copy)]
    struct ImportFixture {
        object: &'static [u8],
        symbol: &'static [u8],
        version: &'static [u8],
    }

    const IMPORTS: [ImportFixture; 4] = [
        ImportFixture {
            object: b"libalpha\xff.so.1",
            symbol: b"alpha\xfe",
            version: b"OMEGA_1\xfd",
        },
        ImportFixture {
            object: b"libalpha\xff.so.1",
            symbol: b"beta",
            version: b"OMEGA_1\xfd",
        },
        ImportFixture {
            object: b"libalpha\xff.so.1",
            symbol: b"alpha\xfe",
            version: b"OMEGA_2",
        },
        ImportFixture {
            object: b"libbeta.so.2",
            symbol: b"gamma",
            version: b"OMEGA_1\xfd",
        },
    ];

    fn interpreter_path(target: TargetProfile) -> &'static [u8] {
        match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
            _ => unreachable!("payload fixture uses a Linux target"),
        }
    }

    fn structural_plan(
        target: TargetProfile,
        imports: &[ImportFixture],
    ) -> ValidatedElfDynamicSectionPlan {
        structural_plan_with_interpreter(target, imports, interpreter_path(target))
    }

    fn structural_plan_with_interpreter(
        target: TargetProfile,
        imports: &[ImportFixture],
        interpreter_path: &[u8],
    ) -> ValidatedElfDynamicSectionPlan {
        let native_target = target.native_target();
        let mut image = FinalImage::with_capacity(
            native_target,
            FinalImageMemory {
                text: vec![0; 64],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            imports.len(),
        );
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("__omega_payload_import_{index}"),
                section: FinalImageSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
            });
            image.symbol_table.imports.insert(FinalImageImport {
                symbol_handle,
                import: FinalImageImportPlan::Normalized(
                    normalize_foreign_locator(
                        ForeignLocatorCandidate::ElfVersioned {
                            object: fixture.object.to_vec(),
                            symbol: fixture.symbol.to_vec(),
                            version: fixture.version.to_vec(),
                        },
                        target,
                    )
                    .expect("valid payload fixture locator"),
                ),
            });
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset: index * 8,
                    byte_width: 4,
                    symbol_handle,
                    addend: 0,
                    kind: if native_target == NativeTarget::linux_arm64() {
                        RelocationKind::Aarch64Branch26
                    } else {
                        RelocationKind::X86_64Relative32
                    },
                });
        }
        let interpreter = normalize_elf_interpreter_plan(interpreter_path.to_vec(), target)
            .expect("valid payload fixture interpreter");
        let inputs =
            plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link preflight");
        plan_elf_dynamic_sections(inputs).expect("valid structural dynamic sections")
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let plan = structural_plan(target, &IMPORTS);
        let payloads = encode_payloads(plan.contents()).expect("encoded payloads");
        let payload_identity = payload_identity(&plan, &payloads);
        Candidate {
            plan,
            payloads,
            payload_identity,
        }
    }

    fn words(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
            .collect()
    }

    #[test]
    fn both_linux_targets_serialize_exact_elf64_lsb_payloads() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let serialized = serialize_elf_dynamic_sections(structural_plan(target, &IMPORTS))
                .expect("validated serialized payloads");
            let payloads = &serialized.payloads;

            assert_eq!(
                payloads.interpreter,
                [interpreter_path(target), &[0]].concat()
            );
            assert_eq!(payloads.dynstr.len(), 64);
            assert_eq!(
                payloads.dynstr,
                b"\0OMEGA_1\xfd\0OMEGA_2\0alpha\xfe\0beta\0gamma\0libalpha\xff.so.1\0libbeta.so.2\0"
            );
            assert_eq!(payloads.dynsym.len(), 120);
            assert_eq!(&payloads.dynsym[..24], &[0; 24]);
            assert_eq!(
                &payloads.dynsym[24..48],
                &[
                    18, 0, 0, 0, 0x12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ]
            );
            assert_eq!(payloads.sysv_hash.len(), 44);
            assert_eq!(
                words(&payloads.sysv_hash),
                [4, 5, 0, 3, 1, 0, 0, 2, 0, 4, 0]
            );
            assert_eq!(payloads.versym, [0, 0, 2, 0, 3, 0, 2, 0, 4, 0]);
            assert_eq!(payloads.verneed.len(), 80);
            assert_eq!(read_u16(&payloads.verneed, 0, "version").unwrap(), 1);
            assert_eq!(read_u16(&payloads.verneed, 2, "count").unwrap(), 2);
            assert_eq!(read_u32(&payloads.verneed, 4, "file").unwrap(), 36);
            assert_eq!(read_u32(&payloads.verneed, 8, "aux").unwrap(), 16);
            assert_eq!(read_u32(&payloads.verneed, 12, "next").unwrap(), 48);
            assert_eq!(read_u16(&payloads.verneed, 22, "other").unwrap(), 2);
            assert_eq!(read_u32(&payloads.verneed, 24, "name").unwrap(), 1);
            assert_eq!(read_u32(&payloads.verneed, 28, "next").unwrap(), 16);
            assert_eq!(read_u16(&payloads.verneed, 38, "other").unwrap(), 3);
            assert_eq!(read_u32(&payloads.verneed, 44, "next").unwrap(), 0);
            assert_eq!(read_u32(&payloads.verneed, 52, "file").unwrap(), 51);
            assert_eq!(read_u16(&payloads.verneed, 70, "other").unwrap(), 4);
            assert_eq!(serialized.dynamic_string_byte_count(), 64);
            assert_eq!(serialized.dynamic_symbol_byte_count(), 120);
            assert_eq!(serialized.system_v_hash_byte_count(), 44);
            assert_eq!(serialized.symbol_version_byte_count(), 10);
            assert_eq!(serialized.version_requirement_byte_count(), 80);
            assert_ne!(serialized.payload_identity(), 0);
            validate_payloads(serialized.plan(), payloads).expect("independent byte replay");
        }
    }

    #[test]
    fn serialized_payloads_and_identity_ignore_import_insertion_order() {
        let forward =
            serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &IMPORTS))
                .expect("forward payloads");
        let reversed = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
        let reverse =
            serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &reversed))
                .expect("reverse payloads");

        assert_eq!(forward.payloads, reverse.payloads);
        assert_eq!(forward.payload_identity(), reverse.payload_identity());
    }

    #[test]
    fn payload_identity_binds_profile_and_exact_serialized_coordinates() {
        let baseline =
            serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &IMPORTS))
                .expect("baseline payloads")
                .payload_identity();
        let changed_profile =
            serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxArm64, &IMPORTS))
                .expect("changed-profile payloads")
                .payload_identity();
        let changed_interpreter = serialize_elf_dynamic_sections(structural_plan_with_interpreter(
            TargetProfile::LinuxX64,
            &IMPORTS,
            b"/another/ld-linux-x86-64.so.2",
        ))
        .expect("changed-interpreter payloads")
        .payload_identity();
        let mut changed_imports = IMPORTS;
        changed_imports[0] = ImportFixture {
            object: b"libalpha\xff.so.1",
            symbol: b"changed_alpha\xfe",
            version: b"OMEGA_1\xfd",
        };
        let changed_coordinate = serialize_elf_dynamic_sections(structural_plan(
            TargetProfile::LinuxX64,
            &changed_imports,
        ))
        .expect("changed-coordinate payloads")
        .payload_identity();

        assert_ne!(baseline, changed_profile);
        assert_ne!(baseline, changed_interpreter);
        assert_ne!(baseline, changed_coordinate);
    }

    #[test]
    fn independent_decoder_rejects_every_payload_and_identity_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| {
                candidate.payloads.interpreter.pop();
            }),
            Box::new(|candidate| candidate.payloads.interpreter.push(0)),
            Box::new(|candidate| candidate.payloads.dynstr[0] = 1),
            Box::new(|candidate| {
                candidate.payloads.dynstr.pop();
            }),
            Box::new(|candidate| {
                candidate.payloads.dynsym.pop();
            }),
            Box::new(|candidate| candidate.payloads.dynsym.push(0)),
            Box::new(|candidate| candidate.payloads.dynsym[24..28].reverse()),
            Box::new(|candidate| candidate.payloads.dynsym[24] = 0),
            Box::new(|candidate| {
                candidate.payloads.sysv_hash.pop();
            }),
            Box::new(|candidate| candidate.payloads.sysv_hash.push(0)),
            Box::new(|candidate| candidate.payloads.sysv_hash[..4].fill(0)),
            Box::new(|candidate| candidate.payloads.sysv_hash[4..8].fill(0xff)),
            Box::new(|candidate| candidate.payloads.sysv_hash[8..12].fill(0xff)),
            Box::new(|candidate| {
                candidate.payloads.sysv_hash[28..32].copy_from_slice(&1u32.to_le_bytes())
            }),
            Box::new(|candidate| {
                candidate.payloads.versym.pop();
            }),
            Box::new(|candidate| candidate.payloads.versym.push(0)),
            Box::new(|candidate| candidate.payloads.versym[2] = 1),
            Box::new(|candidate| {
                candidate.payloads.verneed.pop();
            }),
            Box::new(|candidate| candidate.payloads.verneed.push(0)),
            Box::new(|candidate| candidate.payloads.verneed[2..4].fill(0xff)),
            Box::new(|candidate| candidate.payloads.verneed[8..12].fill(0)),
            Box::new(|candidate| candidate.payloads.verneed[12..16].fill(0)),
            Box::new(|candidate| candidate.payloads.verneed[28..32].fill(0)),
            Box::new(|candidate| candidate.payload_identity ^= 1),
        ];

        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxX64);
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt bytes must reject before sealing payloads");
            assert_eq!(
                error.candidate.plan.inputs().interpreter().target(),
                TargetProfile::LinuxX64,
                "decoder rejection retains exact structural-plan custody",
            );
        }
    }

    #[test]
    fn adversarial_lengths_counts_indexes_and_offsets_reject_without_panicking() {
        for bytes in [Vec::new(), vec![0; 7], vec![0xff; 8], vec![0; 12]] {
            assert!(decode_sysv_hash(&bytes).is_err());
        }
        let out_of_range_hash = [
            1u32.to_le_bytes(),
            2u32.to_le_bytes(),
            u32::MAX.to_le_bytes(),
            0u32.to_le_bytes(),
            0u32.to_le_bytes(),
        ]
        .concat();
        assert!(decode_sysv_hash(&out_of_range_hash).is_err());
        let cyclic_hash = [
            1u32.to_le_bytes(),
            3u32.to_le_bytes(),
            1u32.to_le_bytes(),
            0u32.to_le_bytes(),
            1u32.to_le_bytes(),
            0u32.to_le_bytes(),
        ]
        .concat();
        assert!(decode_sysv_hash(&cyclic_hash).is_err());

        for bytes in [Vec::new(), vec![0; 15], vec![0xff; 16], vec![0; 32]] {
            assert!(decode_verneed(&bytes, 1).is_err());
        }
        let mut cyclic_auxiliary = candidate(TargetProfile::LinuxX64).payloads.verneed;
        cyclic_auxiliary[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_verneed(&cyclic_auxiliary, 2).is_err());

        assert!(decode_dynsym(&[0; 23], 1).is_err());
        assert!(decode_versym(&[0], 1).is_err());
        assert!(referenced_string(&[0], u32::MAX).is_err());
    }
}
