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
    pub(super) source_declaration: symbols::SymbolHandle,
    pub(super) source_application: &'checked typed_trees::SourceTypeApplication,
    pub(super) id: StructuralTypeId,
    pub(super) source_identity: TypeIdentityId,
    pub(super) native_identity: Option<&'checked str>,
    pub(super) policy_identity: Option<&'checked str>,
    pub(super) semantic_identity: Option<&'checked str>,
    pub(super) fields: Vec<NativeFieldProjection<'checked>>,
}

pub(crate) struct NativeFieldProjection<'checked> {
    pub(super) field: StructuralFieldId,
    pub(super) native_identity: Option<&'checked str>,
    pub(super) declaration: symbols::SymbolHandle,
    pub(super) relevance: terminal_psi::BindingRelevance,
}

pub(super) fn lower(
    checked: &CheckedTrees,
    records: &[NativeRecordProjection<'_>],
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<
    (
        NativeLayoutCatalog,
        Vec<typed_trees::SourceRecordCorrespondence>,
    ),
    LoweringError,
> {
    let mut catalog = NativeLayoutCatalog::default();
    let mut source_records = Vec::with_capacity(records.len());
    for record in records {
        let source_identity = record.source_identity;
        let fields = record
            .fields
            .iter()
            .map(|field| NativeLayoutFieldIdentity {
                field: field.field,
                declaration_identity: field
                    .native_identity
                    .map(|identity| identities.intern(identity))
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        let mut candidates =
            checked
                .typed
                .plan_laid_layouts
                .iter()
                .enumerate()
                .filter(|(_, plan)| {
                    checked.typed.normalized_type_identity(plan.application) == source_identity
                });
        let planned_coordinate = candidates.next();
        let planned = planned_coordinate.map(|(_, plan)| plan);
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
        let mut source_realizations = Vec::new();
        if let Some(plan) = planned {
            match &plan.realizations {
                typed_trees::PlanLaidRealizations::Invariant(geometry) => {
                    source_realizations.push(typed_trees::SourceLayoutRealizationCoordinate {
                        target: String::new(),
                        member: Default::default(),
                    });
                    realizations.push(NativeLayoutRealization {
                        target: None,
                        report: geometry.native_layout_report(),
                    })
                }
                typed_trees::PlanLaidRealizations::TargetInputs(span) => {
                    let alternatives = checked.typed.plan_laid_target_layouts.span(*span).ok_or(
                        LoweringError::Unsupported("native layout lost its target member storage"),
                    )?;
                    for (member, alternative) in span.handles().zip(alternatives) {
                        source_realizations.push(typed_trees::SourceLayoutRealizationCoordinate {
                            target: alternative.target.as_str().to_owned(),
                            member,
                        });
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
        let source_fields = record
            .fields
            .iter()
            .map(|field| {
                let schema = match planned {
                    Some(plan) => {
                        match plan
                            .field_symbols
                            .iter()
                            .position(|candidate| *candidate == field.declaration)
                        {
                            Some(ordinal) => *plan.schema_field_symbols.get(ordinal).ok_or(
                                LoweringError::Unsupported(
                                    "native review field lost its source schema correspondence",
                                ),
                            )?,
                            // Erased declarations have no runtime Schema entry and
                            // cannot be a callback's physical inline hop.
                            None if field.relevance.is_erased() => symbols::SymbolHandle::invalid(),
                            None => {
                                return Err(LoweringError::Unsupported(
                                    "native review field lost its source layout owner",
                                ));
                            }
                        }
                    }
                    None => field.declaration,
                };
                Ok(typed_trees::SourceFieldCorrespondence {
                    field: field.field,
                    declaration: field.declaration,
                    schema,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        source_records.push(typed_trees::SourceRecordCorrespondence {
            record: record.id,
            declaration: record.source_declaration,
            application: record.source_application.clone(),
            storage_identity: checked
                .typed
                .type_identity_text(record.source_identity)
                .ok_or(LoweringError::Unsupported(
                    "native record lost its exact storage identity",
                ))?,
            schema: planned.map_or(record.source_declaration, |plan| plan.schema_symbol),
            policy: planned.map_or_else(symbols::SymbolHandle::invalid, |plan| plan.policy_symbol),
            layout: planned_coordinate
                .map(|(ordinal, plan)| {
                    Ok(typed_trees::SourceLayoutPlanCoordinate {
                        ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("native source layout roster exceeds u32")
                        })?,
                        application: plan.application,
                    })
                })
                .transpose()?,
            fields: source_fields,
            realizations: source_realizations,
        });
        catalog.insert(
            record.id,
            record
                .native_identity
                .map(|identity| identities.intern(identity))
                .unwrap_or_default(),
            policy,
            record
                .semantic_identity
                .map(|identity| identities.intern(identity))
                .unwrap_or_default(),
            fields,
            realizations,
        );
    }
    Ok((catalog, source_records))
}
