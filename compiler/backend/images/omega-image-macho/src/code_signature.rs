use sha2::{Digest, Sha256};

use super::bytes::{write_be_u32, write_be_u64};
use super::constants::{
    CODE_SIGNATURE_PAGE_SIZE, CODE_SIGNATURE_PAGE_SIZE_POWER, MACHO_EXECUTABLE_BASE,
};
use super::layout::align_to;

pub(super) fn code_signature_size(code_limit: usize) -> usize {
    let page_count = code_slot_count(code_limit);
    let identifier = code_signature_identifier();
    let code_directory_header_size = 88usize;
    let code_directory_length =
        align_to(code_directory_header_size + identifier.len() + 1, 4) + page_count * 32;
    let super_blob_length = 20 + code_directory_length;
    align_to(super_blob_length, 16)
}

pub(super) fn macho_ad_hoc_code_signature(code_bytes: &[u8]) -> Vec<u8> {
    let code_limit = code_bytes.len();
    let page_count = code_slot_count(code_limit);
    let identifier = code_signature_identifier();
    let code_directory_header_size = 88usize;
    let identifier_offset = code_directory_header_size;
    let hash_offset = align_to(identifier_offset + identifier.len() + 1, 4);
    let code_directory_length = hash_offset + page_count * 32;
    let super_blob_length = 20 + code_directory_length;

    let mut bytes = Vec::with_capacity(align_to(super_blob_length, 16));
    write_be_u32(&mut bytes, 0xfade0cc0);
    write_be_u32(
        &mut bytes,
        u32::try_from(super_blob_length).expect("code signature size overflow"),
    );
    write_be_u32(&mut bytes, 1);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 20);

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
    write_be_u32(&mut bytes, 0);
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
    write_be_u64(&mut bytes, MACHO_EXECUTABLE_BASE);
    write_be_u64(&mut bytes, 0);
    write_be_u64(&mut bytes, 0);

    debug_assert_eq!(bytes.len(), 20 + identifier_offset);
    bytes.extend(identifier.as_bytes());
    bytes.push(0);
    bytes.resize(20 + hash_offset, 0);

    for page_index in 0..page_count {
        let start = page_index * CODE_SIGNATURE_PAGE_SIZE;
        let end = (start + CODE_SIGNATURE_PAGE_SIZE).min(code_limit);
        let digest = Sha256::digest(&code_bytes[start..end]);
        bytes.extend(digest);
    }

    bytes.resize(align_to(super_blob_length, 16), 0);
    bytes
}

fn code_slot_count(code_limit: usize) -> usize {
    code_limit.div_ceil(CODE_SIGNATURE_PAGE_SIZE)
}

fn code_signature_identifier() -> &'static str {
    "omega-program"
}
