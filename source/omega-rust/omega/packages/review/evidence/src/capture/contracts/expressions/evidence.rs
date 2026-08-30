use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractKind,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_contract_call_evidence_arguments(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    target_state_symbol: SymbolHandle,
    authored_count: usize,
) -> Result<Vec<PackageReviewContractEvidenceArgument>, Vec<Diagnostic>> {
    if authored_count == 0 {
        return Ok(Vec::new());
    }
    let Some(fact) = fact else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence-bearing call has no exact owning proof fact",
            context.subject_kind, context.subject_name,
        ))]);
    };
    let matching = compilation
        .facts
        .proof
        .contract_expression_evidence_calls
        .iter()
        .filter(|checked| {
            checked.owner == context.owner
                && checked.fact == fact
                && checked.expression == expression
        })
        .collect::<Vec<_>>();
    let [checked] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence-bearing call has {} exact checked occurrence rows; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len(),
        ))]);
    };
    let Some((target_machine, target_state)) =
        exact_contract_target(&compilation.typed, target_state_symbol)
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence-bearing call has no exact target owner",
            context.subject_kind, context.subject_name,
        ))]);
    };
    let parameters = exact_target_evidence_parameters(compilation, target_machine, target_state);
    if checked.target_machine_symbol != target_machine
        || checked.target_state_symbol != target_state
        || checked.evidence_arguments.len() != authored_count
        || parameters.len() != authored_count
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence-bearing call disagrees with its exact checked target or lane arity",
            context.subject_kind, context.subject_name,
        ))]);
    }

    checked
        .evidence_arguments
        .iter()
        .enumerate()
        .map(|(lane_position, binding)| {
            if binding.lane_position != lane_position {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` evidence-bearing call changed checked lane position {}",
                    context.subject_kind, context.subject_name, lane_position,
                ))]);
            }
            if binding.parameter != parameters[lane_position] {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` evidence-bearing call changed checked parameter binding at lane {}",
                    context.subject_kind, context.subject_name, lane_position,
                ))]);
            }
            let source = compilation.facts.proof.evidence_terms.get(binding.source);
            let parameter = compilation
                .facts
                .proof
                .evidence_terms
                .get(binding.parameter);
            if source.kind != psi_checked_trees::ContractProofFactKind::Requires
                || !evidence_owner_visible_from(source.owner, context.owner)
                || source.proposition != binding.instantiated_proposition
                || parameter.kind != psi_checked_trees::ContractProofFactKind::Requires
                || !evidence_owner_belongs_to_target(
                    parameter.owner,
                    target_machine,
                    target_state,
                )
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` evidence-bearing call changed checked binding at lane {}",
                    context.subject_kind, context.subject_name, lane_position,
                ))]);
            }
            Ok(PackageReviewContractEvidenceArgument {
                lane_position: portable_lane(lane_position)?,
                source: project_term(compilation, source)?,
                parameter: project_term(compilation, parameter)?,
            })
        })
        .collect()
}

fn exact_target_evidence_parameters(
    compilation: &CheckedCompilation,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Vec<psi_arena::Handle<psi_checked_trees::CheckedEvidenceTerm>> {
    compilation
        .facts
        .proof
        .contract_facts
        .iter()
        .filter_map(|(_, contract)| {
            let parameter = contract.evidence_term?;
            let checked = compilation.facts.proof.evidence_terms.get(parameter);
            (contract.kind == psi_checked_trees::ContractProofFactKind::Requires
                && evidence_owner_belongs_to_target(contract.owner, target_machine, target_state)
                && checked.owner == contract.owner
                && checked.kind == contract.kind)
                .then_some(parameter)
        })
        .collect()
}

fn exact_contract_target(
    program: &psi_typed_trees::TypedTrees,
    target: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    if let Some(machine) = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target)
    }) {
        return Some((machine.symbol, target));
    }
    program.traits().iter().find_map(|definition| {
        program
            .trait_machine_signatures(definition)
            .iter()
            .any(|requirement| requirement.symbol == target)
            .then_some((definition.symbol, target))
    })
}

fn evidence_owner_visible_from(
    term_owner: psi_checked_trees::ContractProofFactOwner,
    fact_owner: psi_checked_trees::ContractProofFactOwner,
) -> bool {
    term_owner == fact_owner
        || matches!(
            (term_owner, fact_owner),
            (
                psi_checked_trees::ContractProofFactOwner::Machine {
                    machine_symbol: term_machine,
                },
                psi_checked_trees::ContractProofFactOwner::MachineState {
                    machine_symbol: fact_machine,
                    ..
                },
            ) if term_machine == fact_machine
        )
}

fn evidence_owner_belongs_to_target(
    owner: psi_checked_trees::ContractProofFactOwner,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> bool {
    matches!(
        owner,
        psi_checked_trees::ContractProofFactOwner::Machine { machine_symbol }
            if machine_symbol == target_machine
    ) || matches!(
        owner,
        psi_checked_trees::ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } if machine_symbol == target_machine && state_symbol == target_state
    ) || matches!(
        owner,
        psi_checked_trees::ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } if owner_symbol == target_machine && state_symbol == target_state
    )
}

fn project_term(
    compilation: &CheckedCompilation,
    term: &psi_checked_trees::CheckedEvidenceTerm,
) -> Result<PackageReviewContractEvidenceTerm, Vec<Diagnostic>> {
    let owner = match term.owner {
        psi_checked_trees::ContractProofFactOwner::Machine { machine_symbol } => machine_symbol,
        psi_checked_trees::ContractProofFactOwner::MachineState { state_symbol, .. }
        | psi_checked_trees::ContractProofFactOwner::StateSignature { state_symbol, .. } => {
            state_symbol
        }
        psi_checked_trees::ContractProofFactOwner::OperatorDeclaration { operator_symbol }
        | psi_checked_trees::ContractProofFactOwner::OperatorUse {
            operator_symbol, ..
        } => operator_symbol,
        psi_checked_trees::ContractProofFactOwner::Unknown => {
            return Err(vec![Diagnostic::error(
                "checked evidence term has no stable semantic owner",
            )]);
        }
    };
    Ok(PackageReviewContractEvidenceTerm {
        owner: nominal_identity(compilation, owner)?,
        kind: match term.kind {
            psi_checked_trees::ContractProofFactKind::Requires => {
                PackageReviewContractKind::Requires
            }
            psi_checked_trees::ContractProofFactKind::Ensures => PackageReviewContractKind::Ensures,
        },
        lane_position: portable_lane(term.lane_position)?,
    })
}

fn portable_lane(position: usize) -> Result<u32, Vec<Diagnostic>> {
    u32::try_from(position).map_err(|_| {
        vec![Diagnostic::error(
            "checked evidence lane exceeds the portable package-review range",
        )]
    })
}
