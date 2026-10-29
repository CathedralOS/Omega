//! Declaration consistency for placed-view backing at execution boundaries.
//!
//! Interpreter and native admission receive fresh provider-lent referents beside
//! a verified artifact. Verification of the artifact does not validate these
//! runtime inputs. Both boundaries must rejoin their type, path and qualifications
//! before binding a loan. This shared relation replaces the native-only check;
//! occurrence identity, placement/rights matching, aliasing and retirement remain
//! with each execution owner. No address or provider authority is inferred here.

use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalModule,
};

/// Why supplied backing cannot rejoin an artifact's structural declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedViewReferentError {
    QualificationsNonCanonical,
    BackingUndeclared(StructuralTypeId),
    RangeUnresolved,
    QualificationUndeclared(StructuralDomainId),
    QualificationCarrier(StructuralDomainId),
}

/// Rejoin a provider-lent backing to an already verified module's catalogs.
/// The result establishes only declaration consistency, not provider authority,
/// storage contents, placement compatibility, alias freedom, or a live loan.
/// Qualifications describe the declared root carrier; the path identifies the
/// lent place beneath that root.
pub fn validate_placed_view_referent(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    qualifications: &[StructuralDomainId],
) -> Result<(), PlacedViewReferentError> {
    if qualifications.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PlacedViewReferentError::QualificationsNonCanonical);
    }
    if declared_structural_type(module, structural_type).is_none() {
        return Err(PlacedViewReferentError::BackingUndeclared(structural_type));
    }
    if resolve_referent_place(module, structural_type, path).is_none() {
        return Err(PlacedViewReferentError::RangeUnresolved);
    }
    for domain in qualifications {
        let declaration = module
            .structural_domain(*domain)
            .ok_or(PlacedViewReferentError::QualificationUndeclared(*domain))?;
        if declaration.carrier != structural_type {
            return Err(PlacedViewReferentError::QualificationCarrier(*domain));
        }
    }
    Ok(())
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
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let mut structural_type = root;
    for segment in path {
        let declaration = declared_structural_type(module, structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                StructuralPathSegment::Field(identity),
                terminal_psi::record_or_mixed_shape_pattern!(fields),
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
                        leaf.canonical_leaf_type()?
                            .leaf_declaration(module.structural_types.iter())?
                            .id
                    }
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                },
            ) if index < length => *element,
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
            _ => return None,
        };
    }
    Some(structural_type)
}

/// Representation consistency of a placed input's retained runtime formal.
/// This rejoin supplies no backing or access authority. The host establishes
/// backing at entry; ordinary calls must reborrow a live matching occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedViewInputError {
    MachineMissing,
    DeclarationMissing,
    MachineIdentityMismatch,
    StateIdentityMismatch,
    CarrierIdentityMissing,
    ParameterMissing,
    CarrierMismatch,
    AccessMismatch,
}

pub fn placed_view_input_parameter<'module>(
    module: &'module TerminalModule,
    row: &terminal_psi::TerminalPlacedViewInput,
) -> Result<&'module terminal_psi::StructuralParameterDeclaration, PlacedViewInputError> {
    let machine = module
        .machine(row.machine)
        .ok_or(PlacedViewInputError::MachineMissing)?;
    let declaration = machine
        .declaration
        .as_ref()
        .ok_or(PlacedViewInputError::DeclarationMissing)?;
    if declaration.machine.identity() != row.source_machine_identity {
        return Err(PlacedViewInputError::MachineIdentityMismatch);
    }
    if !module
        .structural_types
        .iter()
        .any(|carrier| carrier.identity == row.carrier_identity)
    {
        return Err(PlacedViewInputError::CarrierIdentityMissing);
    }
    // Only the exact checked callable entry has an input boundary. A later
    // source state uses ordinary block transport and owns no extra host loan.
    // Arbitrary unequal identities must not hide an establishment demand.
    if declaration.state.identity() != row.source_state_identity {
        return Err(PlacedViewInputError::StateIdentityMismatch);
    }
    let parameter = machine
        .structural_parameters
        .get(row.runtime_position as usize)
        .ok_or(PlacedViewInputError::ParameterMissing)?;
    let carrier = module
        .structural_types
        .iter()
        .find(|carrier| carrier.id == parameter.structural_type)
        .ok_or(PlacedViewInputError::CarrierIdentityMissing)?;
    if carrier.identity != row.carrier_identity {
        return Err(PlacedViewInputError::CarrierMismatch);
    }
    if parameter.access != row.access || row.access == terminal_psi::StructuralAccess::Owned {
        return Err(PlacedViewInputError::AccessMismatch);
    }
    Ok(parameter)
}
