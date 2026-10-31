//! Give equality operands their selected type role before value preparation.
//! A spelling never supplies this role: ordinary lexical selection has already
//! distinguished data/type binders from constants, locals and value binders.

use crate::tables::body_expressions;
use diagnostics::Diagnostic;
use symbol_resolved_trees::{
    SymbolResolvedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    types::TypeReference,
};
use symbols::SymbolKind;

pub(crate) fn retain_operands(
    program: &mut SymbolResolvedTrees,
    expressions: &[ExpressionHandle],
) -> Result<(), Vec<Diagnostic>> {
    for &expression in expressions {
        let ExpressionNode::Binary(binary) =
            program.tables.bodies.expressions.expression(expression)
        else {
            continue;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) {
            continue;
        }
        let (left, right) = (binary.left, binary.right);
        let operands = [left, right].map(|operand| has_type_role(program, operand, 0));
        match operands {
            [false, false] => continue,
            [true, true] => {
                for operand in [left, right] {
                    if matches!(
                        program.tables.bodies.expressions.expression(operand),
                        ExpressionNode::TypeExpression(_)
                    ) {
                        continue;
                    }
                    let (reference, occurrences) = materialize_type_operand(program, operand);
                    let reference = program
                        .tables
                        .declarations
                        .child_type_references
                        .append(reference);
                    program
                        .tables
                        .bodies
                        .expressions
                        .attach_authored_selection_occurrences(operand, occurrences);
                    *program.tables.bodies.expressions.expression_mut(operand) =
                        ExpressionNode::TypeExpression(reference);
                }
            }
            _ => {
                return Err(vec![
                    Diagnostic::error(
                        "static type equality cannot mix a type operand with a runtime value",
                    )
                    .with_source_span(program.tables.bodies.expressions.source_span(expression)),
                ]);
            }
        }
    }
    Ok(())
}

pub(crate) fn is_type_predicate(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> bool {
    matches!(program.tables.bodies.expressions.expression(expression), ExpressionNode::Binary(binary)
        if matches!(binary.operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
        && has_type_role(program, binary.left, 0)
        && has_type_role(program, binary.right, 0))
}

fn named_type_operand(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReference> {
    match program.tables.bodies.expressions.expression(expression) {
        ExpressionNode::TypeExpression(reference) => {
            Some(program.child_type_reference(*reference).clone())
        }
        ExpressionNode::Name(path) if path.symbol.is_valid() => {
            let kind = program.symbols.get(path.symbol).kind;
            let type_binder =
                kind == SymbolKind::TypeParameter
                    && program.tables.declarations.data_type_parameters.iter().any(
                        |(_, parameter)| {
                            parameter.symbol == path.symbol
                                && matches!(
                                    parameter.kind,
                                    symbol_resolved_trees::data::TypeParameterKind::Type
                                )
                        },
                    );
            if !type_binder
                && !matches!(
                    kind,
                    SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::Domain
                )
            {
                return None;
            }
            let name = body_expressions(&program)
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            Some(TypeReference::Named {
                supplied_operand_owner: Default::default(),
                symbol: path.symbol,
                name: symbol_resolved_trees::name::DiagnosticName::new(
                    name,
                    program.tables.bodies.expressions.source_span(expression),
                ),
            })
        }
        _ => None,
    }
}

/// Value borrow/array syntax receives type role only when every nested name
/// already selects a type. No spelling or compatible storage chooses the role.
fn has_type_role(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 || !expression.is_valid() {
        return false;
    }
    if named_type_operand(program, expression).is_some() {
        return true;
    }
    match program.tables.bodies.expressions.expression(expression) {
        ExpressionNode::Borrow(borrow) => has_type_role(program, borrow.target, depth + 1),
        ExpressionNode::ArrayLiteral(elements) => {
            match body_expressions(&program).expression_handles(*elements) {
                [element] => has_type_role(program, *element, depth + 1),
                _ => false,
            }
        }
        ExpressionNode::Indexed(indexed) => {
            has_type_role(program, indexed.collection, depth + 1)
                && matches!(
                    program.tables.bodies.expressions.expression(indexed.index),
                    ExpressionNode::Range(_)
                )
        }
        _ => false,
    }
}

fn materialize_type_operand(
    program: &mut SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> (
    TypeReference,
    Vec<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
) {
    use symbol_resolved_trees::types::{
        ConstrainedTypeReference, ConstrainedTypeReferenceStorage, ReferenceTypeReference,
        ReferenceTypeReferenceStorage, SliceTypeReference, SliceTypeReferenceStorage,
        TypeConstraint,
    };
    let mut occurrences = body_expressions(&program)
        .authored_selection_occurrences(expression)
        .collect::<Vec<_>>();
    if let Some(reference) = named_type_operand(program, expression) {
        return (reference, occurrences);
    }
    let reference = match body_expressions(&program).expression(expression).clone() {
        ExpressionNode::Borrow(borrow) => {
            let (reference, child_occurrences) = materialize_type_operand(program, borrow.target);
            occurrences.extend(child_occurrences);
            let referee = program
                .tables
                .declarations
                .child_type_references
                .append(reference);
            TypeReference::Reference(ReferenceTypeReference {
                storage: ReferenceTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    referee,
                    access: borrow.access,
                    lifetime: None,
                },
            })
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let element = body_expressions(&program).expression_handles(elements)[0];
            let (reference, child_occurrences) = materialize_type_operand(program, element);
            occurrences.extend(child_occurrences);
            let element_type = program
                .tables
                .declarations
                .child_type_references
                .append(reference);
            TypeReference::Slice(SliceTypeReference {
                storage: SliceTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    element_type,
                },
            })
        }
        ExpressionNode::Indexed(indexed) => {
            let (base, child_occurrences) = materialize_type_operand(program, indexed.collection);
            occurrences.extend(child_occurrences);
            let base_type = program
                .tables
                .declarations
                .child_type_references
                .append(base);
            let ExpressionNode::Range(range) =
                program.tables.bodies.expressions.expression(indexed.index)
            else {
                unreachable!("classified type range")
            };
            let constraints =
                program
                    .tables
                    .types
                    .constraints
                    .insert_many([TypeConstraint::Range {
                        minimum: range.start,
                        maximum: range.end,
                        end_inclusive: range.end_inclusive,
                    }]);
            TypeReference::Constrained(ConstrainedTypeReference {
                storage: ConstrainedTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    base_type,
                    constraints,
                },
            })
        }
        _ => unreachable!("classified structural type operand"),
    };
    (reference, occurrences)
}

/// The source-role producer identifies type syntax before value-operator
/// occurrences are issued. Bounds remain ordinary value expressions.
pub(crate) fn type_operand_nodes(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    output: &mut Vec<ExpressionHandle>,
) {
    if output.contains(&expression) || !has_type_role(program, expression, 0) {
        return;
    }
    output.push(expression);
    match program.tables.bodies.expressions.expression(expression) {
        ExpressionNode::Borrow(borrow) => type_operand_nodes(program, borrow.target, output),
        ExpressionNode::ArrayLiteral(elements) => {
            for &element in body_expressions(&program).expression_handles(*elements) {
                type_operand_nodes(program, element, output);
            }
        }
        ExpressionNode::Indexed(indexed) => type_operand_nodes(program, indexed.collection, output),
        _ => {}
    }
}
