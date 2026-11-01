//! Publish checked nominal associations and complete native layout provenance.
//! No target is selected and no policy body is evaluated during this projection.

use super::{CheckedTrees, LoweringError, UnitPlans, unsupported};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    NativeLayoutCatalog, NativeLayoutFieldIdentity, NativeLayoutRealization, StructuralTypeShape,
    TargetLayoutInput, TerminalIdentityTableBuilder,
};
use typed_trees::type_identity::TypeIdentityId;

pub(super) fn lower(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(TypeIdentityId, StructuralTypeId)],
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<NativeLayoutCatalog, LoweringError> {
    let mut catalog = NativeLayoutCatalog::default();
    for record in types {
        let StructuralTypeShape::Record { fields, .. } = &record.shape else {
            continue;
        };
        let source_identity = type_ids
            .iter()
            .find_map(|(source, id)| (*id == record.id).then_some(*source))
            .ok_or(LoweringError::Unsupported(
                "native record lost its checked identity association",
            ))?;
        let source = super::catalog::selected_declaration(plans, source_identity)?;
        let checked_trees::CheckedUnitStructuralTypeShape::Record {
            fields: source_fields,
            ..
        } = &source.shape
        else {
            return unsupported("native record differs from its checked shape");
        };
        if source_fields.len() != fields.len() {
            return unsupported("native record changed its exact checked field inventory");
        }
        let fields = source_fields
            .iter()
            .zip(fields)
            .map(|(source, field)| NativeLayoutFieldIdentity {
                field: field.id,
                declaration_identity: source
                    .native_identity
                    .as_deref()
                    .map(|identity| identities.intern(identity))
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        let mut candidates = checked.typed.plan_laid_layouts.iter().filter(|plan| {
            checked.typed.normalized_type_identity(plan.application) == source_identity
        });
        let planned = candidates.next();
        if candidates.next().is_some() {
            return unsupported("native record has multiple checked layout applications");
        }
        let policy = match planned {
            Some(_) => identities.intern(source.native_policy_identity.as_deref().ok_or(
                LoweringError::Unsupported(
                    "planned native record lost its checked policy identity",
                ),
            )?),
            None => Default::default(),
        };
        let mut realizations = Vec::new();
        if let Some(plan) = planned {
            match &plan.realizations {
                typed_trees::PlanLaidRealizations::Invariant(geometry) => {
                    realizations.push(NativeLayoutRealization {
                        target: None,
                        report: geometry.native_layout_report(),
                    })
                }
                typed_trees::PlanLaidRealizations::TargetInputs(span) => {
                    let alternatives = checked.typed.plan_laid_target_layouts.span(*span).ok_or(
                        LoweringError::Unsupported("native layout lost its target member storage"),
                    )?;
                    for alternative in alternatives {
                        realizations.push(NativeLayoutRealization {
                            target: Some(TargetLayoutInput {
                                target: alternative.target.as_str().to_owned(),
                                pointer_size: alternative.target_facts.pointer_size,
                                pointer_alignment: alternative.target_facts.pointer_alignment,
                                little_endian: alternative.target_facts.little_endian,
                            }),
                            report: alternative.geometry.native_layout_report(),
                        });
                    }
                    realizations.sort_by(|left, right| {
                        left.target
                            .as_ref()
                            .map(|target| &target.target)
                            .cmp(&right.target.as_ref().map(|target| &target.target))
                    });
                }
            }
        }
        catalog.insert(
            record.id,
            source
                .native_identity
                .as_deref()
                .map(|identity| identities.intern(identity))
                .unwrap_or_default(),
            policy,
            fields,
            realizations,
        );
    }
    Ok(catalog)
}
