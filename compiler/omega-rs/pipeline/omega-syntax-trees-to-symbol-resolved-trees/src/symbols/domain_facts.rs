use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

pub(super) fn assign_domain_fact_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let domain_symbols = program
        .domain_definitions
        .iter()
        .map(|domain| (domain.name.as_str().to_owned(), domain.symbol))
        .collect::<Vec<_>>();
    let domain_classifiers = program
        .domain_definitions
        .iter()
        .map(|domain| domain.classifier)
        .collect::<Vec<_>>();
    let mut domain_fact_spans = program
        .domain_definitions
        .iter()
        .map(|domain| (domain.facts, None))
        .collect::<Vec<_>>();
    domain_fact_spans.extend(
        program
            .data_definitions
            .iter()
            .map(|data_definition| (data_definition.default_domain, Some(data_definition.symbol))),
    );
    domain_fact_spans.extend(
        program
            .traits
            .iter()
            .map(|trait_definition| (trait_definition.invariants, None)),
    );
    domain_fact_spans.extend(
        program
            .tables
            .declarations
            .signature_contracts
            .iter()
            .map(|(_, contract)| (contract.facts, None)),
    );
    let domain_path_members = &program.tables.declarations.domain_path_members;
    let proof_facts = &mut program.tables.declarations.proof_facts;

    for classifier in domain_classifiers {
        if classifier.is_valid() {
            assign_proof_expression_membership_symbols(
                symbols,
                &mut program.tables.bodies.expressions,
                classifier,
                None,
            );
        }
    }

    for (facts, field_scope) in domain_fact_spans {
        for fact in proof_facts.span_mut_or_empty(facts) {
            match fact {
                omega_symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                    assign_proof_expression_membership_symbols(
                        symbols,
                        &mut program.tables.bodies.expressions,
                        membership.value,
                        field_scope,
                    );
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
                        &mut program.tables.bodies.expressions,
                        *expression,
                        field_scope,
                    );
                }
            }
        }
    }
}

fn resolve_domain_symbol(
    symbols: &SymbolTable,
    domain_symbols: &[(String, SymbolHandle)],
    name: &str,
) -> SymbolHandle {
    if let Some(symbol) =
        symbols.find_child_by_name_and_kind(symbols.root(), name, SymbolKind::Domain)
    {
        return symbol;
    }
    let mut matches = domain_symbols.iter().filter_map(|(full_name, symbol)| {
        full_name
            .rsplit("::")
            .next()
            .is_some_and(|short_name| short_name == name)
            .then_some(*symbol)
    });
    let Some(symbol) = matches.next() else {
        return SymbolHandle::invalid();
    };
    if matches.next().is_some() {
        SymbolHandle::invalid()
    } else {
        symbol
    }
}

fn assign_proof_expression_membership_symbols(
    symbols: &SymbolTable,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
    field_scope: Option<SymbolHandle>,
) {
    let expression_node = expression_table.expression(expression).clone();
    match expression_node {
        omega_symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            for value in expression_table.expression_handles(values).to_vec() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    expression_table,
                    value,
                    field_scope,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_proof_expression_membership_symbols(
                symbols,
                expression_table,
                binary.left,
                field_scope,
            );
            assign_proof_expression_membership_symbols(
                symbols,
                expression_table,
                binary.right,
                field_scope,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    expression_table,
                    call.receiver,
                    field_scope,
                );
            }
            for argument in expression_table.expression_handles(call.arguments).to_vec() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    expression_table,
                    argument,
                    field_scope,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_proof_expression_membership_symbols(
                symbols,
                expression_table,
                indexed.collection,
                field_scope,
            );
            assign_proof_expression_membership_symbols(
                symbols,
                expression_table,
                indexed.index,
                field_scope,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    expression_table,
                    range.start,
                    field_scope,
                );
            }
            if range.end.is_valid() {
                assign_proof_expression_membership_symbols(
                    symbols,
                    expression_table,
                    range.end,
                    field_scope,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_proof_expression_membership_symbols(
                symbols,
                expression_table,
                membership.value,
                field_scope,
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
                membership.domain_symbol = symbols
                    .find_child_by_name_and_kind(symbols.root(), &name, SymbolKind::Domain)
                    .unwrap_or_else(SymbolHandle::invalid);
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            assign_proof_expression_membership_symbols(
                symbols,
                expression_table,
                member.receiver,
                field_scope,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            assign_proof_expression_membership_symbols(
                symbols,
                expression_table,
                inner,
                field_scope,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            assign_proof_expression_membership_symbols(
                symbols,
                expression_table,
                unary.operand,
                field_scope,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            for field in expression_table
                .struct_fields(struct_literal.fields)
                .to_vec()
            {
                assign_proof_expression_membership_symbols(
                    symbols,
                    expression_table,
                    field.value,
                    field_scope,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let Some(scope) = field_scope else {
                return;
            };
            let members = expression_table.name_path_members(path.members);
            if members.len() != 1 {
                return;
            }
            let symbol = symbols
                .find_child_by_name_and_kind(scope, members[0].as_str(), SymbolKind::Field)
                .unwrap_or_else(SymbolHandle::invalid);
            if symbol.is_valid()
                && let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                    expression_table.expression_mut(expression)
            {
                path.head_symbol = symbol;
                path.symbol = symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Cast(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::String(_) => {}
    }
}
