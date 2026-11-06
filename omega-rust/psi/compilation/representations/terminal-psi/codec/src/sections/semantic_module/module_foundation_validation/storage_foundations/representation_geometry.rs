//! External storage geometry consumes every retained layout alternative.
//! Canonical declarations stay target-neutral; the existing geometry owner
//! borrows exact recipes from one issuer input at a time. No host is selected,
//! catalog rewritten, source policy rerun, or access authority introduced.

use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    LayoutApplicationId, StructuralPathSegment, StructuralRecordStorage, StructuralTypeShape,
    TargetLayoutApplication, TargetLayoutInput, TargetLayoutRealization, TerminalModule,
};
use terminal_semantics::{RecordStorageResolver, ReferenceStorageGeometry, StorageGeometryError};

pub(super) fn fixed_region_is_compatible(
    module: &TerminalModule,
    source: StructuralTypeId,
    path: &[StructuralPathSegment],
    reference: StructuralTypeId,
    stored: Option<ScalarType>,
) -> Result<bool, CodecError> {
    let Some(StructuralTypeShape::Reference { referent, .. }) =
        module.structural_type(reference).map(|row| &row.shape)
    else {
        return Ok(false);
    };
    let mut pending = vec![source, *referent];
    let mut visited = std::collections::BTreeSet::new();
    let mut applications = Vec::<&TargetLayoutApplication>::new();
    while let Some(carrier) = pending.pop() {
        if !visited.insert(carrier) {
            continue;
        }
        let declaration =
            module
                .structural_type(carrier)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "reference geometry lost its declaration",
                ))?;
        match &declaration.shape {
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::Record { fields, storage } => {
                if let StructuralRecordStorage::TargetLayout(id) = storage {
                    let application = module
                        .target_layout_applications
                        .iter()
                        .find(|row| row.id == *id && row.record == carrier)
                        .ok_or(CodecError::MalformedStructuralFoundation(
                            "reference geometry lost its exact layout application",
                        ))?;
                    applications.push(application);
                }
                pending.extend(
                    fields
                        .iter()
                        .filter(|field| !field.relevance.is_erased())
                        .filter_map(|field| field.field_type.as_structural()),
                );
            }
            _ => {}
        }
    }
    let Some(first) = applications.first() else {
        return Ok(
            ReferenceStorageGeometry::new(module.structural_types.iter(), |application| {
                Err(StorageGeometryError::UnresolvedExtent(application))
            })
            .region_source_compatible(source, path, reference, stored),
        );
    };
    if first.target_inputs.is_empty()
        || applications
            .iter()
            .any(|row| row.target_inputs != first.target_inputs)
    {
        return malformed("reference geometry has no common complete layout input roster");
    }
    for input in &first.target_inputs {
        let mut geometry =
            ReferenceStorageGeometry::new(module.structural_types.iter(), |application| {
                Err(StorageGeometryError::UnresolvedExtent(application))
            })
            .with_record_storage(RetainedLayout { module, input });
        if !geometry.region_source_compatible(source, path, reference, stored) {
            return Ok(false);
        }
    }
    Ok(true)
}

struct RetainedLayout<'module> {
    module: &'module TerminalModule,
    input: &'module TargetLayoutInput,
}

impl RecordStorageResolver for RetainedLayout<'_> {
    fn realization(
        &self,
        record: StructuralTypeId,
        application: LayoutApplicationId,
    ) -> Result<&TargetLayoutRealization, StorageGeometryError> {
        self.module
            .target_layout_applications
            .iter()
            .find(|row| row.id == application && row.record == record)
            .and_then(|application| {
                application.realizations.iter().find(|row| {
                    row.target == self.input.target
                        && row.pointer_size == self.input.pointer_size
                        && row.pointer_alignment == self.input.pointer_alignment
                        && row.case_tag_size == self.input.case_tag_size
                        && row.case_tag_alignment == self.input.case_tag_alignment
                        && row.little_endian == self.input.little_endian
                })
            })
            .ok_or(StorageGeometryError::UnresolvedLayout(record))
    }
}
