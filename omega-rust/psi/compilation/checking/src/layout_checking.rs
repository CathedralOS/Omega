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
}

impl CheckingInput for LayoutCheckingContext<'_> {
    fn defers_pending_layout_geometry(&self) -> bool {
        self.input.defers_pending_layout_geometry()
    }
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
    // Memos were prepared without this guard. Reusing them could mix distinct
    // representation judgments even though the semantic source is unchanged.
    fn checking_memos(&self) -> Option<&CheckingMemos> {
        None
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

pub(crate) fn layout_family_is_complete(
    program: &dyn CheckingInput,
    layout: &PlanLaidLayout,
) -> bool {
    let PlanLaidRealizations::TargetInputs(span) = layout.realizations else {
        return true;
    };
    let Some(rows) = program.plan_laid_target_layouts.span(span) else {
        return false;
    };
    rows.len() == layout.target_inputs.len()
        && rows
            .iter()
            .zip(&layout.target_inputs)
            .enumerate()
            .all(|(index, (row, input))| {
                row.target_facts == *input
                    && row.machine == layout.policy_plan_machine_symbol
                    && row.target.as_str() == row.target_facts.profile
                    && row.target_facts.validate().is_ok()
                    && !rows[..index]
                        .iter()
                        .any(|previous| previous.target == row.target)
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
                if let Some(layout) = program.plan_laid_layout_for_type(reference) {
                    if !layout_family_is_complete(program, layout) {
                        return Err(());
                    }
                    if matches!(layout.realizations, PlanLaidRealizations::TargetInputs(_)) {
                        let alternatives = layout
                            .target_layouts(program.typed_program())
                            .iter()
                            .map(|row| Some(row.target.as_str()))
                            .collect::<Vec<_>>();
                        // One physical judgment needs a common application of
                        // every participating representation. An empty roster
                        // describes retained, unapplied storage, not packing.
                        match &mut targets {
                            Some(targets) => targets.retain(|target| alternatives.contains(target)),
                            None => targets = Some(alternatives),
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

/// The normalized constructor occurrence is the pending obligation. Walk exact
/// semantic members, not erased implementation arguments or nominal headers.
/// No caller may use this test to claim geometry: it only withholds preliminary
/// physical judgments until policy evaluation installs their complete inputs.
pub(crate) fn pending_layout_geometry(
    program: &dyn CheckingInput,
    roots: impl IntoIterator<Item = TypeReferenceHandle>,
) -> bool {
    let mut work = roots.into_iter().collect::<Vec<_>>();
    let mut visited = std::collections::BTreeSet::new();
    while let Some(reference) = work.pop() {
        if !reference.is_valid() || !visited.insert(program.normalized_type_identity(reference)) {
            continue;
        }
        if let Some(storage) = program.layout_storage_arguments(reference) {
            if program
                .plan_laid_layout_for_type(storage.application)
                .is_none()
            {
                return true;
            }
            work.push(storage.semantic_type);
            continue;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference {
                referee: base_type, ..
            }
            | TypeReferenceNode::FixedArray {
                element_type: base_type,
                ..
            }
            | TypeReferenceNode::Slice {
                element_type: base_type,
            } => work.push(*base_type),
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                if let Some(data) = program.closed_data_definition(reference) {
                    work.extend(
                        program
                            .live_data_fields(program.data_members(data))
                            .map(|field| field.type_reference),
                    );
                }
            }
            _ => {}
        }
    }
    false
}
