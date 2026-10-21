//! Initialization decides its outcome before committing staged bytes.
//!
//! A zero-length CopyBytes touches neither address. Materializing the decision
//! and selecting zero or the complete image length therefore gives rejection
//! no backing effect without introducing an ISA-specific branch expansion.
//! The source image must already be completely encoded in disjoint scratch.
//! These predicates do not admit a profile or mint storage authority: the
//! lifecycle's selected-provider/backing join must supply that independently.

use super::{
    Builder, ImageDirection, ImageHome, LegalizedScalarInstruction, PlaceId,
    SelectedInstructionKind, StructuralTypeDeclaration, TerminalPlacementLifecyclePlan,
    VirtualRegisterId, coordinate, failure, fragmented_scalar, operand_indices, owner_word,
};
use crate::SelectedInstructionError;

mod result_layout;

/// Realize the exact private Initialize operation. Source admission still owns
/// selected-profile provenance, encoding obligations and backing authority.
/// All input transport and rejection payloads are prepared privately; only a
/// successful complete copy can precede publication of the Ready case.
pub(super) fn emit(
    source: &super::LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    plan: &TerminalPlacementLifecyclePlan,
) -> Result<(), SelectedInstructionError> {
    use super::super::super::operation_control;
    use super::super::{leaf_copy, record};
    use super::resource_profile::compatibility;
    use abstract_operations::PlacementLifecycleOperands;
    use terminal_psi::TerminalPlacementLifecycleExecution;
    let super::LegalizedScalarInstructionKind::PlacementLifecycle {
        operands:
            PlacementLifecycleOperands::Initialize {
                storage,
                value,
                profile,
            },
        indices,
        result,
        ..
    } = &row.kind
    else {
        return Err(failure(
            "initialization emitter requires its three retained operands",
        ));
    };
    let TerminalPlacementLifecycleExecution::CompilerPrimitive {
        profile_type: Some(profile_type),
        ..
    } = plan.execution
    else {
        return Err(failure(
            "initialization lost its admitted core profile type",
        ));
    };
    let types = &source
        .structural
        .as_ref()
        .ok_or_else(|| failure("initialization lost its structural signature"))?
        .structural_types;
    let layout = result_layout::ResultLayout::derive(types, result.structural_type, plan)?;
    let mut remaining = indices.as_slice();
    let storage_indices = operand_indices(storage, &mut remaining)?;
    let value_indices = operand_indices(value, &mut remaining)?;
    let profile_indices = operand_indices(profile, &mut remaining)?;
    if !remaining.is_empty() {
        return Err(failure(
            "initialization retained selectors outside its operand projections",
        ));
    }
    let result_pointer = record::initialize_storage(row, builder, result.place, layout.shape)?;
    let result_home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place(&[]),
        place: result.place,
        pointer: result_pointer,
        byte_offset: 0,
    };
    for (argument, selectors, field) in [
        (storage, storage_indices, layout.returned_owner),
        (value, value_indices, layout.returned_value),
    ] {
        leaf_copy::copy_operand(
            source,
            row,
            builder,
            argument,
            selectors,
            leaf_copy::CopyEnd {
                place: result.place,
                pointer: result_pointer,
                byte_offset: field.byte_offset,
                elements: &[],
            },
            field.shape,
        )?;
    }
    let owner_home = ImageHome {
        byte_offset: layout.returned_owner.byte_offset,
        ..result_home
    };
    let base = owner_word(types, row, builder, plan, owner_home, plan.storage.base)?;
    let available = owner_word(types, row, builder, plan, owner_home, plan.storage.length)?;
    let projected = leaf_copy::project_operand(source, row, builder, profile, profile_indices)?;
    let profile_home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place(projected.indices),
        place: profile.place,
        pointer: projected
            .pointer
            .ok_or_else(|| failure("borrowed initialization profile has no addressable home"))?,
        byte_offset: projected.byte_offset,
    };
    let image_length = plan
        .placement
        .layout()
        .size
        .ok_or_else(|| failure("initialization requires a fixed complete image"))?;
    let scratch_bytes = super::offset(compatibility::COMPATIBILITY_SCRATCH_BYTES, image_length)?;
    let scratch = ImageHome::scratch(builder, row, result.place, scratch_bytes)?;
    let assessment = compatibility::assess(
        types,
        profile_type,
        builder,
        row,
        profile_home,
        scratch,
        available,
        &plan.placement,
        plan.operation,
    )?;
    operation_control::require_nonzero(builder, row, assessment.well_formed)?;
    let staged = ImageHome {
        byte_offset: compatibility::COMPATIBILITY_SCRATCH_BYTES,
        ..scratch
    };
    let value_home = ImageHome {
        byte_offset: layout.returned_value.byte_offset,
        ..result_home
    };
    stage_image(types, builder, row, plan, value_home, staged)?;
    let decision = decide(
        builder,
        row,
        result.place,
        base,
        available,
        image_length,
        assessment.modulus,
        assessment.residue,
        assessment.compatible,
    )?;

    // Fill only the selected reason's payload. IncompatibleSupply has none;
    // a success subsequently overwrites the owner alternative and outer tag.
    write_word(
        builder,
        row,
        result_home,
        layout.failure_tag,
        4,
        decision.failure_tag,
    )?;
    let required = fragmented_scalar::constant(builder, row, result.place, image_length)?;
    for (ordinal, coordinates, words) in [
        (0, layout.range, [required, available]),
        (
            1,
            layout.alignment,
            [assessment.modulus, assessment.residue],
        ),
    ] {
        let ordinal = fragmented_scalar::constant(builder, row, result.place, ordinal)?;
        let selected = compare(
            builder,
            row,
            result.place,
            decision.failure_tag,
            ordinal,
            SelectedInstructionKind::MaterializeBooleanEqual,
        )?;
        operation_control::when_nonzero(builder, row, selected, |builder| {
            for (coordinate, word) in coordinates.into_iter().zip(words) {
                write_word(builder, row, result_home, coordinate, 8, word)?;
            }
            Ok(())
        })?;
    }
    let image_pointer =
        super::super::transport_register(builder, result.place, staged.byte_offset)?;
    builder.emit_address_offset(
        staged.byte_offset,
        staged.pointer,
        image_pointer,
        super::super::provenance(row),
    )?;
    staged.storage.footprint(
        builder,
        row,
        staged.place,
        staged.byte_offset,
        coordinate(image_length)?,
        super::super::runtime_address::Written::None,
    )?;
    fragmented_scalar::StorageAccess::Backing.footprint(
        builder,
        row,
        storage.place,
        0,
        coordinate(image_length)?,
        super::super::runtime_address::Written::Structural,
    )?;
    commit(builder, row, storage.place, image_pointer, base, &decision)?;
    operation_control::when_nonzero(builder, row, decision.ready, |builder| {
        leaf_copy::copy_operand(
            source,
            row,
            builder,
            storage,
            storage_indices,
            leaf_copy::CopyEnd {
                place: result.place,
                pointer: result_pointer,
                byte_offset: layout.ready_owner.byte_offset,
                elements: &[],
            },
            layout.ready_owner.shape,
        )
    })?;
    write_word(
        builder,
        row,
        result_home,
        layout.tag,
        4,
        decision.outcome_tag,
    )?;
    builder
        .transport
        .pointers
        .push((result.place, result_pointer));
    Ok(())
}

fn write_word(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    home: ImageHome,
    byte_offset: u32,
    width: u32,
    value: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    super::ScalarImageCodec::Bytes {
        byte_offset,
        width,
        signed: false,
    }
    .write(builder, row, home, value)
}

/// Physical result coordinates, never an admitted source-level token.
pub(super) struct InitializationDecision {
    pub(super) ready: VirtualRegisterId,
    pub(super) outcome_tag: VirtualRegisterId,
    pub(super) failure_tag: VirtualRegisterId,
    pub(super) copy_length: VirtualRegisterId,
}

/// Initialize every staged byte, then encode the complete value. Both homes
/// belong to the caller: scratch is disjoint from the still-live input and
/// provider backing. Padding and inaccessible fields are part of this image;
/// the allocation tail is not. This prepares bytes, not resident custody.
pub(super) fn stage_image(
    declarations: &[StructuralTypeDeclaration],
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    plan: &TerminalPlacementLifecyclePlan,
    value: ImageHome,
    staged: ImageHome,
) -> Result<(), SelectedInstructionError> {
    let byte_count = coordinate(
        plan.placement
            .layout()
            .size
            .ok_or_else(|| failure("resident initialization requires a fixed complete image"))?,
    )?;
    // Resolve the complete scratch interval before emitting any writes. A
    // zero-width image needs no address arithmetic or memory instruction.
    if byte_count != 0 {
        super::offset(staged.byte_offset, u64::from(byte_count))?;
        let pointer = super::super::transport_register(builder, staged.place, staged.byte_offset)?;
        builder.emit_address_offset(
            staged.byte_offset,
            staged.pointer,
            pointer,
            super::super::provenance(row),
        )?;
        let count = fragmented_scalar::constant(builder, row, staged.place, u64::from(byte_count))?;
        let cursor = super::super::transport_register(builder, staged.place, staged.byte_offset)?;
        staged.storage.footprint(
            builder,
            row,
            staged.place,
            staged.byte_offset,
            byte_count,
            super::super::runtime_address::Written::Structural,
        )?;
        builder.emit(
            SelectedInstructionKind::ZeroBytes,
            builder
                .constraints
                .keys
                .zero_bytes
                .ok_or_else(|| failure("resident image target lacks exact byte zeroing"))?,
            &[pointer, count, cursor],
            super::super::provenance(row),
        )?;
    }
    super::transfer_image(
        declarations,
        row,
        builder,
        plan,
        staged,
        value,
        ImageDirection::Encode,
    )
}

/// `supply_compatible` is a Boolean produced from the complete immutable
/// selected profile and actual backing. It is not an ordinary caller flag.
/// Rejection order matches lifecycle execution: range, supply, alignment.
#[allow(clippy::too_many_arguments)]
pub(super) fn decide(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    owner: PlaceId,
    base: VirtualRegisterId,
    available: VirtualRegisterId,
    image_length: u64,
    modulus: VirtualRegisterId,
    residue: VirtualRegisterId,
    supply_compatible: VirtualRegisterId,
) -> Result<InitializationDecision, SelectedInstructionError> {
    let required = fragmented_scalar::constant(builder, row, owner, image_length)?;
    let range = compare(
        builder,
        row,
        owner,
        required,
        available,
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
    )?;
    let one = fragmented_scalar::constant(builder, row, owner, 1)?;
    let zero = fragmented_scalar::constant(builder, row, owner, 0)?;
    let modulus_mask = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::WrappingSubtractI64,
        modulus,
        one,
    )?;
    // Congruence comes from the complete runtime compatibility calculation,
    // not a compile-time guess at provider alignment. Even malformed physical
    // inputs must not accidentally enable a write; the lifecycle caller owns
    // invalid-assertion termination separately from ordinary rejection.
    let present = compare(
        builder,
        row,
        owner,
        one,
        modulus,
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
    )?;
    let low_bits = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::BitwiseAndI64,
        modulus,
        modulus_mask,
    )?;
    let power_of_two = compare(
        builder,
        row,
        owner,
        low_bits,
        zero,
        SelectedInstructionKind::MaterializeBooleanEqual,
    )?;
    let bounded_residue = compare(
        builder,
        row,
        owner,
        residue,
        modulus_mask,
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
    )?;
    let valid_modulus = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::BitwiseAndI64,
        present,
        power_of_two,
    )?;
    let valid_congruence = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::BitwiseAndI64,
        valid_modulus,
        bounded_residue,
    )?;
    let actual_residue = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::BitwiseAndI64,
        base,
        modulus_mask,
    )?;
    let aligned = compare(
        builder,
        row,
        owner,
        actual_residue,
        residue,
        SelectedInstructionKind::MaterializeBooleanEqual,
    )?;
    // Normalize the predicate at this boundary too: only exact Boolean true
    // enables a write. No arbitrary nonzero register may scale the byte count.
    let supplied = compare(
        builder,
        row,
        owner,
        supply_compatible,
        one,
        SelectedInstructionKind::MaterializeBooleanEqual,
    )?;
    let supplied = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::BitwiseAndI64,
        supplied,
        valid_congruence,
    )?;
    let range_and_supply = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::BitwiseAndI64,
        range,
        supplied,
    )?;
    let ready = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::BitwiseAndI64,
        range_and_supply,
        aligned,
    )?;
    let outcome_tag = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::BitwiseXorI64,
        one,
        ready,
    )?;
    // RangeTooSmall=0, Misaligned=1, IncompatibleSupply=2. The failure
    // payload is unobservable on Ready, but its bits remain deterministic.
    let two = fragmented_scalar::constant(builder, row, owner, 2)?;
    let supply_failure = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::WrappingSubtractI64,
        two,
        supplied,
    )?;
    let failure_tag = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::WrappingMultiplyI64,
        range,
        supply_failure,
    )?;
    let copy_length = fragmented_scalar::binary(
        builder,
        row,
        owner,
        SelectedInstructionKind::WrappingMultiplyI64,
        required,
        ready,
    )?;
    Ok(InitializationDecision {
        ready,
        outcome_tag,
        failure_tag,
        copy_length,
    })
}

pub(super) fn compare(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    owner: PlaceId,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    result_kind: SelectedInstructionKind,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let output = super::super::transport_register(builder, owner, 0)?;
    builder.emit(
        SelectedInstructionKind::CompareI64,
        builder.constraints.keys.compare_i64,
        &[left, right],
        super::super::provenance(row),
    )?;
    builder.emit(
        result_kind,
        builder.constraints.keys.materialize_boolean,
        &[output],
        super::super::provenance(row),
    )?;
    Ok(output)
}

/// Commit one complete staged image, or no bytes on rejection. The owner
/// records the exact scratch-read and possible backing-write footprint before
/// calling; this instruction neither selects a profile nor creates a claim.
pub(super) fn commit(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    owner: PlaceId,
    staged: VirtualRegisterId,
    backing: VirtualRegisterId,
    decision: &InitializationDecision,
) -> Result<(), SelectedInstructionError> {
    let cursor = super::super::transport_register(builder, owner, 0)?;
    let byte = super::super::transport_register(builder, owner, 0)?;
    builder.emit(
        SelectedInstructionKind::CopyBytes,
        builder
            .constraints
            .keys
            .copy_bytes
            .ok_or_else(|| failure("resident initialization target lacks exact byte copy"))?,
        &[staged, backing, decision.copy_length, cursor, byte],
        super::super::provenance(row),
    )
}
