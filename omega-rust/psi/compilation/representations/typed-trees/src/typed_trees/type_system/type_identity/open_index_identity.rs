//! Open index operation selection and algebra identities.
//!
//! A selected index operation normalizes under exactly the laws its selection
//! records, each from a different checked law slot of the selected trait:
//! associativity flattens a nested chain of one operation, commutativity sorts
//! the operands, a literal identity drops an operand only when both that
//! identity law and its nullary-to-literal connection are published, and left
//! distributivity expands a product over a sum factor when both operations
//! come from one provider of the trait that declares the law. No law implies
//! another, and a missing law keeps the expression's own structure, so two
//! expressions that need it stay distinct instead of either being rejected;
//! compatibility then asks for an established equality.
//!
//! Expansion has to see which operands are sums, so normalization builds
//! `IndexTerm`s and renders them once. Rendering reproduces the identity text
//! the associative and commutative laws alone produced, so an expression that
//! no new law touches keeps its identity.

use crate::TypedTrees;
use crate::expression::{BinaryOperator, ExpressionHandle, OpenIndexOperationSelection};
use crate::typed_trees::type_system::type_identity::constraint_identity::{atom, compound};
use crate::typed_trees::type_system::type_identity::identity_context::{
    TypeIdentityContext, TypeIdentityQualification, normalize_index_expression,
};
use language_semantics::type_identity::vocabulary::{IndexForm, Label};

/// The most summands one expansion may produce. A runaway product keeps its
/// unexpanded form rather than stalling, as the proof side's polynomial form
/// does; a search limit never establishes inequality.
const MAX_EXPANDED_SUMMANDS: usize = 64;

pub(crate) fn open_index_operation_selection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<&OpenIndexOperationSelection> {
    program.expression_table.open_index_operation(expression)
}

fn provider_identity(
    program: &TypedTrees,
    selection: &OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> String {
    super::substitution::index_argument(
        program,
        selection.application.as_ref().map_or(
            super::IndexIdentityArgument::Subject(selection.provider),
            super::IndexIdentityArgument::StaticArgument,
        ),
        context,
    )
}

pub(crate) fn open_index_algebra_identity(
    program: &TypedTrees,
    selection: &OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> String {
    compound(
        Label::OpenIndexAlgebra,
        [
            atom(
                Label::Provider,
                &provider_identity(program, selection, context),
            ),
            atom(
                Label::Trait,
                &context.name(
                    program,
                    selection.algebra_trait,
                    "<unresolved-algebra-trait>",
                ),
            ),
            atom(Label::Requirement, &selection.algebra_requirement),
        ],
    )
}

pub(crate) fn open_index_operation_identity(
    program: &TypedTrees,
    selection: &OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> String {
    let contract = selection.operation_contract_identity(program);
    if context.qualification == TypeIdentityQualification::Ordinary {
        return contract;
    }
    compound(
        Label::OpenIndexOperation,
        [
            atom(
                Label::Symbol,
                &context.name(program, selection.operator, &contract),
            ),
            atom(Label::Contract, &contract),
        ],
    )
}

fn same_open_index_ac_authority(
    program: &TypedTrees,
    left: &OpenIndexOperationSelection,
    right: &OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> bool {
    left.operator == right.operator
        && open_index_algebra_identity(program, left, context)
            == open_index_algebra_identity(program, right, context)
        && program.normalized_type_identity(left.operand_type)
            == program.normalized_type_identity(right.operand_type)
        && left.algebra_trait == right.algebra_trait
        && left.algebra_requirement == right.algebra_requirement
        && left.commutativity_licensed == right.commutativity_licensed
        && left.associativity_licensed == right.associativity_licensed
        && left.distributes_over == right.distributes_over
        && left.left_identity_literal == right.left_identity_literal
        && left.right_identity_literal == right.right_identity_literal
}

/// Whether `product` distributes over `sum` by a law the trait declares: both
/// selections name one provider of one trait, the product lists the sum's
/// requirement, and the sum does not distribute back. A trait that declares
/// both directions (a distributive lattice) has no single expanded form, so
/// neither direction expands and each expression keeps its own structure.
fn distributes_over(
    program: &TypedTrees,
    product: &OpenIndexOperationSelection,
    sum: &OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> bool {
    product.algebra_trait == sum.algebra_trait
        && product.distributes_over.contains(&sum.algebra_requirement)
        && !sum.distributes_over.contains(&product.algebra_requirement)
        && program.normalized_type_identity(product.operand_type)
            == program.normalized_type_identity(sum.operand_type)
        && provider_identity(program, product, context) == provider_identity(program, sum, context)
}

/// An open index expression after the selected algebra's licensed laws applied.
#[derive(Clone)]
enum IndexTerm<'program> {
    /// Identity text of an operand no selected operation owns.
    Leaf(String),
    Operation {
        operator: BinaryOperator,
        selection: &'program OpenIndexOperationSelection,
        operands: Vec<IndexTerm<'program>>,
    },
}

impl<'program> IndexTerm<'program> {
    fn of(
        program: &'program TypedTrees,
        expression: ExpressionHandle,
        context: &TypeIdentityContext<'_>,
    ) -> Self {
        match (
            program.expression_table.binary_node(expression),
            open_index_operation_selection(program, expression),
        ) {
            (Some(binary), Some(selection)) => Self::operation(
                program,
                binary.operator,
                selection,
                vec![
                    Self::of(program, binary.left, context),
                    Self::of(program, binary.right, context),
                ],
                context,
            ),
            _ => Self::Leaf(normalize_index_expression(program, expression, context)),
        }
    }

    /// One selected operation over operands that are already normal, arranged
    /// by the laws its selection records.
    ///
    /// Associativity flattens a nested chain of the same operation under the
    /// same authority into one operand list, so `(a + b) + c` and `a + (b + c)`
    /// reach the same form; without it the operands stand as written and each
    /// nested operation keeps its own structure. Commutativity sorts the
    /// operands, so `a + b` and `b + a` reach the same form; without it the
    /// authored order is the canonical order. A noncommutative, nonassociative
    /// operation with no distributivity is still usable: it keeps its
    /// structural identity.
    fn operation(
        program: &'program TypedTrees,
        operator: BinaryOperator,
        selection: &'program OpenIndexOperationSelection,
        operands: Vec<Self>,
        context: &TypeIdentityContext<'_>,
    ) -> Self {
        let mut operands: Vec<Self> = if selection.associativity_licensed {
            operands
                .into_iter()
                .flat_map(|operand| match operand {
                    Self::Operation {
                        operator: inner,
                        selection: inner_selection,
                        operands: inner_operands,
                    } if inner == operator
                        && same_open_index_ac_authority(
                            program,
                            selection,
                            inner_selection,
                            context,
                        ) =>
                    {
                        inner_operands
                    }
                    operand => vec![operand],
                })
                .collect()
        } else {
            operands
        };
        // Identities come off before ordering and expansion. `N + 0` is `N`
        // only when this operation recorded both the identity law and the
        // literal connection; a chain of nothing but that literal stays the
        // literal rather than disappearing.
        operands = without_licensed_identities(program, selection, operands, context);
        if let [single] = operands.as_slice() {
            return single.clone();
        }
        if selection.commutativity_licensed {
            operands.sort_by_cached_key(|operand| operand.render(program, context));
        }
        let term = Self::Operation {
            operator,
            selection,
            operands,
        };
        term.expanded(program, context).unwrap_or(term)
    }

    /// Left distributivity: `x * (y + z)` becomes `x * y + x * z`, once for each
    /// summand, and each product is arranged again so a sum nested in `y` or
    /// `z` expands too. The law states the sum as the right operand; the product
    /// commutes any other factor there only when its commutativity is licensed,
    /// and its associativity groups the remaining factors as the left operand.
    /// The new sum uses the selection of the sum it replaces, so the result
    /// flattens and sorts under the addition's own laws.
    fn expanded(
        &self,
        program: &'program TypedTrees,
        context: &TypeIdentityContext<'_>,
    ) -> Option<Self> {
        let Self::Operation {
            operator,
            selection,
            operands,
        } = self
        else {
            return None;
        };
        if selection.distributes_over.is_empty() {
            return None;
        }
        let licensed_sum = |operand: &Self| match operand {
            Self::Operation {
                selection: sum,
                operands,
                ..
            } if distributes_over(program, selection, sum, context) => Some(operands.len()),
            _ => None,
        };
        let sum_position = operands
            .iter()
            .enumerate()
            .position(|(position, operand)| {
                (selection.commutativity_licensed || position + 1 == operands.len())
                    && licensed_sum(operand).is_some()
            })?;
        let Self::Operation {
            operator: sum_operator,
            selection: sum_selection,
            operands: summands,
        } = &operands[sum_position]
        else {
            return None;
        };
        let produced = if selection.commutativity_licensed {
            operands.iter().fold(1usize, |count, operand| {
                count.saturating_mul(licensed_sum(operand).unwrap_or(1))
            })
        } else {
            summands.len()
        };
        if produced > MAX_EXPANDED_SUMMANDS {
            return None;
        }
        let products = summands
            .iter()
            .map(|summand| {
                let mut factors = operands
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| *position != sum_position)
                    .map(|(_, factor)| factor.clone())
                    .collect::<Vec<_>>();
                factors.push(summand.clone());
                Self::operation(program, *operator, selection, factors, context)
            })
            .collect();
        Some(Self::operation(
            program,
            *sum_operator,
            sum_selection,
            products,
            context,
        ))
    }

    fn render(&self, program: &TypedTrees, context: &TypeIdentityContext<'_>) -> String {
        match self {
            Self::Leaf(text) => text.clone(),
            Self::Operation {
                operator,
                selection,
                operands,
            } => compound(
                index_binary_operator_name(*operator),
                [
                    atom(
                        Label::Operation,
                        &open_index_operation_identity(program, selection, context),
                    ),
                    atom(
                        Label::Algebra,
                        &open_index_algebra_identity(program, selection, context),
                    ),
                ]
                .into_iter()
                .chain(
                    operands
                        .iter()
                        .map(|operand| operand.render(program, context)),
                ),
            ),
        }
    }
}

/// Drop operands the selection's literal identity laws remove.
///
/// A right identity removes a non-leading operand, and a leading one only
/// with a left-identity or commutativity law. A left identity is symmetric.
/// When every operand is that same literal, one copy remains. Two different
/// licensed literals in one chain remove nothing.
fn without_licensed_identities<'program>(
    program: &'program TypedTrees,
    selection: &'program OpenIndexOperationSelection,
    operands: Vec<IndexTerm<'program>>,
    context: &TypeIdentityContext<'_>,
) -> Vec<IndexTerm<'program>> {
    let len = operands.len();
    let matches = operands
        .iter()
        .enumerate()
        .map(|(index, operand)| {
            let rendered = operand.render(program, context);
            let left = literal_matches(&selection.left_identity_literal, &rendered);
            let right = literal_matches(&selection.right_identity_literal, &rendered);
            let removable = identity_operand_is_removable(index, len, left, right, selection);
            (rendered, removable)
        })
        .collect::<Vec<_>>();
    let mut shared = None;
    for (rendered, removable) in &matches {
        if !removable {
            continue;
        }
        match &shared {
            None => shared = Some(rendered.clone()),
            Some(existing) if existing == rendered => {}
            Some(_) => return operands,
        }
    }
    let mut kept = Vec::new();
    for (operand, (_, removable)) in operands.into_iter().zip(matches) {
        if removable {
            continue;
        }
        kept.push(operand);
    }
    if kept.is_empty()
        && let Some(text) = shared
    {
        return vec![IndexTerm::Leaf(text)];
    }
    kept
}

fn literal_matches(literal: &str, rendered: &str) -> bool {
    !literal.is_empty() && rendered == atom(IndexForm::Integer, literal)
}

fn identity_operand_is_removable(
    index: usize,
    len: usize,
    left: bool,
    right: bool,
    selection: &OpenIndexOperationSelection,
) -> bool {
    if !left && !right {
        return false;
    }
    let leading = index == 0;
    let trailing = index + 1 == len;
    let commutative = selection.commutativity_licensed;
    (right && (!leading || commutative || left)) || (left && (!trailing || commutative || right))
}

/// The identity text of a binary index expression whose operation the consuming
/// machine's conformance bound selected.
pub(crate) fn normalize_selected_index_operation(
    program: &TypedTrees,
    expression: ExpressionHandle,
    context: &TypeIdentityContext<'_>,
) -> String {
    IndexTerm::of(program, expression, context).render(program, context)
}

pub(crate) fn index_binary_operator_name(operator: BinaryOperator) -> IndexForm {
    match operator {
        BinaryOperator::Add => IndexForm::Add,
        BinaryOperator::And => IndexForm::And,
        BinaryOperator::BitwiseAnd => IndexForm::BitwiseAnd,
        BinaryOperator::BitwiseOr => IndexForm::BitwiseOr,
        BinaryOperator::BitwiseXor => IndexForm::BitwiseXor,
        BinaryOperator::Divide => IndexForm::Divide,
        BinaryOperator::Equal => IndexForm::Equal,
        BinaryOperator::Greater => IndexForm::Greater,
        BinaryOperator::GreaterOrEqual => IndexForm::GreaterOrEqual,
        BinaryOperator::Less => IndexForm::Less,
        BinaryOperator::LessOrEqual => IndexForm::LessOrEqual,
        BinaryOperator::Modulo => IndexForm::Modulo,
        BinaryOperator::Multiply => IndexForm::Multiply,
        BinaryOperator::NotEqual => IndexForm::NotEqual,
        BinaryOperator::Or => IndexForm::Or,
        BinaryOperator::ShiftLeft => IndexForm::ShiftLeft,
        BinaryOperator::ShiftRight => IndexForm::ShiftRight,
        BinaryOperator::Subtract => IndexForm::Subtract,
        BinaryOperator::CaseMembership => IndexForm::CaseMembership,
    }
}
