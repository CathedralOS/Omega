//! Operator contract snapshots, conformance and crash contract refinement.

use super::equality_conjuncts::{
    EqualityConjunct, SourceCorrespondence, collect_equality_conjuncts,
};
use crate::proof_contracts::contract_entailment::{
    Diagnostic, ExpressionHandle, ExpressionNode, Machine, SignatureContractKind,
};
use crate::proof_contracts::contract_structure::{callable_binders, generic_binders};

/// A checked Omega body satisfying an ordinary or boundary operator is a
/// software provider, not an accepted leaf. Its own machine contract is proved
/// by the ordinary entailment pass above; this gate then checks that the proved
/// contract covers the selected operator contract.
///
/// The first checked-software rung deliberately admits the contract language
/// that is already load-bearing for boundary operators: typed identity, type equations, declared Boolean calls,
/// and exact `&&` conjunctions. Provider `requires` may only repeat requirement
/// premises (asking less is valid); every required operator `ensures` conjunct
/// must appear in the provider's proved ensures. Operator parameters are
/// substituted positionally onto provider parameters, and the reserved
/// `result` binder maps only to itself, so renaming is harmless while swapping
/// two parameter roles is not.
pub(crate) fn check_operator_contract_conformance(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    operator: &typed_trees::operator::OperatorDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry_state) = program.machine_states(machine).first() else {
        return; // exact signature validation already reports the missing entry
    };
    let operator_identity =
        typed_trees::operator::boundary_operator_requirement_identity(program, operator);
    let requirement = collect_contracts(
        program,
        operator.symbol,
        program.operator_parameters(operator),
        program.operator_contracts(operator),
    );
    let provider = collect_contracts(
        program,
        machine.symbol,
        program.state_parameters(entry_state),
        program.machine_contracts(machine),
    );

    if requirement.unsupported_ensures {
        diagnostics.push(Diagnostic::error(format!(
            "checked machine `{}` satisfies operator `{operator_identity}`, whose ensures contract is outside checked operator-contract entailment's equality/`&&` rung",
            machine.name,
        )));
        return;
    }
    if provider.unsupported_requires {
        diagnostics.push(Diagnostic::error(format!(
            "checked operator provider `{}` adds a non-equality requires fact while satisfying `{operator_identity}`; checked providers may not ask more than the operator requirement",
            machine.name,
        )));
        return;
    }

    let required_binders = callable_binders(
        &generic_binders(program.operator_type_parameters(operator)),
        program.operator_parameters(operator),
    );
    let provided_binders = callable_binders(
        &generic_binders(program.machine_type_parameters(machine)),
        program.state_parameters(entry_state),
    );
    let mut source_correspondence = SourceCorrespondence::default();
    let mut matches = |requirement_fact, provider_fact| {
        source_correspondence.matching_conjuncts(
            program,
            requirement_fact,
            &required_binders,
            provider_fact,
            &provided_binders,
        )
    };

    for provider_requires_fact in &provider.requires {
        if !requirement
            .requires
            .iter()
            .any(|required| matches(*required, *provider_requires_fact))
        {
            diagnostics.push(Diagnostic::error(format!(
                "checked operator provider `{}` requires `{}`, which operator requirement `{operator_identity}` does not require",
                machine.name,
                program.expression_table.display_name(provider_requires_fact.expression()),
            )));
        }
    }
    for requirement_ensures_fact in &requirement.ensures {
        if !provider
            .ensures
            .iter()
            .any(|provided| matches(*requirement_ensures_fact, *provided))
        {
            diagnostics.push(Diagnostic::error(format!(
                "checked operator provider `{}` proves no ensures matching operator requirement `{operator_identity}` contract `{}`",
                machine.name,
                program.expression_table.display_name(requirement_ensures_fact.expression()),
            )));
        }
    }
    check_operator_crash_contract_refinement(
        program,
        machine,
        operator,
        &operator_identity,
        (&required_binders, &provided_binders),
        &mut source_correspondence,
        diagnostics,
    );
}

fn check_operator_crash_contract_refinement(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    operator: &typed_trees::operator::OperatorDefinition,
    operator_identity: &str,
    binders: (
        &[(symbols::SymbolHandle, String)],
        &[(symbols::SymbolHandle, String)],
    ),
    source_correspondence: &mut SourceCorrespondence,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let requirement_contracts = program.operator_contracts(operator);
    let provider_contracts = program.machine_contracts(machine);
    let mut checked_causes = Vec::new();
    for provider_contract in provider_contracts {
        let SignatureContractKind::Crashes { cause } = provider_contract.kind else {
            continue;
        };
        if checked_causes.contains(&cause) {
            continue;
        }
        checked_causes.push(cause);

        let provider = operator_crash_bucket(program, provider_contracts, cause)
            .expect("a provider crash contract contributes its own bucket");
        let requirement = operator_crash_bucket(program, requirement_contracts, cause);
        let valid = requirement.is_some_and(|requirement| {
            requirement.unconditional
                || (!provider.unconditional
                    && provider.routes.iter().all(|provided| {
                        requirement.routes.iter().any(|required| {
                            let Some(entry) = program.machine_states(machine).first() else {
                                return false;
                            };
                            source_correspondence.matching_values(
                                program,
                                (operator.symbol, program.operator_parameters(operator)),
                                *required,
                                binders.0,
                                (machine.symbol, program.state_parameters(entry)),
                                *provided,
                                binders.1,
                            )
                        })
                    }))
        });
        if !valid {
            diagnostics.push(Diagnostic::error(format!(
                "checked operator provider `{}` does not refine `{operator_identity}`: its `crashes {cause:?}` routes are not contained by the operator crash ceiling",
                machine.name,
            )));
        }
    }
}

struct OperatorCrashBucket {
    unconditional: bool,
    routes: Vec<ExpressionHandle>,
}

fn operator_crash_bucket(
    program: &dyn crate::CheckingInput,
    contracts: &[typed_trees::signature::SignatureContract],
    cause: typed_trees::signature::CrashCause,
) -> Option<OperatorCrashBucket> {
    let matching = contracts
        .iter()
        .filter(|contract| {
            matches!(contract.kind, SignatureContractKind::Crashes { cause: actual } if actual == cause)
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return None;
    }
    let mut unconditional = false;
    let mut routes = Vec::new();
    for contract in matching {
        let mut contract_routes = Vec::new();
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(expression) = fact.as_expression() else {
                continue;
            };
            if matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Boolean(true)
            ) {
                continue;
            }
            contract_routes.push(*expression);
        }
        if contract_routes.is_empty() {
            unconditional = true;
        }
        routes.extend(contract_routes);
    }
    Some(OperatorCrashBucket {
        unconditional,
        routes,
    })
}

struct ContractConjuncts<'scope> {
    requires: Vec<EqualityConjunct<'scope>>,
    ensures: Vec<EqualityConjunct<'scope>>,
    unsupported_requires: bool,
    unsupported_ensures: bool,
}

fn collect_contracts<'scope>(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    parameters: &'scope [typed_trees::signature::StateParameter],
    contracts: &[typed_trees::signature::SignatureContract],
) -> ContractConjuncts<'scope> {
    let mut result = ContractConjuncts {
        requires: Vec::new(),
        ensures: Vec::new(),
        unsupported_requires: false,
        unsupported_ensures: false,
    };
    for contract in contracts {
        let (destination, unsupported) = match contract.kind {
            SignatureContractKind::Requires => {
                (&mut result.requires, &mut result.unsupported_requires)
            }
            SignatureContractKind::Ensures => {
                (&mut result.ensures, &mut result.unsupported_ensures)
            }
            SignatureContractKind::EnsuresForResultCase { .. }
            | SignatureContractKind::Crashes { .. } => continue,
        };
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(expression) = fact.as_expression() else {
                *unsupported = true;
                continue;
            };
            if !collect_equality_conjuncts(program, owner, parameters, *expression, destination) {
                *unsupported = true;
            }
        }
    }
    result
}
