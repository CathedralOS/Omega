//! Ordered graph operations and their durable scalar result homes.
use super::observations;
use super::{KnownUnitInteger, RowSite};
use crate::LoweringError;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, PlaceId, ScalarType};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    BoundaryRealization, BoundarySettlementBinding, BoundarySettlementRealization,
    ScalarFunctionAbi, TargetStructuralParameter, TargetUnitOperation,
    TargetUnitScalarHomeRequirement,
};
use terminal_psi::StructuralTypeShape;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_operation(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    selected_calls: &crate::lowering::selected_boundary_calls::SelectedCallsByCall,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    native_callbacks: &BTreeMap<OperationId, Vec<target_operations::TargetNativeCallbackArgument>>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
) -> Result<(), LoweringError> {
    let (function, structural_types) = (site.function, site.types);
    let selected = if let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        ..
    } = operation
    {
        selected_calls.get(&(function.machine, (*psi_operation).into(), *boundary))
    } else {
        None
    };
    let resolved = selected
        .map(|selected| super::selected_boundary_calls::resolve(operation, selected, functions))
        .transpose()?;
    let operation = resolved
        .as_ref()
        .map_or(operation, |(operation, _)| operation);
    // Opaque foreign ABI calls cannot consume reference-carrier storage. This
    // is a native representation restriction, not a second borrow judgment.
    // Ordinary site.operations go straight to their storage/address owner below;
    // Psi owns access legality and the corpus independently audits custody.
    let boundary_arguments: &[terminal_psi::StructuralArgument] = match operation {
        AbstractOperation::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        _ => &[],
    };
    if !super::references::moved_argument_leaves(site.live, boundary_arguments).is_empty() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let first_output = site.operations.len();
    match operation {
        AbstractOperation::EstablishActivationClaim {
            psi_operation,
            result,
            bound,
            committed,
            backing_type,
            established_case,
            rejected_case,
        } => {
            let committed = super::scalar_sources::source(*committed, function, site.live)?;
            if committed.scalar_type()
                != ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
                )
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let result_home =
                super::aggregate_results::home(*psi_operation, result, structural_types)?;
            if site
                .live
                .structural_homes
                .insert(result.place, result_home.clone())
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            site.operations
                .push(TargetUnitOperation::EstablishActivationClaim {
                    psi_operation: *psi_operation,
                    result_home,
                    bound: *bound,
                    committed,
                    backing_type: *backing_type,
                    established_case: *established_case,
                    rejected_case: *rejected_case,
                });
            site.provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::ReleaseActivationClaim {
            psi_operation,
            source,
            claim,
        } => {
            site.operations
                .push(TargetUnitOperation::ReleaseActivationClaim {
                    psi_operation: *psi_operation,
                    source: source.clone(),
                    claim: *claim,
                });
            site.provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::SplitActivationClaim {
            psi_operation,
            result,
            source,
            claim,
            bound_at,
            committed_at,
            covered_case,
            rejected_case,
        } => {
            let committed = super::scalar_sources::source(*committed_at, function, site.live)?;
            let result_home =
                super::aggregate_results::home(*psi_operation, result, structural_types)?;
            if site
                .live
                .structural_homes
                .insert(result.place, result_home.clone())
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            site.operations
                .push(TargetUnitOperation::SplitActivationClaim {
                    psi_operation: *psi_operation,
                    result_home,
                    source: source.clone(),
                    claim: *claim,
                    bound_at: *bound_at,
                    committed,
                    covered_case: *covered_case,
                    rejected_case: *rejected_case,
                });
            site.provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::MergeActivationClaim {
            psi_operation,
            result,
            lower,
            upper,
            lower_claim,
            upper_claim,
            covered_case,
            rejected_case,
        } => {
            let result_home =
                super::aggregate_results::home(*psi_operation, result, structural_types)?;
            if site
                .live
                .structural_homes
                .insert(result.place, result_home.clone())
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            site.operations
                .push(TargetUnitOperation::MergeActivationClaim {
                    psi_operation: *psi_operation,
                    result_home,
                    lower: lower.clone(),
                    upper: upper.clone(),
                    lower_claim: *lower_claim,
                    upper_claim: *upper_claim,
                    covered_case: *covered_case,
                    rejected_case: *rejected_case,
                });
            site.provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::RegisterAccess {
            psi_operation,
            service,
            event,
            result,
        } => {
            let event = event.try_map_inputs(|value| {
                super::scalar_sources::source(*value, function, site.live)
            })?;
            let result_home = result
                .map(|result| {
                    super::primitive_storage::retain_result(*psi_operation, result, site.live)
                })
                .transpose()?;
            site.operations.push(TargetUnitOperation::RegisterAccess {
                psi_operation: *psi_operation,
                service: *service,
                event,
                result_home,
            });
            site.provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::PortWrite {
            psi_operation,
            service,
            port,
            value,
        } => {
            site.operations.push(TargetUnitOperation::PortWrite {
                psi_operation: *psi_operation,
                service: *service,
                port: *port,
                value: *value,
            });
            site.provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::AtomicEvent {
            psi_operation,
            event,
            ..
        } => super::atomic_access::lower(*psi_operation, event, site, prepared),
        AbstractOperation::FloatResultPolicy {
            psi_operation,
            result,
            format,
            policy,
        } => {
            // Resolve every input from the same site.live environment as the
            // preceding call. The adapter never invokes its provider again.
            let source = |value| super::scalar_sources::source(value, function, site.live);
            let policy = match policy {
                terminal_psi::FloatResultPolicy::Trapping { raw_result } => {
                    target_operations::TargetFloatResultPolicy::Trapping {
                        raw_result: source(*raw_result)?,
                    }
                }
                terminal_psi::FloatResultPolicy::Saturating {
                    raw_result,
                    eligible,
                } => target_operations::TargetFloatResultPolicy::Saturating {
                    eligible: source(*eligible)?,
                    raw_result: source(*raw_result)?,
                },
            };
            let scalar_type = ScalarType::IeeeFloat(*format);
            if !policy
                .semantic_policy()
                .accepts_operand_types(*format, |value| {
                    policy
                        .operands()
                        .find(|operand| operand.source_value() == value)
                        .map(|operand| operand.scalar_type())
                })
            {
                return Err(LoweringError::ValueTypeMismatch(*result));
            }
            let result_home = TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type,
                shape: crate::lowering::scalar_abi::fixed_native_scalar_shape(scalar_type)
                    .ok_or(LoweringError::ValueTypeMismatch(*result))?,
            };
            if site
                .live
                .scalar_homes
                .insert(*result, result_home)
                .is_some()
            {
                return Err(LoweringError::DuplicateValue(*result));
            }
            site.provenance.operations.push(*psi_operation);
            site.operations
                .push(TargetUnitOperation::FloatResultPolicy {
                    psi_operation: *psi_operation,
                    result_home,
                    format: *format,
                    policy,
                });
            Ok(())
        }
        AbstractOperation::IeeeFloatCompare {
            psi_operation,
            result,
            comparison,
            format,
            left,
            right,
        } => {
            let left = super::scalar_sources::source(*left, function, site.live)?;
            let right = super::scalar_sources::source(*right, function, site.live)?;
            if left.scalar_type() != ScalarType::IeeeFloat(*format)
                || right.scalar_type() != ScalarType::IeeeFloat(*format)
            {
                return Err(LoweringError::ValueTypeMismatch(*result));
            }
            let result_home = TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type: ScalarType::Boolean,
                shape: ValueShape::integer(1, 1),
            };
            site.live.scalar_homes.insert(*result, result_home);
            site.provenance.operations.push(*psi_operation);
            site.operations.push(TargetUnitOperation::IeeeFloatCompare {
                result_home,
                comparison: *comparison,
                format: *format,
                left,
                right,
            });
            Ok(())
        }
        AbstractOperation::ScalarRepresentationView {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => {
            let operand = super::scalar_sources::source(*operand, function, site.live)?;
            if operand.scalar_type() != *source_type
                || terminal_semantics::ScalarRepresentationViewPair::classify(
                    *source_type,
                    *target_type,
                )
                .is_none()
            {
                return Err(LoweringError::ValueTypeMismatch(*result));
            }
            let result_home = TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type: *target_type,
                shape: crate::lowering::scalar_abi::fixed_native_scalar_shape(*target_type)
                    .ok_or(LoweringError::ValueTypeMismatch(*result))?,
            };
            let duplicate = match target_type {
                ScalarType::Integer(_) => site
                    .live
                    .integers
                    .insert(*result, KnownUnitInteger::Home(result_home))
                    .is_some(),
                ScalarType::Boolean | ScalarType::IeeeFloat(_) => site
                    .live
                    .scalar_homes
                    .insert(*result, result_home)
                    .is_some(),
            };
            if duplicate {
                return Err(LoweringError::DuplicateValue(*result));
            }
            site.provenance.operations.push(*psi_operation);
            site.operations
                .push(TargetUnitOperation::ScalarRepresentationView {
                    result_home,
                    operand,
                });
            Ok(())
        }
        AbstractOperation::EstablishScalarArray { .. } => {
            super::scalar_arrays::establish(operation, site)
        }
        AbstractOperation::EstablishNativeCodeLayout {
            psi_operation,
            result,
            machine,
        } => {
            let result_home =
                super::aggregate_results::home(*psi_operation, result, structural_types)?;
            site.live
                .structural_homes
                .insert(result.place, result_home.clone());
            site.operations
                .push(TargetUnitOperation::EstablishNativeCodeLayout {
                    psi_operation: *psi_operation,
                    result_home,
                    machine: *machine,
                });
            site.provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::EstablishRecord { .. }
        | abstract_operations::structural_array_or_case_pattern!(..) => {
            super::records::establish(operation, site)
        }
        AbstractOperation::EstablishPlacedReference { .. } => {
            super::references::establish_placed(operation, site)
        }
        AbstractOperation::EstablishReference { .. } => {
            super::references::establish(operation, site)
        }
        AbstractOperation::ReleaseReference { .. } => super::references::release(operation, site),
        AbstractOperation::SelectImplementation {
            psi_operation,
            result,
            table,
        } => {
            if dynamic_dispatch.table(*table).is_none() {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let result_home =
                super::primitive_storage::retain_result(*psi_operation, *result, site.live)?;
            site.provenance.operations.push(*psi_operation);
            site.operations
                .push(TargetUnitOperation::SelectImplementation {
                    result_home,
                    table: *table,
                });
            Ok(())
        }
        // Direct and selected calls share argument transport and result homes;
        // the call target alone determines how invocation finds executable code.
        abstract_operations::call_or_call_structural_pattern!(..) => super::calls::lower_call(
            operation,
            site,
            target,
            functions,
            dynamic_dispatch,
            scalar_abis,
            prepared,
            parameters_by_place,
        ),
        AbstractOperation::StructuralCaseMembership { .. } => {
            super::structural_case::observe(operation, site)
        }
        AbstractOperation::StructuralLeafCopy { .. }
        | AbstractOperation::MoveStructuralValue { .. }
        | AbstractOperation::TransformContent { .. } => {
            super::leaf_copy::transport(operation, site)
        }
        AbstractOperation::MoveStructuralField { .. } => {
            super::borrowed_windows::move_field(operation, site, prepared)
        }
        AbstractOperation::StoreStructuralField { .. } => {
            super::borrowed_windows::store_field(operation, site, prepared)
        }
        AbstractOperation::EstablishZeroedStructural { .. } => {
            super::records::establish_zeroed_structural(operation, site)
        }
        AbstractOperation::EstablishStructuralLocal { .. } => {
            super::records::establish_structural_local(operation, site)
        }
        AbstractOperation::EstablishTrivialAffineLocal { .. } => {
            super::records::establish_trivial_affine_local(operation, site)
        }
        AbstractOperation::EstablishPrimitiveLocal { .. }
        | AbstractOperation::PrimitiveLocalStore { .. }
        | AbstractOperation::StorageAddress { .. }
        | AbstractOperation::PrimitiveScalarRead { .. }
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | AbstractOperation::StructuralScalarFieldRead { .. } => {
            super::primitive_storage::lower(operation, site, prepared)
        }
        AbstractOperation::StructuralByteSequenceFieldByteStore { .. } => {
            super::byte_write::replace_byte(operation, site, prepared)
        }
        AbstractOperation::StructuralByteViewFieldStore { .. } => {
            super::byte_write::replace_view_descriptor(operation, site, prepared)
        }
        abstract_operations::byte_sequence_store_pair_pattern!(..) => {
            super::byte_write::replace(operation, site, prepared)
        }
        AbstractOperation::ByteSequenceWrite { .. } => {
            super::byte_write::lower(operation, site, prepared)
        }
        AbstractOperation::ElementViewWrite { .. } => {
            super::element_view_write::lower(operation, site, prepared)
        }
        AbstractOperation::EstablishByteSequenceConcatenation { .. } => {
            super::byte_concatenation::lower(operation, site, prepared)
        }
        AbstractOperation::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes,
            qualifications,
        } => {
            if !matches!(
                place.kind,
                semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type: declared,
                    ..
                } if declared == structural_type.id
            ) || !matches!(
                structural_type.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
            ) || structural_types.get(&structural_type.id).copied() != Some(structural_type)
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            if site
                .live
                .views
                .insert(place.id, (*psi_operation, structural_type.id))
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            site.operations
                .push(TargetUnitOperation::EstablishByteSequenceLiteral {
                    psi_operation: *psi_operation,
                    place: *place,
                    structural_type: structural_type.clone(),
                    bytes: bytes.clone(),
                    qualifications: qualifications.clone(),
                });
            site.provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::EstablishElementView {
            psi_operation,
            result,
            destination,
            ..
        } => {
            let values = observations::scalar_values(site.live, &prepared.scalar_parameters)?;
            let (view, _) = super::super::scalar::element_views::element_view_for_place(
                function,
                structural_types,
                &prepared.parameters,
                site.live.element_view_backings(),
                &values,
                &site.live.lengths,
                *destination,
                &mut Vec::new(),
            )?;
            if site
                .live
                .views
                .insert(*destination, (*psi_operation, result.structural_type))
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            // Both shared and mutable establishments own their descriptor,
            // while the backing access remains the view's separate permission.
            // Record the exact producer contract for forwarding copyable shared
            // descriptors as well as cleaning up affine mutable descriptors.
            let home = super::aggregate_results::home(*psi_operation, result, structural_types)?;
            if site
                .live
                .structural_homes
                .insert(*destination, home)
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            site.operations
                .push(TargetUnitOperation::EstablishElementView {
                    result: result.clone(),
                    view,
                });
            site.provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::StructuralScalarFieldStore { .. } => {
            let reference_destination = match operation {
                AbstractOperation::StructuralScalarFieldStore {
                    destination, path, ..
                } => super::references::referent_projection(
                    function,
                    site.live,
                    structural_types,
                    destination.place(),
                    path,
                    terminal_psi::StructuralAccess::WriteOnlyBorrow,
                ),
                _ => None,
            };
            crate::lowering::unit::lower_field_store(
                operation,
                function,
                structural_types,
                parameters_by_place,
                &site.live.structural_homes,
                reference_destination,
                &super::scalar_sources::ScalarSources::from(&*site.live),
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                site.operations,
                site.provenance,
            )
        }
        AbstractOperation::WriteOnlyPrimitiveStore { value, .. } => {
            crate::lowering::unit::write_only_primitive_store::lower_write_only_primitive_store(
                operation,
                function,
                structural_types,
                parameters_by_place,
                &site.live.structural_homes,
                &site.live.integers,
                &site.live.booleans,
                &site.live.ieee_float_constants,
                &site.live.scalar_homes,
                site.live.scalar_block_parameters.get(&value.value).copied(),
                &super::scalar_sources::ScalarSources::from(&*site.live),
                site.operations,
                site.provenance,
            )
        }
        AbstractOperation::BoundaryCall { boundary, .. } => {
            // One admitted settlement selects the row owner; the recognizer is
            // read once here instead of once per guard, and the row owners no
            // longer re-derive the same realization.
            let Some(realization) = settlements
                .get(boundary)
                .map(|binding| &binding.realization)
            else {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            };
            match realization {
                BoundarySettlementRealization::Builtin(
                    BoundaryRealization::PlacementLifecycle { .. },
                ) => super::placement_lifecycles::lower(
                    operation,
                    site,
                    boundary_machines,
                    settlements,
                ),
                BoundarySettlementRealization::Builtin(
                    BoundaryRealization::PlacedScalarAccess(_),
                ) => super::placed_scalar_access::lower(operation, site, settlements),
                BoundarySettlementRealization::Builtin(
                    BoundaryRealization::ProgramImageRetainedSuffixStart,
                ) => super::program_image::lower(
                    operation,
                    target,
                    boundary_machines,
                    settlements,
                    site,
                ),
                BoundarySettlementRealization::Builtin(
                    BoundaryRealization::NearestIeeeFloatArithmetic(_),
                ) => super::ieee_float_arithmetic::lower(
                    operation,
                    site,
                    target,
                    boundary_machines,
                    settlements,
                ),
                BoundarySettlementRealization::Builtin(
                    BoundaryRealization::HostedWriteByteI32(_)
                    | BoundaryRealization::HostedExitProcessI32(_)
                    | BoundaryRealization::HostedReadByte(_),
                )
                | BoundarySettlementRealization::NormalizedForeignCall(_) => {
                    crate::lowering::unit::boundary_call::lower_boundary_call(
                        operation,
                        function,
                        target,
                        structural_types,
                        boundary_machines,
                        settlements,
                        native_callbacks,
                        parameters_by_place,
                        &site.live.structural_homes,
                        &site.live.address_joins,
                        &site.live.block_views,
                        &mut BTreeMap::new(),
                        &mut BTreeSet::new(),
                        &BTreeMap::new(),
                        &mut site.live.integers,
                        &mut site.live.scalar_homes,
                        &site.live.booleans,
                        &site.live.ieee_float_constants,
                        &site.live.scalar_block_parameters,
                        site.operations,
                        site.provenance,
                    )?;
                    if let Some(
                        TargetUnitOperation::BoundarySettlement {
                            result: target_operations::TargetBoundaryResult::Structural(home),
                            ..
                        }
                        | TargetUnitOperation::NormalizedForeignCall {
                            result: target_operations::TargetBoundaryResult::Structural(home),
                            ..
                        },
                    ) = site.operations.last()
                        && site
                            .live
                            .structural_homes
                            .insert(home.place(), home.clone())
                            .is_some()
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    Ok(())
                }
                _ => Err(LoweringError::unsupported_control_flow(function.machine)),
            }
        }
        AbstractOperation::ByteSequenceLength { .. }
        | abstract_operations::byte_read_predicate_equal_pattern!(..)
        | abstract_operations::element_view_or_byte_subslice_pattern!(..)
        | AbstractOperation::ElementViewLength { .. }
        | AbstractOperation::ElementViewRead { .. }
        | abstract_operations::integer_equal_less_pair_pattern!(..)
        | AbstractOperation::IntegerLessOrEqual { .. }
        | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
        | AbstractOperation::IntegerArithmetic { .. }
        | AbstractOperation::TrappingInteger { .. }
        | AbstractOperation::IntegerBitwiseAnd { .. }
        | abstract_operations::bitwise_or_xor_pair_pattern!(..)
        | AbstractOperation::IntegerBitwiseNot { .. }
        | AbstractOperation::IntegerShift { .. }
        | AbstractOperation::IntegerExactCast { .. } => {
            observations::lower(operation, site, prepared)
        }
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type: ScalarType::Integer(scalar_type),
            value,
        } => crate::lowering::unit::scalar_definitions::lower_integer_constant(
            function.machine,
            *psi_operation,
            *result,
            *scalar_type,
            *value,
            false,
            &mut BTreeMap::new(),
            &mut site.live.integers,
            site.operations,
            site.provenance,
        ),
        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => crate::lowering::unit::scalar_definitions::lower_boolean_constant(
            function.machine,
            *psi_operation,
            *result,
            *value,
            false,
            &mut site.live.booleans,
            site.operations,
            site.provenance,
        ),
        AbstractOperation::IeeeFloatConstant {
            psi_operation,
            result,
            value,
        } => crate::lowering::unit::scalar_definitions::lower_ieee_float_constant(
            *psi_operation,
            *result,
            *value,
            false,
            &mut site.live.ieee_float_constants,
            site.operations,
            site.provenance,
        ),
        AbstractOperation::IntegerWiden { .. } => {
            crate::lowering::unit::scalar_definitions::lower_integer_widen(
                operation,
                function.machine,
                &prepared.scalar_parameters,
                false,
                &mut site.live.integers,
                site.operations,
                site.provenance,
            )
        }
        _ => Err(LoweringError::unsupported_control_flow(function.machine)),
    }?;
    if let (Some(selected), Some((_, origin))) = (selected, resolved) {
        super::selected_boundary_calls::retain_origin(
            &mut site.operations[first_output..],
            selected,
            origin,
        )?;
    }
    Ok(())
}
