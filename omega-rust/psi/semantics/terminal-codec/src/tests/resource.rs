use crate::sections::semantic_module::wire::decode_counted;
use crate::{CodecError, Reader};

#[test]
fn counted_decoder_rejects_impossible_capacity_before_allocation() {
    let bytes = u32::MAX.to_le_bytes();
    let mut reader = Reader::new(&bytes);
    assert_eq!(
        decode_counted::<u8>(&mut reader, |reader| reader.u8()),
        Err(CodecError::UnexpectedEnd)
    );
}
