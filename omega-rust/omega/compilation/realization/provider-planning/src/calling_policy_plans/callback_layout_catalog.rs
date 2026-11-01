//! Canonical callback geometry beside geometry-free source-review addresses.
//! No Source-shaped layout is reconstructed from the canonical physical values.

mod validation;
pub(super) use validation::validate;

use calling_conventions::{
    BoundaryCallbackLayout, CallbackDataGeometry, CallbackInlineFieldGeometry,
    CallbackSlotGeometry, NativePlace,
};
use checked_trees::typed_trees::{
    SourceLayoutPlanCoordinate, SourceLayoutRealizationCoordinate, SourceRecordCorrespondence,
};
use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
use semantic_vocabulary::StructuralTypeId;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackRecordReviewSource {
    pub record: StructuralTypeId,
    pub declaration: SymbolHandle,
    pub application: checked_trees::typed_trees::SourceTypeApplication,
    pub storage_identity: String,
    pub schema: SymbolHandle,
    pub policy: SymbolHandle,
    pub layout: Option<SourceLayoutPlanCoordinate>,
    pub realization: Option<SourceLayoutRealizationCoordinate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackFieldReviewSource {
    pub declaration: SymbolHandle,
    pub schema: SymbolHandle,
    pub child: CallbackRecordReviewSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackSlotReviewSource {
    pub realization: SourceLayoutRealizationCoordinate,
    pub ordinal: u32,
    pub selection: PrivateCallbackSlotSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackLayoutReviewSource {
    pub root: CallbackRecordReviewSource,
    pub fields: Vec<CallbackFieldReviewSource>,
    pub terminal: CallbackSlotReviewSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackLayoutEntry {
    pub(crate) geometry: BoundaryCallbackLayout,
    pub(crate) review: CallbackLayoutReviewSource,
}

impl BoundaryCallbackLayoutEntry {
    pub fn geometry(&self) -> &BoundaryCallbackLayout {
        &self.geometry
    }
    pub fn review_source(&self) -> &CallbackLayoutReviewSource {
        &self.review
    }
    pub const fn formal_ordinal(&self) -> u32 {
        self.geometry.formal_ordinal
    }
    pub const fn native_ordinal(&self) -> u32 {
        self.geometry.native_ordinal
    }
    pub const fn destination(&self) -> &NativePlace {
        &self.geometry.destination
    }
    pub fn root_layout(&self) -> &CallbackDataGeometry {
        &self.geometry.root
    }
    pub fn inline_fields(&self) -> Result<&[CallbackInlineFieldGeometry], String> {
        self.geometry.fields()
    }
    pub fn terminal_slot(&self) -> &CallbackSlotGeometry {
        &self.geometry.terminal
    }
    pub fn composed_offset(&self) -> Result<u64, String> {
        self.geometry
            .fields()?
            .iter()
            .try_fold(self.geometry.terminal.offset, |offset, field| {
                offset
                    .checked_add(field.offset)
                    .ok_or_else(|| "callback composed offset overflows".into())
            })
    }
    pub(crate) fn source_free(&self) -> Result<BoundaryCallbackLayout, String> {
        Ok(self.geometry.clone())
    }

    pub(crate) fn from_canonical(
        module: &terminal_psi::TerminalModule,
        geometry: BoundaryCallbackLayout,
        coordinates: &layout::CallbackLayoutCoordinates,
        sources: &[SourceRecordCorrespondence],
        target: target::TargetProfile,
    ) -> Result<Self, String> {
        let source = |record: StructuralTypeId| {
            let mut found = sources.iter().filter(|row| row.record == record);
            let row = found
                .next()
                .ok_or("callback review lost its exact producer record")?;
            if found.next().is_some() {
                return Err("callback review has ambiguous producer records");
            }
            Ok(row)
        };
        let project =
            |record: &SourceRecordCorrespondence| -> Result<CallbackRecordReviewSource, String> {
                Ok(CallbackRecordReviewSource {
                    record: record.record,
                    declaration: record.declaration,
                    application: record.application.clone(),
                    storage_identity: record.storage_identity.clone(),
                    schema: record.schema,
                    policy: record.policy,
                    layout: record.layout.clone(),
                    realization: if record.layout.is_some() {
                        Some(selected_realization(record, target)?.clone())
                    } else {
                        None
                    },
                })
            };
        if geometry.destination != coordinates.destination {
            return Err("callback review coordinates name another native destination".into());
        }
        let mut parent = source(coordinates.root).map_err(str::to_owned)?;
        let root = project(parent)?;
        let mut fields = Vec::with_capacity(coordinates.fields.len());
        for identity in &coordinates.fields {
            let original = parent
                .fields
                .iter()
                .find(|field| field.field == *identity)
                .ok_or("callback review lost its producer-owned inline field")?;
            if !original.schema.is_valid() {
                return Err("callback inline field has no source schema declaration".into());
            }
            let field = module
                .structural_type(parent.record)
                .and_then(|row| row.shape.record_fields())
                .and_then(|rows| rows.iter().find(|field| field.id == *identity))
                .ok_or("callback inline path lost its exact canonical field")?;
            let child = field
                .field_type
                .as_structural()
                .ok_or("callback inline field has no canonical child record")?;
            let child = source(child).map_err(str::to_owned)?;
            fields.push(CallbackFieldReviewSource {
                declaration: original.declaration,
                schema: original.schema,
                child: project(child)?,
            });
            parent = child;
        }
        if parent.record != coordinates.terminal_record || fields.len() != geometry.fields()?.len()
        {
            return Err("callback review path differs from the canonical containing record".into());
        }
        let realization = selected_realization(parent, target)?;
        if parent.layout.is_none() {
            return Err("callback slot has no source layout application".into());
        }
        let native = module
            .native_layouts
            .record(parent.record)
            .ok_or("callback slot lost its native record")?;
        let selected = module
            .native_layouts
            .for_target(native, target.target_name())?
            .ok_or("callback slot lost its selected native realization")?;
        let slot = selected
            .report
            .private_callback_demands
            .get(coordinates.slot_ordinal as usize)
            .ok_or("callback slot lost its exact native declaration ordinal")?;
        if slot.slot_application.application_commitment != geometry.terminal.conformance_commitment
        {
            return Err(
                "callback slot correspondence substituted its exact selected application".into(),
            );
        }
        Ok(Self {
            geometry,
            review: CallbackLayoutReviewSource {
                root,
                fields,
                terminal: CallbackSlotReviewSource {
                    realization: realization.clone(),
                    ordinal: coordinates.slot_ordinal,
                    selection: slot.slot_application.clone(),
                },
            },
        })
    }
}

fn selected_realization(
    record: &SourceRecordCorrespondence,
    target: target::TargetProfile,
) -> Result<&SourceLayoutRealizationCoordinate, String> {
    match record.realizations.as_slice() {
        [invariant] if invariant.target.is_empty() => Ok(invariant),
        rows => {
            let mut matching = rows.iter().filter(|row| row.target == target.target_name());
            let row = matching
                .next()
                .ok_or("callback review lost its exact target realization")?;
            if matching.next().is_some() {
                return Err("callback review has duplicate target realizations".into());
            }
            Ok(row)
        }
    }
}
