//! Capture checked target-layout extents without demanding executable policy
//! bodies. Layout evaluation already owns their validated geometry; task
//! storage needs that result, not a fabricated module-local machine roster.
use crate::unit::attached_unit::{bodies::UnitPlans, catalog::selected_declaration};
use checked_trees::{CheckedTrees, CheckedUnitRecordStorage, CheckedUnitStructuralTypeShape};
use diagnostics::Diagnostic;
use terminal_psi::*;

pub(super) fn project(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    types: &[StructuralTypeDeclaration],
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
) -> Result<Vec<TerminalTaskTargetLayout>, Vec<Diagnostic>> {
    let failure = |message: &'static str| vec![Diagnostic::error(message)];
    let mut layouts = Vec::new();
    for record in types {
        let StructuralTypeShape::Record {
            storage: StructuralRecordStorage::TargetLayout(id),
            ..
        } = &record.shape
        else {
            continue;
        };
        let identity = type_ids
            .iter()
            .find_map(|(identity, candidate)| (*candidate == record.id).then_some(*identity))
            .ok_or_else(|| failure("task layout lost its exact checked type"))?;
        let source = selected_declaration(plans, identity).map_err(super::lowering_diagnostics)?;
        let CheckedUnitStructuralTypeShape::Record {
            storage: CheckedUnitRecordStorage::TargetLayout { application, .. },
            ..
        } = &source.shape
        else {
            return Err(failure("task layout lost its exact checked application"));
        };
        let plan = checked
            .typed
            .plan_laid_layout(*application)
            .ok_or_else(|| failure("task layout lost its evaluated policy"))?;
        let typed_trees::PlanLaidRealizations::TargetFamily(span) = plan.realizations else {
            return Err(failure("task target layout names an invariant policy"));
        };
        let alternatives = checked
            .typed
            .plan_laid_target_layouts
            .span(span)
            .ok_or_else(|| failure("task layout lost its target alternatives"))?;
        let family = checked
            .typed
            .target_machine_families
            .iter()
            .find(|family| family.canonical == plan.policy_plan_machine_symbol)
            .ok_or_else(|| failure("task layout lost its policy family"))?;
        if alternatives.is_empty() || alternatives.len() != family.members.len() {
            return Err(failure(
                "task layout does not cover its complete policy family",
            ));
        }
        let mut realizations = Vec::new();
        for (alternative, member) in alternatives.iter().zip(&family.members) {
            if alternative.target != member.target || alternative.machine != member.machine {
                return Err(failure(
                    "task layout alternative changed exact policy member custody",
                ));
            }
            let geometry = &alternative.geometry;
            let byte_size = geometry
                .validated_layout
                .size
                .ok_or_else(|| failure("task layout alternative has no fixed extent"))?;
            if usize::try_from(byte_size).ok() != Some(geometry.size)
                || usize::try_from(geometry.validated_layout.align).ok() != Some(geometry.align)
            {
                return Err(failure(
                    "task layout extent differs from its validated geometry",
                ));
            }
            realizations.push(TerminalTaskLayoutRealization {
                target: alternative.target.as_str().to_owned(),
                storage: semantic_vocabulary::AuthoredRecordStorage {
                    byte_size,
                    byte_alignment: geometry.validated_layout.align,
                },
            });
        }
        realizations.sort_by(|left, right| left.target.cmp(&right.target));
        layouts.push(TerminalTaskTargetLayout {
            id: *id,
            record: record.id,
            realizations,
        });
    }
    layouts.sort_by_key(|layout| layout.id);
    Ok(layouts)
}
