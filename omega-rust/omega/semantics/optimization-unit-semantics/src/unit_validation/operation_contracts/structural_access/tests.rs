//! A derived descriptor never acquires the access of an owning result.
use super::*;

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
