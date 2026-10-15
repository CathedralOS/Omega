//! Whole-owner loans preserve capacity, live-length metadata and predicates.
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeShape, TerminalModule,
};

/// Rejoin an ordinary call loan to the original whole bounded owner. An
/// exclusive callee must return every predicate the caller's field requires;
/// passing that field through a bare mutable carrier would lose the invariant.
pub fn bounded_byte_owner_capacity(
    module: &TerminalModule,
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    bounded_byte_owner_capacity_from_catalogs(
        &module.structural_types,
        &module.structural_domains,
        actual,
        argument,
        expected,
    )
}

pub fn bounded_byte_owner_capacity_from_catalogs(
    types: &[terminal_psi::StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    bounded_byte_owner_capacity_in(types.iter(), domains.iter(), actual, argument, expected)
}

pub fn bounded_byte_owner_capacity_in<'type_declaration, 'domain>(
    types: impl Iterator<Item = &'type_declaration terminal_psi::StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    if actual.place != argument.place
        || argument.access != expected.access
        || expected.multiplicity != StructuralMultiplicity::Unrestricted
        || !matches!(
            actual.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        )
        || argument.access == StructuralAccess::Owned
        || !crate::reference_source_projection_accessible(
            types.clone(),
            actual.structural_type,
            actual.multiplicity,
            actual.access,
            &argument.path,
            argument.access,
        )
    {
        return None;
    }
    let expected_type = types
        .clone()
        .find(|row| row.id == expected.structural_type)?;
    let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
        expected_type.shape
    else {
        return None;
    };
    let required = crate::byte_parameter_predicates_in(domains.clone(), expected)?;
    let (actual_capacity, predicates) = if argument.path.is_empty() {
        let declaration = types.clone().find(|row| row.id == actual.structural_type)?;
        let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
            declaration.shape
        else {
            return None;
        };
        (
            capacity,
            crate::byte_parameter_predicates_in(domains.clone(), actual)?,
        )
    } else {
        if !actual.qualifications.is_empty() || !actual.projected_qualifications.is_empty() {
            return None;
        }
        let field = crate::runtime_structural_field(types, actual.structural_type, &argument.path)?;
        let ByteSequenceCarrier::BoundedOwned { capacity } =
            field.field_type.byte_sequence_carrier()?
        else {
            return None;
        };
        (capacity, field.field_type.byte_predicates())
    };
    if actual_capacity != capacity
        || !predicates.implies(required)
        || (argument.access != StructuralAccess::SharedBorrow && !required.implies(predicates))
    {
        return None;
    }
    Some(capacity)
}
