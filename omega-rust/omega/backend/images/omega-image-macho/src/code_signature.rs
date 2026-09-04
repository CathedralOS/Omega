//! The ad-hoc CodeDirectory: a SHA-256 per 16 KiB page of the finished file, which
//! is why it can only be built last.

use sha2::{Digest, Sha256};

use super::bytes::{write_be_u32, write_be_u64};
use super::constants::{CODE_SIGNATURE_PAGE_SIZE, CODE_SIGNATURE_PAGE_SIZE_POWER};
use super::layout::align_to;

pub(super) fn code_signature_size(code_limit: usize) -> usize {
    let page_count = code_slot_count(code_limit);
    let identifier = code_signature_identifier();
    let code_directory_header_size = 88usize;
    let special_slot_count = 2usize;
    let hash_offset =
        align_to(code_directory_header_size + identifier.len() + 1, 4) + special_slot_count * 32;
    let code_directory_length = hash_offset + page_count * 32;
    let super_blob_length = super_blob_header_size()
        + code_directory_length
        + requirements_blob().len()
        + empty_entitlements_blob().len();
    align_to(super_blob_length, 16)
}

pub(super) fn macho_ad_hoc_code_signature(
    code_bytes: &[u8],
    executable_segment_limit: usize,
) -> Vec<u8> {
    let code_limit = code_bytes.len();
    let page_count = code_slot_count(code_limit);
    let identifier = code_signature_identifier();
    let code_directory_header_size = 88usize;
    let special_slot_count = 2usize;
    let identifier_offset = code_directory_header_size;
    let special_hash_offset = align_to(identifier_offset + identifier.len() + 1, 4);
    let hash_offset = special_hash_offset + special_slot_count * 32;
    let code_directory_length = hash_offset + page_count * 32;
    let code_directory_offset = super_blob_header_size();
    let requirements_offset = code_directory_offset + code_directory_length;
    let entitlements_offset = requirements_offset + requirements_blob().len();
    let super_blob_length = entitlements_offset + empty_entitlements_blob().len();

    let mut bytes = Vec::with_capacity(align_to(super_blob_length, 16));
    write_be_u32(&mut bytes, 0xfade0cc0);
    write_be_u32(
        &mut bytes,
        u32::try_from(super_blob_length).expect("code signature size overflow"),
    );
    write_be_u32(&mut bytes, 3);
    write_be_u32(&mut bytes, 0);
    write_be_u32(
        &mut bytes,
        u32::try_from(code_directory_offset).expect("CodeDirectory offset overflow"),
    );
    write_be_u32(&mut bytes, 2);
    write_be_u32(
        &mut bytes,
        u32::try_from(requirements_offset).expect("requirements offset overflow"),
    );
    write_be_u32(&mut bytes, 0x10000);
    write_be_u32(
        &mut bytes,
        u32::try_from(entitlements_offset).expect("entitlements offset overflow"),
    );

    let code_directory_start = bytes.len();
    write_be_u32(&mut bytes, 0xfade0c02);
    write_be_u32(
        &mut bytes,
        u32::try_from(code_directory_length).expect("CodeDirectory size overflow"),
    );
    write_be_u32(&mut bytes, 0x20400);
    write_be_u32(&mut bytes, 0x2);
    write_be_u32(
        &mut bytes,
        u32::try_from(hash_offset).expect("CodeDirectory hash offset overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(identifier_offset).expect("CodeDirectory identifier offset overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(special_slot_count).expect("CodeDirectory special slot count overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(page_count).expect("CodeDirectory page count overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(code_limit).expect("CodeDirectory code limit overflow"),
    );
    bytes.push(32);
    bytes.push(2);
    bytes.push(0);
    bytes.push(CODE_SIGNATURE_PAGE_SIZE_POWER);
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
    write_be_u64(&mut bytes, 1);

    debug_assert_eq!(bytes.len(), code_directory_start + identifier_offset);
    bytes.extend(identifier.as_bytes());
    bytes.push(0);
    bytes.resize(code_directory_start + special_hash_offset, 0);

    let requirements = requirements_blob();
    bytes.extend(Sha256::digest(requirements));
    bytes.extend([0u8; 32]);
    debug_assert_eq!(bytes.len(), code_directory_start + hash_offset);

    for page_index in 0..page_count {
        let start = page_index * CODE_SIGNATURE_PAGE_SIZE;
        let end = (start + CODE_SIGNATURE_PAGE_SIZE).min(code_limit);
        let digest = Sha256::digest(&code_bytes[start..end]);
        bytes.extend(digest);
    }

    debug_assert_eq!(bytes.len(), requirements_offset);
    bytes.extend(requirements);
    debug_assert_eq!(bytes.len(), entitlements_offset);
    bytes.extend(empty_entitlements_blob());
    bytes.resize(align_to(super_blob_length, 16), 0);
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

fn code_slot_count(code_limit: usize) -> usize {
    code_limit.div_ceil(CODE_SIGNATURE_PAGE_SIZE)
}

fn code_signature_identifier() -> &'static str {
    "omega-program"
}
