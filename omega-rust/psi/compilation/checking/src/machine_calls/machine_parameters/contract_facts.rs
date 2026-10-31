use crate::frozen_program::expression_table;
use diagnostics::Diagnostic;
use symbols::JoinedPath;
use typed_trees::data::TypeParameter;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_contract_facts(
    program: &dyn crate::CheckingInput,
    label: &str,
    parameter: &TypeParameter,
    requirement: &typed_trees::signature::StateSignature,
    actual_contracts: &[SignatureContract],
    required_parameters: &[StateParameter],
    actual_parameters: &[StateParameter],
    actual_owner: symbols::SymbolHandle,
    substitutions: &[(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
    )],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_contracts = program.state_signature_contracts(requirement);
    validate_callable_contract_refinement(
        program,
        label,
        parameter.name.as_str(),
        required_contracts,
        actual_contracts,
        required_parameters,
        actual_parameters,
        requirement.symbol,
        actual_owner,
        substitutions,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_callable_contract_refinement(
    program: &dyn crate::CheckingInput,
    label: &str,
    requirement_identity: &str,
    required_contracts: &[SignatureContract],
    actual_contracts: &[SignatureContract],
    required_parameters: &[StateParameter],
    actual_parameters: &[StateParameter],
    required_owner: symbols::SymbolHandle,
    actual_owner: symbols::SymbolHandle,
    substitutions: &[(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
    )],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for kind in [
        SignatureContractKind::Requires,
        SignatureContractKind::Ensures,
    ] {
        let required = normalized_facts(
            program,
            required_contracts,
            &kind,
            required_parameters,
            substitutions,
        );
        let actual = normalized_facts(program, actual_contracts, &kind, actual_parameters, &[]);
        let valid = match &kind {
            // Required preconditions may be stronger: callers of the generic
            // already prove them, so the selected implementation may demand a
            // subset. Selected postconditions must cover the requirement.
            SignatureContractKind::Requires => actual.iter().all(|fact| required.contains(fact)),
            SignatureContractKind::Ensures => required.iter().all(|fact| {
                actual.contains(fact)
                    || single_guarantee_left_prefix(
                        program,
                        required_contracts,
                        actual_contracts,
                        required_parameters,
                        actual_parameters,
                        required_owner,
                        actual_owner,
                    )
            }),
            SignatureContractKind::EnsuresForResultCase { .. } => {
                unreachable!("outcome-specific guarantees are not machine-parameter contracts")
            }
            SignatureContractKind::Crashes { .. } => {
                unreachable!("crash routes compare separately")
            }
        };
        if !valid {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: its {} facts are not a conservative refinement",
                requirement_identity,
                contract_kind_name(&kind)
            )));
        }
    }
    validate_crash_contract_refinement(
        program,
        label,
        requirement_identity,
        required_contracts,
        actual_contracts,
        required_parameters,
        actual_parameters,
        substitutions,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_crash_contract_refinement(
    program: &dyn crate::CheckingInput,
    label: &str,
    requirement_identity: &str,
    required_contracts: &[SignatureContract],
    actual_contracts: &[SignatureContract],
    required_parameters: &[StateParameter],
    actual_parameters: &[StateParameter],
    substitutions: &[(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
    )],
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
            &[],
        )
        .expect("an actual crash contract contributes its own bucket");
        let required_bucket = normalized_crash_bucket(
            program,
            required_contracts,
            &actual_contract.kind,
            required_parameters,
            substitutions,
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
                requirement_identity,
            )));
        }
    }
}

struct NormalizedCrashBucket {
    unconditional: bool,
    routes: Vec<String>,
}

fn normalized_crash_bucket(
    program: &dyn crate::CheckingInput,
    contracts: &[SignatureContract],
    kind: &SignatureContractKind,
    parameters: &[StateParameter],
    substitutions: &[(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
    )],
) -> Option<NormalizedCrashBucket> {
    let matching = contracts
        .iter()
        .filter(|contract| &contract.kind == kind)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return None;
    }
    let unconditional = matching.iter().any(|contract| {
        normalized_facts(
            program,
            std::slice::from_ref(*contract),
            kind,
            parameters,
            substitutions,
        )
        .is_empty()
    });
    let mut routes = normalized_facts(program, contracts, kind, parameters, substitutions);
    routes.sort();
    routes.dedup();
    Some(NormalizedCrashBucket {
        unconditional,
        routes,
    })
}

fn normalized_facts(
    program: &dyn crate::CheckingInput,
    contracts: &[SignatureContract],
    kind: &SignatureContractKind,
    parameters: &[StateParameter],
    substitutions: &[(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
    )],
) -> Vec<String> {
    let mut facts = Vec::new();
    for contract in contracts.iter().filter(|contract| &contract.kind == kind) {
        for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
            if matches!(
                fact,
                ProofFact::Expression(expression)
                    if matches!(
                        expression_table(program).expression(*expression),
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
                    expression_table(program).display_name(*expression)
                }
                ProofFact::Membership(membership) => format!(
                    "{} in {}{}",
                    expression_table(program).display_name(membership.value),
                    program.domain_path_members(membership.domain).joined_path(),
                    if membership.domain_arguments.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "<{}>",
                            program
                                .type_reference_table
                                .type_reference_handles(membership.domain_arguments)
                                .iter()
                                .map(|argument| program.normalized_type_identity_text(*argument))
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    }
                ),
                ProofFact::Proposition(application) => {
                    let binders = application
                        .binder_arguments
                        .iter()
                        .map(|argument| argument.path.joined_path())
                        .collect::<Vec<_>>();
                    let arguments = expression_table(program)
                        .expression_handles(application.arguments)
                        .iter()
                        .map(|argument| expression_table(program).display_name(*argument))
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
            let substituted = substitute_type_actuals(program, &raw, substitutions);
            facts.push(alpha_normalize(&substituted, parameters));
        }
    }
    facts.sort();
    facts.dedup();
    facts
}

// A true short-circuit conjunction establishes its left prefix without
// requiring the right operand to be formable independently. Restrict this
// comparison to a single required expression and authenticate every leaf and
// selected operator. More complex requirements remain conservatively refused.
fn single_guarantee_left_prefix(
    program: &dyn crate::CheckingInput,
    required_contracts: &[SignatureContract],
    actual_contracts: &[SignatureContract],
    required_parameters: &[StateParameter],
    actual_parameters: &[StateParameter],
    expected_required_owner: symbols::SymbolHandle,
    expected_actual_owner: symbols::SymbolHandle,
) -> bool {
    for parameters in [required_parameters, actual_parameters] {
        for (index, parameter) in parameters.iter().enumerate() {
            if !parameter.symbol.is_valid()
                || parameters[..index]
                    .iter()
                    .any(|earlier| earlier.symbol == parameter.symbol)
            {
                return false;
            }
        }
    }
    let required = required_contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        .flat_map(|contract| program.tables.proof_facts.span_or_empty(contract.facts))
        .collect::<Vec<_>>();
    let [ProofFact::Expression(required)] = required.as_slice() else {
        return false;
    };
    let Some((required_owner, required_type)) = expression_result_owner(program, *required) else {
        return false;
    };
    if required_owner != expected_required_owner || !expected_actual_owner.is_valid() {
        return false;
    }
    actual_contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        .flat_map(|contract| program.tables.proof_facts.span_or_empty(contract.facts))
        .any(|fact| {
            let ProofFact::Expression(expression) = fact else {
                return false;
            };
            let mut prefix = *expression;
            while let ExpressionNode::Binary(binary) = expression_table(program).expression(prefix)
            {
                if binary.operator != BinaryOperator::And {
                    break;
                }
                prefix = binary.left;
                let Some((actual_owner, actual_type)) = expression_result_owner(program, prefix)
                else {
                    continue;
                };
                if actual_owner != expected_actual_owner {
                    continue;
                }
                if !required_type.is_valid() || !actual_type.is_valid() {
                    continue;
                }
                let required_identity = program.normalized_type_identity(required_type);
                let actual_identity = program.normalized_type_identity(actual_type);
                if !required_identity.is_valid()
                    || !actual_identity.is_valid()
                    || required_identity != actual_identity
                {
                    continue;
                }
                if crate::has_builtin_parameter_bound_expression_meaning(
                    program,
                    required_owner,
                    required_parameters,
                    *required,
                ) && crate::has_builtin_parameter_bound_expression_meaning(
                    program,
                    actual_owner,
                    actual_parameters,
                    prefix,
                ) && same_guarantee_expression(
                    program,
                    *required,
                    prefix,
                    required_owner,
                    actual_owner,
                    required_parameters,
                    actual_parameters,
                ) {
                    return true;
                }
            }
            false
        })
}

fn expression_result_owner(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(
    symbols::SymbolHandle,
    typed_trees::types::TypeReferenceHandle,
)> {
    let mut nodes = Vec::new();
    crate::expression_traversal::collect_expression_tree(program, expression, &mut nodes);
    let mut owner = None;
    for node in nodes {
        if let Some(candidate) = crate::reserved_result_owner(program, node) {
            if owner.is_some_and(|existing| existing != candidate) {
                return None;
            }
            owner = Some(candidate);
        }
    }
    owner
}

fn same_guarantee_expression(
    program: &dyn crate::CheckingInput,
    required: ExpressionHandle,
    actual: ExpressionHandle,
    required_owner: symbols::SymbolHandle,
    actual_owner: symbols::SymbolHandle,
    required_parameters: &[StateParameter],
    actual_parameters: &[StateParameter],
) -> bool {
    match (
        expression_table(program).expression(required),
        expression_table(program).expression(actual),
    ) {
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            if crate::reserved_result_owner(program, required)
                .is_some_and(|(owner, _)| owner == required_owner)
                && crate::reserved_result_owner(program, actual)
                    .is_some_and(|(owner, _)| owner == actual_owner)
            {
                return true;
            }
            let table = expression_table(program);
            let left_members = table.name_path_members(left.members);
            let right_members = table.name_path_members(right.members);
            let left_symbols = table.name_path_member_symbols(left.member_symbols);
            let right_symbols = table.name_path_member_symbols(right.member_symbols);
            if left_members.len() != 1
                || right_members.len() != 1
                || left_symbols.len() != 1
                || right_symbols.len() != 1
                || !left.symbol.is_valid()
                || !right.symbol.is_valid()
                || left.head_symbol != left.symbol
                || right.head_symbol != right.symbol
                || left_symbols[0] != left.symbol
                || right_symbols[0] != right.symbol
            {
                return false;
            }
            let left_index = required_parameters
                .iter()
                .position(|p| p.symbol == left.symbol);
            let right_index = actual_parameters
                .iter()
                .position(|p| p.symbol == right.symbol);
            match (left_index, right_index) {
                (Some(left), Some(right)) => left == right,
                (None, None) => left.symbol == right.symbol,
                _ => false,
            }
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && same_guarantee_expression(
                    program,
                    left.left,
                    right.left,
                    required_owner,
                    actual_owner,
                    required_parameters,
                    actual_parameters,
                )
                && same_guarantee_expression(
                    program,
                    left.right,
                    right.right,
                    required_owner,
                    actual_owner,
                    required_parameters,
                    actual_parameters,
                )
        }
        _ => false,
    }
}

/// Replace type-argument spellings with the exact actual's display. Only a
/// binder named by `substitutions` changes, and only in `<...>` position, so
/// a value parameter with the same spelling stays a value.
fn substitute_type_actuals(
    program: &dyn crate::CheckingInput,
    text: &str,
    substitutions: &[(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
    )],
) -> String {
    if substitutions.is_empty() {
        return text.to_owned();
    }
    let mut names = Vec::new();
    for (symbol, actual) in substitutions {
        let Some(parameter) = program.data_type_parameter_by_symbol(*symbol) else {
            continue;
        };
        if !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type) {
            continue;
        }
        let rendered = program.display_type_reference(*actual);
        if rendered.is_empty()
            || names
                .iter()
                .any(|(name, _)| name == parameter.name.as_str())
        {
            continue;
        }
        names.push((parameter.name.as_str().to_owned(), rendered));
    }
    if names.is_empty() {
        return text.to_owned();
    }
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
        if matches!(previous, Some('<' | ','))
            && let Some((_, rendered)) = names.iter().find(|(name, _)| name == &word)
        {
            output.push_str(rendered);
        } else {
            output.push_str(&word);
        }
    }
    output
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

pub(crate) fn contract_kind_name(kind: &SignatureContractKind) -> &'static str {
    match kind {
        SignatureContractKind::Requires => "requires",
        SignatureContractKind::Ensures => "ensures",
        SignatureContractKind::EnsuresForResultCase { .. } => "outcome-specific ensures",
        SignatureContractKind::Crashes { .. } => "crashes",
    }
}
