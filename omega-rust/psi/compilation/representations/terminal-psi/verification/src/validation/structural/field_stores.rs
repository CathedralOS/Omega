//! Shared destination authority for field replacement. The value contract
//! (scalar assignment, descriptor lifetime, or byte predicates) belongs to its
//! operation; no replacement gets a different root merely from its value kind.
//! Live ownership, child loans, and selector bounds replay independently.
use super::super::{
    PlaceId, StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeId,
    StructuralTypeShape, TerminalMachine, TerminalModule,
};

pub(super) fn parent_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    destination: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let reference = super::super::references::mutable_record_field_destination(
        module,
        machine,
        destination,
        path,
    );
    let structural_type = if let Some((referent, _)) = reference {
        referent
    } else if let Some(result) = super::super::record::loan_source(module, machine, destination) {
        // Mutation borrows the carrier's storage, not the backing referenced by
        // its descriptor fields. Use the same custody as an ordinary mutable
        // record loan; plain-payload transfer eligibility is not store authority.
        // Each field operation still validates its value and backing lifetime.
        result.structural_type
    } else if let Some(result) =
        super::super::record::established_local_source(module, machine, destination)
    {
        result.structural_type
    } else if let Some(result) =
        super::super::element_view::write::mutable_result(module, machine, destination).filter(
            |result| {
                module.structural_types.iter().any(|row| {
                    row.id == result.structural_type
                        && matches!(row.shape, StructuralTypeShape::ElementView { .. })
                })
            },
        )
    {
        result.structural_type
    } else {
        let parameter = terminal_semantics::arrival_parameter(machine, destination)?;
        if !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !matches!(
            parameter.access,
            StructuralAccess::Owned
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ) || !super::scalar_fields::has_empty_structural_custody(machine, destination)
        {
            return None;
        }
        parameter.structural_type
    };
    let path = reference.map_or(path, |(_, path)| path);
    if !terminal_psi::is_structural_scalar_store_path(path)
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == destination)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == destination)
    {
        return None;
    }
    super::runtime_indexes::checked_projection_type(module, structural_type, path)
}
