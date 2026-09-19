//! A Unit call: its structural arguments, claim transfers, requirement
//! obligations, contract places and crash continuations validate against
//! the callee.

use crate::validation::structural_operations::claim_transfers::{
    validate_service_reach, validate_unit_call_claim_transfers,
};
use crate::validation::structural_operations::contract_places::{
    unit_call_contract_propositions, validate_unit_call_contract_places,
};
use crate::validation::structural_operations::crash_continuations::validate_unit_call_crash_continuations;
use crate::validation::structural_operations::structural_arguments::{
    StructuralArgumentSourcePolicy, is_admitted_unit_call_argument_path,
    is_literal_indexed_field_path, is_structural_call_result, is_unrestricted_mutable_subloan,
    is_unrestricted_shared_subloan, is_unrestricted_write_only_subloan,
    validate_structural_arguments,
};
use crate::validation::{
    BTreeMap, MachineId, ModuleError, OperationKind, StructuralAccess, StructuralMultiplicity,
    StructuralPathSegment, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    is_partial_affine_path, partial_affine_root_type, propositions,
};

pub(super) fn validate_call_unit(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::CallUnit {
        callee,
        arguments,
        erased_arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_call_unit")
    };
    let callee = machines
        .get(callee)
        .copied()
        .ok_or(ModuleError::UnknownCallTarget {
            operation: operation.id,
            callee: *callee,
        })?;
    if callee.result != TerminalMachineResult::Unit {
        return Err(ModuleError::UnitCallTargetHasScalarSignature {
            operation: operation.id,
            callee: callee.id,
        });
    }
    if structural_arguments
        .iter()
        .any(|argument| !is_admitted_unit_call_argument_path(module, machine, argument))
    {
        return Err(ModuleError::InvalidStructuralArgumentPath {
            operation: operation.id,
            argument_index: structural_arguments
                .iter()
                .position(|argument| {
                    !is_admitted_unit_call_argument_path(module, machine, argument)
                })
                .unwrap_or_default() as u32,
        });
    }
    let projected = structural_arguments.iter().any(|argument| {
        !argument.path.is_empty()
            && !crate::validation::references::is_reference_projection(module, machine, argument)
    });
    // Independent borrowed projections retain each root's authority;
    // sibling argument counts do not create residual owned custody.
    let ordinary_borrowed_projections = structural_arguments.len()
        == callee.structural_parameters.len()
        && structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .all(|(argument, parameter)| {
                argument.path.is_empty()
                    || crate::validation::references::is_reference_projection(
                        module, machine, argument,
                    )
                    || is_unrestricted_write_only_subloan(module, machine, parameter, argument)
                    || is_unrestricted_shared_subloan(machine, parameter, argument)
                    || is_unrestricted_mutable_subloan(module, machine, parameter, argument)
            });
    let result_projection = structural_arguments.iter().any(|argument| {
        argument.access == StructuralAccess::Owned
            && !argument.path.is_empty()
            && is_structural_call_result(machine, argument.place)
            && partial_affine_root_type(machine, argument.place).is_some()
    });
    // Scalar inputs are independent of this result's residual custody.
    // Only the Jump route validates their transport across cleanup;
    // retain the separate final-return and parameter-root limits.
    let scalar_result_continuation = !machine.parameters.is_empty()
        && result_projection
        && machine.blocks.iter().any(|block| {
            matches!(block.terminator, Terminator::Jump { .. })
                && block
                    .operations
                    .iter()
                    .any(|candidate| candidate.id == operation.id)
        });
    // One consumer may die for several result temporaries at once: every
    // projected operand is then an owned move out of a live partial-affine
    // call result, and the operand roster lines up with the callee's
    // parameter roster so the shared continuation edge can carry each root's
    // residual complement in operand order. Whole-path operands keep their
    // ordinary transfer checks; borrowed or parameter-rooted projections
    // retain the single-argument bound.
    let shared_result_residuals = result_projection
        && structural_arguments.len() == callee.structural_parameters.len()
        && structural_arguments.iter().all(|argument| {
            argument.path.is_empty()
                || (argument.access == StructuralAccess::Owned
                    && is_structural_call_result(machine, argument.place)
                    && partial_affine_root_type(machine, argument.place).is_some())
        });
    if projected
        && !ordinary_borrowed_projections
        && ((machine.result != TerminalMachineResult::Unit)
            || (!machine.parameters.is_empty() && !scalar_result_continuation)
            || (!result_projection && machine.structural_parameters.len() != 1)
            || (!shared_result_residuals
                && (structural_arguments.len() != 1 || callee.structural_parameters.len() != 1)))
    {
        return Err(ModuleError::ProjectedUnitCallOutsideBoundedSlice {
            operation: operation.id,
        });
    }
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        operation.id,
        true,
        StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults,
    )?;
    if let Some(argument_index) = structural_arguments
        .iter()
        .zip(&callee.structural_parameters)
        .position(|(argument, expected)| {
            (is_literal_indexed_field_path(&argument.path)
                || (argument
                    .path
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
                    && argument.access == StructuralAccess::WriteOnlyBorrow))
                && !is_unrestricted_write_only_subloan(module, machine, expected, argument)
                && !is_unrestricted_shared_subloan(machine, expected, argument)
                && !is_unrestricted_mutable_subloan(module, machine, expected, argument)
                && !(argument.access == StructuralAccess::Owned
                    && expected.multiplicity == StructuralMultiplicity::Affine
                    && partial_affine_root_type(machine, argument.place).is_some_and(|root_type| {
                        is_partial_affine_path(module, root_type, &argument.path)
                    }))
        })
    {
        return Err(ModuleError::InvalidStructuralArgumentPath {
            operation: operation.id,
            argument_index: argument_index as u32,
        });
    }
    if let Some((argument_index, _)) = structural_arguments
        .iter()
        .zip(&callee.structural_parameters)
        .enumerate()
        .find(|(_, (argument, expected))| {
            !argument.path.is_empty()
                && (!expected.qualifications.is_empty()
                    || machine
                        .structural_parameters
                        .iter()
                        .find(|actual| actual.place == argument.place)
                        .is_some_and(|actual| !actual.qualifications.is_empty()))
        })
    {
        return Err(ModuleError::InvalidStructuralArgumentPath {
            operation: operation.id,
            argument_index: argument_index as u32,
        });
    }
    validate_unit_call_contract_places(callee, operation.id)?;
    if projected {
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
        {
            if argument.path.is_empty()
                || crate::validation::references::is_reference_projection(module, machine, argument)
            {
                continue;
            }
            let projected_parameter = parameter.place;
            if unit_call_contract_propositions(callee).any(|proposition| {
                propositions::proposition_content_roots(proposition).contains(&projected_parameter)
            }) {
                return Err(
                    ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                        operation: operation.id,
                        callee: callee.id,
                        place: projected_parameter,
                    },
                );
            }
        }
    }
    validate_service_reach(
        operation.id,
        &machine.published_service_ceiling,
        &callee.published_service_ceiling,
    )?;
    if requirement_obligations.len() != callee.contract.requires.len() {
        return Err(ModuleError::CallRequirementArityMismatch {
            operation: operation.id,
            expected: callee.contract.requires.len(),
            actual: requirement_obligations.len(),
        });
    }
    if erased_arguments.len() != callee.contract.erased_scalar_formals.len() {
        return Err(ModuleError::ErasedCallArgumentArityMismatch {
            operation: operation.id,
            expected: callee.contract.erased_scalar_formals.len(),
            actual: erased_arguments.len(),
        });
    }
    crate::validation::validate_erased_argument_terms(machine, operation.id, erased_arguments)?;

    validate_unit_call_claim_transfers(
        module,
        machine,
        callee,
        structural_arguments,
        claim_transfers,
        operation.id,
    )?;
    validate_unit_call_crash_continuations(
        module,
        machine,
        callee,
        arguments,
        structural_arguments,
        crash_continuations,
        operation.id,
    )?;
    Ok(())
}
