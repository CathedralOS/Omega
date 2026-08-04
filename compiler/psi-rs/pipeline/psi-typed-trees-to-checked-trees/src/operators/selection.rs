//! Binding-site selection for domain-owned operator meanings (chapter 8,
//! "Domain-Sensitive Operators"). A domain-owned candidate participates only
//! when its semantic role is selected by an operand declaration, explicit
//! mint, or signature `requires`. Flow-established predicate membership never
//! changes operator meaning.
//!
//! Exactly one active domain meaning wins the expression and takes precedence
//! over the builtin surface.
//! No admissible domain meaning leaves the ordinary operation in place when
//! one exists (unique root candidate or a primitive operand's builtin), and is
//! rejected otherwise. Two or more admissible domain meanings are ambiguous.

use psi_checked_trees::{
    CheckedOperatorCandidateFact, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedOperatorUseFact, CheckedValueOrigin,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use psi_typed_trees::signature::SignatureContractKind;
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::receiver::expression_type_reference_for_origin;

/// Rewrites every `DomainPending` use from static binding-site selections. No
/// proof/fact environment is accepted here, making the activation law hard to
/// violate accidentally.
pub(crate) fn select_pending_domain_operator_meanings(
    program: &TypedTrees,
    operators: &mut CheckedOperatorFacts,
) {
    let pending: Vec<_> = operators
        .uses
        .iter()
        .filter(|(_, operator_use)| {
            operator_use.status == CheckedOperatorResolutionStatus::DomainPending
        })
        .map(|(handle, operator_use)| (handle, *operator_use))
        .collect();

    for (handle, operator_use) in pending {
        let candidates: Vec<CheckedOperatorCandidateFact> =
            operators.candidates(&operator_use).to_vec();
        let (status, selected_operator_symbol) =
            finalize_pending_use(program, &operator_use, &candidates);
        let operator_use = operators.uses.get_mut(handle);
        operator_use.status = status;
        operator_use.selected_operator_symbol = selected_operator_symbol;
    }
}

fn finalize_pending_use(
    program: &TypedTrees,
    operator_use: &CheckedOperatorUseFact,
    candidates: &[CheckedOperatorCandidateFact],
) -> (CheckedOperatorResolutionStatus, SymbolHandle) {
    let admissible: Vec<&CheckedOperatorCandidateFact> = candidates
        .iter()
        .filter(|candidate| candidate.is_domain_owned())
        .filter(|candidate| domain_is_active_at_use(program, operator_use, candidate.domain_symbol))
        .collect();

    match admissible.as_slice() {
        [winner] => (
            CheckedOperatorResolutionStatus::Resolved,
            winner.operator_symbol,
        ),
        [] => inadmissible_domain_fallback(program, operator_use, candidates),
        // Competing active domain meanings for one expression are never ranked.
        _ => (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        ),
    }
}

/// No domain candidate was admissible: the ordinary meaning stays selected
/// when one exists (chapter 8: declaring a domain operator does not replace
/// the builtin). A unique root spelled candidate is that surface; otherwise a
/// primitive left operand keeps its builtin operation. A non-primitive operand
/// with only inadmissible domain meanings has no meaning at all.
fn inadmissible_domain_fallback(
    program: &TypedTrees,
    operator_use: &CheckedOperatorUseFact,
    candidates: &[CheckedOperatorCandidateFact],
) -> (CheckedOperatorResolutionStatus, SymbolHandle) {
    let roots: Vec<&CheckedOperatorCandidateFact> = candidates
        .iter()
        .filter(|candidate| !candidate.is_domain_owned())
        .collect();

    match roots.as_slice() {
        [root] => (
            CheckedOperatorResolutionStatus::Resolved,
            root.operator_symbol,
        ),
        [] if builtin_meaning_exists(program, operator_use) => (
            CheckedOperatorResolutionStatus::BuiltinFallback,
            SymbolHandle::invalid(),
        ),
        [] => (
            CheckedOperatorResolutionStatus::Inadmissible,
            SymbolHandle::invalid(),
        ),
        _ => (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        ),
    }
}

/// Whether any operand binding statically selects `domain_symbol`'s semantic
/// facet. This intentionally has no route to flow facts.
fn domain_is_active_at_use(
    program: &TypedTrees,
    operator_use: &CheckedOperatorUseFact,
    domain_symbol: SymbolHandle,
) -> bool {
    operator_operands(program, operator_use.expression)
        .into_iter()
        .any(|operand| operand_selects_domain(program, operator_use.origin, operand, domain_symbol))
}

fn operator_operands(program: &TypedTrees, expression: ExpressionHandle) -> Vec<ExpressionHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
        ExpressionNode::Indexed(indexed) => {
            let mut operands = vec![indexed.collection];
            match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => {
                    operands.push(range.start);
                    operands.push(range.end);
                }
                _ => operands.push(indexed.index),
            }
            operands
        }
        _ => Vec::new(),
    }
}

fn operand_selects_domain(
    program: &TypedTrees,
    origin: CheckedValueOrigin,
    operand: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    expression_type_reference_for_origin(program, operand, origin).is_some_and(|type_reference| {
        type_selects_semantic_domain(program, type_reference, domain_symbol)
    }) || expression_mint_selects_domain(program, origin, operand, domain_symbol)
        || signature_selects_domain(program, origin, operand, domain_symbol)
}

fn type_selects_semantic_domain(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_selects_semantic_domain(program, *referee, domain_symbol)
        }
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(|constraint| match constraint {
                psi_typed_trees::types::TypeConstraintNode::Domain(domain) => {
                    domain.symbol == domain_symbol
                        && domain.semantic_roles.denotation_dimension.is_some()
                }
                _ => false,
            }),
        _ => false,
    }
}

fn expression_mint_selects_domain(
    program: &TypedTrees,
    origin: CheckedValueOrigin,
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            expression_mint_selects_domain(program, origin, atomic.value, domain_symbol)
        }
        ExpressionNode::Mutable(inner) => {
            expression_mint_selects_domain(program, origin, *inner, domain_symbol)
        }
        ExpressionNode::Cast(cast) => cast_selects_domain(program, cast, domain_symbol),
        ExpressionNode::Name(path) => local_initializer_selects_domain(
            program,
            origin,
            path.symbol,
            &program.expression_table.display_name(expression),
            domain_symbol,
        ),
        _ => false,
    }
}

fn cast_selects_domain(
    program: &TypedTrees,
    cast: &TableCastExpression,
    domain_symbol: SymbolHandle,
) -> bool {
    if cast.semantic_domain.is_empty() {
        return false;
    }
    let Some(domain) = program.domain_definitions().iter().find(|domain| {
        domain.symbol == domain_symbol && domain.semantic_roles.denotation_dimension.is_some()
    }) else {
        return false;
    };
    let authored = program
        .expression_table
        .name_path_members(cast.semantic_domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    domain.name.as_str() == authored
        || domain
            .name
            .as_str()
            .rsplit("::")
            .next()
            .is_some_and(|short| short == authored)
}

fn local_initializer_selects_domain(
    program: &TypedTrees,
    origin: CheckedValueOrigin,
    symbol: SymbolHandle,
    binding_name: &str,
    domain_symbol: SymbolHandle,
) -> bool {
    let CheckedValueOrigin::StateStatement {
        state_symbol,
        statement_index,
        ..
    } = origin
    else {
        return false;
    };
    let Some(state) = crate::semantic_calls::find_state(program, state_symbol) else {
        return false;
    };
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::LocalData(local)
                if (symbol.is_valid() && local.symbol == symbol)
                    || local.name.as_str() == binding_name =>
            {
                Some(local.initial_value)
            }
            _ => None,
        })
        .is_some_and(|initializer| {
            expression_mint_selects_domain(program, origin, initializer, domain_symbol)
        })
}

fn signature_selects_domain(
    program: &TypedTrees,
    origin: CheckedValueOrigin,
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let CheckedValueOrigin::StateStatement { state_symbol, .. } = origin else {
        return false;
    };
    let Some(binding_symbol) = direct_binding_symbol(program, expression) else {
        return false;
    };
    let binding_name = program.expression_table.display_name(expression);
    let machine = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == state_symbol)
    });
    let state = crate::semantic_calls::find_state(program, state_symbol);
    machine
        .into_iter()
        .flat_map(|machine| program.machine_contracts(machine))
        .chain(
            state
                .into_iter()
                .flat_map(|state| program.state_contracts(state)),
        )
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| match fact {
            ProofFact::Membership(membership) => {
                membership.domain_symbol == domain_symbol
                    && (direct_binding_symbol(program, membership.value) == Some(binding_symbol)
                        || (!binding_name.is_empty()
                            && program.expression_table.display_name(membership.value)
                                == binding_name))
                    && program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == domain_symbol)
                        .is_some_and(|domain| domain.semantic_roles.denotation_dimension.is_some())
            }
            ProofFact::Expression(_) => false,
            ProofFact::Proposition(_) => false,
        })
}

fn direct_binding_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => direct_binding_symbol(program, atomic.value),
        ExpressionNode::Mutable(inner) => direct_binding_symbol(program, *inner),
        ExpressionNode::Name(path) => path
            .symbol
            .is_valid()
            .then_some(path.symbol)
            .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol)),
        _ => None,
    }
}

/// Whether the first operand type carries an ordinary builtin operation: a
/// primitive scalar does, a user data type does not. Indexed core surfaces
/// normally retain their root candidate and therefore do not need this path.
fn builtin_meaning_exists(program: &TypedTrees, operator_use: &CheckedOperatorUseFact) -> bool {
    let Some(left_operand) = operator_operands(program, operator_use.expression)
        .first()
        .copied()
    else {
        return false;
    };
    expression_type_reference_for_origin(program, left_operand, operator_use.origin)
        .is_some_and(|type_reference| type_is_primitive(program, type_reference))
}

fn type_is_primitive(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => type_is_primitive(program, *referee),
        TypeReferenceNode::Named { name, .. } => PrimitiveType::from_name(name.as_str()).is_some(),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => false,
    }
}
