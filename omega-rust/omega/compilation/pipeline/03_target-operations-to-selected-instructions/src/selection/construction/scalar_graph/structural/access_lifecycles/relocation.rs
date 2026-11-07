//! Move a checked run's occupied bytes, never its element cleanup obligations.
//! Both original owners are staged for rejection. Only the successful decision
//! enables the copy and publishes the swapped backing descriptors and count.

use super::super::super::operation_control;
use super::super::{leaf_copy, memory};
use super::outcome::{PlacementDecision, compare, write_word};
use super::resource_profile::compatibility::run::{self, RunGeometry, SupplyRole};
use super::*;
use selected_instructions::SelectedMemoryAccessRole;

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    plan: &TerminalAccessLifecyclePlan,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::AccessLifecycle {
        operands:
            AccessLifecycleOperands::Relocate {
                source: run_argument,
                destination,
                source_profile,
                destination_profile,
            },
        indices,
        result,
        ..
    } = &row.kind
    else {
        return Err(failure("relocation lost its four retained operands"));
    };
    let terminal_psi::TerminalAccessLifecycleExecution::CompilerPrimitive {
        profile_type: Some(profile_type),
        ..
    } = plan.execution
    else {
        return Err(failure("relocation lost its exact profile type"));
    };
    let terminal_psi::TerminalAccessOwner::Run {
        carrier,
        storage,
        count,
        ..
    } = plan.storage.owner
    else {
        return Err(failure("relocation requires a retained run owner"));
    };
    let types = &source
        .structural
        .as_ref()
        .ok_or_else(|| failure("relocation lost its structural signature"))?
        .structural_types;
    let layout = result_layout::ResultLayout::derive(types, result.structural_type, plan)?;
    let result_layout::ReturnedLayout::Owned {
        owner: returned_run,
        value: returned_destination,
    } = layout.returned
    else {
        return Err(failure("relocation rejection must return both owners"));
    };
    let vacated = layout
        .ready_vacated
        .ok_or_else(|| failure("relocation Ready lost its vacated backing"))?;
    let count_offset = layout
        .ready_count
        .ok_or_else(|| failure("relocation Ready lost its run count"))?;
    let mut remaining = indices.as_slice();
    let run_indices = operand_indices(run_argument, &mut remaining)?;
    let destination_indices = operand_indices(destination, &mut remaining)?;
    let source_profile_indices = operand_indices(source_profile, &mut remaining)?;
    let destination_profile_indices = operand_indices(destination_profile, &mut remaining)?;
    if !remaining.is_empty() {
        return Err(failure(
            "relocation retained selectors outside its operands",
        ));
    }
    let result_pointer = record::initialize_storage(row, builder, result.place, layout.shape)?;
    let result_home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place {
            indices: &[],
            byte_offset: 0,
        },
        place: result.place,
        pointer: result_pointer,
        byte_offset: 0,
    };
    for (argument, selectors, field) in [
        (run_argument, run_indices, returned_run),
        (destination, destination_indices, returned_destination),
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
    let StructuralTypeShape::Record { fields, .. } =
        &terminal_psi::find_structural_type(types, carrier)
            .ok_or_else(|| failure("relocation lost its run declaration"))?
            .shape
    else {
        return Err(failure("run is not a record"));
    };
    let backing_field = fields
        .iter()
        .find(|field| field.id == storage)
        .ok_or_else(|| failure("run storage field is missing"))?;
    let count_field = fields
        .iter()
        .find(|field| field.id == count)
        .ok_or_else(|| failure("run count field is missing"))?;
    let source_home = ImageHome {
        byte_offset: returned_run.byte_offset,
        ..result_home
    };
    let count =
        ScalarImageCodec::field(types, carrier, count_field)?.read(builder, row, source_home)?;
    let source_home = ImageHome {
        byte_offset: offset(
            returned_run.byte_offset,
            u64::from(field_region(types, carrier, backing_field)?.byte_offset),
        )?,
        ..result_home
    };
    let destination_home = ImageHome {
        byte_offset: returned_destination.byte_offset,
        ..result_home
    };
    let source_base = owner_word(types, row, builder, plan, source_home, plan.storage.base)?;
    let source_length = owner_word(types, row, builder, plan, source_home, plan.storage.length)?;
    let destination_base = owner_word(
        types,
        row,
        builder,
        plan,
        destination_home,
        plan.storage.base,
    )?;
    let destination_length = owner_word(
        types,
        row,
        builder,
        plan,
        destination_home,
        plan.storage.length,
    )?;
    let scratch = ImageHome::scratch(builder, row, result.place, run::SCRATCH_BYTES)?;
    let mut assessments = Vec::with_capacity(2);
    for (profile, selectors, geometry, role) in [
        (
            source_profile,
            source_profile_indices,
            RunGeometry {
                count,
                base: source_base,
                length: source_length,
            },
            SupplyRole::Source,
        ),
        (
            destination_profile,
            destination_profile_indices,
            RunGeometry {
                count,
                base: destination_base,
                length: destination_length,
            },
            SupplyRole::Destination,
        ),
    ] {
        let projected = leaf_copy::project_operand(source, row, builder, profile, selectors)?;
        let profile_home = ImageHome {
            storage: fragmented_scalar::StorageAccess::Place {
                indices: projected.indices,
                byte_offset: 0,
            },
            place: profile.place,
            pointer: projected
                .pointer
                .ok_or_else(|| failure("relocation profile has no addressable home"))?,
            byte_offset: projected.byte_offset,
        };
        let assessment = run::assess(
            types,
            profile_type,
            builder,
            row,
            profile_home,
            scratch,
            geometry,
            &plan.placement,
            role,
        )?;
        operation_control::require_nonzero(builder, row, assessment.well_formed)?;
        assessments.push(assessment.compatible);
    }
    macro_rules! binary {
        ($kind:ident, $left:expr, $right:expr) => {
            fragmented_scalar::binary(
                builder,
                row,
                result.place,
                SelectedInstructionKind::$kind,
                $left,
                $right,
            )
        };
    }
    macro_rules! at_most {
        ($left:expr, $right:expr) => {
            compare(
                builder,
                row,
                result.place,
                $left,
                $right,
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
            )
        };
    }
    let source_end = binary!(WrappingAddI64, source_base, source_length)?;
    let destination_end = binary!(WrappingAddI64, destination_base, destination_length)?;
    let before = at_most!(source_end, destination_base)?;
    let after = at_most!(destination_end, source_base)?;
    let zero = fragmented_scalar::constant(builder, row, result.place, 0)?;
    let empty = compare(
        builder,
        row,
        result.place,
        source_length,
        zero,
        SelectedInstructionKind::MaterializeBooleanEqual,
    )?;
    let disjoint = binary!(BitwiseOrI64, before, after)?;
    let disjoint = binary!(BitwiseOrI64, disjoint, empty)?;
    let compatible = binary!(BitwiseAndI64, assessments[0], assessments[1])?;
    let ready = binary!(BitwiseAndI64, compatible, disjoint)?;
    let one = fragmented_scalar::constant(builder, row, result.place, 1)?;
    let decision = PlacementDecision {
        ready,
        outcome_tag: binary!(BitwiseXorI64, one, ready)?,
        failure_tag: fragmented_scalar::constant(builder, row, result.place, 2)?,
        copy_length: binary!(WrappingMultiplyI64, source_length, ready)?,
    };
    // Relocation reports incompatible geometry or supply uniformly. No bytes
    // are written on failure, and the staged inputs remain the returned value.
    outcome::write_failure(
        builder,
        row,
        result_home,
        &layout,
        &decision,
        [source_length, destination_length],
        [one, zero],
    )?;
    memory(
        builder,
        row,
        run_argument.place,
        0,
        0,
        SelectedMemoryAccessRole::ReadBackingSpan,
    )?;
    memory(
        builder,
        row,
        destination.place,
        0,
        0,
        SelectedMemoryAccessRole::WriteBackingSpan,
    )?;
    initialization::commit(
        builder,
        row,
        destination.place,
        source_base,
        destination_base,
        &decision,
    )?;
    operation_control::when_nonzero(builder, row, ready, |builder| {
        leaf_copy::copy_operand(
            source,
            row,
            builder,
            destination,
            destination_indices,
            leaf_copy::CopyEnd {
                place: result.place,
                pointer: result_pointer,
                byte_offset: layout.ready_backing.byte_offset,
                elements: &[],
            },
            layout.ready_backing.shape,
        )?;
        let mut old_backing = run_argument.clone();
        old_backing.path.extend(
            plan.storage
                .owner_storage_path(types)
                .ok_or_else(|| failure("relocation lost its source backing path"))?,
        );
        leaf_copy::copy_operand(
            source,
            row,
            builder,
            &old_backing,
            run_indices,
            leaf_copy::CopyEnd {
                place: result.place,
                pointer: result_pointer,
                byte_offset: vacated.byte_offset,
                elements: &[],
            },
            vacated.shape,
        )?;
        write_word(builder, row, result_home, count_offset, 8, count)
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
