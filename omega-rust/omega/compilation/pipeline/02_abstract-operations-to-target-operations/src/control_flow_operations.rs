/*
  Per-operation dispatch and scalar/array/float lowering inside the common
  target control-flow graph (`operations`, `scalar_sources`, `scalar_arrays`,
  `accessor_scalar_access`, `ieee_float_arithmetic`).

  Dispatches each `AbstractOperation` in a basic block to its scalar,
  structural, call, or memory lowering owner while updating block-local
  `LiveDefinitions`.
*/

use super::*;
pub(crate) mod accessor_scalar_access {
    use super::RowSite;
    use crate::LoweringError;
    use crate::unit::known_integers::insert_scalar_home;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
    use semantic_vocabulary::BoundaryMachineId;
    use std::collections::BTreeMap;
    use target_operations::{
        BoundaryRealization, BoundarySettlementBinding, BoundarySettlementRealization,
        TargetUnitOperation, TargetUnitScalarHomeRequirement,
    };
    use terminal_psi::TerminalAccessorKind;

    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        macro_rules! invalid {
            () => {
                LoweringError::unsupported_control_flow(function.machine)
            };
        }
        let AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            arguments,
            structural_arguments,
            result,
            completion_claim_sources: _,
            completion_receipts,
        } = operation
        else {
            return Err(invalid!());
        };
        let binding = settlements.get(boundary).ok_or_else(|| invalid!())?;
        let BoundarySettlementRealization::Builtin(BoundaryRealization::AccessorScalarAccess(
            realization,
        )) = binding.realization
        else {
            return Err(invalid!());
        };
        let demand = realization.demand;
        let [argument] = structural_arguments.as_slice() else {
            return Err(invalid!());
        };
        if argument.access != demand.access
            || super::references::reference_source_type(function, live, types, argument)
                != Some(demand.accessor)
            || !completion_receipts.is_empty()
        {
            return Err(invalid!());
        }
        let location = argument.clone();
        super::references::normalized_source(function, live, types, &location)?;
        let operand = match (demand.operation, arguments.as_slice()) {
            (TerminalAccessorKind::Read, []) => None,
            (TerminalAccessorKind::Write, [value]) => {
                let source = super::scalar_sources::source(*value, function, live)?;
                if source.scalar_type() != demand.scalar {
                    return Err(invalid!());
                }
                Some(source)
            }
            _ => return Err(invalid!()),
        };
        let result_home = match (demand.operation, result) {
            (TerminalAccessorKind::Read, AbstractBoundaryResult::Scalar(result))
                if result.scalar_type == demand.scalar =>
            {
                let home = TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: result.value,
                    scalar_type: demand.scalar,
                    shape: crate::scalar_abi::fixed_native_scalar_shape(demand.scalar)
                        .ok_or_else(|| invalid!())?,
                };
                insert_scalar_home(&mut live.integers, &mut live.scalar_homes, home)?;
                Some(home)
            }
            (TerminalAccessorKind::Write, AbstractBoundaryResult::Unit) => None,
            _ => return Err(invalid!()),
        };
        operations.push(TargetUnitOperation::AccessorScalarAccess {
            psi_operation: *psi_operation,
            boundary: *boundary,
            realization,
            location,
            operand,
            result_home,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}

pub(crate) mod ieee_float_arithmetic {
    use super::RowSite;
    use crate::LoweringError;
    use crate::unit::known_integers::insert_scalar_home;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
    use semantic_vocabulary::BoundaryMachineId;
    use std::collections::BTreeMap;
    use target::NativeTarget;
    use target_operations::{
        BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
        BoundarySettlementRealization, CompilerBuiltinExecution, TargetUnitOperation,
        TargetUnitScalarHomeRequirement,
    };

    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        target: NativeTarget,
        boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let AbstractOperation::BoundaryCall {
            psi_operation,
            result,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        } = operation
        else {
            unreachable!("inline float arithmetic routes only boundary calls")
        };
        let mismatch = LoweringError::BoundaryRealizationMismatch(*boundary);
        let binding = settlements
            .get(boundary)
            .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
        let declaration = boundary_machines
            .get(boundary)
            .copied()
            .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
        let BoundarySettlementRealization::Builtin(
            BoundaryRealization::NearestIeeeFloatArithmetic(realization),
        ) = binding.realization
        else {
            return Err(mismatch);
        };
        let execution = BoundaryExecutionBinding::CompilerBuiltin(
            CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                operation: realization.operation,
                format: realization.format,
            },
        );
        if realization.operation == target_operations::IeeeFloatArithmeticOperation::Classify {
            let AbstractBoundaryResult::Structural(result) = result else {
                return Err(mismatch);
            };
            let operand = semantic_vocabulary::ScalarType::IeeeFloat(realization.format);
            if binding.execution != execution
                || !realization.supports_target(target)
                || declaration.scalar_parameters != [operand]
                || !declaration.structural_parameters.is_empty()
                || !matches!(&declaration.result, terminal_psi::BoundaryMachineResult::Structural(declared) if declared.structural_type == result.structural_type && declared.multiplicity == result.multiplicity && declared.qualifications == result.qualifications && declared.projected_qualifications == result.projected_qualifications)
                || arguments.len() != 1
                || !structural_arguments.is_empty()
                || !completion_claim_sources.is_empty()
                || !completion_receipts.is_empty()
            {
                return Err(mismatch);
            }
            let argument = super::scalar_sources::source(arguments[0], function, live)?;
            if argument.scalar_type() != operand {
                return Err(mismatch);
            }
            if !result.is_unrestricted_plain() || !result.qualification_establishments.is_empty() {
                return Err(mismatch);
            }
            let layout = types
                .get(result.structural_type)
                .and_then(|declaration| {
                    target_operations::conventional_float_class_layout(declaration)
                })
                .ok_or(LoweringError::BoundaryRealizationMismatch(*boundary))?;
            let layout = target_operations::TargetStructuralHomeLayout::Sum(layout);
            let home = target_operations::TargetStructuralHomeRequirement {
                origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation: *psi_operation,
                    result: result.clone(),
                },
                layout,
            };
            if live
                .structural_homes
                .insert(result.place, home.clone())
                .is_some()
            {
                return Err(mismatch);
            }
            provenance.operations.push(*psi_operation);
            operations.push(TargetUnitOperation::IeeeFloatClassify {
                psi_operation: *psi_operation,
                boundary: *boundary,
                execution,
                realization,
                result_home: home,
                argument,
            });
            return Ok(());
        }
        let scalar = realization.operation.result_type(realization.format);
        let operand = realization.operation.operand_type(realization.format);
        let AbstractBoundaryResult::Scalar(result) = result else {
            return Err(mismatch);
        };
        if binding.execution != execution
            || !realization.supports_target(target)
            || !realization.signature_matches(&declaration.scalar_parameters, scalar)
            || !declaration.structural_parameters.is_empty()
            || declaration.result != terminal_psi::BoundaryMachineResult::Scalar(scalar.into())
            || result.scalar_type != scalar
            || arguments.len() != realization.operation.operand_count()
            || !structural_arguments.is_empty()
            || !completion_claim_sources.is_empty()
            || !completion_receipts.is_empty()
        {
            return Err(mismatch);
        }
        let sources = arguments
            .iter()
            .map(|argument| super::scalar_sources::source(*argument, function, live))
            .collect::<Result<Vec<_>, _>>()?;
        if sources.iter().any(|source| source.scalar_type() != operand) {
            return Err(LoweringError::ValueTypeMismatch(result.value));
        }
        let shape = crate::scalar_abi::fixed_native_scalar_shape(scalar).ok_or(mismatch)?;
        let result_home = TargetUnitScalarHomeRequirement {
            defining_operation: *psi_operation,
            source_value: result.value,
            scalar_type: scalar,
            shape,
        };
        insert_scalar_home(&mut live.integers, &mut live.scalar_homes, result_home)?;
        provenance.operations.push(*psi_operation);
        operations.push(TargetUnitOperation::NearestIeeeFloatArithmetic {
            psi_operation: *psi_operation,
            boundary: *boundary,
            execution,
            realization,
            result_home,
            arguments: sources,
        });
        Ok(())
    }
}

pub(crate) mod operations {
    use super::RowSite;
    use super::observations;
    use super::selected_boundary_calls::retain_origin;
    use crate::LoweringError;
    use crate::unit::known_integers::insert_scalar_home;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use calling_conventions::ValueShape;
    use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, PlaceId, ScalarType};
    use semantic_vocabulary::{IntegerSign, IntegerType};
    use std::collections::BTreeMap;
    use target::NativeTarget;
    use target_operations::{
        BoundaryRealization, BoundarySettlementBinding, BoundarySettlementRealization,
        ScalarFunctionAbi, TargetStructuralParameter, TargetUnitOperation,
        TargetUnitScalarHomeRequirement,
    };
    use terminal_psi::StructuralTypeShape;
    use terminal_semantics::ScalarRepresentationViewPair;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_operation(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        target: NativeTarget,
        functions: &BTreeMap<MachineId, &AbstractFunction>,
        dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
        callables: &terminal_psi::TerminalCallableCatalog,
        boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
        selected_calls: &crate::selected_boundary_calls::SelectedCallsByCall,
        scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
        native_callbacks: &BTreeMap<
            OperationId,
            Vec<target_operations::TargetNativeCallbackArgument>,
        >,
        installed_interrupts: Option<&provider_admission::AdmittedInstalledInterruptCohort>,
        prepared: &crate::function_signature::PreparedFunctionSignature,
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
                        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| {
                            LoweringError::unsupported_control_flow(function.machine)
                        })?,
                    )
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let result_home = site.establish_home(*psi_operation, result)?;
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
                let result_home = site.establish_home(*psi_operation, result)?;
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
                let result_home = site.establish_home(*psi_operation, result)?;
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
            AbstractOperation::ActivateTaskRegister {
                psi_operation,
                service,
                result,
                entries,
                gdt,
                task_state,
                selector,
                scratch,
            } => {
                let selector = super::scalar_sources::source(*selector, function, site.live)?;
                let result_home =
                    super::primitive_storage::retain_result(*psi_operation, *result, site.live)?;
                site.operations
                    .push(TargetUnitOperation::ActivateTaskRegister {
                        psi_operation: *psi_operation,
                        service: *service,
                        entries: *entries,
                        gdt: gdt.clone(),
                        task_state: task_state.clone(),
                        selector,
                        scratch: scratch.clone(),
                        result_home,
                    });
                site.provenance.operations.push(*psi_operation);
                Ok(())
            }
            AbstractOperation::ReadTaskRegister {
                psi_operation,
                service,
                result,
            } => {
                let result_home =
                    super::primitive_storage::retain_result(*psi_operation, *result, site.live)?;
                site.operations.push(TargetUnitOperation::ReadTaskRegister {
                    psi_operation: *psi_operation,
                    service: *service,
                    result_home,
                });
                site.provenance.operations.push(*psi_operation);
                Ok(())
            }
            AbstractOperation::ReloadSegmentDescriptors {
                psi_operation,
                service,
                result,
                entries,
                gdt,
                kind,
                selector,
                scratch,
            } => {
                let selector = super::scalar_sources::source(*selector, function, site.live)?;
                let result_home =
                    super::primitive_storage::retain_result(*psi_operation, *result, site.live)?;
                site.operations
                    .push(TargetUnitOperation::ReloadSegmentDescriptors {
                        psi_operation: *psi_operation,
                        service: *service,
                        entries: *entries,
                        gdt: gdt.clone(),
                        kind: *kind,
                        selector,
                        scratch: scratch.clone(),
                        result_home,
                    });
                site.provenance.operations.push(*psi_operation);
                Ok(())
            }
            AbstractOperation::ReadSegmentSelector {
                register,
                psi_operation,
                service,
                result,
            } => {
                let result_home =
                    super::primitive_storage::retain_result(*psi_operation, *result, site.live)?;
                site.operations
                    .push(TargetUnitOperation::ReadSegmentSelector {
                        register: *register,
                        psi_operation: *psi_operation,
                        service: *service,
                        result_home,
                    });
                site.provenance.operations.push(*psi_operation);
                Ok(())
            }
            AbstractOperation::GlobalDescriptorTable {
                psi_operation,
                service,
                operation,
                source,
            } => {
                site.operations
                    .push(TargetUnitOperation::GlobalDescriptorTable {
                        psi_operation: *psi_operation,
                        service: *service,
                        operation: *operation,
                        source: source.clone(),
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
                    shape: crate::scalar_abi::fixed_native_scalar_shape(scalar_type)
                        .ok_or(LoweringError::ValueTypeMismatch(*result))?,
                };
                if std::sync::Arc::make_mut(&mut site.live.scalar_homes)
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
                std::sync::Arc::make_mut(&mut site.live.scalar_homes).insert(*result, result_home);
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
                    || ScalarRepresentationViewPair::classify(*source_type, *target_type).is_none()
                {
                    return Err(LoweringError::ValueTypeMismatch(*result));
                }
                let result_home = TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    scalar_type: *target_type,
                    shape: crate::scalar_abi::fixed_native_scalar_shape(*target_type)
                        .ok_or(LoweringError::ValueTypeMismatch(*result))?,
                };
                insert_scalar_home(
                    &mut site.live.integers,
                    &mut site.live.scalar_homes,
                    result_home,
                )?;
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
            AbstractOperation::EstablishAccessReference { .. } => {
                super::references::establish_access(operation, site)
            }
            AbstractOperation::EstablishReference { .. } => {
                super::references::establish(operation, site)
            }
            AbstractOperation::ReleaseReference { .. } => {
                super::references::release(operation, site)
            }
            AbstractOperation::SelectMachine {
                psi_operation,
                result,
                descriptor,
            } => {
                let selected = callables
                    .descriptor(*descriptor)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if result.scalar_type
                    != semantic_vocabulary::ScalarType::Callable(selected.contract)
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let result_home =
                    super::primitive_storage::retain_result(*psi_operation, *result, site.live)?;
                site.provenance.operations.push(*psi_operation);
                site.operations.push(TargetUnitOperation::SelectMachine {
                    result_home,
                    descriptor: *descriptor,
                });
                Ok(())
            }
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
            abstract_operations::call_or_call_structural_pattern!(..) => super::calls::lower_call(
                operation,
                site,
                target,
                functions,
                dynamic_dispatch,
                callables,
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
                ) || structural_types.get(structural_type.id).copied() != Some(structural_type)
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
                let (view, _, _) = crate::scalar::element_views::resolve_element_view(
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
                let home =
                    super::aggregate_results::home(*psi_operation, result, structural_types)?;
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
                crate::unit::lower_field_store(
                    operation,
                    function,
                    structural_types,
                    parameters_by_place,
                    &site.live.structural_homes,
                    reference_destination,
                    &super::scalar_sources::ScalarSources::from(&*site.live),
                    site.operations,
                    site.provenance,
                )
            }
            AbstractOperation::WriteOnlyPrimitiveStore { value, .. } => {
                crate::unit::write_only_primitive_store::lower_write_only_primitive_store(
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
                let Some(settlement) = settlements.get(boundary) else {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                };
                match &settlement.realization {
                    BoundarySettlementRealization::Builtin(
                        BoundaryRealization::EmptyRunRelease,
                    ) => {
                        let declaration = boundary_machines
                            .get(boundary)
                            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                        super::run_custody::lower(operation, site, declaration, settlement)
                    }
                    BoundarySettlementRealization::Builtin(
                        BoundaryRealization::StaticInterrupt {
                            operation:
                                language_core::static_interrupts::StaticInterruptOperation::Install,
                            ..
                        },
                    ) => {
                        let operation_id = operation.psi_operation().ok_or_else(|| {
                            LoweringError::unsupported_control_flow(function.machine)
                        })?;
                        let invalid = |reason| LoweringError::InvalidInstalledInterrupt {
                            machine: function.machine,
                            operation: operation_id,
                            reason,
                        };
                        let scope = super::interrupt_installation::scope(
                            operation_id,
                            installed_interrupts,
                        )
                        .map_err(|reason| {
                            LoweringError::InvalidInstalledInterrupt {
                                machine: function.machine,
                                operation: operation_id,
                                reason,
                            }
                        })?;
                        let mut calls = Vec::with_capacity(2);
                        for (role, machine, actual_position) in [
                            (
                                target_operations::InstallationCallRole::Validate,
                                scope.validator(),
                                0,
                            ),
                            (
                                target_operations::InstallationCallRole::Work,
                                scope.work(),
                                5,
                            ),
                        ] {
                            let callee = functions.get(&machine).copied().ok_or_else(|| {
                            invalid(
                                "installation helper body is absent from the executable closure",
                            )
                        })?;
                            let [formal] = callee.structural_parameters.as_slice() else {
                                return Err(invalid(
                                    "installation helper must have one structural formal",
                                ));
                            };
                            if !callee.parameters.is_empty()
                                || formal.access != terminal_psi::StructuralAccess::SharedBorrow
                            {
                                return Err(invalid(
                                    "installation helper must take one shared reference without scalar formals",
                                ));
                            }
                            let signature = super::call_signature::prepare(
                                site.function.machine,
                                abstract_operations::AbstractCallSignature::Machine(callee),
                                target,
                                site.types,
                            )?;
                            let [placement] = signature.parameters.as_slice() else {
                                return Err(invalid(
                                    "installation helper calling plan must contain one argument placement",
                                ));
                            };
                            let mut actual = scope.arguments()[actual_position].clone();
                            actual.access = terminal_psi::StructuralAccess::SharedBorrow;
                            let argument = super::calls::structural_argument(
                                &actual,
                                &abstract_operations::StructuralCallFormal::from(formal),
                                placement,
                                machine,
                                site.function,
                                site.types,
                                prepared,
                                parameters_by_place,
                                site.live,
                                site.operations,
                            )?;
                            calls.push(target_operations::TargetInstallationCall {
                                role,
                                callee: machine,
                                call_plan: signature.call_plan.into_call_plan(),
                                argument,
                            });
                        }
                        let calls = calls.try_into().map_err(|_| {
                            invalid("installation requires ordered Validate and Work calls")
                        })?;
                        super::interrupt_installation::lower(
                            operation,
                            site,
                            scope,
                            calls,
                            boundary_machines.get(boundary).copied().ok_or_else(|| {
                                invalid("installation boundary declaration is absent")
                            })?,
                            settlements.get(boundary).ok_or_else(|| {
                                invalid("installation boundary settlement is absent")
                            })?,
                        )
                    }
                    BoundarySettlementRealization::Builtin(
                        BoundaryRealization::StaticInterrupt { .. },
                    ) => super::static_interrupts::lower(
                        operation,
                        site,
                        boundary_machines.get(boundary).copied().ok_or_else(|| {
                            LoweringError::unsupported_control_flow(function.machine)
                        })?,
                        settlements.get(boundary).ok_or_else(|| {
                            LoweringError::unsupported_control_flow(function.machine)
                        })?,
                    ),
                    BoundarySettlementRealization::Builtin(
                        BoundaryRealization::AccessLifecycle { .. },
                    ) => super::access_lifecycles::lower(
                        operation,
                        site,
                        boundary_machines,
                        settlements,
                    ),
                    BoundarySettlementRealization::Builtin(
                        BoundaryRealization::AccessorScalarAccess(_),
                    ) => super::accessor_scalar_access::lower(operation, site, settlements),
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
                        crate::unit::boundary_call::lower_boundary_call(
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
                        {
                            if site
                                .live
                                .structural_homes
                                .insert(home.place(), home.clone())
                                .is_some()
                            {
                                return Err(LoweringError::unsupported_control_flow(
                                    function.machine,
                                ));
                            }
                            let (_, result) = home.operation_result().ok_or_else(|| {
                                LoweringError::unsupported_control_flow(function.machine)
                            })?;
                            super::references::boundary_result_references(
                                function,
                                structural_types,
                                result,
                                site.live,
                            )?;
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
            } => crate::unit::scalar_definitions::lower_integer_constant(
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
            } => crate::unit::scalar_definitions::lower_boolean_constant(
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
            } => crate::unit::scalar_definitions::lower_ieee_float_constant(
                *psi_operation,
                *result,
                *value,
                false,
                &mut site.live.ieee_float_constants,
                site.operations,
                site.provenance,
            ),
            AbstractOperation::IntegerWiden { .. } => {
                crate::unit::scalar_definitions::lower_integer_widen(
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
            retain_origin(&mut site.operations[first_output..], selected, origin)?;
        }
        Ok(())
    }
}

pub(crate) mod scalar_arrays {
    use super::RowSite;
    use super::aggregate_results::established_home;
    use crate::LoweringError;
    use abstract_operations::AbstractOperation;
    use calling_conventions::ValueShape;
    use calling_conventions::terminal_layout::StructuralLayouts;
    use semantic_vocabulary::{ScalarType, StructuralTypeId};
    use target_operations::TargetUnitOperation;
    use terminal_psi::StructuralTypeShape;

    pub(crate) fn shape(
        structural_type: StructuralTypeId,
        types: &StructuralLayouts<'_>,
    ) -> Result<(ScalarType, u64, ValueShape), LoweringError> {
        let invalid = || LoweringError::UnsupportedStructuralArray(structural_type);
        let mut current = structural_type;
        let mut count = Some(1_u64);
        let mut empty = false;
        let mut array = false;
        for _ in 0..types.len() {
            match types.get(current).ok_or_else(invalid)?.shape {
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    array = true;
                    empty |= length == 0;
                    count = count.and_then(|count| count.checked_mul(length));
                    current = element;
                }
                StructuralTypeShape::PrimitiveScalar(scalar_type) if array => {
                    let leaf =
                        super::primitive_storage::native_shape(scalar_type).ok_or_else(invalid)?;
                    let count = if empty { 0 } else { count.ok_or_else(invalid)? };
                    let size = count
                        .checked_mul(u64::from(leaf.byte_size))
                        .and_then(|size| u32::try_from(size).ok())
                        .ok_or_else(invalid)?;
                    return Ok((
                        scalar_type,
                        count,
                        ValueShape::integer(size, if size == 0 { 1 } else { leaf.alignment }),
                    ));
                }
                _ => return Err(invalid()),
            }
        }
        Err(invalid())
    }

    pub(crate) fn establish(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let AbstractOperation::EstablishScalarArray {
            psi_operation,
            result,
            elements,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let (scalar_type, count, _) = shape(result.structural_type, types)?;
        if u64::try_from(elements.len()).ok() != Some(count)
            || elements.iter().any(|value| {
                !super::scalar_sources::source(*value, function, live)
                    .is_ok_and(|source| source.scalar_type() == scalar_type)
            })
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let result_home = established_home(live, *psi_operation, result, types, function.machine)?;
        operations.push(TargetUnitOperation::EstablishScalarArray {
            psi_operation: *psi_operation,
            result_home,
            elements: elements.clone(),
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}

pub(crate) mod scalar_sources {
    use super::LiveDefinitions;
    use crate::LoweringError;
    use crate::unit::known_integers::KnownUnitInteger;
    use abstract_operations::AbstractFunction;
    use semantic_vocabulary::{OperationId, ValueId};
    use std::collections::BTreeMap;
    use target_operations::{
        TargetStructuralRuntimeIndex, TargetUnitScalarArgumentSource,
        TargetUnitScalarHomeRequirement,
    };

    #[derive(Clone, Copy)]
    pub(crate) struct ScalarSources<'a> {
        pub integers: &'a BTreeMap<ValueId, KnownUnitInteger>,
        pub scalar_homes: &'a BTreeMap<ValueId, TargetUnitScalarHomeRequirement>,
        pub booleans: &'a BTreeMap<ValueId, (OperationId, bool)>,
        pub ieee_float_constants:
            &'a BTreeMap<ValueId, (OperationId, semantic_vocabulary::IeeeFloatValue)>,
        pub scalar_block_parameters:
            &'a BTreeMap<ValueId, target_operations::TargetScalarBlockValue>,
    }

    impl<'a> From<&'a LiveDefinitions> for ScalarSources<'a> {
        fn from(live: &'a LiveDefinitions) -> Self {
            Self {
                integers: &live.integers,
                scalar_homes: &live.scalar_homes,
                booleans: &live.booleans,
                ieee_float_constants: &live.ieee_float_constants,
                scalar_block_parameters: &live.scalar_block_parameters,
            }
        }
    }

    pub(crate) fn source(
        value: ValueId,
        function: &AbstractFunction,
        live: &LiveDefinitions,
    ) -> Result<TargetUnitScalarArgumentSource, LoweringError> {
        resolved_source(value, function, &ScalarSources::from(live))
    }

    pub(crate) fn resolved_source(
        value: ValueId,
        function: &AbstractFunction,
        sources: &ScalarSources<'_>,
    ) -> Result<TargetUnitScalarArgumentSource, LoweringError> {
        if let Some(known) = sources.integers.get(&value) {
            return Ok(known.into_target_source(value));
        }
        if let Some(home) = sources.scalar_homes.get(&value) {
            return Ok(TargetUnitScalarArgumentSource::Home(*home));
        }
        if let Some((defining_operation, immediate)) = sources.booleans.get(&value) {
            return Ok(TargetUnitScalarArgumentSource::BooleanImmediate {
                defining_operation: *defining_operation,
                source_value: value,
                value: *immediate,
            });
        }
        if let Some((defining_operation, immediate)) = sources.ieee_float_constants.get(&value) {
            return Ok(TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                defining_operation: *defining_operation,
                source_value: value,
                value: *immediate,
            });
        }
        if let Some(parameter) = sources.scalar_block_parameters.get(&value) {
            return Ok(TargetUnitScalarArgumentSource::BlockParameter(*parameter));
        }
        let (position, parameter) = function
            .parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.value == value)
            .ok_or(LoweringError::UnknownValue(value))?;
        Ok(TargetUnitScalarArgumentSource::Parameter {
            parameter_index: u32::try_from(position)
                .map_err(|_| LoweringError::UnknownValue(value))?,
            source_value: value,
            scalar_type: parameter.scalar_type,
        })
    }

    pub(crate) fn runtime_indices(
        runs: Vec<(ValueId, u32)>,
        function: &AbstractFunction,
        sources: &ScalarSources<'_>,
    ) -> Result<Vec<TargetStructuralRuntimeIndex>, LoweringError> {
        runs.into_iter()
        .map(|(index, stride)| {
            let operand = resolved_source(index, function, sources)?;
            if !matches!(operand.scalar_type(), semantic_vocabulary::ScalarType::Integer(integer)
                if integer.bits() <= 64)
            {
                return Err(LoweringError::ValueTypeMismatch(index));
            }
            Ok(TargetStructuralRuntimeIndex { operand, stride })
        })
        .collect()
    }
}
