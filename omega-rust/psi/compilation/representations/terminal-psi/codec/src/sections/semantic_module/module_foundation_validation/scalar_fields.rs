//! Exact stored scalar membership joins at external artifact ingress.
//! These are representation correspondences, not predicate/provenance replay.
//! A declaration states what each actual initializer/replacement must carry;
//! reading it snapshots that same contract, never a numerically equal domain.

use super::{CodecError, TerminalMachine, TerminalModule, malformed};
use semantic_vocabulary::{ScalarDomainId, ScalarQualificationSetId, StructuralFieldId, ValueId};
use terminal_psi::{
    OperationKind, RecordFieldValue, StructuralFieldDeclaration, StructuralTypeShape,
};
fn fields(module: &TerminalModule) -> impl Iterator<Item = &StructuralFieldDeclaration> {
    module.structural_types.iter().flat_map(|owner| {
        let (fields, cases) = match &owner.shape {
            StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
            StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases.as_slice()),
            _ => (&[][..], &[][..]),
        };
        fields
            .iter()
            .chain(cases.iter().flat_map(|case| &case.fields))
    })
}

fn domains(
    module: &TerminalModule,
    set: ScalarQualificationSetId,
) -> Result<&[ScalarDomainId], CodecError> {
    super::qualification_domains(module, set).ok_or(CodecError::MalformedStructuralFoundation(
        "stored scalar names an absent qualification set",
    ))
}

fn required(
    module: &TerminalModule,
    field: StructuralFieldId,
) -> Result<&[ScalarDomainId], CodecError> {
    module
        .scalar_qualifications
        .fields
        .iter()
        .find(|row| row.field == field)
        .map_or(Ok(&[][..]), |row| {
            domains(module, row.value_type.qualifications)
        })
}

fn validate_write(
    module: &TerminalModule,
    machine: &TerminalMachine,
    field: StructuralFieldId,
    value: ValueId,
) -> Result<(), CodecError> {
    let required = required(module, field)?;
    if required.is_empty() {
        return Ok(());
    }
    let value = super::values(machine).find(|row| row.id == value).ok_or(
        CodecError::MalformedStructuralFoundation("scalar field write lost its exact SSA source"),
    )?;
    let actual = domains(module, value.qualifications)?;
    if required.iter().any(|domain| !actual.contains(domain)) {
        return malformed("scalar field write lacks its required domain identity");
    }
    Ok(())
}

/// Primitive referents currently retain a carrier and numeric restriction,
/// not a field's ScalarDomainIds. A writable projection cannot erase that
/// receiving contract, even if a local store happens to write an issued value.
/// Reference<Record> keeps its declared field roster and remains supported.
/// Shared views cannot write or introduce membership and use existing admission.
pub(super) fn validate_primitive_reference_presentation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    access: terminal_psi::StructuralAccess,
) -> Result<(), CodecError> {
    if access == terminal_psi::StructuralAccess::SharedBorrow {
        return Ok(());
    }
    if let Some(field) = storage_field(module, machine, place, path)?
        && !required(module, field)?.is_empty()
    {
        return malformed("primitive reference loses scalar field membership");
    }
    Ok(())
}

fn storage_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Result<Option<StructuralFieldId>, CodecError> {
    let root = super::structural_place_type(machine, place).ok_or(
        CodecError::MalformedStructuralFoundation("scalar storage reference lost its declaration"),
    )?;
    Ok(
        terminal_semantics::runtime_structural_field(module.structural_types.iter(), root, path)
            .map(|field| field.id),
    )
}

/// An explicit byte-offset interpretation retains geometry, not a scalar-field
/// origin correspondence. It cannot introduce target membership or lend away
/// a writable source invariant. Exact ordinary record references retain their
/// declaration; shared views into a bare, nonmutating target may only forget
/// observations. Shared atomic/descriptor targets are not read-only views.
pub(super) fn validate_storage_interpretation(
    module: &TerminalModule,
    source: semantic_vocabulary::StructuralTypeId,
    reference: semantic_vocabulary::StructuralTypeId,
    access: terminal_psi::StructuralAccess,
) -> Result<(), CodecError> {
    let Some(StructuralTypeShape::Reference { referent, .. }) =
        module.structural_type(reference).map(|row| &row.shape)
    else {
        return malformed("scalar field storage interpretation lost its reference type");
    };
    let read_only = access == terminal_psi::StructuralAccess::SharedBorrow
        && terminal_semantics::shared_mutation(module.structural_types.iter(), *referent, &[])
            == terminal_semantics::SharedMutation::None;
    if contains_membership(module, *referent)? || !read_only && contains_membership(module, source)?
    {
        return malformed("storage reinterpretation lacks scalar field membership correspondence");
    }
    Ok(())
}

fn contains_membership(
    module: &TerminalModule,
    root: semantic_vocabulary::StructuralTypeId,
) -> Result<bool, CodecError> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let declaration =
            module
                .structural_type(current)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "scalar field storage interpretation lost a stored type",
                ))?;
        let (fields, cases) = match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases.as_slice()),
            StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
            StructuralTypeShape::FixedArray { element, length } => {
                if *length != 0 {
                    pending.push(*element);
                }
                continue;
            }
            // Pointer contents are not inline receiving storage. Its geometry
            // and access remain with the existing reference admission owner.
            _ => continue,
        };
        for field in fields
            .iter()
            .chain(cases.iter().flat_map(|case| &case.fields))
            .filter(|field| !field.relevance.is_erased())
        {
            if !required(module, field.id)?.is_empty() {
                return Ok(true);
            }
            if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                pending.push(child);
            }
        }
    }
    Ok(false)
}

pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    for row in &module.scalar_qualifications.fields {
        let mut matching = fields(module).filter(|field| field.id == row.field);
        let first = matching
            .next()
            .ok_or(CodecError::MalformedStructuralFoundation(
                "scalar field membership lost its declaration",
            ))?;
        // The wire permits the same field ID in separate owners. The catalog
        // requirement applies to all of them, so every declaration must agree;
        // choosing the first would make admission depend on type order.
        for field in std::iter::once(first).chain(matching) {
            if field.relevance.is_erased()
                || matches!(
                    field.field_type,
                    terminal_psi::StructuralFieldType::Atomic(_)
                        | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
                )
                || field.field_type.scalar_type() != Some(row.value_type.scalar_type)
                || row.value_type.qualifications.is_empty()
            {
                return malformed("scalar field membership changes its declared carrier");
            }
        }
        for domain in domains(module, row.value_type.qualifications)? {
            if super::scalar_domain(module, *domain)
                .is_none_or(|entry| entry.carrier != row.value_type.scalar_type)
            {
                return malformed("scalar field membership has a different domain carrier");
            }
        }
    }
    for machine in &module.machines {
        for operation in machine.operations() {
            match &operation.kind {
                OperationKind::EstablishZeroedStructural {} => {
                    let result = operation.result.structural().ok_or(
                        CodecError::MalformedStructuralFoundation(
                            "zero provisioning lost its structural result",
                        ),
                    )?;
                    if !terminal_semantics::zeroed_structural_fields_satisfy(
                        module.structural_types.iter(),
                        result.structural_type,
                        |field| {
                            required(module, field.id).is_ok_and(|domains| {
                                domains.iter().all(|id| {
                                    super::scalar_domain(module, *id).is_some_and(|domain| {
                                        domain.predicate.is_none()
                                            && domain.establishment_routes.is_empty()
                                            && domain.has_no_required_memberships()
                                    })
                                })
                            })
                        },
                    ) {
                        return malformed(
                            "zero provisioning cannot establish scalar field membership",
                        );
                    }
                }
                OperationKind::EstablishRecord { fields }
                | OperationKind::EstablishStructuralCase { fields, .. } => {
                    for field in fields {
                        if let RecordFieldValue::Scalar { value, .. } = field.value {
                            validate_write(module, machine, field.field, value)?;
                        }
                    }
                }
                OperationKind::StructuralScalarFieldStore { field, value, .. } => {
                    validate_write(module, machine, *field, *value)?;
                }
                OperationKind::WriteOnlyPrimitiveStore {
                    destination,
                    path,
                    value,
                    ..
                } => {
                    if let Some(field) = storage_field(module, machine, *destination, path)? {
                        validate_write(module, machine, field, *value)?;
                    }
                }
                _ => {}
            }
            let Some(result) = operation
                .result
                .scalar_ref()
                .filter(|row| !row.qualifications.is_empty())
            else {
                continue;
            };
            let field = match &operation.kind {
                OperationKind::ScalarStructuralField { field, .. }
                | OperationKind::BooleanStructuralField { field, .. } => Some(*field),
                OperationKind::PrimitiveScalarRead { source, path } => {
                    storage_field(module, machine, *source, path)?
                }
                _ => continue,
            }
            .ok_or(CodecError::MalformedStructuralFoundation(
                "qualified scalar read lost its exact field declaration",
            ))?;
            let supplied = required(module, field)?;
            if domains(module, result.qualifications)?
                .iter()
                .any(|domain| !supplied.contains(domain))
            {
                return malformed(
                    "scalar field read introduces membership absent from its declaration",
                );
            }
        }
    }
    Ok(())
}
