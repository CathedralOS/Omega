//! Select the checked entry prefix ending at an authored dependency preparation.

mod demand;
#[cfg(test)]
mod tests;

use diagnostics::Diagnostic;
use source_text::{SourceId, SourceSpan, Span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::StateHandle;
use syntax_trees::statement::{StatementHandle, StatementNode};

const BUILD_ENTRY: &str = "build";
const BUILD_TYPE: &str = "Build";
const PREPARATION: &str = "prepare_dependency";

/// An authored Build import for which the loader has no selected module binding yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingBuildImport {
    pub source: SourceId,
    pub span: Span,
    pub target: crate::SourceImportTarget,
}

/// An authored suspending call and the captured declaration that owns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPreparationSite {
    pub owner: source_admission::RetainedDeclaration,
    pub call: Span,
    pub projects_suffix: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildStage {
    incoming: Vec<BuildPreparationSite>,
    outgoing: Vec<Vec<BuildPreparationSite>>,
    advisories: Vec<Diagnostic>,
    can_return: bool,
}

impl BuildStage {
    /// A complete checked alternative can return without entering this preparation's arm.
    pub fn can_finish_before_preparation(&self) -> bool {
        self.can_return
    }

    pub fn unused_import_advisories(&self) -> &[Diagnostic] {
        &self.advisories
    }

    pub(super) fn report_unused_imports(
        &mut self,
        typed: &crate::source_state::TypedTrees,
        imports: &[PendingBuildImport],
    ) {
        if !self.outgoing.is_empty() || imports.is_empty() {
            return;
        }
        // An admitted final stage used only declarations in its selected scope.
        // An unchecked use in a dormant declaration still needs its import.
        let mut incomplete_files = typed
            .deferred_refusals
            .iter()
            .map(|row| row.source_span.source_id.0)
            .collect::<Vec<_>>();
        incomplete_files.sort_unstable();
        incomplete_files.dedup();
        let product_uses = PendingBuildImport::used_by_product(imports, typed);
        for (import, used) in imports.iter().zip(product_uses) {
            if !used && incomplete_files.binary_search(&import.source.0).is_err() {
                let diagnostic =
                    Diagnostic::warning(format!("unused deferred Build import `{}`", import.target))
                        .with_source_span(SourceSpan::new(import.source, import.span));
                self.advisories.push(diagnostic);
            }
        }
    }

    pub fn outgoing_preparation_path(&self) -> Option<&[BuildPreparationSite]> {
        let [only] = self.outgoing.as_slice() else {
            return None;
        };
        Some(only)
    }
    pub fn outgoing_preparation_paths(&self) -> &[Vec<BuildPreparationSite>] {
        &self.outgoing
    }

    /// Commit the one arm that the interpreter actually reached.
    pub fn commit_preparation(&mut self, index: usize) -> Result<(), &'static str> {
        if index >= self.outgoing.len() {
            return Err("the Build reached a preparation outside its admitted paths");
        }
        let chosen = self.outgoing.swap_remove(index);
        self.outgoing.clear();
        self.outgoing.push(chosen);
        Ok(())
    }

    pub fn incoming_preparation_path(&self) -> Option<&[BuildPreparationSite]> {
        (!self.incoming.is_empty()).then_some(&self.incoming)
    }
    #[cfg(test)]
    fn outgoing_parked_preparation(&self) -> Option<Span> {
        self.outgoing_preparation_path()?
            .last()
            .map(|site| site.call)
    }
}

struct AuthoredCall {
    owner: SourceSpan,
    call: Span,
    state: StateHandle,
    through: usize,
    prepares: bool,
    terminal_arm: bool,
}

pub(super) struct EntryPlan {
    entry: SourceSpan,
    calls: Vec<AuthoredCall>,
    truncate: Vec<usize>,
    pub(super) stage: BuildStage,
}

impl EntryPlan {
    pub(super) fn new(
        syntax: &mut SyntaxTrees,
        entry: SourceSpan,
        previous: Option<&BuildStage>,
        has_pending_imports: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        let incoming = match previous {
            Some(stage) if stage.outgoing.is_empty() => {
                return Err(vec![Diagnostic::error(
                    "the selected Build stage has already finished",
                )]);
            }
            Some(stage) => stage
                .outgoing_preparation_path()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "the Build must identify the preparation it reached before resuming",
                    )]
                })?
                .to_vec(),
            None => Vec::new(),
        };
        if has_pending_imports {
            // A transition evaluates each subject in a local before it selects an arm.
            // Plan and project the same statements that name resolution will check.
            syntax_trees_to_symbol_resolved_trees::resolution::prepare_dispatches(syntax);
        }
        let mut calls = Vec::new();
        let selected = syntax.root_item_handles().iter().any(|handle| {
            syntax
                .root_item(*handle)
                .as_machine()
                .is_some_and(|machine| {
                    machine.name.source_span() == entry
                        && machine.name.as_str(syntax) == BUILD_ENTRY
                        && machine.attached_data.is_none()
                })
        });
        let mut machines = if has_pending_imports {
            syntax.root_item_handles().to_vec()
        } else {
            Vec::new()
        };
        while let Some(handle) = machines.pop() {
            let Some(machine) = syntax.items.item(handle).as_machine() else {
                continue;
            };
            let owner = machine.name.source_span();
            machines.extend(machine.nested_machines.handles());
            for &state in syntax.items.state_handles(machine.states) {
                let statements = syntax
                    .items
                    .statements(syntax.items.state(state).statements);
                // Arms share the end of their dispatch. A subject or an ordinary
                // statement can stop the stage immediately after its own evaluation.
                let mut through = statements.len();
                for (position, &statement) in statements.iter().enumerate().rev() {
                    let node = syntax.statements.statement(statement);
                    if !matches!(node, StatementNode::Transition(arm) if !arm.ends_dispatch) {
                        through = position + 1;
                    }
                    if let Some(call) = suspending_call(syntax, statement) {
                        calls.push(AuthoredCall {
                            owner,
                            call,
                            state,
                            through,
                            prepares: preparation(syntax, statement).is_some(),
                            terminal_arm: matches!(node, StatementNode::Transition(_)),
                        });
                    }
                }
            }
        }
        if !selected {
            return Err(vec![Diagnostic::error(
                "selected Build entry is missing from the parsed stage",
            )]);
        }
        calls.sort_unstable_by_key(|call| (call.owner.source_id.0, call.call.end));
        Ok(Self {
            entry,
            calls,
            truncate: Vec::new(),
            stage: BuildStage {
                incoming,
                outgoing: Vec::new(),
                advisories: Vec::new(),
                can_return: false,
            },
        })
    }

    pub(super) fn can_park(&self) -> bool {
        self.calls.iter().any(|call| call.prepares)
    }

    fn call_at(&self, owner: SourceSpan, site: Span) -> Option<usize> {
        // A resolved module call can include its qualifier in the target span.
        // Its final identifier and enclosing declaration keep their authored coordinates.
        let row = self.calls.partition_point(|call| {
            (call.owner.source_id.0, call.call.end) < (owner.source_id.0, site.end)
        });
        self.calls[row..]
            .iter()
            .take_while(|call| {
                (call.owner.source_id.0, call.call.end) == (owner.source_id.0, site.end)
            })
            .position(|call| call.owner == owner && site.start <= call.call.start)
            .map(|offset| row + offset)
    }

    pub(super) fn select(
        &mut self,
        trees: &symbol_resolved_trees::source_resolved::SymbolResolvedTrees,
        imports: &[PendingBuildImport],
    ) -> Result<Vec<crate::source_state::ParkedBuildLocals>, Vec<Diagnostic>> {
        let selected = demand::select(trees, self, imports)?;
        let mut parked = Vec::new();
        self.stage.can_return = selected.can_return;
        for path in selected.paths {
            let mut sites = Vec::with_capacity(path.len());
            for frame in path {
                let call = &self.calls[frame.authored];
                let owner = crate::source_symbols::retain_declaration(&trees.symbols, frame.owner)
                    .map_err(|why| vec![Diagnostic::error(why).with_source_span(call.owner)])?;
                sites.push(BuildPreparationSite {
                    owner,
                    call: call.call,
                    projects_suffix: frame.needs_projection,
                });
                if frame.needs_projection && !self.truncate.contains(&frame.authored) {
                    if self
                        .truncate
                        .iter()
                        .any(|&index| self.calls[index].state == call.state)
                    {
                        return Err(vec![Diagnostic::error("two preparation paths require different checked prefixes of the same machine body").with_source_span(call.owner)]);
                    }
                    let state = trees.machine_state(frame.state);
                    let state_source = trees.symbols.symbol_source_span(state.symbol);
                    if state_source.is_none()
                        && trees
                            .machine(frame.owner)
                            .and_then(|machine| trees.machine_state_handles(machine.states).first())
                            != Some(&frame.state)
                    {
                        return Err(vec![
                            Diagnostic::error(
                                "the suspended Build body has no unique authored state or entry",
                            )
                            .with_source_span(call.owner),
                        ]);
                    }
                    self.truncate.push(frame.authored);
                    parked.push(crate::source_state::ParkedBuildLocals {
                        entry: call.owner,
                        state: state_source,
                        pending_completion_at: SourceSpan::new(call.owner.source_id, call.call),
                        declarations: frame.declarations,
                        deferred_subjects: frame.deferred_subjects,
                    });
                }
            }
            self.stage.outgoing.push(sites);
        }
        Ok(parked)
    }

    pub(super) fn project(&self, syntax: &mut SyntaxTrees) {
        for &index in &self.truncate {
            let call = &self.calls[index];
            let span = syntax.items.state(call.state).statements;
            let count = u32::try_from(call.through).expect("a statement span fits its arena");
            syntax.items.state_mut(call.state).statements =
                arena::HandleSpan::from_parts_or_empty(span.start(), count);
        }
    }
}

fn suspending_call(syntax: &SyntaxTrees, statement: StatementHandle) -> Option<Span> {
    let expression = match syntax.statements.statement(statement) {
        StatementNode::Call(call) => {
            return call
                .operational_acknowledgement
                .acknowledges_suspend
                .then(|| call.target.source_span().span);
        }
        StatementNode::Transition(arm) if !arm.continuation.is_valid() && arm.target.is_valid() => {
            return suspending_arm(syntax, arm.target);
        }
        StatementNode::LocalData(local) => local.initial_value,
        StatementNode::Expression(expression) => *expression,
        _ => return None,
    };
    if !expression.is_valid() {
        return None;
    }
    let ExpressionNode::Call(call) = syntax.expressions.expression(expression) else {
        return None;
    };
    call.operational_acknowledgement
        .acknowledges_suspend
        .then(|| call.target.source_span().span)
}
fn suspending_arm(
    syntax: &SyntaxTrees,
    target: syntax_trees::statement::TransitionTargetHandle,
) -> Option<Span> {
    match syntax.statements.transition_target(target) {
        syntax_trees::statement::TransitionTargetNode::Named {
            operational_acknowledgement,
            path,
            ..
        } if operational_acknowledgement.acknowledges_suspend => syntax
            .statements
            .identifier_path_members(*path)
            .last()
            .map(|name| name.source_span().span),
        syntax_trees::statement::TransitionTargetNode::Value(value) => {
            let ExpressionNode::Call(call) = syntax.expressions.expression(*value) else {
                return None;
            };
            call.operational_acknowledgement
                .acknowledges_suspend
                .then(|| call.target.source_span().span)
        }
        _ => None,
    }
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
