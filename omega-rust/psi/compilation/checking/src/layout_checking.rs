//! Universal layout judgments borrow one exact alternative at a time.
//!
//! This view never changes the typed program or deployment selection. Every
//! nested representation query sees the same target guard, and the operation
//! publishes evidence only after every relevant guard succeeds. Geometry
//! without a guard is available only for invariant applications.

use crate::{CheckingInput, CheckingMemos, PreparedEffectPlans};
use language_semantics::declaration_selection::AuthoredDeclarationSelections;
use std::sync::Arc;
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{PlanLaidLayout, PlanLaidRealizations, TypedTrees};

#[derive(Debug)]
pub struct LayoutCheckingContext<'a> {
    input: &'a dyn CheckingInput,
    target: Option<&'a str>,
}

impl<'a> LayoutCheckingContext<'a> {
    pub fn new(input: &'a dyn CheckingInput, target: Option<&'a str>) -> Self {
        Self { input, target }
    }

    /// Authored bodies own their target guard even in a portable library.
    /// An enclosing exact guard propagates through an unscoped helper.
    pub fn for_owner(input: &'a dyn CheckingInput, owner: symbols::SymbolHandle) -> Self {
        let typed = input.typed_program();
        let authored = typed
            .machine_by_symbol(owner)
            .or_else(|| typed.machine_holding_state(owner))
            .and_then(|machine| typed.machine_authored_target(machine));
        Self::new(input, input.layout_target().or(authored))
    }
}

impl CheckingInput for LayoutCheckingContext<'_> {
    fn retained_borrow_custodies(&self) -> &[checked_trees::RetainedBorrowCustodyFact] {
        self.input.retained_borrow_custodies()
    }

    fn typed_program(&self) -> &TypedTrees {
        self.input.typed_program()
    }
    fn layout_target(&self) -> Option<&str> {
        self.target
    }
    fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
        self.input.authored_declaration_selections()
    }
    // A changed guard cannot borrow unguarded representation judgments.
    // Merely carrying the same owner/context need not discard its memos.
    fn checking_memos(&self) -> Option<&CheckingMemos> {
        if self.target == self.input.layout_target() {
            self.input.checking_memos()
        } else {
            None
        }
    }
    fn proof_only_classification(&self) -> Arc<ProofOnlyClassification> {
        // Containment and erasure are fixed by the typed program, not this
        // geometry guard. Share only this query, never the guarded memos.
        self.input.proof_only_classification()
    }
    fn prepared_effect_plans(&self) -> Option<&PreparedEffectPlans> {
        self.input.prepared_effect_plans()
    }
    fn prepared_mathematics(
        &self,
    ) -> Option<crate::mathematical_applications::PreparedMathematicalView<'_>> {
        self.input.prepared_mathematics()
    }
    fn opaque_representation_selections(
        &self,
    ) -> &[representation_selections::OpaqueRepresentationSelection] {
        self.input.opaque_representation_selections()
    }
}

pub(crate) fn layout_application_is_complete(
    program: &dyn CheckingInput,
    layout: &PlanLaidLayout,
) -> bool {
    let span = match layout.realizations {
        PlanLaidRealizations::Pending => return false,
        PlanLaidRealizations::TargetFamily(span) => span,
        PlanLaidRealizations::TargetContexts { contexts, layouts } => {
            let (Some(contexts), Some(rows)) = (
                program.plan_laid_contexts.span(contexts),
                program.plan_laid_target_layouts.span(layouts),
            ) else {
                return false;
            };
            return !contexts.is_empty()
                && contexts.len() == rows.len()
                && contexts
                    .windows(2)
                    .all(|pair| pair[0].as_str() < pair[1].as_str())
                && contexts.iter().zip(rows).all(|(context, row)| {
                    context == &row.target && row.machine == layout.policy_plan_machine_symbol
                });
        }
        _ => return true,
    };
    let Some(rows) = program.plan_laid_target_layouts.span(span) else {
        return false;
    };
    let Some(machine) = program.machine(layout.policy_plan_machine_symbol) else {
        return false;
    };
    let Some(members) = crate::machine_calls::calls::target_family_members(program, machine) else {
        return false;
    };
    !rows.is_empty()
        && rows.len() == members.len()
        && rows.iter().enumerate().all(|(index, row)| {
            !rows[..index]
                .iter()
                .any(|previous| previous.target == row.target || previous.machine == row.machine)
                && members
                    .iter()
                    .any(|member| member.target == row.target && member.machine == row.machine)
        })
}

/// Guards needed by the exact participating types, including nested records.
/// Unrelated family applications elsewhere in the package impose no guard.
/// A caller already checking one alternative retains it through nested queries.
pub fn layout_checking_targets(
    program: &dyn CheckingInput,
    roots: impl IntoIterator<Item = TypeReferenceHandle>,
) -> Result<Vec<Option<&str>>, ()> {
    if let Some(target) = program.layout_target() {
        return Ok(vec![Some(target)]);
    }
    let roots: Vec<_> = roots.into_iter().collect();
    let memos = crate::frozen_program::frozen_program_memos(program);
    if let Some(memos) = memos
        && let Ok(rosters) = memos.layout_target_rosters.lock()
        && let Some(roster) = rosters.get(&roots)
    {
        return roster
            .as_ref()
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        row.map(|index| {
                            program.plan_laid_target_layouts.storage_slice()[index]
                                .target
                                .as_str()
                        })
                    })
                    .collect()
            })
            .map_err(|_| ());
    }
    let result = discover_layout_checking_targets(program, roots.iter().copied());
    if let Some(memos) = memos {
        // Discovery returns only target names borrowed from these exact rows.
        // Retain their storage positions so lookup can reborrow from `program`.
        let roster = result
            .as_ref()
            .map(|targets| {
                targets
                    .iter()
                    .map(|target| {
                        target.map(|target| {
                            program
                                .plan_laid_target_layouts
                                .storage_slice()
                                .iter()
                                .position(|row| std::ptr::eq(row.target.as_str(), target))
                                .expect("layout target discovery returns an exact stored row")
                        })
                    })
                    .collect()
            })
            .map_err(|_| ());
        if let Ok(mut rosters) = memos.layout_target_rosters.lock() {
            rosters.insert(roots, roster);
        }
    }
    result
}

fn discover_layout_checking_targets(
    program: &dyn CheckingInput,
    roots: impl IntoIterator<Item = TypeReferenceHandle>,
) -> Result<Vec<Option<&str>>, ()> {
    let mut pending: Vec<_> = roots.into_iter().collect();
    let mut visited = Vec::new();
    let mut targets: Option<Vec<Option<&str>>> = None;
    while let Some(reference) = pending.pop() {
        if !reference.is_valid() || visited.contains(&reference) {
            continue;
        }
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err(());
        }
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            _ => {
                let Some(data) =
                    crate::value_custody::places::data_definition_for_type(program, reference)
                else {
                    continue;
                };
                if let Some(layout) = program.plan_laid_layout(data.symbol) {
                    if !layout_application_is_complete(program, layout) {
                        return Err(());
                    }
                    let rows = layout.target_layouts(program.typed_program());
                    if !rows.is_empty() {
                        // Every operand must exist in the same context. Input
                        // preparation has already enforced mandatory demands;
                        // absence there cannot disappear at this domain join.
                        match &mut targets {
                            None => {
                                targets =
                                    Some(rows.iter().map(|row| Some(row.target.as_str())).collect())
                            }
                            Some(targets) => targets.retain(|target| {
                                rows.iter().any(|row| *target == Some(row.target.as_str()))
                            }),
                        }
                        if targets.as_ref().is_some_and(Vec::is_empty) {
                            return Err(());
                        }
                    }
                }
                pending.extend(
                    program
                        .data_member_fields(program.data_members(data))
                        .filter(|field| !field.relevance.is_erased())
                        .map(|field| field.type_reference),
                );
            }
        }
    }
    Ok(targets.unwrap_or_else(|| vec![None]))
}
