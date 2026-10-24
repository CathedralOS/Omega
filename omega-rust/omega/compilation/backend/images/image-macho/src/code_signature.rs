//! The ad-hoc CodeDirectory: a SHA-256 per ISA page of the finished file, which
//! is why it can only be built last.
//!
//! One [`CodeSignatureLayout`] derives every offset of the blob, its length
//! included. The file plan needs that length before the finished bytes exist -
//! `LC_CODE_SIGNATURE` and the `__LINKEDIT` extent are written earlier - and the
//! writer fills the same offsets from the same code limit afterwards, so the
//! recorded extent and the emitted blob cannot drift apart. The finished
//! container's `__LINKEDIT` extent is checked against the actual file length by
//! `loader_mapping::validate_macho_container_mapping`.

use sha2::{Digest, Sha256};

use crate::file_layout::bytes::{write_be_u32, write_be_u64};
use crate::file_layout::layout::align_to;
use crate::isa::MachoIsa;

/// The CodeDirectory prefix, summed from the fields it stands for: nine 32-bit
/// header words, the four hash/version bytes, four 32-bit spare, scatter and
/// team words, and the four 64-bit code-limit and code-segment words.
const CODE_DIRECTORY_HEADER_SIZE: usize = 9 * 4 + 4 + 4 * 4 + 4 * 8;

/// The slots an ad-hoc signature fills with the requirements and (empty)
/// entitlements hashes; `nSpecialSlots` is their count.
const SPECIAL_SLOT_COUNT: usize = 2;

/// Every offset of one ad-hoc signature blob for an exact code limit.
struct CodeSignatureLayout {
    page_count: usize,
    page_size: usize,
    identifier_offset: usize,
    special_hash_offset: usize,
    hash_offset: usize,
    code_directory_offset: usize,
    code_directory_length: usize,
    requirements_offset: usize,
    entitlements_offset: usize,
    super_blob_length: usize,
}

impl CodeSignatureLayout {
    /// `code_limit` is the number of file bytes the signature covers.
    fn for_code_limit(code_limit: usize, identifier: &str, isa: MachoIsa) -> Self {
        let page_size = usize::try_from(isa.page_size()).expect("Mach-O page size");
        let page_count = code_limit.div_ceil(page_size);
        let special_hash_offset = align_to(CODE_DIRECTORY_HEADER_SIZE + identifier.len() + 1, 4);
        let hash_offset = special_hash_offset + SPECIAL_SLOT_COUNT * 32;
        let code_directory_offset = super_blob_header_size();
        let code_directory_length = hash_offset + page_count * 32;
        let requirements_offset = code_directory_offset + code_directory_length;
        let entitlements_offset = requirements_offset + requirements_blob().len();
        Self {
            page_count,
            page_size,
            identifier_offset: CODE_DIRECTORY_HEADER_SIZE,
            special_hash_offset,
            hash_offset,
            code_directory_offset,
            code_directory_length,
            requirements_offset,
            entitlements_offset,
            super_blob_length: entitlements_offset + empty_entitlements_blob().len(),
        }
    }

    /// The blob padded to its sixteen-byte alignment: the recorded extent and
    /// the emitted byte count.
    fn blob_length(&self) -> usize {
        align_to(self.super_blob_length, 16)
    }
}

/// The blob length for the exact `identifier` bound into the CodeDirectory.
/// The identifier length participates in offset arithmetic, so the caller must
/// supply the same value that `macho_ad_hoc_code_signature_for_container` later
/// writes; a late substitution would corrupt `LC_CODE_SIGNATURE`'s recorded
/// extent.
pub(super) fn code_signature_size(code_limit: usize, identifier: &str, isa: MachoIsa) -> usize {
    CodeSignatureLayout::for_code_limit(code_limit, identifier, isa).blob_length()
}

/// `identifier` is the already-validated signing identity (the authored
/// application identifier, or the executable-leaf fallback). It is written
/// verbatim after the CodeDirectory header.
pub(super) fn macho_ad_hoc_code_signature_for_container(
    code_bytes: &[u8],
    executable_segment_limit: usize,
    identifier: &str,
    isa: MachoIsa,
    main_binary: bool,
) -> Vec<u8> {
    let code_limit = code_bytes.len();
    let layout = CodeSignatureLayout::for_code_limit(code_limit, identifier, isa);

    let mut bytes = Vec::with_capacity(layout.blob_length());
    write_be_u32(&mut bytes, 0xfade0cc0);
    write_be_u32(
        &mut bytes,
        u32::try_from(layout.super_blob_length).expect("code signature size overflow"),
    );
    write_be_u32(&mut bytes, 3);
    write_be_u32(&mut bytes, 0);
    write_be_u32(
        &mut bytes,
        u32::try_from(layout.code_directory_offset).expect("CodeDirectory offset overflow"),
    );
    write_be_u32(&mut bytes, 2);
    write_be_u32(
        &mut bytes,
        u32::try_from(layout.requirements_offset).expect("requirements offset overflow"),
    );
    write_be_u32(&mut bytes, 0x10000);
    write_be_u32(
        &mut bytes,
        u32::try_from(layout.entitlements_offset).expect("entitlements offset overflow"),
    );

    let code_directory_start = bytes.len();
    write_be_u32(&mut bytes, 0xfade0c02);
    write_be_u32(
        &mut bytes,
        u32::try_from(layout.code_directory_length).expect("CodeDirectory size overflow"),
    );
    write_be_u32(&mut bytes, 0x20400);
    write_be_u32(&mut bytes, 0x2);
    write_be_u32(
        &mut bytes,
        u32::try_from(layout.hash_offset).expect("CodeDirectory hash offset overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(layout.identifier_offset).expect("CodeDirectory identifier offset overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(SPECIAL_SLOT_COUNT).expect("CodeDirectory special slot count overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(layout.page_count).expect("CodeDirectory page count overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(code_limit).expect("CodeDirectory code limit overflow"),
    );
    bytes.push(32);
    bytes.push(2);
    bytes.push(0);
    bytes.push(isa.code_signature_page_power());
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 0);
    write_be_u64(
        &mut bytes,
        u64::try_from(code_limit).expect("CodeDirectory code limit overflow"),
    );
    write_be_u64(&mut bytes, 0);
    write_be_u64(
        &mut bytes,
        u64::try_from(executable_segment_limit).expect("CodeDirectory exec segment limit overflow"),
    );
    write_be_u64(&mut bytes, u64::from(main_binary));
    // The header's field widths are what `CODE_DIRECTORY_HEADER_SIZE` sums, so
    // the identifier must land exactly where the layout says.
    debug_assert_eq!(bytes.len(), code_directory_start + layout.identifier_offset);

    bytes.extend(identifier.as_bytes());
    bytes.push(0);
    bytes.resize(code_directory_start + layout.special_hash_offset, 0);

    let requirements = requirements_blob();
    bytes.extend(Sha256::digest(requirements));
    bytes.extend([0u8; 32]);
    debug_assert_eq!(bytes.len(), code_directory_start + layout.hash_offset);

    for page_index in 0..layout.page_count {
        let start = page_index * layout.page_size;
        let end = (start + layout.page_size).min(code_limit);
        let digest = Sha256::digest(&code_bytes[start..end]);
        bytes.extend(digest);
    }
    debug_assert_eq!(bytes.len(), layout.requirements_offset);

    bytes.extend(requirements);
    debug_assert_eq!(bytes.len(), layout.entitlements_offset);
    bytes.extend(empty_entitlements_blob());
    bytes.resize(layout.blob_length(), 0);
    bytes
}

fn super_blob_header_size() -> usize {
    12 + 3 * 8
}

fn requirements_blob() -> &'static [u8; 12] {
    b"\xfa\xde\x0c\x01\0\0\0\x0c\0\0\0\0"
}

fn empty_entitlements_blob() -> &'static [u8; 8] {
    b"\xfa\xde\x0b\x01\0\0\0\x08"
}

/// Read the CodeDirectory identifier spelled inside one emitted executable.
///
/// This is the only public read of the signed container's authored identity:
/// package publication replays it against the retained build identifier and
/// the `CFBundleIdentifier` it wrote, so a substituted executable cannot pass
/// with matching producer assertions alone. `LC_CODE_SIGNATURE` fields are
/// little-endian like every load command; the superblob it points at is the
/// one big-endian structure in the file. Malformed input returns `None`.
pub fn code_signature_identifier(bytes: &[u8]) -> Option<String> {
    fn little(bytes: &[u8], offset: usize) -> Option<u32> {
        crate::codec_bytes::u32_at(bytes, offset)
    }
    fn big(bytes: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_be_bytes(
            bytes.get(offset..offset + 4)?.try_into().ok()?,
        ))
    }

    let command_count = little(bytes, 16)? as usize;
    let mut cursor = 32usize;
    let mut signature = None;
    for _ in 0..command_count {
        let command_size = little(bytes, cursor + 4)? as usize;
        let end = cursor.checked_add(command_size)?;
        if little(bytes, cursor)? == 0x1d
            && signature
                .replace((
                    little(bytes, cursor + 8)? as usize,
                    little(bytes, cursor + 12)? as usize,
                ))
                .is_some()
        {
            return None;
        }
        cursor = end;
    }
    let (offset, size) = signature?;
    let blob = bytes.get(offset..offset.checked_add(size)?)?;
    if big(blob, 0)? != 0xfade_0cc0 {
        return None;
    }
    let directory = blob.get(big(blob, 16)? as usize..)?;
    if big(directory, 0)? != 0xfade_0c02 {
        return None;
    }
    let identifier = directory.get(big(directory, 20)? as usize..)?;
    let end = identifier.iter().position(|byte| *byte == 0)?;
    String::from_utf8(identifier[..end].to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::{code_signature_size, macho_ad_hoc_code_signature_for_container};
    use crate::isa::MachoIsa;

    /// The extent `LC_CODE_SIGNATURE` records and the emitted blob are one
    /// derivation: the plan asks this layout for the length at the signature
    /// offset and the writer fills the same offsets for the bytes it hashed.
    /// Page count, identifier length and ISA enter both through one function.
    #[test]
    fn recorded_signature_extent_is_the_emitted_blob_length() {
        for isa in [MachoIsa::Aarch64, MachoIsa::X86_64] {
            let page = usize::try_from(isa.page_size()).expect("Mach-O page size");
            for code_limit in [page, page + 1, 3 * page + 7] {
                for identifier in ["omega-program", "com.omega.window-app"] {
                    let code = vec![0u8; code_limit];
                    let blob = macho_ad_hoc_code_signature_for_container(
                        &code, code_limit, identifier, isa, true,
                    );
                    assert_eq!(
                        blob.len(),
                        code_signature_size(code_limit, identifier, isa),
                        "{identifier} over {code_limit} bytes on {}",
                        isa.format_name(),
                    );
                }
            }
        }
    }
}
