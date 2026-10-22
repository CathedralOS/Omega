//! Shared geometry/supply decision and rejection encoding for placement.
//!
//! View and Initialize have different effects and custody, but decide the
//! same range, supply and alignment obligations in the same order. Keeping
//! the decision here prevents their public rejection semantics from drifting.
use super::{
    Builder, ImageHome, LegalizedScalarInstruction, PlaceId, SelectedInstructionKind,
    VirtualRegisterId, fragmented_scalar,
};
use crate::SelectedInstructionError;

pub(super) fn write_word(
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
pub(super) struct PlacementDecision {
    pub(super) ready: VirtualRegisterId,
    pub(super) outcome_tag: VirtualRegisterId,
    pub(super) failure_tag: VirtualRegisterId,
    pub(super) copy_length: VirtualRegisterId,
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
) -> Result<PlacementDecision, SelectedInstructionError> {
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
    Ok(PlacementDecision {
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

/// Populate only the selected rejection reason. Ready publication belongs to
/// the caller and happens after its operation-specific pointer or byte effect.
pub(super) fn write_failure(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    home: ImageHome,
    layout: &super::result_layout::ResultLayout,
    decision: &PlacementDecision,
    range: [VirtualRegisterId; 2],
    alignment: [VirtualRegisterId; 2],
) -> Result<(), SelectedInstructionError> {
    write_word(
        builder,
        row,
        home,
        layout.failure_tag,
        4,
        decision.failure_tag,
    )?;
    for (ordinal, coordinates, words) in
        [(0, layout.range, range), (1, layout.alignment, alignment)]
    {
        let ordinal = fragmented_scalar::constant(builder, row, home.place, ordinal)?;
        let selected = compare(
            builder,
            row,
            home.place,
            decision.failure_tag,
            ordinal,
            SelectedInstructionKind::MaterializeBooleanEqual,
        )?;
        super::super::super::operation_control::when_nonzero(builder, row, selected, |builder| {
            for (coordinate, word) in coordinates.into_iter().zip(words) {
                write_word(builder, row, home, coordinate, 8, word)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}
