//! Bounded byte reads and domain-separated digests, without compiler vocabulary.
//!
//! Representation owners choose their tags, field order and identity domains.
//! These primitives only preserve the bytes those owners supply.
#![forbid(unsafe_code)]

pub mod codec_bytes;
pub mod cursor;
pub mod digest_encoder;

#[cfg(test)]
mod tests {
    use super::{cursor::Cursor, digest_encoder::{DomainDigestEncoder, hex}};

    #[test]
    fn digest_preserves_the_standard_sha256_byte_protocol() {
        let mut digest = DomainDigestEncoder::new(b"");
        digest.raw(b"abc");
        assert_eq!(hex(&digest.finish()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
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
