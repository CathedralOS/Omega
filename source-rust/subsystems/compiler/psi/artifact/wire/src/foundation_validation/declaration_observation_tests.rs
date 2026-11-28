use super::*;
use crate::semantic_module::entry_tests::{identity_table, unit_machine};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, PlaceId, ScalarReadProjection, ScalarTerm, StructuralFieldId,
};
use terminal_psi::{
    BoundaryParameterKind, ContractClause, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalCallableEnvelope, TerminalCallableTermination,
    TerminalDeclarationEnvelope, TerminalMachineResult,
};

fn receive(module: &TerminalModule) -> Result<TerminalModule, CodecError> {
    crate::decode_module(&crate::module_wire::encode_raw(module).unwrap())
}

#[test]
fn bodyless_contract_observations_resolve_against_the_input_and_result_declarations() {
    let mut module = TerminalModule::declarations();
    module.identities = identity_table(3);
    let ty = StructuralTypeId::new(1).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let carrier = ScalarType::Integer(integer);
    let identity = TerminalIdentityIndex(3);
    module
        .types
        .structural_types
        .push(StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            quotient_carrier: None,
            id: ty,
            identity,
            owns_linear_claim: false,
            multiplicity: StructuralMultiplicity::Unrestricted,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            shape: StructuralTypeShape::PrimitiveScalar(carrier),
            scalar_restriction: None,
            requires: Vec::new(),
        });
    let input = PlaceId::new(1).unwrap();
    let result = PlaceId::new(2).unwrap();
    let mut contract = unit_machine(MachineId::new(1).unwrap()).contract;
    let declaration = contract.declaration.as_mut().unwrap();
    declaration.semantic_declaration = TerminalIdentityIndex(1);
    declaration.callable = TerminalIdentityIndex(2);
    declaration.parameter_types = vec![identity];
    declaration.result_type = identity;
    contract
        .parameter_order
        .push(BoundaryParameterKind::Structural);
    contract
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: input,
            position: 0,
            structural_type: ty,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: vec![],
            projected_qualifications: vec![],
            additional_input_qualifications: vec![],
        });
    contract.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: result,
        structural_type: ty,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![],
        projected_qualifications: vec![],
        reference_sources: vec![],
    });
    let predicate = |root, path| {
        crate::contract_forms::equal(
            ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type: carrier,
            },
            ScalarTerm::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(8),
            },
        )
        .unwrap()
    };
    let observed = vec![ScalarReadProjection::Alignment];
    contract.requires = vec![predicate(input, observed.clone())];
    contract.ensures = vec![ContractClause {
        obligation: ObligationId::new(1).unwrap(),
        proposition: predicate(result, observed.clone()),
    }];
    module
        .execution
        .callables
        .declaration_envelopes
        .push(TerminalDeclarationEnvelope {
            contract: contract.id,
            envelope: TerminalCallableEnvelope {
                service_reach: vec![],
                invocations: vec![],
                suspends: false,
                blocks: false,
                termination: TerminalCallableTermination::NotGuaranteed,
            },
        });
    module.execution.declaration_contracts.push(contract);
    assert_eq!(receive(&module).unwrap(), module);
    let nonexistent_field = vec![
        ScalarReadProjection::Static(CanonicalStructuralPathSegment::Field(
            StructuralFieldId::new(1).unwrap(),
        )),
        ScalarReadProjection::Alignment,
    ];
    for root in [input, result] {
        let mut changed = module.clone();
        let contract = &mut changed.execution.declaration_contracts[0];
        if root == input {
            contract.requires[0] = predicate(root, nonexistent_field.clone());
        } else {
            contract.ensures[0].proposition = predicate(root, nonexistent_field.clone());
        }
        assert_eq!(
            receive(&changed),
            malformed(
                "structural read predicate disagrees with its declaration path or scalar carrier"
            )
        );
    }
    module.execution.declaration_contracts[0].structural_parameters[0].access =
        StructuralAccess::WriteOnlyBorrow;
    assert_eq!(
        receive(&module),
        malformed("structural read predicate has no readable declared root")
    );
}
