use omega_core::byte_predicates::ByteSequencePredicate;
use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

pub(super) fn assign_domain_fact_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let domain_symbols = program
        .domain_definitions
        .iter()
        .map(|domain| (domain.name.as_str().to_owned(), domain.symbol))
        .collect::<Vec<_>>();
    let mut domain_fact_spans = program
        .domain_definitions
        .iter()
        .map(|domain| domain.facts)
        .collect::<Vec<_>>();
    domain_fact_spans.extend(
        program
            .traits
            .iter()
            .map(|trait_definition| trait_definition.invariants),
    );
    domain_fact_spans.extend(
        program
            .tables
            .declarations
            .signature_contracts
            .iter()
            .map(|(_, contract)| contract.facts),
    );
    domain_fact_spans.extend(
        program
            .data_definitions
            .iter()
            .map(|definition| definition.where_facts),
    );
    let domain_path_members = &program.tables.declarations.domain_path_members;
    let proof_facts = &mut program.tables.declarations.proof_facts;

    for facts in domain_fact_spans {
        for fact in proof_facts.span_mut_or_empty(facts) {
            match fact {
                omega_symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                    let name = domain_path_members
                        .span_or_empty(membership.domain)
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    membership.domain_symbol =
                        resolve_domain_symbol(symbols, &domain_symbols, &name);
                }
                omega_symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                    assign_proof_expression_membership_symbols(
                        symbols,
                        &domain_symbols,
                        &mut program.tables.bodies.expressions,
                        *expression,
                    );
                }
            }
        }
    }

    update_data_membership_zero_gates(program);
}

/// Membership facts begin conservatively zero-gated during declaration
/// lowering because their domain symbols do not exist yet. Once all symbols
/// are assigned, clear that conservative contribution exactly when every
/// membership names a recognized byte-predicate fact that accepts the empty byte
/// sequence. Expression facts have already contributed their independent
/// zero result, so this pass only ever preserves or clears membership gating.
fn update_data_membership_zero_gates(program: &mut SymbolResolvedTrees) {
    let membership_results = program
        .data_definitions
        .iter()
        .map(|definition| {
            let facts = program
                .tables
                .declarations
                .proof_facts
                .span_or_empty(definition.where_facts);
            let mut saw_membership = false;
            let mut memberships_admit_zero = true;
            for fact in facts {
                let omega_symbol_resolved_trees::domain::ProofFact::Membership(membership) = fact
                else {
                    continue;
                };
                saw_membership = true;
                memberships_admit_zero &=
                    resolved_domain_byte_predicate(program, membership.domain_symbol)
                        .is_some_and(|predicate| predicate.holds_for(&[]));
            }
            let expressions_gate_zero = facts
                .iter()
                .filter_map(|fact| match fact {
                    omega_symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                        Some(*expression)
                    }
                    omega_symbol_resolved_trees::domain::ProofFact::Membership(_) => None,
                })
                .any(|expression| {
                    crate::data::zero_fold(&program.tables.bodies.expressions, expression)
                        .is_none_or(|value| value == 0)
                });
            (
                saw_membership,
                memberships_admit_zero,
                expressions_gate_zero,
            )
        })
        .collect::<Vec<_>>();

    let mut membership_results = membership_results.into_iter();
    program.data_definitions.for_each_mut(|definition| {
        let (saw_membership, memberships_admit_zero, expressions_gate_zero) = membership_results
            .next()
            .expect("one zero result per data definition");
        definition.zero_gated =
            expressions_gate_zero || (saw_membership && !memberships_admit_zero);
    });
}

fn resolved_domain_byte_predicate(
    program: &SymbolResolvedTrees,
    domain_symbol: SymbolHandle,
) -> Option<ByteSequencePredicate> {
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.symbol == domain_symbol)?;
    let [omega_symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(domain.facts)
    else {
        return None;
    };
    let omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        return None;
    };
    if call.receiver.is_valid() {
        return None;
    }
    let predicate = ByteSequencePredicate::from_name(call.target.as_str())?;
    let [argument] = program
        .tables
        .bodies
        .expressions
        .expression_handles(call.arguments)
    else {
        return None;
    };
    let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
        program.tables.bodies.expressions.expression(*argument)
    else {
        return None;
    };
    let members = program
        .tables
        .bodies
        .expressions
        .name_path_members(path.members);
    matches!(members, [member] if member.as_str() == "self").then_some(predicate)
}

fn assign_proof_expression_membership_symbols(
    symbols: &SymbolTable,
    domain_symbols: &[(String, SymbolHandle)],
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let expression_node = expression_table.expression(expression).clone();
    match expression_node {
        omega_symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            for value in expression_table.expression_handles(values).to_vec() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    value,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_proof_expression_membership_symbols(
                symbols,
                domain_symbols,
                expression_table,
                binary.left,
            );
            assign_proof_expression_membership_symbols(
                symbols,
                domain_symbols,
                expression_table,
                binary.right,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    call.receiver,
                );
            }
            for argument in expression_table.expression_handles(call.arguments).to_vec() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    argument,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_proof_expression_membership_symbols(
                symbols,
                domain_symbols,
                expression_table,
                indexed.collection,
            );
            assign_proof_expression_membership_symbols(
                symbols,
                domain_symbols,
                expression_table,
                indexed.index,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    range.start,
                );
            }
            if range.end.is_valid() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    range.end,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_proof_expression_membership_symbols(
                symbols,
                domain_symbols,
                expression_table,
                membership.value,
            );
            let name = expression_table
                .name_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
                expression_table.expression_mut(expression)
            {
                membership.domain_symbol = resolve_domain_symbol(symbols, domain_symbols, &name);
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            assign_proof_expression_membership_symbols(
                symbols,
                domain_symbols,
                expression_table,
                member.receiver,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            assign_proof_expression_membership_symbols(
                symbols,
                domain_symbols,
                expression_table,
                inner,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            assign_proof_expression_membership_symbols(
                symbols,
                domain_symbols,
                expression_table,
                unary.operand,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            for field in expression_table
                .struct_fields(struct_literal.fields)
                .to_vec()
            {
                assign_proof_expression_membership_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    field.value,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Cast(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Name(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::String(_) => {}
    }
}

fn resolve_domain_symbol(
    symbols: &SymbolTable,
    domain_symbols: &[(String, SymbolHandle)],
    name: &str,
) -> SymbolHandle {
    if name.contains("::") {
        return domain_symbols
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, symbol)| *symbol)
            .or_else(|| {
                symbols.find_child_by_name_and_kind(symbols.root(), name, SymbolKind::Domain)
            })
            .unwrap_or_else(SymbolHandle::invalid);
    }

    let mut matches = domain_symbols
        .iter()
        .filter(|(candidate, _)| candidate.rsplit("::").next().unwrap_or(candidate) == name);
    let Some((_, symbol)) = matches.next() else {
        return SymbolHandle::invalid();
    };
    if matches.next().is_some() {
        SymbolHandle::invalid()
    } else {
        *symbol
    }
}
