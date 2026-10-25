//! Family dispatch selects an exact member before choosing its execution route.
//! Psi admitted the head/member refinement; catalog selection supplies only its
//! deployment association. Checked members enter an ordinary call frame and
//! transfer their actual result claims and backing, never boundary issuance.
//! Opaque members retain the head's ordinary provider/import route and contract;
//! catalog selection alone grants neither a provider nor a host adapter.

use crate::capabilities::TerminalUnsupportedCapability;
use crate::custody::{bind_active_structural_arguments, validate_boundary_requirements};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, ResultClaimOrigin, TerminalExecution};
use crate::program::AdmittedTerminalProgram;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_installation::AdmittedTargetMachineSelections;
use terminal_psi::{
    BoundaryMachineDeclaration, ClaimTransfer, Operation, OperationKind, OperationResult,
    TargetMachineCallable, TargetMachineSupply,
};

/// Resolve the exact deployment member without granting its execution supply.
/// Checked bodies use ordinary call custody; opaque declarations still demand
/// a separately admitted provider or a receiving host's exact import adapter.
pub(crate) fn selected_member(
    program: &AdmittedTerminalProgram<'_>,
    selections: Option<&AdmittedTargetMachineSelections>,
    boundary: BoundaryMachineId,
) -> Result<TargetMachineCallable, TerminalInterpretError> {
    let head = TargetMachineCallable::Boundary(boundary);
    let selections =
        selections.ok_or(TerminalInterpretError::TargetMachineSelectionMissing(head))?;
    if selections.terminal_psi() != program.identity {
        return Err(TerminalInterpretError::TargetMachineSelectionIdentityMismatch);
    }
    let selected = selections
        .member(head)
        .map_err(TerminalInterpretError::TargetMachineSelection)?;
    match (selected.callable(), selected.supply()) {
        (
            TargetMachineCallable::Machine(member),
            TargetMachineSupply::CheckedBody | TargetMachineSupply::Boundary,
        ) => {
            // A bodied Boundary is an ordinary checked adapter. Its retained
            // executable identity supplies custody; an opaque Boundary never
            // enters this arm merely by naming the same supply mode.
            program.verified_machine(member)?;
            Ok(selected.callable())
        }
        (TargetMachineCallable::Boundary(member), supply)
            if supply != TargetMachineSupply::CheckedBody =>
        {
            program.verified_boundary_machine(&member)?;
            Ok(selected.callable())
        }
        (callable, supply) => Err(TerminalInterpretError::UnsupportedCapability(
            TerminalUnsupportedCapability::TargetMachineMemberSupply {
                head,
                callable,
                supply,
            },
        )),
    }
}

/// Resolve a checked body for a domain-ending invocation. The body runs before
/// any host completion; selecting it grants no authority to simulate its leaf.
pub(crate) fn completion_callee(
    program: &AdmittedTerminalProgram<'_>,
    selections: Option<&AdmittedTargetMachineSelections>,
    supply: Option<crate::effects::AdmittedBoundarySupply>,
    boundary: BoundaryMachineId,
) -> Result<Option<MachineId>, TerminalInterpretError> {
    if program.is_target_family_head(boundary)
        && let TargetMachineCallable::Machine(member) =
            selected_member(program, selections, boundary)?
    {
        return Ok(Some(member));
    }
    match supply {
        Some(crate::effects::AdmittedBoundarySupply::CheckedProvider(callee)) => Ok(Some(callee)),
        None if program.has_provider(&boundary) => Err(
            TerminalInterpretError::ProviderInstallationMissing(boundary),
        ),
        _ => Ok(None),
    }
}

/// The same result capability decision runs during called-scope preflight and
/// before frame entry. A selected body cannot issue extra head domains.
pub(crate) fn validate_result_transport(
    operation: &Operation,
    member: &terminal_psi::TerminalMachine,
) -> Result<(), TerminalInterpretError> {
    let OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
        return Err(malformed());
    };
    let origin = ResultClaimOrigin::TargetFamilyReturn {
        head: boundary,
        member: member.id,
    };
    let supported = match &operation.result {
        OperationResult::Unit => member.result == terminal_psi::TerminalMachineResult::Unit,
        OperationResult::Scalar(result) => member
            .result
            .scalar()
            .is_some_and(|declared| declared.scalar_type == result.scalar_type),
        OperationResult::Structural(result) => member.result.structural().is_some_and(|declared| {
            result.structural_type == declared.structural_type
                && result.multiplicity == declared.multiplicity
                && origin.result_qualifications_match(result, &declared.qualifications)
                && origin.result_projected_qualifications_match(
                    result,
                    &declared.projected_qualifications,
                    &[],
                )
                && result
                    .qualification_establishments
                    .iter()
                    .all(|row| declared.qualifications.contains(&row.domain))
        }),
    };
    if supported { Ok(()) } else { Err(malformed()) }
}

impl TerminalExecution<'_> {
    pub(crate) fn execute_target_family_call(
        &mut self,
        operation: &Operation,
        head: &BoundaryMachineDeclaration,
        member_id: MachineId,
        scalar_arguments: &[TerminalScalarValue],
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        let program = std::sync::Arc::clone(&self.program);
        let member = program.verified_machine(member_id)?;
        validate_result_transport(operation, member)?;
        let prepared = self.prepare_structural_call_arguments(member_id, structural_arguments)?;
        if head.structural_parameters.len() != member.structural_parameters.len() {
            return Err(malformed());
        }
        let mut opaque_parameters = Vec::new();
        let mut opaque_values = Vec::new();
        for (head_parameter, member_parameter) in head
            .structural_parameters
            .iter()
            .zip(&member.structural_parameters)
        {
            if let Some(value) = prepared.values.get(&member_parameter.place) {
                opaque_parameters.push(head_parameter.clone());
                opaque_values.push(value.clone());
            } else if !prepared.scalar_arrays.contains_key(&member_parameter.place)
                || head_parameter.structural_type != member_parameter.structural_type
            {
                return Err(malformed());
            }
        }
        bind_active_structural_arguments(
            &opaque_parameters,
            &opaque_values,
            &self.program.structural_types(),
            &self.structural_cases,
            self.account.as_ref(),
        )?;
        validate_boundary_requirements(head, |argument_index| {
            member
                .structural_parameters
                .get(argument_index)
                .and_then(|parameter| prepared.values.get(&parameter.place))
        })?;
        // The checked receipt retains its current argument-relative path.
        // The ordinary frame binder authenticates that exact path against held
        // custody and active Case guards; neither entry-history reconstruction
        // nor an empty-path fallback may supply member-entry custody.
        let transfers = completion_receipts
            .iter()
            .map(|receipt| ClaimTransfer {
                claim: receipt.claim,
                argument_index: receipt.argument_index,
                path: receipt.path.clone(),
            })
            .collect::<Vec<_>>();
        match &operation.result {
            OperationResult::Unit => self.begin_unit_call(
                member_id,
                scalar_arguments,
                structural_arguments,
                prepared,
                &transfers,
            )?,
            OperationResult::Scalar(result) => self.begin_structural_scalar_call(
                member_id,
                *result,
                scalar_arguments,
                structural_arguments,
                prepared,
                &transfers,
            )?,
            OperationResult::Structural(result) => self.begin_structural_result_call(
                member_id,
                result.clone(),
                scalar_arguments,
                structural_arguments,
                prepared,
                &transfers,
                ResultClaimOrigin::TargetFamilyReturn {
                    head: head.id,
                    member: member_id,
                },
            )?,
        }
        Ok(OperationFlow::Redispatch)
    }
}
