use psi_language_semantics::byte_predicates::ByteSequencePredicate;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::expressions::assign_membership_symbol;
use super::targets::resolve_free_machine_entry_state_symbol;

pub(super) fn assign_domain_fact_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let domain_symbols = program
        .domain_definitions
        .iter()
        .map(|domain| {
            (
                domain.name.as_str().to_owned(),
                domain.symbol,
                domain.semantic_id,
            )
        })
        .collect::<Vec<_>>();
    let alias_symbols = program
        .domain_definitions
        .iter()
        .map(|domain| {
            domain
                .alias
                .as_ref()
                .map(|alias| {
                    alias
                        .constituents
                        .iter()
                        .map(|constituent| {
                            let name = program
                                .domain_path_members(constituent.domain)
                                .iter()
                                .map(|member| member.as_str())
                                .collect::<Vec<_>>()
                                .join("::");
                            resolve_domain_symbol(symbols, &domain_symbols, &name)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let mut alias_symbols = alias_symbols.into_iter();
    program.domain_definitions.for_each_mut(|domain| {
        let resolved = alias_symbols
            .next()
            .expect("one alias-resolution set per domain");
        let Some(alias) = domain.alias.as_mut() else {
            return;
        };
        for (constituent, symbol) in alias.constituents.iter_mut().zip(resolved) {
            constituent.domain_symbol = symbol;
        }
    });
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
                psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                    let name = domain_path_members
                        .span_or_empty(membership.domain)
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    membership.domain_symbol =
                        resolve_domain_symbol(symbols, &domain_symbols, &name);
                }
                psi_symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                    assign_proof_expression_symbols(
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
                let psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) = fact
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
                    psi_symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                        Some(*expression)
                    }
                    psi_symbol_resolved_trees::domain::ProofFact::Membership(_) => None,
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
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(domain.facts)
    else {
        return None;
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
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
    let psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
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

fn assign_proof_expression_symbols(
    symbols: &SymbolTable,
    domain_symbols: &[(
        String,
        SymbolHandle,
        psi_language_semantics::SemanticDomainId,
    )],
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let expression_node = expression_table.expression(expression).clone();
    match expression_node {
        psi_symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                atomic.value,
            );
            if atomic.result.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    atomic.result,
                );
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            for value in expression_table.expression_handles(values).to_vec() {
                assign_proof_expression_symbols(symbols, domain_symbols, expression_table, value);
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_proof_expression_symbols(symbols, domain_symbols, expression_table, binary.left);
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                binary.right,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    call.receiver,
                );
            }
            for argument in expression_table.expression_handles(call.arguments).to_vec() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    argument,
                );
            }
            if !call.receiver.is_valid() && !call.target_symbol.is_valid() {
                let target_symbol =
                    resolve_free_machine_entry_state_symbol(symbols, call.target.as_str());
                if let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
                    expression_table.expression_mut(expression)
                {
                    call.target_symbol = target_symbol;
                }
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            assign_proof_expression_symbols(symbols, domain_symbols, expression_table, cast.value);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                indexed.collection,
            );
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                indexed.index,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    range.start,
                );
            }
            if range.end.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    range.end,
                );
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_proof_expression_symbols(
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
            // Proof expressions live in the shared body-expression table, but
            // are not visited by the ordinary machine-expression resolver.
            // Assign the same exact declared-domain or Type::Case identities
            // here, at their symbol-resolution owner, before reconciling the
            // specialized domain lookup below.
            assign_membership_symbol(symbols, expression_table, membership.domain, expression);
            let domain_symbol = resolve_domain_symbol(symbols, domain_symbols, &name);
            if let psi_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
                expression_table.expression_mut(expression)
            {
                membership.domain_symbol = domain_symbol;
                if domain_symbol.is_valid() {
                    membership.case_type_symbol = SymbolHandle::invalid();
                    membership.case_symbol = SymbolHandle::invalid();
                }
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                member.receiver,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                inner.target,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                unary.operand,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            for field in expression_table
                .struct_fields(struct_literal.fields)
                .to_vec()
            {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    field.value,
                );
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::Name(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::String(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::ZeroValue(_) => {}
    }
}

fn resolve_domain_symbol(
    symbols: &SymbolTable,
    domain_symbols: &[(
        String,
        SymbolHandle,
        psi_language_semantics::SemanticDomainId,
    )],
    name: &str,
) -> SymbolHandle {
    if name.contains("::") {
        return domain_symbols
            .iter()
            .find(|(candidate, _, _)| candidate == name)
            .map(|(_, symbol, _)| *symbol)
            .or_else(|| {
                symbols.find_child_by_name_and_kind(symbols.root(), name, SymbolKind::Domain)
            })
            .unwrap_or_else(SymbolHandle::invalid);
    }

    let mut matches = domain_symbols
        .iter()
        .filter(|(candidate, _, _)| candidate.rsplit("::").next().unwrap_or(candidate) == name);
    let Some((first_name, symbol, first_semantic_id)) = matches.next() else {
        return SymbolHandle::invalid();
    };
    // Capacity-specialized carriers normalize their domain owner to the same
    // published name (`[u8; N]::Utf8`). Multiple declarations with that exact
    // normalized name are one semantic lookup candidate, not an ambiguous set.
    // Distinct owners that merely share the trailing presentation name remain
    // ambiguous and fail closed.
    if matches.any(|(candidate, _, semantic_id)| {
        candidate != first_name || semantic_id != first_semantic_id
    }) {
        SymbolHandle::invalid()
    } else {
        *symbol
    }
}
