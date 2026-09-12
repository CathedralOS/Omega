//! A derived descriptor never acquires the access of an owning result.
use super::*;

#[test]
fn shared_affine_owner_loan_preserves_owner_and_rejects_access_or_place_substitution() {
    use terminal_psi::{StructuralAccess as Access, StructuralMultiplicity as Multiplicity};
    let mut unit = crate::tests::projected_shared_structural_scalar_call_unit();
    let types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let caller = &mut unit.functions[0];
    caller.structural_parameters[0].access = Access::Owned;
    caller.structural_parameters[0].multiplicity = Multiplicity::Affine;
    let mut parameter = caller.structural_parameters[0].clone();
    parameter.access = Access::SharedBorrow;
    parameter.multiplicity = Multiplicity::Unrestricted;
    let argument = terminal_psi::StructuralArgument {
        place: caller.structural_parameters[0].place,
        access: Access::SharedBorrow,
        path: Vec::new(),
    };
    let accepts = |argument: &terminal_psi::StructuralArgument,
                   parameter: &terminal_psi::StructuralParameterDeclaration| {
        structural_arguments_match(
            caller,
            std::slice::from_ref(argument),
            std::slice::from_ref(parameter),
            &types,
            StructuralProjectionPolicy::Projected,
            false,
        )
    };
    assert!(accepts(&argument, &parameter));
    assert_eq!(
        structural_source_contract(caller, argument.place, false)
            .unwrap()
            .multiplicity,
        Multiplicity::Affine
    );
    for access in [
        Access::MutableBorrow,
        Access::WriteOnlyBorrow,
        Access::Owned,
    ] {
        let mut changed = argument.clone();
        changed.access = access;
        let mut destination = parameter.clone();
        destination.access = access;
        assert!(!accepts(&changed, &destination));
    }
    let mut changed = argument;
    changed.place = PlaceId::new(999).unwrap();
    assert!(!accepts(&changed, &parameter));
}

#[test]
fn subslice_source_contract_retains_shared_access_and_exact_result_identity() {
    let mut unit = crate::tests::projected_shared_structural_scalar_call_unit();
    let caller = &mut unit.functions[0];
    let parameter = &caller.structural_parameters[0];
    let place = PlaceId::new(900).unwrap();
    let structural_type = parameter.structural_type;
    caller.blocks[0].nodes[0].operation = O::ByteSequenceSubslice {
        psi_operation: OperationId::new(900).unwrap(),
        result: terminal_psi::StructuralOperationResult {
            place,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        source: parameter.place,
        start: ValueId::new(901).unwrap(),
        end: ValueId::new(902).unwrap(),
        length: ValueId::new(903).unwrap(),
        obligation: semantic_vocabulary::ObligationId::new(900).unwrap(),
    };
    // This tests source classification only. Whole-unit validation separately
    // checks the actual bounds, producer dominance and scalar operand custody.
    let contract = structural_source_contract(caller, place, false).unwrap();
    assert_eq!(contract.structural_type, structural_type);
    assert_eq!(
        contract.multiplicity,
        terminal_psi::StructuralMultiplicity::Unrestricted
    );
    assert_eq!(
        contract.access,
        terminal_psi::StructuralAccess::SharedBorrow
    );
    assert!(structural_access_can_supply(
        contract.access,
        terminal_psi::StructuralAccess::SharedBorrow
    ));
    for access in [
        terminal_psi::StructuralAccess::Owned,
        terminal_psi::StructuralAccess::MutableBorrow,
        terminal_psi::StructuralAccess::WriteOnlyBorrow,
    ] {
        assert!(!structural_access_can_supply(contract.access, access));
    }
    assert!(structural_source_contract(caller, PlaceId::new(999).unwrap(), false).is_none());
}
