/*
  Aggregate record/result homes (`aggregate_results`, `records`), primitive
  local storage (`primitive_storage`), field/index observations (`observations`),
  byte/element writes (`byte_write`, `element_view_write`), borrowed windows
  (`borrowed_windows`), access lifecycles (`access_lifecycles`), and atomic
  accesses (`atomic_access`) inside the common target control-flow graph.

  Tracks dominating structural homes, scalar field projections, mutable slice
  writes, and affine/linear lifecycle transitions across basic blocks.
*/

use super::*;
pub(crate) mod access_lifecycles {
    use super::RowSite;
    use super::aggregate_results::publish_home;
    use super::references::retain_access_result;
    use crate::LoweringError;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AccessLifecycleOperands};
    use optimization_unit_semantics::access_references::borrowed_view_formation;
    use semantic_vocabulary::BoundaryMachineId;
    use std::collections::BTreeMap;
    use target_operations::{
        BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
        BoundarySettlementRealization, CompilerBuiltinExecution, TargetUnitOperation,
    };
    use terminal_psi::{StructuralAccess, TerminalAccessLifecycleExecution};

    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        boundaries: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            structural_arguments,
            arguments,
            result,
            ..
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let invalid = |reason| LoweringError::InvalidAccessLifecycle {
            machine: function.machine,
            operation: *psi_operation,
            boundary: *boundary,
            reason,
        };
        let plan = types
            .catalog()
            .access_lifecycles()
            .iter()
            .find(|plan| plan.boundary == *boundary)
            .ok_or_else(|| invalid("native lifecycle lost its retained whole-value plan"))?;
        let TerminalAccessLifecycleExecution::CompilerPrimitive { profile_type, .. } =
            &plan.execution
        else {
            return Err(invalid(
                "a selected provider lifecycle is not a compiler primitive",
            ));
        };
        let settlement = settlements
            .get(boundary)
            .ok_or_else(|| invalid("native lifecycle has no execution settlement"))?;
        if settlement.execution
            != BoundaryExecutionBinding::CompilerBuiltin(
                CompilerBuiltinExecution::AccessLifecycle {
                    boundary: *boundary,
                    operation: plan.operation,
                },
            )
            || settlement.realization
                != BoundarySettlementRealization::Builtin(BoundaryRealization::AccessLifecycle {
                    boundary: *boundary,
                })
        {
            return Err(invalid(
                "native lifecycle settlement changed its boundary or operation",
            ));
        }
        if !arguments.is_empty() {
            return Err(invalid("native lifecycle has unexpected scalar arguments"));
        }
        let operands =
            AccessLifecycleOperands::from_arguments(plan.operation, structural_arguments)
                .map_err(invalid)?;
        let expected = match plan.operation {
            terminal_psi::TerminalAccessLifecycleOperation::Initialize => [
                Some(plan.storage.carrier),
                Some(plan.schema),
                *profile_type,
                None,
            ],
            terminal_psi::TerminalAccessLifecycleOperation::MoveOut => [
                Some(plan.storage.owner_carrier()),
                *profile_type,
                None,
                None,
            ],
            terminal_psi::TerminalAccessLifecycleOperation::View => {
                [Some(plan.storage.carrier), *profile_type, None, None]
            }
            terminal_psi::TerminalAccessLifecycleOperation::Relocate => [
                Some(plan.storage.owner_carrier()),
                Some(plan.storage.carrier),
                *profile_type,
                *profile_type,
            ],
        };
        for (argument, expected) in operands.arguments().zip(expected) {
            let actual = if argument.access == StructuralAccess::Owned {
                super::aggregate_results::value_input(function, types, live, argument)?.0
            } else {
                super::references::reference_source_type(function, live, types, argument)
                    .ok_or_else(|| {
                        invalid("native lifecycle borrowed input has no current referent")
                    })?
            };
            if Some(actual) != expected {
                return Err(invalid(
                    "native lifecycle operand carrier differs from its retained role",
                ));
            }
        }
        let AbstractBoundaryResult::Structural(result) = result else {
            return Err(invalid("native lifecycle lost its structural result"));
        };
        if plan.operation == terminal_psi::TerminalAccessLifecycleOperation::View {
            let declaration = boundaries
                .get(boundary)
                .ok_or_else(|| invalid("borrowed View lost its exact boundary declaration"))?;
            let (path, backing) = borrowed_view_formation(types.catalog(), declaration, result)
                .ok_or_else(|| invalid("borrowed View lost its Ready interpretation"))?;
            let AccessLifecycleOperands::View { storage, .. } = &operands else {
                return Err(invalid("borrowed View lost its storage operand"));
            };
            retain_access_result(function, types, live, result, storage, path, backing)?;
        }
        let Some(result_home) = publish_home(live, *psi_operation, result, types)? else {
            return Err(invalid("native lifecycle replaces an existing result home"));
        };
        operations.push(TargetUnitOperation::AccessLifecycle {
            psi_operation: *psi_operation,
            boundary: *boundary,
            operands,
            result_home,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}

pub(crate) mod aggregate_borrows {
    use super::LiveDefinitions;
    use crate::LoweringError;
    use crate::structural_layout::structural_shape;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use target_operations::{TargetStructuralArgument, TargetStructuralParameter};
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

    pub(crate) fn is_reference(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        types: &StructuralTypeLookup<'_>,
    ) -> bool {
        matches!(
            parameter.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
            && matches!(
                types
                    .get(parameter.structural_type)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Record { .. } | StructuralTypeShape::Sum { .. })
            )
    }

    pub(crate) fn argument(
        argument: &terminal_psi::StructuralArgument,
        declaration: &terminal_psi::StructuralParameterDeclaration,
        destination: &TargetStructuralParameter,
        function: &abstract_operations::AbstractFunction,
        prepared: &crate::function_signature::PreparedFunctionSignature,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralArgument, LoweringError> {
        let invalid = || LoweringError::UnknownStructuralType(declaration.structural_type);
        if !is_reference(declaration, types) || argument.access != declaration.access {
            return Err(invalid());
        }
        if live.address_joins.contains(&argument.place)
            || prepared.parameters.iter().any(|parameter| {
                parameter.place == argument.place && parameter.access != StructuralAccess::Owned
            })
        {
            return super::borrowed_calls::storage_argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        let (root_type, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
            if home.access() != StructuralAccess::Owned {
                return Err(invalid());
            }
            let source = home.argument_source();
            (home.structural_type(), source)
        } else {
            let parameter = prepared.parameter(argument.place).ok_or_else(invalid)?;
            if parameter.access != StructuralAccess::Owned {
                return Err(invalid());
            }
            (
                parameter.structural_type,
                parameter.placement.clone().into(),
            )
        };
        let (selected, shape, offset, indices) = if argument.path.is_empty() {
            (
                root_type,
                structural_shape(root_type, types)?,
                0,
                Vec::new(),
            )
        } else if argument
            .path
            .iter()
            .all(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Field(_)))
        {
            let (selected, shape, offset) =
                crate::structural_layout::resolve_structural_projection_path(
                    root_type,
                    &argument.path,
                    types,
                )?;
            (selected, shape, offset, Vec::new())
        } else {
            let (selected, shape, offset, runs) =
                crate::structural_layout::runtime_projection(root_type, &argument.path, types)?;
            let indices = super::scalar_sources::runtime_indices(
                runs,
                function,
                &super::scalar_sources::ScalarSources::from(live),
            )?;
            (selected, shape, offset, indices)
        };
        if selected != declaration.structural_type {
            return Err(invalid());
        }
        Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: root_type,
            structural_type: selected,
            shape: crate::structural_signature::StructuralCallSignature::parameter_shape(
                crate::structural_signature::ParameterLane::Internal,
                selected,
                shape,
                argument.access,
                types,
            ),
            source_byte_offset: offset,
            fixed_array_length: None,
            element_stride: None,
            indices,
            source,
            destination: destination.placement.clone(),
        })
    }
}

pub(crate) mod aggregate_results {
    use crate::LoweringError;
    use crate::structural_layout::structural_sum_layout;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use calling_conventions::terminal_layout::StructuralLayouts;
    use semantic_vocabulary::{OperationId, StructuralTypeId};
    use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};
    use terminal_semantics::{canonical_static_projection, view_descriptor_parameter};

    pub(crate) fn value_input(
        function: &abstract_operations::AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        live: &super::LiveDefinitions,
        argument: &terminal_psi::StructuralArgument,
    ) -> Result<(StructuralTypeId, terminal_psi::StructuralMultiplicity), LoweringError> {
        let invalid = |reason| LoweringError::InvalidStructuralValueInput {
            machine: function.machine,
            place: argument.place,
            reason,
        };
        if argument.access != terminal_psi::StructuralAccess::Owned {
            return Err(invalid(
                "a value input must transfer or copy its descriptor, not borrow it",
            ));
        }
        let (root, multiplicity) =
            if let Some(home) = live.structural_homes.get(&argument.place) {
                if home.access() != terminal_psi::StructuralAccess::Owned {
                    return Err(invalid("the live home does not own the selected value"));
                }
                (home.structural_type(), home.multiplicity())
            } else if let Some((producer, structural_type)) = live.views.get(&argument.place) {
                let mut producers = function
                    .operations
                    .iter()
                    .filter(|operation| operation.psi_operation() == Some(*producer));
                let operation = producers
                    .next()
                    .ok_or_else(|| invalid("the live descriptor producer is absent"))?;
                if producers.next().is_some() {
                    return Err(invalid("the live descriptor names duplicate producers"));
                }
                let declared = if let Some((_, result)) = operation.structural_result() {
            if result.place != argument.place {
                return Err(invalid("the descriptor producer publishes another place"));
            }
            (result.structural_type, result.multiplicity)
        } else if let abstract_operations::AbstractOperation::EstablishByteSequenceLiteral {
            place,
            structural_type,
            ..
        } = operation
        {
            if place.id != argument.place || !structural_type.shape.is_shared_readable_byte_view() {
                return Err(invalid(
                    "the literal does not publish this immutable byte descriptor",
                ));
            }
            (
                structural_type.id,
                terminal_psi::StructuralMultiplicity::Unrestricted,
            )
        } else {
            return Err(invalid(
                "the live descriptor producer has no value declaration",
            ));
        };
                if declared.0 != *structural_type {
                    return Err(invalid(
                        "the descriptor producer type differs from its live declaration",
                    ));
                }
                declared
            } else {
                let parameter = function.arrival_parameter(argument.place).ok_or_else(|| {
                    invalid("the value has no live home, descriptor or arrival declaration")
                })?;
                if parameter.access != terminal_psi::StructuralAccess::Owned
                    && (!argument.path.is_empty()
                        || !view_descriptor_parameter(parameter, types.values().copied())
                        || live.structural_observation_type(function, argument.place)
                            != Some(parameter.structural_type))
                {
                    return Err(invalid(
                        "a borrowed arrival can supply only its live whole view descriptor",
                    ));
                }
                (parameter.structural_type, parameter.multiplicity)
            };
        let selected = canonical_static_projection(types.values().copied(), root, &argument.path)
            .map(|(_, declaration)| declaration.id)
            .ok_or_else(|| {
                invalid("the value projection does not select a declared owned child")
            })?;
        Ok((selected, multiplicity))
    }

    fn sum_layout(
        structural_type: StructuralTypeId,
        types: &StructuralLayouts<'_>,
    ) -> Result<calling_conventions::ConventionalSumLayout, LoweringError> {
        let invalid = || LoweringError::UnsupportedStructuralSum(structural_type);
        let declaration = types.get(structural_type).ok_or_else(invalid)?;
        if !matches!(
            declaration.shape,
            terminal_psi::sum_or_mixed_shape_pattern!()
        ) {
            return Err(invalid());
        }
        structural_sum_layout(structural_type, types)
    }

    pub(crate) fn block_home(
        block: semantic_vocabulary::BlockId,
        declaration: &terminal_psi::StructuralParameterDeclaration,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralHomeRequirement, LoweringError> {
        if !super::owned_arguments::is_owned_parameter(declaration, types) {
            return Err(LoweringError::UnsupportedStructuralSum(
                declaration.structural_type,
            ));
        }
        let result = terminal_psi::StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: declaration.place,
            structural_type: declaration.structural_type,
            multiplicity: declaration.multiplicity,
            qualifications: declaration.qualifications.clone(),
            projected_qualifications: declaration.projected_qualifications.clone(),
        };
        Ok(TargetStructuralHomeRequirement {
            origin: target_operations::TargetStructuralHomeOrigin::BlockParameter {
                block,
                declaration: declaration.clone(),
            },
            layout: result_home_layout(&result, types)?,
        })
    }

    pub(crate) fn result_home_layout(
        result: &terminal_psi::StructuralResultDeclaration,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralHomeLayout, LoweringError> {
        value_home_layout(result.structural_type, types)
    }

    pub(crate) fn value_home_layout(
        structural_type: StructuralTypeId,
        types: &StructuralLayouts<'_>,
    ) -> Result<TargetStructuralHomeLayout, LoweringError> {
        let declaration = types
            .get(structural_type)
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        if matches!(
            declaration.shape,
            terminal_psi::sum_or_mixed_shape_pattern!()
        ) {
            return Ok(TargetStructuralHomeLayout::Sum(sum_layout(
                structural_type,
                types,
            )?));
        }
        Ok(TargetStructuralHomeLayout::Aggregate(
            crate::structural_layout::structural_shape(structural_type, types)?,
        ))
    }

    pub(crate) fn home(
        operation: OperationId,
        result: &terminal_psi::StructuralOperationResult,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralHomeRequirement, LoweringError> {
        let declaration = terminal_psi::StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: result.place,
            structural_type: result.structural_type,
            multiplicity: result.multiplicity,
            qualifications: result.qualifications.clone(),
            projected_qualifications: result.projected_qualifications.clone(),
        };
        Ok(TargetStructuralHomeRequirement {
            origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                operation,
                result: result.clone(),
            },
            layout: result_home_layout(&declaration, types)?,
        })
    }

    pub(crate) fn publish_home(
        live: &mut super::LiveDefinitions,
        operation_id: OperationId,
        result: &terminal_psi::StructuralOperationResult,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<Option<TargetStructuralHomeRequirement>, LoweringError> {
        let result_home = home(operation_id, result, types)?;
        Ok(live
            .structural_homes
            .insert(result.place, result_home.clone())
            .is_none()
            .then_some(result_home))
    }

    pub(crate) fn established_home(
        live: &mut super::LiveDefinitions,
        operation_id: OperationId,
        result: &terminal_psi::StructuralOperationResult,
        types: &StructuralTypeLookup<'_>,
        machine: semantic_vocabulary::MachineId,
    ) -> Result<TargetStructuralHomeRequirement, LoweringError> {
        publish_home(live, operation_id, result, types)?
            .ok_or_else(|| LoweringError::unsupported_control_flow(machine))
    }
}

pub(crate) mod atomic_access {
    use super::RowSite;
    use crate::LoweringError;
    use crate::structural_layout::reference_projection_regions;
    use abstract_operations::AbstractAtomicEvent;
    use semantic_vocabulary::OperationId;
    use target_operations::TargetUnitOperation;
    use terminal_psi::{AtomicAccessEvent, StructuralAccess};

    pub(crate) fn lower(
        operation: OperationId,
        event: &AbstractAtomicEvent,
        site: &mut RowSite<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        if let AbstractAtomicEvent::Fence { ordering } = event {
            operations.push(TargetUnitOperation::AtomicFence {
                psi_operation: operation,
                ordering: *ordering,
            });
            provenance.operations.push(operation);
            return Ok(());
        }
        let location = event
            .location()
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let (access, result) = match event {
            AbstractAtomicEvent::Load {
                ordering, result, ..
            } => (
                AtomicAccessEvent::Load {
                    ordering: *ordering,
                },
                Some(*result),
            ),
            AbstractAtomicEvent::Store {
                ordering, value, ..
            } => (
                AtomicAccessEvent::Store {
                    ordering: *ordering,
                    value: *value,
                },
                None,
            ),
            AbstractAtomicEvent::ReadModifyWrite {
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
            AbstractAtomicEvent::Swap {
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
            AbstractAtomicEvent::CompareExchange {
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
            AbstractAtomicEvent::CompareExchangeOnce {
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
            AbstractAtomicEvent::Fence { .. } => {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
        };
        let (root, root_access) =
            super::primitive_storage::field_root(function, types, prepared, live, location.root)?;
        if !access.ordering_is_legal()
            || access.observes_resident() && root_access == StructuralAccess::WriteOnlyBorrow
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let cell =
            terminal_semantics::atomic_cell_type(types.values().copied(), root, &location.path)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let scalar_type = cell.scalar_type();
        let integer =
            target_operations::operations::atomic_storage_integer_type(access, scalar_type)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let regions = reference_projection_regions(root, &location.path, types)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if !regions.readable
            || regions
                .pointer_slots
                .iter()
                .any(|slot| !slot.indices.is_empty())
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let (field_byte_offset, bytes) = crate::structural_layout::static_atomic_geometry(
            regions.root,
            regions.path,
            scalar_type,
            types,
        )?;
        if bytes != u32::from(integer.bits() / 8) || field_byte_offset % bytes != 0 {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let operands = access
            .operands()
            .into_iter()
            .map(|value| {
                let source = super::scalar_sources::source(value, function, live)?;
                if source.scalar_type() != scalar_type {
                    return Err(LoweringError::ValueTypeMismatch(value));
                }
                Ok(source)
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let result_home = result
            .map(|result| {
                if result.scalar_type != scalar_type {
                    return Err(LoweringError::ValueTypeMismatch(result.value));
                }
                super::primitive_storage::retain_result(operation, result, live)
            })
            .transpose()?;
        let outcome_home =
            if let AbstractAtomicEvent::CompareExchangeOnce {
                outcome, custody, ..
            } = event
            {
                if !event.custody_is_consistent()
                    || !terminal_psi::types::atomic_once_result_is_plain(outcome, *custody)
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let layout = types
                    .get(outcome.structural_type)
                    .and_then(|row| {
                        target_operations::conventional_atomic_once_layout(row, integer, *custody)
                    })
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                let home = target_operations::TargetStructuralHomeRequirement {
                    origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                        operation,
                        result: outcome.clone(),
                    },
                    layout: target_operations::TargetStructuralHomeLayout::Sum(layout),
                };
                if live
                    .structural_homes
                    .insert(outcome.place, home.clone())
                    .is_some()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                Some(home)
            } else {
                None
            };
        operations.push(TargetUnitOperation::AtomicAccess {
            psi_operation: operation,
            location: terminal_psi::StructuralArgument {
                place: location.root,
                access: root_access,
                path: location.path.clone(),
            },
            scalar_type: integer,
            field_byte_offset,
            event: access,
            operands,
            result_home,
            outcome_home,
        });
        provenance.operations.push(operation);
        Ok(())
    }
}

pub(crate) mod borrowed_windows {
    use super::references::{window_projection, window_storage};
    use super::{LiveDefinitions, RowSite};
    use crate::LoweringError;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use semantic_vocabulary::StructuralFieldId;
    use std::collections::BTreeSet;
    use target_operations::TargetUnitOperation;
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralPathSegment,
    };

    pub(crate) fn move_field(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, types) = (site.function, site.types);
        let AbstractOperation::MoveStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let referent = window_referent(
            function,
            site.live,
            types,
            source,
            path,
            StructuralAccess::MutableBorrow,
        )?;
        if !window_root(
            function,
            prepared,
            site.live,
            source,
            StructuralAccess::MutableBorrow,
        ) || !root_window_loan(
            function,
            site.live,
            types,
            source,
            path,
            *field,
            StructuralAccess::MutableBorrow,
        ) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let (geometry_type, geometry_path) = referent
            .as_ref()
            .map_or((source.structural_type(), path.as_slice()), |referent| {
                (referent.referent, referent.path.as_slice())
            });
        let mut path = field_path(types, geometry_type, geometry_path, *field)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if let Some(referent) = &referent {
            let mut full = referent.carrier_path.clone();
            full.push(StructuralPathSegment::Referent);
            full.extend(path);
            path = full;
        }
        if site
            .live
            .references
            .keys()
            .any(|(carrier, _)| *carrier == result.place)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut relocations = Vec::new();
        let mut moved_leaves = BTreeSet::new();
        let (storage, storage_path) =
            if super::references::contains_reference(types, result.structural_type) {
                super::references::window_storage(
                    function,
                    types,
                    site.live,
                    source.place(),
                    &path,
                )?
            } else {
                (source.place(), path.clone())
            };
        super::references::subtree_leaves(
            function,
            types,
            site.live,
            &[],
            result.structural_type,
            &StructuralArgument {
                place: storage,
                path: storage_path,
                access: StructuralAccess::Owned,
            },
            true,
            &mut relocations,
            &mut moved_leaves,
        )?;
        super::leaf_copy::copy_extent(
            *psi_operation,
            result,
            source.place(),
            &path.into(),
            source.structural_type(),
            site,
        )?;
        super::references::relocate_leaves(site.live, result.place, relocations, moved_leaves);
        Ok(())
    }

    pub(crate) fn store_field(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let AbstractOperation::StoreStructuralField {
            disposition,
            psi_operation,
            destination,
            path,
            field,
            value,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let referent = window_referent(
            function,
            live,
            types,
            destination,
            path,
            disposition.required_access(),
        )?;
        let whole_owned_store = path.is_empty()
            && field.is_none()
            && destination.parameter().is_none()
            && *disposition == terminal_psi::StructuralStoreDisposition::Vacant;
        if !window_root(
            function,
            prepared,
            live,
            destination,
            disposition.required_access(),
        ) || (!whole_owned_store
            && !root_window_loan(
                function,
                live,
                types,
                destination,
                path,
                *field,
                disposition.required_access(),
            ))
            || value.access != StructuralAccess::Owned
            || !value.path.is_empty()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let (geometry_type, geometry_path) = referent.as_ref().map_or(
            (destination.structural_type(), path.as_slice()),
            |referent| (referent.referent, referent.path.as_slice()),
        );
        let full_path = field_path(types, geometry_type, geometry_path, *field)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let (field_type, shape, byte_offset, indices) =
            crate::structural_layout::reference_referent_projection(
                destination.structural_type(),
                &{
                    let mut full = path.clone();
                    full.extend_from_slice(&full_path[geometry_path.len()..]);
                    full
                },
                disposition.required_access(),
                None,
                types,
            )?;
        let indices = super::scalar_sources::runtime_indices(
            indices,
            function,
            &super::scalar_sources::ScalarSources::from(&*live),
        )?;
        let (value_type, multiplicity) =
            super::aggregate_results::value_input(function, types, live, value)?;
        if value_type != field_type
            || super::aggregate_results::value_home_layout(value_type, types)?.shape() != shape
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut custody_path = full_path;
        if let Some(referent) = &referent {
            let mut full = referent.carrier_path.clone();
            full.push(StructuralPathSegment::Referent);
            full.extend(custody_path);
            custody_path = full;
        }
        let (storage, custody_path) = if super::references::contains_reference(types, field_type) {
            window_storage(function, types, live, destination.place(), &custody_path)?
        } else {
            (destination.place(), custody_path)
        };
        if *disposition == terminal_psi::StructuralStoreDisposition::ReplaceDiscardable {
            super::references::discard_subtree(function, types, live, storage, &custody_path)?;
        }
        if live.references.keys().any(|(carrier, path)| {
            *carrier == storage
                && path
                    .strip_prefix(custody_path.as_slice())
                    .is_some_and(|relative| !relative.contains(&StructuralPathSegment::Referent))
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut relocations = Vec::new();
        let mut moved_leaves = BTreeSet::new();
        super::references::subtree_leaves(
            function,
            types,
            live,
            &custody_path,
            field_type,
            value,
            multiplicity != StructuralMultiplicity::Unrestricted,
            &mut relocations,
            &mut moved_leaves,
        )?;
        super::references::relocate_leaves(live, storage, relocations, moved_leaves);
        operations.push(TargetUnitOperation::StoreStructuralField {
            psi_operation: *psi_operation,
            destination: destination.clone(),
            path: path.clone(),
            field: *field,
            value: value.place,
            byte_offset,
            indices,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }

    pub(crate) fn window_referent(
        function: &AbstractFunction,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
        root: &abstract_operations::StructuralWindowRoot,
        path: &[StructuralPathSegment],
        required_access: StructuralAccess,
    ) -> Result<Option<super::references::ReferentProjection>, LoweringError> {
        if !path.contains(&StructuralPathSegment::Referent) {
            return Ok(None);
        }
        let referent =
            window_projection(function, live, types, root.place(), path, required_access)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if referent.carrier_type != root.structural_type() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok(Some(referent))
    }

    fn root_window_loan(
        function: &AbstractFunction,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
        root: &abstract_operations::StructuralWindowRoot,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
        required_access: StructuralAccess,
    ) -> bool {
        if terminal_semantics::structural_window_selects_child(path, field) {
            return true;
        }
        if path.is_empty() {
            return root.parameter().is_some();
        }
        if !path.contains(&StructuralPathSegment::Referent) {
            return false;
        }
        let Ok(descriptors) =
            super::references::window_descriptors(function, types, live, root.place(), path)
        else {
            return false;
        };
        let Some(origins) = descriptors
            .into_iter()
            .flat_map(|leaf| leaf.root.origins())
            .map(|origin| origin.base.root_place())
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        terminal_semantics::structural_window_origins(
            function.structural_parameters.iter().chain(
                function
                    .block_entries
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            ),
            origins,
            required_access,
            |place| {
                live.structural_homes.get(&place).is_some_and(|home| {
                    home.access() == StructuralAccess::Owned
                        && home.multiplicity() != StructuralMultiplicity::Linear
                        && match &home.origin {
                            target_operations::TargetStructuralHomeOrigin::OperationResult {
                                result,
                                ..
                            } => result.claims.is_empty(),
                            target_operations::TargetStructuralHomeOrigin::BlockParameter {
                                ..
                            } => true,
                        }
                }) || (live.owned_arrivals.contains(&place)
                    && function
                        .structural_parameter(place)
                        .is_some_and(|parameter| {
                            parameter.access == StructuralAccess::Owned
                                && parameter.multiplicity != StructuralMultiplicity::Linear
                                && !function
                                    .entry_claims
                                    .iter()
                                    .any(|claim| claim.input == place)
                        }))
            },
        )
    }

    fn window_root(
        function: &AbstractFunction,
        prepared: &crate::function_signature::PreparedFunctionSignature,
        live: &LiveDefinitions,
        root: &abstract_operations::StructuralWindowRoot,
        required_access: StructuralAccess,
    ) -> bool {
        match root {
            abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
                live.structural_homes
                    .get(&parameter.place)
                    .is_some_and(|home| {
                        home.origin
                            == target_operations::TargetStructuralHomeOrigin::BlockParameter {
                                block: *block,
                                declaration: parameter.clone(),
                            }
                            && parameter.access == StructuralAccess::Owned
                    })
            }
            abstract_operations::StructuralWindowRoot::Parameter(root) => {
                required_access.admits(root.access)
                    && ((function.structural_parameters.contains(root)
                        && prepared.parameter(root.place).is_some_and(|parameter| {
                            parameter.access == root.access
                                && parameter.structural_type == root.structural_type
                        }))
                        || super::transfers::address_parameter(function, live, root.place)
                            == Some(root)
                        || (live.block_views.contains(&root.place)
                            && matches!(
                                root.access,
                                terminal_psi::exclusive_borrow_pair_pattern!()
                            )
                            && function.block_structural_parameter(root.place) == Some(root)))
            }
            abstract_operations::StructuralWindowRoot::OwnedResult { producer, result } => live
                .structural_homes
                .get(&result.place)
                .is_some_and(|home| home.operation_result() == Some((*producer, result))),
        }
    }

    fn field_path(
        types: &StructuralTypeLookup<'_>,
        root: semantic_vocabulary::StructuralTypeId,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
    ) -> Option<Vec<StructuralPathSegment>> {
        terminal_semantics::structural_window_projection(types.values().copied(), root, path, field)
            .map(|projection| projection.path)
    }
}

pub(crate) mod byte_concatenation {
    use super::RowSite;
    use super::aggregate_results::established_home;
    use crate::LoweringError;
    use crate::function_signature::PreparedFunctionSignature;
    use crate::scalar::descriptor_operands::observes_count;
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::IntegerSign;
    use target_operations::TargetUnitOperation;
    use terminal_psi::control_flow::ByteSequenceOperand;
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};

    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let AbstractOperation::EstablishByteSequenceConcatenation {
            psi_operation,
            result,
            left,
            right,
            capacity_obligation,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.claims.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !types
                .get(result.structural_type)
                .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        for operand in [left, right] {
            let (source, length) = match operand {
                ByteSequenceOperand::Whole { source, length } => {
                    let (selected, indices) =
                        crate::scalar::descriptor_operands::descriptor_projection(
                            function,
                            types,
                            live.element_view_backings(),
                            source,
                        )?;
                    let values =
                        super::observations::scalar_values(live, &prepared.scalar_parameters)?;
                    if source.access != StructuralAccess::SharedBorrow
                        || !indices.is_empty()
                        || !types.get(selected).is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::ByteSequence(_)
                            )
                        })
                        || !observes_count(function, &values, *length, source, true)
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    (source.place, *length)
                }
                ByteSequenceOperand::Field {
                    source,
                    field,
                    length,
                } => {
                    let authority = super::primitive_storage::byte_field_observation(
                        function,
                        types,
                        prepared,
                        live,
                        source.place,
                        &source.path,
                        *field,
                        terminal_semantics::SequenceFieldObservation::Contents,
                    )?;
                    if source.access != StructuralAccess::SharedBorrow
                        || authority.access == StructuralAccess::WriteOnlyBorrow
                        || !function.operations.iter().any(|candidate| {
                            matches!(candidate,
                        AbstractOperation::StructuralByteSequenceFieldLength {
                            source: measured, path, field: measured_field, result, ..
                        } if *measured == source.place && *path == source.path
                            && measured_field == field && result.value == *length)
                        })
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    (source.place, *length)
                }
            };
            if source == result.place {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let scalar = live
                .integers
                .get(&length)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?
                .scalar_type();
            if scalar.sign() != IntegerSign::Unsigned || scalar.bits() != 64 || scalar.is_address()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
        }
        established_home(live, *psi_operation, result, types, function.machine)?;
        if live
            .views
            .insert(result.place, (*psi_operation, result.structural_type))
            .is_some()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        operations.push(TargetUnitOperation::EstablishByteSequenceConcatenation {
            psi_operation: *psi_operation,
            result: result.clone(),
            left: left.clone(),
            right: right.clone(),
            capacity_obligation: *capacity_obligation,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}

pub(crate) mod byte_write {
    use super::observations::bounded_owner_storage;
    use super::primitive_storage::field_root;
    use super::{LiveDefinitions, RowSite};
    use crate::LoweringError;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId};
    use target_operations::TargetUnitOperation;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

    pub(crate) fn replace(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, structural_types, live, operations, provenance) = site.view();
        let (psi_operation, destination, source, length, obligation, field_destination) =
            match operation {
                AbstractOperation::ByteSequenceStore {
                    psi_operation,
                    destination,
                    source,
                    length,
                    obligation,
                } => {
                    let (structural_type, access, multiplicity) = if let Some(home) =
                        live.structural_homes.get(destination)
                    {
                        (home.structural_type(), home.access(), home.multiplicity())
                    } else {
                        let parameter =
                            function.arrival_parameter(*destination).ok_or_else(|| {
                                LoweringError::unsupported_control_flow(function.machine)
                            })?;
                        if !parameter.projected_qualifications.is_empty() {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        }
                        (
                            parameter.structural_type,
                            parameter.access,
                            parameter.multiplicity,
                        )
                    };
                    if !matches!(
                        access,
                        StructuralAccess::Owned
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    ) || multiplicity != StructuralMultiplicity::Unrestricted
                        || !structural_types
                            .get(structural_type)
                            .is_some_and(|declaration| {
                                declaration.shape.is_bounded_owned_byte_sequence()
                            })
                        || function
                            .entry_claims
                            .iter()
                            .any(|claim| claim.input == *destination)
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    (psi_operation, destination, source, length, obligation, None)
                }
                AbstractOperation::StructuralByteSequenceFieldStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    source,
                    length,
                    obligation,
                } => (
                    psi_operation,
                    destination,
                    source,
                    length,
                    obligation,
                    Some((
                        writable_field(
                            function,
                            structural_types,
                            prepared,
                            live,
                            *destination,
                            path,
                            *field,
                        )?,
                        *field,
                    )),
                ),
                _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
            };
        if source == destination
            || live.lengths.get(length) != Some(source)
            || !(function.arrival_parameter(*source).is_some() || live.views.contains_key(source))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let length_value = *live
            .integers
            .get(length)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if length_value.scalar_type()
            != IntegerType::new(IntegerSign::Unsigned, 64)
                .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        operations.push(if let Some((destination, field)) = field_destination {
            TargetUnitOperation::StructuralByteSequenceFieldStore {
                psi_operation: *psi_operation,
                destination,
                field,
                source: *source,
                length: length_value.into_target_source(*length),
                obligation: *obligation,
            }
        } else {
            TargetUnitOperation::ByteSequenceStore {
                psi_operation: *psi_operation,
                destination: *destination,
                source: *source,
                length: length_value.into_target_source(*length),
                obligation: *obligation,
            }
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }

    fn writable_field(
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
        live: &LiveDefinitions,
        destination: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Result<terminal_psi::StructuralArgument, LoweringError> {
        writable_byte_field(
            function,
            structural_types,
            prepared,
            live,
            destination,
            path,
            field,
            |carrier| {
                matches!(
                    carrier,
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                )
            },
        )
    }

    pub(crate) fn replace_view_descriptor(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, structural_types, live, operations, provenance) = site.view();
        let AbstractOperation::StructuralByteViewFieldStore {
            psi_operation,
            destination,
            path,
            field,
            source,
            ..
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let shared_view = terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        };
        let destination_argument = match field {
            Some(field) => writable_byte_field(
                function,
                structural_types,
                prepared,
                live,
                *destination,
                path,
                *field,
                |carrier| carrier == shared_view,
            )?,
            None => {
                let home = live
                    .structural_homes
                    .get(destination)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                if !path.is_empty()
                    || !home.projected_qualifications().is_empty()
                    || !matches!(
                        structural_types
                            .get(home.structural_type())
                            .map(|declaration| &declaration.shape),
                        Some(declaration) if declaration.is_shared_readable_byte_view()
                    )
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                terminal_psi::StructuralArgument {
                    place: *destination,
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                }
            }
        };
        let source_type = live.structural_observation_type(function, *source);
        if source == destination
            || !source_type.is_some_and(|identity| {
                structural_types
                    .get(identity)
                    .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
            })
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let indices = super::primitive_storage::field_indices(
            function,
            structural_types,
            prepared,
            live,
            &destination_argument,
        )?;
        operations.push(TargetUnitOperation::StructuralByteViewFieldStore {
            psi_operation: *psi_operation,
            destination: destination_argument,
            field: *field,
            source: *source,
            indices,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }

    fn writable_byte_field(
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
        live: &LiveDefinitions,
        destination: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
        carrier_admitted: impl Fn(terminal_psi::ByteSequenceCarrier) -> bool,
    ) -> Result<terminal_psi::StructuralArgument, LoweringError> {
        let reference_destination = super::references::referent_projection(
            function,
            live,
            structural_types,
            destination,
            path,
            StructuralAccess::MutableBorrow,
        );
        let (root_type, access) = if let Some(reference) = &reference_destination {
            (reference.carrier_type, StructuralAccess::MutableBorrow)
        } else {
            field_root(function, structural_types, prepared, live, destination)?
        };
        if !matches!(
            access,
            StructuralAccess::Owned
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let root = destination;
        let (projection_root, projection_path) = reference_destination
            .as_ref()
            .map(|reference| (reference.referent, reference.path.as_slice()))
            .unwrap_or((root_type, path));
        if !terminal_psi::is_structural_scalar_store_path(projection_path)
            || function
                .entry_claims
                .iter()
                .any(|claim| claim.input == root)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let carrier = if projection_path.is_empty() {
            projection_root
        } else {
            crate::structural_layout::runtime_projection(
                projection_root,
                projection_path,
                structural_types,
            )?
            .0
        };
        let StructuralTypeShape::Record { fields, .. } = &structural_types
            .get(carrier)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
            .shape
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let mut matching = fields.iter().filter(|candidate| candidate.id == field);
        let selected = matching
            .next()
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if matching.next().is_some()
            || selected.relevance.is_erased()
            || !selected
                .field_type
                .byte_sequence_carrier()
                .is_some_and(carrier_admitted)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let (whole, projected) = if let Some(parameter) = function.arrival_parameter(root) {
            (
                parameter.qualifications.as_slice(),
                parameter.projected_qualifications.as_slice(),
            )
        } else {
            let home = live
                .structural_homes
                .get(&root)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            (home.qualifications(), home.projected_qualifications())
        };
        let mut written = path.to_vec();
        written.push(StructuralPathSegment::Field(selected.identity));
        if !terminal_semantics::structural_memberships_disjoint_from_write(
            whole, projected, &written,
        ) {
            return Err(LoweringError::InvalidStructuralValueInput {
                machine: function.machine,
                place: root,
                reason: "byte field write overlaps a standing carrier membership",
            });
        }
        Ok(terminal_psi::StructuralArgument {
            place: root,
            access,
            path: path.to_vec(),
        })
    }

    pub(crate) fn replace_byte(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, structural_types, live, operations, provenance) = site.view();
        let AbstractOperation::StructuralByteSequenceFieldByteStore {
            psi_operation,
            destination,
            path,
            field,
            index,
            value,
            length,
            obligation,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let destination_argument = writable_field(
            function,
            structural_types,
            prepared,
            live,
            *destination,
            path,
            *field,
        )?;
        if !function.operations.iter().any(|operation| matches!(operation,
        AbstractOperation::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field, result, .. }
        if source == destination && terminal_semantics::structural_paths_select_same_location(measured_path, path) && measured_field == field && result.value == *length))
    { return Err(LoweringError::unsupported_control_flow(function.machine)); }
        let index_value = *live
            .integers
            .get(index)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let byte_value = *live
            .integers
            .get(value)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let length_value = *live
            .integers
            .get(length)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        for (scalar_type, bits) in [
            (index_value.scalar_type(), 64),
            (byte_value.scalar_type(), 8),
            (length_value.scalar_type(), 64),
        ] {
            if scalar_type.sign() != IntegerSign::Unsigned
                || scalar_type.bits() != bits
                || scalar_type.is_address()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
        }
        operations.push(TargetUnitOperation::StructuralByteSequenceFieldByteStore {
            psi_operation: *psi_operation,
            destination: destination_argument,
            field: *field,
            index: index_value.into_target_source(*index),
            value: byte_value.into_target_source(*value),
            length: *length,
            obligation: *obligation,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }

    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, structural_types, live, operations, provenance) = site.view();
        let AbstractOperation::ByteSequenceWrite {
            psi_operation,
            destination,
            index,
            value,
            length,
            obligation,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        if live.lengths.get(length) != Some(destination)
            || !(prepared.parameter(*destination).is_some()
                || live.block_views.contains(destination)
                || live.views.contains_key(destination)
                || live.structural_homes.contains_key(destination)
                || bounded_owner_storage(function, structural_types, live, *destination))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let values = super::observations::scalar_values(live, &prepared.scalar_parameters)?;
        let (view, access) = crate::scalar::byte_views::resolve_byte_view(
            function,
            structural_types,
            &prepared.parameters,
            live.element_view_backings(),
            &values,
            &live.lengths,
            *destination,
            &mut Vec::new(),
        )?;
        if !matches!(access, terminal_psi::mutating_access_pair_pattern!()) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let index_value = *live
            .integers
            .get(index)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let byte_value = *live
            .integers
            .get(value)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        for (scalar_type, bits) in [
            (index_value.scalar_type(), 64),
            (byte_value.scalar_type(), 8),
        ] {
            if scalar_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
                || scalar_type.bits() != bits
                || scalar_type.is_address()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
        }
        operations.push(TargetUnitOperation::ByteSequenceWrite {
            psi_operation: *psi_operation,
            destination: *destination,
            view,
            index: index_value.into_target_source(*index),
            value: byte_value.into_target_source(*value),
            length: *length,
            obligation: *obligation,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}

pub(crate) mod element_view_write {
    use super::{RowSite, observations};
    use crate::LoweringError;
    use crate::scalar::descriptor_operands::observes_count;
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::IntegerSign;
    use target_operations::TargetUnitOperation;
    use terminal_psi::StructuralTypeShape;
    use terminal_semantics::element_view_leaf_scalar;

    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, structural_types, live, operations, provenance) = site.view();
        let AbstractOperation::ElementViewWrite {
            path,
            psi_operation,
            destination,
            index,
            value,
            length,
            obligation,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let values = observations::scalar_values(live, &prepared.scalar_parameters)?;
        if !observes_count(function, &values, *length, destination, false) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let (view_structural_type, indices) =
            crate::scalar::descriptor_operands::descriptor_projection(
                function,
                structural_types,
                live.element_view_backings(),
                destination,
            )?;
        let StructuralTypeShape::ElementView { element, .. } = &structural_types
            .get(view_structural_type)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?
            .shape
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let Some(element_scalar) =
            element_view_leaf_scalar(structural_types.catalog().as_slice(), *element, path)
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let index_value = *live
            .integers
            .get(index)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let element_value = super::scalar_sources::source(*value, function, live)?;
        if index_value.scalar_type().sign() != IntegerSign::Unsigned
            || index_value.scalar_type().bits() != 64
            || index_value.scalar_type().is_address()
            || element_scalar != element_value.scalar_type()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        operations.push(TargetUnitOperation::ElementViewWrite {
            path: path.clone(),
            psi_operation: *psi_operation,
            destination: destination.clone(),
            indices,
            index: index_value.into_target_source(*index),
            value: element_value,
            length: *length,
            obligation: *obligation,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}

pub(crate) mod leaf_copy {
    use super::RowSite;
    use super::aggregate_results::established_home;
    use crate::LoweringError;
    use crate::structural_layout::structural_shape;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralTypeId,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use target_operations::TargetUnitOperation;
    use terminal_psi::{StructuralOperationResult, StructuralPathSegment, StructuralTypeShape};

    pub(crate) fn transport(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
    ) -> Result<(), LoweringError> {
        let (function, types) = (site.function, site.types);
        let owned_path;
        let (psi_operation, result, source, path, transfer) = match operation {
            AbstractOperation::StructuralLeafCopy {
                psi_operation,
                result,
                source,
                path,
            } => (*psi_operation, result, *source, path, None),
            AbstractOperation::MoveStructuralValue {
                psi_operation,
                result,
                source,
            }
            | AbstractOperation::TransformContent {
                psi_operation,
                result,
                source,
                ..
            } => {
                owned_path = source.path.clone().into();
                (
                    *psi_operation,
                    result,
                    source.place,
                    &owned_path,
                    Some(source),
                )
            }
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
        let root_type = site
            .live
            .structural_observation_type(function, source)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if transfer.is_none()
            && let Some(home) = site.live.structural_homes.get(&source)
            && home.has_claims()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut relocations = Vec::new();
        let mut moved_leaves = BTreeSet::new();
        let copied = if let Some(argument) = transfer {
            super::references::subtree_leaves(
                function,
                types,
                site.live,
                &[],
                result.structural_type,
                argument,
                true,
                &mut relocations,
                &mut moved_leaves,
            )?;
            BTreeMap::new()
        } else {
            super::references::copied_leaves(function, types, site.live, result, source, path)?
        };
        copy_extent(psi_operation, result, source, path, root_type, site)?;
        site.live.references.extend(copied);
        super::references::relocate_leaves(site.live, result.place, relocations, moved_leaves);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn copy_extent(
        psi_operation: OperationId,
        result: &StructuralOperationResult,
        source: PlaceId,
        path: &abstract_operations::StructuralCopyPath,
        root_type: StructuralTypeId,
        site: &mut RowSite<'_>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let (endpoint, shape, byte_offset, indices) = match path {
            abstract_operations::StructuralCopyPath::Case(path) => {
                let (endpoint, shape, offset) =
                    crate::structural_layout::case_projection(root_type, path, types)?;
                (endpoint, shape, offset, Vec::new())
            }
            abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty() => (
                root_type,
                structural_shape(root_type, types)?,
                0,
                Vec::new(),
            ),
            abstract_operations::StructuralCopyPath::Ordinary(path) => {
                crate::structural_layout::runtime_leaf_copy_projection(
                    root_type,
                    path,
                    result.structural_type,
                    types,
                )?
            }
        };
        let byte_carrier = endpoint != result.structural_type
            && byte_carrier_leaf_copy(types, root_type, path, result.structural_type);
        let shape = if byte_carrier {
            crate::structural_layout::structural_shape(result.structural_type, types)?
        } else {
            shape
        };
        let quotient = matches!(path, abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty())
            && abstract_operations::quotients::licensed_copy(
                types.catalog(),
                function.machine,
                psi_operation,
                source,
                root_type,
                result,
                function.operations.iter(),
            );
        let occurrence_access = match path {
            abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty() => function
                .arrival_parameter(source)
                .map(|parameter| parameter.access)
                .or_else(|| {
                    function.operations.iter().find_map(|operation| {
                        let declaration = operation.structural_storage()?;
                        (declaration.place == source).then(|| {
                            terminal_semantics::descriptor_results::descriptor_value_access(
                                types.catalog().iter(),
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
                types.catalog().iter(),
                endpoint,
                occurrence_access,
                result.structural_type,
                result.multiplicity,
            )
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let indices = super::scalar_sources::runtime_indices(
            indices,
            function,
            &super::scalar_sources::ScalarSources::from(&*live),
        )?;
        let result_home = established_home(live, psi_operation, result, types, function.machine)?;
        if result_home.layout.shape() != shape {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        operations.push(TargetUnitOperation::StructuralLeafCopy {
            psi_operation,
            result_home,
            source,
            path: path.clone(),
            byte_offset,
            indices,
        });
        provenance.operations.push(psi_operation);
        Ok(())
    }

    fn byte_carrier_leaf_copy(
        types: &StructuralTypeLookup<'_>,
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
        let Some(parent) = types.subtree(root, prefix) else {
            return false;
        };
        let Some(declaration) = types.get(parent) else {
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
        if !types.values().any(|candidate| {
            candidate.id == *element
                && candidate.shape == StructuralTypeShape::PrimitiveScalar(byte_type)
        }) {
            return false;
        };
        let Some(layout) =
            terminal_semantics::byte_carrier_scalar_layout(types.values().copied(), expected)
        else {
            return false;
        };
        index
            .checked_add(layout.size)
            .is_some_and(|end| end <= *length)
    }
}

pub(crate) mod observations {
    use super::aggregate_results::established_home;
    use super::{KnownUnitInteger, LiveDefinitions, RowSite};
    use crate::LoweringError;
    use crate::scalar::try_lower_integer_operation;
    use crate::scalar::{
        KnownInteger, KnownScalar, byte_views, element_views, equal_boolean, equal_integer,
        order_integer, scalar_parameter_location,
    };
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{
        AbstractFunction, AbstractOperation, AbstractParameter, IntegerArithmeticOperator,
        IntegerOverflowPolicy,
    };
    use calling_conventions::ValueShape;
    use semantic_vocabulary::{IntegerSign, PlaceId, ScalarType, ValueId};
    use std::collections::BTreeMap;
    use target_operations::{
        ScalarAbiValue, TargetBooleanExpression, TargetIntegerExpression, TargetScalarExpression,
        TargetUnitOperation, TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
    };

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, structural_types, live, operations, provenance) = site.parts();
        let mut values = scalar_values(live, &prepared.scalar_parameters)?;

        let lower_byte_observation = |operation, values, provenance_operations| {
            byte_views::lower_byte_observation_with_lengths(
                operation,
                function,
                structural_types,
                &prepared.parameters,
                live.element_view_backings(),
                values,
                &live.lengths,
                provenance_operations,
            )
        };
        let lower_element_observation = |operation, values, provenance_operations| {
            element_views::lower_element_observation_with_lengths(
                operation,
                function,
                structural_types,
                &prepared.parameters,
                live.element_view_backings(),
                values,
                &live.lengths,
                provenance_operations,
            )
        };
        match operation {
            AbstractOperation::ByteSequenceRead { .. } => {
                lower_byte_observation(operation, &mut values, &mut provenance.operations)?;
            }
            AbstractOperation::ByteSequencePredicate { operand, .. } => {
                byte_sequence_operand_gate(function, structural_types, prepared, live, operand)?;
                lower_byte_observation(operation, &mut values, &mut provenance.operations)?;
            }
            AbstractOperation::ByteSequenceEqual { left, right, .. } => {
                for operand in [left, right] {
                    byte_sequence_operand_gate(
                        function,
                        structural_types,
                        prepared,
                        live,
                        operand,
                    )?;
                }
                lower_byte_observation(operation, &mut values, &mut provenance.operations)?;
            }
            AbstractOperation::ElementViewLength { .. }
            | AbstractOperation::ByteSequenceLength { .. }
            | AbstractOperation::ByteSequenceSubslice { .. }
            | AbstractOperation::ElementViewSubslice { .. } => match operation {
                AbstractOperation::ElementViewLength { .. } => {
                    lower_element_observation(operation, &mut values, &mut provenance.operations)?;
                }
                AbstractOperation::ByteSequenceLength { .. } => {
                    lower_byte_observation(operation, &mut values, &mut provenance.operations)?;
                }
                _ => {}
            },
            AbstractOperation::ElementViewRead { result, .. }
                if matches!(result.scalar_type, ScalarType::IeeeFloat(_)) => {}
            AbstractOperation::ElementViewRead { .. } => {
                lower_element_observation(operation, &mut values, &mut provenance.operations)?;
            }
            _ => {}
        }
        if let AbstractOperation::ByteSequenceSubslice {
            psi_operation,
            result,
            ..
        } = operation
        {
            let (view, _) = byte_views::resolve_byte_view(
                function,
                structural_types,
                &prepared.parameters,
                live.element_view_backings(),
                &values,
                &live.lengths,
                result.place,
                &mut Vec::new(),
            )?;
            record_view_result(
                live,
                function.machine,
                structural_types,
                provenance,
                *psi_operation,
                result,
            )?;
            operations.push(TargetUnitOperation::ByteSequenceSubslice {
                result: result.clone(),
                view,
            });
            return Ok(());
        }
        if let AbstractOperation::ElementViewSubslice {
            psi_operation,
            result,
            ..
        } = operation
        {
            let (view, _, _) = element_views::resolve_element_view(
                function,
                structural_types,
                &prepared.parameters,
                live.element_view_backings(),
                &values,
                &live.lengths,
                result.place,
                &mut Vec::new(),
            )?;
            record_view_result(
                live,
                function.machine,
                structural_types,
                provenance,
                *psi_operation,
                result,
            )?;
            operations.push(TargetUnitOperation::ElementViewSubslice {
                result: result.clone(),
                view,
            });
            return Ok(());
        }
        let (psi_operation, result, scalar_type, expression) = match operation {
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                result,
                scalar_type,
                arithmetic,
                left,
                right,
            } if arithmetic.policy() == IntegerOverflowPolicy::Exact
                && matches!(
                    arithmetic.operator(),
                    IntegerArithmeticOperator::Add | IntegerArithmeticOperator::Subtract
                ) =>
            {
                use crate::scalar::{IntegerBinaryKind, lower_integer_binary};
                let known = lower_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::Arithmetic(*arithmetic),
                    *psi_operation,
                )?;
                provenance.operations.push(*psi_operation);
                (
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*scalar_type),
                    TargetScalarExpression::Integer {
                        scalar_type: *scalar_type,
                        expression: known.into_expression(*result),
                    },
                )
            }
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::IntegerBitwiseAnd {
                psi_operation,
                result,
                ..
            }
            | abstract_operations::bitwise_or_xor_pair_pattern!(psi_operation, result, ..)
            | AbstractOperation::IntegerBitwiseNot {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::IntegerShift {
                psi_operation,
                result,
                ..
            } => {
                if !try_lower_integer_operation(operation, &mut values, &mut provenance.operations)?
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let known = values
                    .remove(result)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                let expression = known.into_expression(*result)?;
                let TargetScalarExpression::Integer { scalar_type, .. } = &expression else {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                };
                (
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*scalar_type),
                    expression,
                )
            }
            AbstractOperation::IntegerExactCast {
                psi_operation,
                obligation,
                result,
                source_type,
                target_type,
                operand,
            } => {
                let operand_value = match values.get(operand).cloned() {
                    Some(KnownScalar::Integer { scalar_type, value })
                        if scalar_type == *source_type
                            && source_type.can_exact_cast_to(*target_type) =>
                    {
                        value
                    }
                    Some(_) => return Err(LoweringError::IntegerExactCastTypeMismatch(*result)),
                    None => return Err(LoweringError::UnknownValue(*operand)),
                };
                provenance.operations.push(*psi_operation);
                (
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*target_type),
                    TargetScalarExpression::Integer {
                        scalar_type: *target_type,
                        expression: TargetIntegerExpression::IntegerExactCast {
                            psi_operation: *psi_operation,
                            obligation: *obligation,
                            source_type: *source_type,
                            operand: Box::new(operand_value.into_expression(*operand)),
                        },
                    },
                )
            }
            AbstractOperation::TrappingInteger {
                psi_operation,
                result,
                scalar_type,
                operand_type,
                operation: primitive,
            } => {
                let operand =
                    |value: ValueId, expected: semantic_vocabulary::IntegerType| match values
                        .get(&value)
                        .cloned()
                    {
                        Some(KnownScalar::Integer {
                            scalar_type,
                            value: known,
                        }) if scalar_type == expected => Ok(Box::new(known.into_expression(value))),
                        Some(_) => Err(LoweringError::ValueTypeMismatch(value)),
                        None => Err(LoweringError::UnknownValue(value)),
                    };
                use terminal_psi::TrappingIntegerOperation as T;
                let (left, right) = match *primitive {
                    terminal_psi::trapping_arith_set_pattern!(left, right, ..) => {
                        if operand_type != scalar_type {
                            return Err(LoweringError::ValueTypeMismatch(*result));
                        }
                        (
                            operand(left, *scalar_type)?,
                            Some(operand(right, *scalar_type)?),
                        )
                    }
                    T::ShiftLeft { value, count } | T::ShiftRight { value, count } => (
                        operand(value, *scalar_type)?,
                        Some(operand(count, *operand_type)?),
                    ),
                    T::Convert { operand: source } => (operand(source, *operand_type)?, None),
                };
                provenance.operations.push(*psi_operation);
                (
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*scalar_type),
                    TargetScalarExpression::Integer {
                        scalar_type: *scalar_type,
                        expression: TargetIntegerExpression::Trapping {
                            psi_operation: *psi_operation,
                            primitive: primitive.primitive(),
                            operand_type: *operand_type,
                            left,
                            right,
                        },
                    },
                )
            }
            AbstractOperation::ByteSequenceLength {
                psi_operation,
                result,
                source,
            } => {
                if source.path.is_empty() {
                    live.lengths.insert(result.value, source.place);
                }
                let expression =
                    removed_scalar_expression(&mut values, function.machine, result.value)?;
                (*psi_operation, result.value, result.scalar_type, expression)
            }
            AbstractOperation::ElementViewLength {
                psi_operation,
                result,
                source,
            } => {
                if source.path.is_empty() {
                    live.lengths.insert(result.value, source.place);
                }
                let expression =
                    removed_scalar_expression(&mut values, function.machine, result.value)?;
                (*psi_operation, result.value, result.scalar_type, expression)
            }
            abstract_operations::byte_read_predicate_equal_pattern!(psi_operation, result, ..) => {
                let expression = values
                    .remove(&result.value)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                    .into_expression(result.value)?;
                (*psi_operation, result.value, result.scalar_type, expression)
            }
            AbstractOperation::ElementViewRead {
                psi_operation,
                result,
                ..
            } if matches!(result.scalar_type, ScalarType::IeeeFloat(_)) => {
                let expression = element_views::float_element_read(
                    operation,
                    function,
                    structural_types,
                    &prepared.parameters,
                    live.element_view_backings(),
                    &values,
                    &live.lengths,
                )?;
                provenance.operations.push(*psi_operation);
                (*psi_operation, result.value, result.scalar_type, expression)
            }
            AbstractOperation::ElementViewRead {
                psi_operation,
                result,
                ..
            } => {
                let expression = values
                    .remove(&result.value)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                    .into_expression(result.value)?;
                (*psi_operation, result.value, result.scalar_type, expression)
            }
            AbstractOperation::BooleanEqual {
                psi_operation,
                result,
                left,
                right,
            } => {
                let operand = |value| {
                    let known = values
                        .get(&value)
                        .cloned()
                        .ok_or(LoweringError::UnknownValue(value))?;
                    let TargetScalarExpression::Boolean(expression) =
                        known.into_expression(value)?
                    else {
                        return Err(LoweringError::ValueTypeMismatch(value));
                    };
                    Ok(KnownScalar::BooleanRuntime(expression))
                };
                let known =
                    equal_boolean(operand(*left)?, operand(*right)?, *psi_operation, *result)?;
                provenance.operations.push(*psi_operation);
                (
                    *psi_operation,
                    *result,
                    ScalarType::Boolean,
                    known.into_expression(*result)?,
                )
            }
            AbstractOperation::BooleanNot {
                psi_operation,
                result,
                operand,
            } => {
                let known_operand = values
                    .get(operand)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*operand))?;
                let TargetScalarExpression::Boolean(operand_expression) =
                    known_operand.into_expression(*operand)?
                else {
                    return Err(LoweringError::ValueTypeMismatch(*result));
                };
                provenance.operations.push(*psi_operation);
                (
                    *psi_operation,
                    *result,
                    ScalarType::Boolean,
                    TargetScalarExpression::Boolean(TargetBooleanExpression::Not {
                        psi_operation: *psi_operation,
                        operand: Box::new(operand_expression),
                    }),
                )
            }
            abstract_operations::integer_equal_less_pair_pattern!(
                psi_operation,
                result,
                left,
                right,
                ..
            )
            | AbstractOperation::IntegerLessOrEqual {
                psi_operation,
                result,
                left,
                right,
            } => {
                let left_value = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right_value = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let known = if matches!(operation, AbstractOperation::IntegerEqual { .. }) {
                    equal_integer(
                        *left,
                        left_value,
                        *right,
                        right_value,
                        *psi_operation,
                        *result,
                    )?
                } else {
                    order_integer(
                        *left,
                        left_value,
                        *right,
                        right_value,
                        *psi_operation,
                        *result,
                        matches!(operation, AbstractOperation::IntegerLessOrEqual { .. }),
                    )?
                };
                provenance.operations.push(*psi_operation);
                (
                    *psi_operation,
                    *result,
                    ScalarType::Boolean,
                    known.into_expression(*result)?,
                )
            }
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
        let shape = ValueShape::for_scalar(scalar_type)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let home = TargetUnitScalarHomeRequirement {
            defining_operation: psi_operation,
            source_value: result,
            scalar_type,
            shape,
        };
        match scalar_type {
            ScalarType::Callable(_) | ScalarType::IeeeFloat(_) | ScalarType::Boolean => {
                std::sync::Arc::make_mut(&mut live.scalar_homes).insert(result, home);
            }
            ScalarType::Integer(_) => {
                live.integers.insert(result, KnownUnitInteger::Home(home));
            }
        }
        operations.push(TargetUnitOperation::ScalarDefinition {
            result_home: home,
            expression,
        });
        Ok(())
    }

    pub(crate) fn scalar_values(
        live: &LiveDefinitions,
        parameters: &[ScalarAbiValue],
    ) -> Result<BTreeMap<ValueId, KnownScalar>, LoweringError> {
        let mut values = live
            .integers
            .iter()
            .map(|(value, known)| {
                let scalar_type = known.scalar_type();
                let expression = match *known {
                    KnownUnitInteger::BlockParameter {
                        block,
                        value,
                        scalar_type,
                    } => TargetIntegerExpression::BlockParameter(
                        target_operations::TargetScalarBlockValue {
                            block,
                            value,
                            scalar_type: ScalarType::Integer(scalar_type),
                        },
                    ),
                    KnownUnitInteger::Parameter {
                        parameter_index, ..
                    } => {
                        let parameter_index = usize::try_from(parameter_index)
                            .map_err(|_| LoweringError::UnknownValue(*value))?;
                        let parameter = parameters
                            .get(parameter_index)
                            .ok_or(LoweringError::UnknownValue(*value))?;
                        if parameter.value != *value
                            || parameter.scalar_type != ScalarType::Integer(scalar_type)
                        {
                            return Err(LoweringError::ValueTypeMismatch(*value));
                        }
                        TargetIntegerExpression::Parameter {
                            source_value: *value,
                            parameter_index,
                            location: scalar_parameter_location(
                                &AbstractParameter {
                                    value: *value,
                                    scalar_type: parameter.scalar_type,
                                },
                                &parameter.placement,
                            )?,
                        }
                    }
                    KnownUnitInteger::Immediate { value: literal, .. } => {
                        TargetIntegerExpression::Immediate {
                            source_value: *value,
                            value: literal,
                        }
                    }
                    KnownUnitInteger::Home(home) => TargetIntegerExpression::ScalarHome(home),
                };
                Ok((
                    *value,
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Runtime(expression),
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
        for (parameter_index, parameter) in parameters.iter().enumerate() {
            if parameter.scalar_type == ScalarType::Boolean {
                values.insert(
                    parameter.value,
                    KnownScalar::BooleanRuntime(TargetBooleanExpression::Parameter {
                        source_value: parameter.value,
                        parameter_index,
                        location: scalar_parameter_location(
                            &AbstractParameter {
                                value: parameter.value,
                                scalar_type: parameter.scalar_type,
                            },
                            &parameter.placement,
                        )?,
                    }),
                );
            }
        }
        for (value, (_, immediate)) in &live.booleans {
            values.insert(*value, KnownScalar::Boolean(*immediate));
        }
        for (value, home) in live.scalar_homes.iter() {
            if home.scalar_type == ScalarType::Boolean {
                values.insert(
                    *value,
                    KnownScalar::BooleanRuntime(TargetBooleanExpression::ScalarHome(*home)),
                );
            }
        }
        for (value, parameter) in &live.scalar_block_parameters {
            if matches!(parameter.scalar_type, ScalarType::IeeeFloat(_)) {
                continue;
            }
            if parameter.scalar_type != ScalarType::Boolean || parameter.value != *value {
                return Err(LoweringError::ValueTypeMismatch(*value));
            }
            values.insert(
                *value,
                KnownScalar::BooleanRuntime(TargetBooleanExpression::BlockParameter(*parameter)),
            );
        }
        Ok(values)
    }

    pub(crate) fn bounded_owner_storage(
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        live: &LiveDefinitions,
        source: PlaceId,
    ) -> bool {
        let bounded_owner = |structural_type| {
            structural_types
                .get(structural_type)
                .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
        };
        (live.address_joins.contains(&source)
            && super::transfers::address_parameter(function, live, source)
                .is_some_and(|parameter| bounded_owner(parameter.structural_type)))
            || live
                .structural_homes
                .get(&source)
                .is_some_and(|home| bounded_owner(home.structural_type()))
    }

    fn byte_sequence_operand_gate(
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
        live: &LiveDefinitions,
        operand: &terminal_psi::control_flow::ByteSequenceOperand,
    ) -> Result<(), LoweringError> {
        match operand {
            terminal_psi::control_flow::ByteSequenceOperand::Whole { .. } => {}
            terminal_psi::control_flow::ByteSequenceOperand::Field {
                source,
                field,
                length,
            } => {
                let authority = super::primitive_storage::byte_field_observation(
                    function,
                    structural_types,
                    prepared,
                    live,
                    source.place,
                    &source.path,
                    *field,
                    terminal_semantics::SequenceFieldObservation::Contents,
                )?;
                if source.access != terminal_psi::StructuralAccess::SharedBorrow
                    || authority.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
                    || !function.operations.iter().any(|candidate| {
                        matches!(candidate,
                    AbstractOperation::StructuralByteSequenceFieldLength {
                        source: measured, path, field: measured_field, result, ..
                    } if *measured == source.place && *path == source.path
                        && measured_field == field && result.value == *length)
                    })
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let scalar = live
                    .integers
                    .get(length)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                    .scalar_type();
                if scalar.sign() != IntegerSign::Unsigned
                    || scalar.bits() != 64
                    || scalar.is_address()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
            }
        }
        Ok(())
    }

    fn removed_scalar_expression(
        values: &mut BTreeMap<ValueId, KnownScalar>,
        machine: semantic_vocabulary::MachineId,
        result_value: ValueId,
    ) -> Result<TargetScalarExpression, LoweringError> {
        values
            .remove(&result_value)
            .ok_or_else(|| LoweringError::unsupported_control_flow(machine))?
            .into_expression(result_value)
    }

    fn record_view_result(
        live: &mut LiveDefinitions,
        machine: semantic_vocabulary::MachineId,
        structural_types: &StructuralTypeLookup<'_>,
        provenance: &mut TerminalPsiProvenance,
        psi_operation: semantic_vocabulary::OperationId,
        result: &terminal_psi::StructuralOperationResult,
    ) -> Result<(), LoweringError> {
        if live
            .views
            .insert(result.place, (psi_operation, result.structural_type))
            .is_some()
        {
            return Err(LoweringError::unsupported_control_flow(machine));
        }
        established_home(live, psi_operation, result, structural_types, machine)?;
        provenance.operations.push(psi_operation);
        Ok(())
    }
}

pub(crate) mod primitive_storage {
    use super::{LiveDefinitions, RowSite};
    use crate::LoweringError;
    use crate::function_signature::PreparedFunctionSignature;
    use crate::structural_layout::primitive_read_projection;
    use crate::unit::known_integers::insert_scalar_home;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use calling_conventions::ValueShape;
    use calling_conventions::terminal_layout::StructuralLayouts;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, OperationId, ScalarType, StructuralTypeId,
    };
    use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};
    use target_operations::{TargetUnitOperation, TargetUnitScalarHomeRequirement};
    use terminal_psi::{
        StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralTypeShape,
    };
    use terminal_semantics::observed_field_declaration;
    use terminal_semantics::projected_field_declaration;

    pub(crate) fn is_primitive_reference(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        types: &StructuralLayouts<'_>,
    ) -> bool {
        parameter.multiplicity == StructuralMultiplicity::Unrestricted
            && parameter.access != StructuralAccess::Owned
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty()
            && types
                .get(parameter.structural_type)
                .is_some_and(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(scalar)
                if native_shape(scalar).is_some())
                })
    }

    pub(crate) fn shape(
        scalar: abstract_operations::AbstractResult,
    ) -> Result<ValueShape, LoweringError> {
        native_shape(scalar.scalar_type).ok_or(LoweringError::ValueTypeMismatch(scalar.value))
    }

    pub(crate) fn native_shape(scalar: ScalarType) -> Option<ValueShape> {
        ValueShape::for_scalar(scalar)
    }

    fn unsigned(function: &AbstractFunction, bits: u16) -> Result<ScalarType, LoweringError> {
        Ok(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, bits)
                .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
        ))
    }

    pub(crate) fn byte_field_observation(
        function: &AbstractFunction,
        types: &crate::structural_type_lookup::StructuralTypeLookup<'_>,
        prepared: &PreparedFunctionSignature,
        live: &LiveDefinitions,
        source: semantic_vocabulary::PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
        observation: terminal_semantics::SequenceFieldObservation,
    ) -> Result<terminal_psi::StructuralArgument, LoweringError> {
        let (structural_type, access) = field_root(function, types, prepared, live, source)?;
        let selected =
            projected_field_declaration(types.values().copied(), structural_type, path, field)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if !terminal_semantics::sequence_field_allows_observation(
            types.values().copied(),
            &selected.field_type,
            observation,
        ) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok(terminal_psi::StructuralArgument {
            place: source,
            access,
            path: path.to_vec(),
        })
    }

    pub(crate) fn field_root(
        function: &AbstractFunction,
        types: &crate::structural_type_lookup::StructuralTypeLookup<'_>,
        prepared: &PreparedFunctionSignature,
        live: &LiveDefinitions,
        place: semantic_vocabulary::PlaceId,
    ) -> Result<(StructuralTypeId, StructuralAccess), LoweringError> {
        let structural_type = live
            .views
            .get(&place)
            .map(|(_, identity)| *identity)
            .or_else(|| {
                live.structural_homes
                    .get(&place)
                    .map(|home| home.structural_type())
            })
            .or_else(|| {
                function
                    .arrival_parameter(place)
                    .map(|parameter| parameter.structural_type)
            })
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if matches!(
            types
                .get(structural_type)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::ElementView { .. })
        ) {
            let values = super::observations::scalar_values(live, &prepared.scalar_parameters)?;
            let (_, resolved_type, access) = crate::scalar::element_views::resolve_element_view(
                function,
                types,
                &prepared.parameters,
                live.element_view_backings(),
                &values,
                &live.lengths,
                place,
                &mut Vec::new(),
            )?;
            if resolved_type != structural_type {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            return Ok((structural_type, access));
        }
        if live.structural_homes.contains_key(&place) {
            return Ok((structural_type, StructuralAccess::Owned));
        }
        let parameter = function
            .arrival_parameter(place)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        Ok((structural_type, parameter.access))
    }

    pub(crate) fn field_indices(
        function: &AbstractFunction,
        types: &crate::structural_type_lookup::StructuralTypeLookup<'_>,
        prepared: &PreparedFunctionSignature,
        live: &LiveDefinitions,
        argument: &terminal_psi::StructuralArgument,
    ) -> Result<Vec<target_operations::TargetStructuralRuntimeIndex>, LoweringError> {
        let (root, _) = field_root(function, types, prepared, live, argument.place)?;
        let carrier_path = match argument.path.split_last() {
            Some((terminal_psi::StructuralPathSegment::Case(_), prefix)) => prefix,
            _ => &argument.path,
        };
        let regions =
            crate::structural_layout::reference_projection_regions(root, carrier_path, types)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let (_, _, _, leaf_indices) =
            crate::structural_layout::runtime_projection(regions.root, regions.path, types)?;
        let runs = regions.selector_runs(leaf_indices);
        super::scalar_sources::runtime_indices(
            runs,
            function,
            &super::scalar_sources::ScalarSources::from(live),
        )
    }

    pub(crate) fn retain_result(
        operation: OperationId,
        result: abstract_operations::AbstractResult,
        live: &mut LiveDefinitions,
    ) -> Result<TargetUnitScalarHomeRequirement, LoweringError> {
        let home = TargetUnitScalarHomeRequirement {
            defining_operation: operation,
            source_value: result.value,
            scalar_type: result.scalar_type,
            shape: shape(result)?,
        };
        insert_scalar_home(&mut live.integers, &mut live.scalar_homes, home)?;
        Ok(home)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        prepared: &PreparedFunctionSignature,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let scalar_matches = |identity, scalar_type| {
            types.get(identity).is_some_and(|declaration|
            matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(expected) if expected == scalar_type))
        };
        let (identity, lowered) = match operation {
            AbstractOperation::StructuralByteSequenceFieldLength {
                psi_operation,
                result,
                source,
                path,
                field,
            } => {
                if result.scalar_type != unsigned(function, 64)? {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let source = byte_field_observation(
                    function,
                    types,
                    prepared,
                    live,
                    *source,
                    path,
                    *field,
                    terminal_semantics::SequenceFieldObservation::Length,
                )?;
                let indices = field_indices(function, types, prepared, live, &source)?;
                retain_result(*psi_operation, *result, live)?;
                (
                    *psi_operation,
                    TargetUnitOperation::StructuralByteSequenceFieldLength {
                        psi_operation: *psi_operation,
                        result: *result,
                        source,
                        field: *field,
                        indices,
                    },
                )
            }
            AbstractOperation::StructuralByteSequenceFieldRead {
                psi_operation,
                result,
                source,
                path,
                field,
                index,
                length,
                obligation,
            } => {
                let argument = byte_field_observation(
                    function,
                    types,
                    prepared,
                    live,
                    *source,
                    path,
                    *field,
                    terminal_semantics::SequenceFieldObservation::Contents,
                )?;
                if argument.access == StructuralAccess::WriteOnlyBorrow
                || result.scalar_type != unsigned(function, 8)?
                || !function.operations.iter().any(|operation| matches!(operation,
                    AbstractOperation::StructuralByteSequenceFieldLength { source: measured, path: measured_path, field: measured_field, result: measured_result, .. }
                    if measured == source && terminal_semantics::structural_paths_select_same_location(measured_path, path) && measured_field == field && measured_result.value == *length))
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
                let index_value = *live
                    .integers
                    .get(index)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                let length_value = live
                    .integers
                    .get(length)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if ScalarType::Integer(index_value.scalar_type()) != unsigned(function, 64)?
                    || ScalarType::Integer(length_value.scalar_type()) != unsigned(function, 64)?
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let indices = field_indices(function, types, prepared, live, &argument)?;
                retain_result(*psi_operation, *result, live)?;
                (
                    *psi_operation,
                    TargetUnitOperation::StructuralByteSequenceFieldRead {
                        psi_operation: *psi_operation,
                        result: *result,
                        source: argument,
                        field: *field,
                        indices,
                        index: index_value.into_target_source(*index),
                        length: *length,
                        obligation: *obligation,
                    },
                )
            }
            AbstractOperation::StructuralScalarFieldRead {
                psi_operation,
                result,
                source,
                path,
                field,
            } => {
                let (psi_operation, result, place, field) =
                    (*psi_operation, *result, *source, *field);
                let mut through_reference = false;
                let (structural_type, access) = if let Some((_, structural_type)) =
                    live.views.get(&place)
                {
                    (*structural_type, StructuralAccess::SharedBorrow)
                } else if let Some(descriptor) =
                    live.references
                        .get(&(place, Vec::new()))
                        .and_then(|descriptor| {
                            match types
                                .get(descriptor.carrier_type)
                                .map(|declaration| &declaration.shape)
                            {
                                Some(StructuralTypeShape::Reference {
                                    access: terminal_psi::reading_borrow_pair_pattern!(),
                                    ..
                                }) => Some(descriptor),
                                _ => None,
                            }
                        })
                {
                    let (referent, access) = match types
                        .get(descriptor.carrier_type)
                        .map(|declaration| &declaration.shape)
                    {
                        Some(StructuralTypeShape::Reference { referent, access }) => {
                            (*referent, *access)
                        }
                        _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
                    };
                    let home = live
                        .structural_homes
                        .get(&place)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                    if descriptor.carrier_type != home.structural_type()
                        || descriptor.referent != referent
                        || home.has_claims()
                        || home.multiplicity() == StructuralMultiplicity::Linear
                        || !home.qualifications().is_empty()
                        || !home.projected_qualifications().is_empty()
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    through_reference = true;
                    (referent, access)
                } else if let Some(home) = live.structural_homes.get(&place) {
                    (home.structural_type(), StructuralAccess::Owned)
                } else {
                    let source = function
                        .structural_parameters
                        .iter()
                        .chain(
                            function
                                .block_structural_parameters()
                                .filter(|parameter| live.address_joins.contains(&parameter.place)),
                        )
                        .find(|parameter| parameter.place == place)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                    if !matches!(
                        source.access,
                        StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
                    ) {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    (source.structural_type, source.access)
                };
                let access = terminal_semantics::observed_field_backing_access(
                    types.values().copied(),
                    structural_type,
                    path,
                    access,
                )
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                let declaration = observed_field_declaration(
                    types.values().copied(),
                    structural_type,
                    path,
                    field,
                )
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if matches!(
                    declaration.field_type,
                    StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
                ) && !matches!(access, terminal_psi::mutating_access_pair_pattern!())
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                if declaration.field_type.scalar_type() != Some(result.scalar_type) {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let (carrier_path, case) = match path.split_last() {
                    Some((
                        semantic_vocabulary::CanonicalStructuralPathSegment::Case(case),
                        prefix,
                    )) => (prefix, Some(*case)),
                    _ => (path.as_slice(), None),
                };
                let mut runtime_path = terminal_semantics::canonical_structural_storage_path(
                    types.values().copied(),
                    structural_type,
                    carrier_path,
                )
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if through_reference {
                    runtime_path.insert(0, terminal_psi::StructuralPathSegment::Referent);
                }
                retain_result(psi_operation, result, live)?;
                (
                    psi_operation,
                    TargetUnitOperation::StructuralScalarFieldRead {
                        psi_operation,
                        result,
                        source: terminal_psi::StructuralArgument {
                            place,
                            access,
                            path: runtime_path,
                        },
                        case,
                        field,
                    },
                )
            }
            AbstractOperation::EstablishPrimitiveLocal {
                psi_operation,
                result,
                value,
                ..
            } => {
                if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || !scalar_matches(result.structural_type, value.scalar_type)
                    || super::scalar_sources::source(value.value, function, live)?.scalar_type()
                        != value.scalar_type
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let shape = shape(*value)?;
                if live
                    .structural_homes
                    .insert(
                        result.place,
                        TargetStructuralHomeRequirement {
                            origin:
                                target_operations::TargetStructuralHomeOrigin::OperationResult {
                                    operation: *psi_operation,
                                    result: result.clone(),
                                },
                            layout: TargetStructuralHomeLayout::Aggregate(shape),
                        },
                    )
                    .is_some()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                (
                    *psi_operation,
                    TargetUnitOperation::EstablishPrimitiveLocal {
                        psi_operation: *psi_operation,
                        result: result.clone(),
                        value: *value,
                        shape,
                    },
                )
            }
            AbstractOperation::PrimitiveLocalStore {
                psi_operation,
                destination,
                value,
            } => {
                let home = live
                    .structural_homes
                    .get(destination)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if !scalar_matches(home.structural_type(), value.scalar_type)
                    || super::scalar_sources::source(value.value, function, live)?.scalar_type()
                        != value.scalar_type
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                (
                    *psi_operation,
                    TargetUnitOperation::PrimitiveLocalStore {
                        psi_operation: *psi_operation,
                        destination: *destination,
                        value: *value,
                    },
                )
            }
            AbstractOperation::StorageAddress {
                psi_operation,
                result,
                source,
                path,
            } => {
                let identity = live
                    .structural_homes
                    .get(source)
                    .map(|home| home.structural_type())
                    .or_else(|| live.views.get(source).map(|(_, identity)| *identity))
                    .or_else(|| {
                        function
                            .structural_parameters
                            .iter()
                            .chain(function.block_structural_parameters())
                            .find(|parameter| parameter.place == *source)
                            .map(|parameter| parameter.structural_type)
                    })
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                let runs =
                    crate::structural_layout::storage_address_indices(identity, path, types)?;
                let indices = super::scalar_sources::runtime_indices(
                    runs,
                    function,
                    &super::scalar_sources::ScalarSources::from(&*live),
                )?;
                retain_result(*psi_operation, *result, live)?;
                (
                    *psi_operation,
                    TargetUnitOperation::StorageAddress {
                        psi_operation: *psi_operation,
                        result: *result,
                        source: *source,
                        path: path.clone(),
                        indices,
                    },
                )
            }
            AbstractOperation::PrimitiveScalarRead {
                psi_operation,
                result,
                source,
                path,
            } => {
                let identity = if let Some(home) = live.structural_homes.get(source) {
                    home.structural_type()
                } else {
                    function
                        .structural_parameters
                        .iter()
                        .chain(function.block_structural_parameters().filter(|parameter| {
                            live.address_joins.contains(&parameter.place)
                                || live.block_views.contains(&parameter.place)
                        }))
                        .find(|parameter| {
                            parameter.place == *source
                                && (matches!(
                                    parameter.access,
                                    terminal_psi::reading_borrow_pair_pattern!()
                                ) || parameter.access == StructuralAccess::Owned)
                        })
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                        .structural_type
                };
                let projection = primitive_read_projection(identity, path, types)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if projection.scalar != result.scalar_type {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let runs = projection.selector_runs();
                let indices = super::scalar_sources::runtime_indices(
                    runs,
                    function,
                    &super::scalar_sources::ScalarSources::from(&*live),
                )?;
                retain_result(*psi_operation, *result, live)?;
                (
                    *psi_operation,
                    TargetUnitOperation::PrimitiveScalarRead {
                        psi_operation: *psi_operation,
                        result: *result,
                        source: *source,
                        path: path.clone(),
                        indices,
                    },
                )
            }
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
        operations.push(lowered);
        provenance.operations.push(identity);
        Ok(())
    }
}

pub(crate) mod records {
    use super::RowSite;
    use super::aggregate_results::established_home;
    use crate::LoweringError;
    use abstract_operations::AbstractOperation;
    use std::collections::BTreeSet;
    use target_operations::TargetUnitOperation;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::{
        StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralTypeShape,
    };
    use terminal_semantics::inline_carrier_declaration;

    pub(crate) fn establish(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let (psi_operation, result, fields, result_case, elements) = match operation {
            AbstractOperation::EstablishRecord {
                psi_operation,
                result,
                fields,
            } => (psi_operation, result, fields.as_slice(), None, None),
            AbstractOperation::EstablishCase {
                psi_operation,
                result,
                fields,
                result_case,
            } => (
                psi_operation,
                result,
                fields.as_slice(),
                Some(*result_case),
                None,
            ),
            AbstractOperation::EstablishStructuralArray {
                psi_operation,
                result,
                elements,
            } => (psi_operation, result, &[][..], None, Some(elements)),
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
        let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
        let shape = &types
            .get(result.structural_type)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?
            .shape;
        let declarations: Vec<(
            Option<semantic_vocabulary::StructuralCaseId>,
            &StructuralFieldDeclaration,
        )> = match (shape, result_case) {
            (StructuralTypeShape::Record { fields, .. }, None) => fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| (None, field))
                .collect(),
            (StructuralTypeShape::Sum { cases }, Some(result_case)) => cases
                .iter()
                .find(|case| case.id == result_case)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?
                .fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| (Some(result_case), field))
                .collect(),
            (
                StructuralTypeShape::Mixed {
                    fields: common,
                    cases,
                },
                Some(result_case),
            ) => common
                .iter()
                .filter(|field| !matches!(field.field_type, StructuralFieldType::Erased { .. }))
                .map(|field| (None, field))
                .chain(
                    cases
                        .iter()
                        .find(|case| case.id == result_case)
                        .ok_or(LoweringError::unsupported_control_flow(function.machine))?
                        .fields
                        .iter()
                        .filter(|field| !field.relevance.is_erased())
                        .map(|field| (Some(result_case), field)),
                )
                .collect(),
            (
                StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(length),
                    ..
                },
                None,
            ) if elements
                .is_some_and(|elements| u64::try_from(elements.len()).ok() == Some(*length)) =>
            {
                Vec::new()
            }
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
        if declarations.len() != fields.len() || live.structural_homes.contains_key(&result.place) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut consumed = BTreeSet::new();
        let mut relocations = Vec::new();
        let mut moved_leaves = BTreeSet::new();
        let element_type = match shape {
            StructuralTypeShape::FixedArray { element, .. } => {
                Some(StructuralFieldType::Structural(*element))
            }
            _ => None,
        };
        enum Input<'a> {
            Field(&'a terminal_psi::RecordFieldValue),
            Element(&'a terminal_psi::StructuralArgument),
        }
        let operands = fields
            .iter()
            .zip(declarations.iter().copied())
            .map(|(field, (case, declaration))| {
                if field.field != declaration.id || declaration.relevance.is_erased() {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let mut destination = Vec::new();
                if let Some(case) = case {
                    destination.push(terminal_psi::StructuralPathSegment::Case(case));
                }
                destination.push(StructuralPathSegment::Field(declaration.identity));
                Ok((
                    &declaration.field_type,
                    Some(declaration),
                    destination,
                    Input::Field(&field.value),
                ))
            })
            .chain(
                elements
                    .into_iter()
                    .flatten()
                    .enumerate()
                    .map(|(index, argument)| {
                        Ok((
                            element_type
                                .as_ref()
                                .ok_or(LoweringError::unsupported_control_flow(function.machine))?,
                            None,
                            vec![StructuralPathSegment::FixedIndex(index as u64)],
                            Input::Element(argument),
                        ))
                    }),
            );
        for operand in operands {
            let (field_type, field_declaration, destination, value) = operand?;
            match value {
                Input::Field(terminal_psi::RecordFieldValue::Scalar {
                    value,
                    range_obligation,
                }) => {
                    let source = super::scalar_sources::source(*value, function, live)?;
                    if field_type.scalar_type() != Some(source.scalar_type())
                        || terminal_semantics::scalar_field_requires_evidence(field_type)
                            != range_obligation.is_some()
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                }
                Input::Field(terminal_psi::RecordFieldValue::Structural(argument))
                | Input::Element(argument) => {
                    let byte_carrier = match field_type {
                        StructuralFieldType::Structural(child) => {
                            types
                                .get(*child)
                                .and_then(|declaration| match declaration.shape {
                                    StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                                    _ => None,
                                })
                        }
                        field_type => field_type.byte_sequence_carrier(),
                    }
                    .filter(|carrier| {
                        matches!(
                            carrier,
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    });
                    let descriptor_member = match field_type {
                        StructuralFieldType::Structural(child) => {
                            types.get(*child).is_some_and(|child| {
                                matches!(child.shape, StructuralTypeShape::ElementView { .. })
                                    || child.shape.is_borrowed_byte_carrier()
                            })
                        }
                        field_type => field_type
                            .byte_sequence_carrier()
                            .is_some_and(|carrier| carrier.is_borrowed()),
                    };
                    let nested = match field_type {
                        StructuralFieldType::Structural(nested) => *nested,
                        _ if byte_carrier.is_some() => {
                            let nested = live
                                .structural_observation_type(function, argument.place)
                                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                            if !types.get(nested).is_some_and(|declaration| {
                                Some(&declaration.shape)
                                    == byte_carrier.map(StructuralTypeShape::ByteSequence).as_ref()
                            }) {
                                return Err(LoweringError::unsupported_control_flow(
                                    function.machine,
                                ));
                            }
                            nested
                        }
                        _ if descriptor_member => {
                            let nested = live
                                .structural_observation_type(function, argument.place)
                                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                            let expected = field_declaration.and_then(|field| {
                                inline_carrier_declaration(types.values().copied(), field)
                            });
                            if expected.is_none_or(|declaration| {
                                terminal_semantics::borrowed_referent_presentation(
                                    types.values().copied(),
                                    nested,
                                    declaration.id,
                                ) != Some(
                                    terminal_semantics::BorrowedReferentPresentation::Unchanged,
                                )
                            }) {
                                return Err(LoweringError::InvalidStructuralValueInput {
                                    machine: function.machine,
                                    place: argument.place,
                                    reason: "the view descriptor differs from its receiving field carrier",
                                });
                            }
                            nested
                        }
                        _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
                    };
                    let reference_bearing = super::references::contains_reference(types, nested);
                    let (selected, multiplicity) =
                        super::aggregate_results::value_input(function, types, live, argument)?;
                    if selected != nested {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    if argument.access != StructuralAccess::Owned
                        || (byte_carrier.is_some()
                            && multiplicity != StructuralMultiplicity::Unrestricted)
                        || (multiplicity != StructuralMultiplicity::Unrestricted
                            && (consumed.iter().any(
                                |(place, path): &(
                                    semantic_vocabulary::PlaceId,
                                    Vec<terminal_psi::StructuralPathSegment>,
                                )| {
                                    *place == argument.place
                                        && (path.starts_with(&argument.path)
                                            || argument.path.starts_with(path))
                                },
                            ) || !consumed.insert((argument.place, argument.path.clone()))))
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    if reference_bearing && !descriptor_member {
                        super::references::subtree_leaves(
                            function,
                            types,
                            live,
                            &destination,
                            nested,
                            argument,
                            multiplicity != StructuralMultiplicity::Unrestricted,
                            &mut relocations,
                            &mut moved_leaves,
                        )?;
                    }
                }
            }
        }
        if live
            .references
            .keys()
            .any(|(carrier, _)| *carrier == result.place)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        for (place, path) in consumed {
            if path.is_empty() {
                live.structural_homes.remove(&place);
            }
        }
        super::references::relocate_leaves(live, result.place, relocations, moved_leaves);
        live.structural_homes
            .insert(result.place, result_home.clone());
        operations.push(if let Some(elements) = elements {
            TargetUnitOperation::EstablishStructuralArray {
                psi_operation: *psi_operation,
                result_home,
                elements: elements.clone(),
            }
        } else {
            match result_case {
                Some(result_case) => TargetUnitOperation::EstablishCase {
                    psi_operation: *psi_operation,
                    result_home,
                    result_case,
                    fields: fields.to_vec(),
                },
                None => TargetUnitOperation::EstablishRecord {
                    psi_operation: *psi_operation,
                    result_home,
                    fields: fields.to_vec(),
                },
            }
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }

    pub(crate) fn establish_trivial_affine_local(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let AbstractOperation::EstablishTrivialAffineLocal {
            psi_operation,
            place,
            structural_type,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        if !matches!(
            place.kind,
            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                structural_type: declared,
                construction: None,
                ..
            } if declared == structural_type.id
        ) || types.get(structural_type.id).copied() != Some(structural_type)
            || !matches!(&structural_type.shape, StructuralTypeShape::Record { fields, .. } if fields.is_empty())
            || live.structural_homes.contains_key(&place.id)
            || live
                .trivial_affine_locals
                .insert(place.id, (*psi_operation, structural_type.id))
                .is_some()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        operations.push(TargetUnitOperation::EstablishTrivialAffineLocal {
            psi_operation: *psi_operation,
            place: *place,
            structural_type: structural_type.clone(),
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }

    pub(crate) fn establish_structural_local(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let AbstractOperation::EstablishStructuralLocal {
            psi_operation,
            result,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let result_home = established_home(live, *psi_operation, result, types, function.machine)?;
        if terminal_semantics::descriptor_results::is_descriptor(
            types.values().copied(),
            result.structural_type,
        ) && live
            .views
            .insert(result.place, (*psi_operation, result.structural_type))
            .is_some()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        operations.push(TargetUnitOperation::EstablishStructuralLocal {
            psi_operation: *psi_operation,
            result_home,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }

    pub(crate) fn establish_zeroed_structural(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let AbstractOperation::EstablishZeroedStructural {
            psi_operation,
            result,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let result_home = established_home(live, *psi_operation, result, types, function.machine)?;
        operations.push(TargetUnitOperation::EstablishZeroedStructural {
            psi_operation: *psi_operation,
            result_home,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}
