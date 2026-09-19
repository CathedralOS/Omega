//! Operator spellings, operand matching and declared domain constraints.

use crate::TypedTrees;
use crate::operator::indexing;
use crate::typed_trees::declarations::operator::operand_signatures::normalized_operand_parameters;
use crate::typed_trees::declarations::operator::type_matching::type_reference_matches;
use crate::typed_trees::declarations::operator::{
    OperatorConstBinding, OperatorDefinition, SpelledOperator, selected_trait_operator_meanings,
};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;

/// Enumerate an operator `spelling`, optionally narrowed by the first operand.
/// This receiver-only query remains useful to validation that merely asks
/// whether a spelling exists for a carrier. Complete use-site resolution goes
/// through [`resolve_spelling_for_operands`]. Return types never distinguish.
///
/// Three declaration forms bind a spelling: authored root `operator`
/// declarations, domain-owned operators, and token-bearing machines
/// (`machine + Vec2::add(...)`). The machine form is the declaration and its
/// own executable supply; it enters the candidate set through the
/// operator-signature view in [`TypedTrees::machine_token_bindings`], which
/// shares the machine's symbol and entry-state spans. That view exists only
/// because every candidate consumer reads an [`OperatorDefinition`]; the
/// `operator` introducer retirement will invert this so [`SpelledOperator`]
/// wraps machine signatures directly. Symbol resolution already rejected
/// bindings without a semantic home in their operand tuple, so structural
/// operand matching here is the closed-family selection rule.
pub fn resolve_spelling<'program>(
    program: &'program TypedTrees,
    spelling: OperatorSpelling,
    receiver_type: Option<TypeReferenceHandle>,
) -> Vec<SpelledOperator<'program>> {
    // A token-bearing machine attached to a domain is that domain's family
    // meaning: it participates only where an operand binding selects the
    // domain, exactly like a domain-homed `operator` declaration.
    let root_candidates = program
        .operators()
        .iter()
        .chain(program.machine_token_bindings())
        .filter(|operator| operator.spelling == Some(spelling))
        .map(|operator| SpelledOperator {
            operator,
            domain: program.domain_definitions().iter().find(|domain| {
                operator.home_domain.is_valid() && domain.symbol == operator.home_domain
            }),
        });
    let domain_candidates = program.domain_definitions().iter().flat_map(|domain| {
        program
            .domain_operators(domain)
            .iter()
            .filter(move |operator| operator.spelling == Some(spelling))
            .map(move |operator| SpelledOperator {
                operator,
                domain: Some(domain),
            })
    });

    root_candidates
        .chain(domain_candidates)
        .filter(|candidate| match receiver_type {
            Some(receiver_type) => {
                operator_matches_receiver(program, candidate.operator, receiver_type)
            }
            None => true,
        })
        .collect()
}

/// Resolve a spelling against the complete operand tuple. `None` retains a
/// candidate for an operand whose type is not recoverable at this stage;
/// every known position must match, and generic parameter bindings are shared
/// across the tuple. This is the single use-site resolution authority. The
/// checked stage records its outcome as durable evidence
/// (`CheckedOperatorFacts`) for diagnostics and proof lowering rather than
/// re-resolving.
pub fn resolve_spelling_for_operands<'program>(
    program: &'program TypedTrees,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Vec<SpelledOperator<'program>> {
    resolve_spelling(program, spelling, None)
        .into_iter()
        .filter(|candidate| operator_matches_operands(program, candidate.operator, operand_types))
        .collect()
}

/// Admit builtin expression meaning only when neither declared nor selected
/// trait meanings apply and every authored occurrence retains builtin custody.
/// Declared operators treat unknown operands as wildcard candidates, whereas
/// selected trait matching requires at least one retained operand type. Callers
/// excluding selected trait meaning must supply the actual operand types;
/// absence of a later checked operator row is not builtin authority.
pub fn has_builtin_spelled_expression_meaning(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    expression: crate::expression::ExpressionHandle,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionLateBinding as LateBinding,
        AuthoredDeclarationSelectionTarget as Target,
    };
    resolve_spelling_for_operands(program, spelling, operand_types).is_empty()
        && selected_trait_operator_meanings(program, machine_symbol, spelling, operand_types)
            .is_empty()
        && program
            .expression_table
            .authored_selection_occurrences(expression)
            .all(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        matches!(
                            selection.target(),
                            Target::Intrinsic(Intrinsic::BuiltinOperator)
                                | Target::LateBound(LateBinding::CheckedOperator)
                        )
                    })
            })
}

fn operator_matches_operands(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    operator_matches_operands_with_indexed_collection(program, operator, operand_types, false)
}

pub(crate) fn operator_matches_operands_with_indexed_collection(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
    indexed_collection: bool,
) -> bool {
    let parameters = program.operator_parameters(operator);
    if parameters.len() != operand_types.len() {
        return false;
    }
    let type_parameters = program.operator_type_parameters(operator);
    let mut bindings = Vec::new();
    let mut const_bindings = Vec::new();
    operand_types
        .iter()
        .zip(normalized_operand_parameters(parameters))
        .enumerate()
        .all(|(position, (actual, expected))| {
            actual.is_none_or(|actual| {
                let (matched_actual, matched_expected) = if indexed_collection && position == 0 {
                    indexing::shared_collection_elements(program, actual, expected.type_reference)
                        .unwrap_or((actual, expected.type_reference))
                } else {
                    (actual, expected.type_reference)
                };
                (type_reference_matches(
                    program,
                    matched_actual,
                    matched_expected,
                    None,
                    type_parameters,
                    &mut bindings,
                    &mut const_bindings,
                ) || indexed_receiver_self_match(
                    program,
                    indexed_collection && position == 0 && expected.is_self,
                    actual,
                    expected.type_reference,
                    type_parameters,
                    &mut bindings,
                    &mut const_bindings,
                )) && declared_domain_constraints_match(program, actual, expected.type_reference)
                    && declared_domain_constraints_match(program, matched_actual, matched_expected)
            })
        })
}

/// The `[]`/`[..]` receiver loan behind the ordinary match: position zero as
/// a machine's `self` parameter borrows the collection exactly as a named
/// `collection.at(index)` does. A failed direct attempt can leave speculative
/// type-parameter bindings behind, so the loan retries on copies that replace
/// the working sets only when the retry succeeds.
fn indexed_receiver_self_match(
    program: &TypedTrees,
    applies: bool,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    type_parameters: &[crate::data::TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    const_bindings: &mut Vec<OperatorConstBinding>,
) -> bool {
    if !applies {
        return false;
    }
    let mut retry_bindings = bindings.clone();
    let mut retry_const_bindings = const_bindings.clone();
    let matched = indexing::receiver_self_match(
        program,
        actual,
        expected,
        type_parameters,
        &mut retry_bindings,
        &mut retry_const_bindings,
    );
    if matched {
        *bindings = retry_bindings;
        *const_bindings = retry_const_bindings;
    }
    matched
}

/// Operator operand matching is structurally permissive about refinements, but
/// a declared semantic domain named by the operator parameter is part of that
/// meaning's dispatch key. In particular, `Quantity<METER>` and
/// `Quantity<KILOMETER>` share one family symbol while carrying different
/// normalized instance identities; stripping both constrained shells would
/// make ordinary per-unit overloads ambiguous.
pub(crate) fn declared_domain_constraints_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let expected_domains = declared_domain_constraints(program, expected);
    if expected_domains.is_empty() {
        return true;
    }
    let actual_domains = declared_domain_constraints(program, actual);
    expected_domains.iter().all(|expected| {
        actual_domains
            .iter()
            .any(|actual| declared_domain_constraint_matches(program, actual, expected))
    })
}

fn declared_domain_constraints(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<&crate::types::DomainConstraint> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            declared_domain_constraints(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut domains = declared_domain_constraints(program, *base_type);
            domains.extend(
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .filter_map(|constraint| match constraint {
                        crate::types::TypeConstraintNode::Domain(domain) => Some(domain),
                        _ => None,
                    }),
            );
            domains
        }
        _ => Vec::new(),
    }
}

fn declared_domain_constraint_matches(
    program: &TypedTrees,
    actual: &crate::types::DomainConstraint,
    expected: &crate::types::DomainConstraint,
) -> bool {
    if actual.semantic_id.is_valid() && expected.semantic_id.is_valid() {
        return actual.semantic_id == expected.semantic_id;
    }
    if !actual.symbol.is_valid() || actual.symbol != expected.symbol {
        return false;
    }
    actual.arguments.len() == expected.arguments.len()
        && actual
            .arguments
            .iter()
            .zip(&expected.arguments)
            .all(|(actual, expected)| {
                program.normalized_type_identity(*actual)
                    == program.normalized_type_identity(*expected)
            })
}

/// Whether the operator's first parameter (its receiver) accepts a value of
/// `receiver_type`, binding the operator's own type parameters structurally.
fn operator_matches_receiver(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    receiver_type: TypeReferenceHandle,
) -> bool {
    let Some(receiver_parameter) = program.operator_parameters(operator).first() else {
        return false;
    };
    type_reference_matches(
        program,
        receiver_type,
        receiver_parameter.type_reference,
        None,
        program.operator_type_parameters(operator),
        &mut Vec::new(),
        &mut Vec::new(),
    )
}

/// All candidate indices carrying `spelling`, regardless of operand type. Useful
/// when a site only knows its receiver shape (e.g. "this is a slice") rather
/// than a fully normalized operand key.
pub fn candidates_for_spelling(
    operators: &[OperatorDefinition],
    spelling: OperatorSpelling,
) -> Vec<usize> {
    operators
        .iter()
        .enumerate()
        .filter(|(_, operator)| operator.spelling == Some(spelling))
        .map(|(index, _)| index)
        .collect()
}
