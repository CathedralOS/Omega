//! Project the selected Build entry onto the statements checked at one preparation boundary.
//! A preparation parks the live interpreter. End the checked program there only
//! when the remaining statements can start without its locals.

mod lexical_uses;
#[cfg(test)]
mod tests;

use diagnostics::Diagnostic;
use source_text::{SourceSpan, Span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::{Item, ItemHandle, Machine, StateHandle};
use syntax_trees::statement::{StatementHandle, StatementNode};

/// The same captured entry can receive different source-file IDs when the
/// next program is assembled. Keep the authored preparation spans and the
/// statement offsets where the interpreter can give up the entry frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStage {
    index: usize,
    preparations: Vec<Span>,
    program_ends: Vec<usize>,
}

impl BuildStage {
    pub fn next_index(&self) -> Option<usize> {
        (self.index < self.program_ends.len()).then(|| self.index + 1)
    }

    pub fn preparations(&self) -> &[Span] {
        &self.preparations
    }

    pub fn same_entry_plan(&self, other: &Self) -> bool {
        self.preparations == other.preparations && self.program_ends == other.program_ends
    }
}

const BUILD_ENTRY: &str = "build";
const BUILD_TYPE: &str = "Build";
const PREPARATION: &str = "prepare_dependency";

/// Keeps, in the selected Build entry, the statements of stage
/// `stage` only, and reports how many stages the entry has. An entry with no
/// top-level preparation is left as it is.
pub(crate) fn select_build_stage(
    syntax: &mut SyntaxTrees,
    selected_entry: SourceSpan,
    stage: usize,
) -> Result<BuildStage, Vec<Diagnostic>> {
    let (item, machine, state) = entry(syntax, selected_entry).ok_or_else(|| {
        vec![Diagnostic::error(
            "selected Build entry is missing from the parsed stage",
        )]
    })?;
    let span = syntax.items.state(state).statements;
    let statements = syntax.items.statements(span).to_vec();
    let (positions, preparations): (Vec<_>, Vec<_>) = statements
        .iter()
        .enumerate()
        .filter_map(|(position, &statement)| {
            preparation(syntax, statement).map(|site| (position, site))
        })
        .unzip();
    let program_ends = if positions.is_empty() {
        Vec::new()
    } else {
        lexical_uses::program_ends(syntax, &machine, &statements, &positions)?
    };
    let selected = BuildStage {
        index: stage,
        preparations,
        program_ends,
    };
    if stage > selected.program_ends.len() {
        return Err(vec![Diagnostic::error(
            "Build entry has no stage after its last dependency preparation",
        )]);
    }
    if selected.program_ends.is_empty() {
        return Ok(selected);
    }
    let first = stage
        .checked_sub(1)
        .map_or(0, |previous| selected.program_ends[previous] + 1);
    let end = selected
        .program_ends
        .get(stage)
        .map_or(statements.len(), |end| end + 1);
    let offset = u32::try_from(first).expect("a statement span is indexed by u32");
    let count = u32::try_from(end - first).expect("a statement span is counted by u32");
    syntax.items.state_mut(state).statements =
        arena::HandleSpan::from_parts_or_empty(span.handle_at(offset), count);
    if selected.next_index().is_some() {
        let mut declaration = machine;
        declaration.nested_machines = arena::HandleSpan::empty();
        syntax.items.replace_item(item, Item::Machine(declaration));
    }
    Ok(selected)
}

/// The selected Build declaration with the state that holds its
/// statements.
fn entry(
    syntax: &SyntaxTrees,
    selected_entry: SourceSpan,
) -> Option<(ItemHandle, Machine, StateHandle)> {
    syntax.root_item_handles().iter().find_map(|handle| {
        let machine = syntax.root_item(*handle).as_machine()?;
        if machine.attached_data.is_some()
            || machine.name.as_str(syntax) != BUILD_ENTRY
            || machine.name.source_span() != selected_entry
        {
            return None;
        }
        let state = *syntax.items.state_handles(machine.states).first()?;
        Some((*handle, machine.clone(), state))
    })
}

/// The authored operation name in a top-level dependency preparation.
fn preparation(syntax: &SyntaxTrees, statement: StatementHandle) -> Option<Span> {
    let target = match syntax.statements.statement(statement) {
        StatementNode::Call(call)
            if call.operational_acknowledgement.acknowledges_suspend
                && matches!(syntax.statements.identifier_path_members(call.receiver),
                [owner] if owner.as_str(syntax) == BUILD_TYPE) =>
        {
            &call.target
        }
        StatementNode::Expression(expression) => {
            let ExpressionNode::Call(call) = syntax.expressions.expression(*expression) else {
                return None;
            };
            if !call.operational_acknowledgement.acknowledges_suspend
                || !call.receiver.is_valid()
                || !matches!(syntax.expressions.expression(call.receiver),
                    ExpressionNode::Name { members, .. } if matches!(
                        syntax.expressions.identifier_path_members(*members),
                        [owner] if owner.as_str(syntax) == BUILD_TYPE))
            {
                return None;
            }
            &call.target
        }
        _ => return None,
    };
    (target.as_str(syntax) == PREPARATION).then(|| target.source_span().span)
}
