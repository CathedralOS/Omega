//! Authenticated stores can replace the whole contents of an owned root.
use super::storage_foundations::structural_fields::validate_store_structural_field;
use crate::semantic_module::entry_tests::unit_machine;
use semantic_vocabulary::{
    ClaimId, MachineId, OperationId, PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    EntryClaim, NominalCleanupSelection, NominalDeclarationIdentity, Operation, OperationKind,
    OperationResult, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralStoreDisposition,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex, TerminalMachine,
    TerminalModule,
};

#[test]
fn replacing_an_owned_root_requires_its_prior_contents_to_be_discardable() {
    let record = StructuralTypeId::new(1).unwrap();
    let mut module = TerminalModule::declarations();
    module
        .types
        .structural_types
        .push(StructuralTypeDeclaration {
            id: record,
            identity: TerminalIdentityIndex(1),
            declaration_identity: Default::default(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            quotient_carrier: None,
            owns_linear_claim: false,
            multiplicity: StructuralMultiplicity::Unrestricted,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            nominal_cleanup: None,
            scalar_restriction: None,
            requires: Vec::new(),
        });
    let mut machine = unit_machine(MachineId::new(1).unwrap());
    let destination = PlaceId::new(1).unwrap();
    let source = PlaceId::new(2).unwrap();
    for (position, place) in [(0, destination), (1, source)] {
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter { position },
        });
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                position,
                place,
                structural_type: record,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            });
    }
    let operation = Operation {
        id: OperationId::new(1).unwrap(),
        static_reach_binding: None,
        required_contract: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::StoreStructuralField {
            disposition: StructuralStoreDisposition::ReplaceDiscardable,
            destination,
            path: Vec::new(),
            field: None,
            value: StructuralArgument::whole(source, StructuralAccess::Owned),
        },
    };
    let admitted = |module: &TerminalModule, machine: &TerminalMachine| {
        validate_store_structural_field(module, machine, &operation).is_ok()
    };
    assert!(admitted(&module, &machine));
    machine.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(!admitted(&module, &machine));
    machine.structural_parameters[0].access = StructuralAccess::Owned;
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
    assert!(!admitted(&module, &machine));
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    machine.entry_claims.push(EntryClaim {
        claim: ClaimId::new(1).unwrap(),
        input: destination,
        path: Vec::new(),
    });
    assert!(!admitted(&module, &machine));
    machine.entry_claims.clear();
    module.types.structural_types[0].owns_linear_claim = true;
    assert!(!admitted(&module, &machine));
    module.types.structural_types[0].owns_linear_claim = false;
    module.types.structural_types[0].nominal_cleanup = Some(NominalCleanupSelection {
        declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
        cleanup_machine: MachineId::new(2).unwrap(),
    });
    assert!(!admitted(&module, &machine));
}
