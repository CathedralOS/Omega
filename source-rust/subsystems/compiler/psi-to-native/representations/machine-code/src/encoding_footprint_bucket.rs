/*
Interning buckets for decoded footprints (`omega.selected-form-decoded-footprint.storage.v1`).

`SharedFootprints` keys equal footprints to one shared row through this digest so cloned
encodings share immutable footprints. The digest is a storage key only; no check compares it,
and the encoding itself carries no content hash.
*/

use super::SelectedFormDecodedFootprint;
use crate::encode_units;
use crate::identity_encoding::encode_views;

pub(super) fn footprint_bucket(footprint: &SelectedFormDecodedFootprint) -> [u8; 32] {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.selected-form-decoded-footprint.storage.v1",
    );
    encode_views(&mut hasher, &footprint.register_reads);
    encode_views(&mut hasher, &footprint.register_writes);
    encode_units(&mut hasher, &footprint.implicit_defs);
    encode_units(&mut hasher, &footprint.implicit_clobbers);
    selected_instructions::encode_machine_encoded_effects_identity(&mut hasher, &footprint.encoded);
    hasher.finish()
}
