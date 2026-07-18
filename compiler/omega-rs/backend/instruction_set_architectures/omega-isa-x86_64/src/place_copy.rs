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
//! Index discipline (the ScaledIndex rung): a place may carry AT MOST ONE
//! runtime scaled index, its slot readable from the place's own base region
//! -- the index loads into r11 (32-bit, zero-extended) and scales IMMEDIATELY
//! AFTER the base materializes and BEFORE any deref consumes the base, then
//! `add reg, r11` fires at the step's position in the walk. On the
//! shared-base path an index is legal only on a side that DEREFS (a direct
//! side's add would mutate the base the other side still needs), and only
//! one side may be indexed (r11 is the single index scratch). Everything
//! else REFUSES LOUDLY -- legalization, not silent truncation.

use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{Place, PlaceStep};

/// Which side of the copy a base-materialization relocation site belongs to.
/// The relocation walker maps a side to that place's own region -- this is
/// how `CopyPlaces` patches BY PLACE REGION instead of by per-kind offset
/// functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaceCopySide {
    #[default]
    Source,
    Target,
    /// The source place's ScaledIndex slot base (cross-region index): the
    /// walker patches it from the step's own `index_region`.
    SourceIndex,
    /// The target place's ScaledIndex slot base.
    TargetIndex,
}

/// Four covers every emitted shape: two bases today, plus room for the
/// machine-indexed rung's separate index-base materializations.
pub const PLACE_COPY_MAX_SITES: usize = 4;

/// The base-materialization relocation sites of one place copy: the byte
/// position of each `mov r??, imm64(0)` placeholder WITHIN the encoded
/// instruction, tagged with the side whose region patches it. Recorded by
/// the SAME walk that emits the bytes -- lockstep by construction, never a
/// hand-maintained offset constant.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlaceCopySites {
    sites: [(u32, PlaceCopySide); PLACE_COPY_MAX_SITES],
    len: u8,
}

impl PlaceCopySites {
    fn record(&mut self, byte_offset: usize, side: PlaceCopySide) {
        debug_assert!(usize::from(self.len) < PLACE_COPY_MAX_SITES);
        if usize::from(self.len) < PLACE_COPY_MAX_SITES {
            self.sites[usize::from(self.len)] = (byte_offset as u32, side);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, PlaceCopySide)> + '_ {
        self.sites[..usize::from(self.len)]
            .iter()
            .map(|(offset, side)| (*offset as usize, *side))
    }
}

#[derive(Clone, Copy)]
enum AddressRegister {
    /// The source-address register (r14).
    Source,
    /// The target-address register (r15).
    Target,
}

impl AddressRegister {
    fn side(self) -> PlaceCopySide {
        match self {
            AddressRegister::Source => PlaceCopySide::Source,
            AddressRegister::Target => PlaceCopySide::Target,
        }
    }
}

/// Emit the address computation for `place` into the chosen register and
/// return the RESIDUAL displacement: the trailing run of constant offsets is
/// folded into the subsequent load/store displacements instead of being
/// added to the register, exactly as the retired per-variant encoders did.
fn materialize_place_address(
    bytes: &mut Vec<u8>,
    sites: &mut PlaceCopySites,
    place: &Place,
    register: AddressRegister,
) -> Result<usize, Diagnostic> {
    sites.record(bytes.len(), register.side());
    match register {
        AddressRegister::Source => super::append_mov_r14_imm64(bytes, 0),
        AddressRegister::Target => super::append_mov_r15_imm64(bytes, 0),
    }
    // The index (at most one) loads and scales BEFORE any deref consumes the
    // base register its slot is addressed from.
    let index_side = match register {
        AddressRegister::Source => PlaceCopySide::SourceIndex,
        AddressRegister::Target => PlaceCopySide::TargetIndex,
    };
    prepare_place_index(bytes, sites, place, register, index_side)?;
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
            PlaceStep::ScaledIndex { .. } => append_scaled_index_add(bytes, register),
        }
    }
    Ok(displacement)
}

/// Pre-load the place's runtime index (if any) into r11 and scale it by the
/// element size. Refuses more than one index per place: r11 is the single
/// index scratch. A SAME-region index reads through the place's own base
/// register; a CROSS-region index first materializes the index region's
/// base into r11 itself (a recorded relocation site), then loads through it
/// -- no extra scratch register enters the discipline.
fn prepare_place_index(
    bytes: &mut Vec<u8>,
    sites: &mut PlaceCopySites,
    place: &Place,
    base_register: AddressRegister,
    index_side: PlaceCopySide,
) -> Result<(), Diagnostic> {
    let mut indices = place.steps().iter().filter_map(|step| match step {
        PlaceStep::ScaledIndex {
            index_region,
            index_offset,
            element_byte_size,
        } => Some((*index_region, *index_offset, *element_byte_size)),
        _ => None,
    });
    let Some((index_region, index_offset, element_byte_size)) = indices.next() else {
        return Ok(());
    };
    if indices.next().is_some() {
        return Err(Diagnostic::error(
            "place materializer: at most one runtime scaled index per place \
             (r11 is the single index scratch)",
        ));
    }
    if index_region == place.region {
        match base_register {
            AddressRegister::Source => super::append_load_r11_from_r14(bytes, index_offset)?,
            AddressRegister::Target => super::append_load_index_r11_from_r15(bytes, index_offset)?,
        }
    } else {
        sites.record(bytes.len(), index_side);
        super::append_mov_r11_imm64(bytes, 0);
        super::append_load_r11_from_r11(bytes, index_offset)?;
    }
    super::append_imul_r11_imm32(bytes, super::element_scale(element_byte_size)?);
    Ok(())
}

fn append_scaled_index_add(bytes: &mut Vec<u8>, register: AddressRegister) {
    match register {
        AddressRegister::Source => super::append_add_r14_r11(bytes),
        AddressRegister::Target => super::append_add_r15_r11(bytes),
    }
}

/// Copy `byte_count` bytes from `source` to `target`: materialize both
/// addresses, then move the bytes in aligned 8/4/1 chunks through rax.
pub fn encode_place_copy(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_place_copy_with_sites(source, target, byte_count).map(|(bytes, _)| bytes)
}

fn encode_place_copy_with_sites(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let source_displacement =
        materialize_place_address(&mut bytes, &mut sites, source, AddressRegister::Source)?;
    let target_displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    append_copy_chunks(
        &mut bytes,
        source_displacement,
        target_displacement,
        byte_count,
    )?;
    Ok((bytes, sites))
}

/// The `CopyPlaces` entry: ONE routine that picks the emission shape from the
/// place pair itself -- shared-base when both places root in the SAME region
/// and a side derefs (the shape every retired same-region indexed/pointee
/// encoder hand-spelled), two-base otherwise (including same-region direct
/// pairs, which the retired plain copy materialized as two identical
/// patched bases). Returns the bytes AND the base relocation sites recorded
/// by the same walk; the relocation walker patches each site from the
/// corresponding place's own region.
pub fn encode_copy_places(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    if source.region == target.region && (place_derefs(source) || place_derefs(target)) {
        encode_place_copy_shared_base_with_sites(source, target, byte_count)
    } else {
        encode_place_copy_with_sites(source, target, byte_count)
    }
}

/// The SHARED-BASE copy: both places root in the SAME region, so ONE base
/// materialization (into r15) serves both -- each place's FIRST deref loads
/// its own address register THROUGH the shared base, source before target
/// (the source's pointer must be read before the target's deref consumes
/// r15). Requires the source to start with a deref (after any const prefix);
/// the target may be a pure-const path (r15 stays the base) or start with a
/// deref of its own. This is the one-relocation shape the retired same-region
/// indexed/pointee encoders hand-spelled; the region stays documentation on
/// the transitional path (callers pick this entry where the retired encoder
/// shared its base, so walker byte math is unchanged).
pub fn encode_place_copy_shared_base(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_place_copy_shared_base_with_sites(source, target, byte_count).map(|(bytes, _)| bytes)
}

fn encode_place_copy_shared_base_with_sites(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let source_derefs = place_derefs(source);
    let target_derefs = place_derefs(target);
    let source_indexed = place_has_index(source);
    let target_indexed = place_has_index(target);
    if source_indexed && target_indexed {
        return Err(Diagnostic::error(
            "shared-base place copy: only one side may carry a runtime index \
             (r11 is the single index scratch)",
        ));
    }
    if (source_indexed && !source_derefs) || (target_indexed && !target_derefs) {
        return Err(Diagnostic::error(
            "shared-base place copy: a runtime index is only legal on a \
             dereferencing side (an index add on a direct side would mutate the \
             shared base) -- route this pair through encode_place_copy",
        ));
    }

    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    if source_derefs {
        // Base lives in r15 (the target register); the first source deref
        // hops its address to r14 BEFORE any target deref consumes r15.
        // The single base serves BOTH places (same region by precondition);
        // the site carries the register's own side.
        sites.record(bytes.len(), PlaceCopySide::Target);
        super::append_mov_r15_imm64(&mut bytes, 0);
        prepare_place_index(
            &mut bytes,
            &mut sites,
            if source_indexed { source } else { target },
            AddressRegister::Target,
            if source_indexed {
                PlaceCopySide::SourceIndex
            } else {
                PlaceCopySide::TargetIndex
            },
        )?;
        let source_displacement =
            walk_hopping_side(&mut bytes, source, HopDirection::BaseR15SourceHops)?;
        let target_displacement = walk_base_side(&mut bytes, target, AddressRegister::Target)?;
        append_copy_chunks(
            &mut bytes,
            source_displacement,
            target_displacement,
            byte_count,
        )?;
        Ok((bytes, sites))
    } else if target_derefs {
        // The mirror: the source is direct, so the base lives in r14 (the
        // source register) and the first target deref hops to r15.
        sites.record(bytes.len(), PlaceCopySide::Source);
        super::append_mov_r14_imm64(&mut bytes, 0);
        prepare_place_index(
            &mut bytes,
            &mut sites,
            target,
            AddressRegister::Source,
            PlaceCopySide::TargetIndex,
        )?;
        let target_displacement =
            walk_hopping_side(&mut bytes, target, HopDirection::BaseR14TargetHops)?;
        let source_displacement = walk_base_side(&mut bytes, source, AddressRegister::Source)?;
        append_copy_chunks(
            &mut bytes,
            source_displacement,
            target_displacement,
            byte_count,
        )?;
        Ok((bytes, sites))
    } else {
        Err(Diagnostic::error(
            "shared-base place copy requires a dereferencing side -- \
             a direct pair routes through encode_place_copy",
        ))
    }
}

fn place_derefs(place: &Place) -> bool {
    place
        .steps()
        .iter()
        .any(|step| matches!(step, PlaceStep::Deref))
}

fn place_has_index(place: &Place) -> bool {
    place
        .steps()
        .iter()
        .any(|step| matches!(step, PlaceStep::ScaledIndex { .. }))
}

#[derive(Clone, Copy)]
enum HopDirection {
    /// The shared base is r15; the hopping side address lands in r14.
    BaseR15SourceHops,
    /// The shared base is r14; the hopping side address lands in r15.
    BaseR14TargetHops,
}

/// Walk the side whose first deref HOPS off the shared base into its own
/// register; subsequent steps continue there. Returns the residual
/// displacement. An index add fires only after the hop (enforced by the
/// dereferencing-side check above).
fn walk_hopping_side(
    bytes: &mut Vec<u8>,
    place: &Place,
    direction: HopDirection,
) -> Result<usize, Diagnostic> {
    let mut steps = place.steps().iter();
    let mut prefix = 0usize;
    loop {
        match steps.next() {
            Some(PlaceStep::ConstOffset(offset)) => prefix += offset,
            Some(PlaceStep::Deref) => break,
            Some(PlaceStep::ScaledIndex { .. }) => {
                return Err(Diagnostic::error(
                    "shared-base place copy: a runtime index cannot precede the \
                     hopping deref (the add would target the shared base)",
                ));
            }
            None => unreachable!("walk_hopping_side requires a dereferencing place"),
        }
    }
    match direction {
        HopDirection::BaseR15SourceHops => super::append_load_r14_from_r15(bytes, prefix)?,
        HopDirection::BaseR14TargetHops => super::append_load_r15_from_r14(bytes, prefix)?,
    }
    let own_register = match direction {
        HopDirection::BaseR15SourceHops => AddressRegister::Source,
        HopDirection::BaseR14TargetHops => AddressRegister::Target,
    };
    let mut displacement = 0usize;
    for step in steps {
        match step {
            PlaceStep::ConstOffset(offset) => displacement += offset,
            PlaceStep::Deref => {
                match own_register {
                    AddressRegister::Source => {
                        super::append_load_r14_from_r14(bytes, displacement)?
                    }
                    AddressRegister::Target => {
                        super::append_load_r15_from_r15(bytes, displacement)?
                    }
                }
                displacement = 0;
            }
            PlaceStep::ScaledIndex { .. } => append_scaled_index_add(bytes, own_register),
        }
    }
    Ok(displacement)
}

/// Walk the side that stays ON the shared base register (its derefs, if any,
/// consume the base in place -- legal because the hopping side already left).
fn walk_base_side(
    bytes: &mut Vec<u8>,
    place: &Place,
    register: AddressRegister,
) -> Result<usize, Diagnostic> {
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
            PlaceStep::ScaledIndex { .. } => append_scaled_index_add(bytes, register),
        }
    }
    Ok(displacement)
}

fn append_copy_chunks(
    bytes: &mut Vec<u8>,
    source_displacement: usize,
    target_displacement: usize,
    byte_count: usize,
) -> Result<(), Diagnostic> {
    super::for_each_runtime_copy_chunk(
        source_displacement,
        target_displacement,
        byte_count,
        |offset, chunk_size| {
            super::append_load_rax_from_r14(bytes, source_displacement + offset, chunk_size)?;
            super::append_store_rax_to_r15(bytes, target_displacement + offset, chunk_size)?;
            Ok(())
        },
    )
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

    /// The shared-base shape with a pure-const target must be byte-for-byte
    /// the retired fixed-indexed-to-frame encoder: one base mov, the
    /// descriptor deref hops the source to r14, chunks fold the offsets.
    #[test]
    fn shared_base_const_target_matches_the_fixed_indexed_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(12))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 96);
        let bytes = encode_place_copy_shared_base(&source, &target, 8).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_load_r14_from_r15(&mut expected, 48).expect("deref");
        super::super::for_each_runtime_copy_chunk(12, 96, 8, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 12 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 96 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// The shared-base shape with BOTH sides dereferencing must order the
    /// source's pointer read BEFORE the target's deref consumes r15 -- the
    /// retired fixed-indexed-to-pointee layout.
    #[test]
    fn shared_base_double_deref_matches_the_pointee_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(12))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 72)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(4))
            .unwrap();
        let bytes = encode_place_copy_shared_base(&source, &target, 4).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_load_r14_from_r15(&mut expected, 48).expect("source deref");
        super::super::append_load_r15_from_r15(&mut expected, 72).expect("target deref");
        super::super::for_each_runtime_copy_chunk(12, 4, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 12 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 4 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// A direct (deref-free) source is not a shared-base shape -- the entry
    /// refuses instead of silently emitting a wrong-base copy.
    #[test]
    fn shared_base_refuses_a_direct_source() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 8);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 16);
        assert!(encode_place_copy_shared_base(&source, &target, 8).is_err());
    }

    /// The canonical two-base shape pins the target relocation position the
    /// walker's fixed-indexed-to-storage arm mirrors: source base (10) +
    /// descriptor deref (7) puts the target base mov at +17 (imm at +19).
    #[test]
    fn canonical_deref_source_puts_the_target_base_at_17() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 40)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(8))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::Machine, 64);
        let bytes = encode_place_copy(&source, &target, 8).expect("encodes");
        assert_eq!(super::super::FRAME_FIXED_INDEXED_COPY_TARGET_IMM_OFFSET, 17);
        // 49 BF = mov r15, imm64 at the pinned offset.
        assert_eq!(&bytes[17..19], &[0x49, 0xbf]);
    }

    /// The shared-base runtime-indexed source (the from_frame_indexed
    /// family): the index loads from the SHARED base and scales BEFORE the
    /// hopping deref, then adds onto the hopped source address.
    #[test]
    fn shared_base_indexed_source_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 56,
                element_byte_size: 4,
            })
            .unwrap()
            .with_step(PlaceStep::ConstOffset(8))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 96);
        let bytes = encode_place_copy_shared_base(&source, &target, 4).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_load_index_r11_from_r15(&mut expected, 56).expect("index");
        super::super::append_imul_r11_imm32(&mut expected, 4);
        super::super::append_load_r14_from_r15(&mut expected, 48).expect("hop");
        super::super::append_add_r14_r11(&mut expected);
        super::super::for_each_runtime_copy_chunk(8, 96, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 8 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 96 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// The mirror: a runtime-indexed TARGET (the to_frame_indexed write face
    /// the old product never built on x86_64) -- base in r14, the index loads
    /// from it, the target hops to r15 and adds the scaled index.
    #[test]
    fn shared_base_indexed_target_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 24);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 56,
                element_byte_size: 8,
            })
            .unwrap();
        let bytes = encode_place_copy_shared_base(&source, &target, 8).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0);
        super::super::append_load_r11_from_r14(&mut expected, 56).expect("index");
        super::super::append_imul_r11_imm32(&mut expected, 8);
        super::super::append_load_r15_from_r14(&mut expected, 48).expect("hop");
        super::super::append_add_r15_r11(&mut expected);
        super::super::for_each_runtime_copy_chunk(24, 0, 8, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 24 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// The CROSS-REGION index (rung 2c-vii, the machine-indexed family): a
    /// MACHINE-region array indexed by a FRAME-resident slot. r11 first
    /// materializes the index region's base (a recorded SourceIndex
    /// relocation site), then loads the index through itself -- no extra
    /// scratch register. The machine base has no deref (inline array), so
    /// the scaled add fires at the step's walk position.
    #[test]
    fn cross_region_index_materializes_its_own_base() {
        let source = Place::at(RuntimeStorageRegion::Machine, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 16,
                element_byte_size: 4,
            })
            .unwrap()
            .with_step(PlaceStep::ConstOffset(0))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 64);
        let (bytes, sites) = encode_copy_places(&source, &target, 4).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0); // machine base (Source site @0)
        let index_base_offset = expected.len();
        super::super::append_mov_r11_imm64(&mut expected, 0); // frame base for the index
        super::super::append_load_r11_from_r11(&mut expected, 16).expect("index");
        super::super::append_imul_r11_imm32(&mut expected, 4);
        super::super::append_add_r14_r11(&mut expected);
        let target_base_offset = expected.len();
        super::super::append_mov_r15_imm64(&mut expected, 0); // frame target base
        super::super::for_each_runtime_copy_chunk(32, 64, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 32 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 64 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);

        let recorded: Vec<(usize, PlaceCopySide)> = sites.iter().collect();
        assert_eq!(
            recorded,
            vec![
                (0, PlaceCopySide::Source),
                (index_base_offset, PlaceCopySide::SourceIndex),
                (target_base_offset, PlaceCopySide::Target),
            ]
        );
    }

    /// Index refusals: two indices on one place; both sides indexed; an index
    /// on a direct shared side.
    #[test]
    fn scaled_index_refusals() {
        let indexed = |offset: usize| {
            Place::at(RuntimeStorageRegion::RuntimeFrame, offset)
                .with_step(PlaceStep::Deref)
                .unwrap()
                .with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 8,
                    element_byte_size: 4,
                })
                .unwrap()
        };
        // Two indices on one place (two-base path).
        let double = indexed(0)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 16,
                element_byte_size: 4,
            })
            .unwrap();
        let plain = Place::at(RuntimeStorageRegion::Machine, 64);
        assert!(encode_place_copy(&double, &plain, 4).is_err());
        // Both sides indexed (shared base).
        assert!(encode_place_copy_shared_base(&indexed(0), &indexed(32), 4).is_err());
        // Index on a DIRECT side (shared base): would mutate the shared base.
        let direct_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 0)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 8,
                element_byte_size: 4,
            })
            .unwrap();
        assert!(encode_place_copy_shared_base(&direct_indexed, &indexed(32), 4).is_err());
    }
}
