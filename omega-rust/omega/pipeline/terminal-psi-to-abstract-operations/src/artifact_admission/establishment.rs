//! Provider establishment supplies bound at the executable input boundary.
//!
//! A `Placed<P, T>` direct-entry input is custody, not data: the roster row
//! seals the placement the view interprets, and neither the roster nor a
//! pointer is the authority that lends the referent. The ordinary
//! establishment route supplies one [`TerminalPlacedViewEstablishment`] per
//! declared direct-entry roster row — the provider's loan of the exact
//! qualified backing — and admission binds each supply to its row so the
//! realized executable's entry boundary can lend it for the invocation's
//! duration. The loan is invocation-scoped: the bound set is retained as
//! executable-entry evidence, and on failure the supply returns to the
//! provider unbound.
//!
//! Every supplied `input` must equal its declared roster row exactly —
//! machine, position, declaration identities, access and binding flags, and
//! both sealed placement identities — so a stale layout, qualified backing,
//! range, rights, or occurrence answers no row here and rejects before
//! access. A row left without a supply still fails custody, and an
//! exclusive-borrow referent may not overlap another established referent.
//!
//! The loan's referent is module-visible custody, not a bare host carrier:
//! its declared structural type must be a type the artifact declares, its
//! path must resolve through that declared shape graph to a real place, and
//! every domain qualification it asserts must be a domain the artifact
//! declares over that carrier. A supply whose backing, range, or
//! qualifications cannot rejoin the module's own catalogs is a stale or
//! substituted establishment and rejects before access.

pub use terminal_interpreter::TerminalPlacedViewEstablishment;

use super::ArtifactLoweringError;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule,
};

const fn exclusive(access: StructuralAccess) -> bool {
    matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    )
}

fn referent_overlaps(
    left: &terminal_interpreter::TerminalStructuralValue,
    right: &terminal_interpreter::TerminalStructuralValue,
) -> bool {
    left.opaque_identity == right.opaque_identity
        && (left.path.starts_with(&right.path) || right.path.starts_with(&left.path))
}

/// One module-declared structural carrier, joined by exact id. The artifact's
/// catalog is the only authority this boundary can name a backing with: a
/// referent declaring a type the module never carries cannot be the qualified
/// backing the roster row sealed.
fn declared_structural_type(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
) -> Option<&StructuralTypeDeclaration> {
    let mut matching = module
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}

/// Resolve the lent place's path inside its declared carrier. Every segment
/// must name a real declared step — a relevant record or mixed-shape field, an
/// in-range fixed index, or a reference crossing — and a leaf field may end
/// the path only when its canonical shape is itself declared. A stale or
/// substituted range fails to resolve rather than binding an invented place.
fn resolve_referent_place(
    module: &TerminalModule,
    referent: &terminal_interpreter::TerminalStructuralValue,
) -> Option<StructuralTypeId> {
    let mut structural_type = referent.structural_type;
    for segment in &referent.path {
        let declaration = declared_structural_type(module, structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let mut matching = fields
                    .iter()
                    .filter(|field| field.identity == *identity && !field.relevance.is_erased());
                let field = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape()?;
                        module
                            .structural_types
                            .iter()
                            .find(|declaration| declaration.shape == shape)?
                            .id
                    }
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
            _ => return None,
        };
    }
    Some(structural_type)
}

/// Join each direct-entry placed-view roster row to one supplied provider
/// establishment and return the bound set in roster order.
///
/// Only rows on the module's entry machine are entry inputs; a row on any
/// other machine declares a call-bound input whose custody arrives through
/// its caller, a route the executable input boundary does not carry — such
/// modules keep failing closed beside the establishment-less entrance. The
/// interpreter additionally checks each exclusive referent against the
/// invocation's structural arguments; this boundary admits no argument
/// channel, so establishments are the only live carriers to check.
pub(super) fn establish_native_placed_view_inputs(
    module: &TerminalModule,
    establishments: &[TerminalPlacedViewEstablishment],
) -> Result<Vec<TerminalPlacedViewEstablishment>, ArtifactLoweringError> {
    if module
        .placed_view_inputs
        .iter()
        .any(|row| row.machine != module.entry)
    {
        // A subordinate input is bound by its call, not by the executable's
        // host: this input boundary cannot carry it and there is nothing to
        // answer it with yet.
        return Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering);
    }
    let entry_rows = module
        .placed_view_inputs
        .iter()
        .filter(|row| row.machine == module.entry)
        .collect::<Vec<_>>();
    // Every supply must answer one declared entry row exactly; custody the
    // artifact never demanded cannot be bound at its input boundary.
    let mut seen = BTreeSet::new();
    for establishment in establishments {
        if !entry_rows.contains(&&establishment.input) {
            return Err(ArtifactLoweringError::PlacedViewEstablishmentUnexpected {
                machine: establishment.input.machine,
                position: establishment.input.position,
            });
        }
        if !seen.insert(&establishment.input) {
            return Err(ArtifactLoweringError::PlacedViewEstablishmentDuplicate {
                machine: establishment.input.machine,
                position: establishment.input.position,
            });
        }
        let referent = &establishment.referent;
        if referent
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ArtifactLoweringError::PlacedViewEstablishmentQualificationsNonCanonical);
        }
        // The supply is a provider loan of module-visible custody, not a bare
        // host carrier: its declared type must be a type the artifact itself
        // declares, its path must resolve through that declared shape graph to
        // a real place, and every domain it asserts must be a domain the
        // artifact declares over that carrier. A supply whose backing, range,
        // or qualifications cannot rejoin the module's own catalogs is a
        // stale or substituted establishment, not the provider's exact loan.
        if declared_structural_type(module, referent.structural_type).is_none() {
            return Err(
                ArtifactLoweringError::PlacedViewEstablishmentBackingUndeclared(
                    referent.structural_type,
                ),
            );
        }
        if resolve_referent_place(module, referent).is_none() {
            return Err(ArtifactLoweringError::PlacedViewEstablishmentRangeUnresolved);
        }
        for domain in &referent.qualifications {
            let Some(declaration) = module
                .structural_domains
                .iter()
                .find(|declaration| declaration.id == *domain)
            else {
                return Err(
                    ArtifactLoweringError::PlacedViewEstablishmentQualificationUndeclared(*domain),
                );
            };
            if declaration.carrier != referent.structural_type {
                return Err(
                    ArtifactLoweringError::PlacedViewEstablishmentQualificationCarrier(*domain),
                );
            }
        }
    }
    let mut bound: Vec<(
        &terminal_psi::TerminalPlacedViewInput,
        &TerminalPlacedViewEstablishment,
    )> = Vec::with_capacity(entry_rows.len());
    for row in entry_rows {
        let Some(establishment) = establishments
            .iter()
            .find(|establishment| establishment.input == *row)
        else {
            return Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering);
        };
        for (previous_row, previous) in &bound {
            if (exclusive(row.access) || exclusive(previous_row.access))
                && referent_overlaps(&establishment.referent, &previous.referent)
            {
                return Err(ArtifactLoweringError::PlacedViewEstablishmentAliasing(
                    establishment.referent.opaque_identity,
                ));
            }
        }
        bound.push((row, establishment));
    }
    Ok(bound
        .into_iter()
        .map(|(_, establishment)| establishment.clone())
        .collect())
}
