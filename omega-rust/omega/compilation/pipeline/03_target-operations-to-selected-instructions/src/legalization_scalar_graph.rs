/*
We validate scalar-graph input custody for target legalization and auditing,
checking call origins, signatures, aggregate results, borrowed windows, byte
and element views, normalized foreign calls, and structural call/case/field
operations while delegating reference custody and target-row matching to
`legalization_reference_custody` and `legalization_target`.
*/

#[allow(unused_imports)]
pub(crate) use crate::legalization::LegalizationError;
#[allow(unused_imports)]
pub(crate) use crate::legalization::{accepted_fact, references_obligation};
#[allow(unused_imports)]
pub(crate) use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractStructuralCallResult, IntegerShiftPolicy,
};
#[allow(unused_imports)]
pub(crate) use calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape,
    evaluate_call_plan,
};
#[allow(unused_imports)]
pub(crate) use legalized_operations::{SaturatingCarrier, TrappingForm, TrappingOperation};
#[allow(unused_imports)]
pub(crate) use optimization_unit::{
    OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    ValueDefinitionSite,
};
#[allow(unused_imports)]
pub(crate) use semantic_vocabulary::{
    IntegerSign, IntegerType, MachineId, ObligationId, OperationId, ScalarType, ValueId,
};
#[allow(unused_imports)]
pub(crate) use target_operations::{TargetFunction, TargetOperationPlan, TargetUnitOperation};
pub(crate) mod address_joins {
    use super::{AbstractOperation, AbstractOperationPlan, CallPlan, PsiOptimizationFunction};
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::target::control_flow::sources::dominates;
    use crate::structural_inputs::structural_reference_input::{
        fixed_byte_array_view, fixed_element_array_view, internal_borrow_shape, parameter_shape,
        runtime_borrowed_projection,
    };
    use abstract_operations::control_flow::address_joins::is_address_join_in;
    use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralPlaceKind};
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

    pub(crate) fn parameter<'a>(
        function: &'a PsiOptimizationFunction,
        place: PlaceId,
        plan: &AbstractOperationPlan,
    ) -> Option<(BlockId, &'a StructuralParameterDeclaration)> {
        parameter_from_rows(
            function.entry,
            place,
            plan,
            super::structural_case::arrivals(function, place),
            function.structural_places.iter(),
        )
    }

    pub(crate) fn parameter_from_rows<'function>(
        entry: BlockId,
        place: PlaceId,
        plan: &AbstractOperationPlan,
        mut matching: impl Iterator<Item = (BlockId, &'function StructuralParameterDeclaration)>,
        mut declarations: impl Iterator<Item = &'function terminal_psi::StructuralPlaceDeclaration>,
    ) -> Option<(BlockId, &'function StructuralParameterDeclaration)> {
        let (block, parameter) = matching.next()?;
        (matching.next().is_none()
            && block != entry
            && is_address_join_in(parameter, &plan.structural_types)
            && declarations.any(|declaration| {
                declaration.id == place
                    && declaration.kind
                        == (StructuralPlaceKind::BlockParameter {
                            block,
                            position: parameter.position,
                        })
            }))
        .then_some((block, parameter))
    }

    pub(crate) fn call_argument(
        semantic: &terminal_psi::StructuralArgument,
        destination: &StructuralParameterDeclaration,
        call_operation: semantic_vocabulary::InvocationSite,
        caller: &PsiOptimizationFunction,
        call: &CallPlan,
        parameter_ordinal: usize,
        native: &target_operations::TargetOperationPlan,
        plan: &AbstractOperationPlan,
    ) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
        let (block, parameter) =
            parameter(caller, semantic.place, plan).ok_or(LegalizationError::custody())?;
        let call_block = super::invocations::position(caller, call_operation, None)
            .ok_or(LegalizationError::custody())?
            .0;
        if let Some(window) = window_argument(
            semantic,
            destination,
            block,
            parameter,
            call,
            parameter_ordinal,
            plan,
        ) {
            return if dominates(caller, block, call_block) {
                window
            } else {
                Err(LegalizationError::custody())
            };
        }
        let (selected, offset, elements) = runtime_borrowed_projection(
            parameter.structural_type,
            &semantic.path,
            semantic.access,
            Some(destination.structural_type),
            &plan.structural_types,
        )
        .ok_or(LegalizationError::custody())?;
        let shape = internal_borrow_shape(selected, &plan.structural_types)
            .ok_or(LegalizationError::custody())?;
        let placement = call
            .parameters
            .get(parameter_ordinal)
            .ok_or(LegalizationError::custody())?;
        if !loan_access_permitted(parameter.access, semantic.access)
            || destination.access != semantic.access
            || destination.multiplicity != StructuralMultiplicity::Unrestricted
            || destination.structural_type != selected
            || !dominates(caller, block, call_block)
            || placement.shape != shape
        {
            return Err(LegalizationError::custody());
        }
        Ok(target_operations::TargetStructuralArgument {
            place: semantic.place,
            access: semantic.access,
            path: semantic.path.clone(),
            root_structural_type: parameter.structural_type,
            structural_type: selected,
            shape,
            source_byte_offset: offset,
            fixed_array_length: None,
            element_stride: None,
            indices: super::target::control_flow::sources::call_indices(
                native,
                caller,
                target_operations::CallSiteOwner::from(call_operation),
                &elements,
            )?,
            source: target_operations::TargetStructuralArgumentSource::BlockParameter {
                block,
                place: parameter.place,
            },
            destination: placement.clone(),
        })
    }

    fn window_argument(
        semantic: &terminal_psi::StructuralArgument,
        destination: &StructuralParameterDeclaration,
        block: BlockId,
        parameter: &StructuralParameterDeclaration,
        call: &CallPlan,
        parameter_ordinal: usize,
        plan: &AbstractOperationPlan,
    ) -> Option<Result<target_operations::TargetStructuralArgument, LegalizationError>> {
        let types = &plan.structural_types;
        let (source_byte_offset, length, stride) =
            fixed_byte_array_view(parameter, semantic, destination.structural_type, types)
                .map(|(offset, length)| (offset, length, 1))
                .or_else(|| {
                    fixed_element_array_view(
                        parameter,
                        semantic,
                        destination.structural_type,
                        types,
                    )
                })?;
        let Some(shape) = parameter_shape(destination, types) else {
            return Some(Err(LegalizationError::custody()));
        };
        let Some(placement) = call.parameters.get(parameter_ordinal) else {
            return Some(Err(LegalizationError::custody()));
        };
        if destination.access != semantic.access
            || destination.multiplicity != StructuralMultiplicity::Unrestricted
            || placement.shape != shape
        {
            return Some(Err(LegalizationError::custody()));
        }
        Some(Ok(target_operations::TargetStructuralArgument {
            place: semantic.place,
            access: semantic.access,
            path: semantic.path.clone(),
            root_structural_type: parameter.structural_type,
            structural_type: destination.structural_type,
            shape,
            source_byte_offset,
            fixed_array_length: Some(length),
            element_stride: Some(stride),
            indices: Vec::new(),
            source: target_operations::TargetStructuralArgumentSource::BlockParameter {
                block,
                place: parameter.place,
            },
            destination: placement.clone(),
        }))
    }

    pub(crate) fn edge_bindings(
        function: &PsiOptimizationFunction,
        source_block: BlockId,
        target: BlockId,
        bindings: &[abstract_operations::AbstractStructuralBinding],
        plan: &AbstractOperationPlan,
    ) -> bool {
        let Some(destination) = function.block(target) else {
            return false;
        };
        bindings.len() == destination.structural_parameters.len()
            && bindings
                .iter()
                .zip(&destination.structural_parameters)
                .all(|(binding, declaration)| {
                    if !is_address_join_in(declaration, &plan.structural_types) {
                        return true;
                    }
                    binding.parameter == declaration.place
                        && binding.argument.access == declaration.access
                        && optimization_unit_semantics::descriptor_argument_accessible(
                            function,
                            &binding.argument,
                            &plan.structural_types,
                            declaration.access,
                        )
                        && abstract_operations::control_flow::address_joins::is_address_projection(
                            &binding.argument.path,
                        )
                        && root_type(
                            function,
                            source_block,
                            binding.argument.place,
                            &binding.argument.path,
                            declaration.access,
                            plan,
                        )
                        .and_then(|root| {
                            abstract_operations::control_flow::address_joins::borrowed_referent_type(
                                plan.structural_types.iter(),
                                root,
                                &binding.argument.path,
                                binding.argument.access,
                            )
                        })
                        .is_some_and(|referent| referent == declaration.structural_type)
                })
    }

    fn loan_access_permitted(source: StructuralAccess, destination: StructuralAccess) -> bool {
        use StructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
        matches!(
            (source, destination),
            (
                MutableBorrow,
                SharedBorrow | MutableBorrow | WriteOnlyBorrow
            ) | (Owned | SharedBorrow, SharedBorrow)
                | (WriteOnlyBorrow, WriteOnlyBorrow)
        )
    }

    fn root_type(
        function: &PsiOptimizationFunction,
        block: BlockId,
        place: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        access: StructuralAccess,
        plan: &AbstractOperationPlan,
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        let permitted = |source: &StructuralParameterDeclaration| {
            abstract_operations::control_flow::address_joins::loan_access_permitted(
                plan.structural_types.iter(),
                source.structural_type,
                path,
                source.access,
                access,
            )
        };
        if let Some(source) = function.structural_parameter(place) {
            return permitted(source).then_some(source.structural_type);
        }
        if let Some((owner, joined)) = parameter(function, place, plan) {
            return ((owner == block || dominates(function, owner, block)) && permitted(joined))
                .then_some(joined.structural_type);
        }
        let (operation, result) = if let Some((operation, result, _)) =
            super::primitive_locals::producer(function, place)
        {
            (operation, result.clone())
        } else if let Some((operation, result)) =
            function
                .block_nodes()
                .find_map(|node| match &node.operation {
                    AbstractOperation::EstablishElementView {
                        psi_operation,
                        result,
                        ..
                    }
                    | AbstractOperation::ElementViewSubslice {
                        psi_operation,
                        result,
                        ..
                    } if result.place == place => Some((*psi_operation, result.clone())),
                    _ => None,
                })
        {
            (operation, result)
        } else {
            match super::structural_case::source_owner(function, place).ok()? {
                legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                    operation,
                    result,
                } => (operation, result),
                legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                    block: owner,
                    declaration,
                } => {
                    return ((owner == block || dominates(function, owner, block))
                        && permitted(&declaration))
                    .then_some(declaration.structural_type);
                }
                legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
                | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                    ..
                } => {
                    return None;
                }
            }
        };
        let owner = operation_block(function, operation)?;
        (owner == block || dominates(function, owner, block)).then_some(result.structural_type)
    }

    fn operation_block(
        function: &PsiOptimizationFunction,
        operation: OperationId,
    ) -> Option<BlockId> {
        function.blocks.iter().find_map(|block| {
            block
                .nodes
                .iter()
                .any(|node| node.operation.psi_operation() == Some(operation))
                .then_some(block.id)
        })
    }
}
pub(crate) mod aggregate_results {
    use super::{
        AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
        CallPlan, CallSignature, CallingPolicy, PsiOptimizationFunction, TargetFunction,
        TargetOperationPlan, ValueDefinitionSite, evaluate_call_plan, scalar_shape,
    };
    use crate::LegalizationError;
    use crate::structural_inputs::structural_reference_input::{
        field_shape, owned_storage_shape, primitive_array_shape,
    };
    use abstract_operations::AbstractStructuralCallResult;
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
    use terminal_psi::{StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape};

    pub(crate) mod borrowed_arguments {
        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input::target;
        use crate::structural_inputs::structural_reference_input::{
            internal_borrow_shape, runtime_borrowed_projection,
        };

        use super::{
            AbstractOperationPlan, CallPlan, PsiOptimizationFunction, StructuralMultiplicity,
            TargetOperationPlan,
        };
        use crate::legalization::scalar_graph_input::aggregate_results::home_layout;
        use crate::legalization::scalar_graph_input::structural_case;
        use crate::legalization::scalar_graph_input::structural_parameters;
        pub(crate) fn reconstruct(
            argument: &terminal_psi::StructuralArgument,
            position: usize,
            call_operation: semantic_vocabulary::InvocationSite,
            caller: &PsiOptimizationFunction,
            callee: abstract_operations::AbstractCallSignature<'_>,
            call: &CallPlan,
            native: &TargetOperationPlan,
            plan: &AbstractOperationPlan,
        ) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
            reconstruct_at_owner(
                argument,
                position,
                target_operations::CallSiteOwner::from(call_operation),
                caller,
                callee,
                call,
                native,
                plan,
                || {
                    crate::legalization::scalar_graph_input::address_joins::parameter(
                        caller,
                        argument.place,
                        plan,
                    )
                },
            )
        }

        pub(crate) fn reconstruct_at_owner<'caller>(
            argument: &terminal_psi::StructuralArgument,
            position: usize,
            owner: target_operations::CallSiteOwner,
            caller: &'caller PsiOptimizationFunction,
            callee: abstract_operations::AbstractCallSignature<'_>,
            call: &CallPlan,
            native: &TargetOperationPlan,
            plan: &AbstractOperationPlan,
            address_join: impl FnOnce() -> Option<(
                semantic_vocabulary::BlockId,
                &'caller terminal_psi::StructuralParameterDeclaration,
            )>,
        ) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
            if matches!(owner, target_operations::CallSiteOwner::Installation { .. }) {
                return Err(LegalizationError::custody());
            }
            let declaration = crate::legalization::scalar_graph_input::call_signature::parameter(
                callee,
                position,
                argument.place,
            )?;
            let destination = &declaration;
            if argument.access != destination.access
                || !matches!(
                    argument.access,
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                )
                || destination.multiplicity != StructuralMultiplicity::Unrestricted
            {
                return Err(LegalizationError::custody());
            }
            if address_join().is_some() {
                return crate::legalization::scalar_graph_input::address_joins::call_argument(
                    argument,
                    destination,
                    match owner {
                        target_operations::CallSiteOwner::Operation(operation) => {
                            semantic_vocabulary::InvocationSite::Operation(operation)
                        }
                        target_operations::CallSiteOwner::InvokeNoNormal(edge) => {
                            semantic_vocabulary::InvocationSite::InvokeNoNormal(edge)
                        }
                        _ => return Err(LegalizationError::custody()),
                    },
                    caller,
                    call,
                    callee.scalar_parameter_count() + position,
                    native,
                    plan,
                );
            }
            let (root, source) = if let Some(parameter) =
                caller.structural_parameter(argument.place)
            {
                if !matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::Owned
                        | terminal_psi::StructuralAccess::MutableBorrow
                ) && parameter.access != argument.access
                {
                    return Err(LegalizationError::custody());
                }
                let retained = native
                    .function(caller.machine)
                    .and_then(structural_parameters)
                    .and_then(|parameters| {
                        parameters
                            .iter()
                            .find(|value| value.place == argument.place)
                    })
                    .ok_or(LegalizationError::custody())?;
                (parameter.structural_type, retained.placement.clone().into())
            } else {
                let site = |operation| {
                    super::node_site(caller, |candidate| {
                        candidate.psi_operation() == Some(operation)
                    })
                };
                let after = if let Some(operation) = owner.operation() {
                    site(operation)
                } else {
                    caller.blocks.iter().find_map(|block| {
                        block
                            .nodes
                            .iter()
                            .position(|node| {
                                node.operation
                                    .cleanup_rosters()
                                    .any(|(edge, _)| Some(edge) == owner.edge())
                            })
                            .map(|position| (block.id, position))
                    })
                }
                .ok_or(LegalizationError::custody())?;
                if let Some((block, declaration)) =
                    super::borrowed_block_arrival(caller, argument.place, plan)
                {
                    if !target::control_flow::sources::dominates(caller, block, after.0)
                        || !(declaration.access == argument.access
                            || declaration.access == terminal_psi::StructuralAccess::MutableBorrow)
                    {
                        return Err(LegalizationError::custody());
                    }
                    (
                        declaration.structural_type,
                        target_operations::TargetStructuralArgumentSource::BlockParameter {
                            block,
                            place: declaration.place,
                        },
                    )
                } else {
                    match structural_case::source_owner(caller, argument.place)? {
                        legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                            operation,
                            result,
                        } => {
                            home_layout(&result, plan)?;
                            let before = site(operation).ok_or(LegalizationError::custody())?;
                            if (before.0 == after.0 && before.1 >= after.1)
                                || (before.0 != after.0
                                    && !target::control_flow::sources::dominates(caller, before.0, after.0))
                            {
                                return Err(LegalizationError::custody());
                            }
                            (
                                result.structural_type,
                                target_operations::TargetStructuralArgumentSource::StructuralHome {
                                    psi_operation: operation,
                                },
                            )
                        }
                        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                            block,
                            declaration,
                        } => {
                            if !target::control_flow::sources::dominates(caller, block, after.0)
                                || !structural_case::block_parameter_declared(caller, block, &declaration)
                            {
                                return Err(LegalizationError::custody());
                            }
                            (
                                declaration.structural_type,
                                target_operations::TargetStructuralArgumentSource::BlockParameter {
                                    block,
                                    place: declaration.place,
                                },
                            )
                        }
                        legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
                        | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                            ..
                        } => {
                            return Err(LegalizationError::custody());
                        }
                    }
                }
            };
            let (selected, offset, elements) = runtime_borrowed_projection(
                root,
                &argument.path,
                argument.access,
                Some(destination.structural_type),
                &plan.structural_types,
            )
            .ok_or(LegalizationError::custody())?;
            let indices = if elements.is_empty() {
                Vec::new()
            } else {
                target::control_flow::sources::call_indices(native, caller, owner, &elements)?
            };
            let shape = internal_borrow_shape(selected, &plan.structural_types)
                .ok_or(LegalizationError::custody())?;
            let placement = call
                .parameters
                .get(callee.scalar_parameter_count() + position)
                .ok_or(LegalizationError::custody())?;
            if !terminal_semantics::borrowed_referent_matches(
                plan.structural_types.iter(),
                selected,
                destination.structural_type,
            ) || placement.shape != shape
            {
                return Err(LegalizationError::custody());
            }
            Ok(target_operations::TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
                root_structural_type: root,
                structural_type: destination.structural_type,
                shape,
                source_byte_offset: offset,
                fixed_array_length: None,
                element_stride: None,
                indices,
                source,
                destination: placement.clone(),
            })
        }

        pub(crate) fn view_arrival<'caller>(
            caller: &'caller PsiOptimizationFunction,
            place: semantic_vocabulary::PlaceId,
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Option<(
            semantic_vocabulary::BlockId,
            &'caller terminal_psi::StructuralParameterDeclaration,
        )> {
            let mut arrivals = structural_case::arrivals(caller, place);
            let (block, declaration) = arrivals.next()?;
            let shape =
                terminal_psi::find_structural_type(structural_types, declaration.structural_type)
                    .map(|row| &row.shape)?;
            (arrivals.next().is_none()
                && block != caller.entry
                && structural_case::block_parameter_declared(caller, block, declaration)
                && terminal_semantics::view_descriptor_parameter(
                    declaration,
                    structural_types.iter(),
                )
                && (declaration.qualifications.is_empty()
                    || declaration.access == terminal_psi::StructuralAccess::SharedBorrow
                        && shape.is_borrowed_byte_view()))
            .then_some((block, declaration))
        }
    }
    pub(crate) mod owned_arguments {
        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input::target;
        use crate::structural_inputs::structural_reference_input::{
            RuntimeElement, owned_storage_shape, runtime_projection,
        };

        use super::{
            AbstractOperation, AbstractOperationPlan, CallPlan, PsiOptimizationFunction,
            StructuralMultiplicity, StructuralPlaceKind, TargetOperationPlan,
        };
        use crate::legalization::scalar_graph_input::structural_case;
        use crate::legalization::scalar_graph_input::structural_parameters;

        pub(crate) fn reconstruct(
            argument: &terminal_psi::StructuralArgument,
            position: usize,
            call_operation: semantic_vocabulary::InvocationSite,
            caller: &PsiOptimizationFunction,
            callee: abstract_operations::AbstractCallSignature<'_>,
            call: &CallPlan,
            native: &TargetOperationPlan,
            plan: &AbstractOperationPlan,
        ) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
            let declaration = crate::legalization::scalar_graph_input::call_signature::parameter(
                callee,
                position,
                argument.place,
            )?;
            let destination = &declaration;
            if destination.access != terminal_psi::StructuralAccess::Owned {
                return Err(LegalizationError::custody());
            }
            let shape =
                crate::structural_inputs::parameter_shape(destination, &plan.structural_types)
                    .ok_or(LegalizationError::custody())?;
            let value_shape =
                owned_storage_shape(destination.structural_type, &plan.structural_types)
                    .ok_or(LegalizationError::custody())?;
            if shape != value_shape {
                return Err(LegalizationError::custody());
            }
            let mut block_parameters = caller.blocks.iter().flat_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .filter(move |parameter| parameter.place == argument.place)
                    .map(move |parameter| (block.id, parameter))
            });
            let block_parameter = block_parameters.next();
            if block_parameters.next().is_some() {
                return Err(LegalizationError::custody());
            }
            let (source, root_structural_type, projection) = if let Some(parameter) =
                caller.structural_parameter(argument.place)
            {
                if parameter.access != destination.access
                    || (argument.path.is_empty()
                        && parameter.multiplicity != destination.multiplicity)
                {
                    return Err(LegalizationError::custody());
                }
                let retained = native
                    .function(caller.machine)
                    .and_then(structural_parameters)
                    .and_then(|parameters| {
                        parameters
                            .iter()
                            .find(|retained| retained.place == argument.place)
                    })
                    .ok_or(LegalizationError::custody())?;
                let root_shape =
                    owned_storage_shape(parameter.structural_type, &plan.structural_types)
                        .ok_or(LegalizationError::custody())?;
                if retained.structural_type != parameter.structural_type
                    || retained.access != parameter.access
                    || retained.multiplicity != parameter.multiplicity
                    || retained.shape != root_shape
                    || retained.placement.shape != root_shape
                    || retained.projected_qualifications != parameter.projected_qualifications
                {
                    return Err(LegalizationError::custody());
                }
                (
                    retained.placement.clone().into(),
                    parameter.structural_type,
                    owned_projection(
                        parameter.structural_type,
                        destination.structural_type,
                        argument,
                        plan,
                    )?,
                )
            } else if let Some((block, parameter)) = block_parameter {
                let call_site = call_site(caller, call_operation, argument)?;
                if block == caller.entry
                    || parameter.access != destination.access
                    || argument.access != parameter.access
                    || (argument.path.is_empty()
                        && parameter.multiplicity != destination.multiplicity)
                    || !target::control_flow::sources::dominates(caller, block, call_site.0)
                    || !structural_case::block_parameter_declared(caller, block, parameter)
                {
                    return Err(LegalizationError::custody());
                }
                let offset = owned_projection(
                    parameter.structural_type,
                    destination.structural_type,
                    argument,
                    plan,
                )?;
                (
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place: parameter.place,
                    },
                    parameter.structural_type,
                    offset,
                )
            } else if let Some((producer, structural_type, producer_site)) =
                trivial_affine_local(caller, argument.place)
            {
                if !argument.path.is_empty()
                    || structural_type != destination.structural_type
                    || destination.multiplicity != StructuralMultiplicity::Affine
                    || shape.byte_size != 0
                    || !precedes(caller, producer_site, call_operation, argument)?
                {
                    return Err(LegalizationError::custody());
                }
                (
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: producer,
                    },
                    structural_type,
                    (0, Vec::new()),
                )
            } else {
                let (producer, result) = structural_case::source_result(caller, argument.place)?;
                if argument.path.is_empty() && result.multiplicity != destination.multiplicity {
                    return Err(LegalizationError::custody());
                }
                let offset = owned_projection(
                    result.structural_type,
                    destination.structural_type,
                    argument,
                    plan,
                )?;
                let producer_site = node_site(caller, |operation| {
                    operation
                        .structural_result()
                        .is_some_and(|(actual_producer, actual)| {
                            actual_producer == producer && actual == result
                        })
                })
                .ok_or(LegalizationError::custody())?;
                if !precedes(caller, producer_site, call_operation, argument)?
                    || !structural_case::records_operation_result(caller, producer, result)
                {
                    return Err(LegalizationError::custody());
                }
                (
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: producer,
                    },
                    result.structural_type,
                    offset,
                )
            };
            let (source_byte_offset, elements) = projection;
            let indices = target::control_flow::sources::call_indices(
                native,
                caller,
                target_operations::CallSiteOwner::from(call_operation),
                &elements,
            )?;
            let placement = call
                .parameters
                .get(callee.scalar_parameter_count() + position)
                .ok_or(LegalizationError::custody())?;
            if placement.shape != shape {
                return Err(LegalizationError::custody());
            }
            Ok(target_operations::TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
                root_structural_type,
                structural_type: destination.structural_type,
                shape,
                source_byte_offset,
                fixed_array_length: None,
                element_stride: None,
                indices,
                source,
                destination: placement.clone(),
            })
        }

        fn owned_projection(
            root: semantic_vocabulary::StructuralTypeId,
            destination: semantic_vocabulary::StructuralTypeId,
            argument: &terminal_psi::StructuralArgument,
            plan: &AbstractOperationPlan,
        ) -> Result<(u32, Vec<RuntimeElement>), LegalizationError> {
            if argument.path.is_empty() {
                return (root == destination)
                    .then_some((0, Vec::new()))
                    .ok_or(LegalizationError::custody());
            }
            let (selected, offset, indices) =
                runtime_projection(root, &argument.path, &plan.structural_types)
                    .ok_or(LegalizationError::custody())?;
            if selected != destination {
                return Err(LegalizationError::custody());
            }
            Ok((offset, indices))
        }

        pub(crate) fn node_site(
            caller: &PsiOptimizationFunction,
            matches: impl Fn(&AbstractOperation) -> bool,
        ) -> Option<(semantic_vocabulary::BlockId, usize)> {
            caller.blocks.iter().find_map(|block| {
                block
                    .nodes
                    .iter()
                    .position(|node| matches(&node.operation))
                    .map(|position| (block.id, position))
            })
        }

        pub(crate) fn precedes(
            caller: &PsiOptimizationFunction,
            producer_site: (semantic_vocabulary::BlockId, usize),
            call_operation: semantic_vocabulary::InvocationSite,
            argument: &terminal_psi::StructuralArgument,
        ) -> Result<bool, LegalizationError> {
            let call_site = call_site(caller, call_operation, argument)?;
            Ok(if producer_site.0 == call_site.0 {
                producer_site.1 < call_site.1
            } else {
                target::control_flow::sources::dominates(caller, producer_site.0, call_site.0)
            })
        }

        fn call_site(
            caller: &PsiOptimizationFunction,
            call_operation: semantic_vocabulary::InvocationSite,
            argument: &terminal_psi::StructuralArgument,
        ) -> Result<(semantic_vocabulary::BlockId, usize), LegalizationError> {
            super::super::invocations::position(caller, call_operation, Some(argument))
                .ok_or(LegalizationError::custody())
        }

        pub(crate) fn trivial_affine_local(
            caller: &PsiOptimizationFunction,
            place: semantic_vocabulary::PlaceId,
        ) -> Option<(
            semantic_vocabulary::OperationId,
            semantic_vocabulary::StructuralTypeId,
            (semantic_vocabulary::BlockId, usize),
        )> {
            let mut establishments = caller.blocks.iter().flat_map(|block| {
                block
                    .nodes
                    .iter()
                    .enumerate()
                    .filter_map(move |(position, node)| {
                        trivial_affine_candidate(block.id, position, &node.operation, place)
                    })
            });
            let establishment = establishments.next()?;
            establishments.next().is_none().then_some(establishment)
        }

        pub(crate) fn trivial_affine_candidate(
            block: semantic_vocabulary::BlockId,
            position: usize,
            operation: &AbstractOperation,
            place: semantic_vocabulary::PlaceId,
        ) -> Option<(
            semantic_vocabulary::OperationId,
            semantic_vocabulary::StructuralTypeId,
            (semantic_vocabulary::BlockId, usize),
        )> {
            match operation {
                AbstractOperation::EstablishTrivialAffineLocal {
                    psi_operation,
                    place: declaration,
                    structural_type,
                } if declaration.id == place
                    && matches!(declaration.kind, StructuralPlaceKind::TrivialAffineLocal { structural_type: declared, construction: None, .. } if declared == structural_type.id)
                    && matches!(&structural_type.shape, terminal_psi::StructuralTypeShape::Record { fields, .. } if fields.is_empty()) =>
                {
                    Some((*psi_operation, structural_type.id, (block, position)))
                }
                _ => None,
            }
        }
    }
    pub(crate) use owned_arguments::{node_site, precedes};

    pub(crate) fn uses(function: &PsiOptimizationFunction) -> bool {
        function.result.structural().is_some()
            || function
                .structural_parameters
                .iter()
                .chain(function.block_structural_parameters())
                .any(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned)
            || function.block_nodes().any(|node| {
                matches!(
                    node.operation,
                    AbstractOperation::StructuralCaseMembership { .. }
                        | abstract_operations::byte_len_or_field_read_pattern!(..)
                        | AbstractOperation::StructuralScalarFieldRead { .. }
                        | AbstractOperation::EstablishStructuralArray { .. }
                        | AbstractOperation::EstablishScalarArray { .. }
                        | AbstractOperation::EstablishByteSequenceConcatenation { .. }
                        | AbstractOperation::EstablishActivationClaim { .. }
                        | AbstractOperation::ReleaseActivationClaim { .. }
                        | AbstractOperation::EstablishNativeCodeLayout { .. }
                        | AbstractOperation::EstablishRecord { .. }
                        | AbstractOperation::EstablishReference { .. }
                        | AbstractOperation::EstablishAccessReference { .. }
                        | AbstractOperation::ReleaseReference { .. }
                        | AbstractOperation::EstablishCase { .. }
                        | AbstractOperation::StructuralLeafCopy { .. }
                        | AbstractOperation::MoveStructuralValue { .. }
                        | AbstractOperation::TransformContent { .. }
                        | abstract_operations::move_or_store_structural_field_pattern!()
                        | AbstractOperation::EstablishTrivialAffineLocal { .. }
                        | AbstractOperation::EstablishZeroedStructural { .. }
                        | AbstractOperation::EstablishStructuralLocal { .. }
                        | AbstractOperation::CallStructural {
                            result: AbstractStructuralCallResult::Structural { .. },
                            ..
                        }
                        | AbstractOperation::BoundaryCall {
                            result: abstract_operations::AbstractBoundaryResult::Structural(_),
                            ..
                        }
                ) || matches!(&node.operation,
                        AbstractOperation::CallStructural {
                            structural_arguments,
                            result:
                                AbstractStructuralCallResult::Unit
                                | AbstractStructuralCallResult::Scalar(_),
                            ..
                        }
                            if structural_arguments.iter().any(|argument|
                                    argument.access == terminal_psi::StructuralAccess::Owned))
                    || matches!(&node.operation,
                        AbstractOperation::StructuralCase { source, .. }
                            if function.structural_parameter(*source).is_some())
            })
    }

    pub(crate) fn sum_layout(
        result: &StructuralOperationResult,
        plan: &AbstractOperationPlan,
    ) -> Result<calling_conventions::ConventionalSumLayout, LegalizationError> {
        sum_type_layout(result.structural_type, plan)
    }

    pub(crate) fn sum_type_layout(
        structural_type: semantic_vocabulary::StructuralTypeId,
        plan: &AbstractOperationPlan,
    ) -> Result<calling_conventions::ConventionalSumLayout, LegalizationError> {
        let mut declarations = plan
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == structural_type);
        let declaration = declarations.next().ok_or(LegalizationError::custody())?;
        let (common, cases) = match &declaration.shape {
            StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases.as_slice()),
            _ => return Err(LegalizationError::custody()),
        };
        if declarations.next().is_some() {
            return Err(LegalizationError::custody());
        }
        let shape_of = |field: &terminal_psi::StructuralFieldDeclaration| {
            field_shape(&field.field_type, &plan.structural_types, &mut Vec::new())
                .ok_or(LegalizationError::custody())
        };
        let common = common
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .map(shape_of)
            .collect::<Result<Vec<_>, _>>()?;
        let payloads = cases
            .iter()
            .map(|case| {
                case.fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .map(shape_of)
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        calling_conventions::evaluate_conventional_sum_layout(&common, &payloads)
            .map_err(|_| LegalizationError::custody())
    }

    pub(crate) fn cleanup(
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        actions: &[terminal_psi::TerminalAffineCleanupAction],
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> bool {
        let owner = |place: semantic_vocabulary::PlaceId| {
            if let Some((_, structural_type, _)) = function_inputs.trivial_affine_local(place) {
                return Some((structural_type, StructuralMultiplicity::Affine));
            }
            if let Some(parameter) = function_inputs.arrival_parameter(place) {
                return (parameter.access == terminal_psi::StructuralAccess::Owned
                    || (parameter.is_unqualified_root()
                        && terminal_semantics::view_descriptor_parameter(
                            parameter,
                            structural_types.iter(),
                        )))
                .then_some((parameter.structural_type, parameter.multiplicity));
            }
            let (_, result) = function_inputs.source_result(place).ok()?;
            Some((result.structural_type, result.multiplicity))
        };
        let discardable = |place| {
            owner(place)
                .is_some_and(|(_, multiplicity)| multiplicity == StructuralMultiplicity::Affine)
        };
        actions.iter().all(|action| {
            let path = action.path();
            let selected_type = |root| {
                terminal_semantics::runtime_structural_path_tip(structural_types.iter(), root, path)
                    .map(|declaration| declaration.id)
            };
            match action {
                terminal_psi::TerminalAffineCleanupAction::RetireHome(place) => {
                    owner(*place).is_some()
                }
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    discardable(*place)
                }
                terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                    owner(discard.place).and_then(|(root, _)| selected_type(root))
                        == Some(discard.structural_type)
                }
                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    owner(cleanup.place).is_some_and(|(structural_type, multiplicity)| {
                        let whole_claim_contract = match multiplicity {
                            StructuralMultiplicity::Affine => cleanup.consumed_claims.is_empty(),
                            StructuralMultiplicity::Linear => !cleanup.consumed_claims.is_empty(),
                            StructuralMultiplicity::Unrestricted => false,
                        };
                        selected_type(structural_type) == Some(cleanup.structural_type)
                            && (!cleanup.path.is_empty() || whole_claim_contract)
                    }) && cleanup
                        .consumed_claims
                        .windows(2)
                        .all(|pair| pair[0] < pair[1])
                        && cleanup.requirement_obligations.is_empty()
                }
            }
        })
    }

    pub(crate) fn header(
        target: &TargetFunction,
        abstracted: &AbstractFunction,
        optimized: &PsiOptimizationFunction,
        native: ::target::NativeTarget,
        plan: &AbstractOperationPlan,
    ) -> Result<CallPlan, LegalizationError> {
        let graph = &target.graph;
        if target.machine != abstracted.machine
            || target.machine != optimized.machine
            || (target.attachment != abstracted.attachment
                || target.nominal_cleanup_subject != abstracted.nominal_cleanup_subject)
            || (target.attachment != optimized.attachment
                || target.nominal_cleanup_subject != optimized.nominal_cleanup_subject)
            || (match abstracted.result {
                AbstractFunctionResult::Unit => target
                    .scalar_abi
                    .as_ref()
                    .is_some_and(|abi| abi.result.is_some()),
                AbstractFunctionResult::Structural(_) => target.scalar_abi.is_some(),
                AbstractFunctionResult::Scalar(_) => false,
            })
            || (!matches!(abstracted.result, AbstractFunctionResult::Scalar(_))
                && target.mixed_structural_scalar_abi.is_some())
            || optimized.result != abstracted.result
            || !super::place_declarations::matches_producers(optimized)
            || optimized.structural_parameters != abstracted.structural_parameters
            || abstracted.entry_claims != optimized.entry_claim_declarations
            || abstracted.entry_claims.len() != optimized.entry_claims.len()
            || abstracted
                .entry_claims
                .iter()
                .any(|claim| !optimized.entry_claims.contains(&claim.claim))
            || abstracted.published_service_ceiling != optimized.published_service_ceiling
            || abstracted.parameters.len() != optimized.parameters.len()
            || graph.scalar_parameters.len() != abstracted.parameters.len()
            || graph.parameters.len() != abstracted.structural_parameters.len()
        {
            return Err(LegalizationError::custody());
        }
        let result = match &abstracted.result {
            AbstractFunctionResult::Unit => None,
            AbstractFunctionResult::Scalar(result) => {
                Some(scalar_shape(result.scalar_type).ok_or(LegalizationError::custody())?)
            }
            AbstractFunctionResult::Structural(result) => {
                Some(type_home_layout(result.structural_type, plan)?.shape())
            }
        };
        let mut shapes = abstracted
            .parameters
            .iter()
            .map(|parameter| {
                scalar_shape(parameter.scalar_type).ok_or(LegalizationError::custody())
            })
            .collect::<Result<Vec<_>, _>>()?;
        for parameter in &abstracted.structural_parameters {
            shapes.push(
                crate::structural_inputs::parameter_shape(parameter, &plan.structural_types)
                    .ok_or(LegalizationError::custody())?,
            );
        }
        let expected = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: shapes,
                result,
            },
        )
        .map_err(|_| LegalizationError::custody())?;
        if graph.call_plan != expected
            || target.scalar_abi.as_ref().is_some_and(|abi| {
                !graph.parameters.is_empty()
                    || abi.call_plan != expected
                    || abi.parameters != graph.scalar_parameters
            })
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractFunctionResult::Scalar(result) = abstracted.result {
            let retained = match (&target.scalar_abi, &target.mixed_structural_scalar_abi) {
                (Some(abi), None) => Some(abi.result.as_ref().ok_or(LegalizationError::custody())?),
                (None, Some(abi))
                    if !graph.parameters.is_empty()
                        && abi.call_plan == expected
                        && abi.scalar_parameters == graph.scalar_parameters
                        && abi.structural_parameters == graph.parameters =>
                {
                    Some(&abi.result)
                }
                (None, None) => None,
                _ => return Err(LegalizationError::custody()),
            };
            if retained.is_some_and(|retained| {
                retained.value != result.value
                    || retained.scalar_type != result.scalar_type
                    || Some(&retained.placement) != expected.result.as_ref()
            }) {
                return Err(LegalizationError::custody());
            }
        }
        for (position, ((declared, actual), retained)) in abstracted
            .parameters
            .iter()
            .zip(&optimized.parameters)
            .zip(&graph.scalar_parameters)
            .enumerate()
        {
            if actual.value != declared.value
                || actual.scalar_type != declared.scalar_type
                || actual.site != ValueDefinitionSite::FunctionParameter(position as u32)
                || retained.value != declared.value
                || retained.scalar_type != declared.scalar_type
                || retained.placement != expected.parameters[position]
            {
                return Err(LegalizationError::custody());
            }
        }
        for (position, (declared, retained)) in abstracted
            .structural_parameters
            .iter()
            .zip(&graph.parameters)
            .enumerate()
        {
            if declared.position as usize != position
                || retained.place != declared.place
                || retained.structural_type != declared.structural_type
                || retained.access != declared.access
                || retained.multiplicity != declared.multiplicity
                || retained.projected_qualifications != declared.projected_qualifications
                || retained.shape
                    != expected.parameters[abstracted.parameters.len() + position].shape
                || retained.placement != expected.parameters[abstracted.parameters.len() + position]
            {
                return Err(LegalizationError::custody());
            }
        }
        if optimized.block_structural_parameters().any(|parameter| {
            !borrowed_view_parameter(parameter, plan)
                && super::address_joins::parameter(optimized, parameter.place, plan).is_none()
                && block_home_layout(parameter, plan).is_err()
        }) {
            return Err(LegalizationError::custody());
        }
        Ok(expected)
    }

    pub(crate) fn block_home_layout(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        plan: &AbstractOperationPlan,
    ) -> Result<target_operations::TargetStructuralHomeLayout, LegalizationError> {
        if parameter.access != terminal_psi::StructuralAccess::Owned {
            return Err(LegalizationError::custody());
        }
        home_layout(
            &StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                qualifications: parameter.qualifications.clone(),
                projected_qualifications: parameter.projected_qualifications.clone(),
                additional_input_qualifications: parameter.additional_input_qualifications.to_vec(),
                claims: Vec::new(),
            },
            plan,
        )
    }

    fn borrowed_view_parameter(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        plan: &AbstractOperationPlan,
    ) -> bool {
        terminal_semantics::view_descriptor_parameter(parameter, plan.structural_types.iter())
            || abstract_operations::control_flow::owner_loans::is_shared_view_carrier_loan_in(
                parameter,
                &plan.structural_types,
            )
    }

    pub(crate) fn call_argument(
        argument: &terminal_psi::StructuralArgument,
        position: usize,
        call_operation: semantic_vocabulary::OperationId,
        caller: &PsiOptimizationFunction,
        callee: abstract_operations::AbstractCallSignature<'_>,
        call: &CallPlan,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
        custody: &super::reference_custody::Custody,
    ) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
        call_argument_invocation(
            argument,
            position,
            call_operation.into(),
            caller,
            callee,
            call,
            native,
            plan,
            custody,
        )
    }

    pub(crate) fn call_argument_invocation(
        argument: &terminal_psi::StructuralArgument,
        position: usize,
        call_operation: semantic_vocabulary::InvocationSite,
        caller: &PsiOptimizationFunction,
        callee: abstract_operations::AbstractCallSignature<'_>,
        call: &CallPlan,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
        custody: &super::reference_custody::Custody,
    ) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
        let declaration = super::call_signature::parameter(callee, position, argument.place)?;
        let destination = &declaration;
        if argument
            .path
            .iter()
            .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
        {
            let ordinal = callee
                .scalar_parameter_count()
                .checked_add(position)
                .ok_or(LegalizationError::custody())?;
            return super::reference_custody::referent_argument(
                argument,
                destination,
                call.parameters
                    .get(ordinal)
                    .ok_or(LegalizationError::custody())?,
                caller,
                native,
                call_operation,
                custody,
                &plan.structural_types,
            );
        }
        if argument.access != terminal_psi::StructuralAccess::Owned
            && owned_storage_shape(destination.structural_type, &plan.structural_types).is_some()
        {
            return borrowed_arguments::reconstruct(
                argument,
                position,
                call_operation,
                caller,
                callee,
                call,
                native,
                plan,
            );
        }
        if argument.access == terminal_psi::StructuralAccess::Owned {
            return owned_arguments::reconstruct(
                argument,
                position,
                call_operation,
                caller,
                callee,
                call,
                native,
                plan,
            );
        }
        if super::primitive_locals::scalar(&plan.structural_types, destination.structural_type)
            .is_some()
        {
            if !argument.path.is_empty() {
                return borrowed_arguments::reconstruct(
                    argument,
                    position,
                    call_operation,
                    caller,
                    callee,
                    call,
                    native,
                    plan,
                );
            }
            return super::structural_call::primitive_argument(
                argument,
                call_operation,
                caller,
                destination,
                call,
                callee
                    .scalar_parameter_count()
                    .checked_add(position)
                    .ok_or(LegalizationError::custody())?,
                native,
                plan,
            );
        }
        if super::address_joins::parameter(caller, argument.place, plan).is_some() {
            return super::address_joins::call_argument(
                argument,
                destination,
                call_operation,
                caller,
                call,
                callee
                    .scalar_parameter_count()
                    .checked_add(position)
                    .ok_or(LegalizationError::custody())?,
                native,
                plan,
            );
        }
        if !argument.path.is_empty()
            && caller
                .blocks
                .iter()
                .any(|block| block.structural_parameter(argument.place).is_some())
        {
            return Err(LegalizationError::custody());
        }
        super::structural_call::argument_at_invocation(
            argument,
            position,
            call_operation,
            caller,
            callee,
            call,
            native,
            plan,
            custody,
        )
    }

    pub(crate) fn borrowed_block_arrival<'function>(
        function: &'function PsiOptimizationFunction,
        place: PlaceId,
        plan: &AbstractOperationPlan,
    ) -> Option<(
        semantic_vocabulary::BlockId,
        &'function terminal_psi::StructuralParameterDeclaration,
    )> {
        super::address_joins::parameter(function, place, plan)
            .or_else(|| borrowed_arguments::view_arrival(function, place, &plan.structural_types))
    }

    pub(crate) fn result_home(
        function: &PsiOptimizationFunction,
        place: PlaceId,
        plan: &AbstractOperationPlan,
    ) -> Result<target_operations::TargetStructuralHomeRequirement, LegalizationError> {
        let source = super::structural_case::source_owner(function, place).or_else(|_| {
            let mut matching = super::structural_case::arrivals(function, place);
            let (block, declaration) = matching.next().ok_or(LegalizationError::custody())?;
            if matching.next().is_some()
                || !abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter_in(
                    declaration,
                    &plan.structural_types,
                )
            {
                return Err(LegalizationError::custody());
            }
            Ok(
                legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                    block,
                    declaration: declaration.clone(),
                },
            )
        })?;
        let (origin, layout) = match source {
            legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                operation,
                result,
            } => {
                let layout = home_layout(&result, plan)?;
                (
                    target_operations::TargetStructuralHomeOrigin::OperationResult {
                        operation,
                        result,
                    },
                    layout,
                )
            }
            legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                block,
                declaration,
            } => {
                let layout = block_home_layout(&declaration, plan)?;
                (
                    target_operations::TargetStructuralHomeOrigin::BlockParameter {
                        block,
                        declaration,
                    },
                    layout,
                )
            }
            legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
            | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter { .. } => {
                return Err(LegalizationError::custody());
            }
        };
        Ok(target_operations::TargetStructuralHomeRequirement { origin, layout })
    }

    pub(crate) fn home_layout(
        result: &StructuralOperationResult,
        plan: &AbstractOperationPlan,
    ) -> Result<target_operations::TargetStructuralHomeLayout, LegalizationError> {
        let layout = type_home_layout(result.structural_type, plan)?;
        if matches!(
            layout,
            target_operations::TargetStructuralHomeLayout::Sum(_)
        ) {
            sum_layout(result, plan)?;
        }
        Ok(layout)
    }

    pub(crate) fn type_home_layout(
        structural_type: semantic_vocabulary::StructuralTypeId,
        plan: &AbstractOperationPlan,
    ) -> Result<target_operations::TargetStructuralHomeLayout, LegalizationError> {
        let declaration = plan
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .ok_or(LegalizationError::custody())?;
        if matches!(
            declaration.shape,
            terminal_psi::sum_or_mixed_shape_pattern!()
        ) {
            return Ok(target_operations::TargetStructuralHomeLayout::Sum(
                sum_type_layout(structural_type, plan)?,
            ));
        }
        Ok(target_operations::TargetStructuralHomeLayout::Aggregate(
            primitive_array_shape(structural_type, &plan.structural_types)
                .or_else(|| {
                    crate::structural_inputs::shape(structural_type, &plan.structural_types)
                })
                .ok_or(LegalizationError::custody())?,
        ))
    }
}
pub(crate) mod atomic_access {
    use crate::structural_inputs::structural_reference_input::atomic_cell_geometry;
    use abstract_operations::{AbstractAtomicEvent, AbstractResult};
    use semantic_vocabulary::IntegerType;
    use terminal_psi::{
        AtomicAccessEvent, StructuralAccess, StructuralArgument, StructuralTypeDeclaration,
    };

    pub(crate) struct Access {
        pub location: StructuralArgument,
        pub scalar_type: IntegerType,
        pub field_byte_offset: u32,
        pub event: AtomicAccessEvent,
        pub result: Option<AbstractResult>,
        pub outcome: Option<(
            terminal_psi::StructuralOperationResult,
            calling_conventions::ConventionalSumLayout,
        )>,
    }

    pub(crate) fn event(
        source: &AbstractAtomicEvent,
    ) -> Option<(AtomicAccessEvent, Option<AbstractResult>)> {
        use AbstractAtomicEvent as Source;
        Some(match source {
            Source::Load {
                ordering, result, ..
            } => (
                AtomicAccessEvent::Load {
                    ordering: *ordering,
                },
                Some(*result),
            ),
            Source::Store {
                ordering, value, ..
            } => (
                AtomicAccessEvent::Store {
                    ordering: *ordering,
                    value: *value,
                },
                None,
            ),
            Source::ReadModifyWrite {
                ordering,
                operation,
                operand,
                prior,
                ..
            } => (
                AtomicAccessEvent::ReadModifyWrite {
                    ordering: *ordering,
                    operation: (*operation).into(),
                    operand: *operand,
                },
                Some(*prior),
            ),
            Source::Swap {
                ordering,
                value,
                prior,
                ..
            } => (
                AtomicAccessEvent::Swap {
                    ordering: *ordering,
                    value: *value,
                },
                Some(*prior),
            ),
            Source::CompareExchange {
                success,
                failure,
                expected,
                replacement,
                observed,
                ..
            } => (
                AtomicAccessEvent::CompareExchange {
                    success: *success,
                    failure: *failure,
                    expected: *expected,
                    replacement: *replacement,
                },
                Some(*observed),
            ),
            Source::CompareExchangeOnce {
                custody,
                success,
                failure,
                expected,
                replacement,
                ..
            } => (
                AtomicAccessEvent::CompareExchangeOnce {
                    custody: *custody,
                    success: *success,
                    failure: *failure,
                    expected: *expected,
                    replacement: *replacement,
                },
                None,
            ),
            Source::Fence { .. } => return None,
        })
    }

    pub(crate) fn access(
        source: &AbstractAtomicEvent,
        function: &optimization_unit::PsiOptimizationFunction,
        types: &[StructuralTypeDeclaration],
    ) -> Option<Access> {
        let location = source.location()?;
        let (event, result) = event(source)?;
        let root = function
            .arrival_parameter(location.root)
            .map(|parameter| parameter.structural_type)
            .or_else(|| {
                super::structural_case::source_result(function, location.root)
                    .ok()
                    .map(|(_, result)| result.structural_type)
            })?;
        let root_access =
            optimization_unit_semantics::structural_place_access(function, location.root)?;
        if !event.ordering_is_legal()
            || event.observes_resident() && root_access == StructuralAccess::WriteOnlyBorrow
        {
            return None;
        }
        let cell = terminal_semantics::atomic_cell_type(types.iter(), root, &location.path)?;
        let scalar_type = cell.scalar_type();
        let integer =
            target_operations::operations::atomic_storage_integer_type(event, scalar_type)?;
        super::scalar_shape(scalar_type)?;
        if event
            .operands()
            .iter()
            .any(|value| super::value_type(function, *value) != Some(scalar_type))
            || result.is_some_and(|result| result.scalar_type != scalar_type)
        {
            return None;
        }
        let geometry = atomic_cell_geometry(root, &location.path, scalar_type, types)?;
        let offset = geometry.byte_offset;
        let byte_size = geometry.byte_size;
        if byte_size != u32::from(integer.bits() / 8) || offset % byte_size != 0 {
            return None;
        }
        let outcome = if let AbstractAtomicEvent::CompareExchangeOnce {
            outcome, custody, ..
        } = source
        {
            if !source.custody_is_consistent()
                || !terminal_psi::types::atomic_once_result_is_plain(outcome, *custody)
            {
                return None;
            }
            let declaration = types.iter().find(|row| row.id == outcome.structural_type)?;
            Some((
                outcome.clone(),
                target_operations::conventional_atomic_once_layout(declaration, integer, *custody)?,
            ))
        } else {
            None
        };
        Some(Access {
            location: StructuralArgument {
                place: location.root,
                access: root_access,
                path: location.path.clone(),
            },
            scalar_type: integer,
            field_byte_offset: offset,
            event,
            result,
            outcome,
        })
    }
}
pub(crate) mod borrowed_windows {
    use super::{AbstractOperationPlan, PsiOptimizationFunction};
    use crate::LegalizationError;
    use crate::structural_inputs::structural_reference_input::{self, RuntimeElement};
    use abstract_operations::control_flow::address_joins::is_address_join_in;
    use calling_conventions::ValueShape;
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{StructuralAccess, StructuralOperationResult, StructuralPathSegment};

    pub(crate) struct WindowExtent {
        pub(crate) path: Vec<StructuralPathSegment>,
        pub(crate) field_type: StructuralTypeId,
        pub(crate) byte_offset: u32,
        pub(crate) shape: ValueShape,
        pub(crate) elements: Vec<RuntimeElement>,
    }

    pub(crate) fn extent(
        function: &PsiOptimizationFunction,
        root: &abstract_operations::StructuralWindowRoot,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
        plan: &AbstractOperationPlan,
        custody: &super::reference_custody::Custody,
        required_access: StructuralAccess,
    ) -> Result<WindowExtent, LegalizationError> {
        let referent = if path.contains(&StructuralPathSegment::Referent) {
            let referent = super::reference_custody::window_destination(
                function,
                custody,
                &plan.structural_types,
                root.place(),
                path,
                required_access,
            )
            .ok_or(LegalizationError::custody())?;
            if referent.carrier_type != root.structural_type() {
                return Err(LegalizationError::custody());
            }
            Some(referent)
        } else {
            None
        };
        let root_type = match root {
            abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
                if parameter.access != StructuralAccess::Owned
                    || !function.blocks.iter().any(|candidate| {
                        candidate.id == *block
                            && candidate.structural_parameters.contains(parameter)
                    })
                {
                    return Err(LegalizationError::custody());
                }
                parameter.structural_type
            }
            abstract_operations::StructuralWindowRoot::Parameter(root) => {
                if !required_access.admits(root.access)
                    || !(function.structural_parameters.contains(root)
                        || (function
                            .blocks
                            .iter()
                            .any(|block| block.structural_parameters.contains(root))
                            && (is_address_join_in(root, &plan.structural_types)
                                || (matches!(
                                    root.access,
                                    terminal_psi::exclusive_borrow_pair_pattern!()
                                ) && terminal_semantics::view_descriptor_parameter(
                                    root,
                                    plan.structural_types.iter(),
                                )))))
                    || (root.access == StructuralAccess::MutableBorrow
                        && function
                            .entry_claim_declarations
                            .iter()
                            .any(|claim| claim.input == root.place))
                {
                    return Err(LegalizationError::custody());
                }
                root.structural_type
            }
            abstract_operations::StructuralWindowRoot::OwnedResult { result, .. } => {
                result.structural_type
            }
        };
        let (root_type, path) = referent.as_ref().map_or((root_type, path), |referent| {
            (referent.referent, referent.path.as_slice())
        });
        let types = &plan.structural_types;
        if !terminal_semantics::structural_window_selects_child(path, field) {
            let authenticated = if let Some(referent) = &referent {
                super::reference_custody::window_backing_roots(
                    function,
                    custody,
                    types,
                    root.place(),
                    &{
                        let mut full = referent.carrier_path.clone();
                        full.push(StructuralPathSegment::Referent);
                        full.extend_from_slice(path);
                        full
                    },
                    required_access,
                )
                .is_ok_and(|roots| {
                    terminal_semantics::structural_window_origins(
                        function.structural_parameters.iter().chain(
                            function
                                .blocks
                                .iter()
                                .flat_map(|block| &block.structural_parameters),
                        ),
                        roots,
                        required_access,
                        |place| {
                            optimization_unit_semantics::structural_place_supports_window_restoration(
                                function, place,
                            )
                        },
                    )
                })
            } else {
                path.is_empty()
                    && (matches!(
                        root,
                        abstract_operations::StructuralWindowRoot::Parameter(_)
                    ) || required_access == StructuralAccess::MutableBorrow)
            };
            if !authenticated {
                return Err(LegalizationError::custody());
            }
        }
        let projection =
            terminal_semantics::structural_window_projection(types.iter(), root_type, path, field)
                .ok_or(LegalizationError::custody())?;
        let mut full = projection.path;
        let field_type = projection.structural_type;
        if let Some(referent) = &referent {
            let mut original = referent.carrier_path.clone();
            original.push(StructuralPathSegment::Referent);
            original.extend(full);
            full = original;
        }
        let geometry = structural_reference_input::reference_source_geometry(
            root.structural_type(),
            &full,
            types,
        )
        .ok_or(LegalizationError::custody())?;
        let (endpoint, byte_offset, elements) =
            (geometry.selected, geometry.byte_offset, geometry.elements);
        if endpoint != field_type {
            return Err(LegalizationError::custody());
        }
        let shape = structural_reference_input::shape(field_type, types)
            .ok_or(LegalizationError::custody())?;
        Ok(WindowExtent {
            path: full,
            field_type,
            byte_offset,
            shape,
            elements,
        })
    }

    pub(crate) fn moved(
        function: &PsiOptimizationFunction,
        root: &abstract_operations::StructuralWindowRoot,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
        result: &StructuralOperationResult,
        plan: &AbstractOperationPlan,
        custody: &super::reference_custody::Custody,
    ) -> Result<WindowExtent, LegalizationError> {
        if path.is_empty()
            && field.is_none()
            && !matches!(
                root,
                abstract_operations::StructuralWindowRoot::Parameter(_)
            )
        {
            return Err(LegalizationError::custody());
        }
        let extent = extent(
            function,
            root,
            path,
            field,
            plan,
            custody,
            StructuralAccess::MutableBorrow,
        )?;
        if result.structural_type != extent.field_type {
            return Err(LegalizationError::custody());
        }
        Ok(extent)
    }
}
pub(crate) mod byte_views {
    use super::{
        AbstractFunction, AbstractFunctionResult, AbstractOperationPlan, CallPlan, CallingPolicy,
        PsiOptimizationFunction, TargetFunction, ValueDefinitionSite, scalar_shape,
    };
    use crate::LegalizationError;

    pub(crate) fn validate(
        target: &TargetFunction,
        abstracted: &AbstractFunction,
        optimized: &PsiOptimizationFunction,
        native: ::target::NativeTarget,
        plan: &AbstractOperationPlan,
    ) -> Result<CallPlan, LegalizationError> {
        let (call_plan, scalar_parameters, structural_parameters) =
            match (&abstracted.result, &target.mixed_structural_scalar_abi) {
                (AbstractFunctionResult::Unit, None) if target.graph.call_plan.result.is_none() => {
                    (
                        &target.graph.call_plan,
                        &target.graph.scalar_parameters,
                        &target.graph.parameters,
                    )
                }
                (AbstractFunctionResult::Scalar(result), Some(abi))
                    if scalar_shape(result.scalar_type) == Some(abi.result.placement.shape)
                        && abi.result.value == result.value
                        && abi.result.scalar_type == result.scalar_type
                        && Some(&abi.result.placement) == abi.call_plan.result.as_ref() =>
                {
                    (
                        &abi.call_plan,
                        &abi.scalar_parameters,
                        &abi.structural_parameters,
                    )
                }
                _ => return Err(LegalizationError::custody()),
            };
        let parameters =
            abstracted
                .structural_parameters
                .iter()
                .zip(structural_parameters)
                .map(|(semantic, target)| {
                    crate::structural_inputs::structural_unit_input::Parameter { semantic, target }
                })
                .collect::<Vec<_>>();
        if target.machine != abstracted.machine
            || target.machine != optimized.machine
            || (target.attachment != abstracted.attachment
                || target.nominal_cleanup_subject != abstracted.nominal_cleanup_subject)
            || (target.attachment != optimized.attachment
                || target.nominal_cleanup_subject != optimized.nominal_cleanup_subject)
            || target.scalar_abi.is_some()
            || !crate::structural_inputs::structural_unit_input::accepts_graph(
                call_plan,
                &parameters,
                &plan.structural_types,
            )
            || abstracted.parameters.len() != optimized.parameters.len()
            || scalar_parameters.len() != abstracted.parameters.len()
            || call_plan.parameters.len()
                != abstracted.parameters.len() + abstracted.structural_parameters.len()
            || scalar_parameters
                .iter()
                .zip(&abstracted.parameters)
                .zip(&optimized.parameters)
                .zip(&call_plan.parameters)
                .enumerate()
                .any(|(position, (((actual, declared), optimized), placement))| {
                    actual.value != declared.value
                        || scalar_shape(actual.scalar_type).is_none()
                        || declared.scalar_type != actual.scalar_type
                        || optimized.value != declared.value
                        || optimized.scalar_type != declared.scalar_type
                        || optimized.site != ValueDefinitionSite::FunctionParameter(position as u32)
                        || scalar_shape(actual.scalar_type) != Some(placement.shape)
                        || actual.placement != *placement
                })
            || structural_parameters.is_empty()
            || structural_parameters.len() != abstracted.structural_parameters.len()
            || optimized.structural_parameters != abstracted.structural_parameters
            || optimized.result != abstracted.result
            || call_plan.policy != CallingPolicy::native_for_target(native)
            || !abstracted.entry_claims.is_empty()
            || !optimized.entry_claims.is_empty()
            || !optimized.entry_claim_declarations.is_empty()
            || !optimized.content_entry_claims.is_empty()
            || abstracted.published_service_ceiling != optimized.published_service_ceiling
            || !super::place_declarations::matches_producers(optimized)
        {
            return Err(LegalizationError::custody());
        }
        if optimized.block_structural_parameters().any(|parameter| {
            crate::structural_inputs::parameter_shape(parameter, &plan.structural_types).is_none()
        }) {
            return Err(LegalizationError::custody());
        }
        Ok(call_plan.clone())
    }

    pub(crate) fn writable_bytes(
        function: &PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> bool {
        let (structural_type, multiplicity, projected) =
            if let Some(parameter) = function.arrival_parameter(place) {
                (
                    parameter.structural_type,
                    parameter.multiplicity,
                    &parameter.projected_qualifications,
                )
            } else {
                let Ok((_, result)) = super::structural_case::source_result(function, place) else {
                    return false;
                };
                if !result.claims.is_empty() || !result.qualification_establishments.is_empty() {
                    return false;
                }
                (
                    result.structural_type,
                    result.multiplicity,
                    &result.projected_qualifications,
                )
            };
        if !projected.is_empty() {
            return false;
        }
        let Some(declaration) = terminal_psi::find_structural_type(types, structural_type) else {
            return false;
        };
        match optimization_unit_semantics::structural_place_backing_access(
            function,
            place,
            types.iter(),
        ) {
            Some(terminal_psi::StructuralAccess::MutableBorrow) => {
                matches!(
                    multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                ) && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(_)
                )
            }
            Some(terminal_psi::StructuralAccess::Owned) => {
                multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && declaration.shape.is_bounded_owned_byte_sequence()
            }
            _ => false,
        }
    }

    pub(crate) fn view_type(
        function: &PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        let identity = function
            .arrival_parameter(place)
            .map(|parameter| parameter.structural_type)
            .or_else(|| {
                super::structural_case::source_result(function, place)
                    .ok()
                    .map(|(_, result)| result.structural_type)
            })
            .or_else(|| {
                super::literals::declaration_producer(function, place).map(|(_, identity)| identity)
            })?;
        let declaration = terminal_psi::find_structural_type(types, identity)?;
        (declaration.shape.is_borrowed_byte_carrier()
            || matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::ElementView { .. }
            ))
        .then_some(identity)
    }

    pub(crate) fn descriptor_return(
        function: &PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> bool {
        let Some(declared) = function.result.structural() else {
            return false;
        };
        if view_type(function, place, types) != Some(declared.structural_type)
            || !matches!(
                declared.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
            || !terminal_semantics::view_descriptor_projected_qualifications(
                types.iter(),
                declared.structural_type,
                &declared.projected_qualifications,
            )
            || !terminal_semantics::descriptor_results::is_descriptor(
                types.iter(),
                declared.structural_type,
            )
        {
            return false;
        }
        if let Some(parameter) = function.arrival_parameter(place) {
            return matches!(
                parameter.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            ) && (declared.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                || parameter.access != terminal_psi::StructuralAccess::SharedBorrow)
                && (parameter.multiplicity == declared.multiplicity
                    || declared.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                        && parameter.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted)
                && terminal_semantics::view_descriptor_projected_qualifications(
                    types.iter(),
                    parameter.structural_type,
                    &parameter.projected_qualifications,
                );
        }
        if let Ok((_, result)) = super::structural_case::source_result(function, place) {
            return result.multiplicity == declared.multiplicity
                && result.claims.is_empty()
                && terminal_semantics::view_descriptor_projected_qualifications(
                    types.iter(),
                    result.structural_type,
                    &result.projected_qualifications,
                )
                && result.qualification_establishments.is_empty();
        }
        declared.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && super::literals::declaration_producer(function, place).is_some()
    }

    pub(crate) fn element_type(
        function: &PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        let identity = view_type(function, place, types)?;
        match terminal_psi::find_structural_type(types, identity)?.shape {
            terminal_psi::StructuralTypeShape::ElementView { element, .. } => Some(element),
            _ => None,
        }
    }

    pub(crate) fn contains_view(
        function: &PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> bool {
        view_type(function, place, types).is_some()
    }

    pub(crate) fn owned_byte_result(
        function: &PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<(
        semantic_vocabulary::OperationId,
        semantic_vocabulary::StructuralTypeId,
    )> {
        let (producer, result) = super::structural_case::source_result(function, place).ok()?;
        (result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && result.projected_qualifications.is_empty()
            && result.qualification_establishments.is_empty()
            && result.claims.is_empty()
            && types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && declaration.shape.is_bounded_owned_byte_sequence()
            }))
        .then_some((producer, result.structural_type))
    }
    pub(crate) fn reads_owner_layout(
        function: &PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> bool {
        owned_byte_result(function, place, types).is_some()
            || function.arrival_parameter(place).is_some_and(|parameter| {
                types.iter().any(|declaration| {
                    declaration.id == parameter.structural_type
                        && declaration.shape.is_bounded_owned_byte_sequence()
                })
            })
    }

    pub(crate) fn contains_readable_bytes(
        function: &PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> bool {
        view_type(function, place, types).is_some_and(|identity| {
            types
                .iter()
                .any(|row| row.id == identity && row.shape.is_borrowed_byte_carrier())
        }) || reads_owner_layout(function, place, types)
    }

    pub(crate) fn completed_descriptor<'function>(
        function: &'function PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<(
        semantic_vocabulary::OperationId,
        &'function terminal_psi::StructuralOperationResult,
    )> {
        let (producer, result) = super::structural_case::source_result(function, place).ok()?;
        (matches!(
            result.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        ) && terminal_semantics::view_descriptor_projected_qualifications(
            types.iter(),
            result.structural_type,
            &result.projected_qualifications,
        ) && result.qualification_establishments.is_empty()
            && result.claims.is_empty()
            && terminal_semantics::descriptor_results::is_descriptor(
                types.iter(),
                result.structural_type,
            ))
        .then_some((producer, result))
    }

    pub(crate) fn descriptor_result(
        function: &PsiOptimizationFunction,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<(
        semantic_vocabulary::OperationId,
        semantic_vocabulary::StructuralTypeId,
    )> {
        let (producer, result) = completed_descriptor(function, place, types)?;
        function
            .block_nodes()
            .any(|node| node.operation.materialized_structural_result() == Some((producer, result)))
            .then_some((producer, result.structural_type))
    }

    pub(crate) struct CallDescriptorOrigins<'a, 'g> {
        unit: &'a optimization_unit::PsiOptimizationUnit,
        native: &'a target_operations::TargetOperationPlan,
        plan: &'a abstract_operations::AbstractOperationPlan,
        graph: &'g std::cell::OnceCell<
            Option<Vec<terminal_semantics::descriptor_results::DescriptorMachine>>,
        >,
        analysis: Option<terminal_semantics::descriptor_results::DescriptorAnalysis<'g>>,
    }

    impl<'a, 'g> CallDescriptorOrigins<'a, 'g>
    where
        'a: 'g,
    {
        pub(crate) fn new(
            unit: &'a optimization_unit::PsiOptimizationUnit,
            native: &'a target_operations::TargetOperationPlan,
            plan: &'a abstract_operations::AbstractOperationPlan,
            graph: &'g std::cell::OnceCell<
                Option<Vec<terminal_semantics::descriptor_results::DescriptorMachine>>,
            >,
        ) -> Self {
            Self {
                unit,
                native,
                plan,
                graph,
                analysis: None,
            }
        }

        pub(crate) fn resolve(
            &mut self,
            caller: semantic_vocabulary::MachineId,
            result: &terminal_psi::StructuralOperationResult,
        ) -> Result<Vec<legalized_operations::LegalizedDescriptorOrigin>, LegalizationError>
        {
            if !matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            ) {
                return Ok(Vec::new());
            }
            let slots = terminal_semantics::descriptor_results::DescriptorSlotKind::Views
                .owned_slots(
                    &self.unit.structural_types,
                    result.place,
                    result.structural_type,
                )
                .ok_or_else(LegalizationError::custody)?;
            if slots.is_empty() {
                return Ok(Vec::new());
            }
            let analysis = match &mut self.analysis {
                Some(analysis) => analysis,
                None => {
                    let unit = self.unit;
                    let native = self.native;
                    let plan = self.plan;
                    let graph = self
                        .graph
                        .get_or_init(|| {
                            optimization_unit_semantics::descriptor_results::optimized_catalog_graph(
                                unit.functions.iter(),
                                &unit.boundary_machines,
                                &unit.structural_types,
                                &unit.dynamic_dispatch,
                                |function, node| {
                                    super::call_origin::selected_boundary_operation(
                                        node, function, native, plan,
                                    )
                                    .ok()
                                    .map(|selected| match selected {
                                        Some((operation, _)) => std::borrow::Cow::Owned(operation),
                                        None => std::borrow::Cow::Borrowed(&node.operation),
                                    })
                                },
                            )
                        })
                        .as_deref()
                        .ok_or_else(LegalizationError::custody)?;
                    self.analysis.insert(
                        terminal_semantics::descriptor_results::DescriptorAnalysis::new(
                            &self.unit.structural_types,
                            graph,
                        ),
                    )
                }
            };
            analysis
                .try_call_origins(caller, result.place)
                .map_err(|reason| LegalizationError::DescriptorCallOrigins {
                    machine: caller,
                    result: result.place,
                    reason,
                })?
                .into_iter()
                .map(|origin| match origin {
                    terminal_semantics::descriptor_results::DescriptorCallOrigin::Invocation {
                        ..
                    } => Err(LegalizationError::custody()),
                    terminal_semantics::descriptor_results::DescriptorCallOrigin::AccessFormation {
                        ..
                    } => Err(LegalizationError::custody()),
                    terminal_semantics::descriptor_results::DescriptorCallOrigin::Empty => {
                        Ok(legalized_operations::LegalizedDescriptorOrigin::Empty)
                    }
                    terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument {
                        source,
                        projection,
                    } => {
                        let projection = projection.without_runtime_coordinates();
                        Ok(legalized_operations::LegalizedDescriptorOrigin::Argument {
                            source: terminal_psi::StructuralArgument {
                                place: source,
                                path: projection.path,
                                access: terminal_psi::StructuralAccess::SharedBorrow,
                            },
                            exact: projection.exact,
                            indirect: projection.indirect,
                        })
                    }
                    terminal_semantics::descriptor_results::DescriptorCallOrigin::Literal {
                        machine,
                        place,
                    } => {
                        Ok(legalized_operations::LegalizedDescriptorOrigin::Literal { machine, place })
                    }
                })
                .collect()
        }
    }
}
pub(crate) mod call_origin {
    use super::{
        AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, OptimizationNode,
        PsiOptimizationFunction, TargetOperationPlan,
    };
    use crate::LegalizationError;
    use abstract_operations::AbstractStructuralCallResult;
    use target_operations::{NativeCallOrigin, TargetUnitOperation};

    pub(crate) fn validate_boundary_origins(
        native: &TargetOperationPlan,
        unit: &super::PsiOptimizationUnit,
        input: target_operations::BoundaryOriginFacts<'_>,
    ) -> Result<(), LegalizationError> {
        for function in &native.functions {
            for operation in function.graph.operations() {
                if let TargetUnitOperation::Call {
                    origin:
                        NativeCallOrigin::ProgramEntry {
                            boundary,
                            entry,
                            source_signature,
                            ..
                        },
                    target,
                    ..
                } = operation
                {
                    let binding = input.program_entry.ok_or_else(LegalizationError::custody)?;
                    if binding.boundary != *boundary
                        || binding.entry != *entry
                        || binding.source_signature != *source_signature
                        || *target != target_operations::NativeCallTarget::Machine(*entry)
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                if let TargetUnitOperation::Call {
                    origin:
                        NativeCallOrigin::InstalledProvider {
                            assumption: Some(assumption),
                            ..
                        },
                    ..
                } = operation
                    && !input.assumed_bindings.contains(assumption)
                {
                    return Err(LegalizationError::custody());
                }
                let TargetUnitOperation::Call {
                    psi_operation,
                    origin:
                        NativeCallOrigin::TargetFamily {
                            boundary,
                            member,
                            target,
                            completion_claim_sources,
                            completion_receipts,
                        },
                    target: call_target,
                    ..
                } = operation
                else {
                    continue;
                };
                let target_operations::NativeCallTarget::Machine(callee) = call_target else {
                    return Err(LegalizationError::custody());
                };
                let selections = input
                    .target_machine_selections
                    .ok_or_else(LegalizationError::custody)?;
                let module = input.module;
                let head = terminal_psi::TargetMachineCallable::Boundary(*boundary);
                let callable = terminal_psi::TargetMachineCallable::Machine(*member);
                let selected = selections
                    .member(head)
                    .map_err(|_| LegalizationError::custody())?;
                let mut families = module
                    .target_machine_families
                    .iter()
                    .filter(|family| family.canonical == head);
                let family = families.next().ok_or_else(LegalizationError::custody)?;
                if families.next().is_some()
                    || *member != *callee
                    || selections.terminal_psi() != native.psi
                    || selections.target() != target
                    || selected.callable() != callable
                    || !matches!(
                        selected.supply(),
                        terminal_psi::TargetMachineSupply::CheckedBody
                            | terminal_psi::TargetMachineSupply::Boundary
                    )
                    || family
                        .members
                        .iter()
                        .filter(|entry| {
                            entry.target == *target
                                && entry.callable == callable
                                && entry.supply == selected.supply()
                        })
                        .count()
                        != 1
                {
                    return Err(LegalizationError::custody());
                }
                let mut callers = unit
                    .functions
                    .iter()
                    .filter(|caller| caller.machine == function.machine);
                let caller = callers.next().ok_or_else(LegalizationError::custody)?;
                if callers.next().is_some() {
                    return Err(LegalizationError::custody());
                }
                let mut source_rows =
                    caller
                        .block_nodes()
                        .filter_map(|node| match &node.operation {
                            AbstractOperation::BoundaryCall {
                                psi_operation: identity,
                                boundary: source_boundary,
                                completion_claim_sources: sources,
                                completion_receipts: receipts,
                                ..
                            } if identity == psi_operation => {
                                Some((source_boundary, sources, receipts))
                            }
                            _ => None,
                        });
                let Some((source_boundary, sources, receipts)) = source_rows.next() else {
                    return Err(LegalizationError::custody());
                };
                if sources != completion_claim_sources
                    || receipts != completion_receipts
                    || source_boundary != boundary
                    || source_rows.next().is_some()
                {
                    return Err(LegalizationError::custody());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn selected_boundary_operation(
        node: &OptimizationNode,
        caller: &PsiOptimizationFunction,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
    ) -> Result<Option<(AbstractOperation, NativeCallOrigin)>, LegalizationError> {
        let AbstractOperation::BoundaryCall {
            psi_operation,
            result,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        } = &node.operation
        else {
            return Ok(None);
        };
        let function = native
            .function(caller.machine)
            .ok_or_else(|| LegalizationError::custody())?;
        let Some(
            target_call @ TargetUnitOperation::Call {
                origin,
                target: target_operations::NativeCallTarget::Machine(callee),
                ..
            },
        ) = super::unique_row(function, |row| {
            matches!(
                row,
                TargetUnitOperation::Call {
                    psi_operation: identity,
                    target: target_operations::NativeCallTarget::Machine(_),
                    ..
                } if identity == psi_operation
            )
        })?
        else {
            return Ok(None);
        };
        let callee = *callee;
        let family = matches!(origin, NativeCallOrigin::TargetFamily { .. });
        let assumed_entry = matches!(
            origin,
            NativeCallOrigin::InstalledProvider {
                assumption: Some(_),
                ..
            }
        );
        let entry = matches!(origin, NativeCallOrigin::ProgramEntry { .. });
        let provider = match origin {
            NativeCallOrigin::ProgramEntry {
                boundary: selected_boundary,
                entry,
                completion_claim_sources: retained_sources,
                completion_receipts: retained_receipts,
                ..
            } => {
                if selected_boundary != boundary
                    || *entry != callee
                    || retained_sources != completion_claim_sources
                    || retained_receipts != completion_receipts
                {
                    return Err(LegalizationError::custody());
                }
                None
            }
            NativeCallOrigin::TargetFamily {
                boundary: selected_boundary,
                member,
                completion_claim_sources: retained_sources,
                completion_receipts: retained_receipts,
                ..
            } => {
                if selected_boundary != boundary
                    || *member != callee
                    || retained_sources != completion_claim_sources
                    || retained_receipts != completion_receipts
                {
                    return Err(LegalizationError::custody());
                }
                None
            }
            NativeCallOrigin::InstalledProvider {
                boundary: selected_boundary,
                provider,
                assumption,
                completion_claim_sources: retained_sources,
                completion_receipts: retained_receipts,
            } => {
                if selected_boundary != boundary
                    || assumption
                        .as_ref()
                        .map_or(provider.boundary != *boundary, |row| {
                            !row.matches(*boundary, provider)
                        })
                    || provider.candidate != callee
                    || retained_sources != completion_claim_sources
                    || retained_receipts != completion_receipts
                    || !plan
                        .provider_candidates
                        .iter()
                        .any(|candidate| candidate == provider)
                {
                    return Err(LegalizationError::custody());
                }
                Some(provider)
            }
            NativeCallOrigin::Authored => return Err(LegalizationError::custody()),
        };
        let declaration = plan
            .boundary_machine(*boundary)
            .ok_or_else(|| LegalizationError::custody())?;
        let candidate = plan
            .function(callee)
            .ok_or_else(|| LegalizationError::custody())?;
        let provider_declaration = if let NativeCallOrigin::InstalledProvider {
            assumption: Some(row),
            ..
        } = origin
        {
            plan.boundary_machine(row.selected_requirement)
                .ok_or_else(|| LegalizationError::custody())?
        } else {
            declaration
        };
        if provider
            .is_some_and(|provider| provider_declaration.identity != provider.requirement_identity)
            || declaration.scalar_parameters.len() != arguments.len()
            || candidate.parameters.len() != arguments.len()
            || declaration
                .scalar_parameters
                .iter()
                .zip(&candidate.parameters)
                .any(|(expected, actual)| *expected != actual.scalar_type)
            || declaration.structural_parameters.len() != structural_arguments.len()
            || candidate.structural_parameters.len() != structural_arguments.len()
            || provider.is_some_and(|provider| {
                provider.signature.parameters.len() != structural_arguments.len()
                    || provider.refinement.required_domains != provider_declaration.requires
                    || provider.refinement.realized_service_ceiling
                        != candidate.published_service_ceiling
            })
            || (!entry
                && candidate
                    .published_service_ceiling
                    .iter()
                    .any(|service| !declaration.published_service_ceiling.contains(service)))
            || provider.is_some_and(|provider| {
                provider.refinement.positional_parameters.len() != structural_arguments.len()
                    || provider
                        .refinement
                        .positional_parameters
                        .iter()
                        .enumerate()
                        .any(|(position, refinement)| {
                            usize::try_from(refinement.boundary_index).ok() != Some(position)
                                || usize::try_from(refinement.candidate_index).ok()
                                    != Some(position)
                        })
            })
        {
            return Err(LegalizationError::custody());
        }
        for (position, (expected, actual)) in declaration
            .structural_parameters
            .iter()
            .zip(&candidate.structural_parameters)
            .enumerate()
        {
            if expected.position != actual.position
                || expected.structural_type != actual.structural_type
                || expected.access != actual.access
                || expected.multiplicity != actual.multiplicity
                || (!family
                    && !assumed_entry
                    && (expected.qualifications != actual.qualifications
                        || expected.projected_qualifications != actual.projected_qualifications))
                || (family
                    && (actual
                        .qualifications
                        .iter()
                        .any(|domain| !expected.qualifications.contains(domain))
                        || actual
                            .projected_qualifications
                            .iter()
                            .any(|row| !expected.projected_qualifications.contains(row))))
                || provider.is_some_and(|provider| {
                    let signature = &provider.signature.parameters[position];
                    expected.position != signature.position
                        || expected.structural_type != signature.structural_type
                        || expected.access != signature.access
                        || expected.multiplicity != signature.multiplicity
                        || (!assumed_entry
                            && (expected.qualifications != signature.qualifications
                                || expected.projected_qualifications
                                    != signature.projected_qualifications))
                })
            {
                return Err(LegalizationError::custody());
            }
        }
        let transfers = completion_receipts
            .iter()
            .map(|receipt| terminal_psi::ClaimTransfer {
                claim: receipt.claim,
                argument_index: receipt.argument_index,
                path: receipt.path.clone(),
            })
            .collect::<Vec<_>>();
        let operation = match (result, &declaration.result, &candidate.result, target_call) {
            (
                abstract_operations::AbstractBoundaryResult::Scalar(result),
                terminal_psi::BoundaryMachineResult::Scalar(boundary_scalar),
                AbstractFunctionResult::Scalar(candidate_result),
                TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Scalar(actual),
                    ..
                },
            ) if actual.source_value == result.value
                && actual.scalar_type == result.scalar_type
                && actual.defining_operation == *psi_operation
                && Some(actual.shape) == super::scalar_shape(result.scalar_type)
                && result.scalar_type == boundary_scalar.scalar_type
                && candidate_result.scalar_type == boundary_scalar.scalar_type =>
            {
                AbstractOperation::CallStructural {
                    psi_operation: *psi_operation,
                    result: AbstractStructuralCallResult::Scalar(*result),
                    callee: callee.into(),
                    arguments: arguments.clone(),
                    structural_arguments: structural_arguments.clone(),
                    claim_transfers: transfers,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            }
            (
                abstract_operations::AbstractBoundaryResult::Unit,
                terminal_psi::BoundaryMachineResult::Unit,
                AbstractFunctionResult::Unit,
                TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Unit,
                    ..
                },
            ) => AbstractOperation::CallStructural {
                psi_operation: *psi_operation,
                callee: callee.into(),
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers: transfers,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                result: AbstractStructuralCallResult::Unit,
            },
            (
                abstract_operations::AbstractBoundaryResult::Structural(result),
                terminal_psi::BoundaryMachineResult::Structural(boundary_result),
                AbstractFunctionResult::Structural(candidate_result),
                TargetUnitOperation::Call {
                    result:
                        target_operations::TargetCallResult::Structural {
                            result: actual,
                            callee_result,
                            ..
                        },
                    ..
                },
            ) if actual == result
                && matches!(callee_result, target_operations::TargetStructuralResultContract::Machine(retained) if retained == candidate_result)
                && result.structural_type == boundary_result.structural_type
                && candidate_result.structural_type == boundary_result.structural_type
                && result.multiplicity == boundary_result.multiplicity
                && candidate_result.multiplicity == boundary_result.multiplicity
                && result.projected_qualifications == boundary_result.projected_qualifications
                && boundary_result
                    .projected_qualifications
                    .iter()
                    .all(|row| candidate_result.projected_qualifications.contains(row))
                && result
                    .qualifications
                    .iter()
                    .all(|domain| boundary_result.qualifications.contains(domain))
                && if family {
                    boundary_result
                        .qualifications
                        .iter()
                        .all(|domain| candidate_result.qualifications.contains(domain))
                } else {
                    candidate_result
                        .qualifications
                        .iter()
                        .all(|domain| boundary_result.qualifications.contains(domain))
                } =>
            {
                AbstractOperation::CallStructural {
                    psi_operation: *psi_operation,
                    result: AbstractStructuralCallResult::Structural {
                        result: result.clone(),
                        selected_evidence: Vec::new(),
                    },
                    callee: callee.into(),
                    arguments: arguments.clone(),
                    structural_arguments: structural_arguments.clone(),
                    claim_transfers: transfers,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            }
            _ => return Err(LegalizationError::custody()),
        };
        Ok(Some((operation, origin.clone())))
    }
}
pub(crate) mod call_signature {
    use crate::LegalizationError;
    use abstract_operations::{AbstractCallSignature, AbstractCallTarget, AbstractOperationPlan};

    pub(crate) fn signature(
        target: impl Into<AbstractCallTarget>,
        plan: &AbstractOperationPlan,
    ) -> Result<AbstractCallSignature<'_>, LegalizationError> {
        match target.into() {
            AbstractCallTarget::Machine(machine) => plan
                .function(machine)
                .map(AbstractCallSignature::Machine)
                .ok_or(LegalizationError::custody()),
            AbstractCallTarget::Dynamic {
                interface, slot, ..
            } => {
                let interface = plan
                    .dynamic_dispatch
                    .interface(interface)
                    .ok_or(LegalizationError::custody())?;
                let row = interface
                    .slots
                    .get(slot as usize)
                    .filter(|row| row.slot == slot)
                    .ok_or(LegalizationError::custody())?;
                Ok(AbstractCallSignature::Requirement(row))
            }
        }
    }

    pub(crate) fn native_target(
        target: impl Into<AbstractCallTarget>,
    ) -> target_operations::NativeCallTarget {
        target.into()
    }

    pub(crate) fn parameter(
        signature: AbstractCallSignature<'_>,
        position: usize,
        place: semantic_vocabulary::PlaceId,
    ) -> Result<terminal_psi::StructuralParameterDeclaration, LegalizationError> {
        let formal = signature
            .structural_parameter(position)
            .ok_or(LegalizationError::custody())?;
        Ok(terminal_psi::StructuralParameterDeclaration {
            place,
            position: formal.position,
            structural_type: formal.structural_type,
            multiplicity: formal.multiplicity,
            access: formal.access,
            qualifications: formal.qualifications.to_vec(),
            projected_qualifications: formal.projected_qualifications.to_vec(),
            additional_input_qualifications: formal.additional_input_qualifications.to_vec(),
        })
    }
}
pub(crate) mod cleanup_calls {
    use crate::LegalizationError;
    use abstract_operations::{AbstractFunctionResult, AbstractOperation, AbstractOperationPlan};
    use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
    use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall};
    use optimization_unit::PsiOptimizationUnit;
    use target_operations::{CallSiteOwner, TargetOperationPlan, TargetUnitOperation};
    use terminal_psi::{StructuralArgument, TerminalAffineCleanupAction};

    pub(crate) fn expected(
        operation: &AbstractOperation,
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<Vec<(CallSiteOwner, LegalizedScalarCall)>, LegalizationError> {
        let caller = function_inputs.function();
        let mut calls = Vec::new();
        for (edge, actions) in operation.cleanup_rosters() {
            for (ordinal, action) in actions.iter().enumerate() {
                let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
                    continue;
                };
                let callee = unit
                    .function(cleanup.cleanup_machine)
                    .ok_or(LegalizationError::custody())?;
                if callee.machine == caller.machine
                    || callee.attachment != Some(cleanup.structural_type)
                    || callee.nominal_cleanup_subject != Some(cleanup.structural_type)
                    || callee.result != AbstractFunctionResult::Unit
                    || !callee.parameters.is_empty()
                    || callee.structural_parameters.len() > 1
                    || !cleanup.requirement_obligations.is_empty()
                {
                    return Err(LegalizationError::custody());
                }
                let owner = CallSiteOwner::CleanupAction {
                    edge,
                    action_ordinal: ordinal
                        .try_into()
                        .map_err(|_| LegalizationError::custody())?,
                };
                let shapes = callee
                    .structural_parameters
                    .iter()
                    .map(|parameter| {
                        crate::structural_inputs::parameter_shape(parameter, &plan.structural_types)
                            .ok_or(LegalizationError::custody())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let call_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(native.target),
                    &CallSignature {
                        parameters: shapes,
                        result: None,
                    },
                )
                .map_err(|_| LegalizationError::custody())?;
                let mut arguments = Vec::new();
                for (position, parameter) in callee.structural_parameters.iter().enumerate() {
                    if position != 0 || !parameter.is_cleanup_receiver(cleanup) {
                        return Err(LegalizationError::custody());
                    }
                    let semantic = StructuralArgument {
                        place: cleanup.place,
                        path: cleanup.path.clone(),
                        access: parameter.access,
                    };
                    let target =
                        super::aggregate_results::borrowed_arguments::reconstruct_at_owner(
                            &semantic,
                            position,
                            owner,
                            caller,
                            super::call_signature::signature(callee.machine, plan)?,
                            &call_plan,
                            native,
                            plan,
                            || function_inputs.address_join_parameter(semantic.place, plan),
                        )?;
                    let indices = crate::legalization::runtime_indices::projection(
                        caller,
                        unit,
                        optimization_unit::PsiProvenance::Edge(edge),
                        target.root_structural_type,
                        &target.path,
                        &unit.structural_types,
                    )?;
                    arguments.push(LegalizedScalarArgument::Structural {
                        semantic,
                        target,
                        indices,
                    });
                }
                if callee.structural_parameters.is_empty() && cleanup.cleanup_receiver.is_some() {
                    return Err(LegalizationError::custody());
                }
                calls.push((
                    owner,
                    LegalizedScalarCall {
                        source: target_operations::NativeCallOrigin::Authored,
                        target: target_operations::NativeCallTarget::Machine(callee.machine),
                        call_plan,
                        arguments,
                        result_placement: None,
                        structural_result: None,
                        descriptor_origins: Vec::new(),
                        access_bindings: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                ));
            }
        }
        Ok(calls)
    }

    pub(crate) fn validate_target(
        rows: &[TargetUnitOperation],
        operation: &AbstractOperation,
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<(), LegalizationError> {
        let expected = expected(operation, function_inputs, native, plan, unit)?;
        if rows.len() != expected.len() {
            return Err(LegalizationError::custody());
        }
        for (row, (owner, call)) in rows.iter().zip(expected) {
            let TargetUnitOperation::CleanupCall {
                owner: actual,
                callee,
                call_plan,
                arguments,
            } = row
            else {
                return Err(LegalizationError::custody());
            };
            let expected_arguments = call
                .arguments
                .iter()
                .map(|argument| match argument {
                    LegalizedScalarArgument::Structural { target, .. } => target.clone(),
                    _ => unreachable!("cleanup has no scalar parameters"),
                })
                .collect::<Vec<_>>();
            if *actual != owner
                || Some(*callee) != call.target.machine()
                || *call_plan != call.call_plan
                || *arguments != expected_arguments
            {
                return Err(LegalizationError::custody());
            }
        }
        Ok(())
    }
}
pub(crate) mod control {
    use super::{
        AbstractFunctionResult, AbstractOperation, OptimizationNode, PsiProvenance, ScalarType,
    };
    use crate::LegalizationError;
    use terminal_semantics::descriptor_results::is_descriptor;
    pub(crate) fn validate(
        node: &OptimizationNode,
        _body: &[OptimizationNode],
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<(), LegalizationError> {
        let function = function_inputs.function();
        match (&node.operation, &function.result) {
            (AbstractOperation::InvokeNoNormal { psi_edge, .. }, _)
            | (AbstractOperation::ExternalCompletion { psi_edge, .. }, _)
            | (AbstractOperation::Crash { psi_edge, .. }, _) => terminal_edge(node, *psi_edge),
            (AbstractOperation::StructuralCase { .. }, _) => {
                super::structural_case::validate(node, function_inputs, structural_types)
            }
            (
                AbstractOperation::ReturnStructural {
                    psi_edge,
                    source,
                    returned_claims,
                    trivial_affine_locals,
                    cleanup_actions,
                },
                AbstractFunctionResult::Structural(declared),
            ) => {
                if is_descriptor(structural_types.iter(), declared.structural_type) {
                    if !super::byte_views::descriptor_return(function, *source, structural_types)
                        || !returned_claims.is_empty()
                        || !trivial_affine_locals.is_empty()
                        || !super::aggregate_results::cleanup(
                            function_inputs,
                            cleanup_actions,
                            structural_types,
                        )
                    {
                        return Err(LegalizationError::custody());
                    }
                    return terminal_edge(node, *psi_edge);
                }
                let (structural_type, multiplicity) =
                    if let Some(parameter) = function.structural_parameter(*source) {
                        if parameter.access != terminal_psi::StructuralAccess::Owned {
                            return Err(LegalizationError::custody());
                        }
                        (parameter.structural_type, parameter.multiplicity)
                    } else {
                        match super::structural_case::source_owner(function, *source)? {
                        legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                            result,
                            ..
                        } => (result.structural_type, result.multiplicity),
                        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                            declaration,
                            ..
                        } => (declaration.structural_type, declaration.multiplicity),
                        legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
                        | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                            ..
                        } => {
                            return Err(LegalizationError::custody());
                        }
                    }
                    };
                if structural_type != declared.structural_type
                    || multiplicity != declared.multiplicity
                    || !trivial_affine_locals.is_empty()
                    || !super::aggregate_results::cleanup(
                        function_inputs,
                        cleanup_actions,
                        structural_types,
                    )
                {
                    return Err(LegalizationError::custody());
                }
                terminal_edge(node, *psi_edge)
            }
            (
                AbstractOperation::ReturnUnit {
                    psi_edge,
                    cleanup_actions,
                },
                AbstractFunctionResult::Unit,
            ) if cleanup_actions.is_empty()
                || (super::aggregate_results::cleanup(
                    function_inputs,
                    cleanup_actions,
                    structural_types,
                ) && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )])
                || (super::read_byte::cleanup(function, cleanup_actions)
                    && node.ownership
                        == [optimization_unit::OwnershipEvent::Cleanup(
                            cleanup_actions.clone(),
                        )]) =>
            {
                terminal_edge(node, *psi_edge)
            }
            (
                AbstractOperation::Return {
                    psi_edge,
                    result,
                    value,
                    scalar_type,
                    cleanup_actions,
                },
                AbstractFunctionResult::Scalar(declared),
            ) if *result == declared.value
                && *scalar_type == declared.scalar_type
                && function_inputs.value_type(*value) == Some(*scalar_type)
                && (cleanup_actions.is_empty()
                    || (super::aggregate_results::cleanup(
                        function_inputs,
                        cleanup_actions,
                        structural_types,
                    ) && node.ownership
                        == [optimization_unit::OwnershipEvent::Cleanup(
                            cleanup_actions.clone(),
                        )])) =>
            {
                terminal_edge(node, *psi_edge)
            }
            (
                AbstractOperation::Jump {
                    psi_edge,
                    target,
                    bindings,
                    structural_bindings,
                    cleanup_actions,
                },
                _,
            ) => {
                let [edge] = node.successors.as_slice() else {
                    return Err(LegalizationError::custody());
                };
                if edge.psi_edge != *psi_edge
                    || edge.target != *target
                    || edge.bindings != *bindings
                    || edge.structural_bindings != *structural_bindings
                    || edge.cleanup_actions != *cleanup_actions
                {
                    return Err(LegalizationError::custody());
                }
                branch_edges(node, function_inputs, structural_types)
            }
            (
                AbstractOperation::Conditional {
                    condition,
                    when_true,
                    when_false,
                },
                _,
            ) => {
                if function_inputs.value_type(*condition) != Some(ScalarType::Boolean) {
                    return Err(LegalizationError::custody());
                }
                if node.successors.len() != 2 {
                    return Err(LegalizationError::custody());
                }
                for (actual, expected) in node.successors.iter().zip([when_true, when_false]) {
                    if actual.psi_edge != expected.psi_edge
                        || actual.target != expected.target
                        || actual.bindings != expected.bindings
                        || actual.structural_bindings != expected.structural_bindings
                        || actual.cleanup_actions != expected.cleanup_actions
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                branch_edges(node, function_inputs, structural_types)
            }
            _ => Err(LegalizationError::custody()),
        }
    }

    fn terminal_edge(
        node: &OptimizationNode,
        edge: semantic_vocabulary::EdgeId,
    ) -> Result<(), LegalizationError> {
        if !node.successors.is_empty()
            || node.provenance != [PsiProvenance::Edge(edge)]
            || node.work.is_empty()
            || node
                .work
                .iter()
                .any(|work| work.site != PsiProvenance::Edge(edge))
        {
            return Err(LegalizationError::custody());
        }
        Ok(())
    }
    fn branch_edges(
        node: &OptimizationNode,
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<(), LegalizationError> {
        if !node.provenance.is_empty()
            || !node.work.is_empty()
            || node.successors.iter().any(|edge| {
                !super::aggregate_results::cleanup(
                    function_inputs,
                    &edge.cleanup_actions,
                    structural_types,
                ) || edge.provenance != [PsiProvenance::Edge(edge.psi_edge)]
                    || edge.work.is_empty()
                    || edge
                        .work
                        .iter()
                        .any(|work| work.site != PsiProvenance::Edge(edge.psi_edge))
            })
        {
            return Err(LegalizationError::custody());
        }
        Ok(())
    }
}
pub(crate) mod element_views {
    use crate::structural_inputs::structural_reference_input::{
        ReferenceSourceGeometry, descriptor_source_geometry,
    };
    use optimization_unit::PsiOptimizationFunction;
    use terminal_psi::{StructuralAccess, StructuralArgument, StructuralTypeDeclaration};

    pub(crate) fn descriptor_geometry(
        function: &PsiOptimizationFunction,
        source: &StructuralArgument,
        types: &[StructuralTypeDeclaration],
    ) -> Option<ReferenceSourceGeometry> {
        descriptor_geometry_access(function, source, types, false)
    }

    pub(crate) fn descriptor_metadata_geometry(
        function: &PsiOptimizationFunction,
        source: &StructuralArgument,
        types: &[StructuralTypeDeclaration],
    ) -> Option<ReferenceSourceGeometry> {
        descriptor_geometry_access(function, source, types, true)
    }

    fn descriptor_geometry_access(
        function: &PsiOptimizationFunction,
        source: &StructuralArgument,
        types: &[StructuralTypeDeclaration],
        metadata: bool,
    ) -> Option<ReferenceSourceGeometry> {
        let actual = optimization_unit_semantics::structural_place_backing_access(
            function,
            source.place,
            types.iter(),
        )?;
        if (!matches!(
            source.access,
            StructuralAccess::Owned
                | StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
        ) && !(metadata && source.access == StructuralAccess::WriteOnlyBorrow))
            || !(actual == source.access
                || actual == StructuralAccess::Owned
                || actual == StructuralAccess::MutableBorrow
                    && source.access == StructuralAccess::SharedBorrow)
        {
            return None;
        }
        if source.access == StructuralAccess::WriteOnlyBorrow
            && source
                .path
                .iter()
                .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
        {
            return None;
        }
        let root = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == source.place)
            .map(|parameter| parameter.structural_type)
            .or_else(|| {
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters)
                    .find(|parameter| parameter.place == source.place)
                    .map(|parameter| parameter.structural_type)
            })
            .or_else(|| {
                super::structural_case::source_result(function, source.place)
                    .ok()
                    .map(|(_, result)| result.structural_type)
            })
            .or_else(|| {
                super::literals::declaration_producer(function, source.place)
                    .map(|(_, identity)| identity)
            })?;
        let requested = if source.access == StructuralAccess::Owned {
            StructuralAccess::SharedBorrow
        } else {
            source.access
        };
        if if source.access == StructuralAccess::WriteOnlyBorrow {
            !optimization_unit_semantics::descriptor_metadata_argument_accessible(
                function, source, types, requested,
            )
        } else {
            !optimization_unit_semantics::descriptor_argument_accessible(
                function, source, types, requested,
            )
        } {
            return None;
        }
        descriptor_source_geometry(root, &source.path, types)
    }

    pub(crate) fn length_geometry(
        function: &PsiOptimizationFunction,
        source: &StructuralArgument,
        types: &[StructuralTypeDeclaration],
    ) -> Option<ReferenceSourceGeometry> {
        let mut geometry = descriptor_metadata_geometry(function, source, types)?;
        if !geometry.readable
            || !types.iter().any(|declaration| {
                declaration.id == geometry.selected
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ElementView { .. }
                    )
            })
        {
            return None;
        }
        geometry.byte_offset = geometry.byte_offset.checked_add(8)?;
        Some(geometry)
    }

    pub(crate) fn byte_length_geometry(
        function: &PsiOptimizationFunction,
        source: &StructuralArgument,
        types: &[StructuralTypeDeclaration],
    ) -> Option<(ReferenceSourceGeometry, terminal_psi::ByteSequenceCarrier)> {
        let mut geometry = descriptor_metadata_geometry(function, source, types)?;
        if !geometry.readable {
            return None;
        }
        let terminal_psi::StructuralTypeShape::ByteSequence(carrier) = types
            .iter()
            .find(|declaration| declaration.id == geometry.selected)?
            .shape
        else {
            return None;
        };
        if matches!(
            carrier,
            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
        ) {
            geometry.byte_offset = geometry.byte_offset.checked_add(8)?;
        }
        Some((geometry, carrier))
    }

    pub(crate) fn fixed_byte_length(
        function: &PsiOptimizationFunction,
        source: &StructuralArgument,
        types: &[StructuralTypeDeclaration],
    ) -> Option<u64> {
        let geometry = descriptor_metadata_geometry(function, source, types)?;
        if !geometry.readable
            || !types.iter().any(|row| {
                row.id == geometry.selected
                    && matches!(
                        row.shape,
                        terminal_psi::StructuralTypeShape::FixedArray { .. }
                    )
            })
        {
            return None;
        }
        terminal_semantics::fixed_byte_extent(types.iter(), geometry.selected)
    }
}
pub(crate) mod header {
    use super::{
        AbstractFunction, AbstractFunctionResult, CallPlan, CallSignature, CallingPolicy,
        PsiOptimizationFunction, TargetFunction, ValueDefinitionSite, evaluate_call_plan,
        scalar_shape,
    };
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::scalar_register;
    pub(crate) fn function_abi(
        native: ::target::NativeTarget,
        target: &TargetFunction,
        abstracted: &AbstractFunction,
        optimized: &PsiOptimizationFunction,
    ) -> Result<CallPlan, LegalizationError> {
        if target.machine != abstracted.machine
            || target.machine != optimized.machine
            || (target.attachment != abstracted.attachment
                || target.nominal_cleanup_subject != abstracted.nominal_cleanup_subject)
            || (target.attachment != optimized.attachment
                || target.nominal_cleanup_subject != optimized.nominal_cleanup_subject)
            || target.mixed_structural_scalar_abi.is_some()
            || optimized.result != abstracted.result
            || !abstracted.structural_parameters.is_empty()
            || !optimized.structural_parameters.is_empty()
            || !super::place_declarations::matches_producers(optimized)
            || !abstracted.entry_claims.is_empty()
            || !optimized.entry_claim_declarations.is_empty()
            || !optimized.content_entry_claims.is_empty()
            || !optimized.entry_claims.is_empty()
            || abstracted.published_service_ceiling != optimized.published_service_ceiling
            || abstracted.parameters.len() != optimized.parameters.len()
            || abstracted
                .parameters
                .iter()
                .zip(&optimized.parameters)
                .enumerate()
                .any(|(index, (declared, actual))| {
                    scalar_shape(declared.scalar_type).is_none()
                        || actual.value != declared.value
                        || actual.scalar_type != declared.scalar_type
                        || actual.site != ValueDefinitionSite::FunctionParameter(index as u32)
                })
        {
            return Err(LegalizationError::custody());
        }
        let result = match &abstracted.result {
            AbstractFunctionResult::Unit => None,
            AbstractFunctionResult::Scalar(result)
                if scalar_shape(result.scalar_type).is_some() =>
            {
                scalar_shape(result.scalar_type)
            }
            _ => {
                return Err(LegalizationError::custody());
            }
        };
        let expected_parameters = abstracted
            .parameters
            .iter()
            .map(|parameter| {
                scalar_shape(parameter.scalar_type).ok_or(LegalizationError::custody())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expected = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: expected_parameters,
                result,
            },
        )
        .map_err(|_| LegalizationError::custody())?;
        if expected
            .result
            .as_ref()
            .is_some_and(|value| !scalar_register(value))
        {
            return Err(LegalizationError::custody());
        }
        let Some(abi) = &target.scalar_abi else {
            return Err(LegalizationError::custody());
        };
        let result_matches = match (abstracted.result.scalar(), abi.result.as_ref()) {
            (None, None) => expected.result.is_none(),
            (Some(declared), Some(actual)) => {
                actual.value == declared.value
                    && actual.scalar_type == declared.scalar_type
                    && Some(&actual.placement) == expected.result.as_ref()
            }
            _ => false,
        };
        if !result_matches
            || abi.call_plan != expected
            || abi.parameters.len() != abstracted.parameters.len()
            || abi
                .parameters
                .iter()
                .zip(&abstracted.parameters)
                .zip(&expected.parameters)
                .any(|((actual, declared), placement)| {
                    actual.value != declared.value
                        || actual.scalar_type != declared.scalar_type
                        || actual.placement != *placement
                })
            || !target.graph.parameters.is_empty()
            || target.graph.call_plan != expected
            || target.graph.scalar_parameters != abi.parameters
        {
            return Err(LegalizationError::custody());
        }
        Ok(expected)
    }
}
pub(crate) mod hosted_scalar {
    use super::{
        AbstractOperation, AbstractOperationPlan, MachineId, PsiOptimizationFunction,
        TargetOperationPlan,
    };
    use crate::LegalizationError;
    use semantic_vocabulary::OperationId;
    use target_operations::{BoundaryRealization, TargetUnitOperation};

    pub(crate) fn validate_normal_call_completions(
        _native: &TargetOperationPlan,
        optimized: &PsiOptimizationFunction,
        plan: &AbstractOperationPlan,
    ) -> Result<(), LegalizationError> {
        for block in &optimized.blocks {
            for node in &block.nodes {
                let AbstractOperation::BoundaryCall { boundary, .. } = node.operation else {
                    continue;
                };
                if !plan.boundary_machines.iter().any(|declaration| {
                    declaration.id == boundary
                        && declaration.completion
                            == semantic_vocabulary::BoundaryCompletion::NormalReturn
                }) {
                    return Err(LegalizationError::custody());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn hosted_realization(
        native: &TargetOperationPlan,
        machine: MachineId,
        operation: OperationId,
    ) -> Result<BoundaryRealization, LegalizationError> {
        let mut functions = native
            .functions
            .iter()
            .filter(|function| function.machine == machine);
        let function = functions.next().ok_or(LegalizationError::custody())?;
        if functions.next().is_some() {
            return Err(LegalizationError::custody());
        }
        let Some(TargetUnitOperation::BoundarySettlement { realization, .. }) =
            super::unique_row(function, |row| {
                matches!(
                    row,
                    TargetUnitOperation::BoundarySettlement { psi_operation, .. }
                        if *psi_operation == operation
                )
            })?
        else {
            return Err(LegalizationError::custody());
        };
        Ok(*realization)
    }
}
pub(crate) mod ieee_float_arithmetic {
    use super::LegalizationError;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan};
    use optimization_unit::PsiOptimizationUnit;
    use semantic_vocabulary::{MachineId, OperationId, ValueId};
    use target_operations::{
        BoundaryExecutionBinding, CompilerBuiltinExecution, TargetOperationPlan,
        TargetUnitOperation, TargetUnitScalarArgumentSource,
    };

    pub(crate) fn row(
        native: &TargetOperationPlan,
        machine: MachineId,
        operation: OperationId,
    ) -> Result<Option<&TargetUnitOperation>, LegalizationError> {
        let function = native
            .function(machine)
            .ok_or(LegalizationError::custody())?;
        super::unique_row(function, |row| {
            matches!(
                row,
                TargetUnitOperation::NearestIeeeFloatArithmetic { psi_operation, .. }
                | TargetUnitOperation::IeeeFloatClassify { psi_operation, .. }
                    if *psi_operation == operation
            )
        })
    }

    pub(crate) fn validate(
        target: &TargetUnitOperation,
        source: &AbstractOperation,
        native: ::target::NativeTarget,
        plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
        sources: &mut crate::legalization_target::control_flow::sources::AvailableSources<'_>,
    ) -> Result<(), LegalizationError> {
        if let (
            TargetUnitOperation::IeeeFloatClassify {
                psi_operation,
                boundary,
                execution,
                realization,
                result_home,
                argument,
            },
            AbstractOperation::BoundaryCall {
                psi_operation: expected_operation,
                boundary: expected_boundary,
                result: AbstractBoundaryResult::Structural(result),
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            },
        ) = (target, source)
        {
            let declaration = plan
                .boundary_machines
                .iter()
                .find(|row| row.id == *boundary)
                .ok_or(LegalizationError::custody())?;
            let structural_type = plan
                .structural_types
                .iter()
                .find(|row| row.id == result.structural_type)
                .ok_or(LegalizationError::custody())?;
            let layout = target_operations::conventional_float_class_layout(structural_type)
                .ok_or(LegalizationError::custody())?;
            let operand = semantic_vocabulary::ScalarType::IeeeFloat(realization.format);
            if psi_operation != expected_operation
                || boundary != expected_boundary
                || realization.operation
                    != target_operations::IeeeFloatArithmeticOperation::Classify
                || *execution
                    != BoundaryExecutionBinding::CompilerBuiltin(
                        CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                            operation: realization.operation,
                            format: realization.format,
                        },
                    )
                || !realization.supports_target(native)
                || unit.boundary_machine(*boundary) != Some(declaration)
                || declaration.scalar_parameters != [operand]
                || !declaration.structural_parameters.is_empty()
                || !matches!(&declaration.result, terminal_psi::BoundaryMachineResult::Structural(declared) if declared.structural_type == result.structural_type && declared.multiplicity == result.multiplicity && declared.qualifications == result.qualifications && declared.projected_qualifications == result.projected_qualifications)
                || !result.is_unrestricted_plain()
                || !result.qualification_establishments.is_empty()
                || result_home.operation_result() != Some((*psi_operation, result))
                || result_home.layout != target_operations::TargetStructuralHomeLayout::Sum(layout)
                || arguments.as_slice() != [argument.source_value()]
                || argument.scalar_type() != operand
                || !sources
                    .matching(argument.source_value())
                    .filter(|(value, _)| *value == argument.source_value())
                    .map(|(_, available)| available)
                    .eq(std::iter::once(argument))
                || !structural_arguments.is_empty()
                || !completion_claim_sources.is_empty()
                || !completion_receipts.is_empty()
            {
                return Err(LegalizationError::custody());
            }
            return Ok(());
        }
        let (
            TargetUnitOperation::NearestIeeeFloatArithmetic {
                psi_operation,
                boundary,
                execution,
                realization,
                result_home,
                arguments: target_arguments,
            },
            AbstractOperation::BoundaryCall {
                psi_operation: expected_operation,
                boundary: expected_boundary,
                result: AbstractBoundaryResult::Scalar(result),
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            },
        ) = (target, source)
        else {
            return Err(LegalizationError::custody());
        };
        let mut declarations = plan
            .boundary_machines
            .iter()
            .filter(|row| row.id == *boundary);
        let declaration = declarations.next().ok_or(LegalizationError::custody())?;
        if declarations.next().is_some() || unit.boundary_machine(*boundary) != Some(declaration) {
            return Err(LegalizationError::custody());
        }
        let scalar = realization.operation.result_type(realization.format);
        let operand = realization.operation.operand_type(realization.format);
        let expected_execution = BoundaryExecutionBinding::CompilerBuiltin(
            CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                operation: realization.operation,
                format: realization.format,
            },
        );
        let exactly_available = |value: &ValueId, source: &TargetUnitScalarArgumentSource| {
            source.source_value() == *value
                && sources
                    .matching(*value)
                    .filter(|(identity, _)| identity == value)
                    .map(|(_, available)| available)
                    .eq(std::iter::once(source))
        };
        if psi_operation != expected_operation
            || boundary != expected_boundary
            || *execution != expected_execution
            || !realization.supports_target(native)
            || !realization.signature_matches(&declaration.scalar_parameters, scalar)
            || !declaration.structural_parameters.is_empty()
            || declaration.result != terminal_psi::BoundaryMachineResult::Scalar(scalar.into())
            || !declaration.requires.is_empty()
            || !declaration.program_local_root_introductions.is_empty()
            || !declaration.content_guarantees.is_empty()
            || !structural_arguments.is_empty()
            || !completion_claim_sources.is_empty()
            || !completion_receipts.is_empty()
            || result.scalar_type != scalar
            || result_home.defining_operation != *psi_operation
            || result_home.source_value != result.value
            || result_home.scalar_type != scalar
            || Some(result_home.shape) != super::scalar_shape(scalar)
            || arguments.len() != realization.operation.operand_count()
            || !arguments
                .iter()
                .zip(target_arguments)
                .all(|(expected, source)| {
                    source.scalar_type() == operand && exactly_available(expected, source)
                })
            || arguments.len() != target_arguments.len()
        {
            return Err(LegalizationError::custody());
        }
        sources.push((
            result.value,
            TargetUnitScalarArgumentSource::Home(*result_home),
        ));
        Ok(())
    }
}
pub(crate) mod interrupt_installation {
    use crate::LegalizationError;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan};
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
    use target_operations::{InstallationCallRole, TargetOperationPlan, TargetUnitOperation};

    pub(crate) fn validate(
        target: &TargetUnitOperation,
        source: &AbstractOperation,
        caller: &PsiOptimizationFunction,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
        custody: &super::reference_custody::Custody,
    ) -> Result<(), LegalizationError> {
        let TargetUnitOperation::InstallStaticInterrupt {
            psi_operation,
            boundary,
            arguments,
            calls,
            result_home,
        } = target
        else {
            return Err(LegalizationError::custody());
        };
        let fail = |reason| LegalizationError::InstalledInterruptAssociation {
            operation: Some(*psi_operation),
            reason,
        };
        let AbstractOperation::BoundaryCall {
            psi_operation: source_operation,
            boundary: source_boundary,
            arguments: scalar_arguments,
            structural_arguments,
            result: AbstractBoundaryResult::Scalar(result),
            ..
        } = source
        else {
            return Err(fail("installation changed its source operation or result"));
        };
        let declaration = plan
            .boundary_machines
            .iter()
            .find(|row| row.id == *boundary)
            .ok_or_else(|| fail("installation lost its retained boundary declaration"))?;
        if psi_operation != source_operation
            || boundary != source_boundary
            || !scalar_arguments.is_empty()
            || structural_arguments.as_slice() != arguments
            || unit.boundary_machine(*boundary) != Some(declaration)
            || declaration
                .declaration
                .as_ref()
                .and_then(|row| row.execution.static_interrupt_operation())
                != Some(language_core::static_interrupts::StaticInterruptOperation::Install)
            || result.scalar_type != semantic_vocabulary::ScalarType::Boolean
            || result_home.defining_operation != *psi_operation
            || result_home.source_value != result.value
            || result_home.scalar_type != result.scalar_type
            || Some(result_home.shape) != super::scalar_shape(result.scalar_type)
        {
            return Err(fail(
                "installation changed its exact operands or Boolean refusal result",
            ));
        }
        for (position, call) in calls.iter().enumerate() {
            let (role, source_position) = if position == 0 {
                (InstallationCallRole::Validate, 0)
            } else {
                (InstallationCallRole::Work, 5)
            };
            if call.role != role {
                return Err(fail(
                    "installation changed the order of validation and retained work",
                ));
            }
            let expected = super::callee_plan(call.callee, native, plan, unit)?;
            let signature = super::call_signature::signature(call.callee, plan)?;
            if signature.scalar_parameter_count() != 0
                || signature.structural_parameter_count() != 1
                || call.call_plan != expected
            {
                return Err(fail(
                    "installation helper changed its ordinary calling plan",
                ));
            }
            let mut actual = arguments[source_position].clone();
            actual.access = terminal_psi::StructuralAccess::SharedBorrow;
            let expected_argument = super::structural_call::argument_at(
                &actual,
                0,
                *psi_operation,
                caller,
                signature,
                &expected,
                native,
                plan,
                custody,
            )?;
            if call.argument != expected_argument {
                return Err(fail(
                    "installation helper changed its original table or context borrow",
                ));
            }
        }
        Ok(())
    }
}
pub(crate) mod normalized_foreign {
    use super::{
        AbstractOperationPlan, MachineId, PsiOptimizationFunction, PsiOptimizationUnit,
        TargetOperationPlan,
    };
    use crate::LegalizationError;
    use crate::structural_inputs::structural_reference_input::{
        borrowed_byte_view_presentation, projected_scalar_codec, reference_projection,
        runtime_borrowed_projection, runtime_projection, scalar_reference_export_width,
        scalar_storage_type,
    };
    use calling_conventions::{BoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape};
    use semantic_vocabulary::OperationId;
    use target_operations::{
        TargetNativeCallbackArgument, TargetStructuralArgument, TargetStructuralParameter,
        TargetUnitOperation,
    };

    pub(crate) fn result_at(
        result: &abstract_operations::AbstractBoundaryResult,
        declaration: &terminal_psi::BoundaryMachineResult,
        operation: OperationId,
        optimized: &PsiOptimizationFunction,
        plan: &AbstractOperationPlan,
        policy: calling_conventions::CallingPolicy,
    ) -> Result<(target_operations::TargetBoundaryResult, Option<ValueShape>), LegalizationError>
    {
        use target_operations::TargetBoundaryResult;
        match (result, declaration) {
            (
                abstract_operations::AbstractBoundaryResult::Unit,
                terminal_psi::BoundaryMachineResult::Unit,
            ) => Ok((TargetBoundaryResult::Unit, None)),
            (
                abstract_operations::AbstractBoundaryResult::Scalar(result),
                terminal_psi::BoundaryMachineResult::Scalar(declared),
            ) => {
                if result.scalar_type != declared.scalar_type
                    || super::value_type(optimized, result.value) != Some(result.scalar_type)
                {
                    return Err(LegalizationError::custody());
                }
                let shape =
                    super::scalar_shape(result.scalar_type).ok_or(LegalizationError::custody())?;
                Ok((
                    TargetBoundaryResult::Scalar(
                        target_operations::TargetUnitScalarHomeRequirement {
                            defining_operation: operation,
                            source_value: result.value,
                            scalar_type: result.scalar_type,
                            shape,
                        },
                    ),
                    Some(shape),
                ))
            }
            (
                abstract_operations::AbstractBoundaryResult::Structural(result),
                terminal_psi::BoundaryMachineResult::Structural(declared),
            ) => {
                if result.structural_type != declared.structural_type
                    || result.multiplicity != declared.multiplicity
                    || result.qualifications != declared.qualifications
                    || result.projected_qualifications != declared.projected_qualifications
                    || !result.projected_qualifications.is_empty()
                {
                    return Err(LegalizationError::custody());
                }
                let storage = crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &plan.structural_types,
                )
                .ok_or(LegalizationError::custody())?;
                let shape = calling_conventions::classified_boundary_shape(
                    result.structural_type,
                    &plan.structural_types,
                    policy,
                )
                .map_err(|_| LegalizationError::custody())?;
                Ok((
                    TargetBoundaryResult::Structural(
                        target_operations::TargetStructuralHomeRequirement {
                            origin:
                                target_operations::TargetStructuralHomeOrigin::OperationResult {
                                    operation,
                                    result: result.clone(),
                                },
                            layout: target_operations::TargetStructuralHomeLayout::Aggregate(
                                storage,
                            ),
                        },
                    ),
                    Some(shape),
                ))
            }
            _ => Err(LegalizationError::custody()),
        }
    }

    pub(crate) fn row(
        native: &TargetOperationPlan,
        machine: MachineId,
        operation: OperationId,
    ) -> Result<Option<&TargetUnitOperation>, LegalizationError> {
        let function = native
            .function(machine)
            .ok_or(LegalizationError::custody())?;
        super::unique_row(function, |row| {
            matches!(
                row,
                TargetUnitOperation::NormalizedForeignCall { psi_operation, .. }
                    if *psi_operation == operation
            )
        })
    }

    pub(crate) fn validate_native_callback_roster(
        native: &TargetOperationPlan,
    ) -> Result<(), LegalizationError> {
        for (index, callback) in native.native_callback_arguments.iter().enumerate() {
            if native.native_callback_arguments[..index]
                .iter()
                .any(|prior| {
                    prior.terminal_operation == callback.terminal_operation
                        && prior.placement_index == callback.placement_index
                })
            {
                return Err(LegalizationError::custody());
            }
            let mut row = None;
            for function in &native.functions {
                if let Some(found) = super::unique_row(function, |row| {
                    matches!(
                        row,
                        TargetUnitOperation::NormalizedForeignCall { psi_operation, .. }
                            if *psi_operation == callback.terminal_operation
                    )
                })? && row.replace(found).is_some()
                {
                    return Err(LegalizationError::custody());
                }
            }
            let Some(TargetUnitOperation::NormalizedForeignCall { binding, .. }) = row else {
                return Err(LegalizationError::custody());
            };
            if binding.boundary_entry_plan != callback.registrar_boundary_entry_plan {
                return Err(LegalizationError::custody());
            }
        }
        Ok(())
    }

    pub(crate) fn native_callbacks_at<'a>(
        native: &'a TargetOperationPlan,
        operation: OperationId,
        boundary_entry_plan: &BoundaryEntryPlan,
    ) -> Result<Vec<&'a TargetNativeCallbackArgument>, LegalizationError> {
        let materializations = &boundary_entry_plan.call.callback_materializations;
        let matching: Vec<&'a TargetNativeCallbackArgument> = native
            .native_callback_arguments
            .iter()
            .filter(|callback| callback.terminal_operation == operation)
            .collect();
        if materializations.is_empty() {
            return if matching.is_empty() {
                Ok(Vec::new())
            } else {
                Err(LegalizationError::custody())
            };
        }
        if matching.len() != materializations.len() {
            return Err(LegalizationError::custody());
        }
        let pointer_size =
            u16::try_from(native.target.pointer_size).map_err(|_| LegalizationError::custody())?;
        let pointer_alignment = u16::try_from(native.target.pointer_alignment)
            .map_err(|_| LegalizationError::custody())?;
        for (index, callback) in matching.iter().enumerate() {
            let ordinal = usize::try_from(callback.application.native_ordinal)
                .map_err(|_| LegalizationError::custody())?;
            let access_root = boundary_entry_plan.call.parameters.get(ordinal)
                == Some(&callback.application.placement);
            let destination_admitted = match &callback.destination {
                calling_conventions::NativePlace::Parameter(parameter) => {
                    *parameter == callback.application.parameter
                        && callback.field_placements.is_none()
                        && access_root
                }
                calling_conventions::NativePlace::Field { parameter, .. } => {
                    *parameter == callback.application.parameter
                        && callback.field_placements.is_some()
                        && access_root
                }
            };
            if callback.registrar_boundary_entry_plan != *boundary_entry_plan
                || !callback.callback_function.is_valid()
                || callback.callback_function.callback_thunk_placement_index()
                    != Some(callback.placement_index)
                || callback.application.shape != callback.application.placement.shape
                || callback.application.shape
                    != ValueShape::integer(u32::from(pointer_size), pointer_alignment)
                || !destination_admitted
                || !materializations
                    .iter()
                    .any(|materialization| materialization.destination == callback.destination)
                || matching[..index].iter().any(|prior| {
                    prior.placement_index == callback.placement_index
                        || prior.destination == callback.destination
                })
            {
                return Err(LegalizationError::custody());
            }
        }
        if matching
            .iter()
            .filter(|callback| {
                matches!(
                    callback.destination,
                    calling_conventions::NativePlace::Parameter(_)
                )
            })
            .count()
            > 1
        {
            return Err(LegalizationError::custody());
        }
        Ok(matching)
    }

    pub(crate) fn structural_argument_at(
        semantic: &terminal_psi::StructuralArgument,
        position: usize,
        declaration_parameter: &terminal_psi::StructuralParameterDeclaration,
        plan_row: Option<&ValuePlacement>,
        parameters: &[TargetStructuralParameter],
        optimized: &PsiOptimizationFunction,
        native: ::target::NativeTarget,
        policy: calling_conventions::CallingPolicy,
        plan: &AbstractOperationPlan,
        call_operation: OperationId,
        target_plan: &TargetOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<TargetStructuralArgument, LegalizationError> {
        use terminal_psi::{StructuralAccess, StructuralPathSegment};
        let declarations = plan.structural_types.as_slice();
        let (root_type, root_access, root_multiplicity, source, root, presentation_debt) =
            if let Some(caller_parameter) = parameters
                .iter()
                .find(|parameter| parameter.place == semantic.place)
            {
                let semantic_parameter = optimized
                    .structural_parameters
                    .iter()
                    .chain(optimized.block_structural_parameters())
                    .find(|parameter| parameter.place == semantic.place)
                    .ok_or(LegalizationError::custody())?;
                if caller_parameter.structural_type != semantic_parameter.structural_type
                    || caller_parameter.access != semantic_parameter.access
                    || caller_parameter.multiplicity != semantic_parameter.multiplicity
                    || caller_parameter.projected_qualifications
                        != semantic_parameter.projected_qualifications
                    || caller_parameter.shape
                        != crate::structural_inputs::parameter_shape(
                            semantic_parameter,
                            declarations,
                        )
                        .ok_or(LegalizationError::custody())?
                {
                    return Err(LegalizationError::custody());
                }
                (
                    caller_parameter.structural_type,
                    semantic_parameter.access,
                    semantic_parameter.multiplicity,
                    caller_parameter.placement.clone().into(),
                    semantic_parameter.clone(),
                    false,
                )
            } else if let Some((block, parameter)) =
                super::aggregate_results::borrowed_block_arrival(optimized, semantic.place, plan)
            {
                let call_block = super::aggregate_results::node_site(optimized, |operation| {
                    operation.psi_operation() == Some(call_operation)
                })
                .ok_or(LegalizationError::custody())?
                .0;
                let permitted_access = match parameter.access {
                    StructuralAccess::SharedBorrow => {
                        semantic.access == StructuralAccess::SharedBorrow
                    }
                    StructuralAccess::MutableBorrow => {
                        semantic.access != StructuralAccess::Owned || !semantic.path.is_empty()
                    }
                    StructuralAccess::WriteOnlyBorrow => {
                        semantic.access == StructuralAccess::WriteOnlyBorrow
                    }
                    StructuralAccess::Owned => false,
                };
                if !permitted_access
                    || !super::target::control_flow::sources::dominates(
                        optimized, block, call_block,
                    )
                {
                    return Err(LegalizationError::custody());
                }
                (
                    parameter.structural_type,
                    parameter.access,
                    parameter.multiplicity,
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place: parameter.place,
                    },
                    parameter.clone(),
                    false,
                )
            } else {
                let home = super::aggregate_results::result_home(optimized, semantic.place, plan)?;
                if home.access() != terminal_psi::StructuralAccess::Owned {
                    return Err(LegalizationError::custody());
                }
                let site = |operation| {
                    optimized.blocks.iter().find_map(|block| {
                        block
                            .nodes
                            .iter()
                            .position(|node| node.operation.psi_operation() == Some(operation))
                            .map(|position| (block.id, position))
                    })
                };
                let after = site(call_operation).ok_or(LegalizationError::custody())?;
                let (before, source) = match home.origin {
                    target_operations::TargetStructuralHomeOrigin::OperationResult {
                        operation,
                        ..
                    } => (
                        site(operation).ok_or(LegalizationError::custody())?,
                        target_operations::TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: operation,
                        },
                    ),
                    target_operations::TargetStructuralHomeOrigin::BlockParameter {
                        block,
                        ref declaration,
                    } => (
                        (block, 0),
                        target_operations::TargetStructuralArgumentSource::BlockParameter {
                            block,
                            place: declaration.place,
                        },
                    ),
                };
                if (before.0 == after.0 && home.operation_result().is_some() && before.1 >= after.1)
                    || (before.0 != after.0
                        && !super::target::control_flow::sources::dominates(
                            optimized, before.0, after.0,
                        ))
                {
                    return Err(LegalizationError::custody());
                }
                (
                    home.structural_type(),
                    home.access(),
                    home.multiplicity(),
                    source,
                    terminal_psi::StructuralParameterDeclaration {
                        place: home.place(),
                        position: declaration_parameter.position,
                        structural_type: home.structural_type(),
                        access: home.access(),
                        multiplicity: home.multiplicity(),
                        qualifications: home.qualifications().to_vec(),
                        projected_qualifications: home.projected_qualifications().to_vec(),
                        additional_input_qualifications: home
                            .additional_input_qualifications()
                            .to_vec(),
                    },
                    home.has_claims()
                        || home.operation_result().is_some_and(|(_, result)| {
                            !result.qualification_establishments.is_empty()
                        }),
                )
            };
        let pointer_size =
            u16::try_from(native.pointer_size).map_err(|_| LegalizationError::custody())?;
        let pointer_alignment =
            u16::try_from(native.pointer_alignment).map_err(|_| LegalizationError::custody())?;
        let presentation = (semantic.access != StructuralAccess::Owned)
            .then(|| {
                borrowed_byte_view_presentation(
                    &root,
                    semantic,
                    declaration_parameter.structural_type,
                    declarations,
                )
            })
            .flatten();
        let (projected_type, source_byte_offset, elements) =
            if let Some((offset, _, _)) = presentation {
                if presentation_debt {
                    return Err(LegalizationError::custody());
                }
                (declaration_parameter.structural_type, offset, Vec::new())
            } else if semantic.path.contains(&StructuralPathSegment::Referent) {
                let projection = reference_projection(
                    root_type,
                    &semantic.path,
                    semantic.access,
                    Some(declaration_parameter.structural_type),
                    declarations,
                )
                .ok_or(LegalizationError::custody())?;
                if projection.scalar_descriptor {
                    let shape = crate::structural_inputs::shape(projection.selected, declarations)
                        .ok_or(LegalizationError::custody())?;
                    let exported_width = scalar_reference_export_width(
                        projection.selected,
                        semantic.access,
                        declarations,
                    );
                    if exported_width != Some(shape.byte_size) {
                        validate_scalar_backing_width(
                            semantic,
                            shape.byte_size,
                            optimized,
                            plan,
                            call_operation,
                            target_plan,
                            unit,
                        )
                        .map_err(|reason| {
                            LegalizationError::InvalidScalarReferenceBacking {
                                machine: optimized.machine,
                                operation: call_operation,
                                reason,
                            }
                        })?;
                    }
                }
                (
                    projection.selected,
                    projection.referent_offset,
                    projection.elements,
                )
            } else if semantic.access != StructuralAccess::Owned {
                runtime_borrowed_projection(
                    root_type,
                    &semantic.path,
                    semantic.access,
                    Some(declaration_parameter.structural_type),
                    declarations,
                )
                .ok_or(LegalizationError::custody())?
            } else {
                runtime_projection(root_type, &semantic.path, declarations)
                    .ok_or(LegalizationError::custody())?
            };
        let root_shape =
            crate::structural_inputs::structural_reference_input::shape(root_type, declarations)
                .ok_or(LegalizationError::custody())?;
        let projected_shape = crate::structural_inputs::structural_reference_input::shape(
            projected_type,
            declarations,
        )
        .ok_or(LegalizationError::custody())?;
        let destination = plan_row.ok_or(LegalizationError::custody())?;
        let (shape, destination_shape, multiplicity) = match semantic.access {
            StructuralAccess::Owned => {
                let admitted_root = if semantic.path.is_empty() {
                    root_access == StructuralAccess::Owned
                } else {
                    matches!(root_access, terminal_psi::mutating_access_pair_pattern!())
                };
                if !admitted_root {
                    return Err(LegalizationError::custody());
                }
                let classified = calling_conventions::classified_boundary_shape(
                    projected_type,
                    declarations,
                    policy,
                )
                .map_err(|_| LegalizationError::custody())?;
                let multiplicity = if semantic.path.is_empty() {
                    root_multiplicity
                } else if declaration_parameter.multiplicity
                    == terminal_psi::StructuralMultiplicity::Linear
                {
                    terminal_psi::StructuralMultiplicity::Linear
                } else {
                    terminal_psi::StructuralMultiplicity::Affine
                };
                (projected_shape, classified, multiplicity)
            }
            StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow => {
                let descriptor = declarations.iter().any(|declaration| {
                    declaration.id == projected_type
                        && declaration.shape.is_borrowed_view_byte_sequence()
                });
                let destination_shape = if descriptor {
                    calling_conventions::classified_boundary_shape(
                        projected_type,
                        declarations,
                        policy,
                    )
                    .map_err(|_| LegalizationError::custody())?
                } else {
                    if !matches!(destination.locations.as_slice(),
                    [ValueLocation::Register { value_byte_offset: 0, byte_size, .. }]
                    | [ValueLocation::Stack { value_byte_offset: 0, byte_size, .. }]
                        if *byte_size == u32::from(pointer_size))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ValueShape::integer(u32::from(pointer_size), pointer_alignment)
                };
                (
                    ValueShape::borrowed_reference(
                        projected_shape.byte_size,
                        projected_shape.alignment,
                    ),
                    destination_shape,
                    terminal_psi::StructuralMultiplicity::Unrestricted,
                )
            }
        };
        let projects_view_backing = semantic.access != StructuralAccess::Owned
            && !semantic.path.is_empty()
            && declarations.iter().any(|row| {
                row.id == root_type
                    && (row.shape.is_borrowed_byte_carrier()
                        || matches!(
                            row.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                        ))
            });
        if (presentation.is_none()
            && semantic.path.iter().any(|segment| match segment {
                StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_) => false,
                StructuralPathSegment::RuntimeIndex { .. } => {
                    semantic.access == StructuralAccess::Owned
                }
                StructuralPathSegment::Referent => semantic.access == StructuralAccess::Owned,
                _ => true,
            }))
            || usize::try_from(declaration_parameter.position).ok() != Some(position)
            || projected_type != declaration_parameter.structural_type
            || semantic.access != declaration_parameter.access
            || declaration_parameter.multiplicity != multiplicity
            || (presentation.is_none()
                && !projects_view_backing
                && !semantic.path.contains(&StructuralPathSegment::Referent)
                && projected_shape
                    .byte_size
                    .checked_add(source_byte_offset)
                    .is_none_or(|end| end > root_shape.byte_size))
            || destination.shape != destination_shape
        {
            return Err(LegalizationError::custody());
        }
        Ok(TargetStructuralArgument {
            place: semantic.place,
            access: semantic.access,
            path: semantic.path.clone(),
            root_structural_type: root_type,
            structural_type: projected_type,
            shape,
            source_byte_offset,
            fixed_array_length: presentation.and_then(|(_, length, _)| length),
            element_stride: presentation.and_then(|(_, _, stride)| stride),
            indices: super::target::control_flow::sources::call_indices(
                target_plan,
                optimized,
                target_operations::CallSiteOwner::Operation(call_operation),
                &elements,
            )?,
            source,
            destination: destination.clone(),
        })
    }

    fn validate_scalar_backing_width(
        argument: &terminal_psi::StructuralArgument,
        width: u32,
        function: &PsiOptimizationFunction,
        plan: &AbstractOperationPlan,
        operation: OperationId,
        native: &TargetOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<(), &'static str> {
        use std::borrow::Cow;
        use terminal_semantics::descriptor_lifetimes::{
            descriptor_location, descriptor_path, fixed_runtime_index_alternatives,
        };
        use terminal_semantics::descriptor_results::{DescriptorAnalysis, descriptor_machine};
        let types = &plan.structural_types;
        let graph =
            optimization_unit_semantics::descriptor_results::optimized_reference_catalog_graph(
                &unit.functions,
                &unit.boundary_machines,
                types,
                &unit.dynamic_dispatch,
                |caller, node| {
                    super::call_origin::selected_boundary_operation(node, caller, native, plan)
                        .ok()
                        .map(|selected| match selected {
                            Some((operation, _)) => Cow::Owned(operation),
                            None => Cow::Borrowed(&node.operation),
                        })
                },
            )
            .ok_or("reference catalog graph could not be reconstructed")?;
        let current = descriptor_machine(&graph, function.machine)
            .ok_or("reference catalog has no current machine")?;
        let position = function
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .nodes
                    .iter()
                    .position(|node| node.operation.psi_operation() == Some(operation))
                    .and_then(|index| Some((block.id, u32::try_from(index).ok()?)))
            })
            .ok_or("foreign call has no optimized operation position")?;
        let root = *current
            .place_types
            .get(&argument.place)
            .ok_or("foreign argument has no retained root type")?;
        let locations = match descriptor_location(types, argument.place, root, &argument.path, None)
        {
            Some(location) => vec![location],
            None => fixed_runtime_index_alternatives(types, argument.place, root, &argument.path)
                .ok_or("foreign argument has no descriptor coordinates")?,
        };
        let mut analysis = DescriptorAnalysis::with_reference_slots(types, &graph);
        let mut origins = Vec::new();
        for location in locations {
            origins.extend(
                analysis
                    .storage_origins(function.machine, position, &location)
                    .ok_or("foreign argument physical origins could not be reconstructed")?,
            );
        }
        if origins.is_empty() {
            return Err("foreign argument has no physical backing origins");
        }
        for origin in origins {
            let root = *current
                .place_types
                .get(&origin.root)
                .ok_or("physical backing has no retained root type")?;
            let path = descriptor_path(types, root, &origin.path)
                .ok_or("physical backing has no structural descriptor path")?;
            if path.contains(&terminal_psi::StructuralPathSegment::Referent)
                || function
                    .structural_parameter(origin.root)
                    .is_some_and(|parameter| {
                        parameter.access != terminal_psi::StructuralAccess::Owned
                            && scalar_storage_type(root, types).is_some()
                    })
            {
                return Err("physical backing remains an external descriptor");
            }
            let codec = projected_scalar_codec(root, &path, types)
                .ok_or("physical backing has no scalar storage codec")?;
            if u32::from(codec.byte_size()) != width {
                return Err("physical backing width differs from the exported referent");
            }
        }
        Ok(())
    }
}
pub(crate) mod place_declarations {
    use super::{AbstractOperation, PsiOptimizationFunction};
    use semantic_vocabulary::StructuralPlaceKind;

    pub(crate) fn matches_producers(function: &PsiOptimizationFunction) -> bool {
        function.declared_places
            == function
                .structural_places
                .iter()
                .filter(|place| {
                    !matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. })
                })
                .map(|place| place.id)
                .collect::<std::collections::BTreeSet<_>>()
            && function.structural_places.iter().all(|place| {
                if let StructuralPlaceKind::ProviderAttachment { attachment, .. } = place.kind {
                    return function.attachment == Some(attachment);
                }
                if matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. }) {
                    return super::literals::declaration_producer(function, place.id).is_some();
                }
                if let StructuralPlaceKind::TrivialAffineLocal {
                    structural_type, ..
                } = place.kind
                {
                    return function
                        .block_nodes()
                        .filter(|node| {
                            matches!(&node.operation,
                                AbstractOperation::EstablishTrivialAffineLocal {
                                    place: established,
                                    structural_type: declaration,
                                    ..
                                } if *established == *place && declaration.id == structural_type)
                        })
                        .count()
                        == 1;
                }
                if let Some((producer, result, _)) =
                    super::primitive_locals::producer(function, place.id)
                {
                    return place.kind
                        == StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        }
                        && super::primitive_locals::valid_result(function, producer, result);
                }
                if place.kind == StructuralPlaceKind::Result {
                    return function
                        .result
                        .structural()
                        .is_some_and(|result| result.place == place.id);
                }
                if let Ok((producer, result)) =
                    super::structural_case::source_result(function, place.id)
                {
                    return place.kind
                        == StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        };
                }
                function.structural_parameters.iter().any(|parameter| {
                    parameter.place == place.id
                        && place.kind
                            == StructuralPlaceKind::Parameter {
                                position: parameter.position,
                            }
                }) || function.blocks.iter().any(|block| {
                    block.structural_parameters.iter().any(|parameter| {
                        parameter.place == place.id
                            && place.kind
                                == StructuralPlaceKind::BlockParameter {
                                    block: block.id,
                                    position: parameter.position,
                                }
                    })
                })
            })
    }
}
pub(crate) mod read_byte {
    use super::{
        AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit,
        ScalarType, ValueShape,
    };
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::i32_type;
    use calling_conventions::ConventionalSumLayout;
    use target_operations::{
        BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution,
        TargetBoundaryResult, TargetUnitOperation,
    };
    use terminal_psi::{
        StructuralMultiplicity, StructuralOperationResult, TerminalAffineCleanupAction,
    };

    pub(crate) fn layout(
        result: &StructuralOperationResult,
        plan: &AbstractOperationPlan,
    ) -> Result<ConventionalSumLayout, LegalizationError> {
        if result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        let mut declarations = plan
            .structural_types
            .iter()
            .filter(|row| row.id == result.structural_type);
        let declaration = declarations.next().ok_or(LegalizationError::custody())?;
        let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
            return Err(LegalizationError::custody());
        };
        let [empty, byte] = cases.as_slice() else {
            return Err(LegalizationError::custody());
        };
        let [field] = byte.fields.as_slice() else {
            return Err(LegalizationError::custody());
        };
        if declarations.next().is_some()
            || !empty.fields.is_empty()
            || field.relevance.is_erased()
            || !match field.field_type {
                terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                    integer == i32_type()
                }
                terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
                    bounds.integer_type() == i32_type()
                        && bounds.contains(semantic_vocabulary::IntegerValue::Signed(0))
                        && bounds.contains(semantic_vocabulary::IntegerValue::Signed(255))
                }
                _ => false,
            }
        {
            return Err(LegalizationError::custody());
        }
        calling_conventions::evaluate_conventional_sum_layout(
            &[],
            &[vec![], vec![ValueShape::integer(4, 4)]],
        )
        .map_err(|_| LegalizationError::custody())
    }

    pub(crate) fn validate(
        target: &TargetUnitOperation,
        source: &AbstractOperation,
        native: ::target::NativeTarget,
        plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<(), LegalizationError> {
        let (
            TargetUnitOperation::BoundarySettlement {
                psi_operation,
                boundary,
                result: TargetBoundaryResult::Structural(home),
                execution:
                    BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::HostedReadByte),
                realization: BoundaryRealization::HostedReadByte(_),
                scalar_arguments,
                runtime_scalar_arguments,
                arguments,
                byte_sequence_arguments,
                completion_claim_sources,
                completion_receipts,
            },
            AbstractOperation::BoundaryCall {
                psi_operation: expected_operation,
                boundary: expected_boundary,
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                arguments: expected_arguments,
                structural_arguments,
                completion_claim_sources: expected_claims,
                completion_receipts: expected_receipts,
            },
        ) = (target, source)
        else {
            return Err(LegalizationError::custody());
        };
        let mut declarations = plan
            .boundary_machines
            .iter()
            .filter(|row| row.id == *boundary);
        let declaration = declarations.next().ok_or(LegalizationError::custody())?;
        let terminal_psi::BoundaryMachineResult::Structural(signature) = &declaration.result else {
            return Err(LegalizationError::custody());
        };
        if declarations.next().is_some()
            || ![
                ::target::NativeTarget::linux_x64(),
                ::target::NativeTarget::macos_x64(),
                ::target::NativeTarget::linux_arm64(),
                ::target::NativeTarget::macos_arm64(),
            ]
            .contains(&native)
            || unit.boundary_machine(*boundary) != Some(declaration)
            || unit.structural_types != plan.structural_types
            || psi_operation != expected_operation
            || boundary != expected_boundary
            || home.operation_result() != Some((*expected_operation, result))
            || home.layout.sum() != Some(&layout(result, plan)?)
            || signature.structural_type != result.structural_type
            || signature.multiplicity != result.multiplicity
            || signature.qualifications != result.qualifications
            || signature.projected_qualifications != result.projected_qualifications
            || !declaration.scalar_parameters.is_empty()
            || !declaration.structural_parameters.is_empty()
            || !declaration.requires.is_empty()
            || !declaration.program_local_root_introductions.is_empty()
            || !declaration.content_guarantees.is_empty()
            || !scalar_arguments.is_empty()
            || !runtime_scalar_arguments.is_empty()
            || !arguments.is_empty()
            || !byte_sequence_arguments.is_empty()
            || !completion_claim_sources.is_empty()
            || !completion_receipts.is_empty()
            || !expected_arguments.is_empty()
            || !structural_arguments.is_empty()
            || !expected_claims.is_empty()
            || !expected_receipts.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        Ok(())
    }

    pub(crate) fn cleanup(
        function: &PsiOptimizationFunction,
        actions: &[TerminalAffineCleanupAction],
    ) -> bool {
        if !super::place_declarations::matches_producers(function) {
            return false;
        }
        let results = function
            .block_nodes()
            .filter_map(|node| match &node.operation {
                AbstractOperation::BoundaryCall {
                    psi_operation,
                    result: abstract_operations::AbstractBoundaryResult::Structural(result),
                    ..
                } if result.multiplicity == StructuralMultiplicity::Affine
                    && result.claims.is_empty() =>
                {
                    Some((*psi_operation, result.place))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut discarded = std::collections::BTreeSet::new();
        actions.iter().all(|action| {
            let TerminalAffineCleanupAction::DiscardRoot(place) = action else {
                return false;
            };
            discarded.insert(*place)
                && results.iter().filter(|(_, result)| result == place).count() == 1
        })
    }
}
pub(crate) mod scalar_arrays {
    use super::{AbstractOperationPlan, ScalarType, ValueShape, scalar_shape};
    use crate::LegalizationError;

    pub(crate) fn shape(
        result: &terminal_psi::StructuralOperationResult,
        plan: &AbstractOperationPlan,
    ) -> Result<(ScalarType, u64, ValueShape), LegalizationError> {
        if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !result.claims.is_empty()
            || !result.projected_qualifications.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        let mut current = result.structural_type;
        let mut count = Some(1_u64);
        let mut empty = false;
        let mut array = false;
        for _ in 0..plan.structural_types.len() {
            let mut matching = plan
                .structural_types
                .iter()
                .filter(|declaration| declaration.id == current);
            let declaration = matching.next().ok_or(LegalizationError::custody())?;
            if matching.next().is_some() {
                return Err(LegalizationError::custody());
            }
            match declaration.shape {
                terminal_psi::StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    array = true;
                    empty |= length == 0;
                    count = count.and_then(|count| count.checked_mul(length));
                    current = element;
                }
                terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) if array => {
                    let leaf = scalar_shape(scalar_type).ok_or(LegalizationError::custody())?;
                    let count = if empty {
                        0
                    } else {
                        count.ok_or(LegalizationError::custody())?
                    };
                    let size = count
                        .checked_mul(u64::from(leaf.byte_size))
                        .and_then(|size| u32::try_from(size).ok())
                        .ok_or(LegalizationError::custody())?;
                    return Ok((
                        scalar_type,
                        count,
                        ValueShape::integer(size, if size == 0 { 1 } else { leaf.alignment }),
                    ));
                }
                _ => return Err(LegalizationError::custody()),
            }
        }
        Err(LegalizationError::custody())
    }
}
pub(crate) mod structural_case {
    use super::{
        AbstractOperation, AbstractOperationPlan, OptimizationNode, PsiOptimizationFunction,
        PsiProvenance,
    };
    use crate::LegalizationError;
    use crate::structural_inputs::structural_reference_input::{
        RuntimeElement, case_projection, leaf_copy_geometry, reference_source_geometry,
        runtime_projection,
    };
    use semantic_vocabulary::{
        BlockId, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralPlaceKind,
        StructuralTypeId,
    };
    use terminal_psi::{
        StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape, find_structural_type,
    };

    pub(crate) fn source_result(
        function: &PsiOptimizationFunction,
        source: PlaceId,
    ) -> Result<(OperationId, &StructuralOperationResult), LegalizationError> {
        let mut matching = function.block_nodes().filter_map(|node| {
            let (producer, result) = node.operation.structural_result()?;
            (result.place == source).then_some((producer, result))
        });
        let result = matching.next().ok_or(LegalizationError::custody())?;
        if matching.next().is_some() {
            return Err(LegalizationError::custody());
        }
        Ok(result)
    }

    pub(crate) fn records_operation_result(
        function: &PsiOptimizationFunction,
        producer: OperationId,
        result: &StructuralOperationResult,
    ) -> bool {
        function
            .structural_place(result.place)
            .is_some_and(|place| {
                place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    }
            })
    }

    pub(crate) fn block_parameter_declared(
        function: &PsiOptimizationFunction,
        block: BlockId,
        declaration: &StructuralParameterDeclaration,
    ) -> bool {
        function
            .structural_place(declaration.place)
            .is_some_and(|place| {
                place.kind
                    == StructuralPlaceKind::BlockParameter {
                        block,
                        position: declaration.position,
                    }
            })
    }

    pub(crate) fn arrivals<'a>(
        function: &'a PsiOptimizationFunction,
        place: PlaceId,
    ) -> impl Iterator<Item = (BlockId, &'a StructuralParameterDeclaration)> + 'a {
        function.blocks.iter().flat_map(move |block| {
            block
                .structural_parameters
                .iter()
                .filter(move |parameter| parameter.place == place)
                .map(move |parameter| (block.id, parameter))
        })
    }

    pub(crate) fn source_owner(
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> Result<legalized_operations::LegalizedStructuralCaseSource, LegalizationError> {
        if let Ok((operation, result)) = source_result(function, place) {
            return Ok(
                legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                    operation,
                    result: result.clone(),
                },
            );
        }
        let mut parameters = arrivals(function, place);
        let (block, declaration) = parameters.next().ok_or(LegalizationError::custody())?;
        if parameters.next().is_some()
            || declaration.access != terminal_psi::StructuralAccess::Owned
        {
            return Err(LegalizationError::custody());
        }
        Ok(
            legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                block,
                declaration: declaration.clone(),
            },
        )
    }

    pub(crate) fn case_source(
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> Result<legalized_operations::LegalizedStructuralCaseSource, LegalizationError> {
        if let Ok(owner) = source_owner(function, place) {
            return Ok(owner);
        }
        let mut parameters = function
            .structural_parameters
            .iter()
            .filter(|parameter| parameter.place == place);
        if let Some(declaration) = parameters.next() {
            if parameters.next().is_some() {
                return Err(LegalizationError::custody());
            }
            return match declaration.access {
                terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow => Ok(
                    legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                        declaration: declaration.clone(),
                    },
                ),
                terminal_psi::StructuralAccess::Owned => Ok(
                    legalized_operations::LegalizedStructuralCaseSource::Parameter {
                        declaration: declaration.clone(),
                    },
                ),
                _ => Err(LegalizationError::custody()),
            };
        }
        let mut parameters = arrivals(function, place).map(|(_, parameter)| parameter);
        let declaration = parameters.next().ok_or(LegalizationError::custody())?;
        if parameters.next().is_some()
            || !matches!(
                declaration.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
        {
            return Err(LegalizationError::custody());
        }
        Ok(
            legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                declaration: declaration.clone(),
            },
        )
    }

    fn source_identity(
        function: &PsiOptimizationFunction,
        source: PlaceId,
        result: Result<(OperationId, &StructuralOperationResult), LegalizationError>,
    ) -> Result<semantic_vocabulary::StructuralTypeId, LegalizationError> {
        if let Ok((_, result)) = result {
            return Ok(result.structural_type);
        }
        if let Some((_, identity)) = super::literals::declaration_producer(function, source) {
            return Ok(identity);
        }
        let parameter = function
            .structural_parameters
            .iter()
            .chain(function.block_structural_parameters())
            .find(|parameter| parameter.place == source)
            .ok_or(LegalizationError::custody())?;
        if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
            return Err(LegalizationError::custody());
        }
        Ok(parameter.structural_type)
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn dispatch_projection(
        function: &PsiOptimizationFunction,
        source: PlaceId,
        path: &[StructuralPathSegment],
        plan: &AbstractOperationPlan,
    ) -> Result<
        (
            StructuralTypeId,
            u32,
            calling_conventions::ValueShape,
            calling_conventions::ConventionalSumLayout,
        ),
        LegalizationError,
    > {
        let root = source_identity(function, source, source_result(function, source))?;
        let geometry = reference_source_geometry(root, path, &plan.structural_types)
            .ok_or(LegalizationError::custody())?;
        if !geometry.elements.is_empty()
            || !geometry.readable
            || geometry
                .pointer_slots
                .iter()
                .any(|slot| slot.scalar_descriptor)
        {
            return Err(LegalizationError::custody());
        }
        let selected = geometry.selected;
        let byte_offset = geometry.byte_offset;
        let root_shape = super::aggregate_results::type_home_layout(root, plan)?.shape();
        let region_shape =
            super::aggregate_results::type_home_layout(geometry.region_root, plan)?.shape();
        let layout = super::aggregate_results::sum_type_layout(selected, plan)?;
        if byte_offset
            .checked_add(layout.shape.byte_size)
            .is_none_or(|end| end > region_shape.byte_size)
        {
            return Err(LegalizationError::custody());
        }
        Ok((selected, byte_offset, root_shape, layout))
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn membership_layout(
        function: &PsiOptimizationFunction,
        source: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        case: semantic_vocabulary::StructuralCaseId,
        plan: &AbstractOperationPlan,
        result: Result<(OperationId, &StructuralOperationResult), LegalizationError>,
    ) -> Result<(u32, u32, Vec<RuntimeElement>), LegalizationError> {
        let identity = source_identity(function, source, result)?;
        let geometry = reference_source_geometry(identity, path, &plan.structural_types)
            .ok_or(LegalizationError::custody())?;
        if !geometry.readable
            || geometry
                .pointer_slots
                .iter()
                .any(|slot| slot.scalar_descriptor)
        {
            return Err(LegalizationError::custody());
        }
        let identity = geometry.selected;
        let byte_offset = geometry.byte_offset;
        let layout = super::aggregate_results::sum_type_layout(identity, plan)?;
        if layout.tag_byte_offset != 0
            || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
        {
            return Err(LegalizationError::custody());
        }
        let declaration = find_structural_type(plan.structural_types.as_slice(), identity)
            .ok_or(LegalizationError::custody())?;
        let cases: &[terminal_psi::StructuralCaseDeclaration] = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Sum { cases }
            | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
            _ => return Err(LegalizationError::custody()),
        };
        cases
            .iter()
            .position(|candidate| candidate.id == case)
            .and_then(|ordinal| u32::try_from(ordinal).ok())
            .map(|tag| (tag, byte_offset, geometry.elements))
            .ok_or(LegalizationError::custody())
    }

    pub(crate) fn structural_transport_layout(
        function: &PsiOptimizationFunction,
        operation: OperationId,
        source: PlaceId,
        path: &abstract_operations::StructuralCopyPath,
        result: &StructuralOperationResult,
        plan: &AbstractOperationPlan,
    ) -> Result<(u32, calling_conventions::ValueShape, Vec<RuntimeElement>), LegalizationError>
    {
        let identity = source_identity(function, source, source_result(function, source))?;
        if !result.qualification_establishments.is_empty() {
            return Err(LegalizationError::custody());
        }
        let (endpoint, byte_offset, indices) = match path {
            abstract_operations::StructuralCopyPath::Case(path) => {
                let geometry = case_projection(identity, path, &plan.structural_types)
                    .ok_or(LegalizationError::custody())?;
                (geometry.selected, geometry.byte_offset, geometry.elements)
            }
            abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty() => {
                (identity, 0, Vec::new())
            }
            abstract_operations::StructuralCopyPath::Ordinary(path) => {
                let geometry = leaf_copy_geometry(
                    identity,
                    path,
                    result.structural_type,
                    &plan.structural_types,
                )
                .filter(|geometry| geometry.readable)
                .ok_or(LegalizationError::custody())?;
                (geometry.selected, geometry.byte_offset, geometry.elements)
            }
        };
        let byte_carrier = endpoint != result.structural_type
            && byte_carrier_leaf_copy(
                &plan.structural_types,
                identity,
                path,
                result.structural_type,
            );
        let copy_type = if byte_carrier {
            result.structural_type
        } else {
            endpoint
        };
        let quotient = matches!(path, abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty())
            && abstract_operations::quotients::licensed_copy(
                &plan.structural_types,
                function.machine,
                operation,
                source,
                identity,
                result,
                function.block_nodes().map(|node| &node.operation),
            );
        let occurrence_access = match path {
            abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty() => function
                .arrival_parameter(source)
                .map(|parameter| parameter.access)
                .or_else(|| {
                    function.block_nodes().find_map(|node| {
                        let declaration = node.operation.structural_storage()?;
                        (declaration.place == source).then(|| {
                            terminal_semantics::descriptor_results::descriptor_value_access(
                                plan.structural_types.iter(),
                                declaration.structural_type,
                                declaration.multiplicity,
                            )
                            .unwrap_or(declaration.access)
                        })
                    })
                }),
            _ => None,
        };
        if endpoint != result.structural_type
            && !byte_carrier
            && !quotient
            && !terminal_semantics::shared_descriptor_copy_matches(
                plan.structural_types.iter(),
                endpoint,
                occurrence_access,
                result.structural_type,
                result.multiplicity,
            )
        {
            return Err(LegalizationError::custody());
        }
        for element in &indices {
            crate::legalization::runtime_indices::defined_type(function, element.index)
                .filter(|scalar| {
                    matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer)
                        if integer.bits() <= 64)
                })
                .ok_or(LegalizationError::custody())?;
        }
        let shape = crate::structural_inputs::shape(copy_type, &plan.structural_types)
            .ok_or(LegalizationError::custody())?;
        if quotient
            && crate::structural_inputs::shape(result.structural_type, &plan.structural_types)
                != Some(shape)
        {
            return Err(LegalizationError::custody());
        }
        Ok((byte_offset, shape, indices))
    }

    fn byte_carrier_leaf_copy(
        declarations: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &abstract_operations::StructuralCopyPath,
        expected: StructuralTypeId,
    ) -> bool {
        let abstract_operations::StructuralCopyPath::Ordinary(path) = path else {
            return false;
        };
        let Some((StructuralPathSegment::FixedIndex(index), prefix)) = path.split_last() else {
            return false;
        };
        let Some((parent, _, _)) = runtime_projection(root, prefix, declarations) else {
            return false;
        };
        let Some(declaration) = find_structural_type(declarations, parent) else {
            return false;
        };
        let StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } = &declaration.shape
        else {
            return false;
        };
        let byte_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"));
        if !declarations.iter().any(|candidate| {
            candidate.id == *element
                && candidate.shape == StructuralTypeShape::PrimitiveScalar(byte_type)
        }) {
            return false;
        };
        let Some(layout) =
            terminal_semantics::byte_carrier_scalar_layout(declarations.iter(), expected)
        else {
            return false;
        };
        index
            .checked_add(layout.size)
            .is_some_and(|end| end <= *length)
    }

    pub(crate) fn validate(
        node: &OptimizationNode,
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<(), LegalizationError> {
        let function = function_inputs.function();
        let AbstractOperation::StructuralCase { source, cases, .. } = &node.operation else {
            return Err(LegalizationError::custody());
        };
        case_source(function, *source)?;
        if !node.provenance.is_empty()
            || !node.work.is_empty()
            || !node.definitions.is_empty()
            || cases.len() != node.successors.len()
        {
            return Err(LegalizationError::custody());
        }
        for (case, edge) in cases.iter().zip(&node.successors) {
            if edge.psi_edge != case.psi_edge
                || edge.target != case.target
                || !edge.bindings.is_empty()
                || edge.structural_bindings != case.structural_bindings
                || edge.cleanup_actions != case.cleanup_actions
                || edge.provenance != [PsiProvenance::Edge(case.psi_edge)]
                || edge.work.is_empty()
                || edge
                    .work
                    .iter()
                    .any(|work| work.site != PsiProvenance::Edge(case.psi_edge))
            {
                return Err(LegalizationError::custody());
            }
            if !super::aggregate_results::cleanup(
                function_inputs,
                &case.cleanup_actions,
                structural_types,
            ) {
                return Err(LegalizationError::custody());
            }
        }
        Ok(())
    }
}
pub(crate) use hosted_scalar::hosted_realization;
pub(crate) mod invocations {
    use abstract_operations::AbstractOperation;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{BlockId, InvocationSite};
    use terminal_psi::StructuralArgument;

    pub(crate) fn matches(operation: &AbstractOperation, site: InvocationSite) -> bool {
        match site {
            InvocationSite::Operation(identity) => operation.psi_operation() == Some(identity),
            InvocationSite::InvokeNoNormal(edge) => matches!(operation,
                AbstractOperation::InvokeNoNormal { psi_edge, .. } if *psi_edge == edge),
            InvocationSite::ExternalCompletion(edge) => matches!(operation,
                AbstractOperation::ExternalCompletion { psi_edge, .. } if *psi_edge == edge),
        }
    }

    pub(crate) fn position(
        function: &PsiOptimizationFunction,
        site: InvocationSite,
        argument: Option<&StructuralArgument>,
    ) -> Option<(BlockId, usize)> {
        let mut sites = function.blocks.iter().flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter_map(move |(position, node)| {
                    if !matches(&node.operation, site) {
                        return None;
                    }
                    let arguments = node
                        .operation
                        .module_invocation()
                        .map(|call| call.structural_arguments)
                        .or(match &node.operation {
                            abstract_operations::boundary_or_structural_call_pair_pattern!(
                                structural_arguments,
                                ..
                            ) => Some(structural_arguments.as_slice()),
                            _ => None,
                        });
                    if argument.is_some_and(|argument| {
                        !arguments.is_some_and(|arguments| arguments.contains(argument))
                    }) {
                        return None;
                    }
                    Some((block.id, position))
                })
        });
        let position = sites.next()?;
        sites.next().is_none().then_some(position)
    }
}
pub(crate) mod literals {
    use super::{AbstractOperation, PsiOptimizationFunction};
    use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};

    pub(crate) fn producer(
        function: &PsiOptimizationFunction,
        call: semantic_vocabulary::InvocationSite,
        source: PlaceId,
    ) -> Option<(OperationId, semantic_vocabulary::StructuralTypeId)> {
        super::invocations::position(function, call, None)?;
        function.block_nodes().find(|node| {
            super::invocations::matches(&node.operation, call)
                && node.operation.module_invocation().is_some_and(|invocation|
                    invocation.structural_arguments.iter().any(|argument| argument.place == source))
                || matches!(&node.operation,
                    abstract_operations::boundary_or_structural_call_pair_pattern!(psi_operation, structural_arguments, ..)
                    if call == semantic_vocabulary::InvocationSite::Operation(*psi_operation)
                        && structural_arguments.iter().any(|argument| argument.place == source))
        })?;
        declaration_producer(function, source)
    }

    pub(crate) fn declaration_producer(
        function: &PsiOptimizationFunction,
        source: PlaceId,
    ) -> Option<(OperationId, semantic_vocabulary::StructuralTypeId)> {
        let mut declarations = function
            .structural_places
            .iter()
            .filter(|place| place.id == source);
        let declaration = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        let StructuralPlaceKind::ByteSequenceLiteral {
            structural_type: identity,
            ..
        } = declaration.kind
        else {
            return None;
        };
        let mut producers = function
            .block_nodes()
            .filter_map(|node| match &node.operation {
                AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation,
                    place,
                    structural_type,
                    ..
                } if place.id == source => Some((*psi_operation, place, structural_type)),
                _ => None,
            });
        let (operation, place, structural_type) = producers.next()?;
        if producers.next().is_some()
            || place != declaration
            || structural_type.id != identity
            || !structural_type.shape.is_borrowed_byte_view()
        {
            return None;
        }
        Some((operation, identity))
    }
}
pub(crate) mod primitive_locals {
    use super::literals::declaration_producer;
    use super::structural_case::source_result;
    use super::{AbstractOperation, PsiOptimizationFunction, ScalarType, scalar_shape};
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::value_type;
    use crate::structural_inputs::structural_reference_input::primitive_read_geometry;
    use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape,
    };

    pub(crate) fn scalar(
        types: &[terminal_psi::StructuralTypeDeclaration],
        identity: StructuralTypeId,
    ) -> Option<ScalarType> {
        types
            .iter()
            .find_map(|declaration| match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) if declaration.id == identity => {
                    Some(scalar)
                }
                _ => None,
            })
    }
    pub(crate) fn producer(
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> Option<(
        OperationId,
        &StructuralOperationResult,
        abstract_operations::AbstractResult,
    )> {
        let mut matches = function
            .block_nodes()
            .filter_map(|node| match &node.operation {
                AbstractOperation::EstablishPrimitiveLocal {
                    psi_operation,
                    result,
                    value,
                    ..
                } if result.place == place => Some((*psi_operation, result, *value)),
                _ => None,
            });
        let found = matches.next()?;
        matches.next().is_none().then_some(found)
    }
    pub(crate) fn valid_result(
        function: &PsiOptimizationFunction,
        operation: OperationId,
        result: &StructuralOperationResult,
    ) -> bool {
        result.multiplicity == StructuralMultiplicity::Unrestricted
            && result.qualification_establishments.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && has_owned_result_declaration(function, operation, result)
    }

    fn has_owned_result_declaration(
        function: &PsiOptimizationFunction,
        operation: OperationId,
        result: &StructuralOperationResult,
    ) -> bool {
        function.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: operation,
                        structural_type: result.structural_type,
                    }
        })
    }
    pub(crate) fn read_parameter(
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> Option<&terminal_psi::StructuralParameterDeclaration> {
        function
            .structural_parameters
            .iter()
            .chain(function.block_structural_parameters())
            .find(|parameter| parameter.place == place)
    }

    pub(crate) fn read_root_type(
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        source_result(function, place)
            .ok()
            .map(|(_, result)| result.structural_type)
            .or_else(|| read_parameter(function, place).map(|parameter| parameter.structural_type))
            .or_else(|| declaration_producer(function, place).map(|(_, identity)| identity))
    }

    pub(crate) fn validate(
        function: &PsiOptimizationFunction,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<(), LegalizationError> {
        for node in function.block_nodes() {
            match &node.operation {
                AbstractOperation::EstablishPrimitiveLocal {
                    psi_operation,
                    result,
                    value,
                    ..
                } => {
                    if !valid_result(function, *psi_operation, result)
                        || scalar(types, result.structural_type) != Some(value.scalar_type)
                        || scalar_shape(value.scalar_type).is_none()
                        || value_type(function, value.value) != Some(value.scalar_type)
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                AbstractOperation::PrimitiveLocalStore {
                    destination, value, ..
                } => {
                    let (operation, result, _) =
                        producer(function, *destination).ok_or(LegalizationError::custody())?;
                    if !valid_result(function, operation, result)
                        || !result.qualifications.is_empty()
                        || scalar(types, result.structural_type) != Some(value.scalar_type)
                        || scalar_shape(value.scalar_type).is_none()
                        || value_type(function, value.value) != Some(value.scalar_type)
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                AbstractOperation::PrimitiveScalarRead {
                    source,
                    path,
                    result,
                    ..
                } => {
                    let identity = if let Some((operation, local, _)) = producer(function, *source)
                    {
                        if !path.is_empty() || !valid_result(function, operation, local) {
                            return Err(LegalizationError::custody());
                        }
                        local.structural_type
                    } else if let Ok((operation, local)) = source_result(function, *source) {
                        if !has_owned_result_declaration(function, operation, local) {
                            return Err(LegalizationError::custody());
                        }
                        local.structural_type
                    } else {
                        let parameter =
                            read_parameter(function, *source).ok_or(LegalizationError::custody())?;
                        if !(matches!(
                            parameter.access,
                            terminal_psi::reading_borrow_pair_pattern!()
                        ) || parameter.access == StructuralAccess::Owned)
                        {
                            return Err(LegalizationError::custody());
                        }
                        parameter.structural_type
                    };
                    if primitive_read_geometry(identity, path, result.scalar_type, types).is_none()
                        || scalar_shape(result.scalar_type).is_none()
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
pub(crate) use crate::legalization_reference_custody as reference_custody;
pub(crate) mod structural_call {
    use crate::LegalizationError;
    use abstract_operations::AbstractOperationPlan;
    use calling_conventions::{CallPlan, ValueShape};
    use optimization_unit::PsiOptimizationFunction;
    use target_operations::{TargetOperationPlan, TargetStructuralArgument};
    use terminal_psi::{StructuralAccess, StructuralArgument, find_structural_type};

    pub(crate) mod array_homes {
        use super::{
            AbstractOperationPlan, CallPlan, PsiOptimizationFunction, StructuralAccess,
            StructuralArgument, TargetStructuralArgument,
        };
        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input::aggregate_results::{node_site, precedes};
        use crate::legalization::scalar_graph_input::structural_case;
        use crate::structural_inputs::structural_reference_input::{
            fixed_byte_array_view, fixed_element_array_view, parameter_shape,
        };
        use semantic_vocabulary::StructuralPlaceKind;
        use terminal_psi::{StructuralMultiplicity, StructuralParameterDeclaration};

        pub(crate) fn argument(
            semantic: &StructuralArgument,
            call_operation: semantic_vocabulary::InvocationSite,
            caller: &PsiOptimizationFunction,
            destination: &StructuralParameterDeclaration,
            call: &CallPlan,
            parameter_ordinal: usize,
            plan: &AbstractOperationPlan,
            native: &target_operations::TargetOperationPlan,
        ) -> Result<Option<TargetStructuralArgument>, LegalizationError> {
            if !matches!(
                semantic.access,
                terminal_psi::reading_borrow_pair_pattern!()
            ) {
                return Ok(None);
            }
            let types = &plan.structural_types;
            let producer_result = structural_case::source_result(caller, semantic.place).ok();
            let (presented, source) = if let Some((producer, result)) = producer_result {
                (
                    StructuralParameterDeclaration {
                        place: result.place,
                        position: destination.position,
                        structural_type: result.structural_type,
                        multiplicity: result.multiplicity,
                        access: StructuralAccess::Owned,
                        qualifications: result.qualifications.clone(),
                        projected_qualifications: result.projected_qualifications.clone(),
                        additional_input_qualifications: result
                            .additional_input_qualifications
                            .clone(),
                    },
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: producer,
                    },
                )
            } else if let Some(parameter) = caller.structural_parameter(semantic.place) {
                if parameter.access != StructuralAccess::Owned {
                    return Ok(None);
                }
                let target = native
                    .function(caller.machine)
                    .ok_or(LegalizationError::custody())?;
                let source = super::super::structural_parameters(target)
                    .and_then(|parameters| {
                        parameters
                            .iter()
                            .find(|source| source.place == parameter.place)
                    })
                    .ok_or(LegalizationError::custody())?;
                let shape = parameter_shape(parameter, types).ok_or(LegalizationError::custody())?;
                if parameter.multiplicity == StructuralMultiplicity::Linear
                    || source.structural_type != parameter.structural_type
                    || source.access != parameter.access
                    || source.multiplicity != parameter.multiplicity
                    || source.projected_qualifications != parameter.projected_qualifications
                    || source.shape != shape
                    || !caller.structural_places.iter().any(|place| {
                        place.id == parameter.place
                            && place.kind
                                == StructuralPlaceKind::Parameter {
                                    position: parameter.position,
                                }
                    })
                {
                    return Err(LegalizationError::custody());
                }
                (parameter.clone(), source.placement.clone().into())
            } else if let Some((block, parameter)) = caller.blocks.iter().find_map(|block| {
                block
                    .structural_parameter(semantic.place)
                    .map(|parameter| (block.id, parameter))
            }) {
                if parameter.access != StructuralAccess::Owned {
                    return Ok(None);
                }
                if parameter.multiplicity == StructuralMultiplicity::Linear
                    || !structural_case::block_parameter_declared(caller, block, parameter)
                {
                    return Err(LegalizationError::custody());
                }
                (
                    parameter.clone(),
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place: parameter.place,
                    },
                )
            } else {
                return Ok(None);
            };
            if semantic.path.is_empty()
                && terminal_semantics::borrowed_referent_presentation(
                    types.iter(),
                    presented.structural_type,
                    destination.structural_type,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            {
                return Ok(None);
            }
            let byte_window =
                fixed_byte_array_view(&presented, semantic, destination.structural_type, types);
            let element_view =
                fixed_element_array_view(&presented, semantic, destination.structural_type, types);
            let (source_byte_offset, length, stride) = match (byte_window, element_view) {
                (Some((offset, length)), _) => (offset, length, 1),
                (None, Some((offset, length, stride))) => (offset, length, stride),
                (None, None) => return Ok(None),
            };
            if let Some((producer, result)) = producer_result {
                let producer_site = node_site(caller, |operation| {
                    operation.structural_result() == Some((producer, result))
                })
                .ok_or(LegalizationError::custody())?;
                if result.multiplicity == StructuralMultiplicity::Linear
                    || !result.claims.is_empty()
                    || !precedes(caller, producer_site, call_operation, semantic)?
                    || !structural_case::records_operation_result(caller, producer, result)
                {
                    return Err(LegalizationError::custody());
                }
            }
            let shape = parameter_shape(destination, types).ok_or(LegalizationError::custody())?;
            let placement = call
                .parameters
                .get(parameter_ordinal)
                .ok_or(LegalizationError::custody())?;
            if destination.access != semantic.access
                || destination.multiplicity != StructuralMultiplicity::Unrestricted
                || placement.shape != shape
            {
                return Err(LegalizationError::custody());
            }
            Ok(Some(TargetStructuralArgument {
                place: semantic.place,
                access: semantic.access,
                path: semantic.path.clone(),
                root_structural_type: presented.structural_type,
                structural_type: destination.structural_type,
                shape,
                source_byte_offset,
                fixed_array_length: Some(length),
                element_stride: Some(stride),
                indices: Vec::new(),
                source,
                destination: placement.clone(),
            }))
        }
    }
    pub(crate) mod byte_views {
        use super::super::target::control_flow::sources::{call_indices, dominates};
        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input::structural_case;
        use crate::structural_inputs::structural_reference_input::{
            bounded_byte_view, fixed_byte_array_view,
        };
        use abstract_operations::AbstractOperationPlan;
        use calling_conventions::{CallPlan, ValueShape};
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::StructuralPlaceKind;
        use target_operations::{
            TargetOperationPlan, TargetStructuralArgument, TargetStructuralArgumentSource,
        };
        use terminal_psi::{
            StructuralAccess, StructuralArgument, StructuralMultiplicity,
            StructuralParameterDeclaration,
        };
        use terminal_semantics::descriptor_results::is_shared_descriptor;

        pub(crate) fn argument(
            actual: &StructuralArgument,
            call_operation: semantic_vocabulary::InvocationSite,
            caller: &PsiOptimizationFunction,
            formal: &StructuralParameterDeclaration,
            call: &CallPlan,
            ordinal: usize,
            native: &TargetOperationPlan,
            plan: &AbstractOperationPlan,
        ) -> Result<Option<TargetStructuralArgument>, LegalizationError> {
            if actual.access != StructuralAccess::SharedBorrow
                || actual.path.iter().any(|segment| {
                    matches!(
                        segment,
                        terminal_psi::StructuralPathSegment::Referent
                            | terminal_psi::StructuralPathSegment::FixedRange { .. }
                    )
                })
                || formal.access != actual.access
                || !is_shared_descriptor(plan.structural_types.iter(), formal.structural_type)
            {
                return Ok(None);
            }
            let after = super::super::invocations::position(caller, call_operation, Some(actual))
                .ok_or(LegalizationError::custody())?;
            let (root, source, result_custody) =
                if let Some(arrival) = caller.arrival_parameter(actual.place) {
                    let place = caller
                        .structural_place(actual.place)
                        .ok_or(LegalizationError::custody())?;
                    let source = match place.kind {
                        StructuralPlaceKind::Parameter { .. } => {
                            let parameter = native
                                .function(caller.machine)
                                .and_then(super::super::structural_parameters)
                                .and_then(|parameters| {
                                    parameters
                                        .iter()
                                        .find(|parameter| parameter.place == actual.place)
                                })
                                .ok_or(LegalizationError::custody())?;
                            if parameter.structural_type != arrival.structural_type
                                || parameter.access != arrival.access
                                || parameter.multiplicity != arrival.multiplicity
                                || parameter.projected_qualifications
                                    != arrival.projected_qualifications
                            {
                                return Err(LegalizationError::custody());
                            }
                            parameter.placement.clone().into()
                        }
                        StructuralPlaceKind::BlockParameter { block, .. } => {
                            if block != after.0 && !dominates(caller, block, after.0) {
                                return Err(LegalizationError::custody());
                            }
                            TargetStructuralArgumentSource::BlockParameter {
                                block,
                                place: actual.place,
                            }
                        }
                        _ => return Err(LegalizationError::custody()),
                    };
                    (arrival.clone(), source, None)
                } else {
                    let mut results = caller.blocks.iter().flat_map(|block| {
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .filter_map(move |(position, node)| {
                                let (producer, result) = node.operation.structural_result()?;
                                (result.place == actual.place)
                                    .then_some((block.id, position, producer, result))
                            })
                    });
                    let Some((block, position, producer, result)) = results.next() else {
                        return Ok(None);
                    };
                    if results.next().is_some()
                        || (block == after.0 && position >= after.1)
                        || (block != after.0 && !dominates(caller, block, after.0))
                        || !structural_case::records_operation_result(caller, producer, result)
                    {
                        return Err(LegalizationError::custody());
                    }
                    (
                        StructuralParameterDeclaration {
                            place: result.place,
                            position: 0,
                            structural_type: result.structural_type,
                            multiplicity: result.multiplicity,
                            access: StructuralAccess::Owned,
                            qualifications: result.qualifications.clone(),
                            projected_qualifications: result.projected_qualifications.clone(),
                            additional_input_qualifications: result
                                .additional_input_qualifications
                                .clone(),
                        },
                        TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: producer,
                        },
                        Some(result),
                    )
                };
            let (offset, fixed_array_length, elements) = if actual.path.is_empty()
                && super::super::byte_views::descriptor_result(
                    caller,
                    actual.place,
                    &plan.structural_types,
                )
                .is_some_and(|(_, identity)| identity == formal.structural_type)
            {
                (0, None, Vec::new())
            } else if let Some((offset, length)) = fixed_byte_array_view(
                &root,
                actual,
                formal.structural_type,
                &plan.structural_types,
            ) {
                (offset, Some(length), Vec::new())
            } else {
                let Some(geometry) = bounded_byte_view(
                    &root,
                    actual,
                    formal.structural_type,
                    &plan.structural_types,
                ) else {
                    return Ok(None);
                };
                (geometry.offset, None, geometry.elements)
            };
            let destination = call
                .parameters
                .get(ordinal)
                .ok_or(LegalizationError::custody())?;
            if result_custody.is_some_and(|result| {
                !result.claims.is_empty() || !result.qualification_establishments.is_empty()
            }) || formal.multiplicity != StructuralMultiplicity::Unrestricted
                || !formal.projected_qualifications.is_empty()
                || destination.shape != ValueShape::borrowed_reference(16, 8)
            {
                return Err(LegalizationError::custody());
            }
            Ok(Some(TargetStructuralArgument {
                place: actual.place,
                access: actual.access,
                path: actual.path.clone(),
                root_structural_type: root.structural_type,
                structural_type: formal.structural_type,
                shape: destination.shape,
                source_byte_offset: offset,
                fixed_array_length,
                element_stride: fixed_array_length.map(|_| 1),
                indices: call_indices(
                    native,
                    caller,
                    target_operations::CallSiteOwner::from(call_operation),
                    &elements,
                )?,
                source,
                destination: destination.clone(),
            }))
        }
    }
    pub(crate) mod exclusive {
        use super::{
            AbstractOperationPlan, CallPlan, PsiOptimizationFunction, StructuralAccess,
            StructuralArgument, TargetOperationPlan, TargetStructuralArgument, ValueShape,
        };
        use crate::LegalizationError;
        use crate::structural_inputs::structural_reference_input::{
            bounded_byte_field_owner, bounded_byte_view, fixed_byte_array_view,
            fixed_element_array_view, parameter_shape, runtime_projection,
        };
        use semantic_vocabulary::StructuralPlaceKind;
        use target_operations::TargetStructuralArgumentSource;
        use terminal_psi::{StructuralMultiplicity, StructuralParameterDeclaration};

        pub(crate) fn argument(
            semantic: &StructuralArgument,
            call_operation: semantic_vocabulary::InvocationSite,
            caller: &PsiOptimizationFunction,
            destination: &StructuralParameterDeclaration,
            call: &CallPlan,
            scalar_count: usize,
            native: &TargetOperationPlan,
            plan: &AbstractOperationPlan,
        ) -> Result<TargetStructuralArgument, LegalizationError> {
            let source = caller
                .arrival_parameter(semantic.place)
                .ok_or(LegalizationError::custody())?;
            let types = &plan.structural_types;
            let source_shape = parameter_shape(source, types).ok_or(LegalizationError::custody())?;
            let source_placement = arrival_source(source, caller, native, source_shape)?;
            let byte_field =
                bounded_byte_view(source, semantic, destination.structural_type, types);
            let byte_field_projection = byte_field.as_ref().map(|geometry| geometry.offset);
            let byte_owner =
                bounded_byte_field_owner(source, semantic, destination.structural_type, types);
            let geometry = runtime_projection(source.structural_type, &semantic.path, types);
            let projection = geometry
                .as_ref()
                .map(|(selected, offset, _)| (*selected, *offset));
            let byte_view =
                fixed_byte_array_view(source, semantic, destination.structural_type, types);
            let element_view =
                fixed_element_array_view(source, semantic, destination.structural_type, types);
            let (structural_type, source_byte_offset) = match (
                projection,
                byte_field_projection,
                byte_view,
                element_view,
                byte_owner,
            ) {
                (None, None, None, None, Some((offset, _))) => {
                    (destination.structural_type, offset)
                }
                (_, None, Some((offset, _)), _, None)
                | (_, None, None, Some((offset, _, _)), None) => {
                    (destination.structural_type, offset)
                }
                (Some((projected, offset)), None, None, None, None) => (projected, offset),
                (None, Some(offset), None, None, None) => (destination.structural_type, offset),
                _ => return Err(LegalizationError::custody()),
            };
            let descriptor = byte_view.is_some() || byte_field.is_some() || element_view.is_some();
            let shape = parameter_shape(destination, types).ok_or(LegalizationError::custody())?;
            if semantic.place != source.place
                || destination.access != semantic.access
                || !matches!(
                    (source.access, semantic.access),
                    (
                        StructuralAccess::MutableBorrow,
                        StructuralAccess::SharedBorrow
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    ) | (
                        StructuralAccess::WriteOnlyBorrow,
                        StructuralAccess::WriteOnlyBorrow
                    ) | (
                        StructuralAccess::SharedBorrow,
                        StructuralAccess::SharedBorrow
                    )
                )
                || source.multiplicity != StructuralMultiplicity::Unrestricted
                || destination.multiplicity != StructuralMultiplicity::Unrestricted
                || (structural_type != destination.structural_type && !descriptor)
                || call
                    .parameters
                    .get(scalar_count)
                    .is_none_or(|placement| placement.shape != shape)
            {
                return Err(LegalizationError::custody());
            }
            Ok(TargetStructuralArgument {
                place: semantic.place,
                access: semantic.access,
                path: semantic.path.clone(),
                root_structural_type: source.structural_type,
                structural_type: if descriptor {
                    destination.structural_type
                } else {
                    structural_type
                },
                shape,
                source_byte_offset,
                fixed_array_length: byte_view
                    .map(|(_, length)| length)
                    .or_else(|| element_view.map(|(_, length, _)| length)),
                element_stride: byte_view
                    .map(|_| 1)
                    .or_else(|| element_view.map(|(_, _, stride)| stride)),
                indices: super::super::target::control_flow::sources::call_indices(
                    native,
                    caller,
                    target_operations::CallSiteOwner::from(call_operation),
                    byte_field.as_ref().map_or_else(
                        || {
                            geometry
                                .as_ref()
                                .map_or(&[][..], |(_, _, elements)| elements.as_slice())
                        },
                        |geometry| geometry.elements.as_slice(),
                    ),
                )?,
                source: source_placement,
                destination: call.parameters[scalar_count].clone(),
            })
        }

        fn arrival_source(
            source: &StructuralParameterDeclaration,
            caller: &PsiOptimizationFunction,
            native: &TargetOperationPlan,
            source_shape: ValueShape,
        ) -> Result<TargetStructuralArgumentSource, LegalizationError> {
            let place = caller
                .structural_place(source.place)
                .ok_or(LegalizationError::custody())?;
            match place.kind {
                StructuralPlaceKind::BlockParameter { block, .. } => {
                    Ok(TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place: source.place,
                    })
                }
                StructuralPlaceKind::Parameter { .. } => {
                    let parameter = native
                        .function(caller.machine)
                        .and_then(super::super::structural_parameters)
                        .and_then(|parameters| {
                            parameters.iter().find(|row| row.place == source.place)
                        })
                        .ok_or(LegalizationError::custody())?;
                    if parameter.structural_type != source.structural_type
                        || parameter.access != source.access
                        || parameter.multiplicity != source.multiplicity
                        || parameter.projected_qualifications != source.projected_qualifications
                        || parameter.shape != source_shape
                    {
                        return Err(LegalizationError::custody());
                    }
                    Ok(parameter.placement.clone().into())
                }
                _ => Err(LegalizationError::custody()),
            }
        }
    }

    pub(crate) fn argument_at(
        semantic: &StructuralArgument,
        position: usize,
        call_operation: semantic_vocabulary::OperationId,
        caller: &PsiOptimizationFunction,
        called: abstract_operations::AbstractCallSignature<'_>,
        call: &CallPlan,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
        custody: &super::reference_custody::Custody,
    ) -> Result<TargetStructuralArgument, LegalizationError> {
        argument_at_invocation(
            semantic,
            position,
            call_operation.into(),
            caller,
            called,
            call,
            native,
            plan,
            custody,
        )
    }

    pub(crate) fn argument_at_invocation(
        semantic: &StructuralArgument,
        position: usize,
        call_operation: semantic_vocabulary::InvocationSite,
        caller: &PsiOptimizationFunction,
        called: abstract_operations::AbstractCallSignature<'_>,
        call: &CallPlan,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
        custody: &super::reference_custody::Custody,
    ) -> Result<TargetStructuralArgument, LegalizationError> {
        let declaration = crate::legalization::scalar_graph_input::call_signature::parameter(
            called,
            position,
            semantic.place,
        )?;
        let destination_parameter = &declaration;
        let parameter_ordinal = called
            .scalar_parameter_count()
            .checked_add(position)
            .ok_or(LegalizationError::custody())?;
        if semantic
            .path
            .iter()
            .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
        {
            return super::reference_custody::referent_argument(
                semantic,
                destination_parameter,
                call.parameters
                    .get(parameter_ordinal)
                    .ok_or(LegalizationError::custody())?,
                caller,
                native,
                call_operation,
                custody,
                &plan.structural_types,
            );
        }
        if super::address_joins::parameter(caller, semantic.place, plan).is_some() {
            return super::address_joins::call_argument(
                semantic,
                destination_parameter,
                call_operation,
                caller,
                call,
                parameter_ordinal,
                native,
                plan,
            );
        }
        if semantic.access == StructuralAccess::Owned
            || crate::structural_inputs::structural_reference_input::owned_storage_shape(
                destination_parameter.structural_type,
                &plan.structural_types,
            )
            .is_some()
        {
            return super::aggregate_results::call_argument_invocation(
                semantic,
                position,
                call_operation,
                caller,
                called,
                call,
                native,
                plan,
                custody,
            );
        }
        let root_type = caller
            .arrival_parameter(semantic.place)
            .map(|parameter| parameter.structural_type)
            .or_else(|| {
                caller
                    .structural_place(semantic.place)
                    .and_then(|place| place.kind.produced_structural_type())
            });
        if !semantic.path.is_empty()
            && root_type.is_some_and(|root| {
                crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
                    root,
                    &semantic.path,
                    semantic.access,
                    Some(destination_parameter.structural_type),
                    &plan.structural_types,
                )
                .is_some_and(|(selected, _, _)| {
                    matches!(
                        terminal_semantics::borrowed_referent_presentation(
                            plan.structural_types.iter(),
                            selected,
                            destination_parameter.structural_type,
                        ),
                        Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                    )
                })
            })
        {
            return super::aggregate_results::borrowed_arguments::reconstruct(
                semantic,
                position,
                call_operation,
                caller,
                called,
                call,
                native,
                plan,
            );
        }
        if let Some(argument) = byte_views::argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        )? {
            return Ok(argument);
        }
        if let Some(argument) = array_homes::argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            plan,
            native,
        )? {
            return Ok(argument);
        }
        if semantic.path.is_empty()
            && root_type.is_some_and(|root| {
                terminal_semantics::borrowed_referent_presentation(
                    plan.structural_types.iter(),
                    root,
                    destination_parameter.structural_type,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            })
            && plan.structural_types.iter().any(|declaration| {
                declaration.id == destination_parameter.structural_type
                    && (declaration.shape.is_borrowed_byte_carrier()
                        || matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                        ))
            })
        {
            return view_argument(
                semantic,
                call_operation,
                caller,
                destination_parameter,
                call,
                parameter_ordinal,
                native,
                &plan.structural_types,
            );
        }
        if super::primitive_locals::producer(caller, semantic.place).is_some()
            || matches!(
                semantic.access,
                terminal_psi::exclusive_borrow_pair_pattern!()
            )
            || semantic.access == StructuralAccess::SharedBorrow
                && super::primitive_locals::scalar(
                    &plan.structural_types,
                    destination_parameter.structural_type,
                )
                .is_some()
        {
            return primitive_argument(
                semantic,
                call_operation,
                caller,
                destination_parameter,
                call,
                parameter_ordinal,
                native,
                plan,
            );
        }
        if caller.structural_parameters.iter().any(|source| {
            terminal_semantics::fixed_byte_array_window(
                plan.structural_types.iter(),
                source,
                semantic,
                destination_parameter,
            )
            .is_some()
        }) {
            return primitive_argument(
                semantic,
                call_operation,
                caller,
                destination_parameter,
                call,
                parameter_ordinal,
                native,
                plan,
            );
        }
        if semantic.access == StructuralAccess::SharedBorrow
            && semantic.path.iter().all(|segment| {
                matches!(
                    segment,
                    terminal_psi::StructuralPathSegment::Field(_)
                        | terminal_psi::StructuralPathSegment::FixedIndex(_)
                )
            })
        {
            return primitive_argument(
                semantic,
                call_operation,
                caller,
                destination_parameter,
                call,
                parameter_ordinal,
                native,
                plan,
            );
        }
        if semantic.access == StructuralAccess::SharedBorrow
            && semantic.path.iter().any(|segment| {
                matches!(
                    segment,
                    terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                )
            })
        {
            return super::aggregate_results::call_argument_invocation(
                semantic,
                position,
                call_operation,
                caller,
                called,
                call,
                native,
                plan,
                custody,
            );
        }
        Err(LegalizationError::custody())
    }

    fn view_argument(
        semantic: &StructuralArgument,
        call_operation: semantic_vocabulary::InvocationSite,
        caller: &PsiOptimizationFunction,
        destination_parameter: &terminal_psi::StructuralParameterDeclaration,
        call: &CallPlan,
        parameter_ordinal: usize,
        native: &TargetOperationPlan,
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<TargetStructuralArgument, LegalizationError> {
        use target_operations::TargetStructuralArgumentSource;
        let completed =
            super::byte_views::completed_descriptor(caller, semantic.place, structural_types)
                .map(|(producer, result)| (producer, result.structural_type));
        let access = optimization_unit_semantics::structural_place_backing_access(
            caller,
            semantic.place,
            structural_types.iter(),
        )
        .ok_or_else(LegalizationError::custody)?;
        let carrier = find_structural_type(structural_types, destination_parameter.structural_type)
            .map(|declaration| match declaration.shape {
                terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedView { .. },
                ) => ValueShape::borrowed_reference(8, 8),
                _ => ValueShape::borrowed_reference(16, 8),
            })
            .ok_or(LegalizationError::custody())?;
        if !matches!(
            semantic.access,
            terminal_psi::reading_borrow_pair_pattern!()
        ) || !semantic.path.is_empty()
            || destination_parameter.access != semantic.access
            || !terminal_semantics::view_descriptor_parameter(
                destination_parameter,
                structural_types.iter(),
            )
            || !(access == semantic.access
                || access == StructuralAccess::MutableBorrow
                    && semantic.access == StructuralAccess::SharedBorrow)
        {
            return Err(LegalizationError::custody());
        }
        let (structural_type, source) = if let Some((producer, structural_type)) = completed {
            (
                structural_type,
                TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: producer,
                },
            )
        } else if let Some((producer, structural_type)) =
            super::literals::producer(caller, call_operation, semantic.place)
        {
            if !find_structural_type(structural_types, structural_type)
                .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
            {
                return Err(LegalizationError::custody());
            }
            (
                structural_type,
                TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: producer,
                },
            )
        } else if let Some((block, parameter)) = caller.blocks.iter().find_map(|block| {
            block
                .structural_parameter(semantic.place)
                .map(|parameter| (block, parameter))
        }) {
            if block.id == caller.entry
                || terminal_semantics::descriptor_parameter_access(
                    parameter,
                    structural_types.iter(),
                ) != Some(access)
            {
                return Err(LegalizationError::custody());
            }
            (
                parameter.structural_type,
                TargetStructuralArgumentSource::BlockParameter {
                    block: block.id,
                    place: parameter.place,
                },
            )
        } else {
            let source = caller
                .structural_parameter(semantic.place)
                .ok_or(LegalizationError::custody())?;
            let target_caller = native
                .function(caller.machine)
                .ok_or(LegalizationError::custody())?;
            let parameters =
                super::structural_parameters(target_caller).ok_or(LegalizationError::custody())?;
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.place == semantic.place)
                .ok_or(LegalizationError::custody())?;
            if semantic.place != source.place
                || terminal_semantics::descriptor_parameter_access(source, structural_types.iter())
                    != Some(access)
                || !terminal_semantics::view_descriptor_projected_qualifications(
                    structural_types.iter(),
                    source.structural_type,
                    &source.projected_qualifications,
                )
                || parameter.place != source.place
                || parameter.structural_type != source.structural_type
                || parameter.access != source.access
                || parameter.multiplicity != source.multiplicity
                || parameter.shape != carrier
                || !terminal_semantics::view_descriptor_projected_qualifications(
                    structural_types.iter(),
                    parameter.structural_type,
                    &parameter.projected_qualifications,
                )
            {
                return Err(LegalizationError::custody());
            }
            (source.structural_type, parameter.placement.clone().into())
        };
        if terminal_semantics::borrowed_referent_presentation(
            structural_types.iter(),
            structural_type,
            destination_parameter.structural_type,
        ) != Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            || call
                .parameters
                .get(parameter_ordinal)
                .is_none_or(|parameter| parameter.shape != carrier)
        {
            return Err(LegalizationError::custody());
        }
        Ok(TargetStructuralArgument {
            place: semantic.place,
            access: semantic.access,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type: destination_parameter.structural_type,
            shape: carrier,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source,
            destination: call
                .parameters
                .get(parameter_ordinal)
                .ok_or(LegalizationError::custody())?
                .clone(),
        })
    }

    pub(crate) fn primitive_argument(
        semantic: &StructuralArgument,
        call_operation: semantic_vocabulary::InvocationSite,
        caller: &PsiOptimizationFunction,
        destination_parameter: &terminal_psi::StructuralParameterDeclaration,
        call: &CallPlan,
        parameter_ordinal: usize,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
    ) -> Result<TargetStructuralArgument, LegalizationError> {
        use target_operations::TargetStructuralArgumentSource;
        if let Some((producer, result, value)) =
            super::primitive_locals::producer(caller, semantic.place)
        {
            if !super::primitive_locals::valid_result(caller, producer, result)
                || !semantic.path.is_empty()
                || semantic.access == StructuralAccess::Owned
                || destination_parameter.access != semantic.access
                || destination_parameter.structural_type != result.structural_type
                || destination_parameter.multiplicity
                    != terminal_psi::StructuralMultiplicity::Unrestricted
                || !destination_parameter.qualifications.is_empty()
                || !destination_parameter.projected_qualifications.is_empty()
                || super::primitive_locals::scalar(&plan.structural_types, result.structural_type)
                    != Some(value.scalar_type)
            {
                return Err(LegalizationError::custody());
            }
            let shape = crate::structural_inputs::structural_reference_input::parameter_shape(
                destination_parameter,
                &plan.structural_types,
            )
            .ok_or(LegalizationError::custody())?;
            let destination = call
                .parameters
                .get(parameter_ordinal)
                .ok_or(LegalizationError::custody())?
                .clone();
            if destination.shape != shape {
                return Err(LegalizationError::custody());
            }
            return Ok(TargetStructuralArgument {
                place: semantic.place,
                access: semantic.access,
                path: Vec::new(),
                root_structural_type: result.structural_type,
                structural_type: result.structural_type,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                indices: Vec::new(),
                source: TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                    psi_operation: producer,
                },
                destination,
            });
        }
        exclusive::argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        )
    }
}
pub(crate) mod structural_fields {
    use super::byte_views::{contains_readable_bytes, view_type};
    use super::{AbstractOperation, PsiOptimizationFunction, ScalarType};
    use crate::structural_inputs::structural_reference_input::{
        ProjectedByteField, byte_field_storage, case_field_read, field_read, projected_byte_field,
        projected_view_slot,
    };
    use optimization_unit_semantics::byte_field_root;
    use terminal_psi::find_structural_type;

    pub(crate) fn scalar_store_destination(
        function: &PsiOptimizationFunction,
        destination: &abstract_operations::StructuralScalarStoreDestination,
        path: &[terminal_psi::StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
        types: &[terminal_psi::StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        identities: &terminal_psi::TerminalIdentityTable,
        custody: &super::reference_custody::Custody,
    ) -> Option<(
        abstract_operations::StructuralScalarStoreDestination,
        Vec<terminal_psi::StructuralPathSegment>,
    )> {
        let selected = terminal_semantics::projected_field_declaration(
            types.iter(),
            destination.structural_type(),
            path,
            field,
        )?;
        let mut written = path.to_vec();
        written.push(terminal_psi::StructuralPathSegment::Field(
            selected.identity,
        ));
        if !optimization_unit_semantics::structural_write_preserves_content(
            function,
            destination.place(),
            &written,
            types,
            domains,
            identities,
        ) {
            return None;
        }
        if !path.contains(&terminal_psi::StructuralPathSegment::Referent) {
            return Some((destination.clone(), path.to_vec()));
        }
        let resolved = super::reference_custody::referent_destination(
            function,
            custody,
            types,
            destination.place(),
            path,
            terminal_psi::StructuralAccess::WriteOnlyBorrow,
        )?;
        (resolved.carrier_type == destination.structural_type())
            .then(|| (destination.clone(), path.to_vec()))
    }

    fn field_write_preserves_memberships(
        function: &PsiOptimizationFunction,
        types: &[terminal_psi::StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        identities: &terminal_psi::TerminalIdentityTable,
        destination: semantic_vocabulary::PlaceId,
        root: semantic_vocabulary::StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> bool {
        terminal_semantics::projected_field_declaration(types.iter(), root, path, field)
            .is_some_and(|selected| {
                let mut written = path.to_vec();
                written.push(terminal_psi::StructuralPathSegment::Field(
                    selected.identity,
                ));
                optimization_unit_semantics::structural_write_preserves_memberships(
                    function,
                    destination,
                    &written,
                    types,
                    domains,
                    identities,
                )
            })
    }

    fn reference_field_write_preserves_memberships(
        function: &PsiOptimizationFunction,
        types: &[terminal_psi::StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        identities: &terminal_psi::TerminalIdentityTable,
        destination: semantic_vocabulary::PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> bool {
        let root = function
            .arrival_parameter(destination)
            .map(|parameter| parameter.structural_type)
            .or_else(|| {
                super::structural_case::source_result(function, destination)
                    .ok()
                    .map(|(_, result)| result.structural_type)
            });
        root.is_some_and(|root| {
            field_write_preserves_memberships(
                function,
                types,
                domains,
                identities,
                destination,
                root,
                path,
                field,
            )
        })
    }

    pub(crate) fn replacement(
        function: &PsiOptimizationFunction,
        operation: &AbstractOperation,
        types: &[terminal_psi::StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        identities: &terminal_psi::TerminalIdentityTable,
        custody: &super::reference_custody::Custody,
    ) -> Option<terminal_psi::StructuralArgument> {
        if let AbstractOperation::ByteSequenceStore {
            destination,
            source,
            ..
        } = operation
        {
            let (structural_type, access, multiplicity, projected) =
                if let Some(parameter) = function.arrival_parameter(*destination) {
                    (
                        parameter.structural_type,
                        parameter.access,
                        parameter.multiplicity,
                        parameter.projected_qualifications.as_slice(),
                    )
                } else {
                    let (producer, result) =
                        super::structural_case::source_result(function, *destination).ok()?;
                    if !result.claims.is_empty()
                        || !function.structural_places.iter().any(|place| {
                            place.id == *destination
                                && place.kind
                                    == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                                        producer,
                                        structural_type: result.structural_type,
                                    }
                        })
                    {
                        return None;
                    }
                    (
                        result.structural_type,
                        terminal_psi::StructuralAccess::Owned,
                        result.multiplicity,
                        result.projected_qualifications.as_slice(),
                    )
                };
            if source == destination
                || !matches!(
                    access,
                    terminal_psi::StructuralAccess::Owned
                        | terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                )
                || multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !projected.is_empty()
                || !types.iter().any(|declaration| {
                    declaration.id == structural_type
                        && declaration.shape.is_bounded_owned_byte_sequence()
                })
                || !contains_readable_bytes(function, *source, types)
                || function
                    .entry_claim_declarations
                    .iter()
                    .any(|claim| claim.input == *destination || claim.input == *source)
                || function
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == *destination || claim.input.root == *source)
            {
                return None;
            }
            return Some(terminal_psi::StructuralArgument {
                place: *destination,
                access,
                path: Vec::new(),
            });
        }
        let (destination, path, field, source) = match operation {
            AbstractOperation::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                source,
                ..
            } => (destination, path, field, Some(*source)),
            AbstractOperation::StructuralByteSequenceFieldByteStore {
                destination,
                path,
                field,
                ..
            } => (destination, path, field, None),
            _ => return None,
        };
        if path.contains(&terminal_psi::StructuralPathSegment::Referent)
            && !reference_field_write_preserves_memberships(
                function,
                types,
                domains,
                identities,
                *destination,
                path,
                *field,
            )
        {
            return None;
        }
        let semantic_path = path.clone();
        let owned_result = source
            .filter(|_| !path.contains(&terminal_psi::StructuralPathSegment::Referent))
            .and_then(|_| super::structural_case::source_result(function, *destination).ok());
        let (root, root_type, access, path) = if let Some((producer, result)) = owned_result {
            if !function.structural_places.iter().any(|place| {
                place.id == *destination
                    && place.kind
                        == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        }
            }) {
                return None;
            }
            (
                *destination,
                result.structural_type,
                terminal_psi::StructuralAccess::Owned,
                path.clone(),
            )
        } else if let Some(parameter) = function.arrival_parameter(*destination) {
            if !(matches!(
                parameter.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) || source.is_some() && parameter.access == terminal_psi::StructuralAccess::Owned)
                || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return None;
            }
            (
                parameter.place,
                parameter.structural_type,
                parameter.access,
                path.clone(),
            )
        } else {
            let resolved = super::reference_custody::referent_destination(
                function,
                custody,
                types,
                *destination,
                path,
                terminal_psi::StructuralAccess::MutableBorrow,
            )?;
            (
                *destination,
                resolved.referent,
                terminal_psi::StructuralAccess::MutableBorrow,
                resolved.path,
            )
        };
        let (declared_type, multiplicity, declared_access) = if let Some(parameter) =
            function.arrival_parameter(*destination)
        {
            (
                parameter.structural_type,
                parameter.multiplicity,
                parameter.access,
            )
        } else {
            let (_, result) = super::structural_case::source_result(function, *destination).ok()?;
            (
                result.structural_type,
                result.multiplicity,
                terminal_psi::StructuralAccess::Owned,
            )
        };
        if !terminal_semantics::reference_source_projection_accessible(
            types.iter(),
            declared_type,
            multiplicity,
            declared_access,
            &semantic_path,
            terminal_psi::StructuralAccess::WriteOnlyBorrow,
        ) || !terminal_psi::is_static_structural_scalar_store_path(&path)
            || source == Some(*destination)
            || function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == root || Some(claim.input) == source)
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == root || Some(claim.input.root) == source)
            || source.is_some_and(|source| !contains_readable_bytes(function, source, types))
        {
            return None;
        }
        byte_field_storage(root_type, &path, *field, types)?;
        Some(terminal_psi::StructuralArgument {
            place: root,
            access,
            path: semantic_path,
        })
    }

    pub(crate) fn view_replacement(
        function: &PsiOptimizationFunction,
        operation: &AbstractOperation,
        types: &[terminal_psi::StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        identities: &terminal_psi::TerminalIdentityTable,
        custody: &super::reference_custody::Custody,
    ) -> Option<terminal_psi::StructuralArgument> {
        let AbstractOperation::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } = operation
        else {
            return None;
        };
        if path.contains(&terminal_psi::StructuralPathSegment::Referent) {
            let preserves = if let Some(field) = field {
                reference_field_write_preserves_memberships(
                    function,
                    types,
                    domains,
                    identities,
                    *destination,
                    path,
                    *field,
                )
            } else {
                optimization_unit_semantics::structural_write_preserves_memberships(
                    function,
                    *destination,
                    path,
                    types,
                    domains,
                    identities,
                )
            };
            if !preserves {
                return None;
            }
        }
        let (root_type, access, projected_path) = if field.is_none() {
            let home =
                optimization_unit_semantics::declared_descriptor_home(function, *destination)?;
            (
                home.structural_type,
                terminal_psi::StructuralAccess::Owned,
                path.clone(),
            )
        } else if let Some(resolved) = super::reference_custody::referent_destination(
            function,
            custody,
            types,
            *destination,
            path,
            terminal_psi::StructuralAccess::MutableBorrow,
        ) {
            (
                resolved.referent,
                terminal_psi::StructuralAccess::MutableBorrow,
                resolved.path,
            )
        } else {
            let (root, access) = byte_field_root(function, *destination)?;
            if !matches!(
                access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) {
                return None;
            }
            (root, access, path.clone())
        };
        if source == destination
            || !view_type(function, *source, types).is_some_and(|identity| {
                types
                    .iter()
                    .any(|row| row.id == identity && row.shape.is_borrowed_byte_view())
            })
            || function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == *destination)
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *destination)
        {
            return None;
        }
        projected_view_slot(root_type, &projected_path, *field, types)?;
        Some(terminal_psi::StructuralArgument {
            place: *destination,
            access,
            path: path.clone(),
        })
    }

    pub(crate) fn byte_index_operands(
        function: &PsiOptimizationFunction,
        unit: &optimization_unit::PsiOptimizationUnit,
        operation: semantic_vocabulary::OperationId,
        source: &terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Result<Vec<legalized_operations::LegalizedRuntimeIndexOperand>, crate::LegalizationError>
    {
        let geometry = byte_geometry(function, source, field, &unit.structural_types)
            .ok_or(crate::LegalizationError::custody())?;
        crate::legalization::runtime_indices::operands(
            function,
            unit,
            operation,
            &geometry.elements,
        )
    }

    pub(crate) fn view_slot_index_operands(
        function: &PsiOptimizationFunction,
        unit: &optimization_unit::PsiOptimizationUnit,
        operation: semantic_vocabulary::OperationId,
        destination: &terminal_psi::StructuralArgument,
        field: Option<semantic_vocabulary::StructuralFieldId>,
    ) -> Result<Vec<legalized_operations::LegalizedRuntimeIndexOperand>, crate::LegalizationError>
    {
        let geometry = view_slot_geometry(function, destination, field, &unit.structural_types)
            .ok_or(crate::LegalizationError::custody())?;
        crate::legalization::runtime_indices::operands(
            function,
            unit,
            operation,
            &geometry.elements,
        )
    }

    pub(crate) fn view_slot_geometry(
        function: &PsiOptimizationFunction,
        destination: &terminal_psi::StructuralArgument,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<ProjectedByteField> {
        let Some(field) = field else {
            let home =
                optimization_unit_semantics::declared_descriptor_home(function, destination.place)?;
            return (destination.access == terminal_psi::StructuralAccess::Owned)
                .then(|| projected_view_slot(home.structural_type, &destination.path, None, types))
                .flatten();
        };
        byte_geometry(function, destination, field, types)
    }

    pub(crate) fn byte_geometry(
        function: &PsiOptimizationFunction,
        source: &terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<ProjectedByteField> {
        let (root, access) = byte_field_root(function, source.place)?;
        if access != source.access {
            return None;
        }
        projected_byte_field(root, &source.path, field, types)
    }

    pub(crate) fn byte_read(
        function: &PsiOptimizationFunction,
        operation: &AbstractOperation,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<terminal_psi::StructuralArgument> {
        let AbstractOperation::StructuralByteSequenceFieldRead {
            result,
            source,
            path,
            field,
            ..
        } = operation
        else {
            return None;
        };
        let (structural_type, access) = byte_field_root(function, *source)?;
        if access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            || result.scalar_type
                != ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        8,
                    )
                    .ok()?,
                )
        {
            return None;
        }
        let selected = terminal_semantics::projected_field_declaration(
            types.iter(),
            structural_type,
            path,
            *field,
        )?;
        if !terminal_semantics::sequence_field_allows_observation(
            types.iter(),
            &selected.field_type,
            terminal_semantics::SequenceFieldObservation::Contents,
        ) {
            return None;
        }
        projected_byte_field(structural_type, path, *field, types)?;
        Some(terminal_psi::StructuralArgument {
            place: *source,
            access,
            path: path.clone(),
        })
    }

    pub(crate) type FieldObservation = (
        semantic_vocabulary::OperationId,
        abstract_operations::AbstractResult,
        terminal_psi::StructuralArgument,
        Option<semantic_vocabulary::StructuralCaseId>,
        semantic_vocabulary::StructuralFieldId,
    );

    pub(crate) fn read(
        function: &PsiOptimizationFunction,
        operation: &AbstractOperation,
        types: &[terminal_psi::StructuralTypeDeclaration],
        custody: &super::reference_custody::Custody,
    ) -> Option<FieldObservation> {
        if let AbstractOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            source,
            path,
            field,
        } = operation
        {
            let (structural_type, access) = byte_field_root(function, *source)?;
            if result.scalar_type
                != ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .ok()?,
                )
            {
                return None;
            }
            projected_byte_field(structural_type, path, *field, types)?;
            return Some((
                *psi_operation,
                *result,
                terminal_psi::StructuralArgument {
                    place: *source,
                    access,
                    path: path.clone(),
                },
                None,
                *field,
            ));
        }
        let AbstractOperation::StructuralScalarFieldRead {
            psi_operation: operation,
            result,
            source: place,
            path,
            field,
        } = operation
        else {
            return None;
        };
        let (operation, result, place, field) = (*operation, *result, *place, *field);
        let mut through_reference = false;
        let (structural_type, access) = if let Some(parameter) = function
            .structural_parameters
            .iter()
            .chain(function.block_structural_parameters())
            .find(|parameter| parameter.place == place)
        {
            if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return None;
            }
            (parameter.structural_type, parameter.access)
        } else {
            let (_, result) = super::structural_case::source_result(function, place).ok()?;
            let (structural_type, access) =
                match find_structural_type(types, result.structural_type)
                    .map(|declaration| &declaration.shape)
                {
                    Some(terminal_psi::StructuralTypeShape::ElementView { .. }) => (
                        result.structural_type,
                        terminal_psi::StructuralAccess::SharedBorrow,
                    ),
                    Some(terminal_psi::StructuralTypeShape::Reference {
                        referent,
                        access:
                            access @ (terminal_psi::StructuralAccess::SharedBorrow
                            | terminal_psi::StructuralAccess::MutableBorrow),
                    }) => {
                        let leaf = custody.readable_leaf(place, &[])?;
                        if !leaf.matches_operation_result(result) || leaf.referent != *referent {
                            return None;
                        }
                        through_reference = true;
                        (*referent, *access)
                    }
                    _ => (
                        result.structural_type,
                        terminal_psi::StructuralAccess::Owned,
                    ),
                };
            (structural_type, access)
        };
        let access = terminal_semantics::observed_field_backing_access(
            types.iter(),
            structural_type,
            path,
            access,
        )?;
        let declaration = terminal_semantics::observed_field_declaration(
            types.iter(),
            structural_type,
            path,
            field,
        )?;
        if matches!(
            declaration.field_type,
            terminal_psi::StructuralFieldType::Atomic(_)
                | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
        ) && !matches!(
            access,
            terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::MutableBorrow
        ) {
            return None;
        }
        let (carrier_path, case) = match path.split_last() {
            Some((semantic_vocabulary::CanonicalStructuralPathSegment::Case(case), prefix)) => {
                (prefix, Some(*case))
            }
            _ => (path.as_slice(), None),
        };
        let mut runtime_path = terminal_semantics::canonical_structural_storage_path(
            types.iter(),
            structural_type,
            carrier_path,
        )?;
        match case {
            None => field_read(
                structural_type,
                &runtime_path,
                field,
                result.scalar_type,
                types,
            )?,
            Some(case) => case_field_read(
                structural_type,
                &runtime_path,
                case,
                field,
                result.scalar_type,
                types,
            )?,
        };
        if through_reference {
            runtime_path.insert(0, terminal_psi::StructuralPathSegment::Referent);
        }
        let source = terminal_psi::StructuralArgument {
            place,
            access,
            path: runtime_path,
        };
        Some((operation, result, source, case, field))
    }
}
pub(crate) fn structural_parameters(
    target: &TargetFunction,
) -> Option<&[target_operations::TargetStructuralParameter]> {
    if let Some(abi) = &target.mixed_structural_scalar_abi {
        Some(&abi.structural_parameters)
    } else {
        (!target.graph.parameters.is_empty()).then_some(target.graph.parameters.as_slice())
    }
}
pub(crate) fn structural_contract(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Option<legalized_operations::LegalizedStructuralContract> {
    if let Some(parameters) = structural_parameters(target).or_else(|| {
        (abstracted.structural_parameters.is_empty()
            && optimized.structural_parameters.is_empty()
            && !optimized.declared_places.is_empty()
            && place_declarations::matches_producers(optimized))
        .then_some(&[][..])
    }) {
        return Some(legalized_operations::LegalizedStructuralContract {
            structural_types: plan.structural_types.clone(),
            parameters: abstracted
                .structural_parameters
                .iter()
                .zip(parameters)
                .map(
                    |(semantic, target)| legalized_operations::LegalizedCallUnitParameter {
                        semantic: semantic.clone(),
                        target: target.clone(),
                    },
                )
                .collect(),
            structural_places: optimized.structural_places.clone(),
            entry_claims: abstracted.entry_claims.clone(),
            published_service_ceiling: abstracted.published_service_ceiling.clone(),
            result: abstracted.result.structural().cloned(),
        });
    }
    None
}
pub(crate) mod nodes {
    use super::{
        AbstractOperation, IntegerSign, OptimizationNode, PsiOptimizationFunction, PsiProvenance,
        ScalarType, ValueDefinitionSite, ValueId, scalar_shape,
    };
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::exact_cast_has_native_carriers;
    use crate::legalization::scalar_graph_input::integer_type;
    use crate::legalization::scalar_graph_input::u8_type;
    use crate::legalization::scalar_graph_input::u64_type;
    use crate::legalization::scalar_graph_input::{
        saturating_carrier, supports_wrapping_division, trapping_form, trapping_operands_rejoin,
    };
    use abstract_operations::{
        AbstractStructuralCallResult, IntegerArithmetic, IntegerArithmeticOperator,
        IntegerOverflowPolicy,
    };
    use optimization_unit::OptimizationBlock;
    use semantic_vocabulary::{IntegerType, OperationId};
    pub(crate) enum NodeRejection {
        Malformed,
        UnsupportedFamily,
    }

    pub(crate) fn instruction(node: &OptimizationNode) -> Option<(OperationId, Option<ValueId>)> {
        admit(node).ok()
    }

    pub(crate) fn admit(
        node: &OptimizationNode,
    ) -> Result<(OperationId, Option<ValueId>), NodeRejection> {
        if let AbstractOperation::ActivateTaskRegister {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ReadTaskRegister {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ReloadSegmentDescriptors {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ReadSegmentSelector {
            psi_operation,
            result,
            ..
        } = &node.operation
        {
            return Ok((*psi_operation, Some(result.value)));
        }
        if let AbstractOperation::RegisterAccess {
            psi_operation,
            service,
            event,
            result,
            ..
        } = &node.operation
        {
            if !event.operation().is_admitted()
                || service.is_some() != event.operation().requires_service_authority()
                || event.operation().produces_value() != result.is_some()
            {
                return Err(NodeRejection::Malformed);
            }
            return Ok((*psi_operation, result.map(|result| result.value)));
        }
        if let AbstractOperation::AtomicEvent {
            psi_operation,
            event,
            ..
        } = &node.operation
        {
            if matches!(
                event,
                abstract_operations::AbstractAtomicEvent::Fence { .. }
            ) {
                return Ok((*psi_operation, None));
            }
            let (event, result) =
                super::atomic_access::event(event).ok_or(NodeRejection::UnsupportedFamily)?;
            if !event.ordering_is_legal() {
                return Err(NodeRejection::Malformed);
            }
            return Ok((*psi_operation, result.map(|result| result.value)));
        }
        if let abstract_operations::establish_or_subslice_set_pattern!(psi_operation, ..)
        | AbstractOperation::PortWrite { psi_operation, .. }
        | AbstractOperation::GlobalDescriptorTable { psi_operation, .. }
        | AbstractOperation::EstablishActivationClaim { psi_operation, .. }
        | AbstractOperation::SplitActivationClaim { psi_operation, .. }
        | AbstractOperation::MergeActivationClaim { psi_operation, .. }
        | AbstractOperation::ReleaseActivationClaim { psi_operation, .. }
        | AbstractOperation::EstablishNativeCodeLayout { psi_operation, .. }
        | AbstractOperation::EstablishRecord { psi_operation, .. }
        | AbstractOperation::EstablishReference { psi_operation, .. }
        | AbstractOperation::EstablishAccessReference { psi_operation, .. }
        | AbstractOperation::ReleaseReference { psi_operation, .. }
        | AbstractOperation::EstablishCase { psi_operation, .. }
        | AbstractOperation::CallStructural {
            psi_operation,
            result:
                AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Structural { .. },
            ..
        }
        | AbstractOperation::PrimitiveLocalStore { psi_operation, .. }
        | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. }
        | AbstractOperation::ByteSequenceWrite { psi_operation, .. }
        | AbstractOperation::ElementViewWrite { psi_operation, .. }
        | AbstractOperation::StructuralByteSequenceFieldByteStore { psi_operation, .. }
        | AbstractOperation::ByteSequenceStore { psi_operation, .. }
        | AbstractOperation::StructuralByteSequenceFieldStore { psi_operation, .. }
        | AbstractOperation::StructuralByteViewFieldStore { psi_operation, .. }
        | AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. }
        | AbstractOperation::StructuralLeafCopy { psi_operation, .. }
        | AbstractOperation::MoveStructuralValue { psi_operation, .. }
        | AbstractOperation::TransformContent { psi_operation, .. }
        | AbstractOperation::MoveStructuralField { psi_operation, .. }
        | AbstractOperation::StoreStructuralField { psi_operation, .. }
        | AbstractOperation::EstablishZeroedStructural { psi_operation, .. }
        | AbstractOperation::EstablishStructuralLocal { psi_operation, .. }
        | AbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
        | AbstractOperation::StructuralScalarFieldStore { psi_operation, .. } = &node.operation
        {
            Ok((*psi_operation, None))
        } else if let AbstractOperation::BoundaryCall {
            psi_operation,
            result,
            ..
        } = &node.operation
        {
            Ok((*psi_operation, result.scalar().map(|result| result.value)))
        } else {
            scalar_instruction(node).map(|(operation, result)| (operation, Some(result)))
        }
    }
    fn scalar_instruction(
        node: &OptimizationNode,
    ) -> Result<(OperationId, ValueId), NodeRejection> {
        match &node.operation {
            AbstractOperation::SelectImplementation {
                psi_operation,
                result,
                ..
            } if result.scalar_type == target_operations::selector_carrier_type() => {
                Ok((*psi_operation, result.value))
            }
            AbstractOperation::IeeeFloatCompare {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::FloatResultPolicy {
                psi_operation,
                result,
                ..
            } => Ok((*psi_operation, *result)),
            abstract_operations::scalar_storage_observation_pattern!(psi_operation, result, ..) => {
                Ok((*psi_operation, result.value))
            }
            AbstractOperation::IeeeFloatConstant {
                psi_operation,
                result,
                ..
            } => Ok((*psi_operation, *result)),
            AbstractOperation::BooleanConstant {
                psi_operation,
                result,
                ..
            } => Ok((*psi_operation, *result)),
            AbstractOperation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Scalar(result),
                ..
            } if scalar_shape(result.scalar_type).is_some() => Ok((*psi_operation, result.value)),
            abstract_operations::byte_read_or_field_read_pattern!(psi_operation, result, ..)
                if result.scalar_type == ScalarType::Integer(u8_type()) =>
            {
                Ok((*psi_operation, result.value))
            }
            AbstractOperation::ByteSequenceLength {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::ElementViewLength {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::StructuralByteSequenceFieldLength {
                psi_operation,
                result,
                ..
            } if result.scalar_type == ScalarType::Integer(u64_type()) => {
                Ok((*psi_operation, result.value))
            }
            abstract_operations::byte_read_predicate_equal_pattern!(psi_operation, result, ..)
                if result.scalar_type == ScalarType::Boolean =>
            {
                Ok((*psi_operation, result.value))
            }
            AbstractOperation::ElementViewRead {
                psi_operation,
                result,
                ..
            } if scalar_shape(result.scalar_type).is_some() => Ok((*psi_operation, result.value)),
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } if valid_literal(*scalar_type, *value) => Ok((*psi_operation, *result)),
            AbstractOperation::Call {
                psi_operation,
                result,
                scalar_type,
                ..
            } if scalar_shape(*scalar_type).is_some() => Ok((*psi_operation, *result)),
            AbstractOperation::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | abstract_operations::bitwise_or_xor_pair_pattern!(
                psi_operation,
                result,
                scalar_type,
                ..
            ) if scalar_shape(ScalarType::Integer(*scalar_type)).is_some() => {
                Ok((*psi_operation, *result))
            }
            AbstractOperation::IntegerBitwiseNot {
                psi_operation,
                result,
                scalar_type,
                ..
            } if scalar_shape(ScalarType::Integer(*scalar_type)).is_some() => {
                Ok((*psi_operation, *result))
            }
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                result,
                scalar_type,
                arithmetic,
                ..
            } if integer_arithmetic_admitted(*arithmetic, *scalar_type) => {
                Ok((*psi_operation, *result))
            }
            AbstractOperation::IntegerShift {
                psi_operation,
                result,
                value_type,
                count_type,
                ..
            } if scalar_shape(ScalarType::Integer(*value_type)).is_some()
                && scalar_shape(ScalarType::Integer(*count_type)).is_some() =>
            {
                Ok((*psi_operation, *result))
            }
            AbstractOperation::TrappingInteger {
                psi_operation,
                result,
                scalar_type,
                operand_type,
                operation,
            } if trapping_form(*scalar_type, *operand_type, operation.primitive()).is_some() => {
                Ok((*psi_operation, *result))
            }
            abstract_operations::integer_equal_less_pair_pattern!(psi_operation, result, ..)
            | AbstractOperation::IntegerLessOrEqual {
                psi_operation,
                result,
                ..
            } => Ok((*psi_operation, *result)),
            abstract_operations::boolean_not_or_equal_pair_pattern!(psi_operation, result, ..)
            | AbstractOperation::IntegerWiden {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::ScalarRepresentationView {
                psi_operation,
                result,
                ..
            } => Ok((*psi_operation, *result)),
            AbstractOperation::IntegerExactCast {
                psi_operation,
                result,
                ..
            } => Ok((*psi_operation, *result)),
            AbstractOperation::IntegerConstant { .. }
            | abstract_operations::byte_read_or_field_read_pattern!(..)
            | AbstractOperation::ByteSequenceLength { .. }
            | abstract_operations::byte_read_predicate_equal_pattern!(..)
            | AbstractOperation::StructuralByteSequenceFieldLength { .. } => {
                Err(NodeRejection::Malformed)
            }
            _ => Err(NodeRejection::UnsupportedFamily),
        }
    }
    fn integer_arithmetic_admitted(
        arithmetic: IntegerArithmetic,
        scalar_type: IntegerType,
    ) -> bool {
        match (arithmetic.policy(), arithmetic.operator()) {
            (IntegerOverflowPolicy::Saturating, _) => saturating_carrier(scalar_type).is_some(),
            (
                IntegerOverflowPolicy::Wrapping,
                IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder,
            ) => supports_wrapping_division(scalar_type),
            (IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Exact, _) => {
                scalar_shape(ScalarType::Integer(scalar_type)).is_some()
            }
        }
    }

    fn valid_literal(scalar: ScalarType, value: semantic_vocabulary::IntegerValue) -> bool {
        if let Some(integer) = scalar.as_integer()
            && matches!(integer.bits(), 8 | 16 | 32 | 64)
        {
            return match value {
                semantic_vocabulary::IntegerValue::Unsigned(value) => {
                    integer.sign() == IntegerSign::Unsigned && value < (1_u128 << integer.bits())
                }
                semantic_vocabulary::IntegerValue::Signed(value) => {
                    integer.sign() == IntegerSign::Signed
                        && value >= -(1_i128 << (integer.bits() - 1))
                        && value < (1_i128 << (integer.bits() - 1))
                }
            };
        }
        false
    }
    fn subslice_geometry(
        optimized: &PsiOptimizationFunction,
        source: &terminal_psi::StructuralArgument,
        types: &[terminal_psi::StructuralTypeDeclaration],
        shape: fn(&terminal_psi::StructuralTypeShape) -> bool,
    ) -> bool {
        super::element_views::descriptor_geometry(optimized, source, types).is_some_and(
            |geometry| {
                geometry.readable
                    && types.iter().any(|declaration| {
                        declaration.id == geometry.selected && shape(&declaration.shape)
                    })
            },
        )
    }
    pub(crate) fn validate(
        block: &OptimizationBlock,
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        types: &[terminal_psi::StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<(), LegalizationError> {
        let optimized = function_inputs.function();
        let (terminator, body) = block
            .nodes
            .split_last()
            .ok_or(LegalizationError::custody())?;
        for (position, parameter) in block.parameters.iter().enumerate() {
            if (integer_type(parameter.scalar_type).is_none()
                && !(matches!(
                    parameter.scalar_type,
                    ScalarType::Boolean | ScalarType::IeeeFloat(_)
                ))
                && !(matches!(parameter.scalar_type, ScalarType::Integer(_))
                    && scalar_shape(parameter.scalar_type).is_some()))
                || parameter.site
                    != (ValueDefinitionSite::BlockParameter {
                        block: block.id,
                        position: position as u32,
                    })
            {
                return Err(LegalizationError::custody());
            }
        }
        for (position, node) in body.iter().enumerate() {
            if !node
                .operation
                .runtime_indices()
                .into_iter()
                .all(|(index, _)| {
                    matches!(function_inputs.value_type(index), Some(ScalarType::Integer(integer))
                    if integer.bits() <= 64)
                })
            {
                return Err(LegalizationError::custody());
            }
            let (operation, result) = admit(node).map_err(|rejection| match rejection {
                NodeRejection::Malformed => LegalizationError::custody(),
                NodeRejection::UnsupportedFamily => LegalizationError::UnsupportedScalarOperation {
                    machine: optimized.machine,
                    operation: node.operation.clone(),
                },
            })?;
            if !node.successors.is_empty()
                || node.provenance != [PsiProvenance::Operation(operation)]
                || node.work.is_empty()
                || node
                    .work
                    .iter()
                    .any(|work| work.site != PsiProvenance::Operation(operation))
            {
                return Err(LegalizationError::custody());
            }
            if matches!(
                node.operation,
                AbstractOperation::EstablishPrimitiveLocal { .. }
                    | AbstractOperation::PrimitiveLocalStore { .. }
            ) {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::EstablishByteSequenceConcatenation {
                psi_operation,
                result: output,
                left,
                right,
                ..
            } = &node.operation
            {
                if result.is_some()
                    || !node.definitions.is_empty()
                    || output.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !output.claims.is_empty()
                    || !output.projected_qualifications.is_empty()
                    || !output.qualification_establishments.is_empty()
                    || [left, right].iter().any(|operand| {
                        function_inputs.value_type(operand.length())
                            != Some(ScalarType::Integer(u64_type()))
                    })
                    || !optimized.structural_places.iter().any(|place| {
                        place.id == output.place
                            && place.kind
                                == (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                                    producer: *psi_operation,
                                    structural_type: output.structural_type,
                                })
                    })
                {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::ByteSequenceSubslice {
                source,
                start,
                end,
                length,
                ..
            }
            | AbstractOperation::ElementViewSubslice {
                source,
                start,
                end,
                length,
                ..
            } = &node.operation
            {
                let family: fn(&terminal_psi::StructuralTypeShape) -> bool = if matches!(
                    node.operation,
                    AbstractOperation::ByteSequenceSubslice { .. }
                ) {
                    |shape| matches!(shape, terminal_psi::StructuralTypeShape::ByteSequence(_))
                } else {
                    |shape| matches!(shape, terminal_psi::StructuralTypeShape::ElementView { .. })
                };
                if result.is_some()
                    || !node.definitions.is_empty()
                    || !subslice_geometry(optimized, source, types, family)
                    || [start, end, length].iter().any(|value| {
                        function_inputs.value_type(**value) != Some(ScalarType::Integer(u64_type()))
                    })
                {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::EstablishElementView {
                psi_operation,
                result: op_result,
                destination,
                ..
            } = &node.operation
            {
                if result.is_some()
                    || !node.definitions.is_empty()
                    || !optimized.structural_places.iter().any(|place| {
                        place.id == *destination
                            && place.kind
                                == (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                                    producer: *psi_operation,
                                    structural_type: op_result.structural_type,
                                })
                    })
                {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::ByteSequenceStore { length, .. }
            | AbstractOperation::StructuralByteSequenceFieldStore { length, .. } =
                &node.operation
            {
                if result.is_some()
                    || !node.definitions.is_empty()
                    || function_inputs.value_type(*length) != Some(ScalarType::Integer(u64_type()))
                {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::StructuralByteViewFieldStore { .. } = &node.operation {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::StructuralByteSequenceFieldByteStore {
                index,
                value,
                length,
                ..
            } = &node.operation
            {
                if result.is_some()
                    || !node.definitions.is_empty()
                    || function_inputs.value_type(*index) != Some(ScalarType::Integer(u64_type()))
                    || function_inputs.value_type(*length) != Some(ScalarType::Integer(u64_type()))
                    || function_inputs.value_type(*value) != Some(ScalarType::Integer(u8_type()))
                {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::ByteSequenceWrite {
                destination,
                index,
                value,
                length,
                ..
            } = &node.operation
            {
                if result.is_some()
                    || !node.definitions.is_empty()
                    || !super::byte_views::writable_bytes(optimized, *destination, types)
                    || function_inputs.value_type(*index) != Some(ScalarType::Integer(u64_type()))
                    || function_inputs.value_type(*length) != Some(ScalarType::Integer(u64_type()))
                    || function_inputs.value_type(*value) != Some(ScalarType::Integer(u8_type()))
                {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::ElementViewWrite {
                path,
                destination,
                index,
                value,
                length,
                ..
            } = &node.operation
            {
                let geometry =
                    super::element_views::descriptor_geometry(optimized, destination, types);
                let mutable_view = destination.access
                    == terminal_psi::StructuralAccess::MutableBorrow
                    && geometry.as_ref().is_some_and(|geometry| geometry.readable);
                let element =
                    geometry.and_then(|geometry| {
                        types
                            .iter()
                            .find_map(|declaration| match declaration.shape {
                                terminal_psi::StructuralTypeShape::ElementView {
                                    element, ..
                                } if declaration.id == geometry.selected => Some(element),
                                _ => None,
                            })
                    });
                if result.is_some()
                    || !node.definitions.is_empty()
                    || !mutable_view
                    || function_inputs.value_type(*index) != Some(ScalarType::Integer(u64_type()))
                    || function_inputs.value_type(*length) != Some(ScalarType::Integer(u64_type()))
                    || element.and_then(|element| {
                        terminal_semantics::element_view_leaf_scalar(types, element, path)
                    }) != function_inputs.value_type(*value)
                    || function_inputs
                        .value_type(*value)
                        .and_then(scalar_shape)
                        .is_none()
                {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::StructuralScalarFieldStore {
                destination, value, ..
            }
            | AbstractOperation::WriteOnlyPrimitiveStore {
                destination, value, ..
            } = &node.operation
            {
                let declared = match destination {
                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
                        (optimized.structural_parameters.contains(parameter)
                            || matches!(
                                parameter.access,
                                terminal_psi::StructuralAccess::Owned
                                    | terminal_psi::StructuralAccess::MutableBorrow
                                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
                            ) && optimized
                                .blocks
                                .iter()
                                .any(|block| block.structural_parameters.contains(parameter)))
                            && matches!(
                                parameter.access,
                                terminal_psi::StructuralAccess::Owned
                                    | terminal_psi::StructuralAccess::MutableBorrow
                                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
                            )
                    }
                    abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                        producer,
                        result,
                    } => super::structural_case::source_result(optimized, result.place).is_ok_and(
                        |(operation, declared)| operation == *producer && declared == result,
                    ),
                    abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                        block,
                        parameter,
                    } => {
                        matches!(
                            super::structural_case::source_owner(optimized, parameter.place),
                            Ok(legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                                block: owner,
                                declaration,
                            }) if owner == *block && declaration == *parameter
                        )
                    }
                };
                let preserved = match &node.operation {
                    AbstractOperation::StructuralScalarFieldStore { path, field, .. } => {
                        terminal_semantics::projected_field_declaration(
                            types.iter(),
                            destination.structural_type(),
                            path,
                            *field,
                        )
                        .is_some_and(|field| {
                            let mut written = path.clone();
                            written
                                .push(terminal_psi::StructuralPathSegment::Field(field.identity));
                            optimization_unit_semantics::structural_write_preserves_content(
                                optimized,
                                destination.place(),
                                &written,
                                types,
                                domains,
                                identities,
                            )
                        })
                    }
                    AbstractOperation::WriteOnlyPrimitiveStore {
                        path,
                        range_obligation,
                        ..
                    } => {
                        terminal_semantics::primitive_store_restriction(
                            types.iter(),
                            domains.iter(),
                            destination.structural_type(),
                            destination.qualifications(),
                            destination.projected_qualifications(),
                            destination.additional_input_qualifications(),
                            path,
                        )
                        .is_some_and(|restriction| {
                            restriction.is_some() == range_obligation.is_some()
                        }) && terminal_semantics::store_preserves_content(
                            types.iter(),
                            domains.iter(),
                            identities,
                            destination.structural_type(),
                            destination.place(),
                            optimized.content_entry_claims.iter(),
                            path,
                        )
                    }
                    _ => false,
                };
                let refusal = if result.is_some() || !node.definitions.is_empty() {
                    Some("scalar store unexpectedly defines a result")
                } else if !declared {
                    Some("scalar store destination does not rejoin its declaration")
                } else if !preserved {
                    Some("scalar store does not preserve destination memberships or content")
                } else if function_inputs.value_type(value.value) != Some(value.scalar_type) {
                    Some("scalar store value disagrees with its declared type")
                } else if scalar_shape(value.scalar_type).is_none() {
                    Some("scalar store value has no native carrier")
                } else {
                    None
                };
                if let Some(reason) = refusal {
                    return Err(LegalizationError::InvalidScalarStore {
                        machine: optimized.machine,
                        operation,
                        reason,
                    });
                }
                continue;
            }
            if let AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                ..
            } = &node.operation
            {
                if result.is_some()
                    || !node.definitions.is_empty()
                    || super::literals::declaration_producer(optimized, place.id)
                        != Some((*psi_operation, structural_type.id))
                {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if matches!(
                node.operation,
                AbstractOperation::EstablishActivationClaim { .. }
                    | AbstractOperation::SplitActivationClaim { .. }
                    | AbstractOperation::MergeActivationClaim { .. }
                    | AbstractOperation::ReleaseActivationClaim { .. }
                    | AbstractOperation::EstablishNativeCodeLayout { .. }
                    | AbstractOperation::EstablishRecord { .. }
                    | AbstractOperation::EstablishReference { .. }
                    | AbstractOperation::EstablishAccessReference { .. }
                    | AbstractOperation::ReleaseReference { .. }
                    | AbstractOperation::EstablishStructuralArray { .. }
                    | AbstractOperation::EstablishScalarArray { .. }
                    | AbstractOperation::EstablishCase { .. }
                    | AbstractOperation::StructuralLeafCopy { .. }
                    | AbstractOperation::MoveStructuralValue { .. }
                    | AbstractOperation::TransformContent { .. }
                    | abstract_operations::move_or_store_structural_field_pattern!()
                    | AbstractOperation::EstablishTrivialAffineLocal { .. }
                    | AbstractOperation::EstablishZeroedStructural { .. }
                    | AbstractOperation::EstablishStructuralLocal { .. }
                    | AbstractOperation::CallStructural {
                        result: AbstractStructuralCallResult::Structural { .. },
                        ..
                    }
            ) {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Unit,
                ..
            } = &node.operation
            {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::BoundaryCall {
                arguments,
                result: boundary_result,
                ..
            } = &node.operation
            {
                if arguments
                    .iter()
                    .any(|value| function_inputs.value_type(*value).is_none())
                {
                    return Err(LegalizationError::custody());
                }
                if boundary_result.scalar().is_none() {
                    if result.is_some() || !node.definitions.is_empty() {
                        return Err(LegalizationError::custody());
                    }
                    continue;
                }
            }
            if let AbstractOperation::RegisterAccess { result: None, .. } = &node.operation {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if matches!(
                node.operation,
                AbstractOperation::PortWrite { .. }
                    | AbstractOperation::GlobalDescriptorTable { .. }
            ) {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            if let AbstractOperation::AtomicEvent { event, .. } = &node.operation {
                if matches!(
                    event,
                    abstract_operations::AbstractAtomicEvent::Fence { .. }
                ) {
                    if result.is_some() || !node.definitions.is_empty() {
                        return Err(LegalizationError::custody());
                    }
                    continue;
                }
                let (_, observed) =
                    super::atomic_access::event(event).ok_or(LegalizationError::custody())?;
                if observed.is_none() {
                    if result.is_some() || !node.definitions.is_empty() {
                        return Err(LegalizationError::custody());
                    }
                    continue;
                }
            }
            let [definition] = node.definitions.as_slice() else {
                return Err(LegalizationError::custody());
            };
            if Some(definition.value) != result
                || definition.site
                    != (ValueDefinitionSite::Node {
                        block: block.id,
                        node: position as u32,
                    })
            {
                return Err(LegalizationError::custody());
            }
            let expected_type = match &node.operation {
                AbstractOperation::ReadTaskRegister { result, .. }
                | AbstractOperation::ReadSegmentSelector { result, .. } => {
                    let selector_type = ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 16).expect("U16"),
                    );
                    if result.scalar_type != selector_type {
                        return Err(LegalizationError::custody());
                    }
                    selector_type
                }
                AbstractOperation::ActivateTaskRegister {
                    result, selector, ..
                }
                | AbstractOperation::ReloadSegmentDescriptors {
                    result, selector, ..
                } => {
                    if result.scalar_type != ScalarType::Integer(u8_type())
                        || function_inputs.value_type(*selector)
                            != Some(ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 16).expect("U16"),
                            ))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(u8_type())
                }
                AbstractOperation::RegisterAccess {
                    result: Some(result),
                    ..
                } => result.scalar_type,
                AbstractOperation::SelectImplementation { result, .. } => result.scalar_type,
                AbstractOperation::StorageAddress { result, .. } => {
                    if !matches!(result.scalar_type, ScalarType::Integer(integer) if integer.is_address())
                    {
                        return Err(LegalizationError::custody());
                    }
                    result.scalar_type
                }
                AbstractOperation::AtomicEvent { event, .. } => {
                    super::atomic_access::event(event)
                        .and_then(|(_, result)| result)
                        .ok_or(LegalizationError::custody())?
                        .scalar_type
                }
                AbstractOperation::FloatResultPolicy { format, policy, .. } => {
                    let scalar_type = ScalarType::IeeeFloat(*format);
                    if !policy
                        .accepts_operand_types(*format, |value| function_inputs.value_type(value))
                    {
                        return Err(LegalizationError::custody());
                    }
                    scalar_type
                }
                AbstractOperation::IeeeFloatCompare {
                    format,
                    left,
                    right,
                    ..
                } => {
                    if function_inputs.value_type(*left) != Some(ScalarType::IeeeFloat(*format))
                        || function_inputs.value_type(*right)
                            != Some(ScalarType::IeeeFloat(*format))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Boolean
                }
                AbstractOperation::IeeeFloatConstant { value, .. } => {
                    ScalarType::IeeeFloat(value.format())
                }
                AbstractOperation::BooleanConstant { .. } => ScalarType::Boolean,
                AbstractOperation::CallStructural {
                    result: AbstractStructuralCallResult::Scalar(result),
                    ..
                }
                | AbstractOperation::BoundaryCall {
                    result: abstract_operations::AbstractBoundaryResult::Scalar(result),
                    ..
                }
                | AbstractOperation::PrimitiveScalarRead { result, .. }
                | AbstractOperation::StructuralScalarFieldRead { result, .. }
                | AbstractOperation::StructuralByteSequenceFieldLength { result, .. }
                | AbstractOperation::StructuralCaseMembership { result, .. } => result.scalar_type,
                AbstractOperation::ByteSequenceRead {
                    source,
                    index,
                    length,
                    ..
                } => {
                    if !super::byte_views::contains_readable_bytes(optimized, *source, types)
                        || function_inputs.value_type(*index)
                            != Some(ScalarType::Integer(u64_type()))
                        || function_inputs.value_type(*length)
                            != Some(ScalarType::Integer(u64_type()))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(u8_type())
                }
                AbstractOperation::ByteSequenceLength { source, .. } => {
                    if super::element_views::byte_length_geometry(optimized, source, types)
                        .is_none()
                        && super::element_views::fixed_byte_length(optimized, source, types)
                            .is_none()
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(u64_type())
                }
                AbstractOperation::StructuralByteSequenceFieldRead { index, length, .. } => {
                    if function_inputs.value_type(*index) != Some(ScalarType::Integer(u64_type()))
                        || function_inputs.value_type(*length)
                            != Some(ScalarType::Integer(u64_type()))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(u8_type())
                }
                AbstractOperation::ElementViewRead {
                    result,
                    source,
                    index,
                    length,
                    ..
                } => {
                    if !super::byte_views::contains_view(optimized, *source, types)
                        || function_inputs.value_type(*index)
                            != Some(ScalarType::Integer(u64_type()))
                        || function_inputs.value_type(*length)
                            != Some(ScalarType::Integer(u64_type()))
                    {
                        return Err(LegalizationError::custody());
                    }
                    result.scalar_type
                }
                AbstractOperation::ElementViewLength { source, .. } => {
                    if super::element_views::length_geometry(optimized, source, types).is_none() {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(u64_type())
                }
                AbstractOperation::ByteSequencePredicate { operand, .. } => {
                    if function_inputs.value_type(operand.length())
                        != Some(ScalarType::Integer(u64_type()))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Boolean
                }
                AbstractOperation::ByteSequenceEqual { left, right, .. } => {
                    for operand in [left, right] {
                        if function_inputs.value_type(operand.length())
                            != Some(ScalarType::Integer(u64_type()))
                        {
                            return Err(LegalizationError::custody());
                        }
                    }
                    ScalarType::Boolean
                }
                AbstractOperation::IntegerConstant { scalar_type, .. }
                | AbstractOperation::Call { scalar_type, .. } => *scalar_type,
                AbstractOperation::IntegerBitwiseAnd {
                    scalar_type,
                    left,
                    right,
                    ..
                }
                | abstract_operations::bitwise_or_xor_pair_pattern!(scalar_type, left, right, ..) =>
                {
                    if function_inputs.value_type(*left) != Some(ScalarType::Integer(*scalar_type))
                        || function_inputs.value_type(*right)
                            != Some(ScalarType::Integer(*scalar_type))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(*scalar_type)
                }
                AbstractOperation::IntegerBitwiseNot {
                    scalar_type,
                    operand,
                    ..
                } => {
                    if function_inputs.value_type(*operand)
                        != Some(ScalarType::Integer(*scalar_type))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(*scalar_type)
                }
                AbstractOperation::IntegerArithmetic {
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                    ..
                } => {
                    if arithmetic.policy() != IntegerOverflowPolicy::Exact
                        && (!integer_arithmetic_admitted(*arithmetic, *scalar_type)
                            || function_inputs.value_type(*left)
                                != Some(ScalarType::Integer(*scalar_type))
                            || function_inputs.value_type(*right)
                                != Some(ScalarType::Integer(*scalar_type)))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(*scalar_type)
                }
                AbstractOperation::TrappingInteger {
                    scalar_type,
                    operand_type,
                    operation,
                    ..
                } => {
                    if trapping_form(*scalar_type, *operand_type, operation.primitive()).is_none()
                        || !trapping_operands_rejoin(
                            optimized,
                            *scalar_type,
                            *operand_type,
                            *operation,
                        )
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(*scalar_type)
                }
                AbstractOperation::IntegerShift {
                    value_type: shift_value_type,
                    count_type,
                    value,
                    count,
                    ..
                } => {
                    if scalar_shape(ScalarType::Integer(*shift_value_type)).is_none()
                        || scalar_shape(ScalarType::Integer(*count_type)).is_none()
                        || function_inputs.value_type(*value)
                            != Some(ScalarType::Integer(*shift_value_type))
                        || function_inputs.value_type(*count)
                            != Some(ScalarType::Integer(*count_type))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(*shift_value_type)
                }
                abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
                | AbstractOperation::IntegerLessOrEqual { left, right, .. } => {
                    for operand in [*left, *right] {
                        let Some(ScalarType::Integer(carrier)) =
                            function_inputs.value_type(operand)
                        else {
                            return Err(LegalizationError::custody());
                        };
                        if scalar_shape(ScalarType::Integer(carrier)).is_none() {
                            return Err(LegalizationError::custody());
                        }
                    }
                    ScalarType::Boolean
                }
                AbstractOperation::BooleanEqual { left, right, .. } => {
                    if function_inputs.value_type(*left) != Some(ScalarType::Boolean)
                        || function_inputs.value_type(*right) != Some(ScalarType::Boolean)
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Boolean
                }
                AbstractOperation::BooleanNot { operand, .. } => {
                    if function_inputs.value_type(*operand) != Some(ScalarType::Boolean) {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Boolean
                }
                AbstractOperation::IntegerWiden {
                    operand,
                    source_type,
                    target_type,
                    ..
                } => {
                    if !matches!(source_type.bits(), 8 | 16 | 32 | 64)
                        || target_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                        || !matches!(target_type.bits(), 16 | 32 | 64)
                        || !source_type.can_widen_to(*target_type)
                        || function_inputs.value_type(*operand)
                            != Some(ScalarType::Integer(*source_type))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(*target_type)
                }
                AbstractOperation::ScalarRepresentationView {
                    operand,
                    source_type,
                    target_type,
                    ..
                } => {
                    if terminal_semantics::ScalarRepresentationViewPair::classify(
                        *source_type,
                        *target_type,
                    )
                    .is_none()
                        || function_inputs.value_type(*operand) != Some(*source_type)
                    {
                        return Err(LegalizationError::custody());
                    }
                    *target_type
                }
                AbstractOperation::IntegerExactCast {
                    operand,
                    source_type,
                    target_type,
                    ..
                } => {
                    if !exact_cast_has_native_carriers(*source_type, *target_type)
                        || function_inputs.value_type(*operand)
                            != Some(ScalarType::Integer(*source_type))
                    {
                        return Err(LegalizationError::custody());
                    }
                    ScalarType::Integer(*target_type)
                }
                _ => {
                    return Err(LegalizationError::UnsupportedScalarOperation {
                        machine: optimized.machine,
                        operation: node.operation.clone(),
                    });
                }
            };
            if definition.scalar_type != expected_type {
                return Err(LegalizationError::custody());
            }
        }
        super::control::validate(terminator, body, function_inputs, types)
    }
}
pub(crate) use crate::legalization_target as target;
#[allow(unused_imports)]
pub(crate) use header::function_abi;
pub(crate) use nodes::instruction;
#[allow(unused_imports)]
pub(crate) use target::validate_target;

pub(crate) fn u32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).expect("U32")
}
pub(crate) fn u8_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).expect("U8")
}
pub(crate) fn scalar_shape(scalar: ScalarType) -> Option<ValueShape> {
    crate::selection::scalar_shape(scalar)
}

pub(crate) fn saturating_carrier(integer: IntegerType) -> Option<SaturatingCarrier> {
    SaturatingCarrier::from_integer(integer)
}

pub(crate) fn trapping_form(
    scalar_type: IntegerType,
    operand_type: IntegerType,
    primitive: terminal_psi::TrappingIntegerPrimitive,
) -> Option<TrappingForm> {
    use terminal_psi::TrappingIntegerPrimitive as P;
    let carrier = saturating_carrier(scalar_type)?;
    let source = saturating_carrier(operand_type)?;
    let operation = match primitive {
        P::Add | P::Subtract | P::Multiply | P::Divide | P::Remainder if source != carrier => {
            return None;
        }
        P::Add => TrappingOperation::Add,
        P::Subtract => TrappingOperation::Subtract,
        P::Multiply => TrappingOperation::Multiply,
        P::Divide => TrappingOperation::Divide,
        P::Remainder => TrappingOperation::Remainder,
        P::ShiftLeft => TrappingOperation::ShiftLeft,
        P::ShiftRight => TrappingOperation::ShiftRight,
        P::Convert => TrappingOperation::Convert {
            source: source.sign(),
        },
    };
    Some(TrappingForm { operation, carrier })
}

pub(crate) fn trapping_operands_rejoin(
    optimized: &PsiOptimizationFunction,
    scalar_type: IntegerType,
    operand_type: IntegerType,
    operation: terminal_psi::TrappingIntegerOperation,
) -> bool {
    use terminal_psi::TrappingIntegerOperation as T;
    let typed =
        |value, expected| value_type(optimized, value) == Some(ScalarType::Integer(expected));
    match operation {
        terminal_psi::trapping_arith_set_pattern!(left, right, ..) => {
            operand_type == scalar_type && typed(left, scalar_type) && typed(right, scalar_type)
        }
        T::ShiftLeft { value, count } | T::ShiftRight { value, count } => {
            typed(value, scalar_type) && typed(count, operand_type)
        }
        T::Convert { operand } => typed(operand, operand_type),
    }
}

pub(crate) fn supports_wrapping_division(integer: IntegerType) -> bool {
    integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
        && scalar_shape(ScalarType::Integer(integer)).is_some()
}

pub(crate) fn exact_cast_has_native_carriers(source: IntegerType, target: IntegerType) -> bool {
    scalar_shape(ScalarType::Integer(source)).is_some()
        && scalar_shape(ScalarType::Integer(target)).is_some()
        && source.can_exact_cast_to(target)
}

pub(crate) fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("I32")
}
pub(crate) fn u64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).expect("U64")
}

pub(crate) type BlockEntryCustody =
    std::collections::BTreeMap<semantic_vocabulary::BlockId, reference_custody::Custody>;

pub(crate) fn unique_row(
    function: &TargetFunction,
    mut matches: impl FnMut(&TargetUnitOperation) -> bool,
) -> Result<Option<&TargetUnitOperation>, LegalizationError> {
    let mut rows = function.graph.operations().filter(|row| matches(row));
    let row = rows.next();
    if rows.next().is_some() {
        return Err(LegalizationError::custody());
    }
    Ok(row)
}

pub(crate) fn match_input(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    descriptor_inputs: &crate::legalization_descriptor_inputs::DescriptorInputs<'_>,
) -> Result<(), LegalizationError> {
    let native = descriptor_inputs.native();
    let plan = descriptor_inputs.plan();
    let unit = descriptor_inputs.unit();
    primitive_locals::validate(optimized, &unit.structural_types)?;
    let call_plan = if aggregate_results::uses(optimized) {
        aggregate_results::header(target, abstracted, optimized, native.target, plan)?
    } else if structural_parameters(target).is_some() {
        byte_views::validate(target, abstracted, optimized, native.target, plan)?
    } else {
        function_abi(native.target, target, abstracted, optimized)?
    };
    if optimized.blocks.is_empty()
        || optimized.entry != abstracted.entry
        || abstracted.block_entries.len() != optimized.blocks.len()
        || abstracted.block_entries[0].operation_offset != 0
    {
        return Err(LegalizationError::custody());
    }
    let function_inputs = crate::legalization_function_inputs::FunctionInputs::new(optimized);
    for (position, (entry, block)) in abstracted
        .block_entries
        .iter()
        .zip(&optimized.blocks)
        .enumerate()
    {
        let end = abstracted
            .block_entries
            .get(position + 1)
            .map_or(abstracted.operations.len(), |next| next.operation_offset);
        let operations = abstracted
            .operations
            .get(entry.operation_offset..end)
            .ok_or(LegalizationError::custody())?;
        if entry.block != block.id
            || entry.structural_parameters != block.structural_parameters
            || entry.parameters.len() != block.parameters.len()
            || entry
                .parameters
                .iter()
                .zip(&block.parameters)
                .any(|(declared, actual)| {
                    declared.value != actual.value || declared.scalar_type != actual.scalar_type
                })
            || operations.len() != block.nodes.len()
            || operations
                .iter()
                .zip(&block.nodes)
                .any(|(left, right)| left != &right.operation)
        {
            return Err(LegalizationError::custody());
        }
        nodes::validate(
            block,
            &function_inputs,
            &plan.structural_types,
            &unit.structural_domains,
            &unit.identities,
        )?;
    }
    let entry = optimized
        .block(optimized.entry)
        .ok_or(LegalizationError::custody())?;
    if !entry.parameters.is_empty() || !entry.structural_parameters.is_empty() {
        return Err(LegalizationError::custody());
    }
    if optimized
        .parameters
        .iter()
        .zip(&call_plan.parameters)
        .any(|(parameter, placement)| {
            optimized.block_nodes()
                .any(|node| node.uses.iter().any(|used| used.value == parameter.value))
                && !(Some(placement.shape) == scalar_shape(parameter.scalar_type)
                    && (matches!(placement.locations.as_slice(), [ValueLocation::Register {value_byte_offset:0,byte_size,..}] if *byte_size == placement.shape.byte_size)
                        || scalar_stack(placement)))
        })
    {
        return Err(LegalizationError::custody());
    }
    for node in optimized.block_nodes() {
        if let abstract_operations::byte_literal_establish_pair_pattern!(structural_type, ..) =
            &node.operation
            && (!plan.structural_types.contains(structural_type)
                || !unit.structural_types.contains(structural_type))
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractOperation::EstablishZeroedStructural { result, .. }
        | AbstractOperation::EstablishStructuralLocal { result, .. } = &node.operation
            && (!terminal_psi::find_structural_type(&plan.structural_types, result.structural_type)
                .is_some()
                || !terminal_psi::find_structural_type(
                    &unit.structural_types,
                    result.structural_type,
                )
                .is_some())
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractOperation::EstablishElementView {
            result, element, ..
        } = &node.operation
            && (!unit.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && match &declaration.shape {
                        terminal_psi::StructuralTypeShape::ElementView {
                            element: declared,
                            ..
                        } => declared == element,
                        terminal_psi::StructuralTypeShape::ByteSequence(_) => {
                            declaration.shape.is_borrowed_byte_carrier()
                                && unit.structural_types.iter().any(|element_declaration| {
                                    element_declaration.id == *element
                                        && matches!(
                                            element_declaration.shape,
                                            terminal_psi::StructuralTypeShape::PrimitiveScalar(
                                                semantic_vocabulary::ScalarType::Integer(integer)
                                            ) if Ok(integer)
                                                == semantic_vocabulary::IntegerType::new(
                                                    semantic_vocabulary::IntegerSign::Unsigned,
                                                    8,
                                                )
                                        )
                                })
                        }
                        _ => false,
                    }
            }) || !terminal_psi::find_structural_type(
                &plan.structural_types,
                result.structural_type,
            )
            .is_some()
                || !terminal_psi::find_structural_type(&unit.structural_types, *element).is_some())
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractOperation::StructuralLeafCopy { result, .. }
        | AbstractOperation::MoveStructuralValue { result, .. }
        | AbstractOperation::TransformContent { result, .. }
        | AbstractOperation::MoveStructuralField { result, .. } = &node.operation
            && (!terminal_psi::find_structural_type(&plan.structural_types, result.structural_type)
                .is_some()
                || !terminal_psi::find_structural_type(
                    &unit.structural_types,
                    result.structural_type,
                )
                .is_some())
        {
            return Err(LegalizationError::custody());
        }
        let obligation_custody_missing = |psi_operation: OperationId, obligation: ObligationId| {
            accepted_fact(unit, optimized, psi_operation, obligation).is_err()
                || !references_obligation(optimized, psi_operation, obligation)
        };
        let integer_obligation = match &node.operation {
            AbstractOperation::IntegerShift {
                psi_operation,
                policy: IntegerShiftPolicy::Exact { obligation },
                ..
            } => Some((*psi_operation, *obligation)),
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                arithmetic,
                ..
            } => arithmetic
                .obligation()
                .map(|obligation| (*psi_operation, obligation)),
            _ => None,
        };
        if let Some((psi_operation, obligation)) = integer_obligation
            && obligation_custody_missing(psi_operation, obligation)
        {
            return Err(LegalizationError::custody());
        }
        if let abstract_operations::byte_sequence_write_pair_pattern!(psi_operation, obligation, ..)
        | AbstractOperation::StructuralByteSequenceFieldRead {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::EstablishByteSequenceConcatenation {
            psi_operation,
            capacity_obligation: obligation,
            ..
        }
        | abstract_operations::byte_sequence_store_pair_pattern!(psi_operation, obligation, ..)
        | AbstractOperation::ByteSequenceRead {
            psi_operation,
            obligation,
            ..
        }
        | abstract_operations::element_view_or_byte_subslice_pattern!(
            psi_operation,
            obligation,
            ..
        )
        | abstract_operations::element_view_read_write_pair_pattern!(
            psi_operation,
            obligation,
            ..
        ) = &node.operation
            && obligation_custody_missing(*psi_operation, *obligation)
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractOperation::Call {
            callee, arguments, ..
        } = &node.operation
        {
            let call = callee_plan(*callee, native, plan, unit)?;
            if call.result.is_none()
                || call.parameters.len() != arguments.len()
                || !call
                    .parameters
                    .iter()
                    .all(|placement| scalar_register(placement) || scalar_stack(placement))
            {
                return Err(LegalizationError::custody());
            }
        }
        if let AbstractOperation::CallStructural {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } = &node.operation
        {
            let call = callee_plan(*callee, native, plan, unit)?;
            let called = call_signature::signature(*callee, plan)?;
            if call.result.is_some()
                != matches!(
                    node.operation,
                    AbstractOperation::CallStructural {
                        result: AbstractStructuralCallResult::Scalar(_),
                        ..
                    }
                )
                || call.parameters.len() != arguments.len() + structural_arguments.len()
                || called.scalar_parameter_count() != arguments.len()
                || called.structural_parameter_count() != structural_arguments.len()
                || arguments.iter().enumerate().zip(&call.parameters).any(
                    |((position, value), placement)| {
                        function_inputs.value_type(*value) != called.scalar_parameter(position)
                            || called.scalar_parameter(position).and_then(scalar_shape)
                                != Some(placement.shape)
                            || !(scalar_register(placement) || scalar_stack(placement))
                    },
                )
            {
                return Err(LegalizationError::custody());
            }
            for (position, argument) in structural_arguments.iter().enumerate() {
                if argument
                    .path
                    .contains(&terminal_psi::StructuralPathSegment::Referent)
                {
                    continue;
                }
                structural_call::argument_at(
                    argument,
                    position,
                    *psi_operation,
                    optimized,
                    called,
                    &call,
                    native,
                    plan,
                    &reference_custody::Custody::default(),
                )
                .map_err(|error| error.attributed(optimized.machine, Some(*psi_operation)))?;
            }
        }
    }
    validate_target(target, abstracted, &function_inputs, descriptor_inputs).map(|_| ())
}
pub(crate) fn callee_plan(
    callee: impl Into<abstract_operations::AbstractCallTarget>,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<CallPlan, LegalizationError> {
    let target = callee.into();
    let Some(callee) = target.machine() else {
        let signature = call_signature::signature(target, plan)?;
        let mut parameters = (0..signature.scalar_parameter_count())
            .map(|position| scalar_shape(signature.scalar_parameter(position)?))
            .collect::<Option<Vec<_>>>()
            .ok_or(LegalizationError::custody())?;
        for position in 0..signature.structural_parameter_count() {
            let formal = signature
                .structural_parameter(position)
                .ok_or(LegalizationError::custody())?;
            parameters.push(
                crate::structural_inputs::parameter_shape(formal, &plan.structural_types)
                    .ok_or(LegalizationError::custody())?,
            );
        }
        let result = if let Some(result) = signature.structural_result() {
            Some(aggregate_results::type_home_layout(result.structural_type, plan)?.shape())
        } else {
            signature
                .scalar_result()
                .map(|result| scalar_shape(result).ok_or(LegalizationError::custody()))
                .transpose()?
        };
        return evaluate_call_plan(
            CallingPolicy::native_for_target(native.target),
            &CallSignature { parameters, result },
        )
        .map_err(|_| LegalizationError::custody());
    };
    let targets = native
        .functions
        .iter()
        .filter(|function| function.machine == callee)
        .collect::<Vec<_>>();
    let abstracts = plan
        .functions
        .iter()
        .filter(|function| function.machine == callee)
        .collect::<Vec<_>>();
    let optimized = unit
        .functions
        .iter()
        .filter(|function| function.machine == callee)
        .collect::<Vec<_>>();
    let ([target], [abstracted], [optimized]) = (
        targets.as_slice(),
        abstracts.as_slice(),
        optimized.as_slice(),
    ) else {
        return Err(LegalizationError::custody());
    };
    if !aggregate_results::uses(optimized)
        && (!matches!(abstracted.result, AbstractFunctionResult::Unit)
            && !matches!(abstracted.result, AbstractFunctionResult::Scalar(result) if scalar_shape(result.scalar_type).is_some())
            || abstracted
                .parameters
                .iter()
                .any(|parameter| scalar_shape(parameter.scalar_type).is_none()))
    {
        return Err(LegalizationError::custody());
    }
    let call_plan = if aggregate_results::uses(optimized) {
        aggregate_results::header(target, abstracted, optimized, native.target, plan)?
    } else if structural_parameters(target).is_some() {
        byte_views::validate(target, abstracted, optimized, native.target, plan)?
    } else {
        function_abi(native.target, target, abstracted, optimized)?
    };
    if !call_plan.parameters.iter().enumerate().all(|(position, placement)| {
        scalar_register(placement)
            || position.checked_sub(abstracted.parameters.len())
                .and_then(|position| abstracted.structural_parameters.get(position))
                .is_some_and(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned
                    && crate::structural_inputs::structural_reference_input::parameter_shape(parameter, &plan.structural_types) == Some(placement.shape)
                    && (placement.locations.iter().all(|location| matches!(location,
                        calling_conventions::register_or_stack_pair_pattern!(byte_size:1..=8, ..)))
                        || crate::structural_inputs::structural_unit_input::owned_indirect_pointer(parameter, placement).is_some()))
            || scalar_stack(placement)
            || crate::structural_inputs::structural_reference_input::stack_pointer_offset(placement).is_some()
            || matches!(placement.shape.class, calling_conventions::ValueClass::BorrowedReference | calling_conventions::ValueClass::BorrowedScalarReference)
                && matches!(placement.locations.as_slice(),
                    [ValueLocation::Register { value_byte_offset: 0, byte_size: 8, .. }])
            || matches!(placement.shape.class, calling_conventions::ValueClass::BorrowedReference | calling_conventions::ValueClass::BorrowedScalarReference)
                && matches!(
                    placement.locations.as_slice(),
                    [ValueLocation::Indirect {
                        pointer: calling_conventions::IndirectPointerLocation::Register(_),
                        copy_stack_byte_offset: None,
                        byte_size,
                        alignment,
                    }] if *byte_size == placement.shape.byte_size && *alignment == placement.shape.alignment
                )
    }) {
        return Err(LegalizationError::custody());
    }
    Ok(call_plan)
}
pub(crate) fn scalar_carrier_shape(shape: ValueShape) -> bool {
    [
        ValueShape::float(4),
        ValueShape::float(8),
        ValueShape::integer(1, 1),
        ValueShape::integer(2, 2),
        ValueShape::integer(4, 4),
        ValueShape::integer(8, 8),
    ]
    .contains(&shape)
}

pub(crate) fn scalar_register(placement: &ValuePlacement) -> bool {
    scalar_carrier_shape(placement.shape)
        && matches!(placement.locations.as_slice(),
            [ValueLocation::Register {value_byte_offset: 0, byte_size, ..}]
            if *byte_size == placement.shape.byte_size)
}

pub(crate) fn scalar_stack(placement: &ValuePlacement) -> bool {
    scalar_carrier_shape(placement.shape)
        && matches!(placement.locations.as_slice(),
            [ValueLocation::Stack { stack_byte_offset, value_byte_offset: 0, byte_size, alignment }]
                if *byte_size == placement.shape.byte_size && *alignment >= placement.shape.alignment
                    && alignment.is_power_of_two() && stack_byte_offset.is_multiple_of(u32::from(*alignment)))
}
pub(crate) fn i64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 64).expect("I64")
}
pub(crate) fn integer_type(scalar: ScalarType) -> Option<IntegerType> {
    match scalar {
        ScalarType::Integer(integer)
            if integer == u64_type()
                || integer == i64_type()
                || integer == u32_type()
                || integer == i32_type() =>
        {
            Some(integer)
        }
        _ => None,
    }
}

pub(crate) fn value_type(function: &PsiOptimizationFunction, value: ValueId) -> Option<ScalarType> {
    function
        .parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter().chain(block.definitions())),
        )
        .find(|definition| definition.value == value)
        .map(|definition| definition.scalar_type)
}
