//! The Place-pair copy MATERIALIZER (codegen cleanup Phase 6, the Copy*
//! pilot): one routine that walks a source and a target [`Place`] and emits
//! the address computation plus the chunked copy that the per-variant Copy
//! encoders each hand-spelled. Register discipline matches those encoders
//! exactly -- r14 carries the source address, r15 the target address, rax is
//! the chunk scratch -- and a materialized place emits BYTE-FOR-BYTE what
//! the corresponding retired encoder emitted, so the relocation walker's
//! per-kind byte math is unchanged while variants delegate here one by one.
//!
//! Base note: each place's base is `mov rXX, imm64(0)`; WHICH region that
//! placeholder relocates to still lives on the operation kind and is patched
//! by the instruction-record walker (omega-relocations). The walker adopts
//! the place's own region when the kinds themselves collapse -- until then
//! `Place::region` is documentation, not behavior, on this path.
//!
//! Legalization discipline: shapes this materializer cannot yet address
//! (runtime scaled indices) REFUSE LOUDLY -- callers keep routing those
//! through their dedicated encoders until the indexed rung lands.

use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{Place, PlaceStep};

#[derive(Clone, Copy)]
enum AddressRegister {
    /// The source-address register (r14).
    Source,
    /// The target-address register (r15).
    Target,
}

/// Emit the address computation for `place` into the chosen register and
/// return the RESIDUAL displacement: the trailing run of constant offsets is
/// folded into the subsequent load/store displacements instead of being
/// added to the register, exactly as the retired per-variant encoders did.
fn materialize_place_address(
    bytes: &mut Vec<u8>,
    place: &Place,
    register: AddressRegister,
) -> Result<usize, Diagnostic> {
    match register {
        AddressRegister::Source => super::append_mov_r14_imm64(bytes, 0),
        AddressRegister::Target => super::append_mov_r15_imm64(bytes, 0),
    }
    let mut displacement = 0usize;
    for step in place.steps() {
        match step {
            PlaceStep::ConstOffset(offset) => displacement += offset,
            PlaceStep::Deref => {
                match register {
                    AddressRegister::Source => {
                        super::append_load_r14_from_r14(bytes, displacement)?
                    }
                    AddressRegister::Target => {
                        super::append_load_r15_from_r15(bytes, displacement)?
                    }
                }
                displacement = 0;
            }
            PlaceStep::ScaledIndex { .. } => {
                return Err(Diagnostic::error(
                    "place materializer: runtime-indexed place steps are not lowered yet -- \
                     this shape still routes through its dedicated indexed encoder \
                     (the Copy* pilot's indexed rung)",
                ));
            }
        }
    }
    Ok(displacement)
}

/// Copy `byte_count` bytes from `source` to `target`: materialize both
/// addresses, then move the bytes in aligned 8/4/1 chunks through rax.
pub fn encode_place_copy(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();
    let source_displacement =
        materialize_place_address(&mut bytes, source, AddressRegister::Source)?;
    let target_displacement =
        materialize_place_address(&mut bytes, target, AddressRegister::Target)?;
    super::for_each_runtime_copy_chunk(
        source_displacement,
        target_displacement,
        byte_count,
        |offset, chunk_size| {
            super::append_load_rax_from_r14(&mut bytes, source_displacement + offset, chunk_size)?;
            super::append_store_rax_to_r15(&mut bytes, target_displacement + offset, chunk_size)?;
            Ok(())
        },
    )?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target_operations::RuntimeStorageRegion;

    /// The delegated plain copy must be byte-for-byte the retired encoder's
    /// output: base movs then aligned chunks with folded displacements.
    #[test]
    fn plain_copy_matches_the_retired_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 16);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 40);
        let bytes = encode_place_copy(&source, &target, 16).expect("const path encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0);
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::for_each_runtime_copy_chunk(16, 40, 16, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 16 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 40 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// A deref step emits the pointer load exactly where the retired pointee
    /// encoders placed it, and the post-deref offset folds into the chunks.
    #[test]
    fn target_deref_matches_the_pointee_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 8);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(4))
            .unwrap();
        let bytes = encode_place_copy(&source, &target, 4).expect("deref path encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0);
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_load_r15_from_r15(&mut expected, 32).expect("pointer load");
        super::super::for_each_runtime_copy_chunk(8, 4, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 8 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 4 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// Runtime-indexed steps refuse loudly until the indexed rung lands.
    #[test]
    fn scaled_index_refuses_loudly() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 0)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 8,
                element_byte_size: 4,
            })
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 64);
        assert!(encode_place_copy(&source, &target, 4).is_err());
    }
}
