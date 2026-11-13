/*
We select and validate resource access-lifecycle operations (`AccessLifecycle`),
including initialization, relocation, resource-profile compatibility, result
layout, and view projection.
*/

use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    PlaceId, SelectedInstructionKind, VirtualRegisterId, fragmented_scalar, record,
};
use crate::structural_inputs::structural_reference_input;
use abstract_operations::AccessLifecycleOperands;
use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
use semantic_vocabulary::{
    AuthoredBitFragment, AuthoredBitFragments, AuthoredFieldStorage, StructuralTypeId,
};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAccessLifecyclePlan,
};

pub(crate) mod initialization {
    use super::{
        Builder, ImageDirection, ImageHome, LegalizedScalarInstruction, PlaceId,
        SelectedInstructionKind, StructuralTypeDeclaration, TerminalAccessLifecyclePlan,
        VirtualRegisterId, coordinate, failure, fragmented_scalar, operand_indices, owner_word,
    };
    use crate::SelectedInstructionError;

    use super::{
        outcome::{PlacementDecision, decide, write_word},
        result_layout,
    };

    pub(crate) fn emit(
        source: &super::LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        plan: &TerminalAccessLifecyclePlan,
    ) -> Result<(), SelectedInstructionError> {
        use super::resource_profile::compatibility;
        use crate::selection::construction::scalar_graph::operation_control;
        use crate::selection::construction::scalar_graph::structural::{leaf_copy, record};
        use abstract_operations::AccessLifecycleOperands;
        use terminal_psi::TerminalAccessLifecycleExecution;
        let super::LegalizedScalarInstructionKind::AccessLifecycle {
            operands:
                AccessLifecycleOperands::Initialize {
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
        let TerminalAccessLifecycleExecution::CompilerPrimitive {
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
        let result_layout::ReturnedLayout::Owned {
            owner: returned_owner,
            value: returned_value,
        } = layout.returned
        else {
            return Err(failure("initialization lost its owned rejection payloads"));
        };
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
            storage: fragmented_scalar::StorageAccess::Place {
                indices: &[],
                byte_offset: 0,
            },
            place: result.place,
            pointer: result_pointer,
            byte_offset: 0,
        };
        for (argument, selectors, field) in [
            (storage, storage_indices, returned_owner),
            (value, value_indices, returned_value),
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
            byte_offset: returned_owner.byte_offset,
            ..result_home
        };
        let base = owner_word(types, row, builder, plan, owner_home, plan.storage.base)?;
        let available = owner_word(types, row, builder, plan, owner_home, plan.storage.length)?;
        let projected = leaf_copy::project_operand(source, row, builder, profile, profile_indices)?;
        let profile_home = ImageHome {
            storage: fragmented_scalar::StorageAccess::Place {
                indices: projected.indices,
                byte_offset: 0,
            },
            place: profile.place,
            pointer: projected.pointer.ok_or_else(|| {
                failure("borrowed initialization profile has no addressable home")
            })?,
            byte_offset: projected.byte_offset,
        };
        let image_length = plan
            .placement
            .layout()
            .size
            .ok_or_else(|| failure("initialization requires a fixed complete image"))?;
        let scratch_bytes =
            super::offset(compatibility::COMPATIBILITY_SCRATCH_BYTES, image_length)?;
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
            byte_offset: returned_value.byte_offset,
            ..result_home
        };
        stage_image(types, builder, row, plan, value_home, staged)?;
        let compatible = if matches!(
            plan.storage.owner,
            terminal_psi::TerminalAccessOwner::Run { .. }
        ) {
            let required = fragmented_scalar::constant(builder, row, result.place, image_length)?;
            let exact = super::outcome::compare(
                builder,
                row,
                result.place,
                available,
                required,
                SelectedInstructionKind::MaterializeBooleanEqual,
            )?;
            fragmented_scalar::binary(
                builder,
                row,
                result.place,
                SelectedInstructionKind::BitwiseAndI64,
                assessment.compatible,
                exact,
            )?
        } else {
            assessment.compatible
        };
        let decision = decide(
            builder,
            row,
            result.place,
            base,
            available,
            image_length,
            assessment.modulus,
            assessment.residue,
            compatible,
        )?;

        let required = fragmented_scalar::constant(builder, row, result.place, image_length)?;
        super::outcome::write_failure(
            builder,
            row,
            result_home,
            &layout,
            &decision,
            [required, available],
            [assessment.modulus, assessment.residue],
        )?;
        let image_pointer =
            crate::selection::construction::scalar_graph::structural::transport_register(
                builder,
                result.place,
                staged.byte_offset,
            )?;
        builder.emit_address_offset(
            staged.byte_offset,
            staged.pointer,
            image_pointer,
            crate::selection::construction::scalar_graph::structural::provenance(row),
        )?;
        staged.storage.footprint(
            builder,
            row,
            staged.place,
            staged.byte_offset,
            coordinate(image_length)?,
            crate::selection::construction::scalar_graph::structural::runtime_address::Written::None,
        )?;
        fragmented_scalar::StorageAccess::Backing.footprint(
            builder,
            row,
            storage.place,
            0,
            coordinate(image_length)?,
            crate::selection::construction::scalar_graph::structural::runtime_address::Written::Structural,
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
                    byte_offset: layout.ready_backing.byte_offset,
                    elements: &[],
                },
                layout.ready_backing.shape,
            )?;
            if let Some(count_offset) = layout.ready_count {
                let one = fragmented_scalar::constant(builder, row, result.place, 1)?;
                write_word(builder, row, result_home, count_offset, 8, one)?;
            }
            Ok(())
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

    pub(crate) fn stage_image(
        declarations: &[StructuralTypeDeclaration],
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        plan: &TerminalAccessLifecyclePlan,
        value: ImageHome,
        staged: ImageHome,
    ) -> Result<(), SelectedInstructionError> {
        let byte_count =
            coordinate(plan.placement.layout().size.ok_or_else(|| {
                failure("resident initialization requires a fixed complete image")
            })?)?;
        if byte_count != 0 {
            super::offset(staged.byte_offset, u64::from(byte_count))?;
            let pointer =
                crate::selection::construction::scalar_graph::structural::transport_register(
                    builder,
                    staged.place,
                    staged.byte_offset,
                )?;
            builder.emit_address_offset(
                staged.byte_offset,
                staged.pointer,
                pointer,
                crate::selection::construction::scalar_graph::structural::provenance(row),
            )?;
            let count =
                fragmented_scalar::constant(builder, row, staged.place, u64::from(byte_count))?;
            let cursor =
                crate::selection::construction::scalar_graph::structural::transport_register(
                    builder,
                    staged.place,
                    staged.byte_offset,
                )?;
            staged.storage.footprint(
                builder,
                row,
                staged.place,
                staged.byte_offset,
                byte_count,
                crate::selection::construction::scalar_graph::structural::runtime_address::Written::Structural,
            )?;
            builder.emit(
                SelectedInstructionKind::ZeroBytes,
                builder
                    .constraints
                    .keys
                    .zero_bytes
                    .ok_or_else(|| failure("resident image target lacks exact byte zeroing"))?,
                &[pointer, count, cursor],
                crate::selection::construction::scalar_graph::structural::provenance(row),
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

    pub(crate) fn commit(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        owner: PlaceId,
        staged: VirtualRegisterId,
        backing: VirtualRegisterId,
        decision: &PlacementDecision,
    ) -> Result<(), SelectedInstructionError> {
        let cursor = crate::selection::construction::scalar_graph::structural::transport_register(
            builder, owner, 0,
        )?;
        let byte = crate::selection::construction::scalar_graph::structural::transport_register(
            builder, owner, 0,
        )?;
        builder.emit(
            SelectedInstructionKind::CopyBytes,
            builder
                .constraints
                .keys
                .copy_bytes
                .ok_or_else(|| failure("resident initialization target lacks exact byte copy"))?,
            &[staged, backing, decision.copy_length, cursor, byte],
            crate::selection::construction::scalar_graph::structural::provenance(row),
        )
    }
}
pub(crate) mod outcome {
    use super::{
        Builder, ImageHome, LegalizedScalarInstruction, PlaceId, SelectedInstructionKind,
        VirtualRegisterId, fragmented_scalar,
    };
    use crate::SelectedInstructionError;

    pub(crate) fn write_word(
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

    pub(crate) struct PlacementDecision {
        pub(crate) ready: VirtualRegisterId,
        pub(crate) outcome_tag: VirtualRegisterId,
        pub(crate) failure_tag: VirtualRegisterId,
        pub(crate) copy_length: VirtualRegisterId,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn decide(
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
        macro_rules! binary {
            ($kind:expr => $left:expr => $right:expr) => {
                fragmented_scalar::binary(builder, row, owner, $kind, $left, $right)
            };
        }
        macro_rules! cmp {
            ($left:expr => $right:expr => $kind:ident) => {
                compare(
                    builder,
                    row,
                    owner,
                    $left,
                    $right,
                    SelectedInstructionKind::$kind,
                )
            };
        }

        let required = fragmented_scalar::constant(builder, row, owner, image_length)?;
        let range = cmp!(required => available => MaterializeBooleanU64LessOrEqual)?;
        let one = fragmented_scalar::constant(builder, row, owner, 1)?;
        let zero = fragmented_scalar::constant(builder, row, owner, 0)?;
        let modulus_mask = binary!(SelectedInstructionKind::WrappingSubtractI64 => modulus => one)?;
        let present = cmp!(one => modulus => MaterializeBooleanU64LessOrEqual)?;
        let low_bits = binary!(SelectedInstructionKind::BitwiseAndI64 => modulus => modulus_mask)?;
        let power_of_two = cmp!(low_bits => zero => MaterializeBooleanEqual)?;
        let bounded_residue = cmp!(residue => modulus_mask => MaterializeBooleanU64LessOrEqual)?;
        let valid_modulus =
            binary!(SelectedInstructionKind::BitwiseAndI64 => present => power_of_two)?;
        let valid_congruence = fragmented_scalar::binary(
            builder,
            row,
            owner,
            SelectedInstructionKind::BitwiseAndI64,
            valid_modulus,
            bounded_residue,
        )?;
        let actual_residue =
            binary!(SelectedInstructionKind::BitwiseAndI64 => base => modulus_mask)?;
        let aligned = cmp!(actual_residue => residue => MaterializeBooleanEqual)?;
        let supplied = cmp!(supply_compatible => one => MaterializeBooleanEqual)?;
        let supplied =
            binary!(SelectedInstructionKind::BitwiseAndI64 => supplied => valid_congruence)?;
        let range_and_supply =
            binary!(SelectedInstructionKind::BitwiseAndI64 => range => supplied)?;
        let ready = binary!(SelectedInstructionKind::BitwiseAndI64 => range_and_supply => aligned)?;
        let outcome_tag = binary!(SelectedInstructionKind::BitwiseXorI64 => one => ready)?;
        let two = fragmented_scalar::constant(builder, row, owner, 2)?;
        let supply_failure =
            binary!(SelectedInstructionKind::WrappingSubtractI64 => two => supplied)?;
        let failure_tag = fragmented_scalar::binary(
            builder,
            row,
            owner,
            SelectedInstructionKind::WrappingMultiplyI64,
            range,
            supply_failure,
        )?;
        let copy_length =
            binary!(SelectedInstructionKind::WrappingMultiplyI64 => required => ready)?;
        Ok(PlacementDecision {
            ready,
            outcome_tag,
            failure_tag,
            copy_length,
        })
    }

    pub(crate) fn compare(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        owner: PlaceId,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
        result_kind: SelectedInstructionKind,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let output = crate::selection::construction::scalar_graph::structural::transport_register(
            builder, owner, 0,
        )?;
        builder.emit(
            SelectedInstructionKind::CompareI64,
            builder.constraints.keys.compare_i64,
            &[left, right],
            crate::selection::construction::scalar_graph::structural::provenance(row),
        )?;
        builder.emit(
            result_kind,
            builder.constraints.keys.materialize_boolean,
            &[output],
            crate::selection::construction::scalar_graph::structural::provenance(row),
        )?;
        Ok(output)
    }

    pub(crate) fn write_failure(
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
            crate::selection::construction::scalar_graph::operation_control::when_nonzero(
                builder,
                row,
                selected,
                |builder| {
                    for (coordinate, word) in coordinates.into_iter().zip(words) {
                        write_word(builder, row, home, coordinate, 8, word)?;
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    }
}
pub(crate) mod relocation {
    use super::outcome::{PlacementDecision, compare, write_word};
    use super::resource_profile::compatibility::run::{self, RunGeometry, SupplyRole};
    use super::*;
    use crate::selection::construction::scalar_graph::operation_control;
    use crate::selection::construction::scalar_graph::structural::{leaf_copy, memory};
    use selected_instructions::SelectedMemoryAccessRole;

    pub(crate) fn emit(
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
        let count = ScalarImageCodec::field(types, carrier, count_field)?.read(
            builder,
            row,
            source_home,
        )?;
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
        let source_length =
            owner_word(types, row, builder, plan, source_home, plan.storage.length)?;
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
}
pub(crate) mod resource_profile {
    use super::{
        Builder, ImageHome, LegalizedScalarInstruction, SelectedInstructionError,
        SelectedInstructionKind, VirtualRegisterId, failure, fragmented_scalar,
    };
    use crate::selection::construction::scalar_graph::operation_control;

    pub(crate) mod compatibility {
        use super::*;
        use access_plans::{
            AtomicPermissions, BoundaryReach, ExternalRead, FieldAccess, ValidatedAccessPlan,
        };

        macro_rules! bin {
            ($emission:expr => $builder:expr => $kind:ident => $left:expr => $right:expr) => {
                $emission.binary($builder, SelectedInstructionKind::$kind, $left, $right)
            };
        }

        pub(in super::super) mod run {
            use super::*;

            pub(in super::super::super) const SCRATCH_BYTES: u32 = 240;
            const RUN_COMPATIBLE: u32 = 200;
            const RUN_REGION: u32 = 208;
            const SEARCH_LOWER: u32 = 216;
            const SEARCH_UPPER: u32 = 224;

            #[derive(Clone, Copy)]
            pub(in super::super::super) struct RunGeometry {
                pub count: VirtualRegisterId,
                pub base: VirtualRegisterId,
                pub length: VirtualRegisterId,
            }

            #[derive(Clone, Copy)]
            pub(in super::super::super) enum SupplyRole {
                Source,
                Destination,
            }

            pub(in super::super::super) fn assess(
                declarations: &[terminal_psi::StructuralTypeDeclaration],
                profile_type: semantic_vocabulary::StructuralTypeId,
                builder: &mut Builder<'_>,
                row: &LegalizedScalarInstruction,
                profile: ImageHome,
                scratch: ImageHome,
                geometry: RunGeometry,
                placement: &ValidatedAccessPlan,
                role: SupplyRole,
            ) -> Result<Compatibility, SelectedInstructionError> {
                let layout = ProfileLayout::derive(declarations, profile_type)?;
                let well_formed =
                    validate_layout(&layout, builder, row, profile, scratch, geometry.length)?;
                let emission = Emission {
                    row,
                    profile,
                    profile_bytes: layout.bytes,
                    scratch,
                };
                let zero = emission.constant(builder, 0)?;
                let one = emission.constant(builder, 1)?;
                emission.write_scratch(builder, RUN_COMPATIBLE, zero)?;
                let footprint = placement
                    .layout()
                    .size
                    .ok_or_else(|| failure("run cell has no fixed footprint"))?;
                operation_control::when_nonzero(builder, row, well_formed, |builder| {
                    let stride = emission.constant(builder, footprint)?;
                    let occupied = bin!(emission => builder => WrappingMultiplyI64 => geometry.count => stride)?;
                    let exact = emission.equal(builder, occupied, geometry.length)?;
                    emission.write_scratch(builder, RUN_COMPATIBLE, exact)?;
                    if let Some(maximum_count) = u64::MAX.checked_div(footprint) {
                        let bound = emission.constant(builder, maximum_count)?;
                        let fits = emission.at_most(builder, geometry.count, bound)?;
                        intersect(emission, builder, RUN_COMPATIBLE, fits)?;
                    }
                    let end = bin!(emission => builder => WrappingAddI64 => geometry.base => geometry.length)?;
                    let fits = emission.at_most(builder, geometry.base, end)?;
                    intersect(emission, builder, RUN_COMPATIBLE, fits)?;
                    let displacement =
                        emission.constant(builder, u64::from(profile.byte_offset))?;
                    let pointer = bin!(emission => builder => WrappingAddI64 => profile.pointer => displacement)?;
                    let regions = emission.load(builder, pointer, layout.regions.count)?;
                    emission.write_scratch(builder, COVERED, zero)?;
                    extend_coverage(
                        emission,
                        builder,
                        &layout,
                        pointer,
                        regions,
                        geometry.length,
                        CoverageDemand::Stable {
                            read: matches!(role, SupplyRole::Source),
                            write: matches!(role, SupplyRole::Destination),
                        },
                        placement.reach(),
                        [WALK, CANDIDATE],
                    )?;
                    let covered = emission.read_scratch(builder, COVERED)?;
                    let complete = emission.at_most(builder, geometry.length, covered)?;
                    intersect(emission, builder, RUN_COMPATIBLE, complete)?;
                    if matches!(role, SupplyRole::Source) {
                        return Ok(());
                    }
                    let compatible = emission.read_scratch(builder, RUN_COMPATIBLE)?;
                    operation_control::when_nonzero(builder, row, compatible, |builder| {
                        let check = |builder: &mut Builder<'_>, cell| {
                            check_cell(
                                emission,
                                &layout,
                                builder,
                                geometry,
                                placement,
                                footprint,
                                cell,
                                well_formed,
                            )
                        };
                        check(builder, zero)?;
                        if footprint == 0 {
                            return Ok(());
                        }
                        check(builder, one)?;
                        let last = bin!(emission => builder => WrappingSubtractI64 => geometry.count => one)?;
                        check(builder, last)?;
                        emission.repeat(builder, regions, RUN_REGION, |builder, position| {
                            let region = emission.element(builder, pointer, layout.regions, position)?;
                            let start = emission.load(builder, region, layout.start)?;
                            let length = emission.load(builder, region, layout.length)?;
                            let end = bin!(emission => builder => WrappingAddI64 => start => length)?;
                            for boundary in [start, end] {
                                let cell = boundary_cell(emission, builder, geometry.count, stride, boundary)?;
                                let previous = bin!(emission => builder => WrappingSubtractI64 => cell => one)?;
                                check(builder, previous)?;
                                check(builder, cell)?;
                                for distance in [1, 2] {
                                    let distance = emission.constant(builder, distance)?;
                                    let next = bin!(emission => builder => WrappingAddI64 => cell => distance)?;
                                    let no_wrap = emission.at_most(builder, cell, next)?;
                                    operation_control::when_nonzero(builder, row, no_wrap, |builder| {
                                        check(builder, next)
                                    })?;
                                }
                            }
                            Ok(())
                        })
                    })
                })?;
                Ok(Compatibility {
                    well_formed,
                    compatible: emission.read_scratch(builder, RUN_COMPATIBLE)?,
                    modulus: one,
                    residue: zero,
                })
            }

            fn check_cell(
                emission: Emission<'_>,
                layout: &ProfileLayout,
                builder: &mut Builder<'_>,
                geometry: RunGeometry,
                placement: &ValidatedAccessPlan,
                footprint: u64,
                cell: VirtualRegisterId,
                well_formed: VirtualRegisterId,
            ) -> Result<(), SelectedInstructionError> {
                let outside = emission.at_most(builder, geometry.count, cell)?;
                let inside = emission.invert(builder, outside)?;
                operation_control::when_nonzero(builder, emission.row, inside, |builder| {
                    let stride = emission.constant(builder, footprint)?;
                    let origin =
                        bin!(emission => builder => WrappingMultiplyI64 => cell => stride)?;
                    let assessment = assess_cell(
                        emission,
                        layout,
                        builder,
                        geometry.length,
                        placement,
                        terminal_psi::TerminalAccessLifecycleOperation::Initialize,
                        origin,
                        well_formed,
                    )?;
                    let one = emission.constant(builder, 1)?;
                    let mask = bin!(emission => builder => WrappingSubtractI64 => assessment.modulus => one)?;
                    let residue =
                        bin!(emission => builder => BitwiseAndI64 => geometry.base => mask)?;
                    let aligned = emission.equal(builder, residue, assessment.residue)?;
                    intersect(emission, builder, RUN_COMPATIBLE, assessment.compatible)?;
                    intersect(emission, builder, RUN_COMPATIBLE, aligned)
                })
            }

            fn boundary_cell(
                emission: Emission<'_>,
                builder: &mut Builder<'_>,
                count: VirtualRegisterId,
                stride: VirtualRegisterId,
                boundary: VirtualRegisterId,
            ) -> Result<VirtualRegisterId, SelectedInstructionError> {
                let zero = emission.constant(builder, 0)?;
                emission.write_scratch(builder, SEARCH_LOWER, zero)?;
                emission.write_scratch(builder, SEARCH_UPPER, count)?;
                operation_control::while_nonzero(
                    builder,
                    emission.row,
                    |builder| {
                        let lower = emission.read_scratch(builder, SEARCH_LOWER)?;
                        let upper = emission.read_scratch(builder, SEARCH_UPPER)?;
                        let same = emission.equal(builder, lower, upper)?;
                        emission.invert(builder, same)
                    },
                    |builder| {
                        let lower = emission.read_scratch(builder, SEARCH_LOWER)?;
                        let upper = emission.read_scratch(builder, SEARCH_UPPER)?;
                        let distance =
                            bin!(emission => builder => WrappingSubtractI64 => upper => lower)?;
                        let half = fragmented_scalar::shift(
                            builder,
                            emission.row,
                            emission.scratch.place,
                            distance,
                            1,
                            false,
                        )?;
                        let one = emission.constant(builder, 1)?;
                        let odd = bin!(emission => builder => BitwiseAndI64 => distance => one)?;
                        let rounded = bin!(emission => builder => WrappingAddI64 => half => odd)?;
                        let middle =
                            bin!(emission => builder => WrappingAddI64 => lower => rounded)?;
                        let offset =
                            bin!(emission => builder => WrappingMultiplyI64 => middle => stride)?;
                        let below = emission.at_most(builder, offset, boundary)?;
                        operation_control::when_nonzero(builder, emission.row, below, |builder| {
                            emission.write_scratch(builder, SEARCH_LOWER, middle)
                        })?;
                        let above = emission.invert(builder, below)?;
                        operation_control::when_nonzero(builder, emission.row, above, |builder| {
                            let preceding =
                                bin!(emission => builder => WrappingSubtractI64 => middle => one)?;
                            emission.write_scratch(builder, SEARCH_UPPER, preceding)
                        })
                    },
                )?;
                emission.read_scratch(builder, SEARCH_LOWER)
            }
        }

        pub(in super::super) const COMPATIBILITY_SCRATCH_BYTES: u32 = 200;
        const COMPATIBLE: u32 = 48;
        const MODULUS: u32 = 56;
        const RESIDUE: u32 = 64;
        const COVERED: u32 = 72;
        const WALK: u32 = 80;
        const CANDIDATE: u32 = 88;
        const EQUAL: u32 = 96;
        const MATCH: u32 = 104;
        const LEFT_RULE: u32 = 112;
        const RIGHT_RULE: u32 = 120;
        const LEFT_SERVICE: u32 = 128;
        const RIGHT_SERVICE: u32 = 136;
        const ALIGNMENT: u32 = 144;
        const FOUND: u32 = 152;
        const CONTAINS: u32 = 160;
        const SHARED_MODULUS: u32 = 168;
        const INNER_WALK: u32 = 176;
        const INNER_CANDIDATE: u32 = 184;
        const RUN_END: u32 = 192;

        pub(in super::super) struct Compatibility {
            pub(in super::super) well_formed: VirtualRegisterId,
            pub(in super::super) compatible: VirtualRegisterId,
            pub(in super::super) modulus: VirtualRegisterId,
            pub(in super::super) residue: VirtualRegisterId,
        }

        fn intersect(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            cell: u32,
            predicate: VirtualRegisterId,
        ) -> Result<(), SelectedInstructionError> {
            let previous = emission.read_scratch(builder, cell)?;
            let next = bin!(emission => builder => BitwiseAndI64 => previous => predicate)?;
            emission.write_scratch(builder, cell, next)
        }

        fn unite(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            cell: u32,
            predicate: VirtualRegisterId,
        ) -> Result<(), SelectedInstructionError> {
            let previous = emission.read_scratch(builder, cell)?;
            let next = bin!(emission => builder => BitwiseOrI64 => previous => predicate)?;
            emission.write_scratch(builder, cell, next)
        }

        fn contains_service(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            layout: &ProfileLayout,
            region: VirtualRegisterId,
            service: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let zero = emission.constant(builder, 0)?;
            emission.write_scratch(builder, MATCH, zero)?;
            let count = emission.load(builder, region, layout.reach.count)?;
            emission.repeat(builder, count, RIGHT_SERVICE, |builder, position| {
                let entry = emission.element(builder, region, layout.reach, position)?;
                let value = emission.load(builder, entry, layout.service)?;
                let same = emission.equal(builder, value, service)?;
                unite(emission, builder, MATCH, same)
            })?;
            emission.read_scratch(builder, MATCH)
        }

        fn reaches(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            layout: &ProfileLayout,
            region: VirtualRegisterId,
            required: &BoundaryReach,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let one = emission.constant(builder, 1)?;
            emission.write_scratch(builder, CONTAINS, one)?;
            for service in required.services() {
                let service = emission.constant(builder, service.normalized_identity())?;
                let found = contains_service(emission, builder, layout, region, service)?;
                intersect(emission, builder, CONTAINS, found)?;
            }
            emission.read_scratch(builder, CONTAINS)
        }

        fn same_fields(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
            fields: impl IntoIterator<Item = Scalar>,
            accumulated: u32,
        ) -> Result<(), SelectedInstructionError> {
            for field in fields {
                let left_value = emission.load(builder, left, field)?;
                let right_value = emission.load(builder, right, field)?;
                let same = emission.equal(builder, left_value, right_value)?;
                intersect(emission, builder, accumulated, same)?;
            }
            Ok(())
        }

        fn same_rules(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
            array: Counted,
            transfer: &TransferLayout,
        ) -> Result<(), SelectedInstructionError> {
            let left_count = emission.load(builder, left, array.count)?;
            let right_count = emission.load(builder, right, array.count)?;
            let same_count = emission.equal(builder, left_count, right_count)?;
            intersect(emission, builder, EQUAL, same_count)?;
            emission.repeat(builder, left_count, LEFT_RULE, |builder, left_position| {
                let left_entry = emission.element(builder, left, array, left_position)?;
                let zero = emission.constant(builder, 0)?;
                emission.write_scratch(builder, MATCH, zero)?;
                emission.repeat(
                    builder,
                    right_count,
                    RIGHT_RULE,
                    |builder, right_position| {
                        let right_entry =
                            emission.element(builder, right, array, right_position)?;
                        let mut same = emission.constant(builder, 1)?;
                        for field in [transfer.width, transfer.alignment]
                            .into_iter()
                            .chain(transfer.operations.iter().flatten().copied())
                        {
                            let left_value = emission.load(builder, left_entry, field)?;
                            let right_value = emission.load(builder, right_entry, field)?;
                            let equal = emission.equal(builder, left_value, right_value)?;
                            same = bin!(emission => builder => BitwiseAndI64 => same => equal)?;
                        }
                        unite(emission, builder, MATCH, same)
                    },
                )?;
                let found = emission.read_scratch(builder, MATCH)?;
                intersect(emission, builder, EQUAL, found)
            })
        }

        fn same_reach_direction(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            layout: &ProfileLayout,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<(), SelectedInstructionError> {
            let count = emission.load(builder, left, layout.reach.count)?;
            emission.repeat(builder, count, LEFT_SERVICE, |builder, position| {
                let entry = emission.element(builder, left, layout.reach, position)?;
                let service = emission.load(builder, entry, layout.service)?;
                let present = contains_service(emission, builder, layout, right, service)?;
                intersect(emission, builder, EQUAL, present)
            })
        }

        fn equivalent(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            layout: &ProfileLayout,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let one = emission.constant(builder, 1)?;
            emission.write_scratch(builder, EQUAL, one)?;
            same_fields(
                emission,
                builder,
                left,
                right,
                [layout.peer, layout.stable, layout.external, layout.atomic],
                EQUAL,
            )?;
            for (tag, rules, transfer) in [
                (
                    layout.external,
                    layout.external_rules,
                    &layout.external_transfer,
                ),
                (layout.atomic, layout.atomic_rules, &layout.atomic_transfer),
            ] {
                let left_tag = emission.load(builder, left, tag)?;
                let right_tag = emission.load(builder, right, tag)?;
                let left_active = emission.equal(builder, left_tag, one)?;
                let right_active = emission.equal(builder, right_tag, one)?;
                let both_active = emission.binary(
                    builder,
                    SelectedInstructionKind::BitwiseAndI64,
                    left_active,
                    right_active,
                )?;
                operation_control::when_nonzero(builder, emission.row, both_active, |builder| {
                    if transfer.operations.is_none() {
                        same_fields(
                            emission,
                            builder,
                            left,
                            right,
                            [layout.external_read, layout.external_write],
                            EQUAL,
                        )?;
                    }
                    same_rules(emission, builder, left, right, rules, transfer)
                })?;
            }
            same_reach_direction(emission, builder, layout, left, right)?;
            same_reach_direction(emission, builder, layout, right, left)?;
            emission.read_scratch(builder, EQUAL)
        }

        fn stable_permits(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            layout: &ProfileLayout,
            region: VirtualRegisterId,
            read: bool,
            write: bool,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let required = emission.constant(builder, u64::from(read) | (u64::from(write) << 1))?;
            let stable = emission.load(builder, region, layout.stable)?;
            let supplied = bin!(emission => builder => BitwiseAndI64 => stable => required)?;
            emission.equal(builder, supplied, required)
        }

        pub(crate) fn atomic_bits(operations: AtomicPermissions) -> [bool; 12] {
            [
                operations.load,
                operations.store,
                operations.fetch_add,
                operations.fetch_sub,
                operations.fetch_xor,
                operations.fetch_or,
                operations.fetch_and,
                operations.swap,
                operations.compare_exchange,
                operations.compare_exchange_once,
                operations.try_exchange,
                operations.try_exchange_once,
            ]
        }

        fn select_rules(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            region: VirtualRegisterId,
            array: Counted,
            transfer: &TransferLayout,
            width: u16,
            operations: Option<AtomicPermissions>,
        ) -> Result<(), SelectedInstructionError> {
            let count = emission.load(builder, region, array.count)?;
            let required_width = emission.constant(builder, u64::from(width))?;
            emission.repeat(builder, count, LEFT_RULE, |builder, position| {
                let entry = emission.element(builder, region, array, position)?;
                let width = emission.load(builder, entry, transfer.width)?;
                let mut supported = emission.equal(builder, width, required_width)?;
                if let Some(operations) = operations {
                    let one = emission.constant(builder, 1)?;
                    let fields = transfer
                        .operations
                        .as_ref()
                        .ok_or_else(|| failure("atomic profile rule lost its operation fields"))?;
                    for (field, required) in fields.iter().zip(atomic_bits(operations)) {
                        if required {
                            let value = emission.load(builder, entry, *field)?;
                            let present = emission.equal(builder, value, one)?;
                            supported =
                                bin!(emission => builder => BitwiseAndI64 => supported => present)?;
                        }
                    }
                }
                operation_control::when_nonzero(builder, emission.row, supported, |builder| {
                    let alignment = emission.load(builder, entry, transfer.alignment)?;
                    emission.write_scratch(builder, ALIGNMENT, alignment)
                })
            })
        }

        fn effective_alignment(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            layout: &ProfileLayout,
            region: VirtualRegisterId,
            access: &FieldAccess,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let zero = emission.constant(builder, 0)?;
            let one = emission.constant(builder, 1)?;
            emission.write_scratch(builder, ALIGNMENT, zero)?;
            match access {
                FieldAccess::Inaccessible | FieldAccess::Region { .. } => {
                    return Err(failure(
                        "field without primitive transfer geometry acquired a runtime transfer descriptor",
                    ));
                }
                FieldAccess::Stable {
                    transfer_width_bits,
                    read,
                    write,
                    ..
                } => {
                    let supplied =
                        stable_permits(emission, builder, layout, region, *read, *write)?;
                    let bytes = u64::from(*transfer_width_bits / 8);
                    let alignment = emission
                        .constant(builder, if bytes.is_power_of_two() { bytes } else { 1 })?;
                    let alignment = emission.binary(
                        builder,
                        SelectedInstructionKind::WrappingMultiplyI64,
                        alignment,
                        supplied,
                    )?;
                    emission.write_scratch(builder, ALIGNMENT, alignment)?;
                }
                FieldAccess::External {
                    transfer_width_bits,
                    read,
                    write,
                    ..
                } => {
                    let tag = emission.load(builder, region, layout.external)?;
                    let active = emission.equal(builder, tag, one)?;
                    operation_control::when_nonzero(builder, emission.row, active, |builder| {
                        let mut supplied = one;
                        if *read != ExternalRead::None {
                            let expected = emission.constant(
                                builder,
                                if *read == ExternalRead::Read { 1 } else { 2 },
                            )?;
                            let actual = emission.load(builder, region, layout.external_read)?;
                            supplied = emission.equal(builder, actual, expected)?;
                        }
                        if *write {
                            let actual = emission.load(builder, region, layout.external_write)?;
                            supplied =
                                bin!(emission => builder => BitwiseAndI64 => supplied => actual)?;
                        }
                        operation_control::when_nonzero(
                            builder,
                            emission.row,
                            supplied,
                            |builder| {
                                select_rules(
                                    emission,
                                    builder,
                                    region,
                                    layout.external_rules,
                                    &layout.external_transfer,
                                    *transfer_width_bits,
                                    None,
                                )
                            },
                        )
                    })?;
                    if *read != ExternalRead::Take {
                        let selected = emission.read_scratch(builder, ALIGNMENT)?;
                        let missing = emission.equal(builder, selected, zero)?;
                        let stable = stable_permits(
                            emission,
                            builder,
                            layout,
                            region,
                            *read == ExternalRead::Read,
                            *write,
                        )?;
                        let use_stable =
                            bin!(emission => builder => BitwiseAndI64 => missing => stable)?;
                        operation_control::when_nonzero(
                            builder,
                            emission.row,
                            use_stable,
                            |builder| {
                                let bytes = u64::from(*transfer_width_bits / 8);
                                let alignment = emission.constant(
                                    builder,
                                    if bytes.is_power_of_two() { bytes } else { 1 },
                                )?;
                                emission.write_scratch(builder, ALIGNMENT, alignment)
                            },
                        )?;
                    }
                }
                FieldAccess::Atomic {
                    transfer_width_bits,
                    operations,
                    ..
                } => {
                    let tag = emission.load(builder, region, layout.atomic)?;
                    let active = emission.equal(builder, tag, one)?;
                    operation_control::when_nonzero(builder, emission.row, active, |builder| {
                        select_rules(
                            emission,
                            builder,
                            region,
                            layout.atomic_rules,
                            &layout.atomic_transfer,
                            *transfer_width_bits,
                            Some(*operations),
                        )
                    })?;
                }
            }
            emission.read_scratch(builder, ALIGNMENT)
        }

        fn combine_alignment(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            alignment: VirtualRegisterId,
            offset: VirtualRegisterId,
        ) -> Result<(), SelectedInstructionError> {
            let previous_modulus = emission.read_scratch(builder, MODULUS)?;
            let previous_residue = emission.read_scratch(builder, RESIDUE)?;
            let one = emission.constant(builder, 1)?;
            let mask = bin!(emission => builder => WrappingSubtractI64 => alignment => one)?;
            let zero = emission.constant(builder, 0)?;
            let negated_offset =
                bin!(emission => builder => WrappingSubtractI64 => zero => offset)?;
            let residue = bin!(emission => builder => BitwiseAndI64 => negated_offset => mask)?;
            emission.write_scratch(builder, SHARED_MODULUS, previous_modulus)?;
            let smaller = emission.at_most(builder, alignment, previous_modulus)?;
            operation_control::when_nonzero(builder, emission.row, smaller, |builder| {
                emission.write_scratch(builder, SHARED_MODULUS, alignment)
            })?;
            let shared = emission.read_scratch(builder, SHARED_MODULUS)?;
            let shared_mask = bin!(emission => builder => WrappingSubtractI64 => shared => one)?;
            let old =
                bin!(emission => builder => BitwiseAndI64 => previous_residue => shared_mask)?;
            let new = bin!(emission => builder => BitwiseAndI64 => residue => shared_mask)?;
            let consistent = emission.equal(builder, old, new)?;
            intersect(emission, builder, COMPATIBLE, consistent)?;
            let larger = emission.invert(builder, smaller)?;
            operation_control::when_nonzero(builder, emission.row, larger, |builder| {
                emission.write_scratch(builder, MODULUS, alignment)?;
                emission.write_scratch(builder, RESIDUE, residue)
            })
        }

        #[derive(Clone, Copy)]
        enum CoverageDemand<'a> {
            SameRegion(VirtualRegisterId),
            Stable { read: bool, write: bool },
            Repeated(&'a access_plans::FieldRegion),
        }

        fn extend_coverage(
            emission: Emission<'_>,
            builder: &mut Builder<'_>,
            layout: &ProfileLayout,
            pointer: VirtualRegisterId,
            count: VirtualRegisterId,
            target_end: VirtualRegisterId,
            demand: CoverageDemand<'_>,
            reach: &BoundaryReach,
            counters: [u32; 2],
        ) -> Result<(), SelectedInstructionError> {
            emission.repeat(builder, count, counters[0], |builder, _| {
                emission.repeat(builder, count, counters[1], |builder, position| {
                    let covered = emission.read_scratch(builder, COVERED)?;
                    let complete = emission.at_most(builder, target_end, covered)?;
                    let incomplete = emission.invert(builder, complete)?;
                    let candidate = emission.element(builder, pointer, layout.regions, position)?;
                    let start = emission.load(builder, candidate, layout.start)?;
                    let length = emission.load(builder, candidate, layout.length)?;
                    let end = bin!(emission => builder => WrappingAddI64 => start => length)?;
                    let after_start = emission.at_most(builder, start, covered)?;
                    let after_end = emission.at_most(builder, end, covered)?;
                    let before_end = emission.invert(builder, after_end)?;
                    let contains =
                        bin!(emission => builder => BitwiseAndI64 => after_start => before_end)?;
                    let consider =
                        bin!(emission => builder => BitwiseAndI64 => incomplete => contains)?;
                    operation_control::when_nonzero(builder, emission.row, consider, |builder| {
                        let supported = match demand {
                            CoverageDemand::SameRegion(original) => {
                                equivalent(emission, builder, layout, original, candidate)?
                            }
                            CoverageDemand::Stable { read, write } => {
                                let stable = stable_permits(
                                    emission, builder, layout, candidate, read, write,
                                )?;
                                let reachable =
                                    reaches(emission, builder, layout, candidate, reach)?;
                                bin!(emission => builder => BitwiseAndI64 => stable => reachable)?
                            }
                            CoverageDemand::Repeated(region) => {
                                extend_coverage(
                                    emission,
                                    builder,
                                    layout,
                                    pointer,
                                    count,
                                    target_end,
                                    CoverageDemand::SameRegion(candidate),
                                    reach,
                                    [INNER_WALK, INNER_CANDIDATE],
                                )?;
                                let supplied_end = emission.read_scratch(builder, COVERED)?;
                                emission.write_scratch(builder, COVERED, covered)?;
                                emission.write_scratch(builder, RUN_END, target_end)?;
                                let inside = emission.at_most(builder, supplied_end, target_end)?;
                                operation_control::when_nonzero(
                                    builder,
                                    emission.row,
                                    inside,
                                    |builder| {
                                        emission.write_scratch(builder, RUN_END, supplied_end)
                                    },
                                )?;
                                let run_end = emission.read_scratch(builder, RUN_END)?;
                                let span = emission.binary(
                                    builder,
                                    SelectedInstructionKind::WrappingSubtractI64,
                                    run_end,
                                    covered,
                                )?;
                                let width = emission.constant(
                                    builder,
                                    u64::from(region.transfer_width_bits() / 8),
                                )?;
                                let mask = emission.constant(
                                    builder,
                                    u64::from(region.transfer_width_bits() / 8) - 1,
                                )?;
                                let remainder =
                                    bin!(emission => builder => BitwiseAndI64 => span => mask)?;
                                let zero = emission.constant(builder, 0)?;
                                let tiled = emission.equal(builder, remainder, zero)?;
                                let alignment = effective_alignment(
                                    emission,
                                    builder,
                                    layout,
                                    candidate,
                                    &region.scalar_access(),
                                )?;
                                let supplied = emission.nonzero(builder, alignment)?;
                                let single = emission.at_most(builder, span, width)?;
                                let aligned_stride = emission.at_most(builder, alignment, width)?;
                                let stride = emission.binary(
                                    builder,
                                    SelectedInstructionKind::BitwiseOrI64,
                                    single,
                                    aligned_stride,
                                )?;
                                let reachable =
                                    reaches(emission, builder, layout, candidate, reach)?;
                                let mut supported = tiled;
                                for predicate in [supplied, stride, reachable] {
                                    supported = emission.binary(
                                        builder,
                                        SelectedInstructionKind::BitwiseAndI64,
                                        supported,
                                        predicate,
                                    )?;
                                }
                                operation_control::when_nonzero(
                                    builder,
                                    emission.row,
                                    supported,
                                    |builder| {
                                        combine_alignment(emission, builder, alignment, covered)
                                    },
                                )?;
                                supported
                            }
                        };
                        operation_control::when_nonzero(
                            builder,
                            emission.row,
                            supported,
                            |builder| {
                                let end = if matches!(demand, CoverageDemand::Repeated(_)) {
                                    emission.read_scratch(builder, RUN_END)?
                                } else {
                                    let length =
                                        emission.load(builder, candidate, layout.length)?;
                                    bin!(emission => builder => WrappingAddI64 => start => length)?
                                };
                                emission.write_scratch(builder, COVERED, end)
                            },
                        )
                    })
                })
            })
        }

        pub(in super::super) fn assess(
            declarations: &[terminal_psi::StructuralTypeDeclaration],
            profile_type: semantic_vocabulary::StructuralTypeId,
            builder: &mut Builder<'_>,
            row: &LegalizedScalarInstruction,
            profile: ImageHome,
            scratch: ImageHome,
            available: VirtualRegisterId,
            placement: &ValidatedAccessPlan,
            operation: terminal_psi::TerminalAccessLifecycleOperation,
        ) -> Result<Compatibility, SelectedInstructionError> {
            super::super::offset(scratch.byte_offset, u64::from(COMPATIBILITY_SCRATCH_BYTES))?;
            let layout = ProfileLayout::derive(declarations, profile_type)?;
            let well_formed = validate_layout(&layout, builder, row, profile, scratch, available)?;
            let emission = Emission {
                row,
                profile,
                profile_bytes: layout.bytes,
                scratch,
            };
            let origin = emission.constant(builder, 0)?;
            assess_cell(
                emission,
                &layout,
                builder,
                available,
                placement,
                operation,
                origin,
                well_formed,
            )
        }

        fn assess_cell(
            emission: Emission<'_>,
            layout: &ProfileLayout,
            builder: &mut Builder<'_>,
            available: VirtualRegisterId,
            placement: &ValidatedAccessPlan,
            operation: terminal_psi::TerminalAccessLifecycleOperation,
            origin: VirtualRegisterId,
            well_formed: VirtualRegisterId,
        ) -> Result<Compatibility, SelectedInstructionError> {
            let row = emission.row;
            let profile = emission.profile;
            let zero = emission.constant(builder, 0)?;
            let one = emission.constant(builder, 1)?;
            emission.write_scratch(builder, COMPATIBLE, zero)?;
            emission.write_scratch(builder, MODULUS, one)?;
            emission.write_scratch(builder, RESIDUE, zero)?;
            operation_control::when_nonzero(builder, row, well_formed, |builder| {
                let displacement = emission.constant(builder, u64::from(profile.byte_offset))?;
                let pointer = emission.binary(
                    builder,
                    SelectedInstructionKind::WrappingAddI64,
                    profile.pointer,
                    displacement,
                )?;
                let count = emission.load(builder, pointer, layout.regions.count)?;
                let length = emission.constant(builder, placement.access().layout_size_bytes())?;
                let end = bin!(emission => builder => WrappingAddI64 => origin => length)?;
                let nonwrapping = emission.at_most(builder, origin, end)?;
                let contained = emission.at_most(builder, end, available)?;
                let contained =
                    bin!(emission => builder => BitwiseAndI64 => contained => nonwrapping)?;
                emission.write_scratch(builder, COMPATIBLE, contained)?;
                let alignment = emission.constant(builder, placement.layout().align)?;
                combine_alignment(emission, builder, alignment, origin)?;
                if operation != terminal_psi::TerminalAccessLifecycleOperation::View {
                    emission.write_scratch(builder, COVERED, origin)?;
                    extend_coverage(
                        emission,
                        builder,
                        layout,
                        pointer,
                        count,
                        end,
                        CoverageDemand::Stable {
                            read: operation
                                == terminal_psi::TerminalAccessLifecycleOperation::MoveOut,
                            write: operation
                                == terminal_psi::TerminalAccessLifecycleOperation::Initialize,
                        },
                        placement.reach(),
                        [WALK, CANDIDATE],
                    )?;
                    let covered = emission.read_scratch(builder, COVERED)?;
                    let complete = emission.at_most(builder, end, covered)?;
                    intersect(emission, builder, COMPATIBLE, complete)?;
                }
                for descriptor in placement.access().field_descriptors() {
                    let access = placement
                        .access()
                        .field(descriptor.key())
                        .ok_or_else(|| failure("placement descriptor lost its exact field access"))?
                        .access();
                    let offset = descriptor.container_byte_offset();
                    let end = offset
                        .checked_add(u64::from(descriptor.transfer_width_bits() / 8))
                        .ok_or_else(|| failure("placement descriptor interval overflows"))?;
                    let start = emission.constant(builder, offset)?;
                    let start = bin!(emission => builder => WrappingAddI64 => origin => start)?;
                    let end = emission.constant(builder, end)?;
                    let end = bin!(emission => builder => WrappingAddI64 => origin => end)?;
                    emission.write_scratch(builder, FOUND, zero)?;
                    emission.repeat(builder, count, REGION, |builder, position| {
                        let region =
                            emission.element(builder, pointer, layout.regions, position)?;
                        let region_start = emission.load(builder, region, layout.start)?;
                        let region_length = emission.load(builder, region, layout.length)?;
                        let region_end = emission.binary(
                            builder,
                            SelectedInstructionKind::WrappingAddI64,
                            region_start,
                            region_length,
                        )?;
                        let after_start = emission.at_most(builder, region_start, start)?;
                        let after_end = emission.at_most(builder, region_end, start)?;
                        let before_end = emission.invert(builder, after_end)?;
                        let contains_start = emission.binary(
                            builder,
                            SelectedInstructionKind::BitwiseAndI64,
                            after_start,
                            before_end,
                        )?;
                        operation_control::when_nonzero(builder, row, contains_start, |builder| {
                            emission.write_scratch(builder, COVERED, region_end)?;
                            extend_coverage(
                                emission,
                                builder,
                                layout,
                                pointer,
                                count,
                                end,
                                CoverageDemand::SameRegion(region),
                                placement.reach(),
                                [WALK, CANDIDATE],
                            )?;
                            let covered = emission.read_scratch(builder, COVERED)?;
                            let contained = emission.at_most(builder, end, covered)?;
                            let reachable =
                                reaches(emission, builder, layout, region, placement.reach())?;
                            let alignment =
                                effective_alignment(emission, builder, layout, region, access)?;
                            let supplied = emission.nonzero(builder, alignment)?;
                            let supported = emission.binary(
                                builder,
                                SelectedInstructionKind::BitwiseAndI64,
                                contained,
                                reachable,
                            )?;
                            let supported = emission.binary(
                                builder,
                                SelectedInstructionKind::BitwiseAndI64,
                                supported,
                                supplied,
                            )?;
                            emission.write_scratch(builder, FOUND, supported)?;
                            operation_control::when_nonzero(builder, row, supported, |builder| {
                                combine_alignment(emission, builder, alignment, start)
                            })
                        })
                    })?;
                    let found = emission.read_scratch(builder, FOUND)?;
                    intersect(emission, builder, COMPATIBLE, found)?;
                }
                for region in placement.access().field_regions() {
                    let start = emission.constant(builder, region.byte_offset())?;
                    let start = bin!(emission => builder => WrappingAddI64 => origin => start)?;
                    let end = region
                        .byte_offset()
                        .checked_add(region.byte_length())
                        .ok_or_else(|| failure("structural region interval overflows"))?;
                    let end = emission.constant(builder, end)?;
                    let end = bin!(emission => builder => WrappingAddI64 => origin => end)?;
                    emission.write_scratch(builder, COVERED, start)?;
                    extend_coverage(
                        emission,
                        builder,
                        layout,
                        pointer,
                        count,
                        end,
                        if region.observation() == access_plans::ObservationModel::Stable {
                            CoverageDemand::Stable {
                                read: region.permits_read(),
                                write: region.permits_write(),
                            }
                        } else {
                            CoverageDemand::Repeated(region)
                        },
                        placement.reach(),
                        [WALK, CANDIDATE],
                    )?;
                    let covered = emission.read_scratch(builder, COVERED)?;
                    let complete = emission.at_most(builder, end, covered)?;
                    intersect(emission, builder, COMPATIBLE, complete)?;
                    let alignment = emission.constant(builder, region.byte_alignment())?;
                    combine_alignment(emission, builder, alignment, start)?;
                }
                Ok(())
            })?;
            Ok(Compatibility {
                well_formed,
                compatible: emission.read_scratch(builder, COMPATIBLE)?,
                modulus: emission.read_scratch(builder, MODULUS)?,
                residue: emission.read_scratch(builder, RESIDUE)?,
            })
        }
    }
    pub(crate) mod geometry {
        use super::{SelectedInstructionError, failure};
        use crate::structural_inputs::structural_reference_input;
        use semantic_vocabulary::{IntegerSign, ScalarType, StructuralTypeId};
        use terminal_psi::{
            StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
            StructuralTypeShape, find_structural_type,
        };

        #[derive(Clone, Copy)]
        pub(crate) struct Scalar {
            pub(crate) offset: u32,
            pub(crate) width: u32,
        }

        #[derive(Clone, Copy)]
        pub(crate) struct Counted {
            pub(crate) offset: u32,
            pub(crate) stride: u32,
            pub(crate) capacity: u64,
            pub(crate) count: Scalar,
        }

        pub(crate) struct TransferLayout {
            pub(crate) width: Scalar,
            pub(crate) alignment: Scalar,
            pub(crate) operations: Option<[Scalar; 12]>,
        }

        pub(crate) struct ProfileLayout {
            pub(crate) bytes: u32,
            pub(crate) regions: Counted,
            pub(crate) start: Scalar,
            pub(crate) length: Scalar,
            pub(crate) peer: Scalar,
            pub(crate) stable: Scalar,
            pub(crate) external: Scalar,
            pub(crate) external_read: Scalar,
            pub(crate) external_write: Scalar,
            pub(crate) external_rules: Counted,
            pub(crate) external_transfer: TransferLayout,
            pub(crate) atomic: Scalar,
            pub(crate) atomic_rules: Counted,
            pub(crate) atomic_transfer: TransferLayout,
            pub(crate) reach: Counted,
            pub(crate) service: Scalar,
        }

        fn invalid() -> SelectedInstructionError {
            failure("resource profile lost its authenticated complete core carrier geometry")
        }

        struct Node {
            field_type: StructuralFieldType,
            offset: u32,
            width: u32,
        }

        impl Node {
            fn root(
                declarations: &[StructuralTypeDeclaration],
                root: StructuralTypeId,
            ) -> Result<Self, SelectedInstructionError> {
                let shape =
                    structural_reference_input::shape(root, declarations).ok_or_else(|| {
                        failure("resource profile carrier has no complete native shape")
                    })?;
                Ok(Self {
                    field_type: StructuralFieldType::Structural(root),
                    offset: 0,
                    width: shape.byte_size,
                })
            }

            fn declaration<'types>(
                &self,
                declarations: &'types [StructuralTypeDeclaration],
            ) -> Result<&'types StructuralTypeDeclaration, SelectedInstructionError> {
                find_structural_type(
                    declarations,
                    self.field_type.as_structural().ok_or_else(invalid)?,
                )
                .ok_or_else(invalid)
            }

            fn record<const COUNT: usize>(
                &self,
                declarations: &[StructuralTypeDeclaration],
            ) -> Result<[Node; COUNT], SelectedInstructionError> {
                let declaration = self.declaration(declarations)?;
                let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
                    return Err(failure(
                        "resource profile expected a core record declaration",
                    ));
                };
                if fields.len() != COUNT || fields.iter().any(|field| field.relevance.is_erased()) {
                    return Err(failure(
                        "resource profile record changed its complete field roster",
                    ));
                }
                fields
                    .iter()
                    .map(|field| {
                        let (byte_offset, byte_length) = if let Some(scalar) =
                            field.field_type.scalar_type()
                        {
                            structural_reference_input::field_read(
                                declaration.id,
                                &[],
                                field.id,
                                scalar,
                                declarations,
                            )
                            .ok_or_else(|| {
                                failure("resource profile scalar field has no native storage projection")
                            })?
                        } else {
                            let (selected, offset) = structural_reference_input::project(
                                declaration.id,
                                &[StructuralPathSegment::Field(field.identity)],
                                declarations,
                            )
                            .ok_or_else(|| {
                                failure(
                                    "resource profile structural field has no native storage projection",
                                )
                            })?;
                            if Some(selected) != field.field_type.as_structural() {
                                return Err(failure(
                                    "resource profile field projection changed its carrier identity",
                                ));
                            }
                            let shape =
                                structural_reference_input::field_storage_shape(field, declarations)
                                    .ok_or_else(|| {
                                        failure(
                                            "resource profile structural field has no native storage shape",
                                        )
                                    })?;
                            (offset, shape.byte_size)
                        };
                        Ok(Node {
                            field_type: field.field_type.clone(),
                            offset: super::super::offset(self.offset, u64::from(byte_offset))?,
                            width: byte_length,
                        })
                    })
                    .collect::<Result<Vec<_>, SelectedInstructionError>>()?
                    .try_into()
                    .map_err(|_| invalid())
            }

            fn sum<const CASES: usize>(
                &self,
                declarations: &[StructuralTypeDeclaration],
            ) -> Result<(Scalar, [Vec<Node>; CASES]), SelectedInstructionError> {
                let declaration = self.declaration(declarations)?;
                let StructuralTypeShape::Sum { cases } = &declaration.shape else {
                    return Err(failure("resource profile expected a core sum declaration"));
                };
                if cases.len() != CASES {
                    return Err(failure("resource profile sum changed its case roster"));
                }
                let layout = structural_reference_input::case_layout(declaration.id, declarations)
                    .ok_or_else(invalid)?;
                let payloads = cases
                    .iter()
                    .zip(&layout.cases)
                    .map(|(case, layout)| {
                        if case.fields.len() != layout.fields.len()
                            || case.fields.iter().any(|field| field.relevance.is_erased())
                        {
                            return Err(invalid());
                        }
                        case.fields
                            .iter()
                            .zip(&layout.fields)
                            .map(|(field, layout)| {
                                Ok(Node {
                                    field_type: field.field_type.clone(),
                                    offset: super::super::offset(
                                        self.offset,
                                        u64::from(layout.byte_offset),
                                    )?,
                                    width: layout.shape.byte_size,
                                })
                            })
                            .collect::<Result<Vec<_>, SelectedInstructionError>>()
                    })
                    .collect::<Result<Vec<_>, SelectedInstructionError>>()?;
                Ok((
                    Scalar {
                        offset: super::super::offset(
                            self.offset,
                            u64::from(layout.tag_byte_offset),
                        )?,
                        width: layout.tag_shape.byte_size,
                    },
                    payloads.try_into().map_err(|_| invalid())?,
                ))
            }

            fn tag<const CASES: usize>(
                &self,
                declarations: &[StructuralTypeDeclaration],
            ) -> Result<Scalar, SelectedInstructionError> {
                let (tag, payloads) = self.sum::<CASES>(declarations)?;
                if payloads.iter().any(|payload| !payload.is_empty()) {
                    return Err(failure(
                        "resource profile enum unexpectedly carries payload fields",
                    ));
                }
                Ok(tag)
            }

            fn scalar(
                &self,
                declarations: &[StructuralTypeDeclaration],
                boolean: bool,
            ) -> Result<Scalar, SelectedInstructionError> {
                let scalar = match &self.field_type {
                    StructuralFieldType::Structural(_) => {
                        let StructuralTypeShape::PrimitiveScalar(scalar) =
                            self.declaration(declarations)?.shape
                        else {
                            return Err(invalid());
                        };
                        scalar
                    }
                    field => field.scalar_type().ok_or_else(invalid)?,
                };
                let valid = if boolean {
                    scalar == ScalarType::Boolean && self.width == 1
                } else {
                    matches!(scalar, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64)
                        && self.width == 8
                };
                if !valid {
                    return Err(failure(
                        "resource profile scalar differs from its core Boolean or u64 carrier",
                    ));
                }
                Ok(Scalar {
                    offset: self.offset,
                    width: self.width,
                })
            }

            fn counted(
                &self,
                declarations: &[StructuralTypeDeclaration],
                capacity: u64,
            ) -> Result<(Counted, Node), SelectedInstructionError> {
                let [entries, count] = self.record(declarations)?;
                let StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } = entries.declaration(declarations)?.shape
                else {
                    return Err(failure(
                        "resource profile counted carrier has no fixed array",
                    ));
                };
                if length != capacity {
                    return Err(failure(
                        "resource profile counted array changed its core capacity",
                    ));
                }
                let element = Self::root(declarations, element)?;
                let stride = element.width;
                if u64::from(stride).checked_mul(capacity) != Some(u64::from(entries.width)) {
                    return Err(failure(
                        "resource profile fixed array size differs from its element stride",
                    ));
                }
                Ok((
                    Counted {
                        offset: entries.offset,
                        stride,
                        capacity,
                        count: count.scalar(declarations, false)?,
                    },
                    element,
                ))
            }
        }

        impl ProfileLayout {
            pub(crate) fn derive(
                declarations: &[StructuralTypeDeclaration],
                root: StructuralTypeId,
            ) -> Result<Self, SelectedInstructionError> {
                let profile = Node::root(declarations, root)?;
                let (regions, region) = profile.counted(declarations, 32)?;
                let [range, peer, stable, external, atomic, reach] = region.record(declarations)?;
                let [start, length] = range.record(declarations)?;
                let (external_tag, [external_none, external_access]) =
                    external.sum(declarations)?;
                let (atomic_tag, [atomic_none, atomic_access]) = atomic.sum(declarations)?;
                if !external_none.is_empty() || !atomic_none.is_empty() {
                    return Err(invalid());
                }
                let [external_read, external_write, external_rules]: [Node; 3] =
                    external_access.try_into().map_err(|_| invalid())?;
                let [atomic_rules]: [Node; 1] = atomic_access.try_into().map_err(|_| invalid())?;
                let (external_rules, transfer) = external_rules.counted(declarations, 16)?;
                let [width, alignment] = transfer.record(declarations)?;
                let external_transfer = TransferLayout {
                    width: width.scalar(declarations, false)?,
                    alignment: alignment.scalar(declarations, false)?,
                    operations: None,
                };
                let (atomic_rules, atomic_transfer) = atomic_rules.counted(declarations, 16)?;
                let [transfer, operations] = atomic_transfer.record(declarations)?;
                let [width, alignment] = transfer.record(declarations)?;
                let operations = operations
                    .record::<12>(declarations)?
                    .iter()
                    .map(|operation| operation.scalar(declarations, true))
                    .collect::<Result<Vec<_>, _>>()?
                    .try_into()
                    .map_err(|_| invalid())?;
                let atomic_transfer = TransferLayout {
                    width: width.scalar(declarations, false)?,
                    alignment: alignment.scalar(declarations, false)?,
                    operations: Some(operations),
                };
                let (reach, service) = reach.counted(declarations, 32)?;
                Ok(Self {
                    bytes: profile.width,
                    regions,
                    start: start.scalar(declarations, false)?,
                    length: length.scalar(declarations, false)?,
                    peer: peer.tag::<2>(declarations)?,
                    stable: stable.tag::<4>(declarations)?,
                    external: external_tag,
                    external_read: external_read.tag::<3>(declarations)?,
                    external_write: external_write.scalar(declarations, true)?,
                    external_rules,
                    external_transfer,
                    atomic: atomic_tag,
                    atomic_rules,
                    atomic_transfer,
                    reach,
                    service: service.scalar(declarations, false)?,
                })
            }
        }
    }
    #[cfg(test)]
    pub(crate) mod tests {
        use super::super::tests::{execute_operation, targets, with_builder};
        use super::*;
        use access_plans::{
            AtomicCapability, AtomicPermissions, AtomicTransferRule, BoundaryReach,
            BoundaryServiceReachId, ExternalCapability, ExternalReadBehavior, PeerWritability,
            ResourceProfile, ResourceRegion, StableCapability, TransferRule,
        };
        use selected_instructions::{SelectedBlockId, SelectedBlockOrigin};
        use semantic_vocabulary::{
            BlockId, IntegerSign, IntegerType, PlaceId, ScalarType, StructuralCaseId,
            StructuralFieldId, StructuralTypeId,
        };
        use terminal_psi::{
            BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
            StructuralFieldType, StructuralRecordStorage, StructuralTypeDeclaration,
            StructuralTypeShape, TerminalIdentityIndex,
        };

        pub(crate) mod initialization {
            use super::*;
            use abstract_operations::{AccessLifecycleOperands, StructuralTypeCatalog};
            use calling_conventions::{
                CallPlan, CallingPolicy, EntryControl, RegisterSet, ValuePlacement,
            };
            use legalized_operations::{
                LegalizedCallUnitParameter, LegalizedScalarFunction,
                LegalizedScalarInstructionKind, LegalizedStructuralContract,
            };
            use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
            use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralDomainId};
            use terminal_psi::{
                StructuralAccess, StructuralArgument, StructuralMultiplicity,
                StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
                TerminalAccessLifecycleExecution, TerminalAccessLifecycleOperation,
                TerminalAccessLifecyclePlan, TerminalAccessStorage,
            };

            #[derive(Clone, Copy, PartialEq, Eq)]
            enum InputProjection {
                Root,
                Field,
                Indexed,
            }

            pub(crate) fn call_plan() -> CallPlan {
                CallPlan {
                    policy: CallingPolicy::MicrosoftX64,
                    parameters: vec![],
                    result: None,
                    callback_materializations: vec![],
                    ordinary_clobbers: RegisterSet::new([]),
                    stack_alignment: 16,
                    shadow_bytes: 0,
                    entry_control: EntryControl::CallReturn,
                }
            }

            pub(crate) fn aligned_placement() -> access_plans::ValidatedAccessPlan {
                use access_plans::{
                    AccessExposure, AccessFields, AccessPlan, FieldAccess, validate_access_plan,
                };
                let mut layout = placement(
                    0,
                    FieldAccess::Stable {
                        transfer_width_bits: 64,
                        read: true,
                        write: true,
                        exposure: AccessExposure::Exported,
                    },
                )
                .layout()
                .clone();
                layout.align = 8;
                let mut access = AccessFields::inaccessible(&layout).unwrap();
                access
                    .set(
                        access.entries()[0].key(),
                        FieldAccess::Stable {
                            transfer_width_bits: 64,
                            read: true,
                            write: true,
                            exposure: AccessExposure::Exported,
                        },
                    )
                    .unwrap();
                validate_access_plan(AccessPlan {
                    layout,
                    access,
                    reach: BoundaryReach::default(),
                })
                .unwrap()
            }

            #[test]
            fn initializer_emits_original_inputs_complete_image_and_typed_outcomes() {
                let mut types = Types::default();
                let profile_type = types.profile();
                let word = types.add(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )));
                let owner = types.record(&[word, word, word]);
                let schema = types.record(&[word]);
                let returned = types.record(&[owner, schema]);
                let reason = types.sum(&[&[word, word], &[word, word], &[]]);
                let result_type = types.sum(&[&[owner], &[returned, reason]]);
                let owner_wrapper = types.record(&[word, owner]);
                let value_wrapper = types.record(&[word, schema]);
                let owner_array = types.add(StructuralTypeShape::FixedArray {
                    element: owner,
                    length: terminal_psi::ArrayExtent::Literal(2),
                });
                let value_array = types.add(StructuralTypeShape::FixedArray {
                    element: schema,
                    length: terminal_psi::ArrayExtent::Literal(3),
                });
                let borrowed_profile = types.add(StructuralTypeShape::Reference {
                    referent: profile_type,
                    access: StructuralAccess::SharedBorrow,
                });
                types.expose_scalar_fields();
                let profile_layout = ProfileLayout::derive(&types.0, profile_type).unwrap();
                let boundary = BoundaryMachineId::new(8).unwrap();
                let plan = TerminalAccessLifecyclePlan {
                    target: None,
                    boundary,
                    operation: TerminalAccessLifecycleOperation::Initialize,
                    execution: TerminalAccessLifecycleExecution::CompilerPrimitive {
                        profile_type: Some(profile_type),
                        profile_requirements: Vec::new(),
                    },
                    storage: TerminalAccessStorage {
                        owner: terminal_psi::TerminalAccessOwner::Extent,
                        carrier: owner,
                        base: StructuralFieldId::new(3).unwrap(),
                        length: StructuralFieldId::new(2).unwrap(),
                        granted: StructuralDomainId::new(1).unwrap(),
                        vacant: Some(StructuralDomainId::new(2).unwrap()),
                        resident: Some(StructuralDomainId::new(3).unwrap()),
                    },
                    schema,
                    image_fields: vec![StructuralFieldId::new(1).unwrap()],
                    policy_identity: TerminalIdentityIndex(10),
                    policy_plan_machine_identity: TerminalIdentityIndex(11),
                    schema_identity: TerminalIdentityIndex(12),
                    placement: aligned_placement(),
                };
                for target in targets() {
                    for projection in [
                        InputProjection::Root,
                        InputProjection::Field,
                        InputProjection::Indexed,
                    ] {
                        with_builder(target, |builder, original| {
                            let projected = projection != InputProjection::Root;
                            let owner_place = PlaceId::new(1).unwrap();
                            let value_place = PlaceId::new(2).unwrap();
                            let profile_place = PlaceId::new(3).unwrap();
                            let result_place = PlaceId::new(4).unwrap();
                            let mut row = original.clone();
                            let argument = |place, access, path| StructuralArgument {
                                place,
                                access,
                                path,
                            };
                            let field_path = if projection == InputProjection::Field {
                                vec![StructuralPathSegment::Field(TerminalIdentityIndex(2))]
                            } else {
                                vec![]
                            };
                            let index = |number, stride, extent| {
                                legalized_operations::LegalizedRuntimeIndexOperand {
                                    operand: abstract_operations::AbstractResult {
                                        value: semantic_vocabulary::ValueId::new(number).unwrap(),
                                        scalar_type: ScalarType::Integer(
                                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                                        ),
                                    },
                                    stride,
                                    extent: Some(extent),
                                    obligation: semantic_vocabulary::ObligationId::new(number).unwrap(),
                                    accepted_fact:
                                        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                                            b"initializer-index",
                                        ),
                                }
                            };
                            let selectors = if projection == InputProjection::Indexed {
                                vec![index(7, 24, 2), index(8, 8, 3)]
                            } else {
                                vec![]
                            };
                            let path = |position: usize| {
                                selectors.get(position).map_or_else(
                                    || field_path.clone(),
                                    |selector| {
                                        vec![StructuralPathSegment::RuntimeIndex {
                                            index: selector.operand.value,
                                            obligation: selector.obligation,
                                        }]
                                    },
                                )
                            };
                            row.kind = LegalizedScalarInstructionKind::AccessLifecycle {
                                boundary,
                                operands: AccessLifecycleOperands::Initialize {
                                    storage: argument(
                                        owner_place,
                                        StructuralAccess::Owned,
                                        path(0),
                                    ),
                                    value: argument(value_place, StructuralAccess::Owned, path(1)),
                                    profile: argument(
                                        profile_place,
                                        StructuralAccess::SharedBorrow,
                                        if projected {
                                            vec![StructuralPathSegment::Referent]
                                        } else {
                                            vec![]
                                        },
                                    ),
                                },
                                indices: selectors.clone(),
                                result: StructuralOperationResult {
                                    place: result_place,
                                    structural_type: result_type,
                                    multiplicity: StructuralMultiplicity::Linear,
                                    qualifications: vec![],
                                    projected_qualifications: vec![],
                                    additional_input_qualifications: Vec::new(),
                                    qualification_establishments: vec![],
                                    claims: vec![],
                                },
                            };
                            let parameter_types = [
                                match projection {
                                    InputProjection::Root => owner,
                                    InputProjection::Field => owner_wrapper,
                                    InputProjection::Indexed => owner_array,
                                },
                                match projection {
                                    InputProjection::Root => schema,
                                    InputProjection::Field => value_wrapper,
                                    InputProjection::Indexed => value_array,
                                },
                                if projected {
                                    borrowed_profile
                                } else {
                                    profile_type
                                },
                            ];
                            let parameters = [owner_place, value_place, profile_place]
                                .into_iter()
                                .zip(parameter_types)
                                .enumerate()
                                .map(|(position, (place, structural_type))| {
                                    let shape =
                                        crate::structural_inputs::shape(structural_type, &types.0)
                                            .unwrap();
                                    let semantic = StructuralParameterDeclaration {
                                        place,
                                        position: position as u32,
                                        structural_type,
                                        multiplicity: StructuralMultiplicity::Unrestricted,
                                        access: StructuralAccess::Owned,
                                        qualifications: vec![],
                                        projected_qualifications: vec![],
                                        additional_input_qualifications: Vec::new(),
                                    };
                                    LegalizedCallUnitParameter {
                                        target: target_operations::TargetStructuralParameter {
                                            place,
                                            structural_type,
                                            multiplicity: semantic.multiplicity,
                                            access: semantic.access,
                                            projected_qualifications: vec![],
                                            shape,
                                            placement: ValuePlacement {
                                                shape,
                                                locations: vec![],
                                            },
                                        },
                                        semantic,
                                    }
                                })
                                .collect();
                            let source = LegalizedScalarFunction {
                                machine: MachineId::new(1).unwrap(),
                                attachment: None,
                                nominal_cleanup_subject: None,
                                provenance: target_operations::TerminalPsiProvenance {
                                    operations: vec![row.operation],
                                    edges: vec![],
                                },
                                call_plan: call_plan(),
                                parameters: vec![],
                                structural: Some(LegalizedStructuralContract {
                                    result: None,
                                    structural_types: StructuralTypeCatalog::from(types.0.clone())
                                        .with_access_lifecycles(vec![plan.clone()])
                                        .unwrap(),
                                    parameters,
                                    structural_places: vec![],
                                    entry_claims: vec![],
                                    published_service_ceiling: vec![],
                                }),
                                entry_block: BlockId::new(7).unwrap(),
                                blocks: vec![],
                                ownership_frontier_facts: vec![],
                            };
                            builder.open_block = Some(operation_control::OpenBlock {
                                id: SelectedBlockId(0),
                                origin: SelectedBlockOrigin::Source(source.entry_block),
                                start: 0,
                            });
                            builder.next_block = 1;
                            for (position, selector) in selectors.iter().enumerate() {
                                let register = fragmented_scalar::constant(
                                    builder,
                                    &row,
                                    value_place,
                                    position as u64 + 1,
                                )
                                .unwrap();
                                builder.definitions.push((
                                    selector.operand.value,
                                    register,
                                    optimization_unit::ValueDefinitionSite::FunctionParameter(
                                        position as u32,
                                    ),
                                    selector.operand.scalar_type,
                                ));
                            }
                            let arena = (u64::from(profile_layout.bytes) + 255) & !255;
                            for (place, address) in [
                                (owner_place, arena),
                                (value_place, arena + 64),
                                (profile_place, if projected { arena + 128 } else { 0 }),
                            ] {
                                let pointer =
                                    fragmented_scalar::constant(builder, &row, place, address)
                                        .unwrap();
                                builder.transport.pointers.push((place, pointer));
                            }
                            super::super::super::emit(&source, &row, builder).unwrap();
                            let backing_writes = builder
                                .transport
                                .memory
                                .iter()
                                .filter(|access| {
                                    access.role == selected_instructions::SelectedMemoryAccessRole::WriteBacking
                                })
                                .collect::<Vec<_>>();
                            let [write] = backing_writes.as_slice() else {
                                panic!("initialization must declare one complete backing write");
                            };
                            assert_eq!(
                                (write.place, write.byte_offset, write.byte_count),
                                (owner_place, 0, 16)
                            );
                            assert!(builder.transport.memory.iter().any(|read| {
                                read.instruction == write.instruction
                                    && read.role
                                        == selected_instructions::SelectedMemoryAccessRole::ReadLocal {
                                            slot: LocalStorageSlotId::Boundary {
                                                operation: row.operation,
                                            },
                                        }
                                    && read.byte_offset
                                        == super::super::compatibility::COMPATIBILITY_SCRATCH_BYTES
                                    && read.byte_count == write.byte_count
                            }));
                            let frame = [
                                (
                                    FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
                                        operation: row.operation,
                                        place: result_place,
                                    }),
                                    arena + 256,
                                ),
                                (
                                    FrameStorageSlotId::Local(LocalStorageSlotId::Boundary {
                                        operation: row.operation,
                                    }),
                                    arena + 512,
                                ),
                            ];
                            assert_eq!(builder.transport.local_slots.len(), 2);
                            let result_pointer = builder
                                .transport
                                .pointers
                                .iter()
                                .find(|(place, _)| *place == result_place)
                                .unwrap()
                                .1;
                            for outcome in 0..5 {
                                let available = if outcome == 1 { 8 } else { 64 };
                                let mut supplied = region(0, available);
                                if outcome == 2 {
                                    supplied.stable = StableCapability::Read;
                                }
                                let mut bytes = encode(
                                    &profile_layout,
                                    &ResourceProfile {
                                        regions: vec![supplied],
                                    },
                                );
                                bytes.resize((arena + 2048) as usize, 0xa5);
                                if outcome == 4 {
                                    store(&mut bytes, 0, profile_layout.regions.count, 33);
                                }
                                let owner_start = arena as usize
                                    + match projection {
                                        InputProjection::Root => 0,
                                        InputProjection::Field => 8,
                                        InputProjection::Indexed => 24,
                                    };
                                let value_start = arena as usize
                                    + 64
                                    + match projection {
                                        InputProjection::Root => 0,
                                        InputProjection::Field => 8,
                                        InputProjection::Indexed => 16,
                                    };
                                let backing = arena + 1024 + u64::from(outcome == 3);
                                bytes[owner_start..owner_start + 8]
                                    .copy_from_slice(&0xfeed_u64.to_le_bytes());
                                bytes[owner_start + 8..owner_start + 16]
                                    .copy_from_slice(&available.to_le_bytes());
                                bytes[owner_start + 16..owner_start + 24]
                                    .copy_from_slice(&backing.to_le_bytes());
                                bytes[value_start..value_start + 8]
                                    .copy_from_slice(&17_u64.to_le_bytes());
                                bytes[arena as usize + 128..arena as usize + 136]
                                    .copy_from_slice(&0_u64.to_le_bytes());
                                let original = bytes.clone();
                                let registers = execute_operation(builder, &mut bytes, &frame);
                                let backing = backing as usize;
                                assert_eq!(
                                    &bytes[..arena as usize + 256],
                                    &original[..arena as usize + 256]
                                );
                                assert_eq!(&bytes[backing + 16..], &original[backing + 16..]);
                                if outcome == 4 {
                                    assert!(
                                        registers.is_none(),
                                        "malformed supply must not return Rejected"
                                    );
                                    assert_eq!(
                                        &bytes[backing..backing + 16],
                                        &original[backing..backing + 16]
                                    );
                                    continue;
                                }
                                let registers = registers.unwrap();
                                let result_start = registers[result_pointer.0 as usize] as usize;
                                let read = |offset| {
                                    u64::from_le_bytes(
                                        bytes[result_start + offset..result_start + offset + 8]
                                            .try_into()
                                            .unwrap(),
                                    )
                                };
                                assert_eq!(read(0) & 0xffff_ffff, u64::from(outcome != 0));
                                assert_eq!(
                                    &bytes[result_start + 8..result_start + 32],
                                    &original[owner_start..owner_start + 24]
                                );
                                if outcome == 0 {
                                    assert_eq!(&bytes[backing..backing + 8], &17_u64.to_le_bytes());
                                    assert_eq!(&bytes[backing + 8..backing + 16], &[0; 8]);
                                } else {
                                    assert_eq!(read(32), 17);
                                    assert_eq!(
                                        read(40) & 0xffff_ffff,
                                        match outcome {
                                            1 => 0,
                                            2 => 2,
                                            _ => 1,
                                        }
                                    );
                                    if outcome == 1 {
                                        assert_eq!((read(48), read(56)), (16, available));
                                    }
                                    if outcome == 3 {
                                        assert_eq!((read(48), read(56)), (8, 0));
                                    }
                                    assert_eq!(
                                        &bytes[backing..backing + 16],
                                        &original[backing..backing + 16]
                                    );
                                }
                            }
                            if projection == InputProjection::Indexed {
                                for missing in [false, true] {
                                    let mut invalid = row.clone();
                                    let LegalizedScalarInstructionKind::AccessLifecycle {
                                        indices,
                                        ..
                                    } = &mut invalid.kind
                                    else {
                                        unreachable!()
                                    };
                                    if missing {
                                        indices.pop();
                                    } else {
                                        indices.swap(0, 1);
                                    }
                                    with_builder(target, |builder, _| {
                                        let error =
                                            super::super::super::emit(&source, &invalid, builder)
                                                .unwrap_err();
                                        assert!(format!("{error:?}").contains("selector"));
                                    });
                                }
                            }
                        });
                    }
                }
            }
        }
        pub(crate) mod run_relocation {
            use super::*;
            use compatibility::run::{RunGeometry, SupplyRole};

            #[test]
            fn run_supply_checks_cover_the_last_cell_without_walking_the_run() {
                let placement = initialization::aligned_placement();
                let footprint = placement.layout().size.unwrap();
                let mut declarations = Types::default();
                let profile_type = declarations.profile();
                declarations.expose_scalar_fields();
                let layout = ProfileLayout::derive(&declarations.0, profile_type).unwrap();

                for target in targets() {
                    for (count, base, length) in [
                        (1_000_000_000, 4096, 1_000_000_000 * footprint),
                        (1, 4097, footprint),
                        (2, 4096, footprint),
                        (1, u64::MAX - 7, footprint),
                        (0, 0, 0),
                    ] {
                        with_builder(target, |builder, row| {
                            let profile = profile_home(builder, row);
                            let scratch = ImageHome::scratch(
                                builder,
                                row,
                                profile.place,
                                compatibility::run::SCRATCH_BYTES,
                            )
                            .unwrap();
                            let geometry = RunGeometry {
                                count: fragmented_scalar::constant(
                                    builder,
                                    row,
                                    profile.place,
                                    count,
                                )
                                .unwrap(),
                                base: fragmented_scalar::constant(
                                    builder,
                                    row,
                                    profile.place,
                                    base,
                                )
                                .unwrap(),
                                length: fragmented_scalar::constant(
                                    builder,
                                    row,
                                    profile.place,
                                    length,
                                )
                                .unwrap(),
                            };
                            let assessments =
                                [SupplyRole::Source, SupplyRole::Destination].map(|role| {
                                    compatibility::run::assess(
                                        &declarations.0,
                                        profile_type,
                                        builder,
                                        row,
                                        profile,
                                        scratch,
                                        geometry,
                                        &placement,
                                        role,
                                    )
                                    .unwrap()
                                });
                            let valid = ResourceProfile {
                                regions: if length == 0 {
                                    vec![]
                                } else {
                                    vec![region(0, length)]
                                },
                            };
                            let mut profiles = vec![valid.clone()];
                            if length >= footprint {
                                for stable in [StableCapability::Read, StableCapability::Write] {
                                    let mut last = region(length - footprint, footprint);
                                    last.stable = stable;
                                    let mut regions = vec![last];
                                    if length > footprint {
                                        regions.push(region(0, length - footprint));
                                    }
                                    profiles.push(ResourceProfile { regions });
                                }
                            }
                            let source =
                                access_plans::validate_resource_profile(valid, length).unwrap();
                            for profile in profiles {
                                let normalized = access_plans::validate_resource_profile(
                                    profile.clone(),
                                    length,
                                )
                                .unwrap();
                                let geometry_valid = count.checked_mul(footprint) == Some(length)
                                    && base.checked_add(length).is_some();
                                let source_valid = geometry_valid
                                    && access_plans::require_stable_region_supply(
                                        &normalized,
                                        0,
                                        length,
                                        true,
                                        false,
                                        placement.reach(),
                                        "test source",
                                    )
                                    .is_ok();
                                let destination_valid = geometry_valid
                                    && access_plans::validate_run_relocation_resources(
                                        &placement,
                                        count,
                                        base,
                                        &source,
                                        &normalized,
                                    )
                                    .is_ok();
                                let mut bytes = encode(&layout, &profile);
                                bytes.resize(
                                    layout.bytes as usize
                                        + compatibility::run::SCRATCH_BYTES as usize
                                        + 16,
                                    0xd7,
                                );
                                let registers =
                                    run_values(builder, &mut bytes, u64::from(layout.bytes) + 8);
                                for (assessment, expected) in
                                    assessments.iter().zip([source_valid, destination_valid])
                                {
                                    assert_eq!(registers[assessment.well_formed.0 as usize], 1);
                                    assert_eq!(
                                        registers[assessment.compatible.0 as usize],
                                        u64::from(expected),
                                        "count={count}, base={base}, length={length}, profile={profile:?}"
                                    );
                                }
                            }
                        });
                    }
                }
            }
        }

        #[derive(Default)]
        struct Types(Vec<StructuralTypeDeclaration>);

        impl Types {
            fn expose_scalar_fields(&mut self) {
                let scalars: Vec<_> = self
                    .0
                    .iter()
                    .filter_map(|declaration| match declaration.shape {
                        StructuralTypeShape::PrimitiveScalar(scalar) => {
                            Some((declaration.id, scalar))
                        }
                        _ => None,
                    })
                    .collect();
                let lower = |field: &mut StructuralFieldDeclaration| {
                    if let Some((_, scalar)) = scalars
                        .iter()
                        .find(|(identity, _)| field.field_type.as_structural() == Some(*identity))
                    {
                        field.field_type = StructuralFieldType::Scalar(*scalar);
                    }
                };
                for declaration in &mut self.0 {
                    match &mut declaration.shape {
                        StructuralTypeShape::Record { fields, .. } => {
                            fields.iter_mut().for_each(lower)
                        }
                        StructuralTypeShape::Sum { cases } => cases
                            .iter_mut()
                            .flat_map(|case| &mut case.fields)
                            .for_each(lower),
                        _ => {}
                    }
                }
            }
            fn add(&mut self, shape: StructuralTypeShape) -> StructuralTypeId {
                let id = StructuralTypeId::new((self.0.len() + 1).try_into().unwrap()).unwrap();
                self.0.push(StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id,
                    identity: TerminalIdentityIndex(id.get().try_into().unwrap()),
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    copyable: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    shape,
                });
                id
            }

            fn fields(types: &[StructuralTypeId]) -> Vec<StructuralFieldDeclaration> {
                types
                    .iter()
                    .enumerate()
                    .map(|(position, field_type)| StructuralFieldDeclaration {
                        inline_carrier: None,
                        id: StructuralFieldId::new((position + 1).try_into().unwrap()).unwrap(),
                        identity: TerminalIdentityIndex((position + 1).try_into().unwrap()),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(*field_type),
                        storage: None,
                    })
                    .collect()
            }

            fn record(&mut self, types: &[StructuralTypeId]) -> StructuralTypeId {
                self.add(StructuralTypeShape::Record {
                    fields: Self::fields(types),
                    storage: StructuralRecordStorage::Packed,
                })
            }

            fn sum(&mut self, payloads: &[&[StructuralTypeId]]) -> StructuralTypeId {
                self.add(StructuralTypeShape::Sum {
                    cases: payloads
                        .iter()
                        .enumerate()
                        .map(|(position, payload)| StructuralCaseDeclaration {
                            id: StructuralCaseId::new((position + 1).try_into().unwrap()).unwrap(),
                            identity: TerminalIdentityIndex((position + 1).try_into().unwrap()),
                            fields: Self::fields(payload),
                        })
                        .collect(),
                })
            }

            fn counted(
                &mut self,
                element: StructuralTypeId,
                count: StructuralTypeId,
                capacity: u64,
            ) -> StructuralTypeId {
                let array = self.add(StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(capacity),
                });
                self.record(&[array, count])
            }

            fn profile(&mut self) -> StructuralTypeId {
                let word = self.add(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )));
                let boolean = self.add(StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean));
                let range = self.record(&[word, word]);
                let peer = self.sum(&[&[], &[]]);
                let stable = self.sum(&[&[], &[], &[], &[]]);
                let read = self.sum(&[&[], &[], &[]]);
                let transfer = self.record(&[word, word]);
                let rules = self.counted(transfer, word, 16);
                let external = self.sum(&[&[], &[read, boolean, rules]]);
                let operations = self.record(&[boolean; 12]);
                let atomic_transfer = self.record(&[transfer, operations]);
                let atomic_rules = self.counted(atomic_transfer, word, 16);
                let atomic = self.sum(&[&[], &[atomic_rules]]);
                let reach = self.counted(word, word, 32);
                let region = self.record(&[range, peer, stable, external, atomic, reach]);
                self.counted(region, word, 32)
            }
        }

        fn store(bytes: &mut [u8], base: usize, scalar: Scalar, value: u64) {
            let start = base + scalar.offset as usize;
            bytes[start..start + scalar.width as usize]
                .copy_from_slice(&value.to_le_bytes()[..scalar.width as usize]);
        }

        fn entry(base: usize, array: Counted, position: usize) -> usize {
            base + array.offset as usize + position * array.stride as usize
        }

        fn encode(layout: &ProfileLayout, profile: &ResourceProfile) -> Vec<u8> {
            let mut bytes =
                vec![0xd7; layout.bytes as usize + VALIDATION_SCRATCH_BYTES as usize + 16];
            macro_rules! s {
                ($base:expr => $scalar:expr => $value:expr) => {
                    store(&mut bytes, $base, $scalar, $value)
                };
            }

            s!(0 => layout.regions.count => profile.regions.len() as u64);
            for (position, region) in profile.regions.iter().enumerate() {
                let base = entry(0, layout.regions, position);
                store(&mut bytes, base, layout.start, region.offset);
                store(&mut bytes, base, layout.length, region.length);
                store(
                    &mut bytes,
                    base,
                    layout.peer,
                    match region.peer {
                        PeerWritability::Exclusive => 0,
                        PeerWritability::HostileShared => 1,
                    },
                );
                store(
                    &mut bytes,
                    base,
                    layout.stable,
                    match region.stable {
                        StableCapability::None => 0,
                        StableCapability::Read => 1,
                        StableCapability::Write => 2,
                        StableCapability::ReadWrite => 3,
                    },
                );
                match &region.external {
                    ExternalCapability::None => store(&mut bytes, base, layout.external, 0),
                    ExternalCapability::Access {
                        read,
                        write,
                        transfers,
                    } => {
                        store(&mut bytes, base, layout.external, 1);
                        store(
                            &mut bytes,
                            base,
                            layout.external_read,
                            match read {
                                ExternalReadBehavior::None => 0,
                                ExternalReadBehavior::Repeatable => 1,
                                ExternalReadBehavior::Destructive => 2,
                            },
                        );
                        store(&mut bytes, base, layout.external_write, u64::from(*write));
                        s!(base => layout.external_rules.count => transfers.len() as u64);
                        for (position, rule) in transfers.iter().enumerate() {
                            let base = entry(base, layout.external_rules, position);
                            s!(base => layout.external_transfer.width => u64::from(rule.width_bits));
                            s!(base => layout.external_transfer.alignment => rule.alignment_bytes);
                        }
                    }
                }
                match &region.atomic {
                    AtomicCapability::None => store(&mut bytes, base, layout.atomic, 0),
                    AtomicCapability::Access { transfers } => {
                        store(&mut bytes, base, layout.atomic, 1);
                        s!(base => layout.atomic_rules.count => transfers.len() as u64);
                        for (position, rule) in transfers.iter().enumerate() {
                            let base = entry(base, layout.atomic_rules, position);
                            s!(base => layout.atomic_transfer.width => u64::from(rule.transfer.width_bits));
                            s!(base => layout.atomic_transfer.alignment => rule.transfer.alignment_bytes);
                            let operations = rule.operations;
                            for (field, value) in layout
                                .atomic_transfer
                                .operations
                                .unwrap()
                                .into_iter()
                                .zip(compatibility::atomic_bits(operations))
                            {
                                store(&mut bytes, base, field, u64::from(value));
                            }
                        }
                    }
                }
                s!(base => layout.reach.count => region.reach.services().len() as u64);
                for (position, service) in region.reach.services().enumerate() {
                    store(
                        &mut bytes,
                        entry(base, layout.reach, position),
                        layout.service,
                        service.normalized_identity(),
                    );
                }
            }
            bytes
        }

        fn run_values(builder: &Builder<'_>, bytes: &mut [u8], scratch_base: u64) -> Vec<u64> {
            let [slot] = builder.transport.local_slots.as_slice() else {
                panic!("profile assessment must own one real scratch allocation");
            };
            let frame = [(
                selected_instructions::FrameStorageSlotId::Local(slot.id),
                scratch_base,
            )];
            assert!(scratch_base + u64::from(slot.byte_size) <= bytes.len() as u64);
            let mut reads = 0;
            let mut writes = 0;
            for access in &builder.transport.memory {
                use selected_instructions::SelectedMemoryAccessRole;
                match access.role {
                    SelectedMemoryAccessRole::ReadLocal { slot: owner }
                    | SelectedMemoryAccessRole::WriteLocal { slot: owner }
                    | SelectedMemoryAccessRole::AddressLocal { slot: owner } => {
                        assert_eq!(owner, slot.id);
                        assert!(access.byte_offset + access.byte_count <= slot.byte_size);
                        reads += usize::from(matches!(
                            access.role,
                            SelectedMemoryAccessRole::ReadLocal { .. }
                        ));
                        writes += usize::from(matches!(
                            access.role,
                            SelectedMemoryAccessRole::WriteLocal { .. }
                        ));
                    }
                    SelectedMemoryAccessRole::ReadPlace => {}
                    unexpected => {
                        panic!("profile validation invented a source access: {unexpected:?}")
                    }
                }
            }
            assert!(reads > 0 && writes > 0);
            execute_operation(builder, bytes, &frame).expect("assessment itself does not trap")
        }

        fn run(
            builder: &Builder<'_>,
            result: VirtualRegisterId,
            bytes: &mut [u8],
            scratch_base: u64,
        ) -> bool {
            let registers = run_values(builder, bytes, scratch_base);
            assert!(registers[result.0 as usize] <= 1);
            registers[result.0 as usize] == 1
        }

        fn profile_home(
            builder: &mut Builder<'_>,
            row: &LegalizedScalarInstruction,
        ) -> ImageHome<'static> {
            builder.open_block = Some(operation_control::OpenBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(7).unwrap()),
                start: 0,
            });
            builder.next_block = 1;
            let place = PlaceId::new(19).unwrap();
            let pointer = fragmented_scalar::constant(builder, row, place, 0).unwrap();
            ImageHome {
                storage: fragmented_scalar::StorageAccess::Place {
                    indices: &[],
                    byte_offset: 0,
                },
                place,
                pointer,
                byte_offset: 0,
            }
        }

        fn with_validator(
            available: u64,
            test: impl Fn(&Builder<'_>, VirtualRegisterId, &ProfileLayout),
        ) {
            let mut declarations = Types::default();
            let profile_type = declarations.profile();
            declarations.expose_scalar_fields();
            let layout = ProfileLayout::derive(&declarations.0, profile_type).unwrap();
            for target in targets() {
                with_builder(target, |builder, row| {
                    let profile = profile_home(builder, row);
                    let place = profile.place;
                    let scratch =
                        ImageHome::scratch(builder, row, place, VALIDATION_SCRATCH_BYTES).unwrap();
                    let available =
                        fragmented_scalar::constant(builder, row, place, available).unwrap();
                    let result = validate(
                        &declarations.0,
                        profile_type,
                        builder,
                        row,
                        profile,
                        scratch,
                        available,
                    )
                    .unwrap();
                    test(builder, result, &layout);
                });
            }
        }

        fn region(offset: u64, length: u64) -> ResourceRegion {
            ResourceRegion {
                offset,
                length,
                peer: PeerWritability::Exclusive,
                stable: StableCapability::ReadWrite,
                external: ExternalCapability::None,
                atomic: AtomicCapability::None,
                reach: BoundaryReach::from_services([
                    BoundaryServiceReachId::from_normalized_identity(17).unwrap(),
                    BoundaryServiceReachId::from_normalized_identity(29).unwrap(),
                ]),
            }
        }

        fn all_operations() -> AtomicPermissions {
            AtomicPermissions {
                load: true,
                store: true,
                fetch_add: true,
                fetch_sub: true,
                fetch_xor: true,
                fetch_or: true,
                fetch_and: true,
                swap: true,
                compare_exchange: true,
                compare_exchange_once: true,
                try_exchange: true,
                try_exchange_once: true,
            }
        }

        #[test]
        fn runtime_profile_validation_matches_complete_normalizer() {
            let rules: Vec<_> = (1..=16)
                .rev()
                .map(|width| TransferRule {
                    width_bits: width * 8,
                    alignment_bytes: 1_u64 << (width % 8),
                })
                .collect();
            let mut rich = region(0, 256);
            rich.external = ExternalCapability::Access {
                read: ExternalReadBehavior::Repeatable,
                write: true,
                transfers: rules.clone(),
            };
            rich.atomic = AtomicCapability::Access {
                transfers: rules
                    .iter()
                    .map(|transfer| AtomicTransferRule {
                        transfer: *transfer,
                        operations: all_operations(),
                    })
                    .collect(),
            };
            let mut cases = vec![
                ResourceProfile::default(),
                ResourceProfile {
                    regions: vec![rich.clone()],
                },
                ResourceProfile {
                    regions: vec![region(128, 128), region(0, 128)],
                },
                ResourceProfile {
                    regions: (0..32)
                        .rev()
                        .map(|position| region(position * 8, 8))
                        .collect(),
                },
            ];
            for mutation in 0..15 {
                let mut changed = rich.clone();
                match mutation {
                    0 => changed.length = 0,
                    1 => changed.offset = u64::MAX,
                    2 => changed.length = 257,
                    3 => changed.peer = PeerWritability::HostileShared,
                    4 => {
                        changed.external = ExternalCapability::Access {
                            read: ExternalReadBehavior::None,
                            write: false,
                            transfers: rules.clone(),
                        }
                    }
                    5 => {
                        changed.external = ExternalCapability::Access {
                            read: ExternalReadBehavior::Destructive,
                            write: false,
                            transfers: vec![],
                        }
                    }
                    6..=10 => {
                        let mut transfers = rules.clone();
                        match mutation {
                            6 => transfers[1] = transfers[0],
                            7 => transfers[0].width_bits = 7,
                            8 => transfers[0].width_bits = 136,
                            9 => transfers[0].alignment_bytes = 0,
                            _ => transfers[0].alignment_bytes = 3,
                        }
                        changed.external = ExternalCapability::Access {
                            read: ExternalReadBehavior::Repeatable,
                            write: true,
                            transfers,
                        };
                    }
                    11 => changed.atomic = AtomicCapability::Access { transfers: vec![] },
                    12 => {
                        changed.atomic = AtomicCapability::Access {
                            transfers: vec![AtomicTransferRule {
                                transfer: rules[0],
                                operations: AtomicPermissions::default(),
                            }],
                        }
                    }
                    13 => {
                        changed.stable = StableCapability::None;
                        changed.external = ExternalCapability::None;
                        changed.atomic = AtomicCapability::None;
                    }
                    _ => {
                        changed.peer = PeerWritability::HostileShared;
                        changed.stable = StableCapability::None;
                    }
                }
                cases.push(ResourceProfile {
                    regions: vec![changed],
                });
            }
            cases.push(ResourceProfile {
                regions: vec![region(0, 129), region(128, 128)],
            });
            with_validator(256, |builder, result, layout| {
                for (position, profile) in cases.iter().enumerate() {
                    let expected =
                        access_plans::validate_resource_profile(profile.clone(), 256).is_ok();
                    let mut bytes = encode(layout, profile);
                    let before = bytes[..layout.bytes as usize + 8].to_vec();
                    assert_eq!(
                        run(builder, result, &mut bytes, u64::from(layout.bytes) + 8),
                        expected,
                        "profile {position}"
                    );
                    assert_eq!(
                        &bytes[..before.len()],
                        before,
                        "validation mutated its immutable input"
                    );
                    assert_eq!(&bytes[bytes.len() - 8..], &[0xd7; 8]);
                }
            });
        }

        #[test]
        fn zero_backing_accepts_only_an_empty_supply_roster() {
            with_validator(0, |builder, result, layout| {
                for profile in [
                    ResourceProfile::default(),
                    ResourceProfile {
                        regions: vec![region(0, 0)],
                    },
                    ResourceProfile {
                        regions: vec![region(0, 1)],
                    },
                ] {
                    let expected = profile.regions.is_empty();
                    assert_eq!(
                        access_plans::validate_resource_profile(profile.clone(), 0).is_ok(),
                        expected
                    );
                    let mut bytes = encode(layout, &profile);
                    assert_eq!(
                        run(builder, result, &mut bytes, u64::from(layout.bytes) + 8),
                        expected
                    );
                }
            });
        }

        #[test]
        fn malformed_runtime_counts_tags_and_boolean_bits_never_enable_profile() {
            with_validator(256, |builder, result, layout| {
                let mut profile = region(0, 256);
                profile.external = ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: true,
                    transfers: vec![TransferRule {
                        width_bits: 64,
                        alignment_bytes: 8,
                    }],
                };
                profile.atomic = AtomicCapability::Access {
                    transfers: vec![AtomicTransferRule {
                        transfer: TransferRule {
                            width_bits: 64,
                            alignment_bytes: 8,
                        },
                        operations: all_operations(),
                    }],
                };
                let clean = encode(
                    layout,
                    &ResourceProfile {
                        regions: vec![profile],
                    },
                );
                let base = entry(0, layout.regions, 0);
                let atomic = entry(base, layout.atomic_rules, 0);
                let service = entry(base, layout.reach, 0);
                let mut faults = vec![
                    (0, layout.regions.count, 33),
                    (0, layout.regions.count, u64::MAX),
                    (base, layout.peer, 2),
                    (base, layout.stable, 4),
                    (base, layout.external, 2),
                    (base, layout.atomic, 2),
                    (base, layout.external_read, 3),
                    (base, layout.external_write, 2),
                    (base, layout.external_rules.count, 17),
                    (base, layout.atomic_rules.count, u64::MAX),
                    (base, layout.reach.count, 33),
                    (service, layout.service, 0),
                ];
                faults.extend(
                    layout
                        .atomic_transfer
                        .operations
                        .unwrap()
                        .map(|field| (atomic, field, 2)),
                );
                for (base, scalar, value) in faults {
                    let mut bytes = clean.clone();
                    store(&mut bytes, base, scalar, value);
                    assert!(!run(
                        builder,
                        result,
                        &mut bytes,
                        u64::from(layout.bytes) + 8
                    ));
                }
            });
        }

        fn placement(
            offset: u64,
            access: access_plans::FieldAccess,
        ) -> access_plans::ValidatedAccessPlan {
            use access_plans::{AccessFields, AccessPlan, validate_access_plan};
            use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
            let layout = LayoutPlanReport {
                schema_report_fingerprint: 0,
                offsets: None,
                size: Some(16),
                align: 1,
                entries: vec![LayoutFieldEntryReport {
                    field: "value".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset },
                }],
            };
            let mut plan = AccessFields::inaccessible(&layout).unwrap();
            let key = plan.entries()[0].key();
            plan.set(key, access).unwrap();
            validate_access_plan(AccessPlan {
                layout,
                access: plan,
                reach: BoundaryReach::from_services([
                    BoundaryServiceReachId::from_normalized_identity(17).unwrap(),
                ]),
            })
            .unwrap()
        }

        #[test]
        fn runtime_profile_compatibility_matches_normalized_coverage_and_congruence() {
            use access_plans::{AccessExposure, ExternalRead, FieldAccess};
            let exposure = AccessExposure::Exported;
            let placements = [
                placement(
                    0,
                    FieldAccess::Stable {
                        transfer_width_bits: 64,
                        read: true,
                        write: true,
                        exposure,
                    },
                ),
                placement(
                    3,
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: false,
                        exposure,
                    },
                ),
                placement(
                    0,
                    FieldAccess::External {
                        transfer_width_bits: 64,
                        read: ExternalRead::Read,
                        write: true,
                        exposure,
                    },
                ),
                placement(
                    0,
                    FieldAccess::External {
                        transfer_width_bits: 64,
                        read: ExternalRead::Take,
                        write: false,
                        exposure,
                    },
                ),
                placement(
                    0,
                    FieldAccess::Atomic {
                        transfer_width_bits: 64,
                        operations: AtomicPermissions {
                            load: true,
                            compare_exchange_once: true,
                            ..Default::default()
                        },
                        exposure,
                    },
                ),
                placement(0, FieldAccess::Inaccessible),
            ];
            let rules = vec![
                TransferRule {
                    width_bits: 64,
                    alignment_bytes: 32,
                },
                TransferRule {
                    width_bits: 32,
                    alignment_bytes: 4,
                },
            ];
            let mut rich = region(0, 256);
            rich.external = ExternalCapability::Access {
                read: ExternalReadBehavior::Repeatable,
                write: true,
                transfers: rules.clone(),
            };
            rich.atomic = AtomicCapability::Access {
                transfers: rules
                    .iter()
                    .map(|rule| AtomicTransferRule {
                        transfer: *rule,
                        operations: all_operations(),
                    })
                    .collect(),
            };
            let split = |first: ResourceRegion, second: ResourceRegion| ResourceProfile {
                regions: vec![
                    ResourceRegion {
                        offset: 4,
                        length: 252,
                        ..second
                    },
                    ResourceRegion {
                        offset: 0,
                        length: 4,
                        ..first
                    },
                ],
            };
            let mut reversed = rich.clone();
            if let ExternalCapability::Access { transfers, .. } = &mut reversed.external {
                transfers.reverse();
            }
            if let AtomicCapability::Access { transfers } = &mut reversed.atomic {
                transfers.reverse();
            }
            let mut extra = rich.clone();
            extra.reach =
                BoundaryReach::from_services([BoundaryServiceReachId::from_normalized_identity(
                    17,
                )
                .unwrap()]);
            let mut destructive = rich.clone();
            if let ExternalCapability::Access { read, .. } = &mut destructive.external {
                *read = ExternalReadBehavior::Destructive;
            }
            let mut missing_write = rich.clone();
            missing_write.stable = StableCapability::Read;
            let mut missing_reach = rich.clone();
            missing_reach.reach = BoundaryReach::default();
            let mut malformed = rich.clone();
            malformed.length = 0;
            let mut missing_atomic_operation = rich.clone();
            if let AtomicCapability::Access { transfers } = &mut missing_atomic_operation.atomic {
                transfers[0].operations.compare_exchange_once = false;
            }
            let mut missing_atomic_width = rich.clone();
            if let AtomicCapability::Access { transfers } = &mut missing_atomic_width.atomic {
                transfers.remove(0);
            }
            let profiles = vec![
                ResourceProfile::default(),
                ResourceProfile {
                    regions: vec![region(0, 256)],
                },
                ResourceProfile {
                    regions: vec![rich.clone()],
                },
                split(rich.clone(), reversed),
                split(rich.clone(), extra),
                split(rich.clone(), destructive.clone()),
                ResourceProfile {
                    regions: vec![destructive],
                },
                split(rich.clone(), missing_write),
                split(rich.clone(), missing_reach),
                ResourceProfile {
                    regions: vec![malformed],
                },
                ResourceProfile {
                    regions: vec![region(0, 4), region(5, 251)],
                },
                ResourceProfile {
                    regions: (0..32)
                        .rev()
                        .map(|position| region(position * 8, 8))
                        .collect(),
                },
                ResourceProfile {
                    regions: vec![missing_atomic_operation.clone()],
                },
                ResourceProfile {
                    regions: vec![missing_atomic_width],
                },
                split(rich.clone(), missing_atomic_operation),
            ];
            let mut declarations = Types::default();
            let profile_type = declarations.profile();
            declarations.expose_scalar_fields();
            let layout = ProfileLayout::derive(&declarations.0, profile_type).unwrap();
            for target in targets() {
                for (placement_position, placement) in placements.iter().enumerate() {
                    with_builder(target, |builder, row| {
                        let profile = profile_home(builder, row);
                        let place = profile.place;
                        let scratch = ImageHome::scratch(
                            builder,
                            row,
                            place,
                            compatibility::COMPATIBILITY_SCRATCH_BYTES,
                        )
                        .unwrap();
                        let available =
                            fragmented_scalar::constant(builder, row, place, 256).unwrap();
                        let assessment = compatibility::assess(
                            &declarations.0,
                            profile_type,
                            builder,
                            row,
                            profile,
                            scratch,
                            available,
                            placement,
                            terminal_psi::TerminalAccessLifecycleOperation::Initialize,
                        )
                        .unwrap();
                        let first_backing = (u64::from(layout.bytes)
                            + u64::from(compatibility::COMPATIBILITY_SCRATCH_BYTES)
                            + 47)
                            & !31;
                        let backing_bases = [first_backing, first_backing + 257];
                        let staged_base = first_backing + 544;
                        let staged =
                            fragmented_scalar::constant(builder, row, place, staged_base).unwrap();
                        let decisions: Vec<_> = backing_bases
                            .into_iter()
                            .map(|base| {
                                let backing =
                                    fragmented_scalar::constant(builder, row, place, base).unwrap();
                                let decision = super::super::outcome::decide(
                                    builder,
                                    row,
                                    place,
                                    backing,
                                    available,
                                    16,
                                    assessment.modulus,
                                    assessment.residue,
                                    assessment.compatible,
                                )
                                .unwrap();
                                super::super::initialization::commit(
                                    builder, row, place, staged, backing, &decision,
                                )
                                .unwrap();
                                decision
                            })
                            .collect();
                        for (profile_position, profile) in profiles.iter().enumerate() {
                            let normalized =
                                access_plans::validate_resource_profile(profile.clone(), 256);
                            let expected = normalized.as_ref().ok().and_then(|profile| {
                                access_plans::validate_initialization_resources(placement, profile)
                                    .ok()
                            });
                            let mut bytes = encode(&layout, profile);
                            bytes.resize(staged_base as usize + 24, 0xd7);
                            bytes[staged_base as usize..staged_base as usize + 16].fill(0x69);
                            let before = bytes[..layout.bytes as usize + 8].to_vec();
                            let registers =
                                run_values(builder, &mut bytes, u64::from(layout.bytes) + 8);
                            assert_eq!(
                                registers[assessment.well_formed.0 as usize],
                                u64::from(normalized.is_ok())
                            );
                            assert_eq!(
                                registers[assessment.compatible.0 as usize],
                                u64::from(expected.is_some()),
                                "placement {placement_position}, profile {profile_position}"
                            );
                            if let Some(expected) = &expected {
                                let congruence = expected.placement().base_congruence();
                                assert_eq!(
                                    registers[assessment.modulus.0 as usize],
                                    congruence.modulus(),
                                    "placement {placement_position}, profile {profile_position}"
                                );
                                assert_eq!(
                                    registers[assessment.residue.0 as usize],
                                    congruence.residue(),
                                    "placement {placement_position}, profile {profile_position}"
                                );
                            }
                            for (base, decision) in backing_bases.into_iter().zip(&decisions) {
                                let ready = expected.as_ref().is_some_and(|expected| {
                                    expected.placement().base_congruence().admits(base)
                                });
                                assert_eq!(registers[decision.ready.0 as usize], u64::from(ready));
                                assert_eq!(
                                    registers[decision.copy_length.0 as usize],
                                    if ready { 16 } else { 0 }
                                );
                                assert_eq!(
                                    &bytes[base as usize..base as usize + 16],
                                    &[if ready { 0x69 } else { 0xd7 }; 16]
                                );
                                assert!(
                                    bytes[base as usize + 16..base as usize + 256]
                                        .iter()
                                        .all(|byte| *byte == 0xd7),
                                    "initialization touched the allocation tail"
                                );
                            }
                            assert_eq!(
                                &bytes[..before.len()],
                                before,
                                "compatibility mutated its immutable input"
                            );
                            assert_eq!(&bytes[bytes.len() - 8..], &[0xd7; 8]);
                            if profile_position == 3 {
                                let region = entry(0, layout.regions, 0);
                                store(&mut bytes, region, layout.reach.count, 3);
                                for (position, service) in [29, 17, 29].into_iter().enumerate() {
                                    store(
                                        &mut bytes,
                                        entry(region, layout.reach, position),
                                        layout.service,
                                        service,
                                    );
                                }
                                let registers =
                                    run_values(builder, &mut bytes, u64::from(layout.bytes) + 8);
                                assert_eq!(registers[assessment.well_formed.0 as usize], 1);
                                assert_eq!(
                                    registers[assessment.compatible.0 as usize],
                                    u64::from(expected.is_some())
                                );
                            }
                        }
                    });
                }
            }
        }
    }
    use geometry::{Counted, ProfileLayout, Scalar, TransferLayout};

    macro_rules! bin {
        ($emission:expr => $builder:expr => $kind:ident => $left:expr => $right:expr) => {
            $emission.binary($builder, SelectedInstructionKind::$kind, $left, $right)
        };
    }

    pub(crate) const VALIDATION_SCRATCH_BYTES: u32 = 48;
    const VALID: u32 = 0;
    const REGION: u32 = 8;
    const PRIOR_REGION: u32 = 16;
    const TRANSFER: u32 = 24;
    const PRIOR_TRANSFER: u32 = 32;
    const SERVICE: u32 = 40;

    #[derive(Clone, Copy)]
    struct Emission<'row> {
        row: &'row LegalizedScalarInstruction,
        profile: ImageHome<'row>,
        profile_bytes: u32,
        scratch: ImageHome<'row>,
    }

    impl Emission<'_> {
        fn constant(
            self,
            builder: &mut Builder<'_>,
            value: u64,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            fragmented_scalar::constant(builder, self.row, self.scratch.place, value)
        }

        fn binary(
            self,
            builder: &mut Builder<'_>,
            kind: SelectedInstructionKind,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            fragmented_scalar::binary(builder, self.row, self.scratch.place, kind, left, right)
        }

        fn compare(
            self,
            builder: &mut Builder<'_>,
            kind: SelectedInstructionKind,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            super::outcome::compare(builder, self.row, self.scratch.place, left, right, kind)
        }

        fn equal(
            self,
            builder: &mut Builder<'_>,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            self.compare(
                builder,
                SelectedInstructionKind::MaterializeBooleanEqual,
                left,
                right,
            )
        }

        fn at_most(
            self,
            builder: &mut Builder<'_>,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            self.compare(
                builder,
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                left,
                right,
            )
        }

        fn invert(
            self,
            builder: &mut Builder<'_>,
            value: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let one = self.constant(builder, 1)?;
            self.binary(builder, SelectedInstructionKind::BitwiseXorI64, value, one)
        }

        fn nonzero(
            self,
            builder: &mut Builder<'_>,
            value: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let zero = self.constant(builder, 0)?;
            let absent = self.equal(builder, value, zero)?;
            self.invert(builder, absent)
        }

        fn read_scratch(
            self,
            builder: &mut Builder<'_>,
            byte_offset: u32,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            super::ScalarImageCodec::Bytes {
                byte_offset,
                width: 8,
                signed: false,
            }
            .read(builder, self.row, self.scratch)
        }

        fn write_scratch(
            self,
            builder: &mut Builder<'_>,
            byte_offset: u32,
            value: VirtualRegisterId,
        ) -> Result<(), SelectedInstructionError> {
            super::ScalarImageCodec::Bytes {
                byte_offset,
                width: 8,
                signed: false,
            }
            .write(builder, self.row, self.scratch, value)
        }

        fn require(
            self,
            builder: &mut Builder<'_>,
            predicate: VirtualRegisterId,
        ) -> Result<(), SelectedInstructionError> {
            let valid = self.read_scratch(builder, VALID)?;
            let valid = bin!(self => builder => BitwiseAndI64 => valid => predicate)?;
            self.write_scratch(builder, VALID, valid)
        }

        fn load(
            self,
            builder: &mut Builder<'_>,
            pointer: VirtualRegisterId,
            scalar: Scalar,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let mut value = self.constant(builder, 0)?;
            let displacement = self.constant(builder, u64::from(scalar.offset))?;
            let address = bin!(self => builder => WrappingAddI64 => pointer => displacement)?;
            for byte in 0..scalar.width {
                let output =
                    crate::selection::construction::scalar_graph::structural::transport_register(
                        builder,
                        self.profile.place,
                        0,
                    )?;
                self.profile.storage.footprint(
                    builder,
                    self.row,
                    self.profile.place,
                    self.profile.byte_offset,
                    self.profile_bytes,
                    crate::selection::construction::scalar_graph::structural::runtime_address::Written::None,
                )?;
                builder.emit(
                    SelectedInstructionKind::Load8 { byte_offset: byte },
                    builder
                        .constraints
                        .keys
                        .load8
                        .ok_or_else(|| failure("profile validation target lacks byte loads"))?,
                    &[address, output],
                    crate::selection::construction::scalar_graph::structural::provenance(self.row),
                )?;
                let shifted = fragmented_scalar::shift(
                    builder,
                    self.row,
                    self.scratch.place,
                    output,
                    u64::from(byte) * 8,
                    true,
                )?;
                value = bin!(self => builder => BitwiseOrI64 => value => shifted)?;
            }
            Ok(value)
        }

        fn element(
            self,
            builder: &mut Builder<'_>,
            pointer: VirtualRegisterId,
            array: Counted,
            position: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let stride = self.constant(builder, u64::from(array.stride))?;
            let displacement = bin!(self => builder => WrappingMultiplyI64 => position => stride)?;
            let offset = self.constant(builder, u64::from(array.offset))?;
            let displacement = bin!(self => builder => WrappingAddI64 => displacement => offset)?;
            bin!(self => builder => WrappingAddI64 => pointer => displacement)
        }

        fn bounded_count(
            self,
            builder: &mut Builder<'_>,
            pointer: VirtualRegisterId,
            array: Counted,
            nonempty: bool,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let count = self.load(builder, pointer, array.count)?;
            let capacity = self.constant(builder, array.capacity)?;
            let bounded = self.at_most(builder, count, capacity)?;
            self.require(builder, bounded)?;
            if nonempty {
                let present = self.nonzero(builder, count)?;
                self.require(builder, present)?;
            }
            bin!(self => builder => WrappingMultiplyI64 => count => bounded)
        }

        fn repeat(
            self,
            builder: &mut Builder<'_>,
            count: VirtualRegisterId,
            counter: u32,
            body: impl FnOnce(
                &mut Builder<'_>,
                VirtualRegisterId,
            ) -> Result<(), SelectedInstructionError>,
        ) -> Result<(), SelectedInstructionError> {
            let zero = self.constant(builder, 0)?;
            self.write_scratch(builder, counter, zero)?;
            operation_control::while_nonzero(
                builder,
                self.row,
                |builder| {
                    let position = self.read_scratch(builder, counter)?;
                    let finished = self.at_most(builder, count, position)?;
                    self.invert(builder, finished)
                },
                |builder| {
                    let position = self.read_scratch(builder, counter)?;
                    body(builder, position)?;
                    let one = self.constant(builder, 1)?;
                    let next = bin!(self => builder => WrappingAddI64 => position => one)?;
                    self.write_scratch(builder, counter, next)
                },
            )
        }

        fn transfer_rules(
            self,
            builder: &mut Builder<'_>,
            pointer: VirtualRegisterId,
            array: Counted,
            layout: &TransferLayout,
        ) -> Result<(), SelectedInstructionError> {
            let count = self.bounded_count(builder, pointer, array, true)?;
            self.repeat(builder, count, TRANSFER, |builder, position| {
                let entry = self.element(builder, pointer, array, position)?;
                let width = self.load(builder, entry, layout.width)?;
                let alignment = self.load(builder, entry, layout.alignment)?;
                let present = self.nonzero(builder, width)?;
                self.require(builder, present)?;
                let maximum = self.constant(builder, 128)?;
                let bounded = self.at_most(builder, width, maximum)?;
                self.require(builder, bounded)?;
                let byte_mask = self.constant(builder, 7)?;
                let remainder = bin!(self => builder => BitwiseAndI64 => width => byte_mask)?;
                let zero = self.constant(builder, 0)?;
                let whole_bytes = self.equal(builder, remainder, zero)?;
                self.require(builder, whole_bytes)?;
                let present = self.nonzero(builder, alignment)?;
                self.require(builder, present)?;
                let one = self.constant(builder, 1)?;
                let previous = bin!(self => builder => WrappingSubtractI64 => alignment => one)?;
                let bits = bin!(self => builder => BitwiseAndI64 => alignment => previous)?;
                let power = self.equal(builder, bits, zero)?;
                self.require(builder, power)?;
                if let Some(operations) = &layout.operations {
                    let mut any = zero;
                    for operation in operations {
                        let value = self.load(builder, entry, *operation)?;
                        let boolean = self.at_most(builder, value, one)?;
                        self.require(builder, boolean)?;
                        any = self.binary(
                            builder,
                            SelectedInstructionKind::BitwiseOrI64,
                            any,
                            value,
                        )?;
                    }
                    let present = self.nonzero(builder, any)?;
                    self.require(builder, present)?;
                }
                self.repeat(
                    builder,
                    position,
                    PRIOR_TRANSFER,
                    |builder, prior_position| {
                        let prior = self.element(builder, pointer, array, prior_position)?;
                        let prior_width = self.load(builder, prior, layout.width)?;
                        let duplicate = self.equal(builder, width, prior_width)?;
                        let distinct = self.invert(builder, duplicate)?;
                        self.require(builder, distinct)
                    },
                )
            })
        }
    }

    #[cfg(test)]
    fn validate(
        declarations: &[terminal_psi::StructuralTypeDeclaration],
        profile_type: semantic_vocabulary::StructuralTypeId,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        profile: ImageHome,
        scratch: ImageHome,
        available: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let layout = ProfileLayout::derive(declarations, profile_type)?;
        validate_layout(&layout, builder, row, profile, scratch, available)
    }

    fn validate_layout(
        layout: &ProfileLayout,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        profile: ImageHome,
        scratch: ImageHome,
        available: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        super::offset(scratch.byte_offset, u64::from(VALIDATION_SCRATCH_BYTES))?;
        super::offset(profile.byte_offset, u64::from(layout.bytes))?;
        let emission = Emission {
            row,
            profile,
            profile_bytes: layout.bytes,
            scratch,
        };
        let valid = emission.constant(builder, 1)?;
        emission.write_scratch(builder, VALID, valid)?;
        let displacement = emission.constant(builder, u64::from(profile.byte_offset))?;
        let pointer =
            bin!(emission => builder => WrappingAddI64 => profile.pointer => displacement)?;
        let count = emission.bounded_count(builder, pointer, layout.regions, false)?;
        emission.repeat(builder, count, REGION, |builder, position| {
            let region = emission.element(builder, pointer, layout.regions, position)?;
            let start = emission.load(builder, region, layout.start)?;
            let length = emission.load(builder, region, layout.length)?;
            let end = bin!(emission => builder => WrappingAddI64 => start => length)?;
            let nonempty = emission.nonzero(builder, length)?;
            emission.require(builder, nonempty)?;
            let nonoverflow = emission.at_most(builder, start, end)?;
            emission.require(builder, nonoverflow)?;
            let contained = emission.at_most(builder, end, available)?;
            emission.require(builder, contained)?;
            emission.repeat(
                builder,
                position,
                PRIOR_REGION,
                |builder, prior_position| {
                    let prior =
                        emission.element(builder, pointer, layout.regions, prior_position)?;
                    let prior_start = emission.load(builder, prior, layout.start)?;
                    let prior_length = emission.load(builder, prior, layout.length)?;
                    let prior_end = emission.binary(
                        builder,
                        SelectedInstructionKind::WrappingAddI64,
                        prior_start,
                        prior_length,
                    )?;
                    let before = emission.at_most(builder, end, prior_start)?;
                    let after = emission.at_most(builder, prior_end, start)?;
                    let disjoint = bin!(emission => builder => BitwiseOrI64 => before => after)?;
                    emission.require(builder, disjoint)
                },
            )?;
            let peer = emission.load(builder, region, layout.peer)?;
            let stable = emission.load(builder, region, layout.stable)?;
            let external = emission.load(builder, region, layout.external)?;
            let atomic = emission.load(builder, region, layout.atomic)?;
            let one = emission.constant(builder, 1)?;
            let three = emission.constant(builder, 3)?;
            for tag in [peer, external, atomic] {
                let known = emission.at_most(builder, tag, one)?;
                emission.require(builder, known)?;
            }
            let known = emission.at_most(builder, stable, three)?;
            emission.require(builder, known)?;
            let stable_present = emission.nonzero(builder, stable)?;
            let hostile_stable =
                bin!(emission => builder => BitwiseAndI64 => peer => stable_present)?;
            let safe = emission.invert(builder, hostile_stable)?;
            emission.require(builder, safe)?;
            let any = bin!(emission => builder => BitwiseOrI64 => stable => external)?;
            let any =
                emission.binary(builder, SelectedInstructionKind::BitwiseOrI64, any, atomic)?;
            let present = emission.nonzero(builder, any)?;
            emission.require(builder, present)?;
            let external_active = emission.equal(builder, external, one)?;
            operation_control::when_nonzero(builder, row, external_active, |builder| {
                let read = emission.load(builder, region, layout.external_read)?;
                let write = emission.load(builder, region, layout.external_write)?;
                let two = emission.constant(builder, 2)?;
                let known = emission.at_most(builder, read, two)?;
                emission.require(builder, known)?;
                let boolean = emission.at_most(builder, write, one)?;
                emission.require(builder, boolean)?;
                let any =
                    emission.binary(builder, SelectedInstructionKind::BitwiseOrI64, read, write)?;
                let present = emission.nonzero(builder, any)?;
                emission.require(builder, present)?;
                emission.transfer_rules(
                    builder,
                    region,
                    layout.external_rules,
                    &layout.external_transfer,
                )
            })?;
            let atomic_active = emission.equal(builder, atomic, one)?;
            operation_control::when_nonzero(builder, row, atomic_active, |builder| {
                emission.transfer_rules(
                    builder,
                    region,
                    layout.atomic_rules,
                    &layout.atomic_transfer,
                )
            })?;
            let services = emission.bounded_count(builder, region, layout.reach, false)?;
            emission.repeat(builder, services, SERVICE, |builder, service_position| {
                let service_pointer =
                    emission.element(builder, region, layout.reach, service_position)?;
                let service = emission.load(builder, service_pointer, layout.service)?;
                let valid = emission.nonzero(builder, service)?;
                emission.require(builder, valid)
            })
        })?;
        emission.read_scratch(builder, VALID)
    }
}
pub(crate) mod result_layout {
    use super::{
        StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, failure, offset,
    };
    use crate::{SelectedInstructionError, structural_inputs::structural_reference_input};
    use calling_conventions::{PackedFieldLayout, ValueShape};
    use terminal_psi::{
        StructuralCaseDeclaration, StructuralFieldDeclaration, TerminalAccessLifecyclePlan,
    };

    pub(crate) struct ResultLayout {
        pub shape: ValueShape,
        pub tag: u32,
        pub ready_owner: PackedFieldLayout,
        pub ready_backing: PackedFieldLayout,
        pub ready_count: Option<u32>,
        pub ready_vacated: Option<PackedFieldLayout>,
        pub returned: ReturnedLayout,
        pub failure_tag: u32,
        pub range: [u32; 2],
        pub alignment: [u32; 2],
    }

    pub(crate) enum ReturnedLayout {
        Owned {
            owner: PackedFieldLayout,
            value: PackedFieldLayout,
        },
        Borrowed,
    }

    fn declaration(
        types: &[StructuralTypeDeclaration],
        identity: StructuralTypeId,
    ) -> Result<&StructuralTypeDeclaration, SelectedInstructionError> {
        types
            .iter()
            .find(|item| item.id == identity)
            .ok_or_else(|| failure("placement result lost a retained declaration"))
    }

    fn cases(
        types: &[StructuralTypeDeclaration],
        identity: StructuralTypeId,
    ) -> Result<&[StructuralCaseDeclaration], SelectedInstructionError> {
        match &declaration(types, identity)?.shape {
            StructuralTypeShape::Sum { cases } => Ok(cases),
            _ => Err(failure("placement outcome must retain its core sum")),
        }
    }

    fn structural(
        field: &StructuralFieldDeclaration,
    ) -> Result<StructuralTypeId, SelectedInstructionError> {
        field
            .field_type
            .as_structural()
            .ok_or_else(|| failure("placement role lost its structural type"))
    }

    impl ResultLayout {
        pub(crate) fn derive(
            types: &[StructuralTypeDeclaration],
            result: StructuralTypeId,
            plan: &TerminalAccessLifecyclePlan,
        ) -> Result<Self, SelectedInstructionError> {
            let [ready, rejected] = cases(types, result)? else {
                return Err(failure("placement outcome must retain Ready and Rejected"));
            };
            let ([ready_owner], [returned, reason]) =
                (ready.fields.as_slice(), rejected.fields.as_slice())
            else {
                return Err(failure(
                    "placement outcome changed its custody or reason roles",
                ));
            };
            match plan.operation {
                terminal_psi::TerminalAccessLifecycleOperation::Relocate => {
                    let StructuralTypeShape::Record { fields, .. } =
                        &declaration(types, structural(ready_owner)?)?.shape
                    else {
                        return Err(failure(
                            "relocation Ready must contain its run and vacated backing",
                        ));
                    };
                    let [run, vacated] = fields.as_slice() else {
                        return Err(failure("relocation Ready changed its two custody roles"));
                    };
                    if structural(run)? != plan.storage.owner_carrier()
                        || structural(vacated)? != plan.storage.carrier
                    {
                        return Err(failure("relocation Ready changed its run or backing type"));
                    }
                }
                terminal_psi::TerminalAccessLifecycleOperation::Initialize => {
                    if structural(ready_owner)? != plan.storage.owner_carrier() {
                        return Err(failure(
                            "initialization Ready no longer retains the original owner",
                        ));
                    }
                }
                terminal_psi::TerminalAccessLifecycleOperation::View => {
                    if !matches!(
                        declaration(types, structural(ready_owner)?)?.shape,
                        StructuralTypeShape::Reference {
                            access: terminal_psi::reading_borrow_pair_pattern!(),
                            ..
                        }
                    ) {
                        return Err(failure(
                            "borrowed placement Ready must retain its borrowed reference",
                        ));
                    }
                }
                terminal_psi::TerminalAccessLifecycleOperation::MoveOut => {
                    return Err(failure(
                        "extraction does not have a placement establishment outcome",
                    ));
                }
            }
            let ready_type = structural(ready_owner)?;
            let result_layout = structural_reference_input::case_layout(result, types)
                .ok_or_else(|| failure("placement outcome has no native layout"))?;
            let [ready_layout, rejected_layout] = result_layout.cases.as_slice() else {
                return Err(failure("placement outcome layout changed cases"));
            };
            let ([ready_owner], [returned_layout, reason_layout]) = (
                ready_layout.fields.as_slice(),
                rejected_layout.fields.as_slice(),
            ) else {
                return Err(failure("placement outcome layout changed fields"));
            };
            let mut ready_owner = *ready_owner;
            let ready_vacated = if plan.operation
                == terminal_psi::TerminalAccessLifecycleOperation::Relocate
            {
                let StructuralTypeShape::Record { fields, .. } =
                    &declaration(types, ready_type)?.shape
                else {
                    return Err(failure("relocation Ready lost its record layout"));
                };
                let [run, vacated] = fields.as_slice() else {
                    return Err(failure("relocation Ready lost its field pair"));
                };
                let packed = |field: &StructuralFieldDeclaration| -> Result<PackedFieldLayout, SelectedInstructionError> {
                    let region = super::field_region(types, ready_type, field)?;
                    Ok(PackedFieldLayout {
                        shape: structural_reference_input::field_storage_shape(field, types)
                            .ok_or_else(|| failure("relocation result field has no native shape"))?,
                        byte_offset: offset(ready_owner.byte_offset, u64::from(region.byte_offset))?,
                    })
                };
                let vacated = packed(vacated)?;
                ready_owner = packed(run)?;
                Some(vacated)
            } else {
                None
            };
            let returned_type = structural(returned)?;
            let StructuralTypeShape::Record { fields, .. } =
                &declaration(types, returned_type)?.shape
            else {
                return Err(failure(
                    "placement return carrier is not its retained record",
                ));
            };
            let returned_field = |field: &StructuralFieldDeclaration| {
                let region = super::field_region(types, returned_type, field)?;
                let shape = structural_reference_input::field_storage_shape(field, types)
                    .ok_or_else(|| failure("placement returned field has no storage shape"))?;
                Ok(PackedFieldLayout {
                    shape,
                    byte_offset: offset(
                        returned_layout.byte_offset,
                        u64::from(region.byte_offset),
                    )?,
                })
            };
            let returned = match (plan.operation, fields.as_slice()) {
                (terminal_psi::TerminalAccessLifecycleOperation::Relocate, [run, destination])
                    if structural(run)? == plan.storage.owner_carrier()
                        && structural(destination)? == plan.storage.carrier =>
                {
                    ReturnedLayout::Owned {
                        owner: returned_field(run)?,
                        value: returned_field(destination)?,
                    }
                }
                (terminal_psi::TerminalAccessLifecycleOperation::Initialize, [owner, value])
                    if structural(owner)? == plan.storage.carrier
                        && structural(value)? == plan.schema =>
                {
                    ReturnedLayout::Owned {
                        owner: returned_field(owner)?,
                        value: returned_field(value)?,
                    }
                }
                (terminal_psi::TerminalAccessLifecycleOperation::View, []) => {
                    ReturnedLayout::Borrowed
                }
                _ => {
                    return Err(failure(
                        "placement rejection changed its operation's returned custody",
                    ));
                }
            };
            let reason_type = structural(reason)?;
            let reason_layouts = structural_reference_input::case_layout(reason_type, types)
                .ok_or_else(|| failure("placement failure has no native layout"))?;
            let [range, alignment, incompatible] = reason_layouts.cases.as_slice() else {
                return Err(failure("placement failure changed its three core cases"));
            };
            if !incompatible.fields.is_empty() {
                return Err(failure("IncompatibleSupply must have no runtime payload"));
            }
            let pair = |fields: &[PackedFieldLayout]| {
                let [left, right] = fields else {
                    return Err(failure("placement failure lost its two geometry words"));
                };
                if left.shape.byte_size != 8 || right.shape.byte_size != 8 {
                    return Err(failure(
                        "placement failure geometry must be native u64 words",
                    ));
                }
                Ok([
                    offset(reason_layout.byte_offset, u64::from(left.byte_offset))?,
                    offset(reason_layout.byte_offset, u64::from(right.byte_offset))?,
                ])
            };
            let (ready_backing, ready_count) = match plan.storage.owner {
                terminal_psi::TerminalAccessOwner::Extent => (ready_owner, None),
                terminal_psi::TerminalAccessOwner::Run {
                    carrier,
                    storage,
                    count,
                    ..
                } => {
                    let StructuralTypeShape::Record { fields, .. } =
                        &declaration(types, carrier)?.shape
                    else {
                        return Err(failure("run owner lost its record layout"));
                    };
                    let backing = fields
                        .iter()
                        .find(|field| field.id == storage)
                        .ok_or_else(|| failure("run backing field is absent"))?;
                    let count = fields
                        .iter()
                        .find(|field| field.id == count)
                        .ok_or_else(|| failure("run count field is absent"))?;
                    let backing_region = super::field_region(types, carrier, backing)?;
                    let count_region = super::field_region(types, carrier, count)?;
                    let shape = structural_reference_input::field_storage_shape(backing, types)
                        .ok_or_else(|| failure("run backing field has no physical shape"))?;
                    (
                        PackedFieldLayout {
                            shape,
                            byte_offset: offset(
                                ready_owner.byte_offset,
                                u64::from(backing_region.byte_offset),
                            )?,
                        },
                        Some(offset(
                            ready_owner.byte_offset,
                            u64::from(count_region.byte_offset),
                        )?),
                    )
                }
            };
            Ok(Self {
                shape: result_layout.shape,
                tag: result_layout.tag_byte_offset,
                ready_owner,
                ready_backing,
                ready_count,
                ready_vacated,
                returned,
                failure_tag: offset(
                    reason_layout.byte_offset,
                    u64::from(reason_layouts.tag_byte_offset),
                )?,
                range: pair(&range.fields)?,
                alignment: pair(&alignment.fields)?,
            })
        }
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use legalized_operations::LegalizedOperationPlan;
    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        SelectedInstruction, SelectedInstructionProvenance, SelectedSelectionConstraints,
    };
    use semantic_vocabulary::{IntegerValue, MachineId, OperationId, WorkScheduleIdentity};
    use target::NativeTarget;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    pub(crate) fn with_builder(
        target: NativeTarget,
        test: impl FnOnce(&mut Builder<'_>, &LegalizedScalarInstruction),
    ) {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: vec![],
        };
        let catalog = environment.constraints();
        let materialize =
            crate::selection::constraints::row(catalog, constraints.keys.materialize_i64).unwrap();
        let program = LegalizedOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0; 32]),
            },
            optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(
                b"placement-lifecycle test",
            ),
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            target,
            entry: MachineId::new(1).unwrap(),
            scalar_functions: vec![],
        };
        // This single-instruction fixture has no functions or call occurrences.
        assert!(program.scalar_functions.is_empty());
        let callable_invocations =
            terminal_semantics::callable_values::ReachableCallableInvocations {
                roots: std::collections::BTreeSet::new(),
                reached_machines: std::collections::BTreeSet::new(),
                invocations: std::collections::BTreeMap::new(),
                inputs: std::collections::BTreeMap::new(),
            };
        let backings = crate::selection::foreign_scalar_backings::ForeignScalarBackings::new(
            &program,
            &callable_invocations,
        )
        .expect("the instruction fixture has no foreign scalar call backings");
        let installed_interrupts =
            provider_admission::AdmittedInstalledInterruptCohort::empty(program.psi);
        let interrupt_helpers =
            crate::installed_interrupts::PreparedHelperCohort::new(&installed_interrupts);
        let mut builder = Builder {
            installed_interrupts: &installed_interrupts,
            installed_stack_requirements: &[],
            installed_entry_execution: None,
            interrupt_helpers: &interrupt_helpers,
            backings: &backings,
            environment: &environment,
            pending_provenance: SelectedInstructionProvenance::default(),
            required_values: Default::default(),
            transport: Default::default(),
            class: materialize.operands[0].class,
            constraints: &constraints,
            catalog,
            registers: vec![],
            pending_result_origins: vec![],
            instructions: vec![],
            implementation_blocks: vec![],
            open_block: None,
            next_block: 0,
            block_body_end: None,
            edge_work: vec![],
            definitions: vec![],
        };
        let row = LegalizedScalarInstruction {
            operation: OperationId::new(71).unwrap(),
            result: None,
            kind: LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(0)),
            work: vec![],
            effect: optimization_unit::EffectLink {
                input: 0,
                output: 1,
            },
            ownership: vec![],
        };
        test(&mut builder, &row);
    }

    fn declaration(id: StructuralTypeId, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            id,
            identity: terminal_psi::TerminalIdentityIndex(0),
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape,
        }
    }

    pub(crate) fn execute(
        instructions: &[SelectedInstruction],
        registers: &mut [u64],
        bytes: &mut [u8],
    ) -> (u64, u64) {
        execute_with_frame(instructions, registers, bytes, &[])
    }

    pub(crate) fn execute_with_frame(
        instructions: &[SelectedInstruction],
        registers: &mut [u64],
        bytes: &mut [u8],
        frame: &[(selected_instructions::FrameStorageSlotId, u64)],
    ) -> (u64, u64) {
        let mut compared = (0_u64, 0_u64);
        for instruction in instructions {
            let register =
                |operand: u16| instruction.operand(operand).unwrap().virtual_register.0 as usize;
            let value = |operand| registers[register(operand)];
            let (destination, result) = match instruction.kind {
                SelectedInstructionKind::FrameAddress { slot, byte_offset } => {
                    let (_, base) = frame
                        .iter()
                        .find(|(candidate, _)| *candidate == slot)
                        .expect("every allocated scratch slot needs a test frame address");
                    (0, base.checked_add(u64::from(byte_offset)).unwrap())
                }
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(bits),
                } => (0, bits as u64),
                SelectedInstructionKind::Load8 { byte_offset } => (
                    1,
                    u64::from(bytes[(value(0) + u64::from(byte_offset)) as usize]),
                ),
                SelectedInstructionKind::Load16 { byte_offset }
                | SelectedInstructionKind::Load32 { byte_offset }
                | SelectedInstructionKind::Load64 { byte_offset } => {
                    let width = match instruction.kind {
                        SelectedInstructionKind::Load16 { .. } => 2,
                        SelectedInstructionKind::Load32 { .. } => 4,
                        _ => 8,
                    };
                    let start = (value(0) + u64::from(byte_offset)) as usize;
                    let mut word = [0; 8];
                    word[..width].copy_from_slice(&bytes[start..start + width]);
                    (1, u64::from_le_bytes(word))
                }
                SelectedInstructionKind::AddressOffset { byte_offset } => {
                    (1, value(0).checked_add(u64::from(byte_offset)).unwrap())
                }
                SelectedInstructionKind::CopyI64 => (1, value(0)),
                SelectedInstructionKind::ZeroBytes => {
                    let length = usize::try_from(value(1)).unwrap();
                    if length != 0 {
                        let start = usize::try_from(value(0)).unwrap();
                        bytes[start..start + length].fill(0);
                    }
                    continue;
                }
                SelectedInstructionKind::BitwiseOrI64 => (2, value(0) | value(1)),
                SelectedInstructionKind::BitwiseAndI64 => (2, value(0) & value(1)),
                SelectedInstructionKind::BitwiseXorI64 => (2, value(0) ^ value(1)),
                SelectedInstructionKind::WrappingSubtractI64 => {
                    (2, value(0).wrapping_sub(value(1)))
                }
                SelectedInstructionKind::WrappingAddI64 => (2, value(0).wrapping_add(value(1))),
                SelectedInstructionKind::ByteViewAddress => {
                    (2, value(0).checked_add(value(1)).unwrap())
                }
                SelectedInstructionKind::WrappingMultiplyI64 => {
                    (2, value(0).wrapping_mul(value(1)))
                }
                SelectedInstructionKind::CompareI64 => {
                    compared = (value(0), value(1));
                    continue;
                }
                SelectedInstructionKind::CompareI64Zero => {
                    compared = (value(0), 0);
                    continue;
                }
                SelectedInstructionKind::MaterializeBooleanEqual => {
                    (0, u64::from(compared.0 == compared.1))
                }
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
                    (0, u64::from(compared.0 <= compared.1))
                }
                SelectedInstructionKind::CopyBytes => {
                    let length = usize::try_from(value(2)).unwrap();
                    if length != 0 {
                        let source = usize::try_from(value(0)).unwrap();
                        let destination = usize::try_from(value(1)).unwrap();
                        assert!(
                            source + length <= destination || destination + length <= source,
                            "staged image must not overlap the backing"
                        );
                        bytes.copy_within(source..source + length, destination);
                    }
                    continue;
                }
                SelectedInstructionKind::WrappingShiftLeftI64 => {
                    (2, value(0).wrapping_shl(value(1) as u32))
                }
                SelectedInstructionKind::WrappingShiftRightU64 => {
                    (2, value(0).wrapping_shr(value(1) as u32))
                }
                SelectedInstructionKind::Store {
                    byte_offset,
                    byte_size,
                } => {
                    let address = (value(0) + u64::from(byte_offset)) as usize;
                    bytes[address..address + usize::from(byte_size)]
                        .copy_from_slice(&value(1).to_le_bytes()[..usize::from(byte_size)]);
                    continue;
                }
                ref unexpected => panic!("unexpected resident codec instruction: {unexpected:?}"),
            };
            registers[register(destination)] = result;
        }
        compared
    }

    pub(crate) fn targets() -> [NativeTarget; 4] {
        [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ]
    }

    pub(crate) fn execute_operation(
        builder: &Builder<'_>,
        bytes: &mut [u8],
        frame: &[(selected_instructions::FrameStorageSlotId, u64)],
    ) -> Option<Vec<u64>> {
        use selected_instructions::{SelectedBlockId, SelectedTerminator};
        let continuation = builder.open_block.unwrap();
        let mut registers = vec![0; builder.registers.len()];
        let mut current = SelectedBlockId(0);
        let mut visits = 0;
        while current != continuation.id {
            visits += 1;
            assert!(visits < 100_000, "operation did not terminate");
            let block = builder
                .implementation_blocks
                .iter()
                .find(|block| block.id == current)
                .unwrap();
            let compared = execute_with_frame(&block.instructions, &mut registers, bytes, frame);
            current = match &block.terminator {
                SelectedTerminator::Jump { successor, .. } => successor.block,
                SelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                } => {
                    if compared.0 != compared.1 {
                        when_nonzero.block
                    } else {
                        when_zero.block
                    }
                }
                SelectedTerminator::OperationTrap { .. } => return None,
                unexpected => panic!("unexpected operation control {unexpected:?}"),
            };
        }
        execute_with_frame(
            &builder.instructions[continuation.start..],
            &mut registers,
            bytes,
            frame,
        );
        Some(registers)
    }

    #[test]
    fn failed_operation_assertion_traps_without_a_source_edge_or_continuation() {
        use crate::selection::construction::scalar_graph::operation_control::{self, OpenBlock};
        use selected_instructions::{SelectedBlockId, SelectedBlockOrigin, SelectedTerminator};
        use semantic_vocabulary::BlockId;
        for target in targets() {
            for input in [0, 1, 7] {
                with_builder(target, |builder, row| {
                    builder.open_block = Some(OpenBlock {
                        id: SelectedBlockId(0),
                        origin: SelectedBlockOrigin::Source(BlockId::new(7).unwrap()),
                        start: 0,
                    });
                    builder.next_block = 1;
                    let place = PlaceId::new(19).unwrap();
                    let pointer = fragmented_scalar::constant(builder, row, place, 0).unwrap();
                    let predicate =
                        fragmented_scalar::constant(builder, row, place, input).unwrap();
                    operation_control::require_nonzero(builder, row, predicate).unwrap();
                    let one = fragmented_scalar::constant(builder, row, place, 1).unwrap();
                    ScalarImageCodec::Bytes {
                        byte_offset: 0,
                        width: 8,
                        signed: false,
                    }
                    .write(
                        builder,
                        row,
                        ImageHome {
                            storage: fragmented_scalar::StorageAccess::Place {
                                indices: &[],
                                byte_offset: 0,
                            },
                            place,
                            pointer,
                            byte_offset: 0,
                        },
                        one,
                    )
                    .unwrap();
                    let continuation = builder.open_block.unwrap();
                    let mut registers = vec![0; builder.registers.len()];
                    let mut bytes = [0xa5; 8];
                    let mut current = SelectedBlockId(0);
                    let mut trapped = false;
                    for _ in 0..4 {
                        if current == continuation.id {
                            execute(
                                &builder.instructions[continuation.start..],
                                &mut registers,
                                &mut bytes,
                            );
                            break;
                        }
                        let block = builder
                            .implementation_blocks
                            .iter()
                            .find(|block| block.id == current)
                            .unwrap();
                        let compared = execute(&block.instructions, &mut registers, &mut bytes);
                        match &block.terminator {
                            SelectedTerminator::ConditionalBranch {
                                when_nonzero,
                                when_zero,
                                ..
                            } => {
                                current = if compared.0 != compared.1 {
                                    when_nonzero.block
                                } else {
                                    when_zero.block
                                };
                            }
                            SelectedTerminator::OperationTrap {
                                operation,
                                instruction,
                            } => {
                                assert_eq!(*operation, row.operation);
                                assert_eq!(instruction.kind, SelectedInstructionKind::Crash);
                                assert!(
                                    instruction.provenance.edges.is_empty()
                                        && instruction.provenance.work.is_empty()
                                );
                                assert!(
                                    selected_instructions::terminator_successors(&block.terminator)
                                        .is_empty()
                                );
                                trapped = true;
                                break;
                            }
                            unexpected => panic!("unexpected assertion control {unexpected:?}"),
                        }
                    }
                    assert_eq!(trapped, input == 0);
                    assert_eq!(
                        bytes,
                        if trapped {
                            [0xa5; 8]
                        } else {
                            1_u64.to_le_bytes()
                        }
                    );
                });
            }
        }
    }

    #[test]
    fn operation_owned_nested_loops_execute_without_semantic_edges() {
        use crate::selection::construction::scalar_graph::operation_control::{self, OpenBlock};
        use selected_instructions::{
            SelectedBlockId, SelectedBlockOrigin, SelectedSuccessorRole, SelectedTerminator,
        };
        use semantic_vocabulary::BlockId;
        for target in targets() {
            for count in [0_u64, 1, 32] {
                with_builder(target, |builder, row| {
                    let source = BlockId::new(7).unwrap();
                    builder.open_block = Some(OpenBlock {
                        id: SelectedBlockId(0),
                        origin: SelectedBlockOrigin::Source(source),
                        start: 0,
                    });
                    builder.next_block = 1;
                    let place = PlaceId::new(19).unwrap();
                    let pointer = fragmented_scalar::constant(builder, row, place, 0).unwrap();
                    let home = ImageHome {
                        storage: fragmented_scalar::StorageAccess::Place {
                            indices: &[],
                            byte_offset: 0,
                        },
                        place,
                        pointer,
                        byte_offset: 0,
                    };
                    let codec = |byte_offset| ScalarImageCodec::Bytes {
                        byte_offset,
                        width: 8,
                        signed: false,
                    };
                    let decrement = |builder: &mut Builder<'_>, byte_offset| {
                        let current = codec(byte_offset).read(builder, row, home)?;
                        let one = fragmented_scalar::constant(builder, row, place, 1)?;
                        let next = fragmented_scalar::binary(
                            builder,
                            row,
                            place,
                            SelectedInstructionKind::WrappingSubtractI64,
                            current,
                            one,
                        )?;
                        codec(byte_offset).write(builder, row, home, next)
                    };
                    let preceding = OperationId::new(70).unwrap();
                    builder.pending_provenance = SelectedInstructionProvenance {
                        operations: vec![preceding],
                        work: vec![optimization_unit::WorkSettlement {
                            site: optimization_unit::PsiProvenance::Operation(preceding),
                            units: 3,
                        }],
                        ..Default::default()
                    };
                    operation_control::while_nonzero(
                        builder,
                        row,
                        |builder| codec(0).read(builder, row, home),
                        |builder| {
                            let two = fragmented_scalar::constant(builder, row, place, 2)?;
                            codec(8).write(builder, row, home, two)?;
                            operation_control::while_nonzero(
                                builder,
                                row,
                                |builder| codec(8).read(builder, row, home),
                                |builder| {
                                    let accumulated = codec(16).read(builder, row, home)?;
                                    let one = fragmented_scalar::constant(builder, row, place, 1)?;
                                    let next = fragmented_scalar::binary(
                                        builder,
                                        row,
                                        place,
                                        SelectedInstructionKind::WrappingAddI64,
                                        accumulated,
                                        one,
                                    )?;
                                    codec(16).write(builder, row, home, next)?;
                                    decrement(builder, 8)
                                },
                            )?;
                            decrement(builder, 0)
                        },
                    )
                    .unwrap();
                    let one = fragmented_scalar::constant(builder, row, place, 1).unwrap();
                    codec(24).write(builder, row, home, one).unwrap();
                    let continuation = builder.open_block.unwrap();
                    assert_eq!(builder.implementation_blocks.len(), 6);
                    let mut registers = vec![0; builder.registers.len()];
                    let mut bytes = vec![0; 40];
                    bytes[..8].copy_from_slice(&count.to_le_bytes());
                    bytes[32..].fill(0xa7);
                    let mut current = SelectedBlockId(0);
                    let mut visits = 0;
                    let mut charged = 0;
                    while current != continuation.id {
                        visits += 1;
                        assert!(visits <= 400, "operation loop did not terminate");
                        let block = builder
                            .implementation_blocks
                            .iter()
                            .find(|block| block.id == current)
                            .unwrap();
                        let compared = execute(&block.instructions, &mut registers, &mut bytes);
                        charged += block
                            .instructions
                            .iter()
                            .flat_map(|instruction| &instruction.provenance.work)
                            .map(|settlement| settlement.units)
                            .sum::<u64>();
                        assert!(
                            selected_instructions::terminator_instruction(&block.terminator)
                                .provenance
                                .work
                                .is_empty()
                        );
                        for successor in
                            selected_instructions::terminator_successors(&block.terminator)
                        {
                            assert_eq!(
                                successor.role,
                                SelectedSuccessorRole::OperationContinuation {
                                    operation: row.operation
                                }
                            );
                            assert_eq!(successor.psi_edge, None);
                            assert!(
                                successor.work.is_empty()
                                    && successor.bindings.is_empty()
                                    && successor.structural_bindings.is_empty()
                            );
                        }
                        current = match &block.terminator {
                            SelectedTerminator::Jump { successor, .. } => successor.block,
                            SelectedTerminator::ConditionalBranch {
                                when_nonzero,
                                when_zero,
                                ..
                            } => {
                                if compared.0 != compared.1 {
                                    when_nonzero.block
                                } else {
                                    when_zero.block
                                }
                            }
                            unexpected => panic!("unexpected operation control {unexpected:?}"),
                        };
                    }
                    execute(
                        &builder.instructions[continuation.start..],
                        &mut registers,
                        &mut bytes,
                    );
                    assert_eq!(u64::from_le_bytes(bytes[0..8].try_into().unwrap()), 0);
                    assert_eq!(
                        u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
                        count * 2
                    );
                    assert_eq!(u64::from_le_bytes(bytes[24..32].try_into().unwrap()), 1);
                    assert_eq!(&bytes[32..], &[0xa7; 8]);
                    assert_eq!(
                        charged, 3,
                        "preceding source work executes once, not once per native iteration"
                    );
                });
            }
        }
    }

    #[test]
    fn resident_scalar_decode_uses_actual_bytes_and_authored_extension() {
        let cases = [
            (
                LayoutPlacementReport::At { offset: 3 },
                0x0123_4567_89ab_cdef_u64,
                8,
            ),
            (
                LayoutPlacementReport::IntegerAt {
                    offset: 3,
                    stored_width: 24,
                    interpretation: IntegerInterpretation::Unsigned,
                },
                0x00ff_ffff,
                8,
            ),
            (
                LayoutPlacementReport::IntegerAt {
                    offset: 3,
                    stored_width: 24,
                    interpretation: IntegerInterpretation::Signed,
                },
                0xffff_ffff_ff80_0000,
                8,
            ),
            (
                LayoutPlacementReport::IntegerAt {
                    offset: 3,
                    stored_width: 8,
                    interpretation: IntegerInterpretation::Signed,
                },
                u64::MAX,
                8,
            ),
        ];
        for target in targets() {
            for (placement, expected, width) in cases {
                with_builder(target, |builder, row| {
                    let owner = PlaceId::new(9).unwrap();
                    let backing = fragmented_scalar::constant(builder, row, owner, 11).unwrap();
                    let decoded =
                        decode_scalar(builder, row, owner, backing, width, &[placement]).unwrap();
                    let mut bytes = [0xa5; 40];
                    bytes[14..22].copy_from_slice(&expected.to_le_bytes());
                    let original = bytes;
                    let mut registers = vec![0; builder.registers.len()];
                    execute(&builder.instructions, &mut registers, &mut bytes);
                    assert_eq!(registers[decoded.0 as usize], expected);
                    assert_eq!(bytes, original, "decoding must not mutate the resident");
                    assert!(builder.transport.memory.iter().all(|access| {
                        access.place == owner
                            && access.origin
                                == selected_instructions::SelectedMemoryAccessOrigin::Operation(
                                    row.operation,
                                )
                    }));
                });
            }
        }
    }

    #[test]
    fn resident_fragment_transfer_preserves_unrelated_output_bits() {
        for target in targets() {
            with_builder(target, |builder, row| {
                let owner = PlaceId::new(9).unwrap();
                let destination = PlaceId::new(10).unwrap();
                let backing = fragmented_scalar::constant(builder, row, owner, 0).unwrap();
                let placements = [
                    LayoutPlacementReport::Bits {
                        container: 1,
                        container_width: 16,
                        destination_lsb: 5,
                        source_lsb: 0,
                        width: 5,
                    },
                    LayoutPlacementReport::Bits {
                        container: 5,
                        container_width: 8,
                        destination_lsb: 2,
                        source_lsb: 5,
                        width: 3,
                    },
                ];
                let decoded = decode_scalar(builder, row, owner, backing, 1, &placements).unwrap();
                let pointer = fragmented_scalar::constant(builder, row, destination, 16).unwrap();
                let output = AuthoredBitFragments::new([AuthoredBitFragment {
                    container_byte_offset: 2,
                    container_width_bits: 16,
                    destination_lsb: 3,
                    source_lsb: 0,
                    width: 8,
                }]);
                fragmented_scalar::write_register(
                    builder,
                    row,
                    destination,
                    pointer,
                    2,
                    &output,
                    fragmented_scalar::StorageAccess::Place {
                        indices: &[],
                        byte_offset: 0,
                    },
                    decoded,
                    None,
                )
                .unwrap();
                let mut bytes = [0xa5; 32];
                bytes[1..3].copy_from_slice(&(0b10101_u16 << 5).to_le_bytes());
                bytes[5] = 0b110 << 2;
                let original = bytes;
                let mut registers = vec![0; builder.registers.len()];
                execute(&builder.instructions, &mut registers, &mut bytes);
                assert_eq!(registers[decoded.0 as usize], 0b110_10101);
                let previous = u16::from_le_bytes([original[18], original[19]]);
                let actual = u16::from_le_bytes([bytes[18], bytes[19]]);
                assert_eq!(actual, (previous & !(0xff << 3)) | (0b110_10101 << 3));
                assert_eq!(&bytes[..18], &original[..18]);
                assert_eq!(&bytes[20..], &original[20..]);
            });
        }
    }

    #[test]
    fn resident_scalar_decode_refuses_invalid_carriers_and_mixed_placements() {
        with_builder(NativeTarget::linux_x64(), |builder, row| {
            let owner = PlaceId::new(9).unwrap();
            let backing = fragmented_scalar::constant(builder, row, owner, 0).unwrap();
            assert!(
                decode_scalar(
                    builder,
                    row,
                    owner,
                    backing,
                    9,
                    &[LayoutPlacementReport::At { offset: 0 }]
                )
                .is_err()
            );
            assert!(
                decode_scalar(
                    builder,
                    row,
                    owner,
                    backing,
                    8,
                    &[
                        LayoutPlacementReport::At { offset: 0 },
                        LayoutPlacementReport::IntegerAt {
                            offset: 8,
                            stored_width: 8,
                            interpretation: IntegerInterpretation::Unsigned
                        },
                    ]
                )
                .is_err()
            );
            assert!(
                decode_scalar(
                    builder,
                    row,
                    owner,
                    backing,
                    1,
                    &[LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 8,
                        destination_lsb: 7,
                        source_lsb: 0,
                        width: 2
                    },]
                )
                .is_err()
            );
        });
    }

    #[test]
    fn resident_array_transfer_keeps_source_and_destination_strides_independent() {
        use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralFieldId};
        use terminal_psi::{BindingRelevance, StructuralFieldType, TerminalIdentityIndex};
        let scalar = StructuralTypeId::new(1).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        let types = vec![
            declaration(
                scalar,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                )),
            ),
            declaration(
                array,
                StructuralTypeShape::FixedArray {
                    element: scalar,
                    length: terminal_psi::ArrayExtent::Literal(2),
                },
            ),
        ];
        let field = StructuralFieldDeclaration {
            inline_carrier: None,
            id: StructuralFieldId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(array),
            storage: Some(AuthoredFieldStorage::Repeated {
                byte_offset: 16,
                element_stride: 12,
            }),
        };
        let scattered = aggregate_chunks(
            &types,
            &field,
            8,
            &[
                LayoutPlacementReport::At { offset: 40 },
                LayoutPlacementReport::At { offset: 20 },
            ],
        )
        .unwrap();
        assert_eq!(
            scattered,
            vec![
                ImageChunk {
                    source_offset: 20,
                    destination_offset: 0,
                    byte_count: 4
                },
                ImageChunk {
                    source_offset: 40,
                    destination_offset: 12,
                    byte_count: 4
                },
            ]
        );
        let contiguous = aggregate_chunks(
            &types,
            &field,
            8,
            &[LayoutPlacementReport::At { offset: 20 }],
        )
        .unwrap();
        assert_eq!(
            contiguous,
            vec![
                ImageChunk {
                    source_offset: 20,
                    destination_offset: 0,
                    byte_count: 4
                },
                ImageChunk {
                    source_offset: 24,
                    destination_offset: 12,
                    byte_count: 4
                },
            ]
        );
        assert!(
            aggregate_chunks(
                &types,
                &field,
                12,
                &[LayoutPlacementReport::At { offset: 20 }]
            )
            .is_err()
        );
        assert!(aggregate_chunks(&types, &field, 8, &[]).is_err());
    }

    fn image_fixture() -> (Vec<StructuralTypeDeclaration>, TerminalAccessLifecyclePlan) {
        use access_plans::{AccessFields, AccessPlan, BoundaryReach, validate_access_plan};
        use layout_plans::{LayoutFieldEntryReport, LayoutPlanReport};
        use semantic_vocabulary::{
            BoundaryMachineId, IntegerSign, IntegerType, ScalarType, StructuralDomainId,
            StructuralFieldId,
        };
        use terminal_psi::{
            BindingRelevance, StructuralFieldType, StructuralRecordStorage,
            TerminalAccessLifecycleExecution, TerminalAccessLifecycleOperation,
            TerminalAccessStorage, TerminalIdentityIndex,
        };

        let schema = StructuralTypeId::new(1).unwrap();
        let element = StructuralTypeId::new(2).unwrap();
        let array = StructuralTypeId::new(3).unwrap();
        let scalar = |sign, width| ScalarType::Integer(IntegerType::new(sign, width).unwrap());
        let field = |number, field_type| StructuralFieldDeclaration {
            inline_carrier: None,
            id: StructuralFieldId::new(number).unwrap(),
            identity: TerminalIdentityIndex(u32::try_from(number).unwrap()),
            relevance: BindingRelevance::Relevant,
            field_type,
            storage: None,
        };
        let types = vec![
            declaration(
                schema,
                StructuralTypeShape::Record {
                    fields: vec![
                        field(
                            1,
                            StructuralFieldType::Scalar(scalar(IntegerSign::Unsigned, 64)),
                        ),
                        field(
                            2,
                            StructuralFieldType::Scalar(scalar(IntegerSign::Unsigned, 8)),
                        ),
                        field(
                            3,
                            StructuralFieldType::Scalar(scalar(IntegerSign::Signed, 64)),
                        ),
                        field(4, StructuralFieldType::Structural(array)),
                    ],
                    storage: StructuralRecordStorage::Packed,
                },
            ),
            declaration(
                element,
                StructuralTypeShape::PrimitiveScalar(scalar(IntegerSign::Unsigned, 16)),
            ),
            declaration(
                array,
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(2),
                },
            ),
        ];
        let entry = |field: &str, placement| LayoutFieldEntryReport {
            field: field.into(),
            member_identity: None,
            placement,
        };
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 0,
            offsets: None,
            size: Some(32),
            align: 1,
            entries: vec![
                entry(
                    "signed",
                    LayoutPlacementReport::IntegerAt {
                        offset: 4,
                        stored_width: 24,
                        interpretation: IntegerInterpretation::Signed,
                    },
                ),
                entry("plain", LayoutPlacementReport::At { offset: 20 }),
                entry("array", LayoutPlacementReport::At { offset: 16 }),
                entry(
                    "small",
                    LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 8,
                        destination_lsb: 2,
                        source_lsb: 0,
                        width: 3,
                    },
                ),
                entry("array", LayoutPlacementReport::At { offset: 12 }),
                entry(
                    "small",
                    LayoutPlacementReport::Bits {
                        container: 3,
                        container_width: 8,
                        destination_lsb: 1,
                        source_lsb: 3,
                        width: 5,
                    },
                ),
            ],
        };
        let access = AccessFields::inaccessible(&layout).unwrap();
        let placement = validate_access_plan(AccessPlan {
            layout,
            access,
            reach: BoundaryReach::default(),
        })
        .unwrap();
        let image_fields = placement
            .access()
            .plan()
            .entries()
            .iter()
            .map(|entry| {
                StructuralFieldId::new(match entry.field() {
                    "plain" => 1,
                    "small" => 2,
                    "signed" => 3,
                    "array" => 4,
                    name => panic!("unexpected fixture slot {name}"),
                })
                .unwrap()
            })
            .collect();
        let plan = TerminalAccessLifecyclePlan {
            target: None,
            boundary: BoundaryMachineId::new(1).unwrap(),
            operation: TerminalAccessLifecycleOperation::Initialize,
            execution: TerminalAccessLifecycleExecution::CompilerPrimitive {
                profile_type: None,
                profile_requirements: Vec::new(),
            },
            storage: TerminalAccessStorage {
                owner: terminal_psi::TerminalAccessOwner::Extent,
                carrier: StructuralTypeId::new(4).unwrap(),
                base: StructuralFieldId::new(5).unwrap(),
                length: StructuralFieldId::new(6).unwrap(),
                granted: StructuralDomainId::new(1).unwrap(),
                vacant: Some(StructuralDomainId::new(2).unwrap()),
                resident: Some(StructuralDomainId::new(3).unwrap()),
            },
            schema,
            image_fields,
            policy_identity: TerminalIdentityIndex(10),
            policy_plan_machine_identity: TerminalIdentityIndex(11),
            schema_identity: TerminalIdentityIndex(12),
            placement,
        };
        (types, plan)
    }

    #[test]
    fn resident_complete_image_stages_commits_and_decodes_without_touching_rejected_backing() {
        let (types, plan) = image_fixture();
        for target in targets() {
            for supplied in [0, 1] {
                with_builder(target, |builder, row| {
                    let owner = PlaceId::new(9).unwrap();
                    let scratch = owner;
                    let output = PlaceId::new(11).unwrap();
                    let value_pointer =
                        fragmented_scalar::constant(builder, row, owner, 0).unwrap();
                    let value = ImageHome {
                        storage: fragmented_scalar::StorageAccess::Place {
                            indices: &[],
                            byte_offset: 0,
                        },
                        place: owner,
                        pointer: value_pointer,
                        byte_offset: 5,
                    };
                    let mut staged = ImageHome::scratch(builder, row, scratch, 35).unwrap();
                    staged.byte_offset = 3;
                    initialization::stage_image(&types, builder, row, &plan, value, staged)
                        .unwrap();
                    let backing = fragmented_scalar::constant(builder, row, owner, 128).unwrap();
                    let available = fragmented_scalar::constant(builder, row, owner, 48).unwrap();
                    let supply =
                        fragmented_scalar::constant(builder, row, owner, supplied).unwrap();
                    let decision =
                        decide_constants(builder, row, owner, backing, available, 32, 1, 0, supply)
                            .unwrap();
                    let staged_start =
                        fragmented_scalar::constant(builder, row, scratch, 67).unwrap();
                    initialization::commit(builder, row, owner, staged_start, backing, &decision)
                        .unwrap();
                    if supplied == 1 {
                        let output_pointer =
                            fragmented_scalar::constant(builder, row, output, 192).unwrap();
                        transfer_image(
                            &types,
                            row,
                            builder,
                            &plan,
                            ImageHome {
                                storage: fragmented_scalar::StorageAccess::Place {
                                    indices: &[],
                                    byte_offset: 0,
                                },
                                place: owner,
                                pointer: backing,
                                byte_offset: 0,
                            },
                            ImageHome {
                                storage: fragmented_scalar::StorageAccess::Place {
                                    indices: &[],
                                    byte_offset: 0,
                                },
                                place: output,
                                pointer: output_pointer,
                                byte_offset: 0,
                            },
                            ImageDirection::Decode,
                        )
                        .unwrap();
                    }
                    let mut bytes = [0xa5; 240];
                    bytes[5..13].copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
                    bytes[13] = 0b1101_0101;
                    bytes[21..29].copy_from_slice(&(-128_i64).to_le_bytes());
                    bytes[29..33].copy_from_slice(&[0x34, 0x12, 0x78, 0x56]);
                    let original = bytes;
                    let mut registers = vec![0; builder.registers.len()];
                    let frame = [(
                        selected_instructions::FrameStorageSlotId::Local(
                            selected_instructions::LocalStorageSlotId::Boundary {
                                operation: row.operation,
                            },
                        ),
                        64,
                    )];
                    execute_with_frame(&builder.instructions, &mut registers, &mut bytes, &frame);
                    let mut expected_image = [0; 32];
                    expected_image[0] = (0b1101_0101 & 7) << 2;
                    expected_image[3] = (0b1101_0101 >> 3) << 1;
                    expected_image[4..7].copy_from_slice(&[0x80, 0xff, 0xff]);
                    expected_image[12..14].copy_from_slice(&original[29..31]);
                    expected_image[16..18].copy_from_slice(&original[31..33]);
                    expected_image[20..28].copy_from_slice(&original[5..13]);
                    assert_eq!(&bytes[67..99], &expected_image);
                    assert_eq!(&bytes[..67], &original[..67], "input must remain unchanged");
                    assert_eq!(&bytes[99..128], &original[99..128]);
                    assert_eq!(
                        &bytes[160..192],
                        &original[160..192],
                        "allocation tail must remain unchanged"
                    );
                    if supplied == 1 {
                        assert_eq!(&bytes[128..160], &expected_image);
                        assert_eq!(&bytes[192..200], &original[5..13]);
                        assert_eq!(bytes[200], original[13]);
                        assert_eq!(&bytes[201..208], &original[201..208]);
                        assert_eq!(&bytes[208..220], &original[21..33]);
                        assert_eq!(&bytes[220..], &original[220..]);
                    } else {
                        assert_eq!(&bytes[192..], &original[192..]);
                        assert_eq!(&bytes[128..160], &original[128..160]);
                    }
                });
            }
        }
    }

    #[test]
    fn resident_authored_value_codec_keeps_signed_fields_fragments_and_array_strides() {
        use semantic_vocabulary::{AuthoredRecordStorage, IntegerSign, IntegerType, ScalarType};
        use terminal_psi::StructuralRecordStorage;
        let (mut types, plan) = image_fixture();
        let StructuralTypeShape::Record { fields, storage } = &mut types[0].shape else {
            panic!("fixture record");
        };
        *storage = StructuralRecordStorage::Authored(AuthoredRecordStorage {
            byte_size: 40,
            byte_alignment: 1,
        });
        fields[0].storage = Some(AuthoredFieldStorage::Direct { byte_offset: 0 });
        fields[1].storage = Some(AuthoredFieldStorage::Fragmented(AuthoredBitFragments::new(
            [AuthoredBitFragment {
                container_byte_offset: 8,
                container_width_bits: 16,
                destination_lsb: 3,
                source_lsb: 0,
                width: 8,
            }],
        )));
        fields[2].storage = Some(AuthoredFieldStorage::Scalar {
            byte_offset: 12,
            stored_scalar: Some(ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            )),
        });
        fields[3].storage = Some(AuthoredFieldStorage::Repeated {
            byte_offset: 16,
            element_stride: 8,
        });
        for target in targets() {
            with_builder(target, |builder, row| {
                let owner = PlaceId::new(9).unwrap();
                let scratch = owner;
                let result = PlaceId::new(11).unwrap();
                let value = ImageHome {
                    storage: fragmented_scalar::StorageAccess::Place {
                        indices: &[],
                        byte_offset: 0,
                    },
                    place: owner,
                    pointer: fragmented_scalar::constant(builder, row, owner, 0).unwrap(),
                    byte_offset: 0,
                };
                let staged = ImageHome::scratch(builder, row, scratch, 32).unwrap();
                initialization::stage_image(&types, builder, row, &plan, value, staged).unwrap();
                let staged_end = builder.instructions.len();
                let restored = ImageHome {
                    storage: fragmented_scalar::StorageAccess::Place {
                        indices: &[],
                        byte_offset: 0,
                    },
                    place: result,
                    pointer: fragmented_scalar::constant(builder, row, result, 128).unwrap(),
                    byte_offset: 0,
                };
                transfer_image(
                    &types,
                    row,
                    builder,
                    &plan,
                    staged,
                    restored,
                    ImageDirection::Decode,
                )
                .unwrap();
                let mut bytes = [0xa5; 192];
                bytes[..8].copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
                let original_container = 0xa5a5_u16;
                let input_container = (original_container & !(0xff << 3)) | (0xd5 << 3);
                bytes[8..10].copy_from_slice(&input_container.to_le_bytes());
                bytes[12] = 0x80;
                bytes[16..18].copy_from_slice(&[0x34, 0x12]);
                bytes[24..26].copy_from_slice(&[0x78, 0x56]);
                let original = bytes;
                let mut registers = vec![0; builder.registers.len()];
                let frame = [(
                    selected_instructions::FrameStorageSlotId::Local(
                        selected_instructions::LocalStorageSlotId::Boundary {
                            operation: row.operation,
                        },
                    ),
                    64,
                )];
                execute_with_frame(
                    &builder.instructions[..staged_end],
                    &mut registers,
                    &mut bytes,
                    &frame,
                );
                assert_eq!(
                    &bytes[68..71],
                    &[0x80, 0xff, 0xff],
                    "signed input must extend before encoding"
                );
                assert_eq!(&bytes[76..78], &[0x34, 0x12]);
                assert_eq!(&bytes[80..82], &[0x78, 0x56]);
                assert_eq!(&bytes[..64], &original[..64]);
                assert_eq!(&bytes[96..], &original[96..]);
                bytes[68] = 0x81;
                execute(
                    &builder.instructions[staged_end..],
                    &mut registers,
                    &mut bytes,
                );
                assert_eq!(&bytes[128..136], &original[..8]);
                assert_eq!(&bytes[136..138], &input_container.to_le_bytes());
                assert_eq!(bytes[140], 0x81);
                assert_eq!(&bytes[144..146], &original[16..18]);
                assert_eq!(&bytes[152..154], &original[24..26]);
                for position in [
                    138, 139, 141, 142, 143, 146, 147, 148, 149, 150, 151, 154, 167, 168, 191,
                ] {
                    assert_eq!(
                        bytes[position], original[position],
                        "unrelated byte {position}"
                    );
                }
            });
        }
    }

    #[test]
    fn resident_initialization_rejects_before_copy_and_commits_only_the_complete_image() {
        let cases = [
            (16, 16, 1, 0, 1),
            (17, 16, 1, 1, 1),
            (16, 16, 0, 1, 2),
            (17, 16, 0, 1, 2),
            (u64::MAX, 0, 0, 1, 0),
            (16, 7, 1, 1, 0),
            (16, 16, 2, 1, 2),
        ];
        for target in targets() {
            for (base, length, supply, outcome, reason) in cases {
                with_builder(target, |builder, row| {
                    let owner = PlaceId::new(9).unwrap();
                    let staged = fragmented_scalar::constant(builder, row, owner, 0).unwrap();
                    let backing = fragmented_scalar::constant(builder, row, owner, base).unwrap();
                    let available =
                        fragmented_scalar::constant(builder, row, owner, length).unwrap();
                    let supply = fragmented_scalar::constant(builder, row, owner, supply).unwrap();
                    let decision =
                        decide_constants(builder, row, owner, backing, available, 8, 8, 0, supply)
                            .unwrap();
                    initialization::commit(builder, row, owner, staged, backing, &decision)
                        .unwrap();
                    let mut bytes = [0xa5; 40];
                    bytes[..8].copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
                    let original = bytes;
                    let mut registers = vec![0; builder.registers.len()];
                    execute(&builder.instructions, &mut registers, &mut bytes);
                    assert_eq!(registers[decision.outcome_tag.0 as usize], outcome);
                    assert_eq!(registers[decision.failure_tag.0 as usize], reason);
                    assert_eq!(
                        registers[decision.ready.0 as usize],
                        u64::from(outcome == 0)
                    );
                    if outcome == 0 {
                        assert_eq!(&bytes[16..24], &original[..8]);
                        assert_eq!(&bytes[..16], &original[..16]);
                        assert_eq!(
                            &bytes[24..],
                            &original[24..],
                            "allocation tail must remain untouched"
                        );
                        assert_eq!(registers[decision.copy_length.0 as usize], 8);
                    } else {
                        assert_eq!(
                            bytes, original,
                            "rejection must preserve both input and backing"
                        );
                        assert_eq!(registers[decision.copy_length.0 as usize], 0);
                    }
                });
            }
        }
    }

    #[test]
    fn resident_initialization_geometry_handles_high_addresses_and_nonzero_residue() {
        for (base, modulus, residue) in
            [(u64::MAX - 7, 8, 0), (u64::MAX - 3, 8, 4), (u64::MAX, 1, 0)]
        {
            with_builder(NativeTarget::macos_arm64(), |builder, row| {
                let owner = PlaceId::new(9).unwrap();
                let base = fragmented_scalar::constant(builder, row, owner, base).unwrap();
                let available = fragmented_scalar::constant(builder, row, owner, u64::MAX).unwrap();
                let supply = fragmented_scalar::constant(builder, row, owner, 1).unwrap();
                let decision = decide_constants(
                    builder, row, owner, base, available, 8, modulus, residue, supply,
                )
                .unwrap();
                let mut registers = vec![0; builder.registers.len()];
                execute(&builder.instructions, &mut registers, &mut []);
                assert_eq!(registers[decision.ready.0 as usize], 1);
                assert_eq!(registers[decision.copy_length.0 as usize], 8);
                for (modulus, residue) in [(0, 0), (3, 0), (8, 8)] {
                    let decision = decide_constants(
                        builder, row, owner, base, available, 8, modulus, residue, supply,
                    )
                    .unwrap();
                    let mut registers = vec![0; builder.registers.len()];
                    execute(&builder.instructions, &mut registers, &mut []);
                    assert_eq!(registers[decision.ready.0 as usize], 0);
                    assert_eq!(registers[decision.copy_length.0 as usize], 0);
                }
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn decide_constants(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        owner: PlaceId,
        base: VirtualRegisterId,
        available: VirtualRegisterId,
        image_length: u64,
        modulus: u64,
        residue: u64,
        supply: VirtualRegisterId,
    ) -> Result<outcome::PlacementDecision, SelectedInstructionError> {
        let modulus = fragmented_scalar::constant(builder, row, owner, modulus)?;
        let residue = fragmented_scalar::constant(builder, row, owner, residue)?;
        outcome::decide(
            builder,
            row,
            owner,
            base,
            available,
            image_length,
            modulus,
            residue,
            supply,
        )
    }
}
pub(crate) mod view {
    use super::{
        Builder, ImageHome, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, TerminalAccessLifecyclePlan, failure, fragmented_scalar,
        operand_indices, outcome, owner_word, resource_profile, result_layout,
    };
    use crate::SelectedInstructionError;

    pub(crate) fn emit(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        plan: &TerminalAccessLifecyclePlan,
    ) -> Result<(), SelectedInstructionError> {
        use crate::selection::construction::scalar_graph::operation_control;
        use crate::selection::construction::scalar_graph::structural::{leaf_copy, record};
        let LegalizedScalarInstructionKind::AccessLifecycle {
            operands: abstract_operations::AccessLifecycleOperands::View { storage, profile },
            indices,
            result,
            ..
        } = &row.kind
        else {
            return Err(failure(
                "borrowed placement emitter lost its exact operands",
            ));
        };
        let terminal_psi::TerminalAccessLifecycleExecution::CompilerPrimitive {
            profile_type: Some(profile_type),
            ..
        } = plan.execution
        else {
            return Err(failure("borrowed placement lost its original profile type"));
        };
        let types = &source
            .structural
            .as_ref()
            .ok_or_else(|| failure("borrowed placement has no structural signature"))?
            .structural_types;
        let layout = result_layout::ResultLayout::derive(types, result.structural_type, plan)?;
        if !matches!(layout.returned, result_layout::ReturnedLayout::Borrowed)
            || layout.ready_owner.shape.byte_size != 8
        {
            return Err(failure(
                "borrowed placement result changed its reference or empty rejection",
            ));
        }
        let mut remaining = indices.as_slice();
        let storage_indices = operand_indices(storage, &mut remaining)?;
        let profile_indices = operand_indices(profile, &mut remaining)?;
        if !remaining.is_empty() {
            return Err(failure(
                "borrowed placement retained unrelated operand selectors",
            ));
        }
        let projected = leaf_copy::project_operand(source, row, builder, storage, storage_indices)?;
        let owner_home = ImageHome {
            storage: fragmented_scalar::StorageAccess::Place {
                indices: projected.indices,
                byte_offset: 0,
            },
            place: storage.place,
            pointer: projected
                .pointer
                .ok_or_else(|| failure("borrowed extent has no descriptor home"))?,
            byte_offset: projected.byte_offset,
        };
        let base = owner_word(types, row, builder, plan, owner_home, plan.storage.base)?;
        let available = owner_word(types, row, builder, plan, owner_home, plan.storage.length)?;
        let projected = leaf_copy::project_operand(source, row, builder, profile, profile_indices)?;
        let profile_home = ImageHome {
            storage: fragmented_scalar::StorageAccess::Place {
                indices: projected.indices,
                byte_offset: 0,
            },
            place: profile.place,
            pointer: projected
                .pointer
                .ok_or_else(|| failure("borrowed profile has no original home"))?,
            byte_offset: projected.byte_offset,
        };
        let pointer = record::initialize_storage(row, builder, result.place, layout.shape)?;
        let home = ImageHome {
            storage: fragmented_scalar::StorageAccess::Place {
                indices: &[],
                byte_offset: 0,
            },
            place: result.place,
            pointer,
            byte_offset: 0,
        };
        let scratch = ImageHome::scratch(
            builder,
            row,
            result.place,
            resource_profile::compatibility::COMPATIBILITY_SCRATCH_BYTES,
        )?;
        let assessment = resource_profile::compatibility::assess(
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
        let required = plan
            .placement
            .layout()
            .size
            .ok_or_else(|| failure("borrowed placement requires fixed view geometry"))?;
        let decision = outcome::decide(
            builder,
            row,
            result.place,
            base,
            available,
            required,
            assessment.modulus,
            assessment.residue,
            assessment.compatible,
        )?;
        let required = fragmented_scalar::constant(builder, row, result.place, required)?;
        outcome::write_failure(
            builder,
            row,
            home,
            &layout,
            &decision,
            [required, available],
            [assessment.modulus, assessment.residue],
        )?;
        operation_control::when_nonzero(builder, row, decision.ready, |builder| {
            outcome::write_word(builder, row, home, layout.ready_owner.byte_offset, 8, base)
        })?;
        outcome::write_word(builder, row, home, layout.tag, 4, decision.outcome_tag)?;
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }
}

fn failure(reason: &'static str) -> SelectedInstructionError {
    SelectedInstructionError::custody_because(reason)
}

fn coordinate(value: u64) -> Result<u32, SelectedInstructionError> {
    value
        .try_into()
        .map_err(|_| failure("resident image exceeds native byte coordinates"))
}

fn offset(base: u32, displacement: u64) -> Result<u32, SelectedInstructionError> {
    base.checked_add(coordinate(displacement)?)
        .ok_or_else(|| failure("resident image byte offset overflows"))
}

pub(crate) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::AccessLifecycle {
        boundary,
        operands,
        indices,
        result,
    } = &row.kind
    else {
        return Err(failure("resident emitter requires a lifecycle operation"));
    };
    let signature = source
        .structural
        .as_ref()
        .ok_or_else(|| failure("resident extraction lost its structural signature"))?;
    let declarations = &signature.structural_types;
    let plan = declarations
        .access_lifecycles()
        .iter()
        .find(|plan| plan.boundary == *boundary && plan.operation == operands.operation())
        .ok_or_else(|| failure("resident extraction lost its exact retained lifecycle"))?;
    let (storage, profile) = match operands {
        AccessLifecycleOperands::Initialize { .. } => {
            return initialization::emit(source, row, builder, plan);
        }
        AccessLifecycleOperands::View { .. } => return view::emit(source, row, builder, plan),
        AccessLifecycleOperands::Relocate { .. } => {
            return relocation::emit(source, row, builder, plan);
        }
        AccessLifecycleOperands::MoveOut { storage, profile } => (storage, profile),
    };
    let terminal_psi::TerminalAccessLifecycleExecution::CompilerPrimitive {
        profile_type: Some(profile_type),
        ..
    } = plan.execution
    else {
        return Err(failure("extraction lost its admitted profile type"));
    };
    let mut backing = storage.clone();
    backing.path.extend(
        plan.storage
            .owner_storage_path(declarations)
            .ok_or_else(|| failure("extraction lost its owner backing projection"))?,
    );
    let storage = &backing;
    let mut remaining = indices.as_slice();
    let storage_indices = operand_indices(storage, &mut remaining)?;
    let profile_indices = operand_indices(profile, &mut remaining)?;
    if !remaining.is_empty() {
        return Err(failure(
            "extraction retained selectors outside its operands",
        ));
    }
    let result_layout =
        structural_reference_input::case_layout(result.structural_type, declarations)
            .ok_or_else(|| failure("resident extraction has no native result layout"))?;
    let [moved] = result_layout.cases.as_slice() else {
        return Err(failure("resident extraction must have one Moved case"));
    };
    let [owner_field, value_field] = moved.fields.as_slice() else {
        return Err(failure(
            "resident extraction must return its owner and complete value",
        ));
    };
    let result_declaration =
        terminal_psi::find_structural_type(declarations, result.structural_type)
            .ok_or_else(|| failure("resident extraction result declaration is missing"))?;
    let StructuralTypeShape::Sum { cases } = &result_declaration.shape else {
        return Err(failure(
            "resident extraction result is not its authenticated sum",
        ));
    };
    let [case] = cases.as_slice() else {
        return Err(failure("resident extraction result changed cases"));
    };
    let [owner, value] = case.fields.as_slice() else {
        return Err(failure("resident extraction result changed fields"));
    };
    if owner.field_type.as_structural() != Some(plan.storage.carrier)
        || value.field_type.as_structural() != Some(plan.schema)
    {
        return Err(failure(
            "resident extraction result changed its owner or schema",
        ));
    }
    let result_pointer =
        record::initialize_storage(row, builder, result.place, result_layout.shape)?;
    crate::selection::construction::scalar_graph::structural::leaf_copy::copy_operand(
        source,
        row,
        builder,
        storage,
        storage_indices,
        crate::selection::construction::scalar_graph::structural::leaf_copy::CopyEnd {
            place: result.place,
            pointer: result_pointer,
            byte_offset: owner_field.byte_offset,
            elements: &[],
        },
        owner_field.shape,
    )?;
    let owner_home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place {
            indices: &[],
            byte_offset: 0,
        },
        place: result.place,
        pointer: result_pointer,
        byte_offset: owner_field.byte_offset,
    };
    let backing = owner_word(
        declarations,
        row,
        builder,
        plan,
        owner_home,
        plan.storage.base,
    )?;
    let available = owner_word(
        declarations,
        row,
        builder,
        plan,
        owner_home,
        plan.storage.length,
    )?;
    let projected =
        crate::selection::construction::scalar_graph::structural::leaf_copy::project_operand(
            source,
            row,
            builder,
            profile,
            profile_indices,
        )?;
    let profile_home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place {
            indices: projected.indices,
            byte_offset: 0,
        },
        place: profile.place,
        pointer: projected
            .pointer
            .ok_or_else(|| failure("extraction profile has no addressable home"))?,
        byte_offset: projected.byte_offset,
    };
    let scratch = ImageHome::scratch(
        builder,
        row,
        result.place,
        resource_profile::compatibility::COMPATIBILITY_SCRATCH_BYTES,
    )?;
    let assessment = resource_profile::compatibility::assess(
        declarations,
        profile_type,
        builder,
        row,
        profile_home,
        scratch,
        available,
        &plan.placement,
        plan.operation,
    )?;
    use crate::selection::construction::scalar_graph::operation_control;
    operation_control::require_nonzero(builder, row, assessment.well_formed)?;
    let decision = outcome::decide(
        builder,
        row,
        result.place,
        backing,
        available,
        plan.placement.access().layout_size_bytes(),
        assessment.modulus,
        assessment.residue,
        assessment.compatible,
    )?;
    operation_control::require_nonzero(builder, row, decision.ready)?;
    transfer_image(
        declarations,
        row,
        builder,
        plan,
        ImageHome {
            storage: fragmented_scalar::StorageAccess::Backing,
            place: storage.place,
            pointer: backing,
            byte_offset: 0,
        },
        ImageHome {
            storage: fragmented_scalar::StorageAccess::Place {
                indices: &[],
                byte_offset: 0,
            },
            place: result.place,
            pointer: result_pointer,
            byte_offset: value_field.byte_offset,
        },
        ImageDirection::Decode,
    )?;
    builder
        .transport
        .pointers
        .push((result.place, result_pointer));
    Ok(())
}

fn operand_indices<'indices>(
    argument: &terminal_psi::StructuralArgument,
    remaining: &mut &'indices [legalized_operations::LegalizedRuntimeIndexOperand],
) -> Result<&'indices [legalized_operations::LegalizedRuntimeIndexOperand], SelectedInstructionError>
{
    let count = argument
        .path
        .iter()
        .filter(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
        .count();
    let (selected, rest) = remaining
        .split_at_checked(count)
        .ok_or_else(|| failure("lifecycle operand lost a runtime selector"))?;
    *remaining = rest;
    Ok(selected)
}

fn owner_word(
    types: &[StructuralTypeDeclaration],
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    plan: &TerminalAccessLifecyclePlan,
    home: ImageHome,
    identity: semantic_vocabulary::StructuralFieldId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let owner = terminal_psi::find_structural_type(types, plan.storage.carrier)
        .ok_or_else(|| failure("lifecycle owner declaration is missing"))?;
    let StructuralTypeShape::Record { fields, .. } = &owner.shape else {
        return Err(failure("lifecycle owner is not its retained record"));
    };
    let field = fields
        .iter()
        .find(|field| field.id == identity)
        .ok_or_else(|| failure("lifecycle lost an authenticated geometry field"))?;
    let region = field_region(types, plan.storage.carrier, field)?;
    if region.shape.byte_size != 8 {
        return Err(failure("lifecycle geometry must occupy a native u64 word"));
    }
    ScalarImageCodec::field(types, plan.storage.carrier, field)?.read(builder, row, home)
}

#[derive(Clone, Copy)]
pub(crate) struct ImageHome<'indices> {
    storage: fragmented_scalar::StorageAccess<'indices>,
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
}

impl ImageHome<'_> {
    fn scratch(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        anchor: PlaceId,
        byte_size: u32,
    ) -> Result<Self, SelectedInstructionError> {
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot,
            SelectedMemoryAccessRole,
        };
        let slot = LocalStorageSlotId::Boundary {
            operation: row.operation,
        };
        if byte_size == 0
            || builder
                .transport
                .local_slots
                .iter()
                .any(|item| item.id == slot)
        {
            return Err(failure(
                "resident scratch must have one nonempty operation-owned allocation",
            ));
        }
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size,
                alignment: 8,
            });
        let pointer = crate::selection::construction::scalar_graph::structural::transport_register(
            builder, anchor, 0,
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            anchor,
            0,
            byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit_frame_address(
            FrameStorageSlotId::Local(slot),
            pointer,
            crate::selection::construction::scalar_graph::structural::provenance(row),
        )?;
        Ok(Self {
            storage: fragmented_scalar::StorageAccess::Local(slot),
            place: anchor,
            pointer,
            byte_offset: 0,
        })
    }
}

#[derive(Clone, Copy)]
enum ImageDirection {
    Encode,
    Decode,
}

fn transfer_image(
    declarations: &[StructuralTypeDeclaration],
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    plan: &TerminalAccessLifecyclePlan,
    image: ImageHome,
    value: ImageHome,
    direction: ImageDirection,
) -> Result<(), SelectedInstructionError> {
    let declaration = terminal_psi::find_structural_type(declarations, plan.schema)
        .ok_or_else(|| failure("resident image schema is missing"))?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err(failure(
            "resident image requires its retained record schema",
        ));
    };
    let access = plan.placement.access().plan().entries();
    if access.len() != plan.image_fields.len() {
        return Err(failure("resident image lost its complete field-slot join"));
    }
    for (identity, slot) in plan.image_fields.iter().zip(access) {
        let field = fields
            .iter()
            .find(|field| field.id == *identity)
            .ok_or_else(|| failure("resident image field is absent from its schema"))?;
        let placements = plan
            .placement
            .layout()
            .entries
            .iter()
            .filter(|entry| entry.field == slot.field())
            .map(|entry| entry.placement)
            .collect::<Vec<_>>();
        if let Some(scalar) = field.field_type.scalar_type() {
            let width = runtime_abi::ScalarStorageCodec::from_scalar(scalar)
                .ok_or_else(|| failure("resident scalar has no native storage codec"))?
                .byte_size();
            let access_codec = ScalarImageCodec::placement(u32::from(width), &placements)?;
            let owned_codec = ScalarImageCodec::field(declarations, plan.schema, field)?;
            let (source_home, source_codec, destination_home, destination_codec) = match direction {
                ImageDirection::Encode => (value, &owned_codec, image, &access_codec),
                ImageDirection::Decode => (image, &access_codec, value, &owned_codec),
            };
            let bits = source_codec.read(builder, row, source_home)?;
            destination_codec.write(builder, row, destination_home, bits)?;
        } else {
            let region = field_region(declarations, plan.schema, field)?;
            let chunks = aggregate_chunks(
                declarations,
                field,
                u64::from(region.shape.byte_size),
                &placements,
            )?;
            for chunk in chunks {
                let image_offset = offset(image.byte_offset, u64::from(chunk.source_offset))?;
                let value_offset = offset(
                    offset(value.byte_offset, u64::from(region.byte_offset))?,
                    u64::from(chunk.destination_offset),
                )?;
                let (source, source_offset, destination, destination_offset) = match direction {
                    ImageDirection::Encode => (value, value_offset, image, image_offset),
                    ImageDirection::Decode => (image, image_offset, value, value_offset),
                };
                for position in 0..chunk.byte_count {
                    let bits = fragmented_scalar::load_container(
                        builder,
                        row,
                        source.place,
                        source.pointer,
                        offset(source_offset, u64::from(position))?,
                        1,
                        source.storage,
                    )?;
                    fragmented_scalar::store_container(
                        builder,
                        row,
                        destination.place,
                        destination.pointer,
                        offset(destination_offset, u64::from(position))?,
                        1,
                        destination.storage,
                        bits,
                        None,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn field_region(
    declarations: &[StructuralTypeDeclaration],
    schema: StructuralTypeId,
    field: &StructuralFieldDeclaration,
) -> Result<calling_conventions::PackedFieldLayout, SelectedInstructionError> {
    let geometry = structural_reference_input::reference_formation_geometry(
        schema,
        &[StructuralPathSegment::Field(field.identity)],
        declarations,
    )
    .ok_or_else(|| failure("resident field has no native owned storage projection"))?;
    if !geometry.pointer_slots.is_empty()
        || !geometry.elements.is_empty()
        || *geometry.selected != field.field_type
    {
        return Err(failure(
            "resident image field is not its direct owned storage",
        ));
    }
    let shape = structural_reference_input::field_storage_shape(field, declarations)
        .ok_or_else(|| failure("resident field has no native storage shape"))?;
    Ok(calling_conventions::PackedFieldLayout {
        shape,
        byte_offset: geometry.byte_offset,
    })
}

#[derive(Debug, PartialEq, Eq)]
struct ImageChunk {
    source_offset: u32,
    destination_offset: u32,
    byte_count: u32,
}

fn aggregate_chunks(
    declarations: &[StructuralTypeDeclaration],
    field: &StructuralFieldDeclaration,
    byte_count: u64,
    placements: &[LayoutPlacementReport],
) -> Result<Vec<ImageChunk>, SelectedInstructionError> {
    let mut offsets = placements
        .iter()
        .map(|placement| match placement {
            LayoutPlacementReport::At { offset } => coordinate(*offset),
            _ => Err(failure(
                "resident aggregate requires whole or repeated byte placement",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    offsets.sort_unstable();
    let declaration = field
        .field_type
        .as_structural()
        .and_then(|root| terminal_psi::find_structural_type(declarations, root))
        .ok_or_else(|| failure("resident aggregate lost its structural type"))?;
    if let StructuralTypeShape::FixedArray {
        element,
        length: terminal_psi::ArrayExtent::Literal(length),
    } = declaration.shape
    {
        let element_shape = structural_reference_input::shape(element, declarations)
            .ok_or_else(|| failure("resident array element has no native storage"))?;
        let element_size = u64::from(element_shape.byte_size);
        if (offsets.len() != 1 && u64::try_from(offsets.len()).ok() != Some(length))
            || element_size.checked_mul(length) != Some(byte_count)
        {
            return Err(failure(
                "resident element placements do not cover the complete array",
            ));
        }
        let destination_stride = field
            .storage
            .as_ref()
            .and_then(AuthoredFieldStorage::element_stride)
            .unwrap_or(element_size);
        let mut chunks = Vec::new();
        for element_position in 0..length {
            let source_offset = if offsets.len() == 1 {
                offset(
                    offsets[0],
                    element_position
                        .checked_mul(element_size)
                        .ok_or_else(|| failure("resident array source offset overflows"))?,
                )?
            } else {
                *offsets
                    .get(
                        usize::try_from(element_position)
                            .map_err(|_| failure("resident array exceeds host coordinates"))?,
                    )
                    .ok_or_else(|| failure("resident array lost an element placement"))?
            };
            chunks.push(ImageChunk {
                source_offset,
                destination_offset: coordinate(
                    element_position
                        .checked_mul(destination_stride)
                        .ok_or_else(|| failure("resident array destination offset overflows"))?,
                )?,
                byte_count: coordinate(element_size)?,
            });
        }
        return Ok(chunks);
    }
    let [source_offset] = offsets.as_slice() else {
        return Err(failure(
            "resident non-array aggregate needs one complete image placement",
        ));
    };
    Ok(vec![ImageChunk {
        source_offset: *source_offset,
        destination_offset: 0,
        byte_count: coordinate(byte_count)?,
    }])
}

#[cfg(test)]
fn decode_scalar(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    owner: PlaceId,
    backing: VirtualRegisterId,
    width: u32,
    placements: &[LayoutPlacementReport],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    ScalarImageCodec::placement(width, placements)?.read(
        builder,
        row,
        ImageHome {
            storage: fragmented_scalar::StorageAccess::Place {
                indices: &[],
                byte_offset: 0,
            },
            place: owner,
            pointer: backing,
            byte_offset: 0,
        },
    )
}

enum ScalarImageCodec {
    Bytes {
        byte_offset: u32,
        width: u32,
        signed: bool,
    },
    Fragments(AuthoredBitFragments),
}

impl ScalarImageCodec {
    fn field(
        declarations: &[StructuralTypeDeclaration],
        schema: StructuralTypeId,
        field: &StructuralFieldDeclaration,
    ) -> Result<Self, SelectedInstructionError> {
        if let Some(AuthoredFieldStorage::Fragmented(fragments)) = &field.storage {
            return Ok(Self::Fragments(fragments.clone()));
        }
        let region = field_region(declarations, schema, field)?;
        let width = region.shape.byte_size;
        if !(1..=8).contains(&width) {
            return Err(failure(
                "resident owned scalar exceeds its register carrier",
            ));
        }
        let signed = matches!(
            field.storage.as_ref().and_then(AuthoredFieldStorage::stored_scalar),
            Some(semantic_vocabulary::ScalarType::Integer(integer))
                if integer.sign() == semantic_vocabulary::IntegerSign::Signed
        );
        Ok(Self::Bytes {
            byte_offset: region.byte_offset,
            width,
            signed,
        })
    }

    fn placement(
        width: u32,
        placements: &[LayoutPlacementReport],
    ) -> Result<Self, SelectedInstructionError> {
        if !(1..=8).contains(&width) {
            return Err(failure(
                "resident scalar exceeds the native register carrier",
            ));
        }
        match placements {
            [LayoutPlacementReport::At { offset }] => Ok(Self::Bytes {
                byte_offset: coordinate(*offset)?,
                width,
                signed: false,
            }),
            [
                LayoutPlacementReport::IntegerAt {
                    offset,
                    stored_width,
                    interpretation,
                },
            ] => {
                if *stored_width == 0
                    || *stored_width > u64::from(width) * 8
                    || !stored_width.is_multiple_of(8)
                {
                    return Err(failure("resident integer has an invalid stored width"));
                }
                Ok(Self::Bytes {
                    byte_offset: coordinate(*offset)?,
                    width: coordinate(*stored_width / 8)?,
                    signed: *interpretation == IntegerInterpretation::Signed,
                })
            }
            fragments if !fragments.is_empty() => {
                let fragments = fragments
                    .iter()
                    .map(|fragment| {
                        let LayoutPlacementReport::Bits {
                            container,
                            container_width,
                            destination_lsb,
                            source_lsb,
                            width: bit_count,
                        } = *fragment
                        else {
                            return Err(failure(
                                "resident scalar mixes incompatible placement families",
                            ));
                        };
                        if container_width == 0
                            || container_width > 64
                            || !container_width.is_multiple_of(8)
                            || destination_lsb
                                .checked_add(bit_count)
                                .is_none_or(|end| end > container_width)
                            || source_lsb
                                .checked_add(bit_count)
                                .is_none_or(|end| end > u64::from(width) * 8)
                        {
                            return Err(failure(
                                "resident scalar fragment exceeds its container or carrier",
                            ));
                        }
                        Ok(AuthoredBitFragment {
                            container_byte_offset: container,
                            container_width_bits: container_width,
                            destination_lsb,
                            source_lsb,
                            width: bit_count,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::Fragments(AuthoredBitFragments::new(fragments)))
            }
            _ => Err(failure("resident scalar has no image placement")),
        }
    }

    fn read(
        &self,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        home: ImageHome,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        match self {
            Self::Fragments(fragments) => {
                let (first, _) = fragments
                    .byte_envelope()
                    .ok_or_else(|| failure("resident scalar fragments have no envelope"))?;
                fragmented_scalar::read_register(
                    builder,
                    row,
                    home.place,
                    home.pointer,
                    offset(home.byte_offset, first)?,
                    fragments,
                    home.storage,
                )
            }
            Self::Bytes {
                byte_offset,
                width,
                signed,
            } => {
                let value = fragmented_scalar::load_container(
                    builder,
                    row,
                    home.place,
                    home.pointer,
                    offset(home.byte_offset, u64::from(*byte_offset))?,
                    *width,
                    home.storage,
                )?;
                if !signed || *width == 8 {
                    return Ok(value);
                }
                let sign = fragmented_scalar::constant(
                    builder,
                    row,
                    home.place,
                    1_u64 << (*width * 8 - 1),
                )?;
                let biased = fragmented_scalar::binary(
                    builder,
                    row,
                    home.place,
                    SelectedInstructionKind::BitwiseXorI64,
                    value,
                    sign,
                )?;
                fragmented_scalar::binary(
                    builder,
                    row,
                    home.place,
                    SelectedInstructionKind::WrappingSubtractI64,
                    biased,
                    sign,
                )
            }
        }
    }

    fn write(
        &self,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        home: ImageHome,
        bits: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        match self {
            Self::Bytes {
                byte_offset, width, ..
            } => fragmented_scalar::store_container(
                builder,
                row,
                home.place,
                home.pointer,
                offset(home.byte_offset, u64::from(*byte_offset))?,
                *width,
                home.storage,
                bits,
                None,
            ),
            Self::Fragments(fragments) => {
                let (first, _) = fragments
                    .byte_envelope()
                    .ok_or_else(|| failure("resident scalar fragments have no envelope"))?;
                fragmented_scalar::write_register(
                    builder,
                    row,
                    home.place,
                    home.pointer,
                    offset(home.byte_offset, first)?,
                    fragments,
                    home.storage,
                    bits,
                    None,
                )
            }
        }
    }
}
