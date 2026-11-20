//! Project the selected Build entry onto the statements checked at one preparation boundary.
//! At a straight-line wait the next program receives the live storage described
//! by its frame. Other bindings keep the current program running through their last use.

mod frame;
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
    program_ends: Vec<frame::Cut>,
    homes: Vec<Span>,
    saved_homes: Vec<Span>,
    pub(super) excluded_homes: Vec<usize>,
}

impl BuildStage {
    pub fn next_index(&self) -> Option<usize> {
        (self.index < self.program_ends.len()).then(|| self.index + 1)
    }

    /// Source declarations of the compiler-supplied storage arguments, in entry order.
    pub fn frame_homes(&self) -> &[Span] {
        &self.homes
    }

    /// The shared parameter list includes locals whose last use has passed.
    /// Only this wait's saved homes can supply values to the successor.
    pub fn saves_frame_home(&self, local: Span) -> bool {
        self.saved_homes
            .binary_search_by_key(&(local.start, local.end), |span| (span.start, span.end))
            .is_ok()
    }

    pub fn frame_parameter_name(home: Span) -> String {
        format!("$build.home.{}", home.start)
    }

    pub fn preparations(&self) -> &[Span] {
        &self.preparations
    }

    pub fn same_entry_plan(&self, other: &Self) -> bool {
        self.preparations == other.preparations
            && self.program_ends == other.program_ends
            && self.homes == other.homes
            && self.excluded_homes == other.excluded_homes
    }
}

const BUILD_ENTRY: &str = "build";
const BUILD_TYPE: &str = "Build";
const PREPARATION: &str = "prepare_dependency";

/// Keep already-resolved uses with the locals and facts they read. When typing
/// reports a missing declaration beyond a wait, the checked prefix ends at that
/// wait and materializes its live homes for the newly authorized scope.
pub(super) struct EntryPlan {
    item: ItemHandle,
    machine: Machine,
    state: StateHandle,
    statements: Vec<StatementHandle>,
    homes: Vec<usize>,
    stage: BuildStage,
    nested_calls: Vec<(usize, syntax_trees::identifier::Identifier)>,
    incoming: Option<usize>,
    first_statement: usize,
    earliest: usize,
    preferred: usize,
}

impl EntryPlan {
    pub(super) fn new(
        syntax: &SyntaxTrees,
        selected_entry: SourceSpan,
        start: usize,
        excluded_homes: &[usize],
    ) -> Result<Self, Vec<Diagnostic>> {
        let (item, machine, state) = entry(syntax, selected_entry).ok_or_else(|| {
            vec![Diagnostic::error(
                "selected Build entry is missing from the parsed stage",
            )]
        })?;
        let statements = syntax
            .items
            .statements(syntax.items.state(state).statements)
            .to_vec();
        let (positions, preparations): (Vec<_>, Vec<_>) = statements
            .iter()
            .enumerate()
            .filter_map(|(position, &statement)| {
                preparation(syntax, statement).map(|site| (position, site))
            })
            .unzip();
        let lexical_uses::LexicalPlan { cuts, nested_calls } = if positions.is_empty() {
            Default::default()
        } else {
            lexical_uses::program_ends(syntax, &machine, &statements, &positions)?
        };
        let mut cuts = cuts;
        if !excluded_homes.is_empty() {
            cuts.retain(|cut| {
                cut.bindings
                    .iter()
                    .all(|binding| excluded_homes.binary_search(&binding.home).is_err())
            });
        }
        if start > cuts.len() {
            return Err(vec![Diagnostic::error(
                "Build entry has no stage after its last dependency preparation",
            )]);
        }
        let incoming = start.checked_sub(1);
        let first_statement = incoming.map_or(0, |i| cuts[i].end + 1);
        // A later empty-frame boundary may belong to the same preparation we
        // just serviced. Only a wait in this program can authorize another cut.
        let mut eligible = (start..cuts.len()).filter(|&i| cuts[i].wait >= first_statement);
        let earliest = eligible.clone().next().unwrap_or(cuts.len());
        let preferred = eligible
            .find(|&i| cuts[i].bindings.is_empty())
            .unwrap_or(cuts.len());
        let mut homes = cuts
            .iter()
            .flat_map(|cut| cut.bindings.iter().map(|binding| binding.home))
            .collect::<Vec<_>>();
        homes.sort_unstable();
        homes.dedup();
        let stage = BuildStage {
            index: preferred,
            preparations,
            homes: homes
                .iter()
                .map(|&pos| {
                    frame::local(syntax, statements[pos])
                        .name
                        .source_span()
                        .span
                })
                .collect(),
            saved_homes: Vec::new(),
            excluded_homes: excluded_homes.to_vec(),
            program_ends: cuts,
        };
        Ok(Self {
            item,
            machine,
            state,
            statements,
            homes,
            stage,
            nested_calls,
            incoming,
            first_statement,
            earliest,
            preferred,
        })
    }

    /// The parser can only nominate a registration home. Its state parameter
    /// gives ordinary typing the same type occurrence and declaration scope as the local.
    pub(super) fn registration_candidates(&self, syntax: &SyntaxTrees) -> Vec<(usize, usize)> {
        self.homes
            .iter()
            .enumerate()
            .filter_map(|(slot, &home)| {
                let local = frame::local(syntax, self.statements[home]);
                syntax
                    .type_references
                    .type_reference(local.type_reference)
                    .as_named()
                    .is_some_and(|name| name.as_str(syntax) == "DependencyRegistration")
                    .then_some((slot + 1, home))
            })
            .collect()
    }

    /// Resolve just the candidate homes when errors in the speculative body
    /// prevented typing from returning its parameter declarations.
    pub(super) fn project_frame_parameters(
        &self,
        syntax: &mut SyntaxTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        syntax.items.state_mut(self.state).statements = arena::HandleSpan::empty();
        let mut machine = self.machine.clone();
        machine.nested_machines = arena::HandleSpan::empty();
        syntax.items.replace_item(self.item, Item::Machine(machine));
        frame::project_parameters(syntax, self)
    }

    pub(super) fn optional_wait(&self, syntax: &SyntaxTrees) -> Option<Span> {
        (self.earliest < self.preferred).then(|| {
            let wait = self.stage.program_ends[self.earliest].wait;
            preparation(syntax, self.statements[wait])
                .expect("a planned wait has an authored operation")
        })
    }

    pub(super) fn project(
        &self,
        syntax: &mut SyntaxTrees,
        use_frame: bool,
        facts: &[crate::ReturnedIntegerFact],
    ) -> Result<BuildStage, Vec<Diagnostic>> {
        let mut stage = self.stage.clone();
        stage.index = if use_frame {
            self.earliest
        } else {
            self.preferred
        };
        if stage.program_ends.is_empty() {
            return Ok(stage);
        }
        let outgoing = stage.program_ends.get(stage.index);
        if let Some(cut) = outgoing {
            stage.saved_homes = cut
                .bindings
                .iter()
                .map(|binding| {
                    frame::local(syntax, self.statements[binding.home])
                        .name
                        .source_span()
                        .span
                })
                .collect();
            stage
                .saved_homes
                .sort_unstable_by_key(|span| (span.start, span.end));
            stage.saved_homes.dedup();
        }
        let end = outgoing.map_or(self.statements.len(), |cut| cut.end + 1);
        if stage.next_index().is_some() {
            let diagnostics: Vec<_> = self.nested_calls.iter()
                .filter(|(pos, _)| (self.first_statement..end).contains(pos))
                .map(|(_, name)| Diagnostic::error(format!(
                    "not supported yet: a Build statement before `Build::prepare_dependency` that names `{}`, a nested machine checked with the final Build stage", name.as_str(syntax)
                )).with_source_span(name.source_span())).collect();
            if !diagnostics.is_empty() {
                return Err(diagnostics);
            }
        }
        let span = syntax.items.state(self.state).statements;
        let offset = u32::try_from(self.first_statement).expect("statement offset fits its span");
        let count =
            u32::try_from(end - self.first_statement).expect("statement count fits its span");
        syntax.items.state_mut(self.state).statements =
            arena::HandleSpan::from_parts_or_empty(span.handle_at(offset), count);
        let mut declaration = self.machine.clone();
        if !self.homes.is_empty() {
            frame::project(syntax, self, &stage, &mut declaration, facts)?;
        }
        if stage.next_index().is_some() {
            declaration.nested_machines = arena::HandleSpan::empty();
        }
        syntax
            .items
            .replace_item(self.item, Item::Machine(declaration));
        Ok(stage)
    }
}

#[cfg(test)]
fn select_build_stage(
    syntax: &mut SyntaxTrees,
    selected_entry: SourceSpan,
    stage: usize,
) -> Result<BuildStage, Vec<Diagnostic>> {
    EntryPlan::new(syntax, selected_entry, stage, &[])?.project(syntax, true, &[])
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
