//! A storage view borrows the original address. Judge its validity before
//! geometry, then check every retained target input through one prepared
//! layout owner. Import validation and lowered checking use this same rule.

use crate::{RecordStorageResolver, ReferenceStorageGeometry, StorageGeometryError};
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    LayoutApplicationId, StructuralPathSegment, StructuralRecordStorage, StructuralTypeShape,
    TargetLayoutApplication, TargetLayoutInput, TargetLayoutRealization, TerminalModule,
};

/// A borrowed scalar can carry a narrower codec than its displayed type.
/// Require its declared ingress to promise a fixed footprint before exporting
/// that storage as a whole byte region. Owned homes retain their own geometry.
pub fn validate_reference_view_source(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    source: semantic_vocabulary::PlaceId,
) -> Result<(), &'static str> {
    if let Some(parameter) = crate::arrival_parameter(machine, source) {
        let declared = module
            .structural_type(parameter.structural_type)
            .ok_or("reference view source lost its declared ingress type")?;
        if parameter.access != terminal_psi::StructuralAccess::Owned
            && let StructuralTypeShape::PrimitiveScalar(logical) = &declared.shape
        {
            let width = crate::reference_storage_export_byte_width(*logical, parameter.access)
                .ok_or("reference view source has no fixed incoming storage width")?;
            if Some(width) != crate::reference_storage_byte_width(*logical) {
                return Err("reference view source changes its incoming storage width");
            }
        }
        return Ok(());
    }
    let declaration = machine
        .structural_place(source)
        .ok_or("reference view source has no declared storage root")?;
    match declaration.kind {
        semantic_vocabulary::StructuralPlaceKind::OperationResult { .. }
        | semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral { .. }
        | semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal { .. } => Ok(()),
        _ => Err("reference view source has no established owned or incoming storage"),
    }
}

/// Whole-region views retain the captured region's first byte. Another
/// offset needs its own bounds/alignment evidence, which this operation lacks.
pub fn reference_view_offset_is_zero(
    operation: &terminal_psi::Operation,
    value: semantic_vocabulary::ValueId,
) -> bool {
    let Some(result) = operation.result.as_scalar() else {
        return false;
    };
    result.id == value
        && matches!(result.scalar_type, ScalarType::Integer(integer)
            if !integer.is_address() && integer.bits() == Some(64)
                && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned)
        && matches!(
            operation.kind,
            terminal_psi::OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(0)
            }
        )
}

pub fn reference_view_compatible(
    module: &TerminalModule,
    source: StructuralTypeId,
    path: &[StructuralPathSegment],
    reference: StructuralTypeId,
    stored: Option<ScalarType>,
) -> Result<bool, &'static str> {
    // A descriptor may present a wider scalar than its physical codec. Its
    // nominal referent cannot supply the backing extent for another view.
    if path.contains(&StructuralPathSegment::Referent) {
        return Err(
            "reference view crossing requires its captured physical storage correspondence",
        );
    }
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
    fn borrowed_scalar_views_use_the_declared_storage_width() {
        use semantic_vocabulary::{BlockId, ContractId, MachineId, PlaceId};
        let place = PlaceId::new(1).unwrap();
        let mut module = TerminalModule::declarations();
        module.structural_types.push(declaration(
            1,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            )),
        ));
        let mut machine = terminal_psi::TerminalMachine {
            id: MachineId::new(1).unwrap(),
            entry: BlockId::new(1).unwrap(),
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            attachment: None,
            nominal_cleanup_subject: None,
            ranked_scc: None,
            structural_places: vec![terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter { position: 0 },
            }],
            call_formations: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            blocks: Vec::new(),
            contract: terminal_psi::MachineContract::empty(ContractId::new(1).unwrap()),
        };
        machine
            .structural_parameters
            .push(terminal_psi::StructuralParameterDeclaration {
                place,
                position: 0,
                structural_type: ty(1),
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            });
        // A logical u64 can arrive through a u32 codec. Looking at its
        // displayed type alone cannot establish an eight-byte byte view.
        assert_eq!(
            validate_reference_view_source(&module, &machine, place),
            Err("reference view source has no fixed incoming storage width")
        );
        machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
        machine.structural_parameters[0].multiplicity =
            terminal_psi::StructuralMultiplicity::Affine;
        assert_eq!(
            validate_reference_view_source(&module, &machine, place),
            Ok(())
        );
        machine.structural_parameters[0].access = StructuralAccess::Owned;
        machine.structural_parameters[0].multiplicity =
            terminal_psi::StructuralMultiplicity::Unrestricted;
        assert_eq!(
            validate_reference_view_source(&module, &machine, place),
            Ok(())
        );
        machine.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        );
        assert_eq!(
            validate_reference_view_source(&module, &machine, place),
            Ok(())
        );
        machine.structural_parameters.clear();
        assert_eq!(
            validate_reference_view_source(&module, &machine, place),
            Err("reference view source has no established owned or incoming storage")
        );
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
        assert_eq!(
            reference_view_compatible(
                &module,
                ty(5),
                &[StructuralPathSegment::Referent],
                ty(5),
                None
            ),
            Err("reference view crossing requires its captured physical storage correspondence"),
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
