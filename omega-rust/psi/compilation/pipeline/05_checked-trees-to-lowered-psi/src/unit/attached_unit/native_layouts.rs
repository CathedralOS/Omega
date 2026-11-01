//! Publish checked nominal associations and complete native layout provenance.
//! No target is selected and no policy body is evaluated during this projection.

use super::{CheckedTrees, LoweringError, unsupported};
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    NativeLayoutCatalog, NativeLayoutFieldIdentity, NativeLayoutRealization, TargetLayoutInput,
    TerminalIdentityTableBuilder,
};
use typed_trees::type_identity::TypeIdentityId;

/// A record's native association is produced with its Terminal fields, from
/// the same checked record row. Carry each emitted field ID beside its native
/// declaration identity; a later zip of independently reconstructed inventories
/// would have to revalidate a relationship this construction already owns.
pub(crate) struct NativeRecordProjection<'checked> {
    pub(super) id: StructuralTypeId,
    pub(super) source_identity: TypeIdentityId,
    pub(super) native_identity: Option<&'checked str>,
    pub(super) policy_identity: Option<&'checked str>,
    pub(super) fields: Vec<(StructuralFieldId, Option<&'checked str>)>,
}

pub(super) fn lower(
    checked: &CheckedTrees,
    records: &[NativeRecordProjection<'_>],
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<NativeLayoutCatalog, LoweringError> {
    let mut catalog = NativeLayoutCatalog::default();
    for record in records {
        let source_identity = record.source_identity;
        let fields = record
            .fields
            .iter()
            .map(|(field, identity)| NativeLayoutFieldIdentity {
                field: *field,
                declaration_identity: identity
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
            Some(_) => {
                identities.intern(record.policy_identity.ok_or(LoweringError::Unsupported(
                    "planned native record lost its checked policy identity",
                ))?)
            }
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
            record
                .native_identity
                .map(|identity| identities.intern(identity))
                .unwrap_or_default(),
            policy,
            source
                .native_semantic_identity
                .as_deref()
                .map(|identity| identities.intern(identity))
                .unwrap_or_default(),
            fields,
            realizations,
        );
    }
    Ok(catalog)
}
