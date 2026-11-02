//! Open index operation selection and algebra identities.

use crate::TypedTrees;
use crate::expression::{BinaryOperator, ExpressionHandle};
use crate::typed_trees::type_system::type_identity::constraint_identity::{atom, compound};
use crate::typed_trees::type_system::type_identity::identity_context::{
    TypeIdentityContext, TypeIdentityQualification, normalize_index_expression,
};
use language_semantics::type_identity::vocabulary::{IndexForm, Label};

pub(crate) fn open_index_operation_selection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<&crate::expression::OpenIndexOperationSelection> {
    program.expression_table.open_index_operation(expression)
}

pub(crate) fn open_index_algebra_identity(
    program: &TypedTrees,
    selection: &crate::expression::OpenIndexOperationSelection,
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

fn provider_identity(
    program: &TypedTrees,
    selection: &crate::expression::OpenIndexOperationSelection,
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

pub(crate) fn open_index_operation_identity(
    program: &TypedTrees,
    selection: &crate::expression::OpenIndexOperationSelection,
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
    left: &crate::expression::OpenIndexOperationSelection,
    right: &crate::expression::OpenIndexOperationSelection,
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
}

/// `sibling` is `requirement` of the same selected algebra instance as
/// `selection`: one provider application, one trait, one carrier. A sum from
/// another conformance application is not what the law speaks about. When
/// the sum also distributes over the product (a lattice's `max`/`min`), the
/// two expansions undo each other and have no normal form, so neither runs.
fn same_algebra_sibling(
    program: &TypedTrees,
    selection: &crate::expression::OpenIndexOperationSelection,
    sibling: &crate::expression::OpenIndexOperationSelection,
    requirement: &str,
    context: &TypeIdentityContext<'_>,
) -> bool {
    let distributes_back = [
        &sibling.left_distribution_over,
        &sibling.right_distribution_over,
    ]
    .into_iter()
    .any(|over| over.as_deref() == Some(selection.algebra_requirement.as_str()));
    sibling.algebra_requirement == requirement
        && !distributes_back
        && sibling.algebra_trait == selection.algebra_trait
        && program.normalized_type_identity(sibling.operand_type)
            == program.normalized_type_identity(selection.operand_type)
        && provider_identity(program, sibling, context)
            == provider_identity(program, selection, context)
}

/// One open index expression under its retained selections. Leaves are the
/// normalized identities of unselected subexpressions; each operation keeps
/// its selection, so every rewrite consults only the laws that selection's
/// algebra declares.
#[derive(Clone)]
enum IndexTerm<'program> {
    Leaf(String),
    Operation {
        operator: BinaryOperator,
        selection: &'program crate::expression::OpenIndexOperationSelection,
        left: Box<IndexTerm<'program>>,
        right: Box<IndexTerm<'program>>,
    },
}

/// The canonical identity of a selected open index operation, or `None` when
/// `expression` carries no selection.
///
/// Rewrites run bottom-up and each consumes exactly one declared law:
///
/// - a closed identity element `e` is dropped where its law places it
///   (`R(a, e) == a` on the right, `R(e, a) == a` on the left);
///   commutativity lets either law's element stand on either side;
/// - distributivity expands `R(a, S(b, c))` into `S(R(a, b), R(a, c))` (or
///   the right-hand form) only when `S` is the named requirement of the same
///   algebra instance;
/// - associativity flattens a chain of the same operation under the same
///   authority, and commutativity sorts its operands.
///
/// Identity elimination runs before distribution, so `a * (b + 0)` and
/// `a * b` reach one form. A noncommutative, nonassociative operation with no
/// other law keeps its structural identity. Unequal forms never establish
/// inequality, so a budget that stops expanding only loses identifications.
pub(crate) fn normalize_open_index_operation(
    program: &TypedTrees,
    expression: ExpressionHandle,
    context: &TypeIdentityContext<'_>,
) -> Option<String> {
    open_index_operation_selection(program, expression)?;
    // Each expansion is one law step. Chains through three requirements can
    // still cycle; the budget bounds them the way the structural judge caps
    // its polynomial normal form.
    let mut expansions = DISTRIBUTION_BUDGET;
    let term = rewrite(
        program,
        index_term(program, expression, context),
        context,
        &mut expansions,
    );
    Some(render(program, &term, context))
}

const DISTRIBUTION_BUDGET: usize = 64;

fn index_term<'program>(
    program: &'program TypedTrees,
    expression: ExpressionHandle,
    context: &TypeIdentityContext<'_>,
) -> IndexTerm<'program> {
    match (
        program.expression_table.binary_node(expression),
        open_index_operation_selection(program, expression),
    ) {
        (Some(binary), Some(selection)) => IndexTerm::Operation {
            operator: binary.operator,
            selection,
            left: Box::new(index_term(program, binary.left, context)),
            right: Box::new(index_term(program, binary.right, context)),
        },
        _ => IndexTerm::Leaf(normalize_index_expression(program, expression, context)),
    }
}

fn is_identity_element(term: &IndexTerm<'_>, element: Option<&String>) -> bool {
    matches!((term, element), (IndexTerm::Leaf(leaf), Some(element)) if *leaf == atom(IndexForm::Integer, element))
}

fn rewrite<'program>(
    program: &'program TypedTrees,
    term: IndexTerm<'program>,
    context: &TypeIdentityContext<'_>,
    expansions: &mut usize,
) -> IndexTerm<'program> {
    let IndexTerm::Operation {
        operator,
        selection,
        left,
        right,
    } = term
    else {
        return term;
    };
    let left = rewrite(program, *left, context, expansions);
    let right = rewrite(program, *right, context, expansions);
    let commutative = selection.commutativity_licensed;
    let right_identity = selection.right_identity.as_ref();
    let left_identity = selection.left_identity.as_ref();
    if is_identity_element(&right, right_identity)
        || (commutative && is_identity_element(&right, left_identity))
    {
        return left;
    }
    if is_identity_element(&left, left_identity)
        || (commutative && is_identity_element(&left, right_identity))
    {
        return right;
    }
    let product = |left: IndexTerm<'program>, right: IndexTerm<'program>| IndexTerm::Operation {
        operator,
        selection,
        left: Box::new(left),
        right: Box::new(right),
    };
    if *expansions > 0
        && let Some(sum) = selection.left_distribution_over.as_deref()
        && let IndexTerm::Operation {
            operator: sum_operator,
            selection: sum_selection,
            left: first,
            right: second,
        } = &right
        && same_algebra_sibling(program, selection, sum_selection, sum, context)
    {
        *expansions -= 1;
        let expanded = IndexTerm::Operation {
            operator: *sum_operator,
            selection: sum_selection,
            left: Box::new(product(left.clone(), (**first).clone())),
            right: Box::new(product(left, (**second).clone())),
        };
        return rewrite(program, expanded, context, expansions);
    }
    if *expansions > 0
        && let Some(sum) = selection.right_distribution_over.as_deref()
        && let IndexTerm::Operation {
            operator: sum_operator,
            selection: sum_selection,
            left: first,
            right: second,
        } = &left
        && same_algebra_sibling(program, selection, sum_selection, sum, context)
    {
        *expansions -= 1;
        let expanded = IndexTerm::Operation {
            operator: *sum_operator,
            selection: sum_selection,
            left: Box::new(product((**first).clone(), right.clone())),
            right: Box::new(product((**second).clone(), right)),
        };
        return rewrite(program, expanded, context, expansions);
    }
    product(left, right)
}

fn render(program: &TypedTrees, term: &IndexTerm<'_>, context: &TypeIdentityContext<'_>) -> String {
    let (operator, selection, left, right) = match term {
        IndexTerm::Leaf(identity) => return identity.clone(),
        IndexTerm::Operation {
            operator,
            selection,
            left,
            right,
        } => (operator, selection, left, right),
    };
    let mut operands = Vec::new();
    for operand in [left, right] {
        if selection.associativity_licensed {
            collect_associative_operands(
                program,
                operand,
                *operator,
                selection,
                context,
                &mut operands,
            );
        } else {
            operands.push(render(program, operand, context));
        }
    }
    if selection.commutativity_licensed {
        operands.sort();
    }
    compound(
        index_binary_operator_name(*operator),
        std::iter::once(atom(
            Label::Operation,
            &open_index_operation_identity(program, selection, context),
        ))
        .chain(std::iter::once(atom(
            Label::Algebra,
            &open_index_algebra_identity(program, selection, context),
        )))
        .chain(operands),
    )
}

fn collect_associative_operands(
    program: &TypedTrees,
    term: &IndexTerm<'_>,
    operator: BinaryOperator,
    authority: &crate::expression::OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
    operands: &mut Vec<String>,
) {
    if let IndexTerm::Operation {
        operator: child_operator,
        selection,
        left,
        right,
    } = term
        && *child_operator == operator
        && same_open_index_ac_authority(program, authority, selection, context)
    {
        collect_associative_operands(program, left, operator, authority, context, operands);
        collect_associative_operands(program, right, operator, authority, context, operands);
    } else {
        operands.push(render(program, term, context));
    }
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
