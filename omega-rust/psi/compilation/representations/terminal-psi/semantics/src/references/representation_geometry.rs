//! A storage view borrows the original address. Judge its validity before
//! geometry, then check every retained target input through one prepared
//! layout owner. Import validation and lowered checking use this same rule.

use crate::{RecordStorageResolver, ReferenceStorageGeometry, StorageGeometryError};
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    LayoutApplicationId, StructuralPathSegment, StructuralRecordStorage, StructuralTypeShape,
    TargetLayoutApplication, TargetLayoutInput, TargetLayoutRealization, TerminalModule,
};

pub fn reference_view_compatible(
    module: &TerminalModule,
    source: StructuralTypeId,
    path: &[StructuralPathSegment],
    reference: StructuralTypeId,
    stored: Option<ScalarType>,
) -> Result<bool, &'static str> {
    // A reference crossing changes the address base. Its earlier descriptor
    // path still owns custody; geometry starts at the reached referent.
    let (source, path) = if let Some(crossing) = path
        .iter()
        .rposition(|step| *step == StructuralPathSegment::Referent)
    {
        let carrier = crate::runtime_structural_path_tip(
            module.structural_types.iter(),
            source,
            &path[..crossing],
        )
        .ok_or("reference view lost its source descriptor")?;
        let StructuralTypeShape::Reference { referent, .. } = carrier.shape else {
            return Err("reference view crosses a source that is not a reference");
        };
        (referent, &path[crossing + 1..])
    } else {
        (source, path)
    };
    let Some(StructuralTypeShape::Reference { referent, .. }) =
        module.structural_type(reference).map(|row| &row.shape)
    else {
        return Ok(false);
    };
    let Some(StructuralTypeShape::Reference { access, .. }) =
        module.structural_type(reference).map(|row| &row.shape)
    else {
        return Err("reference view lost its borrowed access");
    };
    if *access == terminal_psi::StructuralAccess::Owned {
        return Err("a reference view cannot own its backing");
    }
    if !total_representation(module, *referent)? {
        return Err("reference view requires target validity not established by its source bits");
    }
    if *access != terminal_psi::StructuralAccess::SharedBorrow
        && !total_representation(module, source)?
    {
        return Err("writable reference view cannot preserve the source's validity");
    }
    let mut pending = vec![source, *referent];
    let mut visited = Vec::new();
    let mut applications = Vec::<&TargetLayoutApplication>::new();
    while let Some(carrier) = pending.pop() {
        match visited.binary_search(&carrier) {
            Ok(_) => continue,
            Err(at) => visited.insert(at, carrier),
        }
        let declaration = module
            .structural_type(carrier)
            .ok_or("reference geometry lost its declaration")?;
        match &declaration.shape {
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::Reference { referent, .. } => pending.push(*referent),
            StructuralTypeShape::Record { fields, storage } => {
                if let StructuralRecordStorage::TargetLayout(id) = storage {
                    let application = module
                        .target_layout_applications
                        .iter()
                        .find(|row| row.id == *id && row.record == carrier)
                        .ok_or("reference geometry lost its exact layout application")?;
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
            .view_source_compatible(source, path, reference, stored),
        );
    };
    if first.target_inputs.is_empty()
        || applications
            .iter()
            .any(|row| row.target_inputs != first.target_inputs)
    {
        return Err("reference geometry has no common complete layout input roster");
    }
    for input in &first.target_inputs {
        let mut geometry =
            ReferenceStorageGeometry::new(module.structural_types.iter(), |application| {
                Err(StorageGeometryError::UnresolvedExtent(application))
            })
            .with_record_storage(RetainedLayout { module, input });
        if !geometry.view_source_compatible(source, path, reference, stored) {
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
fn total_representation(
    module: &TerminalModule,
    root: StructuralTypeId,
) -> Result<bool, &'static str> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut fields = Vec::new();
    let total_scalar = |scalar: ScalarType| {
        matches!(scalar,
        ScalarType::Integer(integer) if !integer.is_address())
            || matches!(scalar, ScalarType::IeeeFloat(_))
    };
    while let Some(current) = pending.pop() {
        match visited.binary_search(&current) {
            Ok(_) => continue,
            Err(at) => visited.insert(at, current),
        }
        let declaration = module
            .structural_type(current)
            .ok_or("reference validity lost its declaration")?;
        if declaration.owns_linear_claim
            || declaration.scalar_restriction.is_some()
            || !declaration.requires.is_empty()
        {
            return Ok(false);
        }
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) if total_scalar(*scalar) => {}
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::Record { fields: stored, .. } => {
                for field in stored.iter().filter(|field| !field.relevance.is_erased()) {
                    fields.push(field.id);
                    match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(child) => pending.push(child),
                        terminal_psi::StructuralFieldType::Scalar(scalar)
                            if total_scalar(scalar) => {}
                        _ => return Ok(false),
                    }
                }
            }
            _ => return Ok(false),
        }
    }
    fields.sort_unstable();
    Ok(!module.scalar_qualifications.fields.iter().any(|row| {
        fields.binary_search(&row.field).is_ok()
            && row.value_type.qualifications != semantic_vocabulary::ScalarQualificationSetId::ZERO
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        AuthoredFieldStorage, AuthoredRecordStorage, IntegerSign, IntegerType, StructuralFieldId,
    };
    use terminal_psi::{
        ArrayExtent, BindingRelevance, StructuralAccess, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeDeclaration, TargetLayoutFieldStorage,
        TargetLayoutSchemaArgument, TerminalIdentityIndex,
    };

    fn ty(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }
    fn declaration(raw: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: ty(raw),
            identity: TerminalIdentityIndex(raw as u32),
            shape,
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
            requires: Vec::new(),
        }
    }

    #[test]
    fn a_shared_view_checks_all_targets_and_does_not_create_boolean_validity() {
        // The bytes must fit every retained target, and equal byte counts
        // cannot turn arbitrary stored bytes into valid Boolean values.
        let scalar =
            |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
        let field = |raw| StructuralFieldDeclaration {
            id: StructuralFieldId::new(raw).unwrap(),
            identity: TerminalIdentityIndex(raw as u32),
            field_type: StructuralFieldType::Scalar(scalar(64)),
            storage: None,
            inline_carrier: None,
            relevance: BindingRelevance::Relevant,
        };
        let mut module = TerminalModule::declarations();
        module.structural_types = vec![
            declaration(1, StructuralTypeShape::PrimitiveScalar(scalar(8))),
            declaration(2, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
            declaration(
                3,
                StructuralTypeShape::Record {
                    fields: vec![field(1), field(2)],
                    storage: StructuralRecordStorage::TargetLayout(LayoutApplicationId(1)),
                },
            ),
            declaration(
                4,
                StructuralTypeShape::FixedArray {
                    element: ty(1),
                    length: ArrayExtent::Literal(16),
                },
            ),
            declaration(
                5,
                StructuralTypeShape::Reference {
                    referent: ty(4),
                    access: StructuralAccess::SharedBorrow,
                },
            ),
        ];
        let inputs: Vec<_> = ["linux_x86_64", "macos_arm64"]
            .into_iter()
            .map(|target| TargetLayoutInput {
                target: target.to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 8,
                case_tag_alignment: 8,
                little_endian: true,
            })
            .collect();
        let realizations = inputs
            .iter()
            .map(|input| TargetLayoutRealization {
                target: input.target.clone(),
                pointer_size: input.pointer_size,
                pointer_alignment: input.pointer_alignment,
                case_tag_size: input.case_tag_size,
                case_tag_alignment: input.case_tag_alignment,
                little_endian: input.little_endian,
                schema_argument: TargetLayoutSchemaArgument {
                    abi_version: 1,
                    fields: Vec::new(),
                    retired_field_identities: Vec::new(),
                },
                storage: AuthoredRecordStorage {
                    byte_size: 16,
                    byte_alignment: 8,
                },
                fields: [8, 0]
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, byte_offset)| TargetLayoutFieldStorage {
                        field: StructuralFieldId::new(ordinal as u64 + 1).unwrap(),
                        storage: AuthoredFieldStorage::Direct { byte_offset },
                    })
                    .collect(),
            })
            .collect();
        module
            .target_layout_applications
            .push(TargetLayoutApplication {
                id: LayoutApplicationId(1),
                record: ty(3),
                schema: ty(3),
                policy_identity: TerminalIdentityIndex(6),
                plan_identity: TerminalIdentityIndex(7),
                target_inputs: inputs,
                realizations,
            });
        assert_eq!(
            reference_view_compatible(&module, ty(3), &[], ty(5), None),
            Ok(true)
        );
        module.target_layout_applications[0].realizations[1]
            .storage
            .byte_size = 32;
        assert_eq!(
            reference_view_compatible(&module, ty(3), &[], ty(5), None),
            Ok(false)
        );
        module.target_layout_applications[0].realizations[1]
            .storage
            .byte_size = 16;
        module.structural_types[3].shape = StructuralTypeShape::FixedArray {
            element: ty(1),
            length: ArrayExtent::Literal(15),
        };
        assert_eq!(
            reference_view_compatible(&module, ty(3), &[], ty(5), None),
            Ok(false)
        );
        module.structural_types[3].shape = StructuralTypeShape::FixedArray {
            element: ty(2),
            length: ArrayExtent::Literal(16),
        };
        assert_eq!(
            reference_view_compatible(&module, ty(3), &[], ty(5), None),
            Err("reference view requires target validity not established by its source bits")
        );
    }
}
