//! Lower ordinary calls retained by scalar evaluation using their checked custody.
//! Unit effects and structural values share argument admission and call emission.
use super::{
    CheckedTrees, LoweringError, QualifiedScalarType, storage, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_lowering::branch_states::prepared::LoweredEffectCall;

use crate::unit::attached_unit::bodies::{UnitBody, UnitPlans};
use checked_trees::CheckedUnitEffectOperationPlan;

pub(super) struct Prepared {
    pub(super) argument_types: Vec<QualifiedScalarType>,
    pub(super) call: LoweredEffectCall,
}

/// Borrow the actual published caller signature, independently of its result representation.
pub(super) struct Caller<'a> {
    pub(super) state: symbols::SymbolHandle,
    pub(super) erased_proof_parameters: &'a [checked_trees::CheckedErasedProofParameterPlan],
}

impl<'a> Caller<'a> {
    pub(super) fn published(
        checked: &'a CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
    ) -> Result<Self, LoweringError> {
        Ok(Self {
            state,
            erased_proof_parameters: UnitBody::find(
                UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
                machine,
            )?
            .state_erased_proof_parameters(state)?,
        })
    }
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    state: &Caller<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    bindings: &storage::ScalarBindings,
    value_types: &[QualifiedScalarType],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    result: Option<terminal_psi::StructuralOperationResult>,
    results: &[(
        u32,
        crate::emission::operation_emission::buffer::StructuralBinding,
    )],
) -> Result<Prepared, LoweringError> {
    let Some((call, erased_scalar_arguments, erased_proof_arguments)) = operation
        .call()
        .filter(|call| {
            call.is_module()
                && !matches!(
                    call.result(),
                    checked_trees::CheckedUnitCallResult::Scalar(_)
                )
        })
        .and_then(|call| {
            Some((
                call,
                call.erased_scalar_arguments()?,
                call.erased_proof_arguments()?,
            ))
        })
    else {
        return unsupported("scalar graph requires an ordinary effect call");
    };
    let coordinate = &call.coordinate;
    let target_machine = &call.target.machine;
    let target_state = &call.target.state;
    let target_contract_report_fingerprint = &call.target.contract_report_fingerprint;
    let service_reach = &call.target.service_reach;
    let scalar_arguments = &call.scalar_arguments;
    let structural_arguments = &call.structural_arguments;
    let claim_transfers = call.claim_transfers();
    if call.structural_custody().is_some_and(|custody| {
        !custody.reference_loans.is_empty() || !custody.result_qualifications.is_empty()
    }) {
        return unsupported(
            "scalar effect call requires its returned loan or qualification transport",
        );
    }
    if !erased_scalar_arguments.is_empty() {
        return unsupported("scalar graph Unit call has no erased argument lane");
    }
    UnitBody::find(
        UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
        *target_machine,
    )?
    .entry()?;
    let erased_proof_arguments = erased_proof_arguments
        .iter()
        .map(|term| {
            crate::scalar_lowering::contracts::checked_proof_term(
                checked,
                term,
                state.erased_proof_parameters,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let body = UnitBody::find(
        UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
        *target_machine,
    )?;
    let target = body.entry()?;
    match (body.result()?, call.result(), &result) {
        (
            checked_trees::CheckedControlResultPlan::Unit,
            checked_trees::CheckedUnitCallResult::Unit,
            None,
        ) => {}
        (
            checked_trees::CheckedControlResultPlan::Structural(_),
            checked_trees::CheckedUnitCallResult::Structural { .. },
            Some(_),
        ) => {}
        _ => return unsupported("effect call disagrees with its target result"),
    }
    if target.state != *target_state
        || target.contract_report_fingerprint != *target_contract_report_fingerprint
        || !crate::unit::attached_unit::catalog::checked_unit_target_reach_matches(
            *service_reach,
            target.contract_service_reach,
        )
        || (0..structural_arguments.len()).any(|position| {
            claim_transfers
                .iter()
                .filter(|claim| claim.argument_index as usize == position)
                .count()
                != target
                    .entry_claims
                    .iter()
                    .filter(|claim| claim.parameter_index as usize == position)
                    .count()
        })
        || claim_transfers
            .iter()
            .any(|claim| claim.argument_index as usize >= structural_arguments.len())
        || structural_arguments.len() != target.structural_parameters.len()
        || scalar_arguments.len() != target.scalar_parameters.len()
    {
        return unsupported("scalar Unit call disagrees with its exact target custody");
    }
    let (_, callee_state) = crate::declarations::state_declaration(checked, *target_state)?;
    if !checked
        .facts
        .service_reaches
        .rows
        .services(service_reach.direct)
        .is_empty()
        || !checked
            .facts
            .service_reaches
            .rows
            .services(service_reach.transitive)
            .is_empty()
    {
        return unsupported("scalar Unit calls with services require scalar service lowering");
    }
    let contract = checked
        .facts
        .contract_plans
        .for_machine(*target_machine)
        .ok_or(LoweringError::Unsupported(
            "scalar Unit target has no checked contract",
        ))?;
    // General structural contract substitution belongs to the existing Unit
    // assembler. This graph lane admits only the already plain primitive call
    // contract, and does not silently erase authored requirements or exits.
    // An arithmetic policy (`u64 in Wrapping`) governs the callee's own
    // operations and asks nothing of the caller's argument.
    if !checked.state_contracts(callee_state).is_empty()
        || !contract.closed_scalar_values.requires().is_empty()
        || !contract.closed_scalar_values.ensures().is_empty()
        || !contract.crash.published().is_empty()
        || contract
            .crash
            .structural_runtime_requirements()
            .is_some_and(|requirements| !requirements.is_empty())
        || checked
            .state_parameters(callee_state)
            .iter()
            .any(|parameter| {
                let reference = checked
                    .type_reference_table
                    .type_reference(parameter.type_reference);
                if checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                {
                    !matches!(
                        reference,
                        checked_trees::types::TypeReferenceNode::Named { .. }
                    ) && !validation::is_arithmetic_policy_only_integer(
                        &checked.typed,
                        parameter.type_reference,
                    )
                } else {
                    false
                }
            })
    {
        return unsupported("scalar Unit calls require an empty authored value and crash contract");
    }
    let mut lowered_arguments = Vec::new();
    for (argument, parameter) in structural_arguments
        .iter()
        .zip(target.structural_parameters)
    {
        if !parameter.qualifications.is_empty()
            || parameter.binding_carrier.is_some()
            || parameter.access != argument.access
            || parameter.type_identity != argument.type_identity
        {
            return unsupported("scalar Unit argument does not retain a plain primitive borrow");
        }
        if argument.access == checked_trees::CheckedStructuralAccess::Owned {
            let source = bindings.owned_call_argument(argument, results)?;
            lowered_arguments.push(source);
            continue;
        }
        let source_parameter = checked
            .state_parameters(callee_state)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "scalar Unit formal position is absent",
            ))?;
        let checked_trees::types::TypeReferenceNode::Reference { referee, .. } = checked
            .type_reference_table
            .type_reference(source_parameter.type_reference)
        else {
            return unsupported("scalar Unit structural argument is not a primitive borrow");
        };
        let primitive =
            checked
                .primitive_type_reference(*referee)
                .ok_or(LoweringError::Unsupported(
                    "scalar Unit borrow is not primitive",
                ))?;
        lowered_arguments.push(bindings.primitive_borrow(
            argument,
            terminal_scalar_type(primitive)?,
            structural_types,
        )?);
    }
    let argument_types = target
        .scalar_parameters
        .iter()
        .map(|parameter| terminal_scalar_type(parameter.primitive_type))
        .collect::<Result<Vec<_>, _>>()?;
    let result_claims = match (&result, call.structural_custody()) {
        (Some(result), Some(custody)) => {
            crate::emission::claim_bindings::prepare_call_result_claims(
                &custody.result_claims,
                result.structural_type,
                structural_types,
            )?
        }
        _ => Vec::new(),
    };
    let claim_transfers = crate::emission::claim_bindings::prepare_checked_claim_transfers(
        claim_transfers,
        target.structural_parameters,
        structural_types,
    )?;
    Ok(Prepared {
        call: LoweredEffectCall {
            result,
            result_claims,
            claim_transfers,
            checked_coordinate: *coordinate,
            source_coordinate: SourceCallCoordinate {
                state: state.state,
                statement_index: coordinate.statement_index as usize,
                call_ordinal: coordinate.call_ordinal as usize,
            },
            target_machine: *target_machine,
            target_state: *target_state,
            arguments: argument_types
                .iter()
                .enumerate()
                .map(
                    |(ordinal, scalar_type)| LoweredDirectExpression::Parameter {
                        position: value_types.len() + ordinal,
                        scalar_type: *scalar_type,
                    },
                )
                .collect(),
            erased_proof_arguments,
            structural_arguments: lowered_arguments,
            // The admission guard above keeps authored crash contracts out of
            // this lane, but an inferred ceiling is body evidence, not an
            // authored clause: emit the callee's effective routes so the
            // operation's continuations cover the verifier's substitution.
            crash_routes: crate::unit::effective_crash_routes(checked, *target_machine)?,
        },
        argument_types: argument_types.into_iter().map(Into::into).collect(),
    })
}

impl Prepared {
    /// All operands have already completed through the common formal-order schedule.
    pub(super) fn finish_completed(
        self,
        value_types: &[QualifiedScalarType],
        target: usize,
        computations: &mut super::computations::Expansion<'_>,
    ) -> usize {
        emit_call(self.call, value_types, value_types, target, computations)
    }
}

fn emit_call(
    call: LoweredEffectCall,
    input_types: &[QualifiedScalarType],
    output_types: &[QualifiedScalarType],
    target: usize,
    computations: &mut super::computations::Expansion<'_>,
) -> usize {
    use super::prepared::{
        LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
    };
    computations.push(LoweredScalarBranchState {
        structural_parameters: Vec::new(),
        parameter_types: input_types.to_vec(),
        erased_formal_types: Vec::new(),
        erased_proof_formals: Vec::new(),
        bindings: Vec::new(),
        structural_effects: vec![LoweredScalarEffect::Call(call)],
        terminator: LoweredScalarBranchTerminator::jump(
            target,
            super::computations::parameters(output_types),
        ),
    })
}
