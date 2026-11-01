//! End anonymous descriptor inputs at their borrowing call's normal return.
//! Input custody distinguishes retained descriptor storage from its referent.
//! A returned child loan retains ancestry after an anonymous parent descriptor
//! retires; a result borrowing that descriptor's own storage keeps its home.
//! Fresh affine carriers retire in reverse production order. Named storage
//! and owned transfers retain their existing lifetime owners. The recursive
//! type capability authorizes descriptor-only retirement, including containing
//! aggregates, never disposal of borrowed contents or a nominal cleanup.
use super::super::types::ShapeCollector;
use checked_trees::{CheckedUnitCallPlan, CheckedUnitEffectOperationPlan};
use language_semantics::Multiplicity;

#[allow(clippy::too_many_arguments)]
pub(super) fn completions(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    results: &[(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
    invocation: &CheckedUnitCallPlan,
    preparation: &[CheckedUnitEffectOperationPlan],
    shapes: &ShapeCollector<'_>,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let mut completions = Vec::new();
    for (index, argument) in invocation.structural_arguments.iter().enumerate() {
        let index = u32::try_from(index).ok()?;
        if argument.access == checked_trees::CheckedStructuralAccess::Owned
            || invocation.structural_custody().is_some_and(|custody| {
                custody.retained_reference_inputs.contains(&index)
                    && !argument
                        .call_projection()
                        .contains(&checked_trees::CheckedUnitStructuralPathSegment::Referent)
            })
        {
            continue;
        }
        let ordinal = argument.source_structural_result_binding_ordinal();
        let Some(binding) = preparation
            .iter()
            .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
            .find(|binding| Some(binding.binding_ordinal) == ordinal)
        else {
            continue;
        };
        if binding.local.symbol.is_valid() || binding.multiplicity != Multiplicity::Affine {
            continue;
        }
        if invocation.structural_custody().is_some_and(|custody| {
            custody.retained_reference_inputs.iter().any(|index| {
                invocation
                    .structural_arguments
                    .get(*index as usize)
                    .is_some_and(|source| {
                        source.source_structural_result_binding_ordinal()
                            == Some(binding.binding_ordinal)
                            && !source.call_projection().contains(
                                &checked_trees::CheckedUnitStructuralPathSegment::Referent,
                            )
                    })
            })
        }) {
            continue;
        }
        let declaration = shapes.types.get(&binding.type_identity)?;
        if !declaration.descriptor_cleanup {
            continue;
        }
        let source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: binding.binding_ordinal,
        };
        let loans = super::super::control::statement_sequence::captured_reference_result(
            program,
            facts,
            machine,
            state,
            results,
            preparation
                .iter()
                .find(|operation| operation.structural_result_binding() == Some(binding))?,
            preparation,
        )
        .map(|captured| captured.loans)
        .unwrap_or_default();
        if completions.iter().any(|(_, completion)| matches!(completion,
            CheckedUnitEffectOperationPlan::ReleaseReference { source: prior, .. } if *prior == source)) { continue; }
        completions.push((
            binding.binding_ordinal,
            CheckedUnitEffectOperationPlan::ReleaseReference {
                statement_index: binding.statement_index,
                source,
                path: Vec::new(),
                select_evaluated_storage: false,
                defer_to_exit: false,
                loans,
                selection_complements: Vec::new(),
                owner_cleanups: arena::HandleSpan::empty(),
                case_conditions: Default::default(),
            },
        ));
    }
    completions.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    Some(
        completions
            .into_iter()
            .map(|(_, completion)| completion)
            .collect(),
    )
}
