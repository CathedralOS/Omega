//! Lower ordinary calls retained by scalar evaluation using their checked custody.
//! Unit effects and structural values share argument admission and call emission.
use super::{
    CheckedTrees, LoweringError, Multiplicity, QualifiedScalarType, storage, terminal_scalar_type,
    unsupported,
};
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredEffectCall;

use crate::unit::attached_unit::bodies::{UnitBody, UnitPlans};
use checked_trees::{CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan};

pub(super) struct Prepared {
    pub(super) coordinate: CheckedUnitCallCoordinate,
    pub(super) arguments: Vec<checked_trees::CheckedCallScalarArgument>,
    pub(super) argument_types: Vec<QualifiedScalarType>,
    pub(super) call: LoweredEffectCall,
}

pub(super) fn statement_index(
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<u32, LoweringError> {
    match operation {
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
            Ok(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => {
            Ok(result.statement_index)
        }
        _ => Ok(coordinate(operation)?.statement_index),
    }
}

pub(super) fn coordinate(
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<CheckedUnitCallCoordinate, LoweringError> {
    // A scalar graph retains only ordinary Unit calls.
    match operation.call() {
        Some(call)
            if call.is_module() && *call.result() == checked_trees::CheckedUnitCallResult::Unit =>
        {
            Ok(call.coordinate)
        }
        _ => unsupported("scalar graph retained an unsupported Unit operation"),
    }
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
        if let Some(plan) = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter()
            .find(|plan| plan.machine == machine)
            .and_then(|plan| plan.states.iter().find(|plan| plan.state == state))
        {
            return Ok(Self {
                state,
                erased_proof_parameters: &plan.erased_proof_parameters,
            });
        }
        match UnitBody::find(
            UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
            machine,
        )? {
            UnitBody::Ordinary(plan) if plan.state == state => Ok(Self {
                state,
                erased_proof_parameters: &plan.erased_proof_parameters,
            }),
            UnitBody::Composed(plan) => {
                let plan = plan.states.iter().find(|plan| plan.state == state).ok_or(
                    LoweringError::Unsupported("effect caller has no published state"),
                )?;
                Ok(Self {
                    state,
                    erased_proof_parameters: &plan.erased_proof_parameters,
                })
            }
            _ => unsupported("effect caller has no published state"),
        }
    }
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
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
        custody.reference_loan.is_valid()
            || !custody.result_qualifications.is_empty()
            || !custody.returned_claim_transfers.is_empty()
    }) {
        return unsupported("scalar effect call requires its returned loan or claim transport");
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
            crate::scalar_graph::scalar_contracts::checked_proof_term(
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
            checked_trees::CheckedControlResultPlan::Structural(expected),
            checked_trees::CheckedUnitCallResult::Structural { binding, .. },
            Some(result),
        ) => {
            let declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
                .ok_or(LoweringError::Unsupported(
                    "effect call result has no declared carrier",
                ))?;
            if expected.type_identity != binding.type_identity
                || expected.multiplicity != binding.multiplicity
                || declaration.identity != binding.type_identity
                || result.multiplicity
                    != match binding.multiplicity {
                        Multiplicity::Unrestricted => {
                            terminal_psi::StructuralMultiplicity::Unrestricted
                        }
                        Multiplicity::Affine => terminal_psi::StructuralMultiplicity::Affine,
                        Multiplicity::Linear => terminal_psi::StructuralMultiplicity::Linear,
                    }
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
            {
                return unsupported("effect call substituted its structural result carrier");
            }
        }
        _ => return unsupported("effect call disagrees with its target result"),
    }
    if target.state != *target_state
        || target.contract_report_fingerprint != *target_contract_report_fingerprint
        || !crate::unit::attached_unit::catalog::checked_unit_target_reach_matches(
            *service_reach,
            target.contract_service_reach,
        )
        || !target.entry_claims.is_empty()
        || !claim_transfers.is_empty()
        || structural_arguments.len() != target.structural_parameters.len()
        || scalar_arguments.len() != target.scalar_parameters.len()
    {
        return unsupported("scalar Unit call disagrees with its exact target custody");
    }
    let _ = crate::expression_preparation::source_custody::flow_calls::retain_exact_flow_call(
        checked,
        machine,
        state.state,
        *coordinate,
        *target_state,
    )?;
    let authored = crate::call_sites::locate(checked, state.state, *coordinate)?;
    let (_, caller_state) = crate::declarations::state_declaration(checked, state.state)?;
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
    let borrow = crate::expression_preparation::source_custody::borrow_occurrences::call(
        checked,
        machine,
        state.state,
        *coordinate,
        authored.source_target,
    )?;
    let authored_arguments = &authored.explicit_arguments;
    let access_positions =
        crate::expression_preparation::source_custody::computation_calls::rejoin_call_accesses(
            checked,
            machine,
            state.state,
            coordinate.statement_index,
            borrow,
            authored_arguments,
        )?;
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
            || parameter.fused_service_erasure.is_some()
            || parameter.access != argument.access
            || parameter.type_identity != argument.type_identity
        {
            return unsupported("scalar Unit argument does not retain a plain primitive borrow");
        }
        if argument.access == checked_trees::CheckedStructuralAccess::Owned {
            let source = bindings.owned_call_argument(argument, results, structural_types)?;
            lowered_arguments.push(source);
            continue;
        }
        let source_parameter = checked
            .state_parameters(callee_state)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "scalar Unit formal position is absent",
            ))?;
        let expression = authored
            .structural_arguments
            .iter()
            .find_map(|(position, expression)| {
                (*position == parameter.position).then_some(*expression)
            })
            .ok_or(LoweringError::Unsupported(
                "scalar Unit call lost its authored structural actual",
            ))?;
        let access_position = access_positions
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "scalar Unit argument has no authored access position",
            ))?
            .operand_position()?;
        let actual_position =
            crate::expression_preparation::source_custody::computation_calls::primitive_arguments::validate(
                checked,
                caller_state,
                coordinate.statement_index,
                source_parameter.type_reference,
                expression,
                argument,
                borrow,
                Some(access_position),
            )?;
        if actual_position != access_position {
            return unsupported("scalar Unit call reordered its structural access occurrences");
        }
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
    for ((_, actual), expected) in authored.scalar_arguments.iter().zip(&argument_types) {
        if terminal_scalar_type(*actual)? != *expected {
            return unsupported("scalar Unit argument substituted its formal carrier");
        }
    }
    Ok(Prepared {
        coordinate: *coordinate,
        arguments: scalar_arguments.clone(),
        call: LoweredEffectCall {
            result,
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

    pub(super) fn finish(
        self,
        state: symbols::SymbolHandle,
        bindings: &storage::ScalarBindings,
        value_types: &[QualifiedScalarType],
        target: usize,
        computations: &mut super::computations::Expansion<'_>,
    ) -> Result<usize, LoweringError> {
        let mut argument_types = value_types.to_vec();
        argument_types.extend_from_slice(&self.argument_types);
        let call_block = emit_call(
            self.call,
            &argument_types,
            value_types,
            target,
            computations,
        );
        computations.call_arguments(
            state,
            self.coordinate,
            &self.arguments,
            0,
            bindings,
            value_types,
            call_block,
        )
    }
}

fn emit_call(
    call: LoweredEffectCall,
    input_types: &[QualifiedScalarType],
    output_types: &[QualifiedScalarType],
    target: usize,
    computations: &mut super::computations::Expansion<'_>,
) -> usize {
    use super::prepared_graph::{
        LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
    };
    computations.push(LoweredScalarBranchState {
        structural_parameters: Vec::new(),
        parameter_types: input_types.to_vec(),
        erased_formal_types: Vec::new(),
        erased_proof_formals: Vec::new(),
        bindings: Vec::new(),
        structural_effects: vec![LoweredScalarEffect::Call(call)],
        terminator: LoweredScalarBranchTerminator::Jump {
            cleanup_actions: Vec::new(),
            target,
            arguments: super::computations::parameters(output_types),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
        },
    })
}
