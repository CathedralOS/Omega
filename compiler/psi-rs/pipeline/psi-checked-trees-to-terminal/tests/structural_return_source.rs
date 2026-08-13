use psi_language_semantics::Multiplicity;
use psi_language_semantics::PermissionClaimIdentity;
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{TerminalMachineResult, Terminator};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralResult,
    TerminalStructuralValue,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data ByteUnit {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    data Region [linear] { length: u64; }
    data Scratch { marker: u64; }
    data EmptyScratch {}
    data NominalScratch {}
    machine NominalScratch::drop(&mut self) {}
    domain Region::Owned;
    machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
    satisfies Content<CountedQuantity<ByteUnit>>::project
    {
        CountedQuantity { magnitude: region.length }
    }

    data Main {}
    machine Main::forward(region: Region in Owned) -> Region in Owned {
        region
    }
    machine Main::forward_and_drop(region: Region in Owned, scratch: Scratch) -> Region in Owned {
        region
    }
    machine Main::forward_with_local(region: Region in Owned) -> Region in Owned {
        let scratch: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::forward_with_local_and_drop(
        region: Region in Owned,
        scratch: Scratch
    ) -> Region in Owned {
        let local: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::local_partial_value(region: Region in Owned) -> Region in Owned {
        let scratch: Scratch = Scratch { marker: 1 };
        region
    }
    machine Main::local_nominal_cleanup(region: Region in Owned) -> Region in Owned {
        let scratch: NominalScratch = NominalScratch {};
        region
    }
    machine Main::too_many_locals(region: Region in Owned) -> Region in Owned {
        let first: EmptyScratch = EmptyScratch {};
        let second: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::too_many_discards(
        region: Region in Owned,
        first: Scratch,
        second: Scratch
    ) -> Region in Owned {
        region
    }
    machine Main::through_local(region: Region in Owned) -> Region in Owned {
        let forwarded: Region in Owned = region;
        forwarded
    }
    machine Main::contracted(region: Region in Owned) -> Region in Owned
    requires
        region in Region::Owned
    {
        region
    }
    machine Main::local_control(region: Region in Owned) -> Region in Owned {
        let scratch: EmptyScratch = EmptyScratch {};
        transition { _ -> next(region) }
        state next(region: Region in Owned) -> Region in Owned { region }
    }
    machine Main::main(&mut self) {}
"#;

fn checked_source() -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn whole_root_source_passthrough_reaches_verified_and_interpreted_terminal_psi() {
    let checked = checked_source();

    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .find(|plan| {
            checked.machines().iter().any(|machine| {
                machine.symbol == plan.machine && machine.name.as_str() == "Main::forward"
            })
        })
        .expect("checker should publish Main::forward's exact structural-return plan");
    assert_eq!(plan.structural_parameters.len(), 1);
    assert_eq!(
        plan.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(plan.result.multiplicity, Multiplicity::Linear);
    assert_eq!(
        plan.structural_parameters[0].type_identity,
        plan.result.type_identity
    );
    assert_eq!(
        plan.structural_parameters[0].qualifications,
        plan.result.qualifications
    );
    assert_eq!(plan.returned_parameter_index, 0);
    assert!(plan.trivial_affine_discards.is_empty());
    assert_eq!(plan.entry_claim.claim_identity, plan.transferred_claim);
    assert!(plan.entry_claim.field_path.is_empty());

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward")
        .expect("exact whole-root passthrough should lower");
    let module = &lowered.semantic_module;
    let [machine] = module.machines.as_slice() else {
        panic!("one source machine should produce one terminal machine")
    };
    let TerminalMachineResult::Structural(result) = &machine.result else {
        panic!("source structural result should remain structural")
    };
    assert_eq!(machine.entry_claims.len(), 1);
    assert_eq!(machine.content_entry_claims.len(), 1);
    assert_eq!(machine.content_identity_reshuffles.len(), 1);
    let claim = machine.entry_claims[0].claim;
    assert_eq!(machine.content_entry_claims[0].claim, claim);
    assert_eq!(machine.content_identity_reshuffles[0].claim, claim);
    let Terminator::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("whole-root source return should be an ownership transfer")
    };
    assert_eq!(*source, machine.structural_parameters[0].place);
    assert_eq!(returned_claims, &[claim]);
    assert!(trivial_affine_discards.is_empty());

    let semantic = encode_module(module).expect("canonical structural semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced structural transfer verifies");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type: result.structural_type,
        qualifications: result.qualifications.clone(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("source-produced artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim]
    );
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: vec![claim],
            }
        ))
    );
}

#[test]
fn structural_return_discards_one_claim_free_affine_parameter_after_materialization() {
    let checked = checked_source();
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .find(|plan| {
            checked.machines().iter().any(|machine| {
                machine.symbol == plan.machine && machine.name.as_str() == "Main::forward_and_drop"
            })
        })
        .expect("checker should publish the exact structural return plus affine cleanup");
    assert_eq!(plan.structural_parameters.len(), 2);
    assert_eq!(plan.returned_parameter_index, 0);
    assert_eq!(
        plan.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(
        plan.structural_parameters[1].multiplicity,
        Multiplicity::Affine
    );
    assert_eq!(plan.trivial_affine_discards, [1]);

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_and_drop")
        .expect("exact structural return plus affine cleanup should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one source machine should produce one terminal machine")
    };
    let TerminalMachineResult::Structural(result) = &machine.result else {
        panic!("result should remain structural")
    };
    assert_eq!(machine.structural_parameters.len(), 2);
    let claim = machine.entry_claims[0].claim;
    let Terminator::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("return should transfer custody and discard affine scratch")
    };
    assert_eq!(*source, machine.structural_parameters[0].place);
    assert_eq!(returned_claims, &[claim]);
    assert_eq!(
        trivial_affine_discards,
        &[machine.structural_parameters[1].place]
    );

    let semantic = encode_module(&lowered.semantic_module).expect("semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier reconstructs the exact affine cleanup");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
    let returned = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type: result.structural_type,
        qualifications: result.qualifications.clone(),
    };
    let scratch_parameter = &machine.structural_parameters[1];
    let scratch = TerminalStructuralValue {
        opaque_identity: 0xcafe,
        structural_type: scratch_parameter.structural_type,
        qualifications: scratch_parameter.qualifications.clone(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[returned.clone(), scratch],
    )
    .expect("artifact starts with both structural inputs");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim]
    );
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: returned,
                claims: vec![claim],
            }
        ))
    );
}

#[test]
fn structural_return_establishes_and_discards_one_trivial_affine_local() {
    let checked = checked_source();
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .find(|plan| {
            checked.machines().iter().any(|machine| {
                machine.symbol == plan.machine
                    && machine.name.as_str() == "Main::forward_with_local"
            })
        })
        .expect("checker should publish the exact trivial affine local cleanup");
    assert_eq!(plan.trivial_affine_locals.len(), 1);
    assert_eq!(plan.trivial_affine_locals[0].declaration_ordinal, 0);
    assert_eq!(plan.trivial_affine_local_discard_ordinals, [0]);
    assert!(plan.trivial_affine_discards.is_empty());

    let lowered =
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local")
            .expect("exact affine local structural return should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one source machine should produce one terminal machine")
    };
    let [operation] = machine.blocks[0].operations.as_slice() else {
        panic!("one local establishment operation should be explicit")
    };
    let psi_terminal::OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind
    else {
        panic!("local should use the exact establishment operation")
    };
    assert!(matches!(
        machine
            .structural_places
            .iter()
            .find(|place| place.id == destination)
            .expect("local place declaration")
            .kind,
        psi_core::StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            ..
        }
    ));
    let psi_terminal::Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("local source still returns structural custody")
    };
    assert_eq!(trivial_affine_discards, &[destination]);

    let semantic = encode_module(&lowered.semantic_module).expect("semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier reconstructs local establishment and cleanup");
    let TerminalMachineResult::Structural(result) = &machine.result else {
        unreachable!()
    };
    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type: result.structural_type,
        qualifications: result.qualifications.clone(),
    };
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: vec![machine.entry_claims[0].claim],
            }
        ))
    );
}

#[test]
fn structural_return_cleans_local_before_affine_parameter() {
    let checked = checked_source();
    let lowered =
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local_and_drop")
            .expect("combined local and parameter cleanup should lower");
    let machine = &lowered.semantic_module.machines[0];
    let psi_terminal::OperationKind::EstablishTrivialAffineLocal { destination } =
        machine.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let psi_terminal::Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        trivial_affine_discards,
        &[destination, machine.structural_parameters[1].place]
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier should reconstruct local-before-parameter order");
}

#[test]
fn producer_fences_locals_and_authored_contracts() {
    let checked = checked_source();
    let planned_names = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .map(|plan| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == plan.machine)
                .expect("plan machine remains present")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        planned_names,
        [
            "Main::forward",
            "Main::forward_and_drop",
            "Main::forward_with_local",
            "Main::forward_with_local_and_drop"
        ]
    );
    assert!(psi_checked_trees_to_terminal::lower_machine(&checked, "Main::through_local").is_err());
    assert!(psi_checked_trees_to_terminal::lower_machine(&checked, "Main::contracted").is_err());
    assert!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::too_many_discards").is_err()
    );
    for rejected in [
        "Main::local_partial_value",
        "Main::local_nominal_cleanup",
        "Main::too_many_locals",
        "Main::local_control",
    ] {
        assert!(
            psi_checked_trees_to_terminal::lower_machine(&checked, rejected).is_err(),
            "{rejected} must remain outside the exact affine-local slice"
        );
    }
}

#[test]
fn lowering_rejects_a_stale_checked_claim_join() {
    let mut checked = checked_source();
    let forward_symbol = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::forward")
        .expect("forward machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == forward_symbol)
        .expect("forward plan");
    plan.transferred_claim = PermissionClaimIdentity::Unknown;
    assert!(matches!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward"),
        Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(
            "structural result plan is not one exact whole-root linear transfer with affine cleanup"
        ))
    ));
}

#[test]
fn lowering_rejects_stale_structural_return_cleanup_coordinates() {
    let mut checked = checked_source();
    let symbol = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::forward_and_drop")
        .expect("forward-and-drop machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .expect("forward-and-drop plan");
    plan.trivial_affine_discards.clear();
    assert!(matches!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_and_drop"),
        Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(
            "structural result plan is not one exact whole-root linear transfer with affine cleanup"
        ))
    ));
}

#[test]
fn lowering_rejects_stale_affine_local_declaration_and_cleanup_rows() {
    fn checked_plan() -> (psi_checked_trees::CheckedTrees, psi_symbols::SymbolHandle) {
        let checked = checked_source();
        let symbol = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::forward_with_local")
            .expect("forward-with-local machine")
            .symbol;
        (checked, symbol)
    }

    let (mut checked, symbol) = checked_plan();
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .unwrap();
    plan.trivial_affine_local_discard_ordinals.clear();
    assert!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local").is_err()
    );

    let (mut checked, symbol) = checked_plan();
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .unwrap();
    plan.trivial_affine_locals[0].declaration_ordinal = 1;
    plan.trivial_affine_local_discard_ordinals[0] = 1;
    assert!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local").is_err()
    );

    let (mut checked, symbol) = checked_plan();
    let scratch_type = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .find(|plan| {
            checked.machines().iter().any(|machine| {
                machine.symbol == plan.machine && machine.name.as_str() == "Main::forward_and_drop"
            })
        })
        .unwrap()
        .structural_parameters[1]
        .type_identity
        .clone();
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .unwrap();
    plan.trivial_affine_locals[0].type_identity = scratch_type;
    assert!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local").is_err()
    );
}
