// Contract-expression call evidence argument projection, verifying that each lane
// binds a visible `Requires` evidence term whose instantiated proposition matches
// the target callable's `Requires` parameter lane.

use super::names::portable_ordinal;
use crate::capture::SourceDeclarationInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceOwner, PackageReviewContractEvidenceTerm,
    PackageReviewContractKind, PackageReviewTypeIdentity,
};
use checked_trees::ContractProofFactOwner;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn project_contract_call_evidence_arguments(
    compilation: &SourceDeclarationInput<'_>,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    fact: Option<arena::Handle<typed_trees::domain::ProofFact>>,
    expression: typed_trees::expression::ExpressionHandle,
    authored_count: usize,
) -> Result<Vec<PackageReviewContractEvidenceArgument>, Vec<Diagnostic>> {
    if authored_count == 0 {
        return Ok(Vec::new());
    }
    let Some(fact) = fact else {
        return Err(context.rejected("evidence-bearing call has no exact owning proof fact"));
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
        return Err(context.rejected(format!(
            "evidence-bearing call has {} exact checked occurrence rows; expected one",
            matching.len()
        )));
    };
    let call = compilation.expression_table.call_node(expression)
        .ok_or_else(|| context.rejected("evidence-bearing expression is not a call"))?;
    let runtime = super::calls::exact_checked_runtime_contract_call(compilation, context, expression, call)?;
    enum Endpoint {
        Nominal { machine: SymbolHandle, state: SymbolHandle },
        Runtime { contract: typed_trees::callable_values::RuntimeCallableContractHandle },
    }
    let endpoint = if let Some((binding, _)) = runtime {
        if checked.runtime_contract != Some(binding.contract)
            || checked.target_machine_symbol.is_valid() || checked.target_state_symbol.is_valid() {
            return Err(context.rejected("evidence-bearing runtime call changed its exact checked source"));
        }
        Endpoint::Runtime { contract: binding.contract }
    } else {
        let target = super::calls::resolved_contract_call_symbol(compilation, call)
            .or_else(|| compilation.derive_checked_nominal_call_target(expression))
            .ok_or_else(|| context.rejected("evidence-bearing call has no exact resolved target"))?;
        let (machine, state) = exact_contract_target(compilation.checking_input, target)
            .ok_or_else(|| context.rejected("evidence-bearing call has no exact target owner"))?;
        if checked.runtime_contract.is_some() || checked.target_machine_symbol != machine
            || checked.target_state_symbol != state {
            return Err(context.rejected("evidence-bearing call changed its exact nominal source"));
        }
        Endpoint::Nominal { machine, state }
    };
    let parameters = match endpoint {
        Endpoint::Nominal { machine, state } => typed_trees_to_checked_trees::derive_checked_contract_expression_evidence_parameters(
            &compilation.facts, machine, state,
        ),
        Endpoint::Runtime { contract } => typed_trees_to_checked_trees::derive_checked_runtime_contract_expression_evidence_parameters(
            &compilation.facts, contract,
        ),
    };
    if checked.evidence_arguments.len() != authored_count || parameters.len() != authored_count {
        return Err(context.rejected("evidence-bearing call disagrees with its exact checked lane arity"));
    }

    checked
        .evidence_arguments
        .iter()
        .enumerate()
        .map(|(lane_position, binding)| {
            if binding.lane_position != lane_position {
                return Err(context.rejected(format!(
                    "evidence-bearing call changed checked lane position {}",
                    lane_position
                )));
            }
            if binding.parameter != parameters[lane_position] {
                return Err(context.rejected(format!(
                    "evidence-bearing call changed checked parameter binding at lane {}",
                    lane_position
                )));
            }
            let source = compilation.facts.proof.evidence_terms.get(binding.source);
            let parameter = compilation
                .facts
                .proof
                .evidence_terms
                .get(binding.parameter);
            let current_instantiation = typed_trees_to_checked_trees::
                derive_checked_contract_expression_evidence_instantiation(
                    compilation.checking_input,
                    &compilation.facts,
                    expression,
                    match endpoint { Endpoint::Nominal { state, .. } => state,
                        Endpoint::Runtime { .. } => SymbolHandle::invalid() },
                    binding.parameter,
                );
            if source.kind != checked_trees::ContractProofFactKind::Requires
                || !evidence_owner_visible_from(source.owner, context.owner)
                || source.proposition != binding.instantiated_proposition
                || current_instantiation.as_ref() != Some(&binding.instantiated_proposition)
                || parameter.kind != checked_trees::ContractProofFactKind::Requires
                || !match endpoint {
                    Endpoint::Nominal { machine, state } => evidence_owner_belongs_to_target(parameter.owner, machine, state),
                    Endpoint::Runtime { contract } => parameter.owner == ContractProofFactOwner::RuntimeCallableContract { contract },
                }
            {
                return Err(context.rejected(format!(
                    "evidence-bearing call changed checked binding at lane {}",
                    lane_position
                )));
            }
            Ok(PackageReviewContractEvidenceArgument {
                lane_position: portable_ordinal(lane_position)?,
                source: project_term(compilation, binders, source)?,
                parameter: project_term(compilation, binders, parameter)?,
            })
        })
        .collect()
}

fn exact_contract_target(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    if let Some(machine) = program.machines().iter().find(|machine| {
        typed_trees::state::state_by_symbol(program.machine_states(machine), target).is_some()
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
    term_owner: ContractProofFactOwner,
    fact_owner: ContractProofFactOwner,
) -> bool {
    term_owner == fact_owner
        || matches!(
            (term_owner, fact_owner),
            (
                ContractProofFactOwner::Machine {
                    machine_symbol: term_machine,
                },
                ContractProofFactOwner::MachineState {
                    machine_symbol: fact_machine,
                    ..
                },
            ) if term_machine == fact_machine
        )
}

fn evidence_owner_belongs_to_target(
    owner: ContractProofFactOwner,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> bool {
    matches!(
        owner,
        ContractProofFactOwner::Machine { machine_symbol }
            if machine_symbol == target_machine
    ) || matches!(
        owner,
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } if machine_symbol == target_machine && state_symbol == target_state
    ) || matches!(
        owner,
        ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } if owner_symbol == target_machine && state_symbol == target_state
    )
}

fn project_term(
    compilation: &SourceDeclarationInput<'_>,
    binders: &[(SymbolHandle, String)],
    term: &checked_trees::CheckedEvidenceTerm,
) -> Result<PackageReviewContractEvidenceTerm, Vec<Diagnostic>> {
    let nominal_owner = match term.owner {
        ContractProofFactOwner::RuntimeCallableContract { contract } => {
            let canonical = compilation.runtime_callable_contract_identity(contract, binders)
                .map_err(|reason| vec![Diagnostic::error(format!("checked runtime evidence owner has no complete identity: {reason}"))])?;
            return Ok(PackageReviewContractEvidenceTerm {
                owner: PackageReviewContractEvidenceOwner::Runtime { contract: PackageReviewTypeIdentity { canonical } },
                kind: match term.kind { checked_trees::ContractProofFactKind::Requires => PackageReviewContractKind::Requires,
                    checked_trees::ContractProofFactKind::Ensures => PackageReviewContractKind::Ensures },
                lane_position: portable_ordinal(term.lane_position)?,
            });
        }
        ContractProofFactOwner::Machine { machine_symbol } => machine_symbol,
        ContractProofFactOwner::MachineState { state_symbol, .. }
        | ContractProofFactOwner::StateSignature { state_symbol, .. } => state_symbol,
        ContractProofFactOwner::OperatorDeclaration { operator_symbol }
        | ContractProofFactOwner::OperatorUse {
            operator_symbol, ..
        } => operator_symbol,
        ContractProofFactOwner::CallSignature { .. } => {
            return Err(vec![Diagnostic::error(
                "invocation-owned evidence cannot be exported as a declaration-owned contract lane",
            )]);
        }
        ContractProofFactOwner::Unknown => {
            return Err(vec![Diagnostic::error(
                "checked evidence term has no stable semantic owner",
            )]);
        }
    };
    Ok(PackageReviewContractEvidenceTerm {
        owner: PackageReviewContractEvidenceOwner::Nominal(nominal_identity(compilation, nominal_owner)?),
        kind: match term.kind {
            checked_trees::ContractProofFactKind::Requires => PackageReviewContractKind::Requires,
            checked_trees::ContractProofFactKind::Ensures => PackageReviewContractKind::Ensures,
        },
        lane_position: portable_ordinal(term.lane_position)?,
    })
}
