//! Materialize selected const binders without capturing same-spelled names.
//!
//! A nominal-record const is a constructor value, not a scalar spelling.
//! `Required.minimum` is a use of that value. Replacing only the receiver
//! name leaves the projection on the constructor, and diagnostics then show
//! the carrier (`Configuration.minimum`) instead of the substituted field.
//! Open field expressions become canonical when an enclosing const closes;
//! projection reads the constructor's declared field either way. A literal
//! element position (`Setting[1]`) reads the array literal's own element in
//! the same way. A case payload keeps its case, and a non-constructor value
//! keeps the receiver substitution.

use super::Candidate;
use crate::tables::{expressions_of, expressions_of_mut};
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, boolean_literal_spelling};
use numerics::literals::{IntegerLanding, IntegerLiteral};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use validation::expression_traversal::collect_expression_tree;

pub(super) fn substitute(
    program: &mut TypedTrees,
    candidate: &Candidate,
    expression_start: Option<usize>,
    runtime_expression_roots: &[ExpressionHandle],
) -> Result<(), Diagnostic> {
    for (index, ((binder, name, declared_type), binding)) in candidate
        .template
        .const_parameters
        .iter()
        .zip(&candidate.const_bindings)
        .enumerate()
    {
        // A runtime-bound `Value` binder is realized as an ordinary parameter:
        // executable uses were remapped to it during cloning, so any occurrence
        // still reachable from the clone sits in a static context the runtime
        // subject cannot fill. Template-owned expressions keep their authored
        // binder spelling, so the scan follows clone expression roots rather
        // than an arena index bound.
        if candidate.runtime_value_bindings[index].is_some() {
            let stray = runtime_expression_roots.iter().any(|root| {
                let mut tree = Vec::new();
                collect_expression_tree(program, *root, &mut tree);
                tree.into_iter().any(|handle| {
                    matches!(
                        expressions_of(program).expression(handle),
                        ExpressionNode::Name(path)
                            if binder.is_valid()
                                && path.symbol == *binder
                                && path.head_symbol == *binder
                                && expressions_of(program).name_path_members(path.members)
                                    .len()
                                    == 1
                    )
                })
            });
            if stray {
                return Err(Diagnostic::error(format!(
                    "value parameter `{name}` of machine `{}` is bound to a runtime argument and \
                     cannot appear in a static type, contract, or const context",
                    candidate.template.template_name,
                )));
            }
            continue;
        }
        let occurrences = match expression_start {
            Some(start) => expressions_of(program)
                .iter_expressions_from(start)
                .filter_map(|(handle, _)| {
                    exact_binder_name(program, handle, *binder).then_some(handle)
                })
                .collect::<Vec<_>>(),
            None => expressions_of(program)
                .iter_expressions()
                .filter_map(|(handle, _)| {
                    exact_binder_name(program, handle, *binder).then_some(handle)
                })
                .collect::<Vec<_>>(),
        };
        let projections = const_projections(program, *binder, expression_start);
        if occurrences.is_empty() && projections.is_empty() {
            continue;
        }
        // Each use owns its literal children and their checking context.
        let replacement = binding
            .and_then(|binding| value(program, binding, *declared_type))
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "machine `{}` cannot materialize const parameter `{name}` as its declared value type `{}`",
                    candidate.template.template_name,
                    program.display_type_reference(*declared_type),
                ))
            })?;
        let mut projected_receivers = Vec::new();
        for projection in &projections {
            let Some(projected) = project_const_value(program, &replacement, &projection.steps)
            else {
                continue;
            };
            *expressions_of_mut(program).expression_mut(projection.outer) = projected;
            projected_receivers.push(projection.root);
        }
        for occurrence in occurrences {
            if projected_receivers.contains(&occurrence) {
                continue;
            }
            *expressions_of_mut(program).expression_mut(occurrence) = replacement.clone();
        }
    }
    Ok(())
}

fn exact_binder_name(
    program: &TypedTrees,
    expression: ExpressionHandle,
    binder: symbols::SymbolHandle,
) -> bool {
    let ExpressionNode::Name(path) = expressions_of(program).expression(expression) else {
        return false;
    };
    binder.is_valid()
        && path.symbol == binder
        && path.head_symbol == binder
        && expressions_of(program)
            .name_path_members(path.members)
            .len()
            == 1
}

/// One projection step applied to a materialized const value: a record or
/// case field, or a fixed-array element at a literal position.
enum ProjectionStep {
    Field {
        symbol: symbols::SymbolHandle,
        name: typed_trees::name::Identifier,
        case_variant: Option<typed_trees::name::Identifier>,
    },
    Element(usize),
}

struct ConstProjection {
    outer: ExpressionHandle,
    root: ExpressionHandle,
    /// Steps from the constructor outward: `value.a[1]` is `[a, 1]`.
    steps: Vec<ProjectionStep>,
}

/// The receiver and step of one field or literal-element projection node.
fn projection_step(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, ProjectionStep)> {
    match expressions_of(program).expression(expression) {
        ExpressionNode::Member(member) => Some((
            member.receiver,
            ProjectionStep::Field {
                symbol: member.member_symbol,
                name: member.member.clone(),
                case_variant: member.case_variant.clone(),
            },
        )),
        ExpressionNode::Indexed(indexed) => {
            let ExpressionNode::Integer(index) = expressions_of(program).expression(indexed.index)
            else {
                return None;
            };
            let index = usize::try_from(index.value_i64()?).ok()?;
            Some((indexed.collection, ProjectionStep::Element(index)))
        }
        _ => None,
    }
}

/// Outermost field/element chains in this clone whose root name is the const
/// binder. An inner projection is not its own substitution; the outer node is
/// rewritten to the selected child so the constructor does not stay under it.
fn const_projections(
    program: &TypedTrees,
    binder: symbols::SymbolHandle,
    expression_start: Option<usize>,
) -> Vec<ConstProjection> {
    let candidates = match expression_start {
        Some(start) => expressions_of(program)
            .iter_expressions_from(start)
            .map(|(handle, _)| handle)
            .filter(|handle| projection_step(program, *handle).is_some())
            .collect::<Vec<_>>(),
        None => expressions_of(program)
            .iter_expressions()
            .map(|(handle, _)| handle)
            .filter(|handle| projection_step(program, *handle).is_some())
            .collect::<Vec<_>>(),
    };
    let receivers = candidates
        .iter()
        .filter_map(|handle| projection_step(program, *handle).map(|(receiver, _)| receiver))
        .collect::<Vec<_>>();
    let mut projections = Vec::new();
    for outer in candidates {
        if receivers.contains(&outer) {
            continue;
        }
        let mut steps = Vec::new();
        let mut current = outer;
        let root = loop {
            let Some((receiver, step)) = projection_step(program, current) else {
                break current;
            };
            steps.push(step);
            current = receiver;
        };
        if !exact_binder_name(program, root, binder) {
            continue;
        }
        steps.reverse();
        projections.push(ConstProjection { outer, root, steps });
    }
    projections
}

fn project_const_value(
    program: &TypedTrees,
    value: &ExpressionNode,
    steps: &[ProjectionStep],
) -> Option<ExpressionNode> {
    let mut projected = value.clone();
    for step in steps {
        projected = match step {
            ProjectionStep::Field {
                symbol,
                name,
                case_variant,
            } => project_nominal_field(program, &projected, *symbol, name, case_variant.as_ref())?,
            ProjectionStep::Element(index) => {
                let ExpressionNode::ArrayLiteral(elements) = &projected else {
                    return None;
                };
                let element = *expressions_of(program)
                    .expression_handles(*elements)
                    .get(*index)?;
                expressions_of(program).expression(element).clone()
            }
        };
    }
    Some(projected)
}

fn project_nominal_field(
    program: &TypedTrees,
    value: &ExpressionNode,
    symbol: symbols::SymbolHandle,
    name: &typed_trees::name::Identifier,
    case_variant: Option<&typed_trees::name::Identifier>,
) -> Option<ExpressionNode> {
    let ExpressionNode::StructLiteral(literal) = value else {
        return None;
    };
    if let Some(case_variant) = case_variant {
        if literal.case_name.as_ref() != Some(case_variant) {
            return None;
        }
    } else if literal.case_name.is_some() || literal.case_symbol.is_some() {
        return None;
    }
    let selected = expressions_of(program)
        .struct_fields(literal.fields)
        .iter()
        .find(|declared| {
            if symbol.is_valid() {
                declared.field_symbol == symbol
            } else {
                declared.name.as_str() == name.as_str()
            }
        })?;
    Some(expressions_of(program).expression(selected.value).clone())
}

pub(crate) fn value(
    program: &mut TypedTrees,
    binding: TypeReferenceHandle,
    declared_type: TypeReferenceHandle,
) -> Option<ExpressionNode> {
    // An open constructor or arithmetic recipe is already the const value.
    // Canonical atoms below are the closed form of that same recipe.
    if let TypeReferenceNode::ConstExpression(expression) =
        program.type_reference_table.type_reference(binding)
    {
        let expression = *expression;
        if !expressions_of(program).expression_is_valid(expression) {
            return None;
        }
        return Some(expressions_of(program).expression(expression).clone());
    }
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(binding)
    else {
        return None;
    };
    if symbol.is_valid() {
        let declaration = program.const_declaration_by_symbol(*symbol)?;
        let encoding = declaration.evaluated_value.as_ref()?.encoding()?.to_owned();
        let value = CanonicalConstValue::new("", encoding, "").decode_encoding()?;
        return program.materialize_canonical_const_value(declared_type, &value);
    }
    if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
        return program.materialize_canonical_const_value(declared_type, &value.decode_encoding()?);
    }
    let primitive = program.type_reference_table.primitive_type(declared_type)?;
    if primitive == PrimitiveType::Bool {
        return boolean_literal_spelling(name.as_str()).map(ExpressionNode::Boolean);
    }
    let literal = program.static_const_literal_from_type_reference(binding)?;
    integer_value(program, declared_type, literal)
}

fn integer_value(
    program: &dyn ::validation::CheckingInput,
    declared_type: TypeReferenceHandle,
    literal: IntegerLiteral,
) -> Option<ExpressionNode> {
    let primitive = program.type_reference_table.primitive_type(declared_type)?;
    let landed_type = primitive.landed_landing()?;
    Some(ExpressionNode::Integer(
        literal.with_landing(IntegerLanding {
            landed_type,
            domain: program
                .type_reference_table
                .arithmetic_domain(declared_type),
        }),
    ))
}
