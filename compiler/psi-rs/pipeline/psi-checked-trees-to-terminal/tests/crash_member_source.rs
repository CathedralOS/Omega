use psi_core::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, Proposition, ScalarTerm,
    StructuralFieldId,
};
use psi_proof_kernel::{AdmissionProfile, EvidenceRoute, ProofRule};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    CrashPredicateTerm, CrashRouteGuard, OperationKind, StructuralFieldType, StructuralPathSegment,
    StructuralTypeShape,
};
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalStructuralValue, interpret_terminal_artifact_with_effect_handler_measured,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Packet { should_abort: bool; }
    data Helper {}
    machine Helper::inspect(packet: Packet)
    crashes Abort
        packet.should_abort
    {}

    data Root {}
    machine Root::enter(packet: Packet)
    crashes Abort
        packet.should_abort
    {
        Helper::inspect(packet);
    }
"#;

const NESTED_SOURCE: &str = r#"
    data AbortState { should_abort: bool; }
    data Packet { state: AbortState; }
    data Helper {}
    machine Helper::inspect(packet: Packet)
    crashes Abort
        packet.state.should_abort
    {}

    data Root {}
    machine Root::enter(packet: Packet)
    crashes Abort
        packet.state.should_abort
    {
        Helper::inspect(packet);
    }
"#;

const PROJECTED_SOURCE: &str = r#"
    data AbortState { should_abort: bool; }
    data Packet { state: AbortState; }
    data Spare { value: u64; }
    data Envelope { packet: Packet; spare: Spare; }
    data Helper {}
    machine Helper::inspect(packet: Packet)
    crashes Abort
        packet.state.should_abort
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.packet.state.should_abort
    {
        Helper::inspect(envelope.packet);
    }
"#;

const FIXED_INDEX_SOURCE: &str = r#"
    boundary trait PortIo {}
    data Receipt [linear] { should_abort: bool; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Helper {}
    machine Helper::inspect(receipt: Receipt)
    reaches PortIo
    crashes Abort
        receipt.should_abort
    {
        Receipt::settle(receipt);
    }

    data Root {}
    machine Root::enter(receipts: [Receipt; 1])
    reaches PortIo
    crashes Abort
    {
        Helper::inspect(receipts[0]);
    }
"#;

const COMPOSED_MEMBER_SOURCE: &str = r#"
    data Flag { active: bool; }
    data Pair { left: Flag; right: Flag; armed: bool; }
    data Spare {}
    data Envelope { pair: Pair; spare: Spare; }

    data Helper {}
    machine Helper::inspect(pair: Pair)
    crashes Abort
        pair.left.active == !pair.right.active && pair.armed
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.pair.left.active == !envelope.pair.right.active && envelope.pair.armed
    {
        Helper::inspect(envelope.pair);
    }
"#;

const INTEGER_MEMBER_SOURCE: &str = r#"
    data Metrics { current: u64; limit: u64; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.limit <= metrics.current && metrics.current != metrics.limit
    {}

    data Root {}
    machine Root::enter(metrics: Metrics)
    crashes Abort
        metrics.limit <= metrics.current && metrics.current != metrics.limit
    {
        Helper::inspect(metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_SOURCE: &str = r#"
    data Metrics { current: u64; limit: u64; }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.limit <= metrics.current && metrics.current != metrics.limit
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.limit <= envelope.batch.metrics.current
            && envelope.batch.metrics.current != envelope.batch.metrics.limit
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_ARITHMETIC_SOURCE: &str = r#"
    data Metrics {
        current: u64 [0..=100];
        delta: u64 [0..=100];
        limit: u64;
    }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.current + metrics.delta <= metrics.limit
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.current + envelope.batch.metrics.delta
            <= envelope.batch.metrics.limit
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_SUBTRACTION_SOURCE: &str = r#"
    data Metrics {
        current: u64 [100..=200];
        delta: u64 [0..=100];
        floor: u64;
    }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.floor <= metrics.current - metrics.delta
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.floor
            <= envelope.batch.metrics.current - envelope.batch.metrics.delta
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_MULTIPLICATION_SOURCE: &str = r#"
    data Metrics {
        current: u64 [0..=10];
        factor: u64 [0..=10];
        limit: u64;
    }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.current * metrics.factor <= metrics.limit
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.current * envelope.batch.metrics.factor
            <= envelope.batch.metrics.limit
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_DIVISION_SOURCE: &str = r#"
    data Metrics { current: u64; limit: u64; parity: u64; }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.current / 2u64 <= metrics.limit
            && metrics.current % 2u64 == metrics.parity
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.current / 2u64 <= envelope.batch.metrics.limit
            && envelope.batch.metrics.current % 2u64
                == envelope.batch.metrics.parity
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_BITWISE_SOURCE: &str = r#"
    data Bits { value: u8; other: u8; mask: u8; expected: u8; }
    data Envelope { bits: Bits; spare: Bits; }
    data Helper {}
    machine Helper::inspect(bits: Bits)
    crashes Abort
        (bits.value & bits.mask) == bits.expected
            && (bits.value | bits.other) != bits.expected
            && (bits.value ^ bits.other) <= bits.expected
            && ~bits.value == bits.other
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        (envelope.bits.value & envelope.bits.mask) == envelope.bits.expected
            && (envelope.bits.value | envelope.bits.other) != envelope.bits.expected
            && (envelope.bits.value ^ envelope.bits.other) <= envelope.bits.expected
            && ~envelope.bits.value == envelope.bits.other
    {
        Helper::inspect(envelope.bits);
    }
"#;

const PROJECTED_INTEGER_MEMBER_POLICY_ARITHMETIC_SOURCE: &str = r#"
    data PolicyValues {
        wrapping_left: u8 in Wrapping;
        wrapping_right: u8 in Wrapping;
        wrapping_expected: u8 in Wrapping;
        saturating_left: i8 in Saturating;
        saturating_right: i8 in Saturating;
        saturating_expected: i8 in Saturating;
    }
    data Envelope { values: PolicyValues; spare: PolicyValues; }
    data Helper {}
    machine Helper::inspect(values: PolicyValues)
    crashes Abort
        values.wrapping_left + values.wrapping_right == values.wrapping_expected
            && values.wrapping_left - values.wrapping_right == values.wrapping_expected
            && values.wrapping_left * values.wrapping_right == values.wrapping_expected
            && values.saturating_left + values.saturating_right == values.saturating_expected
            && values.saturating_left - values.saturating_right == values.saturating_expected
            && values.saturating_left * values.saturating_right == values.saturating_expected
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.values.wrapping_left + envelope.values.wrapping_right
                == envelope.values.wrapping_expected
            && envelope.values.wrapping_left - envelope.values.wrapping_right
                == envelope.values.wrapping_expected
            && envelope.values.wrapping_left * envelope.values.wrapping_right
                == envelope.values.wrapping_expected
            && envelope.values.saturating_left + envelope.values.saturating_right
                == envelope.values.saturating_expected
            && envelope.values.saturating_left - envelope.values.saturating_right
                == envelope.values.saturating_expected
            && envelope.values.saturating_left * envelope.values.saturating_right
                == envelope.values.saturating_expected
    {
        Helper::inspect(envelope.values);
    }
"#;

const RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE: &str = r#"
    data Metrics { current: u64; divisor: u64; limit: u64; }
    data Root {}
    machine Root::enter(metrics: Metrics)
    requires
        1 <= metrics.divisor
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {}
"#;

const UNPROVEN_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE: &str = r#"
    data Metrics { current: u64; divisor: u64; limit: u64; }
    data Root {}
    machine Root::enter(metrics: Metrics)
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {}
"#;

const NEGATIVE_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE: &str = r#"
    data Metrics { current: i64; divisor: i64; limit: i64; }
    data Root {}
    machine Root::enter(metrics: Metrics)
    requires
        metrics.divisor <= -2
    crashes Abort
        metrics.current % metrics.divisor <= metrics.limit
    {}
"#;

const RUNTIME_DIVISOR_CALL_SOURCE: &str = r#"
    data Metrics { current: u64; divisor: u64; limit: u64; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    requires
        1 <= metrics.divisor
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {}

    data Root {}
    machine Root::enter(metrics: Metrics)
    requires
        1 <= metrics.divisor
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {
        Helper::inspect(metrics);
    }
"#;

const PROJECTED_RUNTIME_DIVISOR_CALL_SOURCE: &str = r#"
    data Metrics { current: u64; divisor: u64; limit: u64; }
    data Envelope { metrics: Metrics; decoy: Metrics; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    requires
        1 <= metrics.divisor
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    requires
        1 <= envelope.metrics.divisor
    crashes Abort
        envelope.metrics.current / envelope.metrics.divisor <= envelope.metrics.limit
    {
        Helper::inspect(envelope.metrics);
    }
"#;

const DISJUNCTIVE_MEMBER_SOURCE: &str = r#"
    data Flag { active: bool; }
    data Pair { left: Flag; right: Flag; decoy: Flag; }
    data Envelope { pair: Pair; spare: Pair; }
    data Helper {}
    machine Helper::inspect(pair: Pair)
    crashes Abort
        pair.left.active || !pair.right.active
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.pair.left.active || !envelope.pair.right.active
    {
        Helper::inspect(envelope.pair);
    }
"#;

const WHOLE_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Counts { current: u64; limit: u64; }
    CountsEquatable: Counts satisfies Equatable;
    data Pair { active: bool; counts: Counts; }
    PairEquatable: Pair satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Pair, right: Pair)
    crashes Abort
        left == right
    {}

    data Root {}
    machine Root::enter(left: Pair, right: Pair)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }
"#;

#[test]
fn direct_boolean_member_crash_route_survives_source_call_codec_and_interpretation() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("direct Boolean member crash route lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 2);
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [root_route] = root.contract.crash_routes.as_slice() else {
        panic!("caller publishes one member-guarded route")
    };
    let [helper_route] = helper.contract.crash_routes.as_slice() else {
        panic!("callee publishes one member-guarded route")
    };
    assert!(matches!(
        root_route.alternatives.as_slice(),
        [CrashRouteGuard::Predicate(_)]
    ));
    assert!(matches!(
        helper_route.alternatives.as_slice(),
        [CrashRouteGuard::Predicate(_)]
    ));
    let call = root.blocks[0]
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::CallUnit {
                crash_continuations,
                ..
            } => Some(crash_continuations),
            _ => None,
        })
        .expect("caller emits the Unit call");
    assert_eq!(call, &root.contract.crash_routes);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs the exact member-root substitution");
    let bytes = encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );

    let packet = TerminalStructuralValue {
        opaque_identity: 7,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    assert_eq!(
        interpret_terminal_artifact_with_effect_handler_measured(
            &bytes,
            &encode_proof_bundle(&lowered.proof_bundle).expect("proof encode"),
            &AdmissionProfile::default(),
            &[],
            &[packet],
            &mut Accept,
        )
        .expect("member contracts do not reinterpret opaque aggregate runtime data")
        .into_value(),
        TerminalExecutionResult::Unit,
    );
}

#[test]
fn verifier_rejects_unknown_direct_boolean_member_identity() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let mut lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("direct Boolean member crash route lowers");
    let wrong_field = StructuralFieldId::new(u64::MAX).expect("nonzero field");
    let wrong_route = |root| {
        vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(root, wrong_field),
            ),
        ))]
    };
    let caller_root = lowered.semantic_module.machines[0].structural_parameters[0].place;
    lowered.semantic_module.machines[0].contract.crash_routes[0].alternatives =
        wrong_route(caller_root);
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut lowered.semantic_module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("root operation is the Unit call")
    };
    crash_continuations[0].alternatives = wrong_route(caller_root);
    let helper_root = lowered.semantic_module.machines[1].structural_parameters[0].place;
    lowered.semantic_module.machines[1].contract.crash_routes[0].alternatives =
        wrong_route(helper_root);

    let result = psi_terminal_verifier::validate_module(&lowered.semantic_module);
    assert!(
        matches!(
            result,
            Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
        ),
        "unexpected verification result: {result:?}"
    );
}

#[test]
fn nested_boolean_member_path_survives_source_call_codec_verification_interpretation_and_fuel() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(NESTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("nested Boolean member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_predicate)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one nested member predicate")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: route_root,
            path,
        },
    ) = root_predicate.proposition()
    else {
        panic!("nested member route retains a structural Boolean path")
    };
    assert_eq!(*route_root, root.structural_parameters[0].place);
    let [
        CanonicalStructuralPathSegment::Field(outer_field),
        CanonicalStructuralPathSegment::Field(leaf_field),
    ] = path.as_slice()
    else {
        panic!("nested member route retains exactly two canonical field IDs")
    };
    let packet = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Packet type");
    let StructuralTypeShape::Record { fields } = &packet.shape else {
        panic!("Packet is a record")
    };
    let state = fields
        .iter()
        .find(|field| field.id == *outer_field)
        .expect("state field");
    assert_eq!(state.identity, "state");
    let StructuralFieldType::Structural(state_type) = state.field_type else {
        panic!("state field is structural")
    };
    let state = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == state_type)
        .expect("AbortState type");
    let StructuralTypeShape::Record { fields } = &state.shape else {
        panic!("AbortState is a record")
    };
    let leaf = fields
        .iter()
        .find(|field| field.id == *leaf_field)
        .expect("should_abort field");
    assert_eq!(leaf.identity, "should_abort");
    assert_eq!(
        leaf.field_type,
        StructuralFieldType::Scalar(psi_core::ScalarType::Boolean)
    );

    let [helper_route] = helper.contract.crash_routes.as_slice() else {
        panic!("callee publishes one nested member route")
    };
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(crash_continuations, &root.contract.crash_routes);
    assert_ne!(
        root.structural_parameters[0].place,
        helper.structural_parameters[0].place
    );
    assert_ne!(root.contract.crash_routes, [helper_route.clone()]);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier traverses and rebases the exact nested Boolean path");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("nested member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 9,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("nested member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 3);
}

#[test]
fn projected_structural_argument_prefix_rebases_member_crash_routes_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(PROJECTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected structural member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let packet_field = fields
        .iter()
        .find(|field| field.identity == "packet")
        .expect("packet field");

    let [CrashRouteGuard::Predicate(root_predicate)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one projected member predicate")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: route_root,
            path: caller_path,
        },
    ) = root_predicate.proposition()
    else {
        panic!("caller route retains its structural field path")
    };
    assert_eq!(*route_root, root.structural_parameters[0].place);
    assert_eq!(
        caller_path.first(),
        Some(&CanonicalStructuralPathSegment::Field(packet_field.id))
    );

    let [CrashRouteGuard::Predicate(helper_predicate)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one member predicate")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            path: helper_path, ..
        },
    ) = helper_predicate.proposition()
    else {
        panic!("callee route retains its parameter-relative field path")
    };
    assert_eq!(&caller_path[1..], helper_path);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [StructuralPathSegment::Field("packet".into())]
    );
    assert_eq!(crash_continuations, &root.contract.crash_routes);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes the argument and callee field paths");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("projected member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 11,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("projected member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 3);
}

#[test]
fn composed_boolean_member_predicate_rebases_every_path_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn paths(
        proposition: &Proposition,
    ) -> (
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
    ) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("composed member predicate is one conjunction")
        };
        let [equality, armed] = conjuncts.as_slice() else {
            panic!("conjunction retains equality then member assertion")
        };
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::BooleanEqual { left, right }) =
            equality
        else {
            panic!("first conjunct retains Boolean equality")
        };
        let ScalarTerm::BooleanField {
            path: left_path, ..
        } = left.as_ref()
        else {
            panic!("equality left operand is a member path")
        };
        let ScalarTerm::BooleanNot { operand } = right.as_ref() else {
            panic!("equality right operand retains negation")
        };
        let ScalarTerm::BooleanField {
            path: right_path, ..
        } = operand.as_ref()
        else {
            panic!("negated operand is a member path")
        };
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::BooleanField {
                path: armed_path, ..
            },
        ) = armed
        else {
            panic!("second conjunct is the armed member assertion")
        };
        (left_path, right_path, armed_path)
    }

    let tokens = Lexer::new(COMPOSED_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("composed Boolean member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one composed member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one composed member route")
    };
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("callee composed route survives the projected call")
    };
    assert_eq!(continuation, root_route);

    let (root_left, root_right, root_armed) = paths(root_route.proposition());
    let (helper_left, helper_right, helper_armed) = paths(helper_route.proposition());
    assert_eq!(&root_left[1..], helper_left);
    assert_eq!(&root_right[1..], helper_right);
    assert_eq!(&root_armed[1..], helper_armed);
    assert_eq!(root_left[0], root_right[0]);
    assert_eq!(root_left[0], root_armed[0]);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently traverses every composed member path");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("composed member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 17,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("composed member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let Proposition::Conjunction(conjuncts) = predicate.proposition() else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::BooleanField { path: armed, .. })) =
        conjuncts.iter().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::BooleanField { .. })
            )
        })
    else {
        unreachable!()
    };
    let armed = armed.clone();
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::BooleanEqual { left, .. })) =
        conjuncts.iter_mut().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::BooleanEqual { .. })
            )
        })
    else {
        unreachable!()
    };
    let ScalarTerm::BooleanField { path, .. } = left.as_mut() else {
        unreachable!()
    };
    *path = armed;
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected composed-member validation result: {invalid_result:?}"
    );
}

#[test]
fn integer_member_comparisons_rebase_and_validate_exact_leaf_types_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn paths(
        proposition: &Proposition,
    ) -> (
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        IntegerType,
    ) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("integer member route is one conjunction")
        };
        let [nonzero, ordered] = conjuncts.as_slice() else {
            panic!("integer route retains ordering then inequality")
        };
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = ordered
        else {
            panic!("first conjunct retains the ordered member comparison")
        };
        let ScalarTerm::IntegerField {
            path: limit_path,
            scalar_type: limit_type,
            ..
        } = left.as_ref()
        else {
            panic!("ordered left operand is the limit member")
        };
        let ScalarTerm::IntegerField {
            path: current_path,
            scalar_type: current_type,
            ..
        } = right.as_ref()
        else {
            panic!("ordered right operand is the current member")
        };
        assert_eq!(limit_type, scalar_type);
        assert_eq!(current_type, scalar_type);

        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::BooleanNot { operand }) =
            nonzero
        else {
            panic!("second conjunct retains integer inequality as negated equality")
        };
        let ScalarTerm::IntegerEqual { left, right, .. } = operand.as_ref() else {
            panic!("inequality retains one integer equality term")
        };
        let ScalarTerm::IntegerField {
            path: nonzero_path, ..
        } = left.as_ref()
        else {
            panic!("inequality left operand is the current member")
        };
        assert!(matches!(right.as_ref(), ScalarTerm::IntegerField { .. }));
        (limit_path, current_path, nonzero_path, *scalar_type)
    }

    let tokens = Lexer::new(INTEGER_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("integer member crash comparisons lower");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one integer member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one integer member route")
    };
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("integer member route survives the projected call")
    };
    assert_eq!(continuation, root_route);

    let (root_limit, root_current, root_nonzero, integer_type) = paths(root_route.proposition());
    let (helper_limit, helper_current, helper_nonzero, helper_type) =
        paths(helper_route.proposition());
    assert_eq!(
        integer_type,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    );
    assert_eq!(integer_type, helper_type);
    assert_eq!(root_limit, helper_limit);
    assert_eq!(root_current, helper_current);
    assert_eq!(root_nonzero, helper_nonzero);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently checks every integer member path and leaf type");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("integer member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 19,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("integer member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut mistyped = lowered.semantic_module.clone();
    let CrashRouteGuard::Predicate(predicate) =
        &mut mistyped.machines[1].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(
        _,
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        },
    )) = conjuncts.iter_mut().find(|conjunct| {
        matches!(
            conjunct,
            Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { .. })
        )
    })
    else {
        unreachable!()
    };
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    *scalar_type = u32_type;
    for operand in [left, right] {
        let ScalarTerm::IntegerField { scalar_type, .. } = operand.as_mut() else {
            unreachable!()
        };
        *scalar_type = u32_type;
    }
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&mistyped);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected integer-member validation result: {invalid_result:?}"
    );
}

#[test]
fn projected_argument_prefix_rebases_every_integer_member_path_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn paths(
        proposition: &Proposition,
    ) -> (
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
    ) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("projected integer member route is one conjunction")
        };
        let [inequality, ordered] = conjuncts.as_slice() else {
            panic!("projected integer route retains both comparisons")
        };
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual { left, right, .. },
        ) = ordered
        else {
            panic!("ordered comparison remains terminal")
        };
        let (
            ScalarTerm::IntegerField {
                path: ordered_left, ..
            },
            ScalarTerm::IntegerField {
                path: ordered_right,
                ..
            },
        ) = (left.as_ref(), right.as_ref())
        else {
            panic!("ordered operands remain integer member paths")
        };
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::BooleanNot { operand }) =
            inequality
        else {
            panic!("inequality remains a negated equality")
        };
        let ScalarTerm::IntegerEqual { left, right, .. } = operand.as_ref() else {
            panic!("inequality retains its integer equality")
        };
        let (
            ScalarTerm::IntegerField {
                path: unequal_left, ..
            },
            ScalarTerm::IntegerField {
                path: unequal_right,
                ..
            },
        ) = (left.as_ref(), right.as_ref())
        else {
            panic!("inequality operands remain integer member paths")
        };
        (ordered_left, ordered_right, unequal_left, unequal_right)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected integer member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let batch = fields
        .iter()
        .find(|field| field.identity == "batch")
        .expect("batch field");
    let StructuralFieldType::Structural(batch_type) = batch.field_type else {
        panic!("batch has a structural type")
    };
    let batch_declaration = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == batch_type)
        .expect("Batch type");
    let StructuralTypeShape::Record {
        fields: batch_fields,
    } = &batch_declaration.shape
    else {
        panic!("Batch is a record")
    };
    let metrics = batch_fields
        .iter()
        .find(|field| field.identity == "metrics")
        .expect("metrics field");
    let shadow = batch_fields
        .iter()
        .find(|field| field.identity == "shadow")
        .expect("shadow field");

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [
            StructuralPathSegment::Field("batch".into()),
            StructuralPathSegment::Field("metrics".into())
        ]
    );
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one integer member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one integer member route")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one guarded crash continuation")
    };
    assert_eq!(continuation, root_route);
    let (root_ordered_left, root_ordered_right, root_unequal_left, root_unequal_right) =
        paths(root_route.proposition());
    let (helper_ordered_left, helper_ordered_right, helper_unequal_left, helper_unequal_right) =
        paths(helper_route.proposition());
    for (caller_path, callee_path) in [
        root_ordered_left,
        root_ordered_right,
        root_unequal_left,
        root_unequal_right,
    ]
    .into_iter()
    .zip([
        helper_ordered_left,
        helper_ordered_right,
        helper_unequal_left,
        helper_unequal_right,
    ]) {
        assert_eq!(
            &caller_path[..2],
            [
                CanonicalStructuralPathSegment::Field(batch.id),
                CanonicalStructuralPathSegment::Field(metrics.id),
            ]
        );
        assert_eq!(&caller_path[2..], callee_path);
    }

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes every projected integer member path");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("projected integer route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 23,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("projected integer contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. })) =
        conjuncts.iter_mut().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { .. })
            )
        })
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField { path, .. } = left.as_mut() else {
        unreachable!()
    };
    path[1] = CanonicalStructuralPathSegment::Field(shadow.id);
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected projected integer validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_addition_rebases_every_operand_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn addition_fields(
        proposition: &Proposition,
    ) -> (&ScalarTerm, &ScalarTerm, &ScalarTerm, IntegerType) {
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = proposition
        else {
            panic!("member arithmetic route retains its ordered comparison")
        };
        let ScalarTerm::ExactIntegerAdd {
            scalar_type: addition_type,
            left: add_left,
            right: add_right,
        } = left.as_ref()
        else {
            panic!("comparison left operand retains exact addition")
        };
        assert_eq!(addition_type, scalar_type);
        (add_left, add_right, right, *scalar_type)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_ARITHMETIC_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected exact member addition lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one arithmetic member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one arithmetic member route")
    };

    let (root_current, root_delta, root_limit, root_type) =
        addition_fields(root_route.proposition());
    let (helper_current, helper_delta, helper_limit, helper_type) =
        addition_fields(helper_route.proposition());
    assert_eq!(
        root_type,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    );
    assert_eq!(root_type, helper_type);
    for term in [root_current, root_delta, root_limit] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller arithmetic operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(*scalar_type, root_type);
    }
    for term in [helper_current, helper_delta, helper_limit] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("callee arithmetic operand is a typed member path")
        };
        assert_eq!(*field_root, helper.structural_parameters[0].place);
        assert_eq!(path.len(), 1);
        assert_eq!(*scalar_type, helper_type);
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the arithmetic member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes every exact-add member operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("arithmetic member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 41,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("member arithmetic remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. }) = &mut proposition
    else {
        unreachable!()
    };
    let ScalarTerm::ExactIntegerAdd { left, right, .. } = left.as_mut() else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: left_path, ..
    } = left.as_mut()
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: right_path, ..
    } = right.as_ref()
    else {
        unreachable!()
    };
    *left_path = right_path.clone();
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected arithmetic-member validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_subtraction_rebases_every_operand_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn subtraction_fields(
        proposition: &Proposition,
    ) -> (&ScalarTerm, &ScalarTerm, &ScalarTerm, IntegerType) {
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = proposition
        else {
            panic!("member subtraction route retains its ordered comparison")
        };
        let ScalarTerm::ExactIntegerSubtract {
            scalar_type: subtraction_type,
            left: minuend,
            right: subtrahend,
        } = right.as_ref()
        else {
            panic!("comparison right operand retains exact subtraction")
        };
        assert_eq!(subtraction_type, scalar_type);
        (left, minuend, subtrahend, *scalar_type)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_SUBTRACTION_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected exact member subtraction lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one subtraction member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one subtraction member route")
    };

    let (root_floor, root_current, root_delta, root_type) =
        subtraction_fields(root_route.proposition());
    let (helper_floor, helper_current, helper_delta, helper_type) =
        subtraction_fields(helper_route.proposition());
    assert_eq!(
        root_type,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    );
    assert_eq!(root_type, helper_type);
    for term in [root_floor, root_current, root_delta] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller subtraction operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(*scalar_type, root_type);
    }
    for term in [helper_floor, helper_current, helper_delta] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("callee subtraction operand is a typed member path")
        };
        assert_eq!(*field_root, helper.structural_parameters[0].place);
        assert_eq!(path.len(), 1);
        assert_eq!(*scalar_type, helper_type);
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the subtraction member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes every exact-subtract member operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("subtraction member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 43,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("member subtraction remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { right, .. }) = &mut proposition
    else {
        unreachable!()
    };
    let ScalarTerm::ExactIntegerSubtract { left, right, .. } = right.as_mut() else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: left_path, ..
    } = left.as_mut()
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: right_path, ..
    } = right.as_ref()
    else {
        unreachable!()
    };
    *left_path = right_path.clone();
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected subtraction-member validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_multiplication_rebases_every_operand_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn multiplication_fields(proposition: &Proposition) -> [&ScalarTerm; 3] {
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = proposition
        else {
            panic!("member multiplication route retains its ordered comparison")
        };
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type: multiplication_type,
            left: multiplicand,
            right: multiplier,
        } = left.as_ref()
        else {
            panic!("comparison left operand retains exact multiplication")
        };
        assert_eq!(multiplication_type, scalar_type);
        [multiplicand, multiplier, right]
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_MULTIPLICATION_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected exact member multiplication lowers");

    let root = &lowered.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one multiplication member route")
    };
    for term in multiplication_fields(root_route.proposition()) {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller multiplication operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(
            *scalar_type,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        );
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the multiplication member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes every exact-multiply member operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("multiplication member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 47,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("member multiplication remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. }) = &mut proposition
    else {
        unreachable!()
    };
    let ScalarTerm::ExactIntegerMultiply { left, right, .. } = left.as_mut() else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: left_path, ..
    } = left.as_mut()
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: right_path, ..
    } = right.as_ref()
    else {
        unreachable!()
    };
    *left_path = right_path.clone();
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected multiplication-member validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_division_and_remainder_rebase_safe_literals_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn arithmetic_fields(proposition: &Proposition) -> [&ScalarTerm; 3] {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("division and remainder route is one conjunction")
        };
        let mut division = None;
        let mut remainder = None;
        let mut parity = None;
        for conjunct in conjuncts {
            let Proposition::Equal(ScalarTerm::Boolean(true), predicate) = conjunct else {
                continue;
            };
            match predicate {
                ScalarTerm::IntegerLessOrEqual { left, right, .. } => {
                    let ScalarTerm::ExactIntegerDivide {
                        left: dividend,
                        right: divisor,
                        ..
                    } = left.as_ref()
                    else {
                        continue;
                    };
                    assert!(matches!(
                        divisor.as_ref(),
                        ScalarTerm::Integer {
                            value: psi_core::IntegerValue::Unsigned(2),
                            ..
                        }
                    ));
                    division = Some(dividend.as_ref());
                    assert!(matches!(right.as_ref(), ScalarTerm::IntegerField { .. }));
                }
                ScalarTerm::IntegerEqual { left, right, .. } => {
                    let ScalarTerm::ExactIntegerRemainder {
                        left: dividend,
                        right: divisor,
                        ..
                    } = left.as_ref()
                    else {
                        continue;
                    };
                    assert!(matches!(
                        divisor.as_ref(),
                        ScalarTerm::Integer {
                            value: psi_core::IntegerValue::Unsigned(2),
                            ..
                        }
                    ));
                    remainder = Some(dividend.as_ref());
                    parity = Some(right.as_ref());
                }
                _ => {}
            }
        }
        [
            division.expect("division member"),
            remainder.expect("remainder member"),
            parity.expect("remainder comparison member"),
        ]
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_DIVISION_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected exact member division and remainder lower");

    let root = &lowered.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one division/remainder member route")
    };
    for term in arithmetic_fields(root_route.proposition()) {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller division operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(
            *scalar_type,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        );
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the division/remainder member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes division/remainder member operands");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("division/remainder member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 53,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("member division remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut unsafe_divisor =
        psi_checked_trees_to_terminal::lower_machine(&checked, "Helper::inspect")
            .expect("standalone helper division lowers")
            .semantic_module;
    let CrashRouteGuard::Predicate(predicate) =
        &mut unsafe_divisor.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(ScalarTerm::ExactIntegerDivide {
        scalar_type, right, ..
    }) = conjuncts.iter_mut().find_map(|conjunct| {
        let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. }) = conjunct else {
            return None;
        };
        Some(left.as_mut())
    })
    else {
        unreachable!()
    };
    *right =
        Box::new(ScalarTerm::integer(*scalar_type, psi_core::IntegerValue::Unsigned(0)).unwrap());
    *predicate = CrashPredicateTerm::new(proposition);
    let unsafe_result = psi_terminal_verifier::validate_module(&unsafe_divisor);
    assert!(
        matches!(
            unsafe_result,
            Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
        ),
        "unexpected unsafe-divisor validation result: {unsafe_result:?}"
    );

    let tokens = Lexer::new(RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE)
        .tokenize()
        .expect("runtime-divisor tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("runtime-divisor parse");
    let resolved = lower_syntax_trees(&syntax).expect("runtime-divisor resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("runtime-divisor type");
    let checked = lower_typed_trees(typed).expect("runtime-divisor check");
    let runtime_divisor = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("a positive runtime-divisor requirement is explicit terminal safety evidence");
    let runtime_machine = &runtime_divisor.semantic_module.machines[0];
    assert_eq!(runtime_machine.contract.requires.len(), 1);
    assert!(matches!(
        &runtime_machine.contract.requires[0],
        Proposition::LessOrEqual(
            ScalarTerm::Integer {
                value: psi_core::IntegerValue::Unsigned(1),
                ..
            },
            ScalarTerm::IntegerField { .. }
        )
    ));
    psi_terminal_verifier::verify_module(
        &runtime_divisor.semantic_module,
        &runtime_divisor.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently accepts the runtime-divisor requirement");
    let encoded = encode_module(&runtime_divisor.semantic_module)
        .expect("runtime-divisor semantic module encodes");
    assert_eq!(
        decode_module(&encoded),
        Ok(runtime_divisor.semantic_module.clone()),
        "the exact runtime safety requirement survives canonical encoding"
    );

    let mut missing_requirement = runtime_divisor.semantic_module.clone();
    missing_requirement.machines[0].contract.requires.clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module(&missing_requirement),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
    ));

    let mut redirected_requirement = runtime_divisor.semantic_module.clone();
    let StructuralTypeShape::Record { fields } = &redirected_requirement.structural_types[0].shape
    else {
        unreachable!()
    };
    let limit = fields[2].id;
    let Proposition::LessOrEqual(_, ScalarTerm::IntegerField { path, .. }) =
        &mut redirected_requirement.machines[0].contract.requires[0]
    else {
        unreachable!()
    };
    *path = vec![CanonicalStructuralPathSegment::Field(limit)];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected_requirement),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
    ));

    let tokens = Lexer::new(UNPROVEN_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE)
        .tokenize()
        .expect("unproven-runtime-divisor tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("unproven-runtime-divisor parse");
    let resolved = lower_syntax_trees(&syntax).expect("unproven-runtime-divisor resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("unproven-runtime-divisor type");
    let checked = lower_typed_trees(typed).expect("unproven-runtime-divisor check");
    assert!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter").is_err(),
        "a runtime member divisor remains fenced without explicit terminal safety evidence"
    );
}

#[test]
fn bitwise_member_terms_rebase_across_projected_calls_and_codecs() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn inspect_term<'a>(
        term: &'a ScalarTerm,
        bitwise_counts: &mut [usize; 4],
        paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
    ) {
        match term {
            ScalarTerm::IntegerField { path, .. } => paths.push(path),
            ScalarTerm::BooleanNot { operand } => inspect_term(operand, bitwise_counts, paths),
            ScalarTerm::IntegerBitwiseNot { operand, .. } => {
                bitwise_counts[3] += 1;
                inspect_term(operand, bitwise_counts, paths);
            }
            ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
                match term {
                    ScalarTerm::IntegerBitwiseAnd { .. } => bitwise_counts[0] += 1,
                    ScalarTerm::IntegerBitwiseOr { .. } => bitwise_counts[1] += 1,
                    ScalarTerm::IntegerBitwiseXor { .. } => bitwise_counts[2] += 1,
                    _ => unreachable!(),
                }
                inspect_term(left, bitwise_counts, paths);
                inspect_term(right, bitwise_counts, paths);
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. } => {
                inspect_term(left, bitwise_counts, paths);
                inspect_term(right, bitwise_counts, paths);
            }
            _ => {}
        }
    }

    fn inspect_proposition(
        proposition: &Proposition,
    ) -> ([usize; 4], Vec<&[CanonicalStructuralPathSegment]>) {
        fn inspect<'a>(
            proposition: &'a Proposition,
            bitwise_counts: &mut [usize; 4],
            paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
        ) {
            match proposition {
                Proposition::Equal(left, right)
                | Proposition::LessThan(left, right)
                | Proposition::LessOrEqual(left, right) => {
                    inspect_term(left, bitwise_counts, paths);
                    inspect_term(right, bitwise_counts, paths);
                }
                Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                    for proposition in propositions {
                        inspect(proposition, bitwise_counts, paths);
                    }
                }
                _ => {}
            }
        }

        let mut bitwise_counts = [0; 4];
        let mut paths = Vec::new();
        inspect(proposition, &mut bitwise_counts, &mut paths);
        (bitwise_counts, paths)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_BITWISE_SOURCE)
        .tokenize()
        .expect("bitwise tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("bitwise parse");
    let resolved = lower_syntax_trees(&syntax).expect("bitwise resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("bitwise type");
    let checked = lower_typed_trees(typed).expect("bitwise check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected bitwise member predicates lower");
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one bitwise route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one bitwise route")
    };
    let (root_counts, root_paths) = inspect_proposition(root_route.proposition());
    let (helper_counts, helper_paths) = inspect_proposition(helper_route.proposition());
    assert_eq!(root_counts, [1, 1, 1, 1]);
    assert_eq!(helper_counts, root_counts);
    assert_eq!(root_paths.len(), helper_paths.len());

    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let bits = fields
        .iter()
        .find(|field| field.identity == "bits")
        .expect("bits field");
    assert!(
        root_paths
            .iter()
            .all(|path| { path.first() == Some(&CanonicalStructuralPathSegment::Field(bits.id)) })
    );
    assert!(helper_paths.iter().all(|path| path.len() == 1));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected Unit call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "bits"
    ));
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one bitwise crash continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently rebases every nested bitwise member");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("bitwise member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("bitwise fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("bitwise semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("bitwise proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 59,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("bitwise crash predicates remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn total_policy_arithmetic_rebases_across_projected_calls_and_codecs() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn inspect_policy_terms(
        proposition: &Proposition,
    ) -> ([usize; 6], Vec<&[CanonicalStructuralPathSegment]>) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("policy arithmetic route is one conjunction")
        };
        let mut counts = [0; 6];
        let mut paths = Vec::new();
        for conjunct in conjuncts {
            let Proposition::Equal(
                ScalarTerm::Boolean(true),
                ScalarTerm::IntegerEqual { left, right, .. },
            ) = conjunct
            else {
                panic!("each policy arithmetic clause remains an integer equality")
            };
            let (index, operation_left, operation_right) = match left.as_ref() {
                ScalarTerm::WrappingIntegerAdd { left, right, .. } => (0, left, right),
                ScalarTerm::WrappingIntegerSubtract { left, right, .. } => (1, left, right),
                ScalarTerm::WrappingIntegerMultiply { left, right, .. } => (2, left, right),
                ScalarTerm::SaturatingIntegerAdd { left, right, .. } => (3, left, right),
                ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => (4, left, right),
                ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => (5, left, right),
                _ => panic!("unexpected policy arithmetic term"),
            };
            counts[index] += 1;
            for term in [
                operation_left.as_ref(),
                operation_right.as_ref(),
                right.as_ref(),
            ] {
                let ScalarTerm::IntegerField { path, .. } = term else {
                    panic!("policy arithmetic operand remains a typed member path")
                };
                paths.push(path.as_slice());
            }
        }
        (counts, paths)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_POLICY_ARITHMETIC_SOURCE)
        .tokenize()
        .expect("policy arithmetic tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("policy arithmetic parse");
    let resolved = lower_syntax_trees(&syntax).expect("policy arithmetic resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("policy arithmetic type");
    let checked = lower_typed_trees(typed).expect("policy arithmetic check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected wrapping and saturating member arithmetic lowers");
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one policy arithmetic route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one policy arithmetic route")
    };
    let (root_counts, root_paths) = inspect_policy_terms(root_route.proposition());
    let (helper_counts, helper_paths) = inspect_policy_terms(helper_route.proposition());
    assert_eq!(root_counts, [1; 6]);
    assert_eq!(helper_counts, root_counts);
    assert_eq!(root_paths.len(), 18);
    assert_eq!(helper_paths.len(), root_paths.len());

    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let values = fields
        .iter()
        .find(|field| field.identity == "values")
        .expect("values field");
    assert!(
        root_paths.iter().all(|path| {
            path.first() == Some(&CanonicalStructuralPathSegment::Field(values.id))
        })
    );
    assert!(helper_paths.iter().all(|path| path.len() == 1));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected Unit call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "values"
    ));
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one policy arithmetic continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently rebases every policy arithmetic operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("policy arithmetic route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("policy arithmetic fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("policy semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("policy proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 67,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("policy arithmetic predicates remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn signed_runtime_member_divisor_requires_an_overflow_safe_bound() {
    let tokens = Lexer::new(NEGATIVE_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE)
        .tokenize()
        .expect("negative-runtime-divisor tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("negative-runtime-divisor parse");
    let resolved = lower_syntax_trees(&syntax).expect("negative-runtime-divisor resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("negative-runtime-divisor type");
    let checked = lower_typed_trees(typed).expect("negative-runtime-divisor check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("a divisor bounded at or below negative two is total for every dividend");
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently accepts the negative runtime-divisor bound");

    let mut overflow_permitting = lowered.semantic_module.clone();
    let Proposition::LessOrEqual(_, ScalarTerm::Integer { value, .. }) =
        &mut overflow_permitting.machines[0].contract.requires[0]
    else {
        unreachable!()
    };
    *value = psi_core::IntegerValue::Signed(-1);
    assert!(matches!(
        psi_terminal_verifier::validate_module(&overflow_permitting),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
    ));
}

#[test]
fn runtime_divisor_call_requirements_rebase_and_verify_exact_obligations() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(RUNTIME_DIVISOR_CALL_SOURCE)
        .tokenize()
        .expect("runtime-divisor-call tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("runtime-divisor-call parse");
    let resolved = lower_syntax_trees(&syntax).expect("runtime-divisor-call resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("runtime-divisor-call type");
    let checked = lower_typed_trees(typed).expect("runtime-divisor-call check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("a whole-root Unit call carries its exact runtime-divisor requirement");
    let root = &lowered.semantic_module.machines[0];
    let OperationKind::CallUnit {
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("root emits one structural Unit call")
    };
    let [obligation] = requirement_obligations.as_slice() else {
        panic!("the call owns one exact requirement obligation")
    };
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    assert_eq!(lowered.proof_bundle.evidence[0].obligation, *obligation);
    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently rebases and proves the call requirement");
    assert_eq!(verified.accepted_facts().len(), 1);

    let semantics = encode_module(&lowered.semantic_module).expect("call semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("call proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 61,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("verified runtime-divisor call executes as erased proof metadata");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);

    let mut missing = lowered.proof_bundle.clone();
    missing.evidence.clear();
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(id)) if id == *obligation
    ));

    let mut wrong_assumption = lowered.proof_bundle.clone();
    let EvidenceRoute::CertificateDerived(certificate) = &mut wrong_assumption.evidence[0].route
    else {
        unreachable!()
    };
    certificate.proof.rule = ProofRule::Assumption { index: 1 };
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &wrong_assumption,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { .. })
    ));
}

#[test]
fn projected_runtime_divisor_call_rebases_requirement_through_canonical_prefix() {
    let tokens = Lexer::new(PROJECTED_RUNTIME_DIVISOR_CALL_SOURCE)
        .tokenize()
        .expect("projected-runtime-divisor-call tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("projected-runtime-divisor-call parse");
    let resolved = lower_syntax_trees(&syntax).expect("projected-runtime-divisor-call resolve");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("projected-runtime-divisor-call type");
    let checked = lower_typed_trees(typed).expect("projected-runtime-divisor-call check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("a projected Unit call rebases its runtime-divisor requirement");
    let root = &lowered.semantic_module.machines[0];
    let OperationKind::CallUnit {
        structural_arguments,
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("root emits one projected structural Unit call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "metrics"
    ));
    assert_eq!(requirement_obligations.len(), 1);
    let reconstructed =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("the verifier reconstructs the projected call obligation");
    assert_eq!(reconstructed.len(), 1);
    assert_eq!(
        reconstructed[0].obligation.proposition, root.contract.requires[0],
        "the canonical argument prefix rebases the callee premise to the caller path"
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the projected call proof cites the exact rebased caller assumption");

    let semantics = encode_module(&lowered.semantic_module).expect("projected call encodes");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("projected proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );

    let mut wrong_prefix = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut wrong_prefix.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("decoy".to_owned())];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&wrong_prefix),
        Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn proposition_disjunction_rebases_and_verifies_each_member_path_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn field_paths(proposition: &Proposition) -> Vec<&[CanonicalStructuralPathSegment]> {
        fn collect_term<'a>(
            term: &'a ScalarTerm,
            paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
        ) {
            match term {
                ScalarTerm::BooleanField { path, .. } => paths.push(path),
                ScalarTerm::BooleanNot { operand } => collect_term(operand, paths),
                _ => {}
            }
        }
        fn collect<'a>(
            proposition: &'a Proposition,
            paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
        ) {
            match proposition {
                Proposition::Equal(left, right) => {
                    collect_term(left, paths);
                    collect_term(right, paths);
                }
                Proposition::Disjunction(disjuncts) => {
                    for disjunct in disjuncts {
                        collect(disjunct, paths);
                    }
                }
                _ => {}
            }
        }
        let mut paths = Vec::new();
        collect(proposition, &mut paths);
        paths
    }

    let tokens = Lexer::new(DISJUNCTIVE_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("disjunctive projected member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let pair = fields
        .iter()
        .find(|field| field.identity == "pair")
        .expect("pair field");
    let StructuralFieldType::Structural(pair_type) = pair.field_type else {
        panic!("pair has a structural type")
    };
    let pair_declaration = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == pair_type)
        .expect("Pair type");
    let StructuralTypeShape::Record {
        fields: pair_fields,
    } = &pair_declaration.shape
    else {
        panic!("Pair is a record")
    };
    let decoy = pair_fields
        .iter()
        .find(|field| field.identity == "decoy")
        .expect("decoy field");

    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one disjunctive route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one disjunctive route")
    };
    let Proposition::Disjunction(root_disjuncts) = root_route.proposition() else {
        panic!("caller retains terminal proposition disjunction")
    };
    assert_eq!(root_disjuncts.len(), 2);
    let Proposition::Disjunction(helper_disjuncts) = helper_route.proposition() else {
        panic!("callee retains terminal proposition disjunction")
    };
    assert_eq!(helper_disjuncts.len(), 2);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [StructuralPathSegment::Field("pair".into())]
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one disjunctive continuation")
    };
    assert_eq!(continuation, root_route);
    let root_paths = field_paths(root_route.proposition());
    let helper_paths = field_paths(helper_route.proposition());
    assert_eq!(root_paths.len(), 2);
    assert_eq!(helper_paths.len(), 2);
    for root_path in root_paths {
        assert_eq!(
            root_path.first(),
            Some(&CanonicalStructuralPathSegment::Field(pair.id))
        );
        assert!(helper_paths.contains(&&root_path[1..]));
    }

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently reconstructs the disjunctive continuation");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("disjunctive route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 29,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("disjunctive member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Disjunction(disjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::BooleanField { path, .. })) =
        disjuncts.iter_mut().find(|disjunct| {
            matches!(
                disjunct,
                Proposition::Equal(_, ScalarTerm::BooleanField { .. })
            )
        })
    else {
        unreachable!()
    };
    path[1] = CanonicalStructuralPathSegment::Field(decoy.id);
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected disjunctive validation result: {invalid_result:?}"
    );
}

#[test]
fn whole_aggregate_equality_expands_and_reconstructs_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn field_roots(proposition: &Proposition) -> Vec<(psi_core::PlaceId, psi_core::PlaceId)> {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("aggregate equality is one flat conjunction")
        };
        let mut roots = Vec::new();
        for conjunct in conjuncts {
            let Proposition::Equal(ScalarTerm::Boolean(true), term) = conjunct else {
                panic!("aggregate field compare is asserted true")
            };
            match term {
                ScalarTerm::BooleanEqual { left, right } => {
                    let (
                        ScalarTerm::BooleanField {
                            root: left_root, ..
                        },
                        ScalarTerm::BooleanField {
                            root: right_root, ..
                        },
                    ) = (left.as_ref(), right.as_ref())
                    else {
                        panic!("Boolean aggregate fields retain paths")
                    };
                    roots.push((*left_root, *right_root));
                }
                ScalarTerm::IntegerEqual { left, right, .. } => {
                    let (
                        ScalarTerm::IntegerField {
                            root: left_root, ..
                        },
                        ScalarTerm::IntegerField {
                            root: right_root, ..
                        },
                    ) = (left.as_ref(), right.as_ref())
                    else {
                        panic!("integer aggregate fields retain paths")
                    };
                    roots.push((*left_root, *right_root));
                }
                _ => panic!("aggregate equality uses only member equality terms"),
            }
        }
        roots
    }

    let tokens = Lexer::new(WHOLE_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("whole aggregate equality lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one aggregate equality route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one aggregate equality route")
    };
    let root_field_roots = field_roots(root_route.proposition());
    let helper_field_roots = field_roots(helper_route.proposition());
    assert_eq!(root_field_roots.len(), 3);
    assert!(root_field_roots.iter().all(|roots| {
        *roots
            == (
                root.structural_parameters[0].place,
                root.structural_parameters[1].place,
            )
    }));
    assert_eq!(helper_field_roots.len(), 3);
    assert!(helper_field_roots.iter().all(|roots| {
        *roots
            == (
                helper.structural_parameters[0].place,
                helper.structural_parameters[1].place,
            )
    }));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 2);
    assert!(
        structural_arguments
            .iter()
            .all(|argument| argument.path.is_empty())
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains aggregate equality continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes both aggregate roots");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("aggregate equality route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let arguments = root
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 31 + u64::try_from(index).unwrap(),
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
        &mut Accept,
    )
    .expect("aggregate equality remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::IntegerEqual { right, .. })) =
        conjuncts.iter_mut().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::IntegerEqual { .. })
            )
        })
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        root: right_root, ..
    } = right.as_mut()
    else {
        unreachable!()
    };
    *right_root = root.structural_parameters[0].place;
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected aggregate equality validation result: {invalid_result:?}"
    );
}

#[test]
fn fixed_index_argument_prefix_is_canonical_and_rebases_member_crash_routes_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(FIXED_INDEX_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("fixed-index structural member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [StructuralPathSegment::FixedIndex(0)]
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("callee member route survives the fixed-index call")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: continuation_root,
            path: continuation_path,
        },
    ) = continuation.proposition()
    else {
        panic!("continuation is a canonical structural Boolean path")
    };
    assert_eq!(*continuation_root, root.structural_parameters[0].place);
    let [
        CanonicalStructuralPathSegment::FixedIndex(0),
        CanonicalStructuralPathSegment::Field(leaf),
    ] = continuation_path.as_slice()
    else {
        panic!("fixed index precedes the callee-relative Boolean field")
    };
    let [CrashRouteGuard::Predicate(helper_predicate)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one Boolean member route")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            path: helper_path, ..
        },
    ) = helper_predicate.proposition()
    else {
        panic!("callee route retains its member")
    };
    assert_eq!(helper_path, &[CanonicalStructuralPathSegment::Field(*leaf)]);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes the fixed index and callee member");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("fixed-index member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 4);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 13,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("fixed-index member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut out_of_bounds = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut out_of_bounds.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let Proposition::Equal(_, ScalarTerm::BooleanField { root, path }) = predicate.proposition()
    else {
        unreachable!()
    };
    let root = *root;
    let mut path = path.clone();
    path[0] = CanonicalStructuralPathSegment::FixedIndex(1);
    *predicate = CrashPredicateTerm::new(Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field_path(root, path),
    ));
    let invalid_result = psi_terminal_verifier::validate_module(&out_of_bounds);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected fixed-index validation result: {invalid_result:?}"
    );
}

#[test]
fn verifier_rejects_empty_truncated_and_mistyped_boolean_field_paths() {
    let tokens = Lexer::new(NESTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("nested Boolean member crash route lowers");
    let CrashRouteGuard::Predicate(predicate) =
        &lowered.semantic_module.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        panic!("nested route is a predicate")
    };
    let Proposition::Equal(_, ScalarTerm::BooleanField { path, .. }) = predicate.proposition()
    else {
        panic!("nested route is a Boolean field path")
    };
    let [outer, leaf] = path.as_slice() else {
        panic!("nested route has two fields")
    };

    for invalid_path in [
        Vec::new(),
        vec![*outer],
        vec![*leaf, *outer],
        vec![*outer, *leaf, *leaf],
    ] {
        let mut malformed = lowered.semantic_module.clone();
        let replace_path = |predicate: &mut CrashPredicateTerm| {
            let Proposition::Equal(_, ScalarTerm::BooleanField { root, .. }) =
                predicate.proposition()
            else {
                panic!("member route remains a Boolean field predicate")
            };
            let root = *root;
            *predicate = CrashPredicateTerm::new(Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field_path(root, invalid_path.clone()),
            ));
        };
        for machine in &mut malformed.machines {
            for route in &mut machine.contract.crash_routes {
                for alternative in &mut route.alternatives {
                    let CrashRouteGuard::Predicate(predicate) = alternative else {
                        continue;
                    };
                    replace_path(predicate);
                }
            }
            for operation in &mut machine.blocks[0].operations {
                let OperationKind::CallUnit {
                    crash_continuations,
                    ..
                } = &mut operation.kind
                else {
                    continue;
                };
                for route in crash_continuations {
                    for alternative in &mut route.alternatives {
                        let CrashRouteGuard::Predicate(predicate) = alternative else {
                            continue;
                        };
                        replace_path(predicate);
                    }
                }
            }
        }
        let result = psi_terminal_verifier::validate_module(&malformed);
        assert!(
            matches!(
                result,
                Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
            ),
            "unexpected validation result: {result:?}"
        );
    }
}
