use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

pub(super) fn assign_domain_fact_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let mut domain_fact_spans = program
        .domain_definitions
        .iter()
        .map(|domain| domain.facts)
        .collect::<Vec<_>>();
    domain_fact_spans.extend(
        program
            .tables
            .declarations
            .signature_contracts
            .iter()
            .map(|(_, contract)| contract.facts),
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
                    membership.domain_symbol = symbols
                        .find_child_by_name_and_kind(symbols.root(), &name, SymbolKind::Domain)
                        .unwrap_or_else(SymbolHandle::invalid);
                }
                omega_symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                    assign_proof_expression_membership_symbols(
                        symbols,
                        &mut program.tables.bodies.expressions,
                        *expression,
                    );
                }
            }
        }
    }
}

fn assign_proof_expression_membership_symbols(
    symbols: &SymbolTable,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let expression_node = expression_table.expression(expression).clone();
    match expression_node {
        omega_symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            for value in expression_table.expression_handles(values).to_vec() {
                assign_proof_expression_membership_symbols(symbols, expression_table, value);
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_proof_expression_membership_symbols(symbols, expression_table, binary.left);
            assign_proof_expression_membership_symbols(symbols, expression_table, binary.right);
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    expression_table,
                    call.receiver,
                );
            }
            for argument in expression_table.expression_handles(call.arguments).to_vec() {
                assign_proof_expression_membership_symbols(symbols, expression_table, argument);
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_proof_expression_membership_symbols(
                symbols,
                expression_table,
                indexed.collection,
            );
            assign_proof_expression_membership_symbols(symbols, expression_table, indexed.index);
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_proof_expression_membership_symbols(symbols, expression_table, range.start);
            }
            if range.end.is_valid() {
                assign_proof_expression_membership_symbols(symbols, expression_table, range.end);
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_proof_expression_membership_symbols(symbols, expression_table, membership.value);
            let name = expression_table
                .name_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
                expression_table.expression_mut(expression)
            {
                membership.domain_symbol = symbols
                    .find_child_by_name_and_kind(symbols.root(), &name, SymbolKind::Domain)
                    .unwrap_or_else(SymbolHandle::invalid);
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            assign_proof_expression_membership_symbols(symbols, expression_table, member.receiver);
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            assign_proof_expression_membership_symbols(symbols, expression_table, inner);
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            assign_proof_expression_membership_symbols(symbols, expression_table, unary.operand);
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            for field in expression_table
                .struct_fields(struct_literal.fields)
                .to_vec()
            {
                assign_proof_expression_membership_symbols(symbols, expression_table, field.value);
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
