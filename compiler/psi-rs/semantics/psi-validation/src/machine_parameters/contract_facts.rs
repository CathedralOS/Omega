use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::TypeParameter;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_contract_facts(
    program: &TypedTrees,
    label: &str,
    parameter: &TypeParameter,
    requirement: &psi_typed_trees::signature::StateSignature,
    actual_contracts: &[SignatureContract],
    required_parameters: &[StateParameter],
    actual_parameters: &[StateParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_contracts = program.state_signature_contracts(requirement);
    for kind in [
        SignatureContractKind::Requires,
        SignatureContractKind::Ensures,
        SignatureContractKind::Boundary,
    ] {
        let required = normalized_facts(program, required_contracts, &kind, required_parameters);
        let actual = normalized_facts(program, actual_contracts, &kind, actual_parameters);
        let valid = match &kind {
            // Required preconditions may be stronger: callers of the generic
            // already prove them, so the selected implementation may demand a
            // subset. Selected postconditions must cover the requirement.
            SignatureContractKind::Requires => actual.iter().all(|fact| required.contains(fact)),
            SignatureContractKind::Ensures | SignatureContractKind::Boundary => {
                required.iter().all(|fact| actual.contains(fact))
            }
            SignatureContractKind::Crashes { .. } => {
                unreachable!("crash routes compare separately")
            }
        };
        if !valid {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: its {} facts are not a conservative refinement",
                parameter.name,
                contract_kind_name(&kind)
            )));
        }
    }
    validate_crash_contract_refinement(
        program,
        label,
        parameter,
        required_contracts,
        actual_contracts,
        required_parameters,
        actual_parameters,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_crash_contract_refinement(
    program: &TypedTrees,
    label: &str,
    parameter: &TypeParameter,
    required_contracts: &[SignatureContract],
    actual_contracts: &[SignatureContract],
    required_parameters: &[StateParameter],
    actual_parameters: &[StateParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut checked = Vec::<SignatureContractKind>::new();
    for actual_contract in actual_contracts {
        let SignatureContractKind::Crashes { .. } = &actual_contract.kind else {
            continue;
        };
        if checked.contains(&actual_contract.kind) {
            continue;
        }
        checked.push(actual_contract.kind.clone());

        let actual_bucket = normalized_crash_bucket(
            program,
            actual_contracts,
            &actual_contract.kind,
            actual_parameters,
        )
        .expect("an actual crash contract contributes its own bucket");
        let required_bucket = normalized_crash_bucket(
            program,
            required_contracts,
            &actual_contract.kind,
            required_parameters,
        );
        let valid = required_bucket.is_some_and(|required| {
            required.unconditional
                || (!actual_bucket.unconditional
                    && actual_bucket
                        .routes
                        .iter()
                        .all(|route| required.routes.contains(route)))
        });
        if !valid {
            let SignatureContractKind::Crashes { cause } = &actual_contract.kind else {
                unreachable!("checked crash bucket kind")
            };
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: its `crashes {cause:?}` routes are not contained by the required crash ceiling",
                parameter.name,
            )));
        }
    }
}

struct NormalizedCrashBucket {
    unconditional: bool,
    routes: Vec<String>,
}

fn normalized_crash_bucket(
    program: &TypedTrees,
    contracts: &[SignatureContract],
    kind: &SignatureContractKind,
    parameters: &[StateParameter],
) -> Option<NormalizedCrashBucket> {
    let matching = contracts
        .iter()
        .filter(|contract| &contract.kind == kind)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return None;
    }
    let unconditional = matching.iter().any(|contract| {
        normalized_facts(program, std::slice::from_ref(*contract), kind, parameters).is_empty()
    });
    let mut routes = normalized_facts(program, contracts, kind, parameters);
    routes.sort();
    routes.dedup();
    Some(NormalizedCrashBucket {
        unconditional,
        routes,
    })
}

fn normalized_facts(
    program: &TypedTrees,
    contracts: &[SignatureContract],
    kind: &SignatureContractKind,
    parameters: &[StateParameter],
) -> Vec<String> {
    let mut facts = Vec::new();
    for contract in contracts.iter().filter(|contract| &contract.kind == kind) {
        for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
            if matches!(
                fact,
                ProofFact::Expression(expression)
                    if matches!(
                        program.expression_table.expression(*expression),
                        ExpressionNode::Boolean(true)
                    )
            ) {
                // `true` contributes no precondition or postcondition. Treat
                // it as the identity element so an implementation need not
                // redundantly republish a tautology to refine a slot.
                continue;
            }
            let raw = match fact {
                ProofFact::Expression(expression) => {
                    program.expression_table.display_name(*expression)
                }
                ProofFact::Membership(membership) => format!(
                    "{} in {}",
                    program.expression_table.display_name(membership.value),
                    program
                        .domain_path_members(membership.domain)
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::")
                ),
                ProofFact::Proposition(application) => {
                    let binders = application
                        .binder_arguments
                        .iter()
                        .map(|argument| {
                            argument
                                .path
                                .iter()
                                .map(|member| member.as_str())
                                .collect::<Vec<_>>()
                                .join("::")
                        })
                        .collect::<Vec<_>>();
                    let arguments = program
                        .expression_table
                        .expression_handles(application.arguments)
                        .iter()
                        .map(|argument| program.expression_table.display_name(*argument))
                        .collect::<Vec<_>>();
                    let binder_suffix = if binders.is_empty() {
                        String::new()
                    } else {
                        format!("[{}]", binders.join(", "))
                    };
                    format!(
                        "{}{binder_suffix}({})",
                        application.name.as_str(),
                        arguments.join(", ")
                    )
                }
            };
            facts.push(alpha_normalize(&raw, parameters));
        }
    }
    facts.sort();
    facts.dedup();
    facts
}

fn alpha_normalize(text: &str, parameters: &[StateParameter]) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < characters.len() {
        if !characters[cursor].is_ascii_alphanumeric() && characters[cursor] != '_' {
            output.push(characters[cursor]);
            cursor += 1;
            continue;
        }

        let start = cursor;
        while cursor < characters.len()
            && (characters[cursor].is_ascii_alphanumeric() || characters[cursor] == '_')
        {
            cursor += 1;
        }
        let word: String = characters[start..cursor].iter().collect();
        let previous = characters[..start]
            .iter()
            .rev()
            .copied()
            .find(|character| !character.is_whitespace());
        let next = characters[cursor..]
            .iter()
            .copied()
            .find(|character| !character.is_whitespace());
        // Replace value references, never field/case labels, member names,
        // call targets, or type constructors that merely share the spelling.
        let is_declaration_name =
            matches!(previous, Some('.' | ':')) || matches!(next, Some(':' | '(' | '{'));
        if !is_declaration_name
            && let Some(index) = parameters
                .iter()
                .position(|parameter| parameter.name.as_str() == word)
        {
            output.push('$');
            output.push_str(&index.to_string());
        } else {
            output.push_str(&word);
        }
    }
    output
}

fn contract_kind_name(kind: &SignatureContractKind) -> &'static str {
    match kind {
        SignatureContractKind::Requires => "requires",
        SignatureContractKind::Ensures => "ensures",
        SignatureContractKind::Boundary => "boundary",
        SignatureContractKind::Crashes { .. } => "crashes",
    }
}
