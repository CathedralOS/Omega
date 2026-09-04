//! The OMGOBJ container: fixed 44-byte header, then symbols, relocations, and
//! the raw text and data bytes appended verbatim with no padding.

use crate::{ObjectPlan, RelocationPlan};
use bytes::{write_u32, write_u64};
use ids::{architecture_id, object_format_id};
use omega_target::NativeTarget;
use relocations::write_relocations;
use sections::bss_size;
use symbols::write_symbols;

mod bytes;
mod ids;
mod relocations;
mod sections;
mod symbols;

const OBJECT_CONTAINER_VERSION: u32 = 6;

pub struct ObjectContainerInput<'a> {
    pub target: NativeTarget,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectContainerOutput {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub relocations: usize,
}

pub fn emit_omega_object_container(input: ObjectContainerInput<'_>) -> ObjectContainerOutput {
    let bss_bytes = bss_size(input.object);

    let mut bytes = Vec::new();
    bytes.extend(b"OMGOBJ\0\0");
    write_u32(&mut bytes, OBJECT_CONTAINER_VERSION);
    write_u32(&mut bytes, architecture_id(input.target.architecture));
    write_u32(&mut bytes, object_format_id(input.target.object_format));
    write_u64(
        &mut bytes,
        u64::try_from(input.text_bytes.len()).expect("text size overflow"),
    );
    write_u64(
        &mut bytes,
        u64::try_from(input.data_bytes.len()).expect("data size overflow"),
    );
    write_u64(
        &mut bytes,
        u64::try_from(bss_bytes).expect("bss size overflow"),
    );

    write_symbols(&mut bytes, input.object);
    write_relocations(&mut bytes, input.object, input.relocations);

    bytes.extend(input.text_bytes);
    bytes.extend(input.data_bytes);

    ObjectContainerOutput {
        bytes,
        file_name: "omega-backend.omgobj".to_owned(),
        format: "omega-backend-object-container".to_owned(),
        text_bytes: input.text_bytes.len(),
        data_bytes: input.data_bytes.len(),
        bss_bytes,
        symbols: input.object.layout.symbols.len(),
        relocations: input.relocations.record_count(),
    }
}

#[cfg(test)]
mod tests {
    use super::{OBJECT_CONTAINER_VERSION, ObjectContainerInput, emit_omega_object_container};
    use crate::{ObjectPlan, RelocationPlan};
    use omega_target::NativeTarget;

    #[test]
    fn object_container_version_covers_semantic_edge_relocation_origins() {
        let target = NativeTarget::linux_arm64();
        let object = ObjectPlan::with_capacity(target, 0, 0);
        let relocations = RelocationPlan::with_target(target);
        let output = emit_omega_object_container(ObjectContainerInput {
            target,
            object: &object,
            relocations: &relocations,
            text_bytes: &[],
            data_bytes: &[],
        });

        assert_eq!(&output.bytes[..8], b"OMGOBJ\0\0");
        assert_eq!(
            &output.bytes[8..12],
            &OBJECT_CONTAINER_VERSION.to_le_bytes()
        );
    }
}
