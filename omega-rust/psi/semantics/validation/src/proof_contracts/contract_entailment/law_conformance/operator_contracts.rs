//! Operator contract snapshots, conformance and crash contract refinement.

use crate::proof_contracts::contract_entailment::law_conformance::proposition_laws::collect_equality_conjuncts;
use crate::proof_contracts::contract_entailment::{
    BinaryOperator, Diagnostic, ExpressionHandle, ExpressionNode, Machine, ProofFact,
    RESULT_BINDER, SignatureContractKind, SymbolHandle, TypedTrees,
};

pub(crate) fn checked_operator_contract_snapshot(
    program: &TypedTrees,
    contracts: &[typed_trees::signature::SignatureContract],
) -> Vec<u8> {
    use std::collections::HashSet;
    use std::fmt::Debug;
    use typed_trees::expression::ExpressionNode;

    fn append_debug(value: &impl Debug, output: &mut Vec<u8>) {
        let bytes = format!("{value:?}").into_bytes();
        output.extend_from_slice(
            &u64::try_from(bytes.len())
                .expect("checked operator contract snapshot length fits u64")
                .to_le_bytes(),
        );
        output.extend(bytes);
    }

    fn append_expression(
        program: &TypedTrees,
        expression: ExpressionHandle,
        visited: &mut HashSet<(u32, u32)>,
        output: &mut Vec<u8>,
    ) {
        append_debug(&expression, output);
        if !expression.is_valid()
            || !visited.insert((expression.arena_index(), expression.generation()))
        {
            return;
        }
        let node = program.expression_table.expression(expression);
        append_debug(node, output);
        match node {
            ExpressionNode::Match(dispatch) => {
                append_debug(&program.expression_table.match_arms(dispatch.arms), output);
                for child in
                    crate::value_custody::expression_types::match_children(program, *dispatch)
                {
                    append_expression(program, child, visited, output);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                append_expression(program, atomic.value, visited, output);
                append_expression(program, atomic.result, visited, output);
            }
            ExpressionNode::Binary(binary) => {
                append_expression(program, binary.left, visited, output);
                append_expression(program, binary.right, visited, output);
            }
            ExpressionNode::Unary(unary) => {
                append_expression(program, unary.operand, visited, output);
            }
            ExpressionNode::Cast(cast) => {
                append_debug(
                    &program
                        .expression_table
                        .name_path_members(cast.target_label),
                    output,
                );
                append_debug(
                    &program
                        .expression_table
                        .name_path_members(cast.semantic_domain),
                    output,
                );
                append_debug(
                    &program
                        .package_qualified_type_identity(cast.target_type)
                        .into_string(),
                    output,
                );
                for argument in program
                    .type_reference_table
                    .type_reference_handles(cast.semantic_domain_arguments)
                {
                    append_debug(
                        &program
                            .package_qualified_type_identity(*argument)
                            .into_string(),
                        output,
                    );
                }
                append_expression(program, cast.value, visited, output);
            }
            ExpressionNode::Call(call) => {
                append_expression(program, call.receiver, visited, output);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    append_expression(program, *argument, visited, output);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                append_expression(program, indexed.collection, visited, output);
                append_expression(program, indexed.index, visited, output);
            }
            ExpressionNode::Member(member) => {
                append_expression(program, member.receiver, visited, output);
            }
            ExpressionNode::Borrow(inner) => {
                append_expression(program, inner.target, visited, output);
            }
            ExpressionNode::Name(path) => {
                append_debug(
                    &program.expression_table.name_path_members(path.members),
                    output,
                );
                append_debug(
                    &program
                        .expression_table
                        .name_path_member_symbols(path.member_symbols),
                    output,
                );
            }
            ExpressionNode::Range(range) => {
                append_expression(program, range.start, visited, output);
                append_expression(program, range.end, visited, output);
            }
            ExpressionNode::ArrayLiteral(items) => {
                for item in program.expression_table.expression_handles(*items) {
                    append_expression(program, *item, visited, output);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    append_debug(field, output);
                    append_expression(program, field.value, visited, output);
                }
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_) => {}
            ExpressionNode::ZeroValue(type_reference) => {
                append_debug(
                    &program
                        .package_qualified_type_identity(*type_reference)
                        .into_string(),
                    output,
                );
            }
        }
    }

    let mut output = Vec::new();
    let mut visited = HashSet::new();
    for contract in contracts {
        append_debug(contract, &mut output);
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            append_debug(fact, &mut output);
            match fact {
                ProofFact::Expression(expression) => {
                    append_expression(program, *expression, &mut visited, &mut output);
                }
                ProofFact::Membership(membership) => {
                    append_expression(program, membership.value, &mut visited, &mut output);
                    append_debug(&program.domain_path_members(membership.domain), &mut output);
                }
                ProofFact::Proposition(application) => {
                    for argument in program
                        .expression_table
                        .expression_handles(application.arguments)
                    {
                        append_expression(program, *argument, &mut visited, &mut output);
                    }
                }
            }
        }
    }
    output
}

/// A checked Omega body satisfying an ordinary or boundary operator is a
/// software provider, not an accepted leaf. Its own machine contract is proved
/// by the ordinary entailment pass above; this gate then checks that the proved
/// contract covers the selected operator contract.
///
/// The first checked-software rung deliberately admits the contract language
/// that is already load-bearing for boundary operators: equality facts and
/// exact `&&` conjunctions. Provider `requires` may only repeat requirement
/// premises (asking less is valid); every required operator `ensures` conjunct
/// must appear in the provider's proved ensures. Operator parameters are
/// substituted positionally onto provider parameters, and the reserved
/// `result` binder maps only to itself, so renaming is harmless while swapping
/// two parameter roles is not.
pub(crate) fn check_operator_contract_conformance(
    program: &TypedTrees,
    machine: &Machine,
    operator: &typed_trees::operator::OperatorDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry_state) = program.machine_states(machine).first() else {
        return; // exact signature validation already reports the missing entry
    };
    let operator_identity =
        typed_trees::operator::boundary_operator_requirement_identity(program, operator);
    let mut requirement_requires = Vec::new();
    let mut requirement_ensures = Vec::new();
    let mut provider_requires = Vec::new();
    let mut provider_ensures = Vec::new();
    let mut unsupported_requirement = false;
    let mut unsupported_provider_requires = false;

    let collect = |contracts: &[typed_trees::signature::SignatureContract],
                   requires: &mut Vec<ExpressionHandle>,
                   ensures: &mut Vec<ExpressionHandle>,
                   unsupported_requires: &mut bool,
                   unsupported_ensures: &mut bool| {
        for contract in contracts {
            let destination = match &contract.kind {
                SignatureContractKind::Requires => &mut *requires,
                SignatureContractKind::Ensures => &mut *ensures,
                SignatureContractKind::EnsuresForResultCase { .. }
                | SignatureContractKind::Crashes { .. } => continue,
            };
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(expression) = fact else {
                    match &contract.kind {
                        SignatureContractKind::Requires => *unsupported_requires = true,
                        SignatureContractKind::Ensures => *unsupported_ensures = true,
                        SignatureContractKind::EnsuresForResultCase { .. }
                        | SignatureContractKind::Crashes { .. } => {}
                    }
                    continue;
                };
                if !is_equality_conjunction(program, *expression) {
                    match &contract.kind {
                        SignatureContractKind::Requires => *unsupported_requires = true,
                        SignatureContractKind::Ensures => *unsupported_ensures = true,
                        SignatureContractKind::EnsuresForResultCase { .. }
                        | SignatureContractKind::Crashes { .. } => {}
                    }
                }
                collect_equality_conjuncts(program, *expression, destination);
            }
        }
    };
    let mut _unsupported_requirement_requires = false;
    collect(
        program.operator_contracts(operator),
        &mut requirement_requires,
        &mut requirement_ensures,
        &mut _unsupported_requirement_requires,
        &mut unsupported_requirement,
    );
    let mut _unsupported_provider_ensures = false;
    collect(
        program.machine_contracts(machine),
        &mut provider_requires,
        &mut provider_ensures,
        &mut unsupported_provider_requires,
        &mut _unsupported_provider_ensures,
    );

    if unsupported_requirement {
        diagnostics.push(Diagnostic::error(format!(
            "checked machine `{}` satisfies operator `{operator_identity}`, whose ensures contract is outside checked operator-contract entailment's equality/`&&` rung",
            machine.name,
        )));
        return;
    }
    if unsupported_provider_requires {
        diagnostics.push(Diagnostic::error(format!(
            "checked operator provider `{}` adds a non-equality requires fact while satisfying `{operator_identity}`; checked providers may not ask more than the operator requirement",
            machine.name,
        )));
        return;
    }

    let mut name_map: Vec<(SymbolHandle, String, SymbolHandle, String)> = program
        .operator_parameters(operator)
        .iter()
        .zip(program.state_parameters(entry_state))
        .map(|(requirement, provider)| {
            (
                requirement.symbol,
                requirement.name.as_str().to_owned(),
                provider.symbol,
                provider.name.as_str().to_owned(),
            )
        })
        .collect();
    name_map.push((
        SymbolHandle::invalid(),
        RESULT_BINDER.to_owned(),
        SymbolHandle::invalid(),
        RESULT_BINDER.to_owned(),
    ));

    let matches = |requirement_fact: ExpressionHandle, provider_fact: ExpressionHandle| {
        let ExpressionNode::Binary(requirement) =
            program.expression_table.expression(requirement_fact)
        else {
            return false;
        };
        let ExpressionNode::Binary(provider) = program.expression_table.expression(provider_fact)
        else {
            return false;
        };
        [
            (provider.left, provider.right),
            (provider.right, provider.left),
        ]
        .into_iter()
        .any(|(left, right)| {
            operator_contract_expressions_match(program, requirement.left, left, &name_map)
                && operator_contract_expressions_match(program, requirement.right, right, &name_map)
        })
    };

    for provider_requires_fact in &provider_requires {
        if !requirement_requires
            .iter()
            .any(|required| matches(*required, *provider_requires_fact))
        {
            diagnostics.push(Diagnostic::error(format!(
                "checked operator provider `{}` requires `{}`, which operator requirement `{operator_identity}` does not require",
                machine.name,
                program.expression_table.display_name(*provider_requires_fact),
            )));
        }
    }
    for requirement_ensures_fact in &requirement_ensures {
        if !provider_ensures
            .iter()
            .any(|provided| matches(*requirement_ensures_fact, *provided))
        {
            diagnostics.push(Diagnostic::error(format!(
                "checked operator provider `{}` proves no ensures matching operator requirement `{operator_identity}` contract `{}`",
                machine.name,
                program.expression_table.display_name(*requirement_ensures_fact),
            )));
        }
    }
    check_operator_crash_contract_refinement(
        program,
        machine,
        operator,
        &operator_identity,
        &name_map,
        diagnostics,
    );
}

fn check_operator_crash_contract_refinement(
    program: &TypedTrees,
    machine: &Machine,
    operator: &typed_trees::operator::OperatorDefinition,
    operator_identity: &str,
    name_map: &[(SymbolHandle, String, SymbolHandle, String)],
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
                            operator_contract_expressions_match(
                                program, *required, *provided, name_map,
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
    program: &TypedTrees,
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
            let ProofFact::Expression(expression) = fact else {
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

fn operator_contract_expressions_match(
    program: &TypedTrees,
    requirement: ExpressionHandle,
    provider: ExpressionHandle,
    name_map: &[(SymbolHandle, String, SymbolHandle, String)],
) -> bool {
    if !requirement.is_valid() || !provider.is_valid() {
        return requirement.is_valid() == provider.is_valid();
    }
    let table = &program.expression_table;
    match (table.expression(requirement), table.expression(provider)) {
        (ExpressionNode::Name(required), ExpressionNode::Name(provided)) => {
            let required_members = table.name_path_members(required.members);
            let provided_members = table.name_path_members(provided.members);
            if let [required_name] = required_members
                && let Some((_, _, provided_symbol, provided_name)) =
                    name_map
                        .iter()
                        .find(|(candidate_symbol, candidate_name, _, _)| {
                            (required.symbol.is_valid()
                                && candidate_symbol.is_valid()
                                && required.symbol == *candidate_symbol)
                                || candidate_name == required_name.as_str()
                        })
            {
                return matches!(provided_members, [actual]
                    if (provided_symbol.is_valid()
                        && provided.symbol.is_valid()
                        && provided.symbol == *provided_symbol)
                        || actual.as_str() == provided_name);
            }
            table.expressions_structurally_equal(requirement, provider)
        }
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::Borrow(left), ExpressionNode::Borrow(right)) => {
            left.access == right.access
                && operator_contract_expressions_match(program, left.target, right.target, name_map)
        }
        (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
            left.operator == right.operator
                && operator_contract_expressions_match(
                    program,
                    left.operand,
                    right.operand,
                    name_map,
                )
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && operator_contract_expressions_match(program, left.left, right.left, name_map)
                && operator_contract_expressions_match(program, left.right, right.right, name_map)
        }
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            operator_contract_expressions_match(
                program,
                left.collection,
                right.collection,
                name_map,
            ) && operator_contract_expressions_match(program, left.index, right.index, name_map)
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member.as_str() == right.member.as_str()
                && left.case_variant == right.case_variant
                && operator_contract_expressions_match(
                    program,
                    left.receiver,
                    right.receiver,
                    name_map,
                )
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            let target_matches = if left.target_symbol.is_valid() && right.target_symbol.is_valid()
            {
                left.target_symbol == right.target_symbol
            } else {
                left.target.as_str() == right.target.as_str()
            };
            let left_arguments = table.expression_handles(left.arguments);
            let right_arguments = table.expression_handles(right.arguments);
            target_matches
                && left.machine_arguments == right.machine_arguments
                && left.operational_acknowledgement == right.operational_acknowledgement
                && operator_contract_expressions_match(
                    program,
                    left.receiver,
                    right.receiver,
                    name_map,
                )
                && left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| {
                        operator_contract_expressions_match(program, *left, *right, name_map)
                    })
        }
        (ExpressionNode::ArrayLiteral(left), ExpressionNode::ArrayLiteral(right)) => {
            let left = table.expression_handles(*left);
            let right = table.expression_handles(*right);
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    operator_contract_expressions_match(program, *left, *right, name_map)
                })
        }
        (ExpressionNode::Range(left), ExpressionNode::Range(right)) => {
            left.end_inclusive == right.end_inclusive
                && operator_contract_expressions_match(program, left.start, right.start, name_map)
                && operator_contract_expressions_match(program, left.end, right.end, name_map)
        }
        (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
            let left_fields = table.struct_fields(left.fields);
            let right_fields = table.struct_fields(right.fields);
            left.type_name.as_str() == right.type_name.as_str()
                && left.case_name.as_ref().map(|name| name.as_str())
                    == right.case_name.as_ref().map(|name| name.as_str())
                && left_fields.len() == right_fields.len()
                && left_fields.iter().zip(right_fields).all(|(left, right)| {
                    left.name.as_str() == right.name.as_str()
                        && operator_contract_expressions_match(
                            program,
                            left.value,
                            right.value,
                            name_map,
                        )
                })
        }
        (ExpressionNode::Cast(left), ExpressionNode::Cast(right)) => {
            left.target_type == right.target_type
                && table.name_path_members(left.target_label)
                    == table.name_path_members(right.target_label)
                && left.domain == right.domain
                && table.name_path_members(left.semantic_domain)
                    == table.name_path_members(right.semantic_domain)
                && left.semantic_domain_arguments == right.semantic_domain_arguments
                && left.semantic_domain_symbol == right.semantic_domain_symbol
                && left.semantic_domain_id == right.semantic_domain_id
                && left.form == right.form
                && operator_contract_expressions_match(program, left.value, right.value, name_map)
        }
        (ExpressionNode::Atomic(left), ExpressionNode::Atomic(right)) => {
            left.ordering == right.ordering
                && operator_contract_expressions_match(program, left.value, right.value, name_map)
                && operator_contract_expressions_match(program, left.result, right.result, name_map)
        }
        (ExpressionNode::ZeroValue(left), ExpressionNode::ZeroValue(right)) => left == right,
        _ => false,
    }
}

pub(crate) fn is_equality_conjunction(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    match binary.operator {
        BinaryOperator::Equal => true,
        BinaryOperator::And => {
            is_equality_conjunction(program, binary.left)
                && is_equality_conjunction(program, binary.right)
        }
        _ => false,
    }
}
