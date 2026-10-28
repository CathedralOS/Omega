//! Static calls and nested implementation applications supply the same const
//! destinations as data applications. Reachability comes from the existing
//! concrete-owner walk; declaration lookup supplies only the binder carrier.
//! The shared evaluator still resolves original lexical operands and admits
//! every invocation before publishing a canonical value.

use super::{
    HashSet, Identifier, SyntaxTrees, TypeParameterKind, TypeReferenceHandle, TypeReferenceNode,
    declaration_selection::ConstantSelection,
};
use syntax_trees::expression::{ExpressionNode, StaticMachineArgument};
use syntax_trees::statement::StatementNode;

pub(super) fn collect(
    syntax: &SyntaxTrees,
    selection: &ConstantSelection<'_>,
    positions: &[TypeReferenceHandle],
    public_positions: &HashSet<TypeReferenceHandle>,
    pending: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>,
) {
    let mut positions_set: Option<HashSet<TypeReferenceHandle>> = None;
    let mut visit = |name: Identifier, arguments: &[StaticMachineArgument]| {
        let positions_set =
            positions_set.get_or_insert_with(|| positions.iter().copied().collect());
        collect_application(
            syntax,
            selection,
            &name,
            arguments,
            positions_set,
            public_positions,
            pending,
        );
    };
    for (_, expression) in syntax.expressions.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            if !has_const_expression_argument(syntax, &call.machine_arguments) {
                continue;
            }
            let mut name = call.target.as_str(syntax).to_owned();
            if call.receiver.is_valid() {
                let ExpressionNode::Name(path) = syntax.expressions.expression(call.receiver)
                else {
                    continue;
                };
                let prefix = syntax
                    .expressions
                    .identifier_path_members(*path)
                    .iter()
                    .map(|part| part.as_str(syntax))
                    .collect::<Vec<_>>()
                    .join("::");
                name = format!("{prefix}::{name}");
            }
            visit(
                Identifier::new(name, call.target.source_span()),
                &call.machine_arguments,
            );
        }
    }
    for state in super::uses::selected_machine_state_handles(syntax, syntax.root_item_handles()) {
        for statement in syntax
            .items
            .statements(syntax.items.state(state).statements)
        {
            if let StatementNode::Call(call) = syntax.statements.statement(*statement) {
                if !has_const_expression_argument(syntax, &call.machine_arguments) {
                    continue;
                }
                let mut path = syntax
                    .statements
                    .identifier_path_members(call.receiver)
                    .iter()
                    .map(|part| part.as_str(syntax))
                    .collect::<Vec<_>>();
                path.push(call.target.as_str(syntax));
                visit(
                    Identifier::new(path.join("::"), call.target.source_span()),
                    &call.machine_arguments,
                );
            }
        }
    }
}

fn has_const_expression_argument(
    syntax: &SyntaxTrees,
    arguments: &[StaticMachineArgument],
) -> bool {
    arguments.iter().any(|argument| {
        (argument.type_reference.is_valid()
            && matches!(
                syntax
                    .type_references
                    .type_reference(argument.type_reference),
                TypeReferenceNode::ConstExpression(_)
            ))
            || argument.application.as_ref().is_some_and(|application| {
                has_const_expression_argument(syntax, &application.arguments)
            })
    })
}

fn collect_application(
    syntax: &SyntaxTrees,
    selection: &ConstantSelection<'_>,
    name: &Identifier,
    arguments: &[StaticMachineArgument],
    positions: &HashSet<TypeReferenceHandle>,
    public_positions: &HashSet<TypeReferenceHandle>,
    pending: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>,
) {
    if arguments.iter().any(|argument| {
        argument.type_reference.is_valid()
            && matches!(
                syntax
                    .type_references
                    .type_reference(argument.type_reference),
                TypeReferenceNode::ConstExpression(_)
            )
    }) && let Some(parameters) = selection.static_parameters(syntax, name)
        && parameters.len() == arguments.len()
    {
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if let TypeParameterKind::Const {
                type_reference: destination,
            } = parameter.kind
                && positions.contains(&argument.type_reference)
                && matches!(
                    syntax
                        .type_references
                        .type_reference(argument.type_reference),
                    TypeReferenceNode::ConstExpression(_)
                )
                && !pending
                    .iter()
                    .any(|(existing, _, _)| *existing == argument.type_reference)
            {
                pending.push((
                    argument.type_reference,
                    destination,
                    public_positions.contains(&argument.type_reference),
                ));
            }
        }
    }
    for argument in arguments {
        if let Some(application) = &argument.application
            && let Some(first) = argument.path.first()
        {
            let name = Identifier::new(
                argument
                    .path
                    .iter()
                    .map(|part| part.as_str(syntax))
                    .collect::<Vec<_>>()
                    .join("::"),
                first.source_span(),
            );
            collect_application(
                syntax,
                selection,
                &name,
                &application.arguments,
                positions,
                public_positions,
                pending,
            );
        }
    }
}
