//! End anonymous descriptor inputs at their borrowing call's normal return.
//! Input custody distinguishes retained descriptor storage from its referent.
//! A returned child loan retains ancestry after an anonymous parent descriptor
//! retires; a result borrowing that descriptor's own storage keeps its home.
//! Fresh affine descriptors retire in reverse production order. Named storage
//! and owned transfers retain their existing lifetime owners. Type capability authorizes descriptor-only
//! retirement, never disposal of borrowed contents or a nominal cleanup.
use super::super::types::ShapeCollector;
use checked_trees::{CheckedUnitCallPlan, CheckedUnitEffectOperationPlan};
use language_semantics::Multiplicity;

pub(in crate::execution::terminal_unit) fn temporary_descriptor_completions(
    invocation: &CheckedUnitCallPlan,
    preparation: &[CheckedUnitEffectOperationPlan],
    shapes: &ShapeCollector<'_>,
) -> Option<checked_trees::CheckedOperandSequence> {
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
        let descriptor = matches!(
            declaration.shape,
            checked_trees::CheckedUnitStructuralTypeShape::Reference { .. }
                | checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView { .. }
                | checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                    checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
                        | checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }
                )
        );
        if !descriptor || !declaration.descriptor_cleanup {
            continue;
        }
        let source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: binding.binding_ordinal,
        };
        if completions.iter().any(|completion| matches!(completion,
            CheckedUnitEffectOperationPlan::ReleaseReference { source: prior, .. } if *prior == source)) { continue; }
        completions.push(CheckedUnitEffectOperationPlan::ReleaseReference {
            statement_index: binding.statement_index,
            source,
            path: Vec::new(),
            select_evaluated_storage: false,
            defer_to_exit: false,
            loans: Default::default(),
            selection_complements: Vec::new(),
            case_conditions: Default::default(),
        });
    }
    completions.sort_by_key(|completion| match completion {
        CheckedUnitEffectOperationPlan::ReleaseReference {
            source:
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                },
            ..
        } => std::cmp::Reverse(*binding_ordinal),
        _ => unreachable!("call completion contains descriptor releases"),
    });
    Some(checked_trees::CheckedOperandSequence::new(completions))
}
