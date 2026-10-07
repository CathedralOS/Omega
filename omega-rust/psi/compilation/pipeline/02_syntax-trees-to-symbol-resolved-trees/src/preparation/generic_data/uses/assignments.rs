//! Expected types from constant/local declarations and assignments.
use super::super::{ExpressionNode, Item, StatementNode, SyntaxTrees};
use crate::preparation::generic_data::uses::{
    ConstructorFrontier, concrete_state_local_types, relabel_data_literal_for_expected_type,
};

use crate::preparation::declaration_selection;
use crate::preparation::generic_data::Instantiation;

/// A constant's declared type constrains its constructors before the value is
/// copied into consumers. Otherwise a closed `Box<u64>` declaration can leave
/// an open `Box<T>` constructor and field identity in every later projection.
/// Reuse the exact-owner join used by local destinations; a matching shape or
/// name cannot substitute an unrelated template. Retained authored initializers
/// need the same closed identity for independent evaluation replay.
pub(in crate::preparation::generic_data) fn relabel_closed_data_uses_in_constants(
    syntax: &mut SyntaxTrees,
    instances: &[Instantiation],
    selection: Option<&declaration_selection::ConstantSelection>,
    declarations: &[syntax_trees::item::ItemHandle],
) {
    let constants = declarations
        .iter()
        .map(|declaration| syntax.root_item(*declaration))
        .filter_map(|item| {
            let Item::Const(constant) = item else {
                return None;
            };
            Some((
                constant.value,
                constant.type_reference,
                constant
                    .normalization
                    .as_ref()
                    .map(|normalization| normalization.authored_expression),
            ))
        })
        .collect::<Vec<_>>();
    let frontier = ConstructorFrontier {
        parameters: Default::default(),
        prior_statements: &[],
    };
    for (value, destination, original) in constants {
        relabel_data_literal_for_expected_type(
            syntax,
            value,
            destination,
            instances,
            selection,
            &frontier,
        );
        if let Some(original) = original {
            relabel_data_literal_for_expected_type(
                syntax,
                original,
                destination,
                instances,
                selection,
                &frontier,
            );
        }
    }
}

/// An assignment target is another explicit destination type. Relabel a bare
/// generic literal only when that type is available directly from a local or
/// parameter; computed targets remain fail-closed.
pub(in crate::preparation::generic_data) fn relabel_closed_data_uses_in_exact_assignments(
    syntax: &mut SyntaxTrees,
    instances: &[Instantiation],
    selection: Option<&declaration_selection::ConstantSelection>,
    declarations: &[syntax_trees::item::ItemHandle],
) {
    for (state, local_types, statements) in concrete_state_local_types(syntax, declarations) {
        for (index, statement) in statements.iter().enumerate() {
            let frontier = ConstructorFrontier {
                parameters: state.parameters,
                prior_statements: &statements[..index],
            };
            let assignment = match syntax.statements.statement(*statement) {
                StatementNode::LocalData(local) => {
                    relabel_data_literal_for_expected_type(
                        syntax,
                        local.initial_value,
                        local.type_reference,
                        instances,
                        selection,
                        &frontier,
                    );
                    continue;
                }
                StatementNode::Assignment(assignment) => *assignment,
                _ => continue,
            };
            let expected_type = match syntax.expressions.expression(assignment.target) {
                ExpressionNode::Name(path) => {
                    let [name] = syntax.expressions.identifier_path_members(*path) else {
                        continue;
                    };
                    local_types.get(name.as_str()).copied()
                }
                _ => None,
            };
            if let Some(expected_type) = expected_type {
                relabel_data_literal_for_expected_type(
                    syntax,
                    assignment.value,
                    expected_type,
                    instances,
                    selection,
                    &frontier,
                );
            }
        }
    }
}
