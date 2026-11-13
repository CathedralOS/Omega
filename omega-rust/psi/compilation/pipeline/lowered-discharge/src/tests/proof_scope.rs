use super::*;
use terminal_psi::{
    CheckedMachineDeclaration, NominalDeclarationIdentity, ProofCallableAssociation,
    ProofCallableHeader, ProofDeclarationId, ProofScopeDeclaration, TargetMachineCallable,
    TerminalIdentityTableBuilder,
};

fn declared_body() -> structure_elaboration::Elaboration {
    let mut body = machine(vec![u32_type()], vec![returns(slot(0, u32_type()))]);
    body.requires = vec![at_least(1, 1)];
    let mut elaboration = lower(vec![body]).unwrap();
    let mut identities = TerminalIdentityTableBuilder::default();
    let callable = identities.intern("proof-model::entry");
    let integer = identities.intern("u32");
    let declaration = CheckedMachineDeclaration {
        machine: NominalDeclarationIdentity::new(callable),
        state: NominalDeclarationIdentity::new(callable),
        semantic_declaration: callable,
        parameter_types: vec![integer],
        callable,
        result_type: integer,
        contract: [7; 32],
        published_may_suspend: false,
        published_may_block: false,
        specialization: None,
    };
    elaboration.module.identities = identities.build();
    elaboration.module.machines[0].declaration = Some(declaration.clone());
    let scope = &mut elaboration.module.scalar_applications.proof_scope;
    scope
        .declarations
        .insert(ProofScopeDeclaration::Callable(declaration));
    scope.callables.insert(ProofCallableAssociation::Callable {
        declaration: ProofDeclarationId(1),
        callable: TargetMachineCallable::Machine(id(1)),
    });
    scope.callable_headers.insert(ProofCallableHeader {
        declaration: ProofDeclarationId(1),
        callable: Ok(callable),
        ..ProofCallableHeader::default()
    });
    elaboration
}

#[test]
fn accepted_body_status_keeps_its_input_premises_and_exact_owner() {
    // This is a declaration-custody model, not an invocation or authority
    // witness. Its positive body is checked under its actual input promise.
    let elaboration = declared_body();
    let requires = elaboration.module.machines[0].contract.requires.clone();
    assert!(!requires.is_empty());
    assert!(
        !elaboration
            .module
            .scalar_applications
            .proof_scope
            .callable_headers
            .iter()
            .next()
            .unwrap()
            .1
            .supplies_checked_body
    );
    let RouteVerdict::Accepted(accepted) = judge(Ok(elaboration)).unwrap() else {
        panic!("the body satisfies its contract under the stated input")
    };
    assert_eq!(
        accepted.elaboration.module.machines[0].contract.requires,
        requires
    );
    let scope = &accepted.elaboration.module.scalar_applications.proof_scope;
    let header = scope.callable_headers.iter().next().unwrap().1;
    assert!(header.supplies_checked_body);
    assert!(header.compiler_evaluation.is_err());

    let mut forged = declared_body();
    forged
        .module
        .scalar_applications
        .proof_scope
        .declarations
        .for_each_mut(|_, row| {
            let ProofScopeDeclaration::Callable(saved) = row else {
                panic!("one callable")
            };
            saved.contract = [8; 32];
        });
    assert!(matches!(
        judge(Ok(forged)),
        Err(Refusal {
            reason: RefusalReason::ForeignIdentity,
            ..
        })
    ));
    let mut orphan = declared_body();
    let scope = &mut orphan.module.scalar_applications.proof_scope;
    scope.callables = Default::default();
    scope
        .callable_headers
        .for_each_mut(|_, header| header.supplies_checked_body = true);
    assert!(matches!(
        judge(Ok(orphan)),
        Err(Refusal {
            reason: RefusalReason::ForeignIdentity,
            ..
        })
    ));

    let mut missing_header = declared_body();
    missing_header
        .module
        .scalar_applications
        .proof_scope
        .callable_headers = Default::default();
    assert!(matches!(
        judge(Ok(missing_header)),
        Err(Refusal {
            reason: RefusalReason::ForeignIdentity,
            ..
        })
    ));
}
