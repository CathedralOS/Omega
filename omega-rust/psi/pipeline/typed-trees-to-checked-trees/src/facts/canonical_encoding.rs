//! Canonical encodings of contract sets, facts, types and expressions.

use crate::facts::crash_plan_facts::{encode_signature_contract_kind, is_true_crash_route};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::proposition::PropositionLabels;

/// Encode contracts as semantic sets. Crash clauses are first merged by cause:
/// their facts are alternative routes, duplicate routes are irrelevant, and
/// one unconditional clause subsumes every guarded route in the same bucket.
/// This keeps public contract identity independent of clause grouping while
/// preserving the bucket itself as identity-bearing material.
pub(crate) fn encode_contract_set_canonical(
    program: &TypedTrees,
    contracts: &[typed_trees::signature::SignatureContract],
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    entry_prefix: &[u8],
    canonicalize_membership_value: bool,
    include_crashes: bool,
) -> Vec<Vec<u8>> {
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct CrashBucket {
        unconditional: bool,
        routes: Vec<Vec<u8>>,
    }

    let mut encoded = Vec::new();
    let mut crash_buckets = BTreeMap::<Vec<u8>, CrashBucket>::new();
    for contract in contracts {
        let mut contract_prefix = entry_prefix.to_vec();
        encode_signature_contract_kind(&contract.kind, &mut contract_prefix);
        let facts = program.proof_facts.span_or_empty(contract.facts);
        let is_crash = matches!(
            contract.kind,
            typed_trees::signature::SignatureContractKind::Crashes { .. }
        );
        if is_crash && !include_crashes {
            continue;
        }
        if is_crash {
            let bucket = crash_buckets.entry(contract_prefix).or_default();
            if facts.is_empty() || facts.iter().any(|fact| is_true_crash_route(program, fact)) {
                bucket.unconditional = true;
            } else {
                for fact in facts {
                    let mut route = Vec::new();
                    encode_contract_fact_canonical(
                        program,
                        fact,
                        parameter_names,
                        content_conservation,
                        canonicalize_membership_value,
                        &mut route,
                    );
                    bucket.routes.push(route);
                }
            }
            continue;
        }

        for fact in facts {
            let mut contract_bytes = contract_prefix.clone();
            encode_contract_fact_canonical(
                program,
                fact,
                parameter_names,
                content_conservation,
                canonicalize_membership_value,
                &mut contract_bytes,
            );
            encoded.push(contract_bytes);
        }
    }

    for (contract_prefix, mut bucket) in crash_buckets {
        if bucket.unconditional {
            let mut contract = contract_prefix;
            contract.push(0);
            encoded.push(contract);
            continue;
        }
        bucket.routes.sort();
        bucket.routes.dedup();
        for route in bucket.routes {
            let mut contract = contract_prefix.clone();
            contract.push(1);
            contract.extend(route);
            encoded.push(contract);
        }
    }
    encoded.sort();
    encoded
}

pub(crate) fn encode_contract_fact_canonical(
    program: &TypedTrees,
    fact: &typed_trees::domain::ProofFact,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    canonicalize_membership_value: bool,
    output: &mut Vec<u8>,
) {
    match fact {
        typed_trees::domain::ProofFact::Expression(expression) => {
            output.push(1);
            encode_contract_expression_canonical(
                program,
                *expression,
                parameter_names,
                content_conservation,
                output,
            );
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            output.push(2);
            if canonicalize_membership_value {
                encode_expression_canonical(program, membership.value, parameter_names, output);
            } else {
                output.extend(
                    program
                        .expression_table
                        .display_name(membership.value)
                        .as_bytes(),
                );
            }
            output.push(0);
            let domain_path = if canonicalize_membership_value {
                program.domain_path_members(membership.domain)
            } else {
                program
                    .expression_table
                    .name_path_members(membership.domain)
            };
            for member in domain_path {
                output.extend(member.as_str().as_bytes());
                output.push(b':');
            }
            if !membership.domain_arguments.is_empty() {
                // Source-instance contents are stable across interner order;
                // SemanticDomainId itself is only a program-local lookup key.
                output.push(b'<');
                output.extend_from_slice(&(membership.domain_arguments.len() as u64).to_le_bytes());
                for argument in program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                {
                    let identity = program.normalized_type_identity(*argument);
                    output.extend_from_slice(&(identity.as_str().len() as u64).to_le_bytes());
                    output.extend_from_slice(identity.as_str().as_bytes());
                }
                output.push(b'>');
            }
        }
        typed_trees::domain::ProofFact::Proposition(application) => {
            output.push(3);
            encode_proposition_application_canonical(program, application, parameter_names, output);
        }
    }
}

pub(crate) fn encode_type_spelling(text: &str, binders: &[(String, String)], output: &mut Vec<u8>) {
    let mut word = String::new();
    let flush = |word: &mut String, output: &mut Vec<u8>| {
        if word.is_empty() {
            return;
        }
        if let Some((_, replacement)) = binders.iter().find(|(name, _)| name == word) {
            output.extend(replacement.as_bytes());
        } else {
            output.extend(word.as_bytes());
        }
        word.clear();
    };
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            word.push(character);
        } else {
            flush(&mut word, output);
            output.extend(character.to_string().as_bytes());
        }
    }
    flush(&mut word, output);
    output.push(0);
}

/// The same declaration rule governs retained qualification uses and scalar
/// computation eligibility. Alias atoms must all be predicate- and route-free.
pub(crate) fn domain_is_vacuous(
    program: &TypedTrees,
    domain_symbol: SymbolHandle,
    stack: &mut Vec<SymbolHandle>,
) -> bool {
    if !domain_symbol.is_valid() || stack.contains(&domain_symbol) {
        return false;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|candidate| candidate.symbol == domain_symbol)
    else {
        return false;
    };
    if let Some(alias) = domain.alias.as_ref() {
        if alias.constituents.is_empty() {
            return false;
        }
        stack.push(domain_symbol);
        let vacuous = alias
            .constituents
            .iter()
            .all(|constituent| domain_is_vacuous(program, constituent.domain_symbol, stack));
        stack.pop();
        return vacuous;
    }
    !domain.predicate_body.is_present() && domain.establishment_routes.is_empty()
}

/// A stable, spelling-independent byte encoding of a contract fact
/// expression: prefix walk with operator tags, name paths as text, integer
/// literals as text (exact at any magnitude). Deterministic across
/// programs for the same declared clause.
fn encode_expression_canonical(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    out: &mut Vec<u8>,
) {
    use typed_trees::expression::ExpressionNode;
    if !expression.is_valid() {
        out.push(0);
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            out.push(1);
            out.push(binary.operator as u8);
            encode_expression_canonical(program, binary.left, parameter_names, out);
            encode_expression_canonical(program, binary.right, parameter_names, out);
        }
        ExpressionNode::Unary(unary) => {
            out.push(2);
            out.push(unary.operator as u8);
            encode_expression_canonical(program, unary.operand, parameter_names, out);
        }
        ExpressionNode::Integer(value) => {
            out.push(3);
            out.extend(value.text().as_bytes());
            out.push(0);
        }
        ExpressionNode::Boolean(value) => {
            out.push(4);
            out.push(u8::from(*value));
        }
        // Float literals carry their exact suffix-free spelling like
        // `Integer` — the same tag `CrashPredicateExpression::Float` writes,
        // so checked identities and canonical route bytes stay equal.
        ExpressionNode::Float(value) => {
            out.push(0x0a);
            out.extend(value.text().as_bytes());
            out.push(0);
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            // A bare parameter name normalizes to its POSITION -- renames
            // never change the contract identity.
            if let [single] = members
                && let Some(index) = parameter_names
                    .iter()
                    .position(|name| name == single.as_str())
            {
                out.push(9);
                out.extend(
                    u32::try_from(index)
                        .expect("parameter index fits u32")
                        .to_le_bytes(),
                );
                return;
            }
            out.push(5);
            for member in members {
                out.extend(member.as_str().as_bytes());
                out.push(b'.');
            }
            out.push(0);
        }
        ExpressionNode::Member(member) => {
            out.push(6);
            encode_expression_canonical(program, member.receiver, parameter_names, out);
            out.extend(member.member.as_str().as_bytes());
            out.push(0);
        }
        // A `collection[index]` read keeps both children explicit — the same
        // tag `CrashPredicateExpression::Indexed` writes, so checked
        // identities and canonical route bytes stay equal.
        ExpressionNode::Indexed(indexed) => {
            out.push(0x0b);
            encode_expression_canonical(program, indexed.collection, parameter_names, out);
            encode_expression_canonical(program, indexed.index, parameter_names, out);
        }
        ExpressionNode::Call(call) => {
            out.push(7);
            out.extend(call.target.as_str().as_bytes());
            out.push(0);
            encode_expression_canonical(program, call.receiver, parameter_names, out);
            for argument in program.expression_table.expression_handles(call.arguments) {
                encode_expression_canonical(program, *argument, parameter_names, out);
            }
            out.push(0xfe);
        }
        // Anything else falls back to the display name -- stable per
        // spelling (a conservative widening; refine per-node as shapes
        // arrive in contracts).
        other => {
            let _ = other;
            out.push(8);
            out.extend(program.expression_table.display_name(expression).as_bytes());
            out.push(0);
        }
    }
}

fn encode_proposition_application_canonical(
    program: &TypedTrees,
    application: &typed_trees::proposition::PropositionApplication,
    parameter_names: &[String],
    out: &mut Vec<u8>,
) {
    let binder_labels = application
        .binder_arguments
        .iter()
        .map(|binder| binder.display_name())
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            let mut bytes = Vec::new();
            encode_expression_canonical(program, *argument, parameter_names, &mut bytes);
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    if let Some(formula) = program.normalize_proposition_application(
        application,
        Some(PropositionLabels {
            binder_labels: &binder_labels,
            argument_labels: &argument_labels,
        }),
    ) {
        out.extend(formula.identity_label().as_bytes());
    } else {
        // Invalid/cyclic aliases are rejected by validation; retaining a
        // fail-closed marker prevents an accidental identity collision here.
        out.extend(b"invalid-proposition-application");
    }
    out.push(0);
}

pub(crate) fn encode_contract_expression_canonical(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    out: &mut Vec<u8>,
) {
    if let Some(conservation) = content_conservation
        .iter()
        .find(|candidate| candidate.source_expression == expression)
    {
        out.push(0xcc);
        out.extend(
            language_semantics::content::content_conservation_plan_bytes(&conservation.plan),
        );
        return;
    }
    encode_expression_canonical(program, expression, parameter_names, out);
}
