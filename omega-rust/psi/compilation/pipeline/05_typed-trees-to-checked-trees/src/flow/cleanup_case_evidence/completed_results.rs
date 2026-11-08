//! Normal result branches retain the value that completed each selected arm.
//! Operand dependencies are not result branches and never select a death edge.

use super::OccurrenceContexts;
use arena::Handle;
use checked_trees::{CheckFacts, ContextualCleanupDisposition};
use checked_trees::{
    FlowClaimJoinExit, FlowOperandCompletion, FlowOperandFact, FlowOwnedCaseSelection,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMatchArm};
use validation::{CanonicalPlace, CheckingInput};

pub(crate) struct CaseSelections<'a> {
    facts: &'a CheckFacts,
    by_arm: BTreeMap<(u32, u32, u32, u32), Vec<Handle<FlowOwnedCaseSelection>>>,
    by_variant: BTreeMap<(u32, u32, u32, u32), Vec<Handle<FlowOwnedCaseSelection>>>,
}

impl<'a> CaseSelections<'a> {
    pub(crate) fn new(
        facts: &'a CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<Rc<Self>> {
        let control = &facts.flow.control;
        let state = control.state(machine, state)?;
        let mut by_arm = BTreeMap::<_, Vec<_>>::new();
        let mut by_variant = BTreeMap::<_, Vec<_>>::new();
        for statement in control.statements.span(state.statements)? {
            for handle in statement.owned_case_selections.handles() {
                if !control.owned_case_selections.is_valid(handle) {
                    return None;
                }
                let row = control.owned_case_selections.get(handle);
                by_arm
                    .entry((
                        row.expression.arena_index(),
                        row.expression.generation(),
                        row.source_arm.arena_index(),
                        row.source_arm.generation(),
                    ))
                    .or_default()
                    .push(handle);
                if row.miss_excludes_case {
                    by_variant
                        .entry((
                            row.expression.arena_index(),
                            row.expression.generation(),
                            row.variant.arena_index(),
                            row.variant.generation(),
                        ))
                        .or_default()
                        .push(handle);
                }
            }
        }
        Some(Rc::new(Self {
            facts,
            by_arm,
            by_variant,
        }))
    }
}

fn operand(
    control: &checked_trees::FlowControlFacts,
    handle: Handle<FlowOperandFact>,
) -> Option<&FlowOperandFact> {
    control
        .operands
        .is_valid(handle)
        .then(|| control.operands.get(handle))
}

#[derive(Clone)]
struct SelectionRoute {
    parent: Option<Rc<SelectionRoute>>,
    expression: ExpressionHandle,
    arm: Handle<TableMatchArm>,
}

pub(crate) struct CompletedResult<'a> {
    program: &'a dyn CheckingInput,
    facts: &'a CheckFacts,
    captures: Vec<Handle<FlowOperandFact>>,
    route: Option<Rc<SelectionRoute>>,
    cases: Rc<CaseSelections<'a>>,
    contexts: std::cell::OnceCell<Option<OccurrenceContexts>>,
}

/// Group only actual returned alternatives. Shared parents retain one route
/// node; the walk neither expands independent operands nor copies each prefix.
pub(crate) struct CompletedResultNode<'a> {
    pub(crate) expression: ExpressionHandle,
    pub(crate) result: CompletedResult<'a>,
    pub(crate) branches: BTreeMap<(u32, u32), usize>,
}

pub(crate) struct CompletedResults<'a> {
    pub(crate) nodes: Vec<CompletedResultNode<'a>>,
}

pub(crate) fn selected_results<'a>(
    program: &'a dyn CheckingInput,
    facts: &'a CheckFacts,
    exit: FlowClaimJoinExit,
    values: Vec<Handle<FlowOperandFact>>,
    cases: Rc<CaseSelections<'a>>,
) -> Option<CompletedResults<'a>> {
    if values.is_empty() || !std::ptr::eq(cases.facts, facts) {
        return None;
    }
    let control = &facts.flow.control;
    let mut pending = vec![(values, None::<Rc<SelectionRoute>>, 0usize)];
    let mut results = vec![None];
    let mut remaining = control
        .operands
        .len()
        .checked_add(control.operand_alternatives.len())?
        .checked_add(1)?;
    while let Some((captures, route, position)) = pending.pop() {
        remaining = remaining.checked_sub(1)?;
        let first = operand(control, *captures.first()?)?;
        let expression = first.expression;
        // Saved subjects and stored scalar aliases retain completed selected
        // contents. Only their actual Match occurrence produces new control.
        let selected = matches!(first.completion, FlowOperandCompletion::Selected(_))
            && expression.is_valid()
            && matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Match(_)
            );
        let mut branches =
            BTreeMap::<(u32, u32), (Handle<TableMatchArm>, Vec<Handle<FlowOperandFact>>)>::new();
        for capture in &captures {
            let value = operand(control, *capture)?;
            if value.expression != expression
                || value.state_symbol != exit.state_symbol
                || value.statement_index != exit.statement_index
                || value.evaluation != first.evaluation
                || value.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
            {
                return None;
            }
            if !selected {
                continue;
            }
            let FlowOperandCompletion::Selected(alternatives) = value.completion else {
                return None;
            };
            let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
            else {
                return None;
            };
            let alternatives = control.operand_alternatives.span(alternatives)?;
            if alternatives.is_empty() {
                return None;
            }
            let mut seen = BTreeSet::new();
            for alternative in alternatives {
                let arm = alternative.source_arm;
                let ordinal = arm
                    .arena_index()
                    .checked_sub(dispatch.arms.start().arena_index())?;
                if dispatch.arms.handles().nth(ordinal as usize) != Some(arm)
                    || !seen.insert((arm.arena_index(), arm.generation()))
                {
                    return None;
                }
                let authored = program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .get(ordinal as usize)?;
                let completed = operand(control, alternative.value)?;
                if completed.expression != authored.value {
                    return None;
                }
                let values = &mut branches
                    .entry((arm.arena_index(), arm.generation()))
                    .or_insert_with(|| (arm, Vec::new()))
                    .1;
                values.push(alternative.value);
            }
        }
        let mut children = BTreeMap::new();
        if selected {
            for (_, (arm, mut captures)) in branches.into_iter().rev() {
                captures.sort_unstable_by_key(|value| (value.arena_index(), value.generation()));
                captures.dedup();
                let child = results.len();
                results.push(None);
                children.insert((arm.arena_index(), arm.generation()), child);
                pending.push((
                    captures,
                    Some(Rc::new(SelectionRoute {
                        parent: route.clone(),
                        expression,
                        arm,
                    })),
                    child,
                ));
            }
        }
        results[position] = Some(CompletedResultNode {
            expression,
            result: CompletedResult {
                program,
                facts,
                captures,
                route,
                cases: cases.clone(),
                contexts: std::cell::OnceCell::new(),
            },
            branches: children,
        });
    }
    Some(CompletedResults {
        nodes: results.into_iter().collect::<Option<Vec<_>>>()?,
    })
}

impl CompletedResult<'_> {
    pub(super) fn contexts(&self) -> &Option<OccurrenceContexts> {
        let facts = self.facts;
        self.contexts.get_or_init(|| {
            self.captures
                .iter()
                .map(|capture| {
                    let value = operand(&facts.flow.control, *capture)?;
                    Some((
                        value.statement_index,
                        facts
                            .flow
                            .semantic_constraint_contexts(value.constraints)
                            .collect(),
                    ))
                })
                .collect()
        })
    }

    pub(super) fn belongs_to(
        &self,
        program: &dyn CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
    ) -> bool {
        // Result proofs read these immutable plans. A borrowed view must name
        // those same plans; the temporary view's own address has no identity.
        std::ptr::eq(self.program, program)
            && std::ptr::eq(&self.facts.semantic, facts.semantic)
            && std::ptr::eq(&self.facts.flow.control, facts.flow.control)
            && std::ptr::eq(&self.facts.flow.contexts, facts.flow.contexts)
    }

    pub(super) fn proves_case(
        &self,
        disposition: &ContextualCleanupDisposition,
        subject: &CanonicalPlace,
        variant: SymbolHandle,
        required: bool,
    ) -> bool {
        let program = self.program;
        let facts = self.facts;
        let Some(contexts) = self.contexts().as_ref() else {
            return false;
        };
        self.captures
            .iter()
            .zip(contexts)
            .all(|(capture, (_, contexts))| {
                let Some(value) = operand(&facts.flow.control, *capture) else {
                    return false;
                };
                if super::super::place_case_has_value(
                    program,
                    &facts.semantic,
                    contexts,
                    disposition.machine,
                    disposition.state,
                    value.statement_index,
                    subject,
                    variant,
                    required,
                ) {
                    return true;
                }
                let mut route = self.route.as_deref();
                while let Some(selection) = route {
                    let ExpressionNode::Match(dispatch) =
                        program.expression_table.expression(selection.expression)
                    else {
                        return false;
                    };
                    let expression = (
                        selection.expression.arena_index(),
                        selection.expression.generation(),
                    );
                    let current = self.cases.by_arm.get(&(
                        expression.0,
                        expression.1,
                        selection.arm.arena_index(),
                        selection.arm.generation(),
                    ));
                    let prior = self.cases.by_variant.get(&(
                        expression.0,
                        expression.1,
                        variant.arena_index(),
                        variant.generation(),
                    ));
                    for handle in current
                        .into_iter()
                        .flatten()
                        .chain(prior.into_iter().flatten())
                    {
                        let row = facts.flow.control.owned_case_selections.get(*handle);
                        if row.machine_symbol != disposition.machine
                            || row.state_symbol != disposition.state
                            || row.statement_index != disposition.exit.statement_index
                            || row.evaluation != value.evaluation
                            || row.expression != selection.expression
                            || row.root != subject.root
                            || facts.flow.control.operand_referent_segments.span(row.path)
                                != Some(subject.segments.as_slice())
                        {
                            continue;
                        }
                        let current = row.source_arm == selection.arm;
                        let prior = row.source_arm.generation() == selection.arm.generation()
                            && row.source_arm.arena_index() < selection.arm.arena_index()
                            && row.source_arm.arena_index() >= dispatch.arms.start().arena_index()
                            && row.miss_excludes_case;
                        let proves = if current {
                            (row.variant == variant && required)
                                || (row.variant != variant
                                    && !required
                                    && program.symbols.get(row.variant).parent
                                        == program.symbols.get(variant).parent)
                        } else {
                            prior && row.variant == variant && !required
                        };
                        if proves
                            && validation::completed_storage::contents_preserved_from(
                                program,
                                &facts.flow.control,
                                value.storage_snapshot,
                                subject,
                                &[row.selected_storage],
                                validation::completed_storage::StoredObservation::InlineContents,
                            )
                        {
                            return true;
                        }
                    }
                    route = selection.parent.as_deref();
                }
                false
            })
    }
}
