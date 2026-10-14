//! Nominal formation and custody-free carrier storage at representation boundaries.
use super::*;
/// Validate declaration geometry and the exact nominal formation roster at an
/// independent representation boundary. Theorem truth is not decided here.
pub fn validate_quotient_representations(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    catalog: &terminal_psi::TerminalQuotientCatalog,
) -> Result<(), &'static str> {
    for declaration in declarations {
        if let Some(carrier) = declaration.quotient_carrier {
            let mut rows = catalog
                .representations
                .iter()
                .filter(|row| row.quotient == declaration.id && row.carrier == carrier);
            if rows.next().is_none() || rows.next().is_some() {
                return Err("quotient declaration lost its unique formation row");
            }
        }
    }
    for (index, row) in catalog.representations.iter().enumerate() {
        if row.quotient == row.carrier
            || catalog.representations[..index]
                .iter()
                .any(|prior| prior.quotient == row.quotient)
        {
            return Err("duplicate or reflexive quotient representation");
        }
        let quotient = declarations
            .iter()
            .find(|ty| ty.id == row.quotient)
            .ok_or("undeclared quotient representation")?;
        let carrier = declarations
            .iter()
            .find(|ty| ty.id == row.carrier)
            .ok_or("undeclared quotient carrier")?;
        validate_carrier(declarations, row.carrier, &mut Vec::new())?;
        if quotient.quotient_carrier != Some(row.carrier) {
            return Err("quotient formation targets an ordinary declaration");
        }
        if row.formation.quotient_declaration.is_empty()
            || row.formation.relation.is_empty()
            || !same_carrier_shape(&quotient.shape, &carrier.shape)
            || quotient.scalar_restriction != carrier.scalar_restriction
            || quotient.descriptor_cleanup != carrier.descriptor_cleanup
            || quotient.no_code_affine_cleanup != carrier.no_code_affine_cleanup
        {
            return Err("quotient representation identity or retained shape mismatch");
        }
    }
    for (index, row) in catalog.adaptations.iter().enumerate() {
        if catalog
            .representations
            .get(row.representation as usize)
            .is_none()
            || catalog.adaptations[..index]
                .iter()
                .any(|other| other.machine == row.machine && other.copy == row.copy)
        {
            return Err("invalid or duplicate quotient adaptation reference");
        }
        let selected = match row.license {
            TerminalQuotientAdaptationLicense::Mint => None,
            TerminalQuotientAdaptationLicense::Input { operation, .. }
            | TerminalQuotientAdaptationLicense::Result { operation } => Some(operation),
        };
        if selected.is_some_and(|index| catalog.operations.get(index as usize).is_none()) {
            return Err("quotient adaptation references an absent operation");
        }
    }
    Ok(())
}

/// Bind formation export identities to the containing declaration table.
pub fn validate_quotient_representation_identities(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    catalog: &terminal_psi::TerminalQuotientCatalog,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<(), &'static str> {
    for row in &catalog.representations {
        let quotient = declarations
            .iter()
            .find(|ty| ty.id == row.quotient)
            .ok_or("absent quotient declaration")?;
        let carrier = declarations
            .iter()
            .find(|ty| ty.id == row.carrier)
            .ok_or("absent quotient carrier")?;
        if quotient.identity != row.quotient_storage_identity
            || carrier.identity != row.carrier_storage_identity
            || identities
                .text(row.quotient_storage_identity)
                .is_none_or(str::is_empty)
            || identities
                .text(row.carrier_storage_identity)
                .is_none_or(str::is_empty)
        {
            return Err("quotient nominal identity drifted");
        }
    }
    Ok(())
}

/// Executable representation is limited to finite, custody-free value storage.
/// Reference descriptors and routed bindings are not interchangeable with
/// copies of the values to which they grant access.
fn validate_carrier(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    id: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
) -> Result<(), &'static str> {
    use terminal_psi::{StructuralFieldType as F, StructuralTypeShape as S};
    if active.contains(&id) {
        return Err("cyclic quotient carrier storage");
    }
    let declaration = declarations
        .iter()
        .find(|ty| ty.id == id)
        .ok_or("quotient carrier storage is absent")?;
    // Descriptor cleanup is a disposition capability also held by plain
    // scalar records. Inspect the retained shape for reference contents.
    active.push(id);
    let mut children = Vec::new();
    let mut custody_field = false;
    let mut field = |field: &terminal_psi::StructuralFieldDeclaration| {
        custody_field |= matches!(field.field_type, F::Atomic(_))
            || field
                .field_type
                .byte_sequence_carrier()
                .is_some_and(|carrier| {
                    !matches!(
                        carrier,
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                });
        if let F::Structural(child) = field.field_type {
            children.push(child);
        }
    };
    match &declaration.shape {
        S::BindingCarrier
        | S::Reference { .. }
        | S::ElementView { .. }
        | S::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
            | terminal_psi::ByteSequenceCarrier::BoundedView { .. },
        ) => {
            return Err("quotient carrier contains reference or routed storage");
        }
        S::Record { fields, .. } => fields.iter().for_each(&mut field),
        S::Sum { cases } => {
            for case in cases {
                case.fields.iter().for_each(&mut field);
            }
        }
        S::Mixed { fields, cases } => {
            fields.iter().for_each(&mut field);
            for case in cases {
                case.fields.iter().for_each(&mut field);
            }
        }
        S::FixedArray { element, .. } => children.push(*element),
        S::PrimitiveScalar(_) | S::ByteSequence(_) => {}
    }
    if custody_field {
        return Err("quotient carrier contains borrowed or atomic field storage");
    }
    for child in children {
        validate_carrier(declarations, child, active)?;
    }
    active.pop();
    Ok(())
}

/// Field/case IDs are declaration-local. Geometry keeps the same authored
/// identities, storage, relevance and child types without merging those IDs.
fn same_carrier_shape(
    left: &terminal_psi::StructuralTypeShape,
    right: &terminal_psi::StructuralTypeShape,
) -> bool {
    use terminal_psi::StructuralTypeShape as S;
    fn fields(
        left: &[terminal_psi::StructuralFieldDeclaration],
        right: &[terminal_psi::StructuralFieldDeclaration],
    ) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(a, b)| {
                a.identity == b.identity
                    && a.relevance == b.relevance
                    && a.storage == b.storage
                    && a.field_type == b.field_type
            })
    }
    fn cases(
        left: &[terminal_psi::StructuralCaseDeclaration],
        right: &[terminal_psi::StructuralCaseDeclaration],
    ) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(a, b)| a.identity == b.identity && fields(&a.fields, &b.fields))
    }
    match (left, right) {
        (
            S::Record {
                fields: a,
                storage: sa,
            },
            S::Record {
                fields: b,
                storage: sb,
            },
        ) => sa == sb && fields(a, b),
        (S::Sum { cases: a }, S::Sum { cases: b }) => cases(a, b),
        (
            S::Mixed {
                fields: af,
                cases: ac,
            },
            S::Mixed {
                fields: bf,
                cases: bc,
            },
        ) => fields(af, bf) && cases(ac, bc),
        _ => left == right,
    }
}
