/*
We evaluate scalar, borrowed-argument, owned-argument, and normalized-foreign
call ABI placements and carrier normalizations for instruction selection.
*/

use crate::selection::model::SelectedInstructionError;
use crate::structural_inputs::structural_reference_input::owned_storage_shape;
use crate::structural_inputs::structural_unit_input::{accepts_graph, parameter_views};
use calling_conventions::{IndirectPointerLocation, ValueLocation};
use register_model::{RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{SelectedConstraintKeys, SelectedInstructionKind};
use semantic_vocabulary::{IntegerSign, ScalarType, ValueId};
use terminal_psi::StructuralAccess;
pub(crate) mod borrowed_argument {
    use super::{
        CallSignature, LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction,
        StructuralAccess, ValueShape, evaluate_call_plan, scalar_shape,
    };
    use crate::selection::established_view_input::{
        accepts_at, accepts_view_loan, completed_descriptor, element_view_type, view_access,
    };
    use crate::selection::scalar_call_abi::scalar_value_type;
    use crate::structural_inputs::structural_reference_input::{
        bounded_byte_field_owner, fixed_byte_array_view, fixed_element_array_view,
        internal_borrow_shape, owned_storage_shape, runtime_borrowed_projection,
        stored_value_shape,
    };

    pub(crate) fn validate_borrowed_argument(
        source: &LegalizedScalarFunction,
        call: &LegalizedScalarCall,
        site: semantic_vocabulary::InvocationSite,
        argument_index: usize,
    ) -> Option<()> {
        super::input_site::use_cut(source, site)?;
        let signature = source.structural.as_ref()?;
        let LegalizedScalarArgument::Structural {
            semantic,
            target,
            indices,
        } = call.arguments.get(argument_index)?
        else {
            return None;
        };
        let structural_argument_index = call.arguments[..argument_index]
            .iter()
            .filter(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. }))
            .count();
        if call
            .claim_transfers
            .iter()
            .any(|transfer| transfer.argument_index as usize == structural_argument_index)
        {
            return None;
        }
        if semantic
            .path
            .iter()
            .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
            || crate::selection::reference_input::borrowed_argument(
                source, semantic, target, indices,
            )
            .is_some()
        {
            return validate_reference_argument(
                source,
                call,
                site,
                argument_index,
                semantic,
                target,
                indices,
            );
        }
        if source
            .parameters
            .len()
            .checked_add(signature.parameters.len())?
            != source.call_plan.parameters.len()
            || source
                .parameters
                .iter()
                .zip(&source.call_plan.parameters)
                .any(|(parameter, placement)| {
                    scalar_shape(parameter.scalar_type) != Some(placement.shape)
                        || parameter.placement != *placement
                })
        {
            return None;
        }
        let parameters =
            crate::structural_inputs::structural_unit_input::parameter_views(signature);
        let exclusive = matches!(
            semantic.access,
            terminal_psi::exclusive_borrow_pair_pattern!()
        );
        let local = crate::selection::primitive_local_input::local(source, semantic.place);
        let aggregate_home = crate::selection::aggregate_result_input::home(source, semantic.place);
        let aggregate_block_home =
            crate::selection::aggregate_result_input::block_home(source, semantic.place);
        let storage_root = aggregate_home
            .map(|(_, result)| result.structural_type)
            .or_else(|| aggregate_block_home.map(|(_, declaration)| declaration.structural_type))
            .or_else(|| {
                signature
                    .parameters
                    .iter()
                    .find(|parameter| parameter.semantic.place == semantic.place)
                    .filter(|parameter| {
                        matches!(
                            parameter.semantic.access,
                            terminal_psi::mutating_access_pair_pattern!()
                        ) || parameter.semantic.access == semantic.access
                    })
                    .map(|parameter| parameter.semantic.structural_type)
            });
        let storage_loan = storage_root
            .filter(|root| {
                owned_storage_shape(*root, &signature.structural_types).is_some()
                    || terminal_psi::find_structural_type(&signature.structural_types, *root)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::Reference { .. }
                            )
                        })
                        && stored_value_shape(*root, &signature.structural_types).is_some()
            })
            .and_then(|root| {
                let (selected, offset, elements) = runtime_borrowed_projection(
                    root,
                    &semantic.path,
                    semantic.access,
                    Some(target.structural_type),
                    &signature.structural_types,
                )?;
                let shape = internal_borrow_shape(selected, &signature.structural_types)?;
                (target.root_structural_type == root
                    && matches!(
                        terminal_semantics::borrowed_referent_presentation(
                            signature.structural_types.iter(),
                            selected,
                            target.structural_type,
                        ),
                        Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                    )
                    && target.source_byte_offset == offset
                    && crate::structural_inputs::runtime_indices_match(indices, &elements))
                .then_some(shape)
            });
        let local_loan = aggregate_home
            .filter(|(_, result)| {
                matches!(
                    semantic.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                ) && result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            })
            .map(|(_, result)| terminal_psi::StructuralParameterDeclaration {
                place: result.place,
                position: 0,
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access: StructuralAccess::Owned,
                qualifications: result.qualifications.clone(),
                projected_qualifications: result.projected_qualifications.clone(),
                additional_input_qualifications: result.additional_input_qualifications.clone(),
            });
        let address_join = crate::selection::address_join_input::join(source, semantic.place);
        let view_root = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == semantic.place)
            .map(|parameter| &parameter.semantic)
            .or(local_loan.as_ref())
            .or(aggregate_block_home.map(|(_, declaration)| declaration))
            .or(address_join.map(|(_, joined)| joined))
            .or_else(|| {
                source.block_structural_parameters().find(|parameter| {
                    parameter.place == semantic.place
                        && signature.structural_types.iter().any(|declaration| {
                            declaration.id == parameter.structural_type
                                && matches!(
                                    declaration.shape,
                                    terminal_psi::StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                                    )
                                )
                        })
                })
            });
        let byte_view = view_root.and_then(|root| {
            fixed_byte_array_view(
                root,
                semantic,
                target.structural_type,
                &signature.structural_types,
            )
        });
        let byte_field = crate::selection::byte_owner_input::view_argument_at(
            source, site, semantic, target, indices,
        );
        let byte_owner = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == semantic.place)
            .and_then(|parameter| {
                bounded_byte_field_owner(
                    &parameter.semantic,
                    semantic,
                    target.structural_type,
                    &signature.structural_types,
                )
            });
        let element_view = view_root.and_then(|root| {
            fixed_element_array_view(
                root,
                semantic,
                target.structural_type,
                &signature.structural_types,
            )
        });
        let established_view_loan = element_view_type(source, semantic.place)
            .or_else(|| {
                if !semantic.path.is_empty() {
                    return None;
                }
                view_access(source, semantic.place)?;
                crate::selection::established_view_input::view_type(source, semantic.place)
            })
            .and_then(|view_type| {
                let (selected, offset, elements) = runtime_borrowed_projection(
                    view_type,
                    &semantic.path,
                    semantic.access,
                    Some(target.structural_type),
                    &signature.structural_types,
                )?;
                if target.root_structural_type != view_type
                    || !terminal_semantics::borrowed_referent_matches(
                        signature.structural_types.iter(),
                        selected,
                        target.structural_type,
                    )
                    || target.source_byte_offset != offset
                    || target.fixed_array_length.is_some()
                    || target.element_stride.is_some()
                    || !crate::structural_inputs::runtime_indices_match(indices, &elements)
                {
                    return None;
                }
                let shape = internal_borrow_shape(selected, &signature.structural_types)?;
                Some(shape)
            });
        let window = byte_view
            .map(|(offset, _)| offset)
            .or(element_view.map(|(offset, _, _)| offset));
        let destination_shape =
            internal_borrow_shape(target.structural_type, &signature.structural_types)?;
        let projected_loan = projected_loan_shape(source, semantic, target, indices);
        let block_forward = semantic.path.is_empty()
            && terminal_semantics::borrowed_referent_presentation(
                signature.structural_types.iter(),
                target.root_structural_type,
                target.structural_type,
            ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            && target.source_byte_offset == 0
            && indices.is_empty()
            && matches!(
                target.source,
                target_operations::TargetStructuralArgumentSource::BlockParameter { .. }
            );
        let shape = if byte_field.is_some() {
            destination_shape
        } else if let Some((_, joined)) = address_join {
            if !borrow_access_permitted(joined.access, semantic.access) {
                return None;
            }
            if let Some(offset) = window {
                if target.root_structural_type != joined.structural_type
                    || target.source_byte_offset != offset
                    || !indices.is_empty()
                {
                    return None;
                }
                destination_shape
            } else {
                let (selected, offset, elements) = runtime_borrowed_projection(
                    joined.structural_type,
                    &semantic.path,
                    semantic.access,
                    Some(target.structural_type),
                    &signature.structural_types,
                )?;
                if target.root_structural_type != joined.structural_type
                    || !terminal_semantics::borrowed_referent_matches(
                        signature.structural_types.iter(),
                        selected,
                        target.structural_type,
                    )
                    || target.source_byte_offset != offset
                    || !crate::structural_inputs::runtime_indices_match(indices, &elements)
                {
                    return None;
                }
                internal_borrow_shape(selected, &signature.structural_types)?
            }
        } else if let Some(shape) = storage_loan {
            shape
        } else if block_forward && aggregate_block_home.is_none() {
            destination_shape
        } else if let Some((offset, _)) = byte_view {
            if offset != target.source_byte_offset {
                return None;
            }
            destination_shape
        } else if let Some((offset, capacity)) = byte_owner {
            if offset != target.source_byte_offset {
                return None;
            }
            ValueShape::borrowed_reference(u32::try_from(capacity.checked_add(8)?).ok()?, 8)
        } else if let Some((offset, _)) = byte_field {
            if offset != target.source_byte_offset {
                return None;
            }
            destination_shape
        } else if let Some((offset, _, _)) = element_view {
            if offset != target.source_byte_offset {
                return None;
            }
            destination_shape
        } else if let Some(shape) = established_view_loan {
            shape
        } else if let Some((_, result, _, _)) = local {
            if target.structural_type != result.structural_type
                || target.root_structural_type != result.structural_type
                || !semantic.path.is_empty()
                || target.source_byte_offset != 0
            {
                return None;
            }
            internal_borrow_shape(result.structural_type, &signature.structural_types)?
        } else if let Some(shape) = projected_loan {
            shape
        } else if exclusive
            || signature.structural_types.iter().any(|declaration| {
                declaration.id == target.structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                    )
            })
        {
            return None;
        } else {
            destination_shape
        };
        let expected = expected_signature_plan(source, call, argument_index, shape)?;
        let no_indirect_home = !exclusive
            && !block_forward
            && address_join.is_none()
            && storage_loan.is_none()
            && byte_view.is_none()
            && byte_field.is_none()
            && byte_owner.is_none()
            && element_view.is_none()
            && established_view_loan.is_none()
            && projected_loan.is_none();
        if source.attachment.is_some_and(|attachment| {
            !terminal_psi::find_structural_type(&signature.structural_types, attachment).is_some()
        }) || (!parameters.is_empty()
            && !crate::structural_inputs::structural_unit_input::accepts_graph(
                &source.call_plan,
                &parameters,
                &signature.structural_types,
            ))
            || call.call_plan != expected
            || (no_indirect_home
                && (semantic.access != StructuralAccess::SharedBorrow || !semantic.path.is_empty()))
            || target.place != semantic.place
            || target.access != semantic.access
            || target.path != semantic.path
            || (no_indirect_home && target.root_structural_type != target.structural_type)
            || target.shape != shape
            || (no_indirect_home && target.source_byte_offset != 0)
            || target.fixed_array_length
                != byte_view
                    .map(|(_, length)| length)
                    .or(element_view.map(|(_, length, _)| length))
            || target.element_stride
                != byte_view
                    .map(|_| 1)
                    .or(element_view.map(|(_, _, stride)| stride))
            || Some(&target.destination) != expected.parameters.get(argument_index)
        {
            return None;
        }
        match &target.source {
            _ if byte_field.is_some() => {}
            target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
                let (producer, _) = aggregate_home?;
                let home_view =
                    local_loan.is_some() && (byte_view.is_some() || element_view.is_some());
                let descriptor_home = established_view_loan.is_some()
                    && view_access(source, semantic.place).is_some_and(|access| {
                        access == semantic.access
                            || access == StructuralAccess::MutableBorrow
                                && semantic.access == StructuralAccess::SharedBorrow
                    })
                    && completed_descriptor(source, semantic.place)
                        .is_some_and(|(actual, _)| actual == producer);
                if (storage_loan.is_none() && !home_view && !descriptor_home)
                    || *psi_operation != producer
                    || super::input_site::producer_before_use(source, producer, site).is_none()
                {
                    return None;
                }
            }
            target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation,
            } => {
                let (producer, _, _, _) = local?;
                if *psi_operation != producer
                    || super::input_site::producer_before_use(source, producer, site).is_none()
                {
                    return None;
                }
            }
            target_operations::TargetStructuralArgumentSource::Placement(placement) => {
                let parameter = signature
                    .parameters
                    .iter()
                    .find(|parameter| parameter.semantic.place == semantic.place)?;
                if target.root_structural_type != parameter.semantic.structural_type
                    || *placement != parameter.target.placement
                {
                    return None;
                }
            }
            target_operations::TargetStructuralArgumentSource::BlockParameter { block, place }
                if address_join.is_some() =>
            {
                let (owner, joined) = address_join?;
                if *block != owner || *place != joined.place {
                    return None;
                }
            }
            target_operations::TargetStructuralArgumentSource::BlockParameter { block, place }
                if aggregate_block_home.is_some()
                    && (storage_loan.is_some()
                        || byte_view.is_some()
                        || element_view.is_some()) =>
            {
                let (owner, declaration) = aggregate_block_home?;
                if *block != owner || *place != declaration.place {
                    return None;
                }
            }
            target_operations::TargetStructuralArgumentSource::EstablishedElementView {
                ..
            } if established_view_loan.is_some() => {
                if semantic.path.is_empty() {
                    accepts_at(source, site, target)?;
                } else {
                    accepts_view_loan(source, site, target)?;
                }
            }
            target_operations::TargetStructuralArgumentSource::EstablishedByteView { .. }
            | target_operations::TargetStructuralArgumentSource::EstablishedElementView {
                ..
            }
            | target_operations::TargetStructuralArgumentSource::BlockParameter { .. } => {
                accepts_at(source, site, target)?;
            }
        }
        Some(())
    }

    fn validate_reference_argument(
        source: &LegalizedScalarFunction,
        call: &LegalizedScalarCall,
        site: semantic_vocabulary::InvocationSite,
        argument_index: usize,
        semantic: &terminal_psi::StructuralArgument,
        target: &target_operations::TargetStructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Option<()> {
        if let target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } =
            target.source
        {
            super::input_site::producer_before_use(source, psi_operation, site)?;
        }
        crate::selection::reference_input::borrowed_argument(source, semantic, target, indices)?;
        let shape = target.shape;
        let expected = expected_signature_plan(source, call, argument_index, shape)?;
        (call.call_plan == expected
            && Some(&target.destination) == expected.parameters.get(argument_index))
        .then_some(())
    }

    fn borrow_access_permitted(source: StructuralAccess, requested: StructuralAccess) -> bool {
        use StructuralAccess::{MutableBorrow, SharedBorrow, WriteOnlyBorrow};
        matches!(
            (source, requested),
            (
                MutableBorrow,
                SharedBorrow | MutableBorrow | WriteOnlyBorrow
            ) | (SharedBorrow, SharedBorrow)
                | (WriteOnlyBorrow, WriteOnlyBorrow)
        )
    }

    fn projected_loan_shape(
        source: &LegalizedScalarFunction,
        semantic: &terminal_psi::StructuralArgument,
        target: &target_operations::TargetStructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Option<ValueShape> {
        let signature = source.structural.as_ref()?;
        let root = crate::selection::arrival_parameter_input::parameter(source, semantic.place)?;
        if !borrow_access_permitted(root.access, semantic.access) {
            return None;
        }
        let (referent, offset, elements) = runtime_borrowed_projection(
            root.structural_type,
            &semantic.path,
            semantic.access,
            Some(target.structural_type),
            &signature.structural_types,
        )?;
        let shape = internal_borrow_shape(referent, &signature.structural_types)?;
        (target.root_structural_type == root.structural_type
            && target.structural_type == referent
            && target.source_byte_offset == offset
            && crate::structural_inputs::runtime_indices_match(indices, &elements))
        .then_some(shape)
    }

    fn expected_signature_plan(
        source: &LegalizedScalarFunction,
        call: &LegalizedScalarCall,
        argument_index: usize,
        shape: calling_conventions::ValueShape,
    ) -> Option<calling_conventions::CallPlan> {
        evaluate_call_plan(
            source.call_plan.policy,
            &CallSignature {
                parameters: call
                    .arguments
                    .iter()
                    .enumerate()
                    .map(|(position, argument)| match argument {
                        LegalizedScalarArgument::Scalar {
                            source: value,
                            placement,
                        } => {
                            let shape = scalar_shape(scalar_value_type(source, *value)?)?;
                            (placement.shape == shape).then_some(shape)
                        }
                        LegalizedScalarArgument::Structural { target, .. } => {
                            Some(if position == argument_index {
                                shape
                            } else {
                                target.shape
                            })
                        }
                    })
                    .collect::<Option<Vec<_>>>()?,
                result: call
                    .result_placement
                    .as_ref()
                    .map(|placement| placement.shape),
            },
        )
        .ok()
    }
}
pub(crate) mod input_site {
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarTerminator,
    };
    use semantic_vocabulary::{BlockId, InvocationSite, OperationId};

    pub(crate) fn installation_call(
        source: &LegalizedScalarFunction,
        operation: OperationId,
        role: target_operations::InstallationCallRole,
    ) -> Option<&legalized_operations::LegalizedScalarCall> {
        let mut rows = source
            .instructions()
            .filter(|row| row.operation == operation);
        let row = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        let legalized_operations::LegalizedScalarInstructionKind::InstallStaticInterrupt {
            calls,
            ..
        } = &row.kind
        else {
            return None;
        };
        Some(
            &calls[match role {
                target_operations::InstallationCallRole::Validate => 0,
                target_operations::InstallationCallRole::Work => 1,
            }],
        )
    }

    pub(crate) fn use_cut(
        source: &LegalizedScalarFunction,
        site: InvocationSite,
    ) -> Option<(BlockId, usize)> {
        let mut found = None;
        for block in &source.blocks {
            match site {
                InvocationSite::Operation(operation) => {
                    for (position, row) in block.instructions.iter().enumerate() {
                        if row.operation == operation
                            && found.replace((block.id, position)).is_some()
                        {
                            return None;
                        }
                    }
                }
                InvocationSite::InvokeNoNormal(edge) => {
                    if matches!(&block.terminator,
                        LegalizedScalarTerminator::InvokeNoNormal { psi_edge, .. } if *psi_edge == edge)
                        && found
                            .replace((block.id, block.instructions.len()))
                            .is_some()
                    {
                        return None;
                    }
                }
                InvocationSite::ExternalCompletion(edge) => {
                    if matches!(&block.terminator,
                        LegalizedScalarTerminator::ExternalCompletion { psi_edge, .. } if *psi_edge == edge)
                        && found
                            .replace((block.id, block.instructions.len()))
                            .is_some()
                    {
                        return None;
                    }
                }
            }
        }
        found
    }

    pub(crate) fn producer_before_use(
        source: &LegalizedScalarFunction,
        producer: OperationId,
        site: InvocationSite,
    ) -> Option<(BlockId, usize, &LegalizedScalarInstruction)> {
        let used = use_cut(source, site)?;
        let mut producers = source.blocks.iter().flat_map(|block| {
            block
                .instructions
                .iter()
                .enumerate()
                .filter(move |(_, row)| row.operation == producer)
                .map(move |(position, row)| (block.id, position, row))
        });
        let produced = producers.next()?;
        if producers.next().is_some() || produced.0 == used.0 && produced.1 >= used.1 {
            return None;
        }
        Some(produced)
    }

    pub(crate) fn no_normal_result<'source>(
        source: &'source LegalizedScalarFunction,
        site: InvocationSite,
        call: &legalized_operations::LegalizedScalarCall,
    ) -> Option<&'source abstract_operations::AbstractFunctionResult> {
        let InvocationSite::InvokeNoNormal(edge) = site else {
            return None;
        };
        let (block, position) = use_cut(source, site)?;
        let block = source
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)?;
        if position != block.instructions.len() {
            return None;
        }
        match &block.terminator {
            LegalizedScalarTerminator::InvokeNoNormal {
                psi_edge,
                call: retained,
                declared_result,
                ..
            } if *psi_edge == edge && retained == call => Some(declared_result),
            _ => None,
        }
    }
}
pub(crate) mod normalized_foreign {
    use super::{CallSignature, LegalizedScalarFunction, ValueLocation, ValueShape, scalar_shape};
    use crate::SelectedInstructionError;
    use calling_conventions::validate_boundary_entry_plan;
    use legalized_operations::{LegalizedNormalizedForeignCall, LegalizedValueDefinition};
    use register_environment::ValidatedTargetRegisterEnvironment;
    use register_model::{RegisterConstraintKey, RegisterInstructionConstraint};
    use semantic_vocabulary::InvocationSite;
    use target_operations::{
        TargetBoundaryResult, TargetStructuralArgumentSource, TargetUnitScalarArgumentSource,
    };

    fn entry_family(
        call: &LegalizedNormalizedForeignCall,
    ) -> register_model::RegisterConstraintFamily {
        match call.binding.mechanism {
            target_operations::NormalizedForeignCallMechanism::Import { .. }
            | target_operations::NormalizedForeignCallMechanism::Component { .. }
            | target_operations::NormalizedForeignCallMechanism::Table { .. } => {
                register_model::RegisterConstraintFamily::Call
            }
            target_operations::NormalizedForeignCallMechanism::SystemCall(_) => {
                register_model::RegisterConstraintFamily::SystemCall
            }
        }
    }

    fn private_callback_ordinals(call: &LegalizedNormalizedForeignCall) -> Vec<usize> {
        let mut ordinals: Vec<usize> = call
            .callbacks
            .iter()
            .filter(|callback| {
                matches!(
                    callback.destination,
                    calling_conventions::NativePlace::Parameter(_)
                )
            })
            .map(|callback| {
                callback.application.native_ordinal as usize + call.binding.dispatch_operand_count()
            })
            .collect();
        ordinals.sort_unstable();
        ordinals
    }

    pub(crate) fn structural_parameter_positions(
        call: &LegalizedNormalizedForeignCall,
    ) -> impl Iterator<Item = usize> + '_ {
        let private_ordinals = private_callback_ordinals(call);
        (0..call.binding.operand_placements().len()).filter(move |position| {
            !private_ordinals.contains(position)
                && !call
                    .scalar_arguments
                    .iter()
                    .any(|argument| argument.parameter_index as usize == *position)
        })
    }

    fn borrowed_block_argument_root<'source>(
        source: &'source LegalizedScalarFunction,
        operation: semantic_vocabulary::OperationId,
        argument: &target_operations::TargetStructuralArgument,
    ) -> Option<(
        semantic_vocabulary::BlockId,
        &'source terminal_psi::StructuralParameterDeclaration,
    )> {
        if let Some(join) = crate::selection::address_join_input::join(source, argument.place) {
            return Some(join);
        }
        let parameter =
            crate::selection::arrival_parameter_input::parameter(source, argument.place)?;
        if crate::selection::established_view_input::view_type(source, argument.place)
            != Some(parameter.structural_type)
            && crate::selection::established_view_input::element_view_type(source, argument.place)
                != Some(parameter.structural_type)
        {
            return None;
        }
        crate::selection::established_view_input::accepts(source, operation, argument)?;
        let block = source.blocks.iter().find(|block| {
            block
                .structural_parameters
                .iter()
                .any(|actual| actual == parameter)
        })?;
        Some((block.id, parameter))
    }

    pub(crate) fn call_key(
        call: &LegalizedNormalizedForeignCall,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> Option<RegisterConstraintKey> {
        let family = entry_family(call);
        environment
            .selected_keys()
            .rows
            .call_normalized_foreign
            .into_iter()
            .find(|key| {
                key.family == family
                    && environment
                        .constraint(*key)
                        .is_some_and(|row| row.operands.is_empty())
            })
    }

    pub(crate) fn call_constraint(
        call: &LegalizedNormalizedForeignCall,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> Option<RegisterInstructionConstraint> {
        let key = call_key(call, environment)?;
        selected_instructions::calls::constraints::instantiate_foreign_call_constraint(
            call,
            environment.constraint(key)?,
            environment.physical(),
            |register| environment.fixed_register_view(register),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate(
        function: usize,
        source: &LegalizedScalarFunction,
        backings: &crate::selection::foreign_scalar_backings::ForeignScalarBackings<'_>,
        call: &LegalizedNormalizedForeignCall,
        invocation_site: InvocationSite,
        result: Option<LegalizedValueDefinition>,
        key: RegisterConstraintKey,
        constraint: &RegisterInstructionConstraint,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> Result<(), SelectedInstructionError> {
        macro_rules! invalid {
            () => {
                SelectedInstructionError::unsupported_shape(function)
            };
        }
        if invocation_site.operation().is_none()
            && (!matches!(call.result, TargetBoundaryResult::Unit)
                || result.is_some()
                || !call.callbacks.is_empty()
                || !call.structural_arguments.is_empty())
        {
            return Err(invalid!());
        }
        let plan = &call.binding.boundary_entry_plan;
        let operands = call.binding.operand_placements();
        if call.binding.native_target() != Some(environment.target())
            || !call.binding.has_valid_entry_mechanism()
            || call.binding.provider_plan_report_identity()
                != call
                    .provider_execution
                    .provider_plan_report_identity()
                    .get()
        {
            return Err(invalid!());
        }
        let pointer_size =
            u16::try_from(environment.target().pointer_size).map_err(|_| invalid!())?;
        let pointer_alignment =
            u16::try_from(environment.target().pointer_alignment).map_err(|_| invalid!())?;
        if call.callbacks.len() != plan.call.callback_materializations.len() {
            return Err(invalid!());
        }
        let mut private_ordinals = Vec::new();
        for (index, callback) in call.callbacks.iter().enumerate() {
            let ordinal =
                usize::try_from(callback.application.native_ordinal).map_err(|_| invalid!())?;
            let placed = plan.call.parameters.get(ordinal) == Some(&callback.application.placement);
            let destination_admitted = match &callback.destination {
                calling_conventions::NativePlace::Parameter(parameter) => {
                    *parameter == callback.application.parameter
                        && callback.field_placements.is_none()
                        && placed
                }
                calling_conventions::NativePlace::Field { parameter, .. } => {
                    *parameter == callback.application.parameter
                        && callback.field_placements.is_some()
                        && placed
                        && matches!(
                            callback.application.placement.locations.as_slice(),
                            [ValueLocation::Register { value_byte_offset: 0, byte_size, .. }]
                                if *byte_size == u32::from(pointer_size)
                        )
                }
            };
            if Some(callback.terminal_operation) != invocation_site.operation()
                || callback.registrar_boundary_entry_plan != *plan
                || callback.registrar_context != call.callbacks[0].registrar_context
                || !callback.callback_function.is_valid()
                || callback.callback_function.callback_thunk_placement_index()
                    != Some(callback.placement_index)
                || callback.application.shape != callback.application.placement.shape
                || callback.application.shape
                    != ValueShape::integer(u32::from(pointer_size), pointer_alignment)
                || !destination_admitted
                || !plan
                    .call
                    .callback_materializations
                    .iter()
                    .any(|materialization| materialization.destination == callback.destination)
                || call.callbacks[..index].iter().any(|prior| {
                    prior.placement_index == callback.placement_index
                        || prior.destination == callback.destination
                })
            {
                return Err(invalid!());
            }
            if matches!(
                callback.destination,
                calling_conventions::NativePlace::Parameter(_)
            ) {
                private_ordinals.push(ordinal + call.binding.dispatch_operand_count());
            }
        }
        if operands.len()
            != call.scalar_arguments.len()
                + call.structural_arguments.len()
                + private_ordinals.len()
        {
            return Err(invalid!());
        }
        for (index, argument) in call.scalar_arguments.iter().enumerate() {
            if scalar_shape(argument.source.scalar_type()).is_none()
                || index.checked_sub(1).is_some_and(|previous| {
                    call.scalar_arguments[previous].parameter_index >= argument.parameter_index
                })
                || private_ordinals.contains(&(argument.parameter_index as usize))
                || operands.get(argument.parameter_index as usize) != Some(&argument.placement)
                || scalar_shape(argument.source.scalar_type()) != Some(argument.placement.shape)
                || match &argument.source {
                    TargetUnitScalarArgumentSource::IntegerImmediate {
                        scalar_type, value, ..
                    } => semantic_vocabulary::ScalarTerm::integer(*scalar_type, *value).is_err(),
                    _ => false,
                }
            {
                return Err(invalid!());
            }
        }
        if call.structural_arguments.len() != call.structural_indices.len() {
            return Err(invalid!());
        }
        let mut structural_shapes = Vec::new();
        for ((index, argument), indices) in structural_parameter_positions(call)
            .zip(&call.structural_arguments)
            .zip(&call.structural_indices)
        {
            let signature = source.structural.as_ref().ok_or_else(|| invalid!())?;
            let parameter = signature
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == argument.place);
            let (root_type, root_access, expected_source, root, presentation_debt) =
                if let Some(parameter) = parameter {
                    (
                        parameter.semantic.structural_type,
                        parameter.semantic.access,
                        TargetStructuralArgumentSource::Placement(
                            parameter.target.placement.clone(),
                        ),
                        parameter.semantic.clone(),
                        false,
                    )
                } else if let Some((producer, result)) =
                    crate::selection::aggregate_result_input::home(source, argument.place)
                {
                    (
                        result.structural_type,
                        terminal_psi::StructuralAccess::Owned,
                        TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: producer,
                        },
                        terminal_psi::StructuralParameterDeclaration {
                            place: result.place,
                            position: 0,
                            structural_type: result.structural_type,
                            access: terminal_psi::StructuralAccess::Owned,
                            multiplicity: result.multiplicity,
                            qualifications: result.qualifications.clone(),
                            projected_qualifications: result.projected_qualifications.clone(),
                            additional_input_qualifications: result
                                .additional_input_qualifications
                                .clone(),
                        },
                        !result.claims.is_empty()
                            || !result.qualification_establishments.is_empty(),
                    )
                } else if let Some((block, parameter)) = invocation_site
                    .operation()
                    .and_then(|operation| borrowed_block_argument_root(source, operation, argument))
                {
                    (
                        parameter.structural_type,
                        parameter.access,
                        TargetStructuralArgumentSource::BlockParameter {
                            block,
                            place: parameter.place,
                        },
                        parameter.clone(),
                        false,
                    )
                } else if let Some((block, parameter)) =
                    crate::selection::aggregate_result_input::block_home(source, argument.place)
                {
                    (
                        parameter.structural_type,
                        parameter.access,
                        TargetStructuralArgumentSource::BlockParameter {
                            block,
                            place: parameter.place,
                        },
                        parameter.clone(),
                        false,
                    )
                } else {
                    return Err(invalid!());
                };
            let semantic = terminal_psi::StructuralArgument {
                place: argument.place,
                path: argument.path.clone(),
                access: argument.access,
            };
            let presentation = (argument.access != terminal_psi::StructuralAccess::Owned).then(||
                crate::structural_inputs::structural_reference_input::borrowed_byte_view_presentation(
                    &root, &semantic, argument.structural_type, &signature.structural_types,
                )).flatten();
            let (projected_type, source_byte_offset, elements) = if let Some((offset, _, _)) =
                presentation
            {
                if presentation_debt {
                    return Err(invalid!());
                }
                (argument.structural_type, offset, Vec::new())
            } else if argument
                .path
                .contains(&terminal_psi::StructuralPathSegment::Referent)
            {
                let address = crate::selection::reference_input::borrowed_argument(
                    source, &semantic, argument, indices,
                )
                .ok_or_else(|| invalid!())?;
                if address.scalar_descriptor {
                    let scalar =
                        crate::structural_inputs::structural_reference_input::scalar_storage_type(
                            argument.structural_type,
                            &signature.structural_types,
                        )
                        .ok_or_else(|| invalid!())?;
                    crate::selection::reference_input::validate_scalar_foreign_export(
                        source,
                        backings,
                        &semantic,
                        scalar,
                        argument.shape.byte_size,
                    )?;
                }
                let projection =
                    crate::structural_inputs::structural_reference_input::reference_projection(
                        argument.root_structural_type,
                        &argument.path,
                        argument.access,
                        Some(argument.structural_type),
                        &signature.structural_types,
                    )
                    .ok_or_else(|| invalid!())?;
                (
                    argument.structural_type,
                    address.referent_offset,
                    projection.elements,
                )
            } else if argument.access != terminal_psi::StructuralAccess::Owned {
                crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
                    argument.root_structural_type,
                    &argument.path,
                    argument.access,
                    Some(argument.structural_type),
                    &signature.structural_types,
                )
                .ok_or_else(|| invalid!())?
            } else {
                crate::structural_inputs::structural_reference_input::runtime_projection(
                    argument.root_structural_type,
                    &argument.path,
                    &signature.structural_types,
                )
                .ok_or_else(|| invalid!())?
            };
            let projected_shape = crate::structural_inputs::structural_reference_input::shape(
                projected_type,
                &signature.structural_types,
            )
            .ok_or_else(|| invalid!())?;
            let (storage_shape, abi_shape) = match argument.access {
                terminal_psi::StructuralAccess::Owned => {
                    let admitted_root = if argument.path.is_empty() {
                        root_access == terminal_psi::StructuralAccess::Owned
                    } else {
                        matches!(
                            root_access,
                            terminal_psi::StructuralAccess::Owned
                                | terminal_psi::StructuralAccess::MutableBorrow
                        )
                    };
                    if !admitted_root {
                        return Err(invalid!());
                    }
                    let classified = calling_conventions::classified_boundary_shape(
                        projected_type,
                        &signature.structural_types,
                        plan.call.policy,
                    )
                    .map_err(|_| invalid!())?;
                    (projected_shape, classified)
                }
                terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow => {
                    let descriptor = signature.structural_types.iter().any(|declaration| {
                        declaration.id == projected_type
                            && declaration.shape.is_borrowed_view_byte_sequence()
                    });
                    if descriptor {
                        let classified = calling_conventions::classified_boundary_shape(
                            projected_type,
                            &signature.structural_types,
                            plan.call.policy,
                        )
                        .map_err(|_| invalid!())?;
                        (
                            ValueShape::borrowed_reference(
                                projected_shape.byte_size,
                                projected_shape.alignment,
                            ),
                            classified,
                        )
                    } else {
                        if !matches!(argument.destination.locations.as_slice(),
                        [ValueLocation::Register { value_byte_offset: 0, byte_size, .. }]
                        | [ValueLocation::Stack { value_byte_offset: 0, byte_size, .. }]
                            if *byte_size == u32::from(pointer_size))
                        {
                            return Err(invalid!());
                        }
                        (
                            ValueShape::borrowed_reference(
                                projected_shape.byte_size,
                                projected_shape.alignment,
                            ),
                            ValueShape::integer(u32::from(pointer_size), pointer_alignment),
                        )
                    }
                }
            };
            if (presentation.is_none()
                && argument.path.iter().any(|segment| match segment {
                    terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_) => false,
                    terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => false,
                    terminal_psi::StructuralPathSegment::Referent => {
                        argument.access == terminal_psi::StructuralAccess::Owned
                    }
                    _ => true,
                }))
                || argument.root_structural_type != root_type
                || argument.structural_type != projected_type
                || argument.source_byte_offset != source_byte_offset
                || argument.shape != storage_shape
                || argument.fixed_array_length != presentation.and_then(|(_, length, _)| length)
                || argument.element_stride != presentation.and_then(|(_, _, stride)| stride)
                || !crate::structural_inputs::runtime_indices_match(indices, &elements)
                || argument.indices.len() != indices.len()
                || argument
                    .indices
                    .iter()
                    .zip(indices)
                    .any(|(target, selected)| {
                        target.operand.source_value() != selected.operand.value
                            || target.operand.scalar_type() != selected.operand.scalar_type
                            || target.stride != selected.stride
                    })
                || argument.source != expected_source
                || operands.get(index) != Some(&argument.destination)
                || argument.destination.shape != abi_shape
            {
                return Err(invalid!());
            }
            structural_shapes.push((index, abi_shape));
        }
        let result_shape = match (&call.result, &plan.call.result, result) {
            (TargetBoundaryResult::Unit, None, None) => None,
            (TargetBoundaryResult::Scalar(home), Some(placement), Some(result)) => {
                if scalar_shape(result.scalar_type).is_none()
                    || Some(home.defining_operation) != invocation_site.operation()
                    || home.source_value != result.value
                    || home.scalar_type != result.scalar_type
                    || scalar_shape(result.scalar_type) != Some(home.shape)
                    || placement.shape != home.shape
                {
                    return Err(invalid!());
                }
                if !placement.is_complete_scalar_result() {
                    return Err(invalid!());
                }
                Some(home.shape)
            }
            (TargetBoundaryResult::Structural(home), Some(placement), None) => {
                let (producer, result) = home.operation_result().ok_or_else(|| invalid!())?;
                let signature = source.structural.as_ref().ok_or_else(|| invalid!())?;
                let storage = crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &signature.structural_types,
                )
                .ok_or_else(|| invalid!())?;
                let shape = calling_conventions::classified_boundary_shape(
                    result.structural_type,
                    &signature.structural_types,
                    plan.call.policy,
                )
                .map_err(|_| invalid!())?;
                if Some(producer) != invocation_site.operation()
                    || crate::selection::aggregate_result_input::home(source, result.place)
                        != Some((producer, result))
                    || home.layout
                        != target_operations::TargetStructuralHomeLayout::Aggregate(storage)
                    || placement.shape != shape
                {
                    return Err(invalid!());
                }
                Some(shape)
            }
            _ => return Err(invalid!()),
        };
        let mut parameters =
            vec![ValueShape::integer(u32::from(pointer_size), pointer_alignment); operands.len()];
        for argument in &call.scalar_arguments {
            *parameters
                .get_mut(argument.parameter_index as usize)
                .ok_or_else(|| invalid!())? =
                scalar_shape(argument.source.scalar_type()).ok_or_else(|| invalid!())?;
        }
        for (position, shape) in structural_shapes {
            *parameters.get_mut(position).ok_or_else(|| invalid!())? = shape;
        }
        let signature = CallSignature {
            parameters: parameters
                .into_iter()
                .skip(call.binding.dispatch_operand_count())
                .collect(),
            result: result_shape,
        };
        let validated = match call.callbacks.first() {
            Some(callback) => {
                calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                    plan.clone(),
                    &signature,
                    &callback.registrar_context,
                )
            }
            None => validate_boundary_entry_plan(plan.clone(), &signature),
        }
        .map_err(|_| invalid!())?;
        if validated.plan() != plan {
            return Err(invalid!());
        }
        if call_key(call, environment) != Some(key)
            || call_constraint(call, environment).as_ref() != Some(constraint)
        {
            return Err(invalid!());
        }
        Ok(())
    }
}
pub(crate) mod owned_argument {
    use crate::selection::place_declaration::{arrivals, declares_block_parameter};
    use crate::structural_inputs::structural_reference_input::runtime_projection;

    use super::{
        LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction, StructuralAccess,
        evaluate_call_plan,
    };
    use legalized_operations::LegalizedScalarInstructionKind;

    pub(crate) fn validate_owned_arguments(
        source: &LegalizedScalarFunction,
        call: &LegalizedScalarCall,
        site: semantic_vocabulary::InvocationSite,
    ) -> Option<()> {
        super::input_site::use_cut(source, site)?;
        for argument in &call.arguments {
            let LegalizedScalarArgument::Structural {
                semantic,
                target,
                indices,
                ..
            } = argument
            else {
                continue;
            };
            if semantic.access != StructuralAccess::Owned {
                continue;
            }
            let shape = super::owned_value_shape(source, target.structural_type)?;
            let types = &source.structural.as_ref()?.structural_types;
            let (selected, offset, elements) =
                runtime_projection(target.root_structural_type, &semantic.path, types)?;
            if semantic.access != StructuralAccess::Owned
                || target.place != semantic.place
                || target.access != semantic.access
                || target.path != semantic.path
                || selected != target.structural_type
                || target.shape != shape
                || target.destination.shape != shape
                || target.source_byte_offset != offset
                || !crate::structural_inputs::runtime_indices_match(indices, &elements)
                || target.indices.len() != indices.len()
                || target
                    .indices
                    .iter()
                    .zip(indices)
                    .any(|(target, selected)| {
                        target.operand.source_value() != selected.operand.value
                            || target.operand.scalar_type() != selected.operand.scalar_type
                            || target.stride != selected.stride
                    })
                || target.fixed_array_length.is_some()
                || target.element_stride.is_some()
                || !crate::selection::aggregate_result_input::owned_argument_placement(
                    &target.destination,
                )
            {
                return None;
            }
            match target.source {
                target_operations::TargetStructuralArgumentSource::StructuralHome {
                    psi_operation,
                } => {
                    let (_, _, producer) =
                        super::input_site::producer_before_use(source, psi_operation, site)?;
                    if let LegalizedScalarInstructionKind::EstablishTrivialAffineLocal {
                        place,
                        structural_type,
                    } = &producer.kind
                    {
                        if place.id != semantic.place
                            || structural_type.id != target.root_structural_type
                            || !semantic.path.is_empty()
                            || shape.byte_size != 0
                        {
                            return None;
                        }
                        continue;
                    }
                    let (actual_producer, result) =
                        crate::selection::aggregate_result_input::operation_result(
                            source,
                            semantic.place,
                        )?;
                    if actual_producer != psi_operation
                        || result.place != semantic.place
                        || result.structural_type != target.root_structural_type
                    {
                        return None;
                    }
                }
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block,
                    place,
                } => {
                    let mut parameters = arrivals(source, place);
                    let (owner, parameter) = parameters.next()?;
                    if parameters.next().is_some()
                        || block != owner
                        || block == source.entry_block
                        || place != semantic.place
                        || parameter.access != StructuralAccess::Owned
                        || parameter.structural_type != target.root_structural_type
                        || (!semantic.path.is_empty() && !parameter.qualifications.is_empty())
                        || !declares_block_parameter(
                            source.structural.as_ref()?,
                            place,
                            block,
                            parameter.position,
                        )
                    {
                        return None;
                    }
                }
                target_operations::TargetStructuralArgumentSource::Placement(ref placement) => {
                    let parameter = source
                        .structural
                        .as_ref()?
                        .parameters
                        .iter()
                        .find(|parameter| parameter.semantic.place == semantic.place)?;
                    let root_shape = super::owned_value_shape(source, target.root_structural_type)?;
                    if parameter.semantic.access != StructuralAccess::Owned
                        || parameter.semantic.structural_type != target.root_structural_type
                        || placement != &parameter.target.placement
                        || placement.shape != root_shape
                        || !crate::selection::aggregate_result_input::owned_argument_placement(
                            placement,
                        )
                    {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        let expected =
            evaluate_call_plan(source.call_plan.policy, &super::call_abi_signature(call)).ok()?;
        (expected == call.call_plan).then_some(())
    }
}
use borrowed_argument::validate_borrowed_argument;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction};
use register_environment::ValidatedTargetRegisterEnvironment;

use crate::structural_inputs::structural_reference_input::stack_pointer_offset;

pub(crate) fn owned_value_shape(
    source: &LegalizedScalarFunction,
    structural_type: semantic_vocabulary::StructuralTypeId,
) -> Option<ValueShape> {
    let types = &source.structural.as_ref()?.structural_types;
    owned_storage_shape(structural_type, types)
}

pub(crate) fn accepts_stack_parameter_entry(source: &LegalizedScalarFunction) -> bool {
    source.call_plan.result.is_none()
        || (source.structural.is_none()
            && source.parameters.len() == source.call_plan.parameters.len()
            && source
                .parameters
                .iter()
                .zip(&source.call_plan.parameters)
                .all(|(parameter, placement)| {
                    parameter.placement == *placement
                        && scalar_shape(parameter.scalar_type) == Some(placement.shape)
                }))
        || source.structural.as_ref().is_some_and(|signature| {
            let parameters = parameter_views(signature);
            accepts_graph(&source.call_plan, &parameters, &signature.structural_types)
        })
}

pub(crate) fn scalar_stack_placement(
    placement: &calling_conventions::ValuePlacement,
) -> Option<(u32, u16, u16)> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                alignment,
            },
        ] if matches!(
            placement.shape.class,
            calling_conventions::ValueClass::Float | calling_conventions::ValueClass::Integer
        ) && (matches!(*byte_size, 4 | 8)
            || placement.shape.class == calling_conventions::ValueClass::Integer
                && matches!(*byte_size, 1 | 2))
            && *byte_size == placement.shape.byte_size
            && *alignment >= placement.shape.alignment
            && alignment.is_power_of_two()
            && stack_byte_offset.is_multiple_of(u32::from(*alignment)) =>
        {
            Some((*stack_byte_offset, *byte_size as u16, *alignment))
        }
        _ => None,
    }
}

pub(crate) fn register_argument_count(call: &LegalizedScalarCall) -> usize {
    call.arguments
        .iter()
        .map(|argument| placement_register_count(argument.placement()))
        .sum()
}

pub(crate) fn register_argument_order(call: &LegalizedScalarCall) -> Vec<usize> {
    let mut order = call
        .arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| placement_register_count(argument.placement()) != 0)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    order.sort_by_key(|index| {
        call.arguments[*index].placement().shape.class == calling_conventions::ValueClass::Float
    });
    order
}

fn call_abi_signature(call: &LegalizedScalarCall) -> CallSignature {
    CallSignature {
        parameters: call
            .arguments
            .iter()
            .map(|argument| argument.placement().shape)
            .collect(),
        result: call
            .result_placement
            .as_ref()
            .map(|placement| placement.shape),
    }
}

fn is_register_location(location: &&ValueLocation) -> bool {
    matches!(
        location,
        ValueLocation::Register { .. }
            | ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(_),
                ..
            }
    )
}

fn placement_register_count(placement: &calling_conventions::ValuePlacement) -> usize {
    placement
        .locations
        .iter()
        .filter(is_register_location)
        .count()
}

fn placement_register(
    placement: &calling_conventions::ValuePlacement,
) -> Option<target_operations::MachineRegister> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                ..
            },
        ]
        | [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: None,
                ..
            },
        ] => Some(*register),
        _ => None,
    }
}

pub(crate) fn unit_key(
    call: &LegalizedScalarCall,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Option<RegisterConstraintKey> {
    invocation_key(call, None, environment)
}

pub(crate) fn no_normal_key(
    call: &LegalizedScalarCall,
    declared_result: &abstract_operations::AbstractFunctionResult,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Option<RegisterConstraintKey> {
    invocation_key(call, Some(declared_result), environment)
}

fn invocation_key(
    call: &LegalizedScalarCall,
    declared_no_normal_result: Option<&abstract_operations::AbstractFunctionResult>,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Option<RegisterConstraintKey> {
    let order = register_argument_order(call);
    let mut views = Vec::new();
    for index in &order {
        let placement = call.arguments[*index].placement();
        if matches!(&call.arguments[*index], LegalizedScalarArgument::Structural { semantic, .. } if semantic.access == StructuralAccess::Owned)
        {
            for location in &placement.locations {
                let Some(register) = location.direct_register() else {
                    continue;
                };
                views.push(environment.fixed_register_view(register)?);
            }
        } else {
            views.push(environment.fixed_register_view(placement_register(placement)?)?);
        }
    }
    let mut inputs = views.len();
    let hidden = call.result_placement.as_ref().and_then(|placement| {
        crate::selection::aggregate_result_input::indirect_result(placement, call.call_plan.policy)
    });
    if let Some(hidden) = hidden {
        views.push(environment.fixed_register_view(hidden)?);
        inputs += 1;
    } else if let Some(result) = &call.result_placement {
        for location in &result.locations {
            let ValueLocation::Register { register, .. } = location else {
                return None;
            };
            views.push(environment.fixed_register_view(*register)?);
        }
    }
    let keys = environment.selected_keys();
    let structural_result =
        declared_no_normal_result.map_or(call.structural_result.is_some(), |result| {
            matches!(
                result,
                abstract_operations::AbstractFunctionResult::Structural(_)
            )
        });
    let empty_result = structural_result
        && call
            .result_placement
            .as_ref()
            .is_some_and(empty_aggregate_placement)
        && call.result_placement == call.call_plan.result;
    let candidate_keys = if structural_result && !empty_result {
        keys.call_aggregate.iter().collect::<Vec<_>>()
    } else if call.result_placement.is_some() && !empty_result {
        keys.call_scalar.iter().collect::<Vec<_>>()
    } else {
        keys.call_unit.iter().chain(&keys.call_unit_mixed).collect()
    };
    let mut matches =
        candidate_keys.into_iter().filter(|key| {
            environment.constraint(**key).is_some_and(|row| {
                row.operands.len() == views.len()
                    && row.operands.iter().zip(&views).enumerate().all(
                        |(position, (operand, view))| {
                            operand.access
                                == if position < inputs {
                                    RegisterOperandAccess::Use
                                } else {
                                    RegisterOperandAccess::Def
                                }
                                && operand.fixed_view == Some(*view)
                        },
                    )
            })
        });
    let result = *matches.next()?;
    (matches.next().is_none()
        && (declared_no_normal_result.is_none() || keys.invoke_no_normal.contains(&result)))
    .then_some(result)
}

pub(crate) fn empty_aggregate_placement(placement: &calling_conventions::ValuePlacement) -> bool {
    placement.shape == calling_conventions::ValueShape::integer(0, 1)
        && placement.locations.is_empty()
}

pub(crate) fn validate(
    function: usize,
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    owner: target_operations::CallSiteOwner,
    key: RegisterConstraintKey,
    row: &RegisterInstructionConstraint,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::unsupported_shape(function);
    let installation_role = if let target_operations::CallSiteOwner::Installation {
        operation,
        role,
    } = owner
    {
        if input_site::installation_call(source, operation, role) != Some(call) {
            return Err(SelectedInstructionError::custody_because(
                "installation call differs from its exact source operation and helper role",
            ));
        }
        let result_shape = match role {
            target_operations::InstallationCallRole::Validate => scalar_shape(ScalarType::Boolean),
            target_operations::InstallationCallRole::Work => None,
        };
        if call
            .result_placement
            .as_ref()
            .map(|placement| placement.shape)
            != result_shape
            || call.structural_result.is_some()
            || !call.descriptor_origins.is_empty()
        {
            return Err(SelectedInstructionError::custody_because(
                "installation validator must return Boolean and work must have no receiving result",
            ));
        }
        Some(role)
    } else {
        None
    };
    let no_normal = matches!(owner, target_operations::CallSiteOwner::InvokeNoNormal(_))
        || installation_role == Some(target_operations::InstallationCallRole::Work);
    if no_normal {
        call.validate_no_normal_shape()
    } else {
        call.validate_shape()
    }
    .map_err(|_| invalid())?;
    // Legalization already stored this call's calling-convention plan. Selection
    // checks the roles and arguments it is about to encode; it does not compute
    // the plan again.
    if call
        .arguments
        .iter()
        .any(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. }))
    {
        let operation = match owner {
            target_operations::CallSiteOwner::Operation(operation)
            | target_operations::CallSiteOwner::Installation { operation, .. } => {
                semantic_vocabulary::InvocationSite::Operation(operation)
            }
            target_operations::CallSiteOwner::InvokeNoNormal(edge) => {
                semantic_vocabulary::InvocationSite::InvokeNoNormal(edge)
            }
            target_operations::CallSiteOwner::ExternalCompletion(edge) => {
                semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
            }
            _ => return Err(invalid()),
        };
        if source.call_plan.policy != CallingPolicy::native_for_target(environment.target()) {
            return Err(invalid());
        }
        if call.arguments.iter().any(|argument| matches!(argument, LegalizedScalarArgument::Structural { semantic, .. } if semantic.access == StructuralAccess::Owned)) {
            owned_argument::validate_owned_arguments(source, call, operation).ok_or_else(invalid)?;
        }
        for (argument_index, argument) in call.arguments.iter().enumerate() {
            if matches!(argument, LegalizedScalarArgument::Structural { semantic, .. } if semantic.access != StructuralAccess::Owned)
            {
                validate_borrowed_argument(source, call, operation, argument_index)
                    .ok_or_else(|| {
                        SelectedInstructionError::custody_because(
                            "borrowed call argument does not match its retained source geometry and ABI",
                        )
                    })?;
            }
        }
    }
    if no_normal {
        let declared_result = match owner {
            target_operations::CallSiteOwner::Installation {
                role: target_operations::InstallationCallRole::Work,
                ..
            } => &abstract_operations::AbstractFunctionResult::Unit,
            target_operations::CallSiteOwner::InvokeNoNormal(edge) => input_site::no_normal_result(
                source,
                semantic_vocabulary::InvocationSite::InvokeNoNormal(edge),
                call,
            )
            .ok_or_else(|| {
                SelectedInstructionError::custody_because(
                    "no-normal call has no exact retained edge and declared result",
                )
            })?,
            _ => return Err(invalid()),
        };
        let expected = no_normal_key(call, declared_result, environment).ok_or_else(|| {
            SelectedInstructionError::custody_because(
                "no-normal declared ABI has no unique canonical constraint in its result family",
            )
        })?;
        if expected != key
            || row.key != key
            || environment.constraint(key) != Some(row)
            || call.structural_result.is_some()
            || !call.descriptor_origins.is_empty()
        {
            return Err(invalid());
        }
        return Ok(());
    }
    if call.result_placement.as_ref().is_some_and(|placement| {
        crate::selection::aggregate_result_input::indirect_result(placement, call.call_plan.policy)
            .is_some()
    }) {
        crate::selection::aggregate_result_input::call_result(source, call).ok_or_else(invalid)?;
        if unit_key(call, environment) != Some(key)
            || environment.constraint(key) != Some(row)
            || row.key != key
        {
            return Err(invalid());
        }
        return Ok(());
    }
    let count = call.arguments.len();
    let register_count = register_argument_count(call);
    let result = call.call_plan.result.as_ref();
    let aggregate =
        if call.structural_result.is_some() {
            Some(
            crate::selection::aggregate_result_input::call_result(source, call)
                .ok_or_else(|| SelectedInstructionError::custody_because(
                    "structural call result does not match its declared result home and ABI",
                ))?
                .1,
        )
        } else {
            None
        };
    if unit_key(call, environment) != Some(key)
        || environment.constraint(key) != Some(row)
        || row.key != key
        || call.call_plan.parameters.len() != count
        || row.operands.len()
            != register_count + result.map_or(0, |placement| placement.locations.len())
    {
        return Err(invalid());
    }
    if result != call.result_placement.as_ref() {
        return Err(invalid());
    }
    let order = register_argument_order(call);
    for (index, placement) in call.call_plan.parameters.iter().chain(result).enumerate() {
        if index == count && placement.locations.is_empty() {
            continue;
        }
        if matches!(call.arguments.get(index), Some(LegalizedScalarArgument::Structural { semantic, .. }) if semantic.access == StructuralAccess::Owned)
        {
            let start = order
                .iter()
                .take_while(|argument| **argument != index)
                .map(|argument| placement_register_count(call.arguments[*argument].placement()))
                .sum::<usize>();
            if call.arguments[index].placement() != placement {
                return Err(invalid());
            }
            for (fragment, location) in placement
                .locations
                .iter()
                .filter(is_register_location)
                .enumerate()
            {
                let register = location.direct_register().ok_or_else(invalid)?;
                let operand = row.operands.get(start + fragment).ok_or_else(invalid)?;
                if operand.access != RegisterOperandAccess::Use
                    || operand.fixed_view.is_none()
                    || operand.fixed_view != environment.fixed_register_view(register)
                {
                    return Err(invalid());
                }
            }
            continue;
        }
        if index == count && aggregate.is_some() {
            for (fragment, location) in placement.locations.iter().enumerate() {
                let ValueLocation::Register { register, .. } = location else {
                    return Err(invalid());
                };
                let operand = row
                    .operands
                    .get(register_count + fragment)
                    .ok_or_else(invalid)?;
                if operand.access != RegisterOperandAccess::Def
                    || operand.fixed_view.is_none()
                    || operand.fixed_view != environment.fixed_register_view(*register)
                {
                    return Err(invalid());
                }
            }
            continue;
        }
        if scalar_stack_placement(placement).is_some() {
            if !matches!(call.arguments.get(index), Some(LegalizedScalarArgument::Scalar { source: value, placement: actual })
                if actual == placement && scalar_value_shape(source, *value) == Some(placement.shape))
            {
                return Err(invalid());
            }
            continue;
        }
        if stack_pointer_offset(placement).is_some() {
            if !matches!(call.arguments.get(index), Some(LegalizedScalarArgument::Structural { target, .. }) if target.destination == *placement)
            {
                return Err(invalid());
            }
            continue;
        }
        let operand_index = if index == count {
            register_count
        } else {
            if !order.contains(&index) {
                return Err(invalid());
            }
            order
                .iter()
                .take_while(|argument| **argument != index)
                .map(|argument| placement_register_count(call.arguments[*argument].placement()))
                .sum()
        };
        let operand = row.operands.get(operand_index).ok_or_else(invalid)?;
        let register = match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] if matches!(
                placement.shape.class,
                calling_conventions::ValueClass::BorrowedReference
                    | calling_conventions::ValueClass::BorrowedScalarReference
            ) && matches!(
                call.arguments.get(index),
                Some(LegalizedScalarArgument::Structural { .. })
            ) =>
            {
                register
            }
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == placement.shape.byte_size && matches!(*byte_size, 1 | 2 | 4 | 8) => {
                register
            }
            [
                ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(register),
                    copy_stack_byte_offset: None,
                    byte_size,
                    alignment,
                },
            ] if matches!(
                placement.shape.class,
                calling_conventions::ValueClass::BorrowedReference
                    | calling_conventions::ValueClass::BorrowedScalarReference
            ) && *byte_size == placement.shape.byte_size
                && *alignment == placement.shape.alignment
                && matches!(
                    call.arguments.get(index),
                    Some(LegalizedScalarArgument::Structural { .. })
                ) =>
            {
                register
            }
            _ => return Err(invalid()),
        };
        if operand.fixed_view.is_none()
            || operand.fixed_view != environment.fixed_register_view(*register)
            || operand.access
                != if index == count {
                    RegisterOperandAccess::Def
                } else {
                    RegisterOperandAccess::Use
                }
        {
            return Err(invalid());
        }
        if let Some(argument) = call.arguments.get(index) {
            if argument.placement() != placement {
                return Err(invalid());
            }
            if let LegalizedScalarArgument::Scalar { source: value, .. } = argument
                && scalar_value_shape(source, *value) != Some(placement.shape)
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

pub(crate) fn scalar_shape(scalar_type: ScalarType) -> Option<ValueShape> {
    ValueShape::for_scalar(scalar_type)
}

pub(crate) fn integer_carrier_normalization(scalar_type: ScalarType) -> SelectedInstructionKind {
    match scalar_type {
        ScalarType::Boolean => SelectedInstructionKind::ZeroExtendU8,
        ScalarType::Integer(integer) => match (integer.sign(), integer.bits()) {
            (IntegerSign::Unsigned, 8) => SelectedInstructionKind::ZeroExtendU8,
            (IntegerSign::Unsigned, 16) => SelectedInstructionKind::ZeroExtendU16,
            (IntegerSign::Unsigned, 32) => SelectedInstructionKind::ZeroExtendU32,
            (IntegerSign::Signed, 8) => SelectedInstructionKind::SignExtendI8,
            (IntegerSign::Signed, 16) => SelectedInstructionKind::SignExtendI16,
            (IntegerSign::Signed, 32) => SelectedInstructionKind::SignExtendI32,
            _ => SelectedInstructionKind::CopyI64,
        },
        _ => SelectedInstructionKind::CopyI64,
    }
}

pub(crate) fn incoming_float_transfer(
    scalar_type: ScalarType,
    keys: &SelectedConstraintKeys,
) -> Option<(SelectedInstructionKind, RegisterConstraintKey)> {
    match scalar_type {
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some((
            SelectedInstructionKind::Float32ToBits,
            keys.float32_to_bits?,
        )),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some((
            SelectedInstructionKind::Float64ToBits,
            keys.float64_to_bits?,
        )),
        _ => None,
    }
}

pub(crate) fn outgoing_float_transfer(
    scalar_type: ScalarType,
    keys: &SelectedConstraintKeys,
) -> Option<(SelectedInstructionKind, RegisterConstraintKey)> {
    match scalar_type {
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some((
            SelectedInstructionKind::BitsToFloat32,
            keys.bits_to_float32?,
        )),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some((
            SelectedInstructionKind::BitsToFloat64,
            keys.bits_to_float64?,
        )),
        _ => None,
    }
}

pub(crate) fn load_transfer(
    keys: &SelectedConstraintKeys,
    byte_size: u32,
    byte_offset: u32,
) -> Option<(SelectedInstructionKind, Option<RegisterConstraintKey>)> {
    Some(match byte_size {
        1 => (SelectedInstructionKind::Load8 { byte_offset }, keys.load8),
        2 => (SelectedInstructionKind::Load16 { byte_offset }, keys.load16),
        4 => (SelectedInstructionKind::Load32 { byte_offset }, keys.load32),
        8 => (SelectedInstructionKind::Load64 { byte_offset }, keys.load64),
        _ => return None,
    })
}

fn scalar_value_shape(source: &LegalizedScalarFunction, value: ValueId) -> Option<ValueShape> {
    scalar_value_type(source, value).and_then(scalar_shape)
}

fn scalar_value_type(source: &LegalizedScalarFunction, value: ValueId) -> Option<ScalarType> {
    source
        .parameters
        .iter()
        .find(|parameter| parameter.value == value)
        .map(|parameter| parameter.scalar_type)
        .or_else(|| {
            source
                .block_parameters()
                .find(|parameter| parameter.value == value)
                .map(|parameter| parameter.scalar_type)
        })
        .or_else(|| {
            source
                .instructions()
                .filter_map(|instruction| instruction.result)
                .find(|result| result.value == value)
                .map(|result| result.scalar_type)
        })
}
