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

use checked_trees::{
    CheckedOperatorCandidateFact, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedOperatorUseFact, CheckedValueOrigin,
};
use symbols::JoinedPath;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::receiver::expression_type_reference_for_origin;

/// Rewrites every `DomainPending` use from static binding-site selections. No
/// proof/fact environment is accepted here, making the activation law hard to
/// violate accidentally.
/// The typed program is immutable; only the caller's use-status records change.
pub fn select_pending_domain_operator_meanings(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    operator_use: &CheckedOperatorUseFact,
    candidates: &[CheckedOperatorCandidateFact],
) -> (CheckedOperatorResolutionStatus, SymbolHandle) {
    let identities = candidates
        .iter()
        .map(|candidate| (candidate.operator_symbol, candidate.domain_symbol))
        .collect::<Vec<_>>();
    select_scoped_candidates(
        &identities,
        |domain| domain_is_active_at_use(program, operator_use, domain),
        builtin_meaning_exists(program, operator_use),
    )
}

/// Whether any operand binding statically selects `domain_symbol`'s semantic
/// facet. This intentionally has no route to flow facts.
fn domain_is_active_at_use(
    program: &dyn crate::CheckingInput,
    operator_use: &CheckedOperatorUseFact,
    domain_symbol: SymbolHandle,
) -> bool {
    operator_use
        .operands(program)
        .unwrap_or_default()
        .into_iter()
        .any(|operand| operand_selects_domain(program, operator_use.origin, operand, domain_symbol))
}

fn operand_selects_domain(
    program: &dyn crate::CheckingInput,
    origin: CheckedValueOrigin,
    operand: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    expression_type_reference_for_origin(program, operand, origin).is_some_and(|type_reference| {
        typed_trees::domain::type_selects_semantic_domain(program, type_reference, domain_symbol)
    }) || expression_mint_selects_domain(program, origin, operand, domain_symbol)
        || signature_selects_domain(program, origin, operand, domain_symbol)
}

fn expression_mint_selects_domain(
    program: &dyn crate::CheckingInput,
    origin: CheckedValueOrigin,
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            expression_mint_selects_domain(program, origin, atomic.value, domain_symbol)
        }
        ExpressionNode::Borrow(inner) => {
            expression_mint_selects_domain(program, origin, inner.target, domain_symbol)
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
    program: &dyn crate::CheckingInput,
    cast: &TableCastExpression,
    domain_symbol: SymbolHandle,
) -> bool {
    if cast.semantic_domain.is_empty() {
        return false;
    }
    let Some(domain) = typed_trees::domain::domain_by_symbol(program, domain_symbol)
        .filter(|domain| domain.semantic_roles.denotation_dimension.is_some())
    else {
        return false;
    };
    let authored = program
        .expression_table
        .name_path_members(cast.semantic_domain)
        .joined_path();
    domain.name.as_str() == authored
        || domain
            .name
            .as_str()
            .rsplit("::")
            .next()
            .is_some_and(|short| short == authored)
}

fn local_initializer_selects_domain(
    program: &dyn crate::CheckingInput,
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
    let Some(state) = program.state_by_symbol(state_symbol) else {
        return false;
    };
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
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
    program: &dyn crate::CheckingInput,
    origin: CheckedValueOrigin,
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let facts = super::scoped_applications::occurrence_requires(program, origin);
    signature_facts_select_domain(program, &facts, expression, domain_symbol)
}

pub(super) fn signature_facts_select_domain(
    program: &dyn crate::CheckingInput,
    facts: &[&ProofFact],
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(binding) = direct_binding_symbol(program, expression) else {
        return false;
    };
    facts.iter().any(|fact| matches!(fact, ProofFact::Membership(membership)
        if membership.domain_symbol == domain_symbol
            && direct_binding_symbol(program, membership.value) == Some(binding)
            && program.domain_definition(domain_symbol).is_some_and(|domain| domain.semantic_roles.denotation_dimension.is_some())))
}

pub(super) fn scoped_mint_selects_domain(
    program: &dyn crate::CheckingInput,
    origin: Option<CheckedValueOrigin>,
    expression: ExpressionHandle,
    domain: SymbolHandle,
) -> bool {
    if let Some(origin) = origin {
        return expression_mint_selects_domain(program, origin, expression, domain);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) => cast_selects_domain(program, cast, domain),
        ExpressionNode::Borrow(borrow) => {
            scoped_mint_selects_domain(program, None, borrow.target, domain)
        }
        ExpressionNode::Atomic(atomic) => {
            scoped_mint_selects_domain(program, None, atomic.value, domain)
        }
        _ => false,
    }
}

/// Shared declaration-only arbitration. Callers supply actual lexical domain
/// activation; this operation never consults flow-established memberships.
pub(super) fn select_scoped_candidates(
    candidates: &[(SymbolHandle, SymbolHandle)],
    active: impl Fn(SymbolHandle) -> bool,
    builtin: bool,
) -> (CheckedOperatorResolutionStatus, SymbolHandle) {
    let admissible = candidates
        .iter()
        .filter(|(_, domain)| domain.is_valid() && active(*domain))
        .collect::<Vec<_>>();
    match admissible.as_slice() {
        [selected] => return (CheckedOperatorResolutionStatus::Resolved, selected.0),
        [] => {}
        _ => {
            return (
                CheckedOperatorResolutionStatus::Ambiguous,
                SymbolHandle::invalid(),
            );
        }
    }
    let roots = candidates
        .iter()
        .filter(|(_, domain)| !domain.is_valid())
        .collect::<Vec<_>>();
    match roots.as_slice() {
        [selected] => (CheckedOperatorResolutionStatus::Resolved, selected.0),
        [] if builtin => (
            CheckedOperatorResolutionStatus::BuiltinFallback,
            SymbolHandle::invalid(),
        ),
        [] => (
            if candidates.is_empty() {
                CheckedOperatorResolutionStatus::Missing
            } else {
                CheckedOperatorResolutionStatus::Inadmissible
            },
            SymbolHandle::invalid(),
        ),
        _ => (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        ),
    }
}

fn direct_binding_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => direct_binding_symbol(program, atomic.value),
        ExpressionNode::Borrow(inner) => direct_binding_symbol(program, inner.target),
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
fn builtin_meaning_exists(
    program: &dyn crate::CheckingInput,
    operator_use: &CheckedOperatorUseFact,
) -> bool {
    if operator_use.occurrence != checked_trees::CheckedOperatorOccurrence::Expression {
        return false;
    }
    let Some(left_operand) = operator_use
        .operands(program)
        .unwrap_or_default()
        .first()
        .copied()
    else {
        return false;
    };
    expression_type_reference_for_origin(program, left_operand, operator_use.origin)
        .is_some_and(|type_reference| type_is_primitive(program, type_reference))
}

pub(super) fn type_is_primitive(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
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
