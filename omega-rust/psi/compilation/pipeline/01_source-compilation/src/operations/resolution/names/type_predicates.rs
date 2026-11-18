/*
    Give equality operands their selected type role before value preparation.
    A spelling never supplies this role: ordinary lexical selection has already
    distinguished data/type binders from constants, locals and value binders.
*/

use crate::resolution::tables::body_expressions;
use crate::source_resolved::data::TypeParameterKind;
use crate::source_resolved::name::DiagnosticName;
use crate::source_resolved::{
    SymbolResolvedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    types::TypeReference,
};
use crate::source_selections::AuthoredDeclarationSelectionOccurrenceId;
use diagnostics::Diagnostic;
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
            [left_is_type, _] => {
                let value = if left_is_type { right } else { left };
                return Err(vec![
                    Diagnostic::error(mixed_equality_message(program, value)).with_source_span(
                        program.tables.bodies.expressions.source_span(expression),
                    ),
                ]);
            }
        }
    }
    Ok(())
}

/*
    A value binding never hides a builtin type, so in `requires T == i32`
    with a parameter `i32` in scope the operand reads the parameter and
    the equality mixes a type with a value. Name both meanings so the
    author can see which one was read and how to reach the other.
*/
fn mixed_equality_message(program: &SymbolResolvedTrees, value: ExpressionHandle) -> String {
    const MIXED: &str = "static type equality cannot mix a type operand with a runtime value";
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(value) else {
        return MIXED.to_owned();
    };
    let [name] = body_expressions(program).name_path_members(path.members) else {
        return MIXED.to_owned();
    };
    let name = name.as_str();
    if !crate::source_symbols::is_builtin_type_spelling(name) || !path.symbol.is_valid() {
        return MIXED.to_owned();
    }
    let meaning = match program.symbols.get(path.symbol).kind {
        SymbolKind::Parameter => "parameter",
        SymbolKind::Local => "local",
        SymbolKind::Const => "constant",
        SymbolKind::TypeParameter => "generic value binder",
        _ => "value",
    };
    format!(
        "{MIXED}: `{name}` here is the {meaning} `{name}`, a value, not the builtin type \
             `{name}`; rename the {meaning} or name the type through a type alias"
    )
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
                                && matches!(parameter.kind, TypeParameterKind::Type)
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
            let name = body_expressions(program)
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            Some(TypeReference::Named {
                supplied_operand_owner: Default::default(),
                symbol: path.symbol,
                name: DiagnosticName::new(
                    name,
                    program.tables.bodies.expressions.source_span(expression),
                ),
            })
        }
        _ => None,
    }
}

/*
    Value borrow/array syntax receives type role only when every nested name
    already selects a type. No spelling or compatible storage chooses the role.
*/
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
            match body_expressions(program).expression_handles(*elements) {
                [element] => has_type_role(program, *element, depth + 1),
                _ => false,
            }
        }

        _ => false,
    }
}

fn materialize_type_operand(
    program: &mut SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> (TypeReference, Vec<AuthoredDeclarationSelectionOccurrenceId>) {
    use crate::source_resolved::types::{
        ReferenceTypeReference, ReferenceTypeReferenceStorage, SliceTypeReference,
        SliceTypeReferenceStorage,
    };
    let mut occurrences = body_expressions(program)
        .authored_selection_occurrences(expression)
        .collect::<Vec<_>>();
    if let Some(reference) = named_type_operand(program, expression) {
        return (reference, occurrences);
    }
    let reference = match body_expressions(program).expression(expression).clone() {
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
            let element = body_expressions(program).expression_handles(elements)[0];
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

        _ => unreachable!("classified structural type operand"),
    };
    (reference, occurrences)
}

/*
    The source-role producer identifies type syntax before value-operator
    occurrences are issued. Bounds remain ordinary value expressions.
*/
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
            for &element in body_expressions(program).expression_handles(*elements) {
                type_operand_nodes(program, element, output);
            }
        }

        _ => {}
    }
}
