//! A call spelled `select_provider` inside an `Owner::provider_defaults`
//! machine is the build provider-selection intrinsic. Typing finalizes every
//! such authored call selection, for the realized target's declaration and
//! for its target siblings alike, so checking admits those bodies wherever
//! they are typed (the product route, a generated-source extension, or a
//! build-time evaluation probe). Nothing here grants a selection: the build
//! layer harvests only the realized target's declaration.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic as Intrinsic, BuildOperation,
};
use typed_trees::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding as Binding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget as Target,
    TypedTrees,
};

pub(crate) fn finalize_provider_default_calls(typed: &mut TypedTrees) -> Result<(), Diagnostic> {
    let occurrences = typed
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str().ends_with("::provider_defaults"))
        .flat_map(|machine| provider_default_call_occurrences(typed, machine))
        .collect::<Vec<_>>();
    if occurrences.is_empty() {
        return Ok(());
    }
    let mut selections = typed.authored_declaration_selections().clone();
    for occurrence in occurrences {
        if selections.get(occurrence).is_some_and(|selection| {
            selection.target() == Target::Intrinsic(Intrinsic::BuildProviderSelection)
        }) {
            continue;
        }
        selections
            .finalize_intrinsic(
                occurrence,
                Binding::CheckedCall,
                Intrinsic::BuildProviderSelection,
            )
            .map_err(|error| {
                Diagnostic::error(format!(
                    "provider-default call lost its authored selection custody: {error:?}"
                ))
            })?;
    }
    typed.retain_authored_declaration_selections(selections);
    Ok(())
}

fn provider_default_call_occurrences(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<AuthoredDeclarationSelectionOccurrenceId> {
    let mut occurrences = Vec::new();
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                typed_trees::statement::StatementNode::Call(call)
                    if BuildOperation::from_call_target(call.target.as_str())
                        == Some(BuildOperation::ProviderSelection)
                        && !call.target_symbol.is_valid() =>
                {
                    occurrences.extend(call.authored_call_selection);
                }
                typed_trees::statement::StatementNode::Expression(expression) => {
                    if let typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                        && BuildOperation::from_call_target(call.target.as_str())
                            == Some(BuildOperation::ProviderSelection)
                        && !call.target_symbol.is_valid()
                    {
                        occurrences.extend(
                            typed
                                .expression_table
                                .authored_selection_occurrences(*expression)
                                .filter(|occurrence| {
                                    typed
                                        .authored_declaration_selections()
                                        .get(*occurrence)
                                        .is_some_and(|selection| {
                                            selection.kind()
                                                == AuthoredDeclarationSelectionKind::Call
                                        })
                                }),
                        );
                    }
                }
                _ => {}
            }
        }
    }
    occurrences
}
