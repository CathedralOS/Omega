/*!
We share the byte operations used by compiler artifact formats. Each format
chooses its own tags, field order and digest domain. The cursor reads
little-endian fields from a borrowed slice. A fixed-width read advances only
after all its bytes are present. For a counted sequence, we divide the
remaining bytes by the minimum encoded element width before accepting the
count. This avoids multiplying an untrusted count and leaves allocation and
element validation to the decoder.
*/

/*!

Two variable-width fields can have the same concatenated bytes with different
boundaries. We prefix each with its byte length when hashing those fields, so
the digest input records where each field ends. Fixed-width fields already
have boundaries. The tests below pin both the SHA-256 byte stream and cursor
behavior on a short read.
*/

/*!

Write a tagged header and one counted field, then check the count before
reading the field's elements.

```
use wire::{codec_bytes, cursor::Cursor};

let mut bytes = Vec::new();
codec_bytes::encode_header(&mut bytes, b"EXAMPLE\0", 1);
codec_bytes::push_count_u32(&mut bytes, 1);
codec_bytes::push_encoded(&mut bytes, 513u16.to_le_bytes());

let mut input = Cursor::new(&bytes);
assert_eq!(input.take(8).unwrap(), b"EXAMPLE\0");
assert_eq!(input.u32().unwrap(), 1);
assert_eq!(input.counted(2).unwrap(), 1);
assert_eq!(input.u16().unwrap(), 513);
assert_eq!(input.remaining(), 0);
```
*/

#![forbid(unsafe_code)]

pub mod codec_bytes;
pub mod cursor;
pub mod digest_encoder;

#[cfg(test)]
mod tests {
    use super::{
        cursor::Cursor,
        digest_encoder::{DomainDigestEncoder, hex},
    };

    #[test]
    fn digest_preserves_the_standard_sha256_byte_protocol() {
        let mut digest = DomainDigestEncoder::new(b"");
        digest.raw(b"abc");
        assert_eq!(
            hex(&digest.finish()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn framed_fields_do_not_alias_concatenated_fields() {
        let mut left = DomainDigestEncoder::new(b"record\0");
        left.bytes(b"ab");
        left.bytes(b"c");
        let mut right = DomainDigestEncoder::new(b"record\0");
        right.bytes(b"a");
        right.bytes(b"bc");
        assert_ne!(left.finish(), right.finish());
    }

    #[test]
    fn an_incomplete_read_does_not_consume_the_remaining_input() {
        let mut cursor = Cursor::new(&[1, 2, 3]);
        assert_eq!(cursor.byte().unwrap(), 1);
        assert!(cursor.u32().is_err());
        assert_eq!(cursor.remaining(), 2);
        assert_eq!(cursor.u16().unwrap(), 0x0302);
    }
}

mod error_display;
mod record_views;
mod tagged_vocabulary;

pub mod fnv1a;
