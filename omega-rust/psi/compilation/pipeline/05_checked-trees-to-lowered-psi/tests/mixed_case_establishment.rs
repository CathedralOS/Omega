//! A mixed-shape case literal establishes its runtime common fields, then the
//! selected payload, and zero-initializes every common field it leaves
//! unnamed (`checking::value_custody::data::validate_data_shape`).
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use terminal_psi::{OperationKind, RecordFieldValue, StructuralTypeShape};

const SOURCE: &str = r#"
    data Event {
        consumed: bool;
        bonus: i32;
        case Nothing;
        case Treasure(gold: i32);
    }
    data Main { event: Event; }
    machine Main::run(&mut self) {
        self.event = Event::Treasure { gold: 50, consumed: true };
    }
"#;

#[test]
fn owned_mixed_value_forwarding_retains_the_transitive_body() {
    let source = r#"
        data Message { stamp: i32; case Empty; case Number(value: i32); }
        machine relay(message: Message) -> Message { message }
        machine forward(message: Message) -> Message { relay(message) }
        "#;
    for source in [
        source.to_owned(),
        source.replace(
            "relay(message) }",
            "let staged: Message = relay(message); relay(staged) }",
        ),
    ] {
        let checked = crate::front_end::checked_program(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("forward"))
            .expect("forwarding body remains in the exact call closure");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("forwarding independently verifies");
        let encoded = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
        assert_eq!(
            terminal_codec::decode_module(&encoded).expect("decode"),
            lowered.semantic_module
        );
    }
}

#[test]
fn mixed_case_literal_zero_initializes_unnamed_common_fields() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("a mixed case literal lowers");
    let module = &lowered.semantic_module;
    let operations = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let [establishment] = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishStructuralCase { .. }
            )
        })
        .collect::<Vec<_>>()[..]
    else {
        panic!("one mixed case establishment");
    };
    let OperationKind::EstablishStructuralCase { fields, .. } = &establishment.kind else {
        unreachable!()
    };
    let result = establishment
        .result
        .structural()
        .expect("structural result");
    let StructuralTypeShape::Mixed {
        fields: common,
        cases,
    } = &module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .expect("mixed declaration")
        .shape
    else {
        panic!("the literal establishes the mixed shape");
    };
    let treasure = cases
        .iter()
        .find(|case| case.identity.ends_with("Treasure"))
        .expect("treasure case");
    // Declaration order, not authored order: consumed, bonus, then gold.
    let expected = common
        .iter()
        .chain(&treasure.fields)
        .map(|field| field.id)
        .collect::<Vec<_>>();
    assert_eq!(
        fields.iter().map(|field| field.field).collect::<Vec<_>>(),
        expected
    );
    let value_of = |position: usize| {
        let RecordFieldValue::Scalar { value, .. } = fields[position].value else {
            panic!("scalar roster entry");
        };
        operations
            .iter()
            .find(|operation| {
                operation
                    .result
                    .scalar()
                    .is_some_and(|result| result.id == value)
            })
            .map(|operation| operation.kind.clone())
            .expect("defined roster value")
    };
    assert_eq!(
        value_of(1),
        OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Signed(0),
        },
        "the unnamed `bonus` common field establishes as zero"
    );
    terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the mixed establishment independently verifies");
    // Decoding replays the codec's own foundation validation of the roster.
    let bytes = terminal_codec::encode_module(module).expect("encode");
    assert_eq!(
        &terminal_codec::decode_module(&bytes).expect("decode the mixed roster"),
        module
    );
}

/// An erased payload member holds no runtime slot: the literal may omit it
/// when its unique nullary constructor determines it, and the establishment
/// binds only the runtime payload.
#[test]
fn case_literal_leaves_erased_payload_members_out_of_the_roster() {
    let checked = crate::front_end::checked_program(
        r#"
        data Evidence { case Only; }
        data Certified {
            case Pending;
            case Proven(value: i32, proof [erased]: Evidence);
        }
        data Main { record: Certified; }
        machine Main::run(&mut self) {
            self.record = Certified::Proven { value: 70 };
        }
    "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("a case with an erased payload member lowers");
    let module = &lowered.semantic_module;
    let [fields] = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishStructuralCase { fields, .. } => Some(fields),
            _ => None,
        })
        .collect::<Vec<_>>()[..]
    else {
        panic!("one case establishment");
    };
    assert_eq!(fields.len(), 1, "only `value` takes a roster position");
    terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the erased-member establishment independently verifies");
    let bytes = terminal_codec::encode_module(module).expect("encode");
    assert_eq!(
        &terminal_codec::decode_module(&bytes).expect("decode the roster"),
        module
    );
}

/// A payload member under a plain domain that means exactly an interval is a
/// bounded carrier: construction binds it with a range obligation that
/// independent verification proves from the literal. Such a member is
/// numeric-owned scalar content, so the payload-only sum establishes through
/// the scalar case family.
#[test]
fn case_literal_binds_an_interval_domain_payload_with_its_range_obligation() {
    let checked = crate::front_end::checked_program(
        r#"
        domain i32::Ten requires 0 <= self && self <= 10;
        data Cmd { case Dim(amount: i32 in Ten); case Off; }
        data Main { command: Cmd; }
        machine Main::run(&mut self) {
            self.command = Cmd::Dim { amount: 7 };
        }
    "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("an interval-domain payload lowers");
    let module = &lowered.semantic_module;
    let [fields] = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishScalarCase { fields, .. } => Some(fields),
            _ => None,
        })
        .collect::<Vec<_>>()[..]
    else {
        panic!("one case establishment");
    };
    let [amount] = fields.as_slice() else {
        panic!("one payload member");
    };
    assert!(amount.range_obligation.is_some());
    terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the range obligation independently verifies");
}

/// A common field sits at one offset in every case, so `self.event.bonus = 70`
/// stores it directly -- a `Field` path to the mixed carrier and the common
/// field's id, no `Case` step -- and verification admits exactly that field.
#[test]
fn mixed_common_field_store_writes_without_a_case_step() {
    let checked = crate::front_end::checked_program(
        r#"
        data Event {
            consumed: bool;
            bonus: i32;
            case Nothing;
            case Treasure(gold: i32);
        }
        data Main { event: Event; }
        machine Main::run(&mut self) {
            self.event.bonus = 70;
        }
    "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("a mixed common-field store lowers");
    let module = &lowered.semantic_module;
    let StructuralTypeShape::Mixed { fields: common, .. } = &module
        .structural_types
        .iter()
        .find(|declaration| declaration.identity.contains("Event"))
        .expect("the mixed declaration")
        .shape
    else {
        panic!("Event lowers as a mixed shape");
    };
    let bonus = common
        .iter()
        .find(|field| field.identity.contains("bonus"))
        .expect("the common `bonus` field")
        .id;
    let stores = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::StructuralScalarFieldStore { path, field, .. } => Some((path, *field)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(path, field)] = stores.as_slice() else {
        panic!("one common-field store, found {stores:?}");
    };
    assert_eq!(*field, bonus);
    assert!(
        matches!(path.as_slice(), [terminal_psi::StructuralPathSegment::Field(event)] if event.contains("event")),
        "the store reaches the carrier by field alone: {path:?}"
    );
    terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the common-field store verifies");

    // A store naming no common field of the carrier is not a direct field
    // write, whatever the payloads declare.
    let mut forged = module.clone();
    for operation in forged
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
    {
        if let OperationKind::StructuralScalarFieldStore { field, .. } = &mut operation.kind {
            *field = semantic_vocabulary::StructuralFieldId::new(99).unwrap();
        }
    }
    assert!(
        terminal_verifier::verify_module(
            &forged,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "a store of an undeclared common field rejects"
    );
}

/// Coverage ranges over a mixed value's cases only: its common fields select
/// nothing, so a dispatch naming every case needs no fallback arm, with two
/// arms as with three.
#[test]
fn exhaustive_mixed_dispatch_needs_no_fallback() {
    for (source, machine) in [
        (
            r#"
            data Event { bonus: i32; case Nothing; case Treasure(gold: i32); }
            data Main { event: Event; }
            machine Main::run(&mut self) -> i32 {
                transition self.event {
                    Event::Nothing -> none()
                    Event::Treasure { gold } -> found(gold)
                }
                state none(&mut self) -> i32 { 1 }
                state found(&mut self, gold: i32) -> i32 { gold }
            }
        "#,
            "Main::run",
        ),
        (
            r#"
            data Event { bonus: i32; case Nothing; case Treasure(gold: i32); case Enemy(strength: i32); }
            data Main { event: Event; }
            machine Main::run(&mut self) -> i32 {
                transition self.event {
                    Event::Nothing -> none()
                    Event::Treasure { gold } -> found(gold)
                    Event::Enemy { strength } -> found(strength)
                }
                state none(&mut self) -> i32 { 1 }
                state found(&mut self, value: i32) -> i32 { value }
            }
        "#,
            "Main::run",
        ),
    ] {
        let checked = crate::front_end::checked_program(source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name(machine))
            .expect("an exhaustive mixed dispatch lowers");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("an exhaustive mixed dispatch verifies");
    }
}

/// Source for the interpreted mixed-establishment tests: the common `bonus`
/// is recorded without case knowledge, the common `consumed` selects the
/// payload read, and the `Treasure` payload records `gold`.
const OBSERVED_MIXED_SOURCE: &str = r#"
    boundary trait Sink { machine record(value: i32); }
    data Event {
        consumed: bool;
        bonus: i32;
        case Nothing;
        case Treasure(gold: i32);
    }
    machine observe(event: Event) reaches Sink {
        Sink::record(event.bonus);
        transition event.consumed {
            true -> payload(event)
            false -> unconsumed()
        }
        state payload(event: Event) {
            transition event {
                Event::Treasure { gold } -> treasure(gold)
                Event::Nothing -> nothing()
            }
        }
        state treasure(gold: i32) { Sink::record(gold); }
        state nothing() { Sink::record(1); }
        state unconsumed() { Sink::record(2); }
    }
    data Main {}
    machine Main::main() reaches Sink {
        observe(Event::Treasure { consumed: true, bonus: 3, gold: 50 });
    }
"#;

/// Run a verified artifact to completion and return the i32 values its
/// `Sink::record` effects carried, in order.
fn recorded_by_interpreter(semantic: &[u8], proof: &[u8]) -> Vec<i128> {
    use terminal_interpreter::{
        AcceptTerminalEffects, TerminalEffect, TerminalExecution, TerminalExecutionResult,
        TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralInputs,
    };
    let mut execution = TerminalExecution::start_artifact(
        semantic,
        proof,
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("the verified artifact starts");
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    for _ in 0..512 {
        match execution
            .resume(&mut fuel, &mut AcceptTerminalEffects)
            .expect("execution resumes")
        {
            TerminalExecutionStatus::SponsorExhausted(_) => fuel.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                return execution
                    .effects()
                    .iter()
                    .map(|effect| {
                        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                            panic!("only Sink recording effects");
                        };
                        let [TerminalScalarValue::Integer { value, .. }] = arguments.as_slice()
                        else {
                            panic!("one i32 recording argument");
                        };
                        match value {
                            semantic_vocabulary::IntegerValue::Signed(value) => *value,
                            other => panic!("signed recording argument, found {other:?}"),
                        }
                    })
                    .collect();
            }
            other => panic!("mixed observation execution: {other:?}"),
        }
    }
    panic!("mixed observation execution did not complete");
}

/// The terminal interpreter executes a mixed establishment: the roster binds
/// the runtime common fields, then the payload, and each field reads back
/// through its own route -- a common field without a `Case` step, a payload
/// field through one.
#[test]
fn interpreted_mixed_establishment_reads_common_fields_and_payload() {
    let checked = crate::front_end::checked_program(OBSERVED_MIXED_SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(
        terminal_production::TerminalProductionCustody::artifact_only(
            &mut terminal_production::TerminalProductionTimings::default(),
        ),
    )
    .map(|produced| produced.into_artifact())
    .unwrap_or_else(|error| panic!("the mixed observation lowers: {error:?}"));
    assert_eq!(
        recorded_by_interpreter(artifact.semantic_bytes(), artifact.proof_bytes()),
        vec![3, 50],
        "the common `bonus`, then the `Treasure` payload `gold`"
    );
}

/// Common and payload field identities are separate Terminal namespaces
/// (`validation/structural/case.rs` matches a roster by position), so a
/// verified module may number `gold` exactly like the common `consumed`.
/// This producer numbers them apart; renumbering `gold` onto `consumed`'s
/// id keeps the module verified, and the interpreter must still keep the two
/// cells apart -- a runtime keyed by field id alone would read the Boolean
/// `consumed` back as `gold`.
#[test]
fn interpreted_mixed_establishment_keeps_colliding_field_ids_apart() {
    let checked = crate::front_end::checked_program(OBSERVED_MIXED_SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("the mixed observation lowers");
    let mut module = lowered.semantic_module.clone();
    let event = module
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.identity.contains("Event"))
        .expect("the mixed declaration");
    let StructuralTypeShape::Mixed {
        fields: common,
        cases,
    } = &mut event.shape
    else {
        panic!("Event lowers as a mixed shape");
    };
    let consumed = common
        .iter()
        .find(|field| field.identity.contains("consumed"))
        .expect("the common `consumed` field")
        .id;
    let gold_declaration = cases
        .iter_mut()
        .flat_map(|case| &mut case.fields)
        .find(|field| field.identity.contains("gold"))
        .expect("the `gold` payload field");
    let gold = gold_declaration.id;
    assert_ne!(gold, consumed, "the producer numbers the namespaces apart");
    gold_declaration.id = consumed;

    // Every reference to `gold` names it by id beside a `Case` step (or as a
    // payload binding), never on the common route; renumber exactly those.
    let mut renumbered = 0;
    for block in module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
    {
        for operation in &mut block.operations {
            match &mut operation.kind {
                OperationKind::EstablishStructuralCase { fields, .. } => {
                    for binding in fields.iter_mut().filter(|binding| binding.field == gold) {
                        binding.field = consumed;
                        renumbered += 1;
                    }
                }
                OperationKind::IntegerStructuralField { path, field, .. }
                    if *field == gold
                        && matches!(
                            path.last(),
                            Some(semantic_vocabulary::CanonicalStructuralPathSegment::Case(_))
                        ) =>
                {
                    *field = consumed;
                    renumbered += 1;
                }
                _ => {}
            }
        }
        if let terminal_psi::Terminator::StructuralCase { cases, .. } = &mut block.terminator {
            for field in cases
                .iter_mut()
                .flat_map(|case| &mut case.payload_fields)
                .filter(|field| **field == gold)
            {
                *field = consumed;
                renumbered += 1;
            }
        }
    }
    assert!(
        renumbered >= 2,
        "the establishment and the payload read both name `gold`"
    );
    terminal_verifier::verify_module(
        &module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("overlapping common and payload ids verify");
    let semantic = terminal_codec::encode_module(&module).expect("encode");
    let proof =
        terminal_codec::encode_proof_section(&module, &lowered.proof_bundle).expect("encode");
    assert_eq!(
        recorded_by_interpreter(&semantic, &proof),
        vec![3, 50],
        "`consumed` and `gold` keep their own cells"
    );
}
