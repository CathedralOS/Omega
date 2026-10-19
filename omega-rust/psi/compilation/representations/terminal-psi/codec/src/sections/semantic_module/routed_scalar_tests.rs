//! Exact transport joins, not a second proof of the source program.
use super::module_wire::encode_raw;
use super::target_family_tests::{identity_table, unit_machine};
use crate::{decode_module, encode_module, semantic_fingerprint};
use semantic_vocabulary::{
    DomainSemanticId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    Proposition, ScalarDomainId, ScalarQualificationSetId, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    ContractClause, ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarDomainPredicate,
    ScalarQualificationSet, ScalarReturnOrigin, ScalarReturnQualification, TerminalIdentityIndex,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
};

pub(super) fn module() -> TerminalModule {
    let machine_id = MachineId::new(1).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    let subject = ValueId::new(1).unwrap();
    let result = ValueId::new(2).unwrap();
    let domain = ScalarDomainId::new(1).unwrap();
    let qualifications = ScalarQualificationSetId::new(1);
    let predicate = Proposition::LessOrEqual(
        ScalarTerm::Integer {
            scalar_type: scalar_type.as_integer().unwrap(),
            value: IntegerValue::Signed(0),
        },
        ScalarTerm::Value {
            id: subject,
            scalar_type,
        },
    );
    let mut guarantee = predicate.clone();
    guarantee.remap_value_ids(|_| result);
    let mut machine = unit_machine(machine_id);
    machine.declaration.as_mut().unwrap().callable = TerminalIdentityIndex(3);
    machine.parameters.push(ValueDeclaration {
        id: subject,
        scalar_type,
        qualifications: ScalarQualificationSetId::ZERO,
    });
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: result,
        scalar_type,
        qualifications,
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(1).unwrap(),
        value: subject,
        cleanup_actions: Vec::new(),
    };
    machine.contract.ensures.push(ContractClause {
        obligation: ObligationId::new(1).unwrap(),
        proposition: guarantee,
    });
    let mut module = TerminalModule::for_entry(machine_id);
    module.identities = identity_table(4);
    module.machines.push(machine);
    module
        .scalar_qualifications
        .domains
        .push(ScalarDomainDeclaration {
            id: domain,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(4),
            carrier: scalar_type,
            predicate: Some(ScalarDomainPredicate {
                subject,
                proposition: predicate,
            }),
            establishment_routes: vec![ScalarDomainEstablishmentRoute::ExactMachine {
                machine_identity: TerminalIdentityIndex(3),
            }],
        });
    module
        .scalar_qualifications
        .sets
        .push(ScalarQualificationSet {
            id: qualifications,
            domains: vec![domain],
        });
    module
        .scalar_qualifications
        .returns
        .push(ScalarReturnQualification {
            site: terminal_psi::ScalarReturnSite::NormalExit,
            machine: machine_id,
            edge: EdgeId::new(1).unwrap(),
            source: subject,
            result,
            domain,
            origins: vec![ScalarReturnOrigin::Established { route: 0 }],
        });
    module
}

#[test]
fn routed_scalar_result_round_trips_and_commits_predicate_and_origin() {
    let module = module();
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes).unwrap(), module);
    let original = semantic_fingerprint(&module).unwrap();
    let mut changed = module.clone();
    changed.scalar_qualifications.domains[0]
        .predicate
        .as_mut()
        .unwrap()
        .proposition = Proposition::Truth;
    assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
    let mut changed = module.clone();
    changed.scalar_qualifications.returns[0].origins = vec![ScalarReturnOrigin::Forwarded];
    assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
}

#[test]
fn routed_scalar_completion_retains_the_exact_formation_edge() {
    use terminal_psi::ScalarReturnSite;
    let mut module = module();
    let machine = &mut module.machines[0];
    let mut completion = machine.blocks[0].clone();
    completion.id = semantic_vocabulary::BlockId::new(2).unwrap();
    let destination = ValueId::new(3).unwrap();
    completion.parameters.push(ValueDeclaration {
        id: destination,
        ..machine.result.scalar().unwrap()
    });
    completion.terminator = Terminator::Return {
        edge: EdgeId::new(2).unwrap(),
        value: destination,
        cleanup_actions: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Jump {
        edge: EdgeId::new(1).unwrap(),
        target: completion.id,
        arguments: vec![machine.parameters[0].id],
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    machine.blocks.push(completion);
    let formation = &mut module.scalar_qualifications.returns[0];
    formation.site = ScalarReturnSite::Completion {
        argument_ordinal: 0,
    };
    formation.result = destination;
    let mut returned = formation.clone();
    returned.site = ScalarReturnSite::NormalExit;
    returned.edge = EdgeId::new(2).unwrap();
    returned.source = destination;
    returned.result = machine.result.scalar().unwrap().id;
    returned.origins = vec![ScalarReturnOrigin::Forwarded];
    module.scalar_qualifications.returns.push(returned);
    assert_eq!(
        decode_module(&encode_module(&module).unwrap()).unwrap(),
        module
    );
    for mutation in 0..5 {
        let mut malformed = module.clone();
        match mutation {
            0 => {
                malformed.scalar_qualifications.returns.remove(0);
            }
            1 => {
                malformed.scalar_qualifications.returns[0].edge = EdgeId::new(2).unwrap();
            }
            2 => {
                malformed.scalar_qualifications.returns[0].site = ScalarReturnSite::Completion {
                    argument_ordinal: 1,
                };
            }
            3 => {
                malformed.scalar_qualifications.returns[0].origins =
                    vec![ScalarReturnOrigin::Forwarded];
            }
            _ => {
                malformed.scalar_qualifications.returns[0].origins =
                    vec![ScalarReturnOrigin::Vacuous];
            }
        }
        assert!(
            decode_module(&encode_raw(&malformed).unwrap()).is_err(),
            "mutation {mutation}"
        );
    }
    module.scalar_qualifications.domains[0]
        .establishment_routes
        .clear();
    module.scalar_qualifications.domains[0].predicate = None;
    module.scalar_qualifications.returns[0].origins = vec![ScalarReturnOrigin::Vacuous];
    assert_eq!(
        decode_module(&encode_module(&module).unwrap()).unwrap(),
        module
    );
}

#[test]
fn routed_scalar_result_rejects_detached_or_forged_transport() {
    type Mutation = fn(&mut TerminalModule);
    for (label, mutate) in [
        (
            "missing return row",
            (|module| module.scalar_qualifications.returns.clear()) as Mutation,
        ),
        ("wrong edge", |module| {
            module.scalar_qualifications.returns[0].edge = EdgeId::new(9).unwrap()
        }),
        ("wrong source", |module| {
            module.scalar_qualifications.returns[0].source = ValueId::new(9).unwrap()
        }),
        ("wrong result", |module| {
            module.scalar_qualifications.returns[0].result = ValueId::new(9).unwrap()
        }),
        ("wrong issuer", |module| {
            module.scalar_qualifications.domains[0].establishment_routes[0] =
                ScalarDomainEstablishmentRoute::ExactMachine {
                    machine_identity: TerminalIdentityIndex(2),
                }
        }),
        ("missing predicate", |module| {
            module.scalar_qualifications.domains[0].predicate = None
        }),
        ("missing result predicate", |module| {
            module.machines[0].contract.ensures.clear()
        }),
        ("substituted predicate", |module| {
            module.machines[0].contract.ensures[0].proposition = Proposition::Truth
        }),
        ("wrong subject carrier", |module| {
            module.scalar_qualifications.domains[0]
                .predicate
                .as_mut()
                .unwrap()
                .proposition = Proposition::Equal(
                ScalarTerm::Value {
                    id: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Boolean,
                },
                ScalarTerm::Boolean(true),
            );
        }),
        ("false forwarding", |module| {
            module.scalar_qualifications.returns[0].origins = vec![ScalarReturnOrigin::Forwarded]
        }),
        ("missing origin", |module| {
            module.scalar_qualifications.returns[0].origins.clear()
        }),
        ("wrong route", |module| {
            module.scalar_qualifications.returns[0].origins =
                vec![ScalarReturnOrigin::Established { route: 1 }]
        }),
    ] {
        let mut candidate = module();
        mutate(&mut candidate);
        assert!(
            decode_module(&encode_raw(&candidate).unwrap()).is_err(),
            "{label}"
        );
    }
}

#[test]
fn routed_scalar_forwarding_preserves_membership_without_issuer_status() {
    let mut module = module();
    module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(1);
    module.machines[0].declaration.as_mut().unwrap().callable = TerminalIdentityIndex(2);
    module.scalar_qualifications.returns[0].origins = vec![ScalarReturnOrigin::Forwarded];
    assert_eq!(
        decode_module(&encode_module(&module).unwrap()).unwrap(),
        module
    );
}

#[test]
fn routed_scalar_specialization_retains_exact_authored_issuer() {
    let mut module = module();
    let declaration = module.machines[0].declaration.as_mut().unwrap();
    declaration.callable = TerminalIdentityIndex(2);
    declaration.specialization = Some(terminal_psi::CheckedMachineSpecialization {
        template: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
        template_callable: TerminalIdentityIndex(3),
        commitment: [9; 32],
    });
    assert_eq!(
        decode_module(&encode_module(&module).unwrap()).unwrap(),
        module
    );
    module.machines[0]
        .declaration
        .as_mut()
        .unwrap()
        .specialization
        .as_mut()
        .unwrap()
        .template_callable = TerminalIdentityIndex(4);
    assert!(decode_module(&encode_raw(&module).unwrap()).is_err());
}
