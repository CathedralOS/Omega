use crate::CheckingRequest;
use crate::tests::front_end::{typed_program, typed_program_result};
use crate::tests::lower_typed_trees;
use checked_trees::ContractProofFactOwner;

#[test]
fn exact_requirement_lifetime_application_retains_raw_machine_ordinals() {
    let source = r#"
        trait Pair<'left, 'right> {
            machine choose(first: &'left [u8], second: &'right [u8]) -> &'left [u8];
        }

        machine choose<'unused, 'x, 'y>(
            first: &'x [u8],
            second: &'y [u8]
        ) -> &'x [u8]
            satisfies Pair<'x, 'y>::choose
        {
            first
        }
    "#;
    let typed = typed_program_result(source).expect("typed exact requirement application");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("realizing machine");
    let [realization] = typed.machine_trait_conformances(machine) else {
        panic!("one exact requirement realization")
    };
    assert_eq!(realization.trait_lifetime_arguments, [1, 2]);
    assert_eq!(
        typed_trees::machine::normalize_requirement_lifetime_partition(
            &realization.trait_lifetime_arguments,
        ),
        [0, 1],
    );
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("declared lifetime substitution should validate");
}

#[test]
fn exact_requirement_lifetime_application_accepts_repeated_realizer_binder() {
    let source = r#"
        trait Pair<'left, 'right> {
            machine consume(first: &'left [u8], second: &'right [u8]);
        }

        machine consume<'x>(first: &'x [u8], second: &'x [u8])
            satisfies Pair<'x, 'x>::consume
        {
        }
    "#;
    let typed = typed_program_result(source).expect("typed repeated lifetime application");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("realizing machine");
    let [realization] = typed.machine_trait_conformances(machine) else {
        panic!("one exact requirement realization")
    };
    assert_eq!(realization.trait_lifetime_arguments, [0, 0]);
    assert_eq!(
        typed_trees::machine::normalize_requirement_lifetime_partition(
            &realization.trait_lifetime_arguments,
        ),
        [0, 0],
    );
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("repeated lifetime substitution should validate");
}

#[test]
fn exact_requirement_lifetime_application_rejects_signature_substitution_drift() {
    let source = r#"
        trait Pair<'left, 'right> {
            machine choose(first: &'left [u8], second: &'right [u8]) -> &'left [u8];
        }

        machine choose<'x, 'y>(first: &'x [u8], second: &'y [u8]) -> &'x [u8]
            satisfies Pair<'y, 'x>::choose
        {
            first
        }
    "#;
    let typed = typed_program_result(source).expect("typed mismatched lifetime application");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("lifetime drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("lifetime application does not match the declared `satisfies` edge")
    }));
}

#[test]
fn exact_requirement_lifetime_application_requires_complete_in_scope_arguments() {
    let missing = r#"
        trait Reads<'view> {
            machine read(value: &'view [u8]) -> &'view [u8];
        }
        machine read<'scope>(value: &'scope [u8]) -> &'scope [u8]
            satisfies Reads::read
        {
            value
        }
    "#;
    let typed =
        typed_program_result(missing).expect("typing retains missing application for validation");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("missing lifetime must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expects 1 target-trait lifetime argument(s), got 0")
    }));

    let foreign = r#"
        trait Reads<'view> {
            machine read(value: &'view [u8]) -> &'view [u8];
        }
        machine read<'scope>(value: &'scope [u8]) -> &'scope [u8]
            satisfies Reads<'foreign>::read
        {
            value
        }
    "#;
    let error = typed_program_result(foreign)
        .expect_err("foreign lifetime argument must reject during typed lowering")
        .remove(0);
    assert!(error.message.contains("outside its lifetime telescope"));
}

#[test]
fn nested_nominal_machine_parameter_uses_trait_evidence_without_binder_expansion() {
    let source = r#"
        trait Handler {
            machine call(value: bool)
                requires value
                crashes Abort
                    value;
        }

        machine outer<machine Schema>()
        where machine Schema<machine Nested>()
        where machine Nested satisfies Handler::call;
        {
        }
    "#;

    let typed = typed_program(source);

    let outer = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "outer")
        .expect("outer machine");
    let schema = typed
        .machine_type_parameters(outer)
        .first()
        .expect("Schema parameter");
    let typed_trees::data::TypeParameterKind::Machine {
        contract: typed_trees::data::MachineParameterContract::Structural(schema_signature),
    } = &schema.kind
    else {
        panic!("Schema should retain a structural signature")
    };
    let nested = typed
        .state_signature_type_parameters(schema_signature)
        .first()
        .expect("Nested parameter");
    assert!(matches!(
        nested.kind,
        typed_trees::data::TypeParameterKind::Machine {
            contract: typed_trees::data::MachineParameterContract::Nominal { .. }
        }
    ));
    let nested_owner = nested.symbol;

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("nested nominal reference should check");

    assert!(!checked.facts.proof.contract_facts.iter().any(|(_, fact)| {
        matches!(
            fact.owner,
            ContractProofFactOwner::StateSignature { owner_symbol, .. }
                if owner_symbol == nested_owner
        )
    }));
    assert!(
        checked
            .facts
            .contract_plans
            .crash_capsules
            .iter()
            .all(|capsule| capsule.target_machine() != nested_owner),
        "a nested nominal binder must not duplicate its trait requirement capsule"
    );
}

#[test]
fn bounded_installation_reach_retains_exact_unresolved_requirement_through_checked_facts() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary trait InterruptCompletion {
            machine complete() -> u64
            reaches <= MachineControl + PortIo;
        }

        machine pic_complete() -> u64
        satisfies InterruptCompletion::complete
        reaches PortIo
        {
            0
        }

        machine invoke<machine Completion>() -> u64
        where machine Completion satisfies InterruptCompletion::complete;
        {
            Completion()
        }

        machine outer<machine Completion>() -> u64
        where machine Completion satisfies InterruptCompletion::complete;
        {
            invoke<Completion>()
        }
    "#;
    let typed = typed_program(source);
    let requirement = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "InterruptCompletion")
        .and_then(|definition| typed.trait_machine_signatures(definition).first())
        .expect("InterruptCompletion::complete");
    let requirement_symbol = requirement.symbol;
    let upper_bound = requirement.service_reach_row;
    let invoke_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke")
        .expect("invoke machine")
        .symbol;
    let outer_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "outer")
        .expect("outer machine")
        .symbol;

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("bounded reach closure should check");
    let reach = checked
        .facts
        .service_reaches
        .for_machine(invoke_symbol)
        .expect("invoke reach facts");

    assert_eq!(
        reach.unresolved_installation_reaches,
        [flow_effects::InstallationReachRequirement {
            requirement: requirement_symbol,
            upper_bound,
        }]
    );
    let names = checked
        .facts
        .service_reaches
        .rows
        .services(reach.effective)
        .iter()
        .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
        .map(|definition| definition.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["MachineControl", "PortIo"]);
    assert!(
        checked
            .facts
            .service_reaches
            .rows
            .services(reach.concrete_effective)
            .is_empty(),
        "the abstract upper bound must not enter concrete reach"
    );
    assert!(
        checked
            .facts
            .service_reaches
            .rows
            .services(reach.concrete_transitive)
            .is_empty(),
        "the checked body's preselection reach must exclude the abstract upper bound"
    );
    let outer_reach = checked
        .facts
        .service_reaches
        .for_machine(outer_symbol)
        .expect("outer reach facts");
    assert_eq!(
        outer_reach.unresolved_installation_reaches,
        reach.unresolved_installation_reaches
    );
    assert!(
        checked
            .facts
            .service_reaches
            .rows
            .services(outer_reach.concrete_transitive)
            .is_empty(),
        "a wrapper must preserve the callee's unresolved upper bound distinction"
    );
    assert_eq!(
        checked
            .facts
            .contract_plans
            .realized_envelope(outer_symbol)
            .expect("outer realized envelope")
            .unresolved_installation_reaches,
        reach.unresolved_installation_reaches
    );
    assert!(
        checked
            .facts
            .contract_plans
            .realized_envelope(outer_symbol)
            .expect("outer realized envelope")
            .concrete_service_reach
            .is_empty()
    );
}

#[test]
fn nominal_callback_use_retains_exact_evaluated_placement_identity() {
    let source = r#"
        pub boundary trait Handler {
            machine call(value: i32) -> i32;
        }

        boundary machine chosen(value: i32) -> i32
        satisfies Handler::call
        {
            value
        }

        boundary machine register<machine Selected>(value: i32) -> i32
        where machine Selected satisfies Handler::call;
        {
            Selected(value)
        }

        machine caller(value: i32) -> i32 {
            register<chosen>(value)
        }
    "#;
    let mut typed = typed_program(source);
    let handler = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Handler")
        .expect("Handler boundary trait");
    let handler_symbol = handler.symbol;
    let requirement_symbol = typed
        .trait_machine_signatures(handler)
        .first()
        .expect("Handler::call")
        .symbol;
    let expected_fingerprint = 0x2a7c_6b19_d331_85e1;
    typed.record_boundary_calling_plan(typed_trees::typed_trees::BoundaryCallingPlanIdentity {
        boundary_trait: handler_symbol,
        boundary_arguments: Vec::new(),
        requirement_machine: requirement_symbol,
        report_fingerprint: expected_fingerprint,
        commitment: typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
            [0x2a; 32],
        ),
    });

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("nominal callback selection should check");
    let [nominal_use] = checked.facts.nominal_machine_uses.uses.as_slice() else {
        panic!("one nominal callback use")
    };

    let callback_placement = nominal_use
        .callback_placement
        .expect("nominal callback placement identity");
    assert_eq!(
        callback_placement.boundary_calling_plan_report_fingerprint,
        expected_fingerprint
    );
    let published = checked
        .facts
        .contract_plans
        .crash_capsule(
            nominal_use.satisfaction_trait,
            nominal_use.satisfaction_requirement,
        )
        .expect("callback requirement envelope");
    assert_eq!(
        published.target_contract_report_fingerprint(),
        nominal_use
            .published_requirement_envelope
            .contract_report_fingerprint
    );
    assert!(published.published_service_reach().is_empty());
    assert!(published.published_synchronous_invocations().is_empty());
    assert!(!published.published_may_suspend());
    assert!(!published.published_may_block());
    assert_eq!(
        published.published_termination(),
        &language_semantics::TerminationGuarantee::NoGuarantee
    );

    let actual = checked
        .facts
        .contract_plans
        .realized_envelope(nominal_use.selected_machine)
        .expect("selected callback actual envelope");
    assert_eq!(
        actual.contract_report_fingerprint,
        nominal_use
            .selected_actual_envelope
            .contract_report_fingerprint
    );
    assert!(actual.effective_service_reach.is_empty());
    assert!(actual.effective_synchronous_invocations.is_empty());
    assert!(!actual.checked_may_suspend);
    assert!(!actual.checked_may_block);
    assert!(actual.capabilities.is_empty());
    let resources = checked
        .facts
        .contract_plans
        .resource_envelope(nominal_use.selected_machine, nominal_use.selected_entry)
        .expect("selected callback checked resource anchor");
    assert_eq!(resources.machine(), nominal_use.selected_machine);
    assert_eq!(resources.entry(), nominal_use.selected_entry);
    assert_eq!(
        resources.contract_report_fingerprint(),
        nominal_use
            .selected_actual_envelope
            .contract_report_fingerprint
    );
    assert_eq!(
        resources.stack().derivation_obligation(),
        checked_trees::CheckedResourceDerivationObligation::TerminalAndTargetStackClosure
    );
    assert_eq!(
        resources.logical_structural_work().derivation_obligation(),
        checked_trees::CheckedResourceDerivationObligation::TerminalControlAndFuelSchedule
    );
    assert_eq!(
        resources.machine_state().derivation_obligation(),
        checked_trees::CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint
    );
    callback_placement
        .resource_receipt
        .validate_against(resources)
        .expect("callback placement must retain the exact selected entry resource anchor");
    checked
        .facts
        .contract_plans
        .validate_resource_envelopes()
        .expect("resource anchors must independently replay from exact contracts");
}
