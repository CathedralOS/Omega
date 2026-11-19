
use crate::exactly_one::exactly_one;
use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalModule,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessViewReferentError {
    QualificationsNonCanonical,
    BackingUndeclared(StructuralTypeId),
    RangeUnresolved,
    QualificationUndeclared(StructuralDomainId),
    QualificationCarrier(StructuralDomainId),
}

pub fn validate_access_view_referent(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    qualifications: &[StructuralDomainId],
) -> Result<(), AccessViewReferentError> {
    if qualifications.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(AccessViewReferentError::QualificationsNonCanonical);
    }
    if declared_structural_type(module, structural_type).is_none() {
        return Err(AccessViewReferentError::BackingUndeclared(structural_type));
    }
    if resolve_referent_place(module, structural_type, path).is_none() {
        return Err(AccessViewReferentError::RangeUnresolved);
    }
    for domain in qualifications {
        let declaration = module
            .structural_domain(*domain)
            .ok_or(AccessViewReferentError::QualificationUndeclared(*domain))?;
        if declaration.carrier != structural_type {
            return Err(AccessViewReferentError::QualificationCarrier(*domain));
        }
    }
    Ok(())
}

fn declared_structural_type(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
) -> Option<&StructuralTypeDeclaration> {
    let mut matching = module
        .types.structural_types
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}

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
                let field = exactly_one(fields.iter(), |field| {
                    field.identity == *identity && !field.relevance.is_erased()
                })?;
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        leaf.canonical_leaf_type()?
                            .leaf_declaration(module.types.structural_types.iter())?
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
            (
                StructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, .. },
            ) => *referent,
            _ => return None,
        };
    }
    Some(structural_type)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessViewInputError {
    MachineMissing,
    DeclarationMissing,
    MachineIdentityMismatch,
    StateIdentityMismatch,
    CarrierIdentityMissing,
    ParameterMissing,
    CarrierMismatch,
    AccessMismatch,
}

pub fn access_view_input_parameter<'module>(
    module: &'module TerminalModule,
    row: &terminal_psi::TerminalAccessViewInput,
) -> Result<&'module terminal_psi::StructuralParameterDeclaration, AccessViewInputError> {
    let machine = module
        .machine(row.machine)
        .ok_or(AccessViewInputError::MachineMissing)?;
    let declaration = machine
        .declaration
        .as_ref()
        .ok_or(AccessViewInputError::DeclarationMissing)?;
    if declaration.machine.identity() != row.source_machine_identity {
        return Err(AccessViewInputError::MachineIdentityMismatch);
    }
    if !module
        .types.structural_types
        .iter()
        .any(|carrier| carrier.identity == row.carrier_identity)
    {
        return Err(AccessViewInputError::CarrierIdentityMissing);
    }
    if declaration.state.identity() != row.source_state_identity {
        return Err(AccessViewInputError::StateIdentityMismatch);
    }
    let parameter = machine
        .structural_parameters
        .get(row.runtime_position as usize)
        .ok_or(AccessViewInputError::ParameterMissing)?;
    let carrier = module
        .types.structural_types
        .iter()
        .find(|carrier| carrier.id == parameter.structural_type)
        .ok_or(AccessViewInputError::CarrierIdentityMissing)?;
    if carrier.identity != row.carrier_identity {
        return Err(AccessViewInputError::CarrierMismatch);
    }
    if parameter.access != row.access || row.access == terminal_psi::StructuralAccess::Owned {
        return Err(AccessViewInputError::AccessMismatch);
    }
    Ok(parameter)
}
