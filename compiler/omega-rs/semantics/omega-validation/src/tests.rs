use super::{validate_behavior_plan, validate_program};
use omega_source_files_to_tokens::Lexer;
use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use omega_tokens_to_syntax_trees::parse_syntax_trees;

fn typed_program_from_source(source: &str) -> omega_typed_trees::TypedTrees {
    let source = format!("data Main {{}} machine Main::run(&mut self) {{}} {source}");
    let tokens = Lexer::new(&source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("resolve should succeed");
    lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed")
}

#[test]
fn declared_lifetime_parameters_survive_lowering_and_validate() {
    let typed = typed_program_from_source(
        r#"
        data View<'buf> {
            body: &'buf i32;
        }

        machine borrow<'call>(value: &'call i32) -> &'call i32 {
            value
        }
        "#,
    );

    let view = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "View")
        .expect("View");
    assert_eq!(view.lifetime_parameters.len(), 1);
    assert_eq!(view.lifetime_parameters[0].as_str(), "buf");

    let borrow = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "borrow")
        .expect("borrow");
    assert_eq!(borrow.lifetime_parameters.len(), 1);
    assert_eq!(borrow.lifetime_parameters[0].as_str(), "call");
    validate_program(&typed).expect("declared lifetime tags should validate");
}

#[test]
fn explicit_lifetime_arguments_survive_typed_lowering_and_validate() {
    let typed = typed_program_from_source(
        r#"
        data View<'buf, T> {
            body: &'buf T;
        }

        machine consume<'call>(value: View<'call, i32>) {}
        "#,
    );

    let consume = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("consume");
    let entry = &typed.machine_states(consume)[0];
    let parameter = &typed.state_parameters(entry)[0];
    let omega_typed_trees::types::TypeReferenceNode::Generic {
        base_name,
        lifetime_arguments,
        arguments,
        ..
    } = typed
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        panic!("data should retain an erased lifetime application");
    };
    assert_eq!(base_name.as_str(), "View");
    assert_eq!(lifetime_arguments[0].as_str(), "call");
    assert_eq!(
        typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .len(),
        1
    );
    validate_program(&typed).expect("declared lifetime application should validate");
}

#[test]
fn undeclared_and_wrong_arity_lifetime_arguments_reject() {
    let typed = typed_program_from_source(
        r#"
        data Pair<'left, 'right> {
            left: &'left i32;
            right: &'right i32;
        }

        machine bad<'call>(value: Pair<'ghost>) {}
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("invalid lifetime application must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses undeclared lifetime argument `'ghost'")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expected 2 lifetime arguments but got 1")
    }));
}

#[test]
fn undeclared_lifetime_tag_rejects() {
    let typed = typed_program_from_source(
        r#"
        data Bad {
            body: &'ghost i32;
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("undeclared lifetime tag must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses undeclared lifetime `'ghost'")
    }));
}

#[test]
fn carry_policy_survives_lowering_and_derives_through_transparent_data() {
    let typed = typed_program_from_source(
        r#"
        data Inner { value: i32; }
        data Outer [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { inner: Inner; }
        "#,
    );
    let outer = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("Outer");
    assert_eq!(
        outer.properties.carry,
        Some(omega_core::semantics::CarryPolicy::PERMISSIVE)
    );
    validate_program(&typed).expect("transparent scalar aggregate should derive permissive carry");
}

#[test]
fn authored_carry_floor_rejects_a_stricter_stored_borrow() {
    let typed = typed_program_from_source(
        r#"
        data Bad [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { borrowed: &i32; }
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("borrow must fail permissive carry floor");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("declares carry policy")
            && diagnostic.message.contains("field `borrowed`")
    }));
}

#[test]
fn carry_bound_rejects_a_stricter_generic_argument() {
    let typed = typed_program_from_source(
        r#"
        data Envelope<T [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )]> { value: T; }
        data Borrowed { value: &i32; }
        data UsesEnvelope { value: Envelope<Borrowed>; }
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("borrow type must not satisfy permissive carry bound");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("type parameter `T") && diagnostic.message.contains("carry(")
    }));
}

#[test]
fn concrete_generic_carry_derives_from_substituted_arguments() {
    let typed = typed_program_from_source(
        r#"
        data Envelope<T> { value: T; }
        data Nested<T> { value: Envelope<T>; }
        data Concrete [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { value: Nested<i32>; }
        "#,
    );
    validate_program(&typed)
        .expect("a concrete transparent instantiation should derive from its argument");
}

#[test]
fn concrete_generic_carry_keeps_restrictive_substituted_arguments() {
    let typed = typed_program_from_source(
        r#"
        data Envelope<T> { value: T; }
        data Borrowed { value: &i32; }
        data Bad [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { value: Envelope<Borrowed>; }
        "#,
    );
    let diagnostics = validate_program(&typed)
        .expect_err("a concrete borrowed argument must remain carry-restrictive");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("data `Bad` declares carry policy")
            && diagnostic.message.contains("field `value`")
    }));
}

#[test]
fn quotient_admits_proven_equivalence_and_carrier_congruence() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {
            case Zero;
            case Next(prev: Carrier);
        }

        machine equivalent(a: Carrier, b: Carrier) -> bool {
            transition { _ -> (true) }
        }

        machine equivalent_reflexive(a: Carrier)
        ensures equivalent(a, a)
        {
        }

        machine equivalent_symmetric(a: Carrier, b: Carrier)
        requires equivalent(a, b)
        ensures equivalent(b, a)
        {
        }

        machine equivalent_transitive(a: Carrier, b: Carrier, c: Carrier)
        requires
            equivalent(a, b)
            equivalent(b, c)
        ensures equivalent(a, c)
        {
        }

        data Quotient = Carrier % equivalent;

        machine quotient_congruence(a: Carrier, b: Carrier)
        requires equivalent(a, b)
        ensures (a as Quotient) == (b as Quotient)
        {
        }
        "#,
    );
    validate_program(&typed).expect("proven equivalence should admit its quotient");
}

#[test]
fn quotient_rejects_missing_equivalence_law() {
    let typed = typed_program_from_source(
        r#"
        data Carrier { case Zero; case Next(prev: Carrier); }
        machine equivalent(a: Carrier, b: Carrier) -> bool {
            transition { _ -> (true) }
        }
        machine equivalent_reflexive(a: Carrier)
        ensures equivalent(a, a) { }
        machine equivalent_transitive(a: Carrier, b: Carrier, c: Carrier)
        requires equivalent(a, b), equivalent(b, c)
        ensures equivalent(a, c) { }
        data Quotient = Carrier % equivalent;
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("symmetry evidence is mandatory");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("missing a structurally matching symmetry proof machine")
    }));
}

#[test]
fn quotient_construction_is_carrier_only() {
    let typed = typed_program_from_source(
        r#"
        data Carrier { case Zero; case Next(prev: Carrier); }
        machine equivalent(a: Carrier, b: Carrier) -> bool {
            transition { _ -> (true) }
        }
        machine equivalent_reflexive(a: Carrier)
        ensures equivalent(a, a) { }
        machine equivalent_symmetric(a: Carrier, b: Carrier)
        requires equivalent(a, b);
        ensures equivalent(b, a) { }
        machine equivalent_transitive(a: Carrier, b: Carrier, c: Carrier)
        requires equivalent(a, b), equivalent(b, c)
        ensures equivalent(a, c) { }
        data Quotient = Carrier % equivalent;
        machine bad() -> Quotient {
            transition { _ -> (42 as Quotient) }
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("non-carrier mint must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("quotient construction is carrier-only")
    }));
}

#[test]
fn quotient_equality_requires_the_declared_relation_fact() {
    let typed = typed_program_from_source(
        r#"
        data Carrier { case Zero; case Next(prev: Carrier); }
        machine equivalent(a: Carrier, b: Carrier) -> bool {
            transition { _ -> (true) }
        }
        machine equivalent_reflexive(a: Carrier)
        ensures equivalent(a, a) { }
        machine equivalent_symmetric(a: Carrier, b: Carrier)
        requires equivalent(a, b);
        ensures equivalent(b, a) { }
        machine equivalent_transitive(a: Carrier, b: Carrier, c: Carrier)
        requires equivalent(a, b), equivalent(b, c)
        ensures equivalent(a, c) { }
        data Quotient = Carrier % equivalent;
        machine bad_congruence(a: Carrier, b: Carrier)
        ensures (a as Quotient) == (b as Quotient)
        {
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("unrelated representatives must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove quotient equality")
            && diagnostic
                .message
                .contains("equivalent(left_carrier, right_carrier)")
    }));
}

#[test]
fn static_machine_argument_refines_authored_generic_contract() {
    let typed = typed_program_from_source(
        r#"
        data Card {}
        machine Card::power(value: &Card) {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {}

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
        "#,
    );
    validate_program(&typed).expect("matching static machine contract should validate");
}

#[test]
fn static_machine_argument_need_not_republish_a_tautological_postcondition() {
    let typed = typed_program_from_source(
        r#"
        data Token {}

        machine selected(value: &Token) {}

        machine invoke<machine F>(value: &Token)
        where machine F(value: &Token) ensures true;
        {}

        machine caller(value: &Token) {
            invoke<selected>(value);
        }
        "#,
    );

    super::validate_static_machine_selections(&typed)
        .expect("`ensures true` is the postcondition identity");
}

#[test]
fn static_machine_argument_requires_a_published_termination_guarantee() {
    let typed = typed_program_from_source(
        r#"
        machine selected(value: u64) -> u64
        terminates by value;
        {
            transition value > 0 {
                true -> selected(value - 1)
                false -> 0
            }
        }

        machine invoke<machine F>(value: u64) -> u64
        where machine F(value: u64) -> u64 terminates;
        {
            F(value)
        }

        machine caller(value: u64) -> u64 {
            invoke<selected>(value)
        }
        "#,
    );

    let diagnostics = super::validate_static_machine_selections(&typed)
        .expect_err("a private ranking witness must not publish a termination promise");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("the requirement guarantees termination")
    }));
}

#[test]
fn static_machine_argument_accepts_a_published_termination_guarantee() {
    let typed = typed_program_from_source(
        r#"
        machine selected(value: u64) -> u64
        terminates;
        terminates by value;
        {
            transition value > 0 {
                true -> selected(value - 1)
                false -> 0
            }
        }

        machine invoke<machine F>(value: u64) -> u64
        where machine F(value: u64) -> u64 terminates;
        {
            F(value)
        }

        machine caller(value: u64) -> u64 {
            invoke<selected>(value)
        }
        "#,
    );

    super::validate_static_machine_selections(&typed)
        .expect("an explicit published termination guarantee should satisfy the slot");
}

#[test]
fn static_machine_argument_rejects_inferred_suspension_above_slot_ceiling() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        boundary machine suspend_source(value: &Token) suspends;

        machine work(value: &Token) {
            suspend_source(value);
        }

        machine invoke<machine F>(value: &Token)
        where machine F(value: &Token)
        {}

        machine caller(value: &Token) {
            invoke<work>(value);
        }
        "#,
    );

    let operations = omega_effects::infer_operational_may(&typed);
    let worker = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "work")
        .expect("worker machine");
    let summary = operations
        .machines()
        .iter()
        .find(|summary| summary.symbol == worker.symbol)
        .expect("worker operations");
    assert!(summary.transitive_may_suspend, "{summary:#?}");

    let diagnostics = super::validate_static_machine_selections(&typed)
        .expect_err("an inferred suspending provider must not widen a negative slot ceiling");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("may suspend")
            && diagnostic.message.contains("requirement omits `suspends;`")
    }));
}

#[test]
fn static_machine_argument_rejects_blocking_independently_from_suspension() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        boundary machine blocking_source(value: &Token) blocks;

        machine work(value: &Token) {
            blocking_source(value);
        }

        machine invoke<machine F>(value: &Token)
        where machine F(value: &Token) suspends;
        {}

        machine caller(value: &Token) {
            invoke<work>(value);
        }
        "#,
    );

    let operations = omega_effects::infer_operational_may(&typed);
    let worker = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "work")
        .expect("worker machine");
    let summary = operations
        .machines()
        .iter()
        .find(|summary| summary.symbol == worker.symbol)
        .expect("worker operations");
    assert!(summary.transitive_may_block, "{summary:#?}");

    let diagnostics = super::validate_static_machine_selections(&typed)
        .expect_err("allowing suspension must not silently allow blocking");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("may block")
            && diagnostic.message.contains("requirement omits `blocks;`")
    }));
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("may suspend")
            && diagnostic.message.contains("requirement omits `suspends;`")
    }));
}

#[test]
fn static_machine_argument_admits_provider_within_both_operational_ceilings() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        boundary machine waiting_source(value: &Token) suspends; blocks;

        machine work(value: &Token) {
            waiting_source(value);
        }

        machine invoke<machine F>(value: &Token)
        where machine F(value: &Token) suspends; blocks;
        {}

        machine caller(value: &Token) {
            invoke<work>(value);
        }
        "#,
    );

    super::validate_static_machine_selections(&typed)
        .expect("a provider within both pinned axes should validate");
}

#[test]
fn static_machine_argument_rejects_symbol_resolved_service_widening() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Readable {
            machine read();
        }

        boundary trait Queryable {
            machine query();
        }

        machine work(service: &mut Queryable) {
            service.query();
        }

        machine invoke<machine F>(service: &mut Queryable)
        where machine F(service: &mut Queryable) effects Readable;
        {}

        machine caller(service: &mut Queryable) {
            invoke<work>(service);
        }
        "#,
    );

    let diagnostics = super::validate_static_machine_selections(&typed)
        .expect_err("a provider must not widen a symbol-resolved service ceiling");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("service reach `Queryable` exceeds its authored ceiling")
    }));
}

#[test]
fn static_machine_argument_admits_service_reach_within_parent_closure() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Readable {
            machine read();
        }

        boundary trait Filesystem: Readable {
            machine inspect();
        }

        machine work(service: &mut Filesystem) {
            service.read();
        }

        machine invoke<machine F>(service: &mut Filesystem)
        where machine F(service: &mut Filesystem) effects Filesystem;
        {}

        machine caller(service: &mut Filesystem) {
            invoke<work>(service);
        }
        "#,
    );

    super::validate_static_machine_selections(&typed)
        .expect("a child service ceiling includes its boundary-service parents");
}

#[test]
fn recursive_private_operational_inference_reaches_independent_fixed_points() {
    let typed = typed_program_from_source(
        r#"
        boundary trait WaitingSource {
            machine wait() suspends; blocks;
        }

        data A {}
        data B {}

        machine A::step(&mut self, b: &mut B, source: &mut WaitingSource) {
            b.step(self, source);
        }

        machine B::step(&mut self, a: &mut A, source: &mut WaitingSource) {
            a.step(self, source);
            source.wait();
        }
        "#,
    );
    let operations = omega_effects::infer_operational_may(&typed);

    for name in ["A::step", "B::step"] {
        let symbol = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol;
        let summary = operations
            .machines()
            .iter()
            .find(|summary| summary.symbol == symbol)
            .unwrap_or_else(|| panic!("operational summary for {name}"));
        assert!(summary.body_may_suspend, "{name} should infer suspension");
        assert!(summary.body_may_block, "{name} should infer blocking");
        assert!(summary.transitive_may_suspend);
        assert!(summary.transitive_may_block);
    }
}

#[test]
fn published_operational_omission_is_a_negative_ceiling() {
    let typed = typed_program_from_source(
        r#"
        boundary trait WaitingSource {
            machine wait() suspends; blocks;
        }

        data Published { source: WaitingSource; }
        boundary machine Published::entry(&mut self) {
            self.source.wait();
        }
        "#,
    );
    let operations = omega_effects::infer_operational_may(&typed);
    let diagnostics = validate_behavior_plan(&typed, &operations)
        .expect_err("published omission must reject inferred operational behavior");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("omits `suspends;`")
            && diagnostic.message.contains("body may suspend")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("omits `blocks;`")
            && diagnostic.message.contains("body may block")
    }));
}

#[test]
fn static_machine_argument_rejects_callable_shape_mismatch() {
    let typed = typed_program_from_source(
        r#"
        data Card {}
        machine Card::power(value: u64) {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {}

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("shape mismatch must fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("parameter 0 expects `&T`, got `u64`")
    }));
}

#[test]
fn generic_call_rejects_missing_static_machine_argument() {
    let typed = typed_program_from_source(
        r#"
        data Card {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {}

        machine caller(card: &Card) {
            map(card);
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("missing selection must fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires 1 static machine argument(s), got 0")
    }));
}

#[test]
fn static_machine_argument_rejects_stronger_precondition() {
    let typed = typed_program_from_source(
        r#"
        data Card {}
        machine Card::power(value: &Card)
        requires value == value
        {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {}

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("stronger precondition must fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires facts are not a conservative refinement")
    }));
}

#[test]
fn generic_body_call_is_checked_against_machine_parameter_contract() {
    let typed = typed_program_from_source(
        r#"
        data Card {}

        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        {
            F(value);
        }
        "#,
    );
    validate_program(&typed).expect("generic body should use the authored callable contract");
}

#[test]
fn higher_order_machine_parameter_refines_and_forwards_distinct_schema() {
    let typed = typed_program_from_source(
        r#"
        data Index {
            case Zero;
            case Next(previous: Index);
        }

        data Stream<machine S>
        where machine S(index: Index) -> Index;
        {
            case Empty;
            case More(sample: Index, tail: Stream<S>);
        }

        boundary machine sample(index: Index) -> Index;

        machine identity_schema<machine Chosen>(value: Stream<Chosen>) -> Stream<Chosen>
        where machine Chosen(index: Index) -> Index;
        {
            value
        }

        machine forward_schema<machine Schema, machine Selected>(value: Stream<Selected>) -> Stream<Selected>
        where machine Schema<machine Inner>(value: Stream<Inner>) -> Stream<Inner>
        where machine Inner(index: Index) -> Index;
        where machine Selected(index: Index) -> Index;
        {
            Schema<Selected>(value)
        }

        machine accepts_concrete(value: Stream<sample>) -> Stream<sample> {
            forward_schema<identity_schema, sample>(value)
        }
        "#,
    );

    let forward = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward_schema")
        .expect("forward_schema");
    let selected = typed
        .machine_type_parameters(forward)
        .iter()
        .find(|parameter| parameter.name.as_str() == "Selected")
        .expect("Selected");
    let call = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            omega_typed_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "Schema" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("Schema<Selected> call");
    assert_eq!(call.machine_arguments[0].symbol, selected.symbol);
    super::validate_static_machine_selections(&typed)
        .expect("higher-order static schema should refine and validate");
}

#[test]
fn generic_body_call_rejects_argument_outside_machine_parameter_contract() {
    let typed = typed_program_from_source(
        r#"
        data Card {}

        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        {
            F(7);
        }
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("generic body call shape must be checked");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("argument `item`")
            && diagnostic.message.contains("expects `&T`")
    }));
}

#[test]
fn generic_machine_signature_may_forward_type_parameters_into_generic_data() {
    let typed = typed_program_from_source(
        r#"
        data Outcome<T> {
            case Empty;
            case Returned(value: T);
        }

        machine wrap<T>(value: T) -> Outcome<T> {
            Outcome::Returned { value: value }
        }
        "#,
    );

    validate_program(&typed)
        .expect("an in-scope machine type parameter should remain valid inside generic data");
}

#[test]
fn machine_parameterized_data_must_be_proof_only() {
    let typed = typed_program_from_source(
        r#"
        data RuntimeCallback<machine F>
        where machine F(value: u64) -> u64;
        {
            value: u64;
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("finite-layout data must not be indexed by a static machine");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("parameterized by a static machine but has a runtime layout")
    }));
}

#[test]
fn machine_parameter_cannot_be_stored_as_a_data_field() {
    let typed = typed_program_from_source(
        r#"
        data ProofFamily<machine F>
        where machine F(value: u64) -> u64;
        {
            case End;
            case More(callback: F, tail: ProofFamily<F>);
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a static machine parameter is not a value type");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses machine parameter `F` as a stored/runtime type")
    }));
}

#[test]
fn validates_main_entry_surface_from_source_pipeline() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    assert_eq!(typed.machines().len(), 1);
    assert_eq!(typed.machines()[0].name.as_str(), "Main::main");
    assert_eq!(typed.machine_states(&typed.machines()[0]).len(), 1);
    assert_eq!(
        typed.machine_states(&typed.machines()[0])[0].name.as_str(),
        "main"
    );
    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn validates_local_state_call_arguments_from_source_pipeline() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
        take_non_negative(0);

        state take_non_negative(
            &mut self,
            value: u32[exact, non_negative]
        ) {
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let entry = typed
        .machine_states(&typed.machines()[0])
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("entry state");
    let call_argument_count = typed
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            omega_typed_trees::statement::StatementNode::Call(call) => Some(call.arguments.len()),
            omega_typed_trees::statement::StatementNode::Expression(expression) => {
                let omega_typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(*expression)
                else {
                    return None;
                };
                Some(call.arguments.len())
            }
            _ => None,
        })
        .expect("expected call statement");
    assert_eq!(call_argument_count, 1);
    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn rejects_unknown_trait_machine_effects() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: &[u8])
        effects
            stdoutish;
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject effect");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("unknown boundary service `stdoutish`")),
        "expected unknown service diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_unknown_domain_membership_in_domain_body() {
    let source = r#"
    data Player {
    }

    domain Player::Alive {
        self in Player::Valid
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("domain `Player::Alive` references unknown domain `Player::Valid`")),
        "expected unknown domain diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn repeated_normalized_domain_name_requires_equal_semantics() {
    let matching = typed_program_from_source(
        r#"
        domain [u8; 8]::Utf8 { valid_utf8(self); }
        domain [u8; 16]::Utf8 { valid_utf8(self); }
        "#,
    );
    validate_program(&matching)
        .expect("equal capacity specializations should share their normalized domain name");

    let divergent = typed_program_from_source(
        r#"
        domain [u8; 8]::Utf8 { valid_utf8(self); }
        domain [u8; 16]::Utf8 { no_nul(self); }
        "#,
    );
    let diagnostics =
        validate_program(&divergent).expect_err("different semantics under one name must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("declared more than once with different normalized semantics")
    }));
}

#[test]
fn domain_constraints_resolve_same_short_name_by_carrier() {
    let typed = typed_program_from_source(
        r#"
        data Holder {
            signed: i64 in Tagged;
            unsigned: u64 in Tagged;
        }
        domain i64::Tagged {}
        domain u64::Tagged { self >= 0; }
        "#,
    );
    validate_program(&typed)
        .expect("each short domain name should retain its carrier-specific identity");
}

#[test]
fn declared_domain_constraint_with_missing_normalized_identity_fails_closed() {
    let mut typed = typed_program_from_source(
        r#"
        data Holder { value: i64 in Tagged; }
        domain i64::Tagged {}
        "#,
    );
    let (_, constraints) = typed
        .type_reference_table
        .constrained_type_references()
        .into_iter()
        .find(|(_, constraints)| {
            typed
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    matches!(
                        constraint,
                        omega_typed_trees::types::TypeConstraintNode::Domain(_)
                    )
                })
        })
        .expect("declared domain constraint");
    let domain = typed
        .type_reference_table
        .constraints_mut(constraints)
        .iter_mut()
        .find_map(|constraint| match constraint {
            omega_typed_trees::types::TypeConstraintNode::Domain(domain) => Some(domain),
            _ => None,
        })
        .expect("domain constraint");
    domain.symbol = omega_core::symbols::SymbolHandle::invalid();

    let diagnostics = validate_program(&typed)
        .expect_err("a declared-domain constraint cannot fall back to global name lookup");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no `domain` named `Tagged` is declared")
    }));
}

#[test]
fn semantic_only_domain_mint_does_not_invent_a_predicate_obligation() {
    let typed = typed_program_from_source(
        r#"
        domain i64::Km {}

        machine qualify(raw: i64) {
            let distance: i64 = raw as i64 in Km;
        }
        "#,
    );
    validate_program(&typed)
        .expect("a semantic-only qualification owes authority but no predicate proof");
}

#[test]
fn predicate_only_domain_rejects_semantic_qualification() {
    let mut typed = typed_program_from_source(
        r#"
        domain i64::Km {}

        machine qualify(raw: i64) {
            let distance: i64 = raw as i64 in Km;
        }
        "#,
    );
    let domain_roots = typed.roots.domain_definitions;
    let [domain] = typed
        .tables
        .domain_definitions
        .span_mut_or_empty(domain_roots)
    else {
        panic!("one domain declaration")
    };
    domain.facets = omega_core::semantics::DomainFacets {
        predicate: true,
        semantic: None,
    };

    let diagnostics =
        validate_program(&typed).expect_err("predicate-only domains cannot qualify values");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires a semantic facet"))
    );
}

fn validate_contract_source(source: &str) -> Result<(), Vec<omega_core::diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");
    validate_program(&typed)
}

#[test]
fn boundary_out_param_ensures_seeds_the_value_env() {
    // R4 witness mint: `fw.get_size(&mut self.n)` with `ensures size <= 8`
    // leaves `self.n` in [0, 8], so the exact-overflow proof on `n + 1`
    // discharges without a guard.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; n: u32; m: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.m = self.n + 1;
    }
    "#,
    )
    .expect("the ensures-seeded bound should discharge the exact-overflow proof");
}

#[test]
fn boundary_out_param_without_ensures_stays_unproven() {
    // The negative rail: the same shape without the ensures keeps the
    // decision-17 refusal (the call clears the env; nothing re-seeds).
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32);
    }
    data Main { fw: Firmware; n: u32; m: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.m = self.n + 1;
    }
    "#,
    )
    .expect_err("without the ensures the addition must stay unproven");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("may overflow `u32`")),
        "expected the exact-arithmetic refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_out_param_ensures_dies_at_the_next_write() {
    // Env facts are flow-scoped: writing the place kills the seeded bound.
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; n: u32; other: u32; m: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.n = self.other;
        self.m = self.n + 1;
    }
    "#,
    )
    .expect_err("the rebound place must lose the ensures fact");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("may overflow `u32`")),
        "expected an exact-arithmetic refusal after the rebind, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_witness_discharges_recast_footprint() {
    // R4 recast route (the M2 memory-map mini-shape): `ensures size <= 8`
    // on the boundary out-param bounds the transition argument, so the
    // callee's recast footprint `8 + size(u32) <= 12` proves with no guard
    // and no declared range on `off`.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect("the ensures witness should discharge the recast footprint");
}

#[test]
fn boundary_ensures_witness_too_wide_refuses_recast_footprint() {
    // `ensures size <= 9` admits offset 9: a 4-byte view reads bytes 9..13
    // of a 12-byte region -- refused with the footprint named.
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 9;
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect_err("a bound past the footprint must refuse");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("would read past the buffer")),
        "expected the footprint refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_witness_survives_unrelated_intervening_call() {
    // The boundary receiver and the caller's `n` field are distinct places.
    // A second call that receives neither `n` nor its owner cannot erase the
    // first call's established bound.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
        machine poke();
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.fw.poke();
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect("an unreachable caller field must retain its witness");
}

#[test]
fn boundary_ensures_witness_dies_when_intervening_call_borrows_place_mutably() {
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
        machine poke(size: &mut u32);
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.fw.poke(&mut self.n);
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect_err("an explicit exclusive argument must kill the witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("recast")),
        "expected a recast refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_witness_survives_unrelated_internal_call() {
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; other: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.touch_other();
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    machine Main::touch_other(&mut self) {
        self.other = 1;
    }
    "#,
    )
    .expect("an internal call with a disjoint may-write set must preserve the witness");
}

#[test]
fn boundary_ensures_witness_dies_when_internal_call_writes_place() {
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.touch_n();
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    machine Main::touch_n(&mut self) {
        self.n = 1;
    }
    "#,
    )
    .expect_err("an internal may-write must invalidate the witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("recast")),
        "expected a recast refusal, got {diagnostics:#?}"
    );
}

#[test]
fn dependent_entry_fact_survives_unrelated_internal_call() {
    validate_contract_source(
        r#"
    data Main { count: u32; other: u32; remaining: u32; }
    machine Main::main(&mut self) { }
    machine Main::read(&mut self, i: u32 [0..=self.count]) {
        self.touch_other();
        self.remaining = self.count - i;
    }
    machine Main::touch_other(&mut self) {
        self.other = 1;
    }
    "#,
    )
    .expect("a disjoint internal call must preserve the dependent entry fact");
}

#[test]
fn dependent_entry_fact_dies_when_internal_call_writes_bound_field() {
    let diagnostics = validate_contract_source(
        r#"
    data Main { count: u32; remaining: u32; }
    machine Main::main(&mut self) { }
    machine Main::read(&mut self, i: u32 [0..=self.count]) {
        self.touch_count();
        self.remaining = self.count - i;
    }
    machine Main::touch_count(&mut self) {
        self.count = 1;
    }
    "#,
    )
    .expect_err("a call writing the bound field must invalidate the dependent fact");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("may overflow `u32`")),
        "expected the exact-arithmetic refusal, got {diagnostics:#?}"
    );
}

#[test]
fn symbolic_walk_recast_footprint_discharges() {
    // M2 gap 4b, the full chain: the walk state re-enters itself with
    // `offset + desc_size` under the guard `offset + desc_size + desc_size
    // <= map_size`; map_size's upper witness (<= 64) and desc_size's lower
    // witness (>= 8) resolve symbolically through the per-edge meet
    // (self-forwarding edges preserve entry bounds), so the loop edge
    // bounds the offset at 64 - 8 = 56 and the Desc footprint 56 + 8 <= 64
    // discharges. The entry edge is the constant 0.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(map_size: &mut u32, desc_size: &mut u32)
        ensures map_size <= 64 && desc_size >= 8;
    }
    data Desc { a: u32; b: u32; }
    data Main { fw: Firmware; buf: [u8; 64]; map_size: u32; desc_size: u32; }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(&mut self.map_size, &mut self.desc_size);
        transition { _ -> walk(self.map_size, self.desc_size, 0) }
        state walk(&mut self, map_size: u32 in Trapping, desc_size: u32 in Trapping, offset: u32 in Trapping) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size + desc_size <= map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
    }
    "#,
    )
    .expect("the symbolic walk chain should discharge the recast footprint");
}

#[test]
fn boundary_ensures_equalities_couple_symbolic_recast_witnesses() {
    // R1/R4: the boundary supplies bounds through equal out-parameters rather
    // than directly on the values consumed by the walk. Equality transports
    // both the upper map-size witness and lower descriptor-size witness.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(
            map_size: &mut u32,
            desc_size: &mut u32,
            map_limit: &mut u32,
            desc_floor: &mut u32
        )
        ensures map_size == map_limit && map_limit <= 64
            && desc_size == desc_floor && desc_floor >= 8;
    }
    data Desc { a: u32; b: u32; }
    data Main {
        fw: Firmware;
        buf: [u8; 64];
        map_size: u32;
        desc_size: u32;
        map_limit: u32;
        desc_floor: u32;
    }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(
            &mut self.map_size,
            &mut self.desc_size,
            &mut self.map_limit,
            &mut self.desc_floor
        );
        transition { _ -> walk(self.map_size, self.desc_size, 0) }
        state walk(
            &mut self,
            map_size: u32 in Trapping,
            desc_size: u32 in Trapping,
            offset: u32 in Trapping
        ) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size + desc_size <= map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
    }
    "#,
    )
    .expect("equal out-parameters should transport both symbolic bounds");
}

#[test]
fn symbolic_walk_guard_route_discharges() {
    // The owner-corrected HONEST spelling (Cathedral f0b7572): no trait
    // ensures (a vouch would be false under BUFFER_TOO_SMALL) -- a
    // post-call SANITY GUARD establishes both witnesses on the entry
    // edge, and the same symbolic chain discharges.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(map_size: &mut u32, desc_size: &mut u32);
    }
    data Desc { a: u32; b: u32; }
    data Main { fw: Firmware; buf: [u8; 64]; map_size: u32; desc_size: u32; }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(&mut self.map_size, &mut self.desc_size);
        transition self.map_size <= 64 && self.desc_size >= 8 {
            true -> walk(self.map_size, self.desc_size, 0)
            _ -> bad()
        }
        state walk(&mut self, map_size: u32 in Trapping, desc_size: u32 in Trapping, offset: u32 in Trapping) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size + desc_size <= map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
        state bad(&mut self) { }
    }
    "#,
    )
    .expect("the guard-route walk chain should discharge the recast footprint");
}

#[test]
fn symbolic_walk_recast_wide_witness_refuses() {
    // `map_size <= 65` drags the loop-edge bound to 57; 57 + 8 > 64.
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(map_size: &mut u32, desc_size: &mut u32)
        ensures map_size <= 65 && desc_size >= 8;
    }
    data Desc { a: u32; b: u32; }
    data Main { fw: Firmware; buf: [u8; 64]; map_size: u32; desc_size: u32; }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(&mut self.map_size, &mut self.desc_size);
        transition { _ -> walk(self.map_size, self.desc_size, 0) }
        state walk(&mut self, map_size: u32 in Trapping, desc_size: u32 in Trapping, offset: u32 in Trapping) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size + desc_size <= map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
    }
    "#,
    )
    .expect_err("a witness past the footprint must refuse");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("would read past the buffer")),
        "expected the footprint refusal, got {diagnostics:#?}"
    );
}

#[test]
fn symbolic_walk_weak_guard_spelling_refuses() {
    // The `< map_size` spelling bounds the next descriptor's START, not
    // its END: bound 63, 63 + 8 > 64 -- the exact tail overrun the design
    // note calls out; the chain must refuse it.
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(map_size: &mut u32, desc_size: &mut u32)
        ensures map_size <= 64 && desc_size >= 8;
    }
    data Desc { a: u32; b: u32; }
    data Main { fw: Firmware; buf: [u8; 64]; map_size: u32; desc_size: u32; }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(&mut self.map_size, &mut self.desc_size);
        transition { _ -> walk(self.map_size, self.desc_size, 0) }
        state walk(&mut self, map_size: u32 in Trapping, desc_size: u32 in Trapping, offset: u32 in Trapping) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size < map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
    }
    "#,
    )
    .expect_err("the weak guard spelling admits a tail overrun and must refuse");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("would read past the buffer")),
        "expected the footprint refusal, got {diagnostics:#?}"
    );
}

#[test]
fn bracket_range_is_requires_sugar_for_ensures() {
    // R1 bracket-as-sugar (ch12): `k: u64 [0..=8]` IS `requires k >= 0 &&
    // k <= 8` -- the entailment engine assumes it, so `result <= 9`
    // proves for `result = k + 1` with no spelled requires.
    validate_contract_source(
        r#"
    data Main { }
    machine Main::pick(&mut self, k: u64 [0..=8]) -> u64
    ensures
        result <= 9
    {
        transition { _ -> (k + 1) }
    }
    machine Main::main(&mut self) {
        let v: u64 = self.pick(3);
    }
    "#,
    )
    .expect("the bracket range should discharge the ensures");
}

#[test]
fn bracket_range_sugar_does_not_over_prove() {
    // k = 8 makes k + 1 = 9 > 8: the tighter ensures must stay unproven.
    let diagnostics = validate_contract_source(
        r#"
    data Main { }
    machine Main::pick(&mut self, k: u64 [0..=8]) -> u64
    ensures
        result <= 8
    {
        transition { _ -> (k + 1) }
    }
    machine Main::main(&mut self) {
        let v: u64 = self.pick(3);
    }
    "#,
    )
    .expect_err("an ensures the range cannot support must refuse");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "expected the ensures refusal, got {diagnostics:#?}"
    );
}

#[test]
fn refutes_constant_false_ensures_on_empty_proof_machine() {
    let diagnostics = validate_contract_source(
        r#"
    machine false_constant()
    ensures
        1nat + 1nat == 3nat
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect_err("constant-false ensures should be refuted");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "ensures contract proof fact `1 + 1 == 3` is disproved by constant arithmetic"
            )),
        "expected constant refutation diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn refutes_strict_order_asymmetry_between_requires_and_ensures() {
    let diagnostics = validate_contract_source(
        r#"
    machine false_asymmetry(i: u64, j: u64)
    requires
        i < j
    ensures
        j < i
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect_err("asymmetric strict-order ensures should be refuted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "ensures contract proof fact `j < i` is disproved: the requires contract entails its negation"
        )),
        "expected asymmetry refutation diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn proves_entailment_ladder_on_empty_proof_machines() {
    validate_contract_source(
        r#"
    machine sum_in_range(a: i32, b: i32)
    requires
        a in 1..=10
        b in 1..=10
    ensures
        a + b in 2..=20
    {
    }

    machine congruence_add(a: u64, b: u64)
    requires
        a == b
    ensures
        a + 1 == b + 1
    {
    }

    machine square_in_range(a: i32)
    requires
        a in 0..=10
    ensures
        a * a in 0..=100
    {
    }

    machine antisymmetry(a: u64, b: u64)
    requires
        a <= b
        b <= a
    ensures
        a == b
    {
    }

    machine remainder_bound(a: u64)
    ensures
        a % 2 in 0..=1
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("the entailment ladder's true theorems must prove");
}

#[test]
fn rejects_unprovable_in_language_ensures() {
    let diagnostics = validate_contract_source(
        r#"
    machine sum_corner_too_tight(a: i32, b: i32)
    requires
        a in 1..=10
        b in 1..=10
    ensures
        a + b in 2..=19
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect_err("an unprovable in-language ensures must be rejected");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove ensures contract proof fact")),
        "expected cannot-prove diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn stands_down_on_out_of_language_contracts() {
    // `min(a, b)` is an unknown call shape: the engine cannot read it, so an
    // unprovable ensures must be ACCEPTED unchecked rather than rejected.
    validate_contract_source(
        r#"
    machine uses_unknown_call(a: u64, b: u64)
    requires
        min(a, b) >= 1
    ensures
        a >= 1
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("contracts outside the engine's language stand down");
}

#[test]
fn accepts_vacuous_ensures_under_contradictory_requires() {
    // Contradictory hypotheses entail anything (the `absurd` case), so the
    // refutation pass must stand down rather than reject a vacuous truth.
    validate_contract_source(
        r#"
    machine vacuous_truth(i: u64, j: u64)
    requires
        i < j
        j < i
    ensures
        j < i
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("vacuously true contract should not be refuted");
}

#[test]
fn refutation_skips_machines_with_body_statements() {
    // A body could establish facts beyond the requires set; the refutation
    // pass only reasons about empty-body proof artifacts. The constant-false
    // ensures here is still wrong, but rejecting it is the entailment
    // engine's job, not this pass's.
    validate_contract_source(
        r#"
    data Counter {
        value: i32;
    }

    machine Counter::touch(&mut self)
    ensures
        1nat + 1nat == 3nat
    {
        self.value = 1;
    }

    data Main {
        counter: Counter;
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("non-empty bodies are outside the refutation pass");
}

#[test]
fn proves_inductive_theorem_via_recursive_contract_and_decrease() {
    // Ladder rung L7: the accumulator Gauss sum. The recursive arm assumes
    // the machine's own ensures for (n - 1, acc + n) -- justified by the
    // discharged strict decrease of `n` under the n > 0 guard -- and the base
    // arm grounds `result := acc` under n == 0.
    validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64, acc: u64) -> u64
    terminates by n;
    ensures
        result * 2 == acc * 2 + n * (n + 1)
    {
        transition n > 0 {
            true -> self.gauss_sum(n - 1, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .expect("the inductive gauss-sum theorem must prove");
}

#[test]
fn refutes_inductive_false_twin_on_the_base_arm() {
    // The off-by-one twin's inductive step is self-consistent; the base arm
    // (n == 0, result := acc) collapses to 0 == 1 and must be disproved.
    let diagnostics = validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64, acc: u64) -> u64
    terminates by n;
    ensures
        result * 2 == acc * 2 + n * (n + 1) + 1
    {
        transition n > 0 {
            true -> self.gauss_sum(n - 1, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .expect_err("the off-by-one inductive twin must be disproved on its base arm");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "is disproved on the transition arm guarded by `n > 0 == false`: the arm's facts entail its negation"
        )),
        "expected base-arm refutation diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_inductive_step_false_twin_on_the_recursive_arm() {
    // Base case true, step false: the instantiated hypothesis misses the
    // goal, every fact is in-language and the decrease IS discharged, so the
    // recursive arm's unproven obligation rejects.
    let diagnostics = validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64, acc: u64) -> u64
    terminates by n;
    ensures
        result * 2 == acc * 2 + n * (n + 3)
    {
        transition n > 0 {
            true -> self.gauss_sum(n - 1, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .expect_err("the step-false inductive twin must be rejected on its recursive arm");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "cannot prove ensures contract proof fact `result * 2 == acc * 2 + n * n + 3` on the transition arm guarded by `n > 0 == true`"
        )),
        "expected recursive-arm cannot-prove diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn no_induction_hypothesis_without_a_decreases_claim() {
    // SOUNDNESS GATE: without `terminates by ...;` there is no
    // well-founded measure, so the recursive arm gets no induction
    // hypothesis. The goal cannot prove, but the missing hypothesis (not the
    // theorem) may be at fault, so the engine stands down rather than
    // rejecting -- the theorem is accepted UNCHECKED, exactly the bodied
    // status quo.
    validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64, acc: u64) -> u64
    ensures
        result * 2 == acc * 2 + n * (n + 1)
    {
        transition n > 0 {
            true -> self.gauss_sum(n - 1, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .expect("no decreases claim means no hypothesis and no rejection");
}

#[test]
fn no_induction_hypothesis_when_the_call_site_decrease_is_undischarged() {
    // The decreases clause names `n`, but the recursive call passes `n`
    // unchanged: the strict decrease fails at that exact call site, so no
    // hypothesis enters and the arm cannot drive a rejection. (The
    // machine-level termination pass independently rejects this program; the
    // entailment engine must not pile a false ensures rejection on top.)
    let diagnostics = validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64, acc: u64) -> u64
    terminates by n;
    ensures
        result * 2 == acc * 2 + n * (n + 1)
    {
        transition n > 0 {
            true -> self.gauss_sum(n, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .err()
    .unwrap_or_default();
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ensures contract proof fact")),
        "an undischarged call-site decrease must not produce an ensures rejection, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_true_theorems_on_empty_proof_machines() {
    validate_contract_source(
        r#"
    machine pythagorean_three_four_five()
    ensures
        3nat * 3nat + 4nat * 4nat == 5nat * 5nat
    {
    }

    machine less_than_transitive(a: u64, b: u64, c: u64)
    requires
        a < b
        b < c
    ensures
        a < c
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("true theorems should pass the refutation pass");
}

#[test]
fn rejects_unknown_domain_membership_in_contract() {
    let source = r#"
    data Player {
    }

    boundary trait Renderer {
        machine draw(player: Player)
        requires
            player in Player::Drawable;
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "trait `Renderer` state `draw` requires contract references unknown domain `Player::Drawable`"
        )),
        "expected unknown contract domain diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_non_boolean_shaped_proof_fact() {
    let source = r#"
    data Player {
    }

    domain Player::Weird {
        1 + 2
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject fact");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("domain `Player::Weird` proof fact `1 + 2` is not boolean-shaped")),
        "expected non-boolean proof fact diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_domain_import_with_different_target_type() {
    let source = r#"
    data Player {
    }

    data Enemy {
    }

    domain Enemy::Valid {
        true
    }

    domain Player::Alive {
        self in Enemy::Valid
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject import");
    let has_target_mismatch = diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "domain `Player::Alive` imports `Enemy::Valid` but they classify different types",
        )
    });
    assert!(
        has_target_mismatch,
        "expected domain target mismatch diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_domain_import_cycles() {
    let source = r#"
    data Player {
    }

    domain Player::Alive {
        self in Player::Valid
    }

    domain Player::Valid {
        self in Player::Alive
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject cycle");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "domain membership cycle: Player::Alive -> Player::Valid -> Player::Alive",
            )
        }),
        "expected domain cycle diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_machine_effects_outside_trait_ceiling() {
    let source = r#"
    boundary trait Filesystem {}

    boundary trait Console {
        machine write_line(text: &[u8])
        effects
            Console;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::write_line(text: &[u8]) satisfies Console
    effects
        Console + Filesystem
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject extra effect");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("service `Filesystem` is not allowed by the trait requirement")),
        "expected service ceiling diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_machine_effects_within_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: &[u8])
        effects
            Console;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::write_line(text: &[u8]) satisfies Console
    effects
        Console
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn accepts_machine_effects_below_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: &[u8])
        effects
            Console;
    }

    data TestConsole {
    }

    machine TestConsole::write_line(text: &[u8]) satisfies Console {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn rejects_published_service_ceiling_below_reached_services() {
    let source = r#"
    boundary trait Input {
        machine read_line(out: &mut [u8; 32]);
    }

    boundary trait Output {
    }

    data Main {
        input: Input;
    }

    machine Main::main(&mut self)
    effects
        Output
    {
        let line: [u8; 32];
        self.input.read_line(&mut line);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let operations = omega_effects::infer_operational_may(&typed);
    let diagnostics =
        validate_behavior_plan(&typed, &operations).expect_err("service ceiling should fail");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("publishes service reach `Output`")
                && diagnostic
                    .message
                    .contains("reaches undeclared service `Input`")
        }),
        "expected normalized service ceiling diagnostic, got {diagnostics:#?}"
    );
}

/// Tests relocated from `omega-effects` when that crate moved into the
/// `representations` layer. They exercise the effect/capability *analysis*
/// passes, which require building a `TypedTrees` through the front-of-pipeline
/// lowering crates. Those pipeline crates are dev-dependencies here (a
/// semantics crate may depend up into pipeline for tests), but must not be
/// dev-dependencies of `omega-effects` itself, since that would re-introduce a
/// `representations -> pipeline` upward edge.
mod effects_analysis {
    use omega_effects::{
        audit_boundary_provider_calls, build_boundary_provider_approval_registry,
        infer_operational_may, infer_service_reaches,
    };
    use omega_typed_trees::TypedTrees;

    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    fn lower(source: &str) -> TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    #[test]
    fn propagates_canonical_service_reach_to_call_sites() {
        let source = r#"
        boundary trait Console {
            machine write_line(text: &[u8]);
        }

        data ConsoleImpl {
        }

        machine ConsoleImpl::write_line(text: &[u8]) satisfies Console
        effects
            Console
        {
        }

        data Main {
            console: ConsoleImpl;
        }

        machine Main::main(&mut self) {
            self.console.write_line("hello");
        }
        "#;

        let typed = lower(source);
        let operations = infer_operational_may(&typed);
        let services = infer_service_reaches(&typed, &operations);
        let console = typed
            .service_reaches
            .id_for_name("Console")
            .expect("Console service");

        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_services = services
            .for_machine(main_machine.symbol)
            .expect("main service summary");
        assert_eq!(
            services.services(main_services.inferred_transitive),
            &[console]
        );
        let main_state = services
            .states_for(main_services)
            .first()
            .expect("main state");
        let call = services.calls_for(main_state).first().expect("call");
        assert_eq!(services.services(call.inferred_transitive), &[console]);
    }

    #[test]
    fn abstract_boundary_provider_is_authorized() {
        let program = lower(
            r#"
            boundary trait Console {
                machine write_line(text: &[u8]);
            }

            data Main {
                console: Console;
            }

            machine Main::main(&mut self) {
                self.console.write_line("hello");
            }
            "#,
        );
        let operations = infer_operational_may(&program);
        let registry = build_boundary_provider_approval_registry(&program);
        let unapproved = audit_boundary_provider_calls(&program, &operations, &registry);
        assert!(
            unapproved.is_empty(),
            "abstract provider should be approved"
        );
    }

    #[test]
    fn in_package_host_provider_is_unapproved() {
        let program = lower(
            r#"
            boundary trait LocalFiles {
                machine write_bytes(path: &[u8]);
            }

            data Disk {
            }

            machine Disk::write_bytes(path: &[u8]) satisfies LocalFiles {
            }

            data Main {
                disk: Disk;
            }

            machine Main::main(&mut self) {
                self.disk.write_bytes("/etc/passwd");
            }
            "#,
        );
        let operations = infer_operational_may(&program);
        let registry = build_boundary_provider_approval_registry(&program);
        let unapproved = audit_boundary_provider_calls(&program, &operations, &registry);
        assert_eq!(
            unapproved.len(),
            1,
            "in-package host provider should be flagged as unapproved"
        );
        let local_files = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "LocalFiles")
            .expect("LocalFiles boundary");
        assert_eq!(
            unapproved[0].boundary_trait_symbol, local_files.symbol,
            "approval failure must name the exact boundary capability",
        );
    }

    #[test]
    fn in_package_boundary_provider_needs_no_lowercase_effect_to_be_unapproved() {
        let program = lower(
            r#"
            boundary trait LocalClock {
                machine tick(token: i32);
            }

            data FakeClock {}

            machine FakeClock::tick(token: i32) satisfies LocalClock {}

            data Main {
                clock: FakeClock;
            }

            machine Main::main(&mut self) {
                self.clock.tick(0);
            }
            "#,
        );
        let operations = infer_operational_may(&program);
        let registry = build_boundary_provider_approval_registry(&program);
        let local_clock = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "LocalClock")
            .expect("LocalClock boundary");
        assert!(
            !registry
                .provider(local_clock.symbol)
                .expect("LocalClock approval")
                .approved
        );
        let call_targets = operations
            .machines()
            .iter()
            .flat_map(|machine| operations.states.span_or_empty(machine.states))
            .flat_map(|state| operations.calls.span_or_empty(state.calls))
            .map(|call| call.target_state_symbol)
            .collect::<Vec<_>>();
        let unapproved = audit_boundary_provider_calls(&program, &operations, &registry);

        assert_eq!(unapproved.len(), 1, "call targets: {call_targets:?}");
        assert_eq!(unapproved[0].boundary_trait_symbol, local_clock.symbol);
    }
}

/// Tests relocated from `omega-effects::capabilities::providers` when that
/// crate moved into the `representations` layer. They parse source into
/// `SyntaxTrees` to exercise the boundary-provider registry, which needs the
/// front-of-pipeline lexer/parser (dev-dependencies allowed here, but not on
/// `omega-effects` itself - see `effects_analysis` above for the rationale).
mod provider_registry {
    use omega_core::operator_spelling::ProviderCategory;
    use omega_effects::build_provider_registry;
    use omega_syntax_trees::SyntaxTrees;

    use omega_source_files_to_tokens::Lexer;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    fn parse(source: &str) -> SyntaxTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        parse_syntax_trees(&tokens).expect("parse")
    }

    #[test]
    fn host_abi_provider_metadata_comes_from_bound_operator() {
        let syntax = parse(
            r#"
            provider omega::host::WriteBytes : HostAbiCall;

            boundary operator omega::host::write(handle: i64, length: u64) -> u64
            provider omega::host::WriteBytes
            requires
                length > 0;
            "#,
        );

        let mut diagnostics = Vec::new();
        let registry = build_provider_registry(&syntax, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let provider = registry
            .resolve("omega::host::WriteBytes")
            .expect("provider should be registered");
        assert_eq!(provider.category, ProviderCategory::HostAbiCall);

        // contract_ref: sourced from the bound operator's `requires` clause,
        // referenced by the operator's qualified name.
        assert_eq!(provider.contract_ref.as_deref(), Some("omega::host::write"));

        assert!(
            provider.requires_host_authority,
            "host-ABI provider should require explicit host authority"
        );
    }

    #[test]
    fn compute_provider_carries_no_authority() {
        let syntax = parse(
            r#"
            provider omega::language::core::Slice : SliceIndexing;

            boundary operator omega::language::core::index<T>(items: &[T], index: u64) -> T
            spelling []
            provider omega::language::core::Slice
            requires
                index < items.len;
            "#,
        );

        let mut diagnostics = Vec::new();
        let registry = build_provider_registry(&syntax, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let provider = registry
            .resolve("omega::language::core::Slice")
            .expect("provider should be registered");
        assert_eq!(provider.category, ProviderCategory::SliceIndexing);
        assert_eq!(
            provider.contract_ref.as_deref(),
            Some("omega::language::core::index")
        );
        // Pure-compute primitive: no host authority.
        assert!(
            !provider.requires_host_authority,
            "slice indexing provider should not carry host authority"
        );
    }

    #[test]
    fn unbound_provider_keeps_empty_metadata() {
        let syntax = parse("provider omega::host::Unbound : HostAbiCall;");

        let mut diagnostics = Vec::new();
        let registry = build_provider_registry(&syntax, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let provider = registry
            .resolve("omega::host::Unbound")
            .expect("provider should be registered");
        // No operator binds it, so metadata stays at empty defaults.
        assert_eq!(provider.contract_ref, None);
        assert!(!provider.requires_host_authority);
        assert!(provider.target_applicability.is_empty());
    }
}

mod structural_entailment {
    use super::super::validate_program;
    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    /// Run source through parse -> resolve -> typed -> validate. `Nat` is
    /// declared inline (the judge is parametric over any recursive proof
    /// data, so no bundled import is needed). Returns the joined diagnostic
    /// text on failure, or the empty string on success.
    fn validate(body: &str) -> Result<(), String> {
        let source = format!(
            "data Nat {{ case Zero; case Succ(prev: Nat); }}\n\
             data Main {{}}\n\
             machine Main::main(&mut self) {{}}\n\
             machine add(a: Nat, b: Nat) -> Nat terminates by a; {{\n\
             transition a {{ Nat::Zero -> (b) Nat::Succ {{ prev }} -> Nat::Succ {{ prev: add(prev, b) }} }}\n\
             }}\n\
             {body}\n"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering");
        validate_program(&typed).map_err(|diagnostics| {
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    #[test]
    fn reflexivity_proves() {
        validate("machine refl(a: Nat) ensures a == a {}")
            .expect("a == a should prove structurally");
    }

    #[test]
    fn constructor_injectivity_proves() {
        validate(
            "machine inj(a: Nat, b: Nat) \
             requires (Nat::Succ { prev: a }) == (Nat::Succ { prev: b }); \
             ensures a == b; {}",
        )
        .expect("Succ injectivity should prove");
    }

    #[test]
    fn case_disjointness_refutes() {
        let error =
            validate("machine bad(a: Nat) requires a == Nat::Zero; ensures a != Nat::Zero; {}")
                .expect_err("a != Zero under a == Zero should refute");
        assert!(
            error.contains("disproved structurally"),
            "expected a structural disproof, got: {error}"
        );
    }

    #[test]
    fn ground_compute_proves() {
        validate(
            "machine two() ensures \
             (add(Nat::Succ { prev: Nat::Zero }, Nat::Succ { prev: Nat::Zero })) \
             == (Nat::Succ { prev: Nat::Succ { prev: Nat::Zero } }) {}",
        )
        .expect("1 + 1 == 2 should prove by unfolding");
    }

    #[test]
    fn false_ground_compute_refutes() {
        let error = validate(
            "machine bad() ensures \
             (add(Nat::Succ { prev: Nat::Zero }, Nat::Zero)) == Nat::Zero {}",
        )
        .expect_err("add(1, 0) == 0 should refute");
        assert!(
            error.contains("disproved structurally"),
            "expected a structural disproof, got: {error}"
        );
    }

    #[test]
    fn structural_induction_proves() {
        validate(
            "machine right_id(a: Nat) -> Nat terminates by a; \
             ensures result == a { \
             transition a { Nat::Zero -> Nat::Zero \
             Nat::Succ { prev } -> Nat::Succ { prev: right_id(prev) } } }",
        )
        .expect("right identity by induction should prove");
    }

    #[test]
    fn false_inductive_claim_refutes() {
        let error = validate(
            "machine bad(a: Nat) -> Nat terminates by a; \
             ensures result == Nat::Zero { \
             transition a { Nat::Zero -> Nat::Zero \
             Nat::Succ { prev } -> Nat::Succ { prev: bad(prev) } } }",
        )
        .expect_err("a false inductive claim should refute");
        assert!(
            error.contains("disproved structurally"),
            "expected a structural disproof, got: {error}"
        );
    }

    #[test]
    fn application_equation_law_proves() {
        validate(
            "machine succ_law(a: Nat, b: Nat) -> Nat terminates by a; \
             ensures (add(a, Nat::Succ { prev: b })) == (Nat::Succ { prev: add(a, b) }) { \
             transition a { Nat::Zero -> Nat::Zero \
             Nat::Succ { prev } -> Nat::Succ { prev: succ_law(prev, b) } } }",
        )
        .expect("the successor-shift law should prove by induction");
    }

    #[test]
    fn seq_length_cons_proves() {
        // Generic recursive Seq<T>: the judge is parametric over constructor
        // names, so length(Cons(x, s)) == Succ(length(s)) proves by one
        // unfold, the tail's length staying symbolic on both sides.
        validate(
            "data Seq { case Empty; case Cons(head: Nat, tail: Seq); } \
             machine length(s: Seq) -> Nat terminates by s; { \
             transition s { Seq::Empty -> Nat::Zero \
             Seq::Cons { head, tail } -> Nat::Succ { prev: length(tail) } } } \
             machine length_cons(x: Nat, s: Seq) \
             ensures (length(Seq::Cons { head: x, tail: s })) \
             == (Nat::Succ { prev: length(s) }); {}",
        )
        .expect("length_cons should prove by unfolding");
    }

    #[test]
    fn lemma_citation_proves() {
        // right_id proves `result == a`; a caller cites it (its inductive
        // body never finitely unfolds for a symbolic argument).
        validate(
            "machine right_id(a: Nat) -> Nat terminates by a; \
             ensures result == a { \
             transition a { Nat::Zero -> Nat::Zero \
             Nat::Succ { prev } -> Nat::Succ { prev: right_id(prev) } } } \
             machine cite(a: Nat) -> Nat ensures result == a { \
             transition { _ -> (right_id(a)) } }",
        )
        .expect("citing a proven functional ensures should prove");
    }

    /// The law-shaped lemma every statement-citation test consumes:
    /// `result == a` (functional) plus `add(a, Zero) == a` (the LAW an
    /// explicit citation delivers), both by one induction.
    const RIGHT_ID_LAW: &str = "machine right_id(a: Nat) -> Nat \
         terminates by a; \
         ensures result == a; (add(a, Nat::Zero)) == a; { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> Nat::Succ { prev: right_id(prev) } } }";

    #[test]
    fn law_conjunct_proves_alongside_functional_ensures() {
        validate(RIGHT_ID_LAW).expect("both conjuncts should prove by the same induction");
    }

    #[test]
    fn statement_citation_proves() {
        // ch10 "Citing Proofs" / the OWNER_QUESTIONS #14 answer: the bare
        // statement call injects right_id's law instantiated at `b`.
        validate(
            &(RIGHT_ID_LAW.to_owned()
                + " machine cite_zero(b: Nat) ensures (add(b, Nat::Zero)) == b { \
               right_id(b); }"),
        )
        .expect("a statement citation should deliver the law");
    }

    #[test]
    fn uncited_law_fact_fences_and_names_the_missing_citation() {
        // Nothing is global, nothing applies silently: the same goal
        // WITHOUT the citation statement must fence -- and the settled
        // ergonomics mitigation names the lemma whose ensures
        // shape-matches it, with the exact operands.
        let error = validate(
            &(RIGHT_ID_LAW.to_owned()
                + " machine uncited(b: Nat) ensures (add(b, Nat::Zero)) == b {}"),
        )
        .expect_err("an uncited law fact should fence");
        assert!(
            error.contains("no entailment tier judges yet"),
            "expected the N3 fence, got: {error}"
        );
        assert!(
            error.contains("note: `right_id` proves this shape -- cite it: `right_id(b);`"),
            "expected the citation suggestion, got: {error}"
        );
    }

    #[test]
    fn requires_bearing_citation_rejected() {
        // SITE DISCHARGE negative: the citer has no hypothesis establishing
        // the callee's instantiated requires, so the citation refuses and
        // names the undischarged fact.
        let error = validate(
            "machine sym(a: Nat, b: Nat) requires a == b; ensures b == a; {} \
             machine conditional_cite(x: Nat, y: Nat) ensures y == x { \
             sym(x, y); }",
        )
        .expect_err(
            "citing a requires-bearing lemma without establishing its requires should reject",
        );
        assert!(
            error.contains("is not established at this citation site"),
            "expected the site-discharge refusal, got: {error}"
        );
    }

    #[test]
    fn requires_bearing_citation_discharged() {
        // SITE DISCHARGE positive: the citer's own requires establish the
        // callee's instantiated requires, so the citation injects the
        // conditional ensures and the goal proves.
        validate(
            "machine sym(a: Nat, b: Nat) requires a == b; ensures b == a; {} \
             machine conditional_cite(x: Nat, y: Nat) requires x == y; \
             ensures y == x { sym(x, y); }",
        )
        .expect("a citation whose requires are established at the site should prove");
    }

    /// The successor-shift law, the step case's citation material.
    const SUCC_LAW: &str = "machine add_succ_law(a: Nat, b: Nat) -> Nat \
         terminates by a; \
         ensures (add(a, Nat::Succ { prev: b })) == (Nat::Succ { prev: add(a, b) }) { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> Nat::Succ { prev: add_succ_law(prev, b) } } }";

    /// COMMUTATIVITY (N3 rung 2): induction on `a` with per-arm SUB-STATE
    /// proofs -- the base case cites right_id's law at `b`, the step case
    /// cites add_succ_law AT THE CASE PAYLOAD (only reachable from a
    /// sub-state's frame) and takes the IH from its self-application.
    const ADD_COMM: &str = "machine add_comm(a: Nat, b: Nat) -> Nat \
         terminates by a; \
         ensures (add(a, b)) == (add(b, a)) { \
         transition a { Nat::Zero -> base_case(b) \
         Nat::Succ { prev } -> step_case(prev, b) } \
         state base_case(b: Nat) -> Nat { \
         right_id(b); \
         transition { _ -> (b) } } \
         state step_case(prev: Nat, b: Nat) -> Nat { \
         add_succ_law(b, prev); \
         transition { _ -> Nat::Succ { prev: add_comm(prev, b) } } } }";

    #[test]
    fn commutativity_proves_with_per_arm_citations() {
        validate(&(RIGHT_ID_LAW.to_owned() + " " + SUCC_LAW + " " + ADD_COMM))
            .expect("add_comm should prove by induction with per-arm citations");
    }

    #[test]
    fn citing_a_multi_state_lemma_delivers_its_law() {
        // The consumer face of rung 2: add_comm has THREE states (entry +
        // two sub-proofs); a citation reads only the ENTRY signature.
        // Pinned after a real regression -- the single-state `[entry]`
        // gate silently dropped multi-state callees' equations.
        validate(
            &(RIGHT_ID_LAW.to_owned()
                + " "
                + SUCC_LAW
                + " "
                + ADD_COMM
                + " machine use_comm(x: Nat, y: Nat) \
                   ensures (add(x, y)) == (add(y, x)) { \
                   add_comm(x, y); }"),
        )
        .expect("citing the proven add_comm should deliver its law");
    }

    #[test]
    fn commutativity_step_without_citation_fences() {
        // Drop the step case's citation: the IH alone cannot bridge
        // `add(b, Succ(prev))`, so the claim must fence, proving the
        // per-arm citation is load-bearing.
        let uncited = ADD_COMM.replace("add_succ_law(b, prev); ", "");
        let error = validate(&(RIGHT_ID_LAW.to_owned() + " " + SUCC_LAW + " " + &uncited))
            .expect_err("the step case without its citation should fence");
        assert!(
            error.contains("no entailment tier judges yet"),
            "expected the N3 fence, got: {error}"
        );
    }

    #[test]
    fn false_commutativity_shaped_claim_refutes() {
        // Same sub-state shape, wrong theorem: add(a, b) == add(b, b)
        // must refute on the base arm (add(Zero, b) == b vs add(b, b)).
        let error = validate(
            &(RIGHT_ID_LAW.to_owned()
                + " "
                + SUCC_LAW
                + " machine bad_comm(a: Nat, b: Nat) -> Nat \
                   terminates by a; \
                   ensures (add(a, b)) == (add(b, b)) { \
                   transition a { Nat::Zero -> base_case(b) \
                   Nat::Succ { prev } -> step_case(prev, b) } \
                   state base_case(b: Nat) -> Nat { \
                   right_id(b); \
                   transition { _ -> (b) } } \
                   state step_case(prev: Nat, b: Nat) -> Nat { \
                   add_succ_law(b, prev); \
                   transition { _ -> Nat::Succ { prev: bad_comm(prev, b) } } } }"),
        )
        .expect_err("a false comm-shaped claim should not prove");
        assert!(
            error.contains("no entailment tier judges yet")
                || error.contains("disproved structurally"),
            "expected a fence or refutation, got: {error}"
        );
    }

    #[test]
    fn nondescending_substate_recursion_rejected() {
        // The sub-state parameter is bound from `b` (not a payload of the
        // measure), so the self-call through it must reject.
        let error = validate(
            "machine bad(a: Nat, b: Nat) -> Nat \
             terminates by a; \
             ensures (add(a, b)) == (add(a, b)) { \
             transition a { Nat::Zero -> done(b) \
             Nat::Succ { prev } -> spin(b, b) } \
             state done(b: Nat) -> Nat { \
             transition { _ -> (b) } } \
             state spin(prev: Nat, b: Nat) -> Nat { \
             transition { _ -> Nat::Succ { prev: bad(prev, b) } } } }",
        )
        .expect_err("recursion through a non-descending sub-state parameter should reject");
        assert!(
            error.contains("cannot prove the measure"),
            "expected the descent error, got: {error}"
        );
    }

    /// Multiplication (the harness prelude only carries `add`).
    const MUL: &str = "machine mul(a: Nat, b: Nat) -> Nat \
         terminates by a; { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> (add(b, mul(prev, b))) } }";

    /// Associativity: pure induction, no citations -- both sides normalize
    /// by unfolding plus the IH rewrite.
    const ADD_ASSOC: &str = "machine add_assoc(a: Nat, b: Nat, c: Nat) -> Nat \
         terminates by a; \
         ensures (add(add(a, b), c)) == (add(a, add(b, c))) { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> Nat::Succ { prev: add_assoc(prev, b, c) } } }";

    const MUL_ZERO_RIGHT: &str = "machine mul_zero_right(a: Nat) -> Nat \
         terminates by a; \
         ensures (mul(a, Nat::Zero)) == Nat::Zero { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> Nat::Succ { prev: mul_zero_right(prev) } } }";

    /// The right-successor law for mul. Its step case is the citation
    /// CHOREOGRAPHY stress: three reducing rewrites (comm, assoc, comm)
    /// normalize `add(b, add(prev, m))` onto the goal's other side --
    /// explicit Dafny-style proof text, no search.
    const MUL_SUCC_RIGHT: &str = "machine mul_succ_right(a: Nat, b: Nat) -> Nat \
         terminates by a; \
         ensures (mul(a, Nat::Succ { prev: b })) == (add(a, mul(a, b))) { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> step_msr(prev, b) } \
         state step_msr(prev: Nat, b: Nat) -> Nat { \
         add_comm(b, add(prev, mul(prev, b))); \
         add_assoc(prev, mul(prev, b), b); \
         add_comm(mul(prev, b), b); \
         transition { _ -> Nat::Succ { prev: mul_succ_right(prev, b) } } } }";

    /// Multiplication commutes: base cites mul_zero_right, step cites
    /// mul_succ_right at (b, prev) plus the IH.
    const MUL_COMM: &str = "machine mul_comm(a: Nat, b: Nat) -> Nat \
         terminates by a; \
         ensures (mul(a, b)) == (mul(b, a)) { \
         transition a { Nat::Zero -> base_mc(b) \
         Nat::Succ { prev } -> step_mc(prev, b) } \
         state base_mc(b: Nat) -> Nat { \
         mul_zero_right(b); \
         transition { _ -> Nat::Zero } } \
         state step_mc(prev: Nat, b: Nat) -> Nat { \
         mul_succ_right(b, prev); \
         transition { _ -> Nat::Succ { prev: mul_comm(prev, b) } } } }";

    fn lemma_zoo() -> String {
        [
            RIGHT_ID_LAW,
            SUCC_LAW,
            ADD_COMM,
            MUL,
            ADD_ASSOC,
            MUL_ZERO_RIGHT,
            MUL_SUCC_RIGHT,
            MUL_COMM,
        ]
        .join(" ")
    }

    #[test]
    fn add_assoc_proves_without_citations() {
        validate(&(RIGHT_ID_LAW.to_owned() + " " + ADD_ASSOC))
            .expect("associativity should prove by pure induction");
    }

    #[test]
    fn mul_zero_right_proves() {
        validate(&(MUL.to_owned() + " " + MUL_ZERO_RIGHT))
            .expect("the right annihilator should prove by induction");
    }

    #[test]
    fn mul_succ_right_proves_with_citation_choreography() {
        validate(&lemma_zoo())
            .expect("the right-successor law should prove via comm/assoc/comm citations");
    }

    #[test]
    fn mul_comm_proves() {
        validate(&lemma_zoo()).expect("mul_comm should prove citing the mul laws");
    }

    #[test]
    fn mul_succ_right_without_choreography_fences() {
        // Dropping the three rearrangement citations strands the step case:
        // the IH alone cannot bridge the summand order. Since rearrange-mode
        // (2026-07-18) this test ALSO pins the license discipline: the comm
        // and assoc lemmas are IN SCOPE in this very program, but no
        // CommutativeSemiring conformance is declared, so the rearrange tier
        // must NOT fire -- explicit conformance, never scope-sniffing.
        let uncited = MUL_SUCC_RIGHT
            .replace("add_comm(b, add(prev, mul(prev, b))); ", "")
            .replace("add_assoc(prev, mul(prev, b), b); ", "")
            .replace("add_comm(mul(prev, b), b); ", "");
        let error = validate(
            &(RIGHT_ID_LAW.to_owned()
                + " "
                + SUCC_LAW
                + " "
                + ADD_COMM
                + " "
                + MUL
                + " "
                + ADD_ASSOC
                + " "
                + MUL_ZERO_RIGHT
                + " "
                + &uncited),
        )
        .expect_err("the step case without its choreography should fence");
        assert!(
            error.contains("no entailment tier judges yet"),
            "expected the N3 fence, got: {error}"
        );
    }

    #[test]
    fn seq_append_laws_prove() {
        // Right identity and associativity of append -- both pure
        // inductions on the first sequence, no citations.
        validate(
            "data Seq { case Empty; case Cons(head: Nat, tail: Seq); } \
             machine append(s: Seq, t: Seq) -> Seq terminates by s; { \
             transition s { Seq::Empty -> (t) \
             Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: append(tail, t) } } } \
             machine append_empty_right(s: Seq) -> Seq terminates by s; \
             ensures (append(s, Seq::Empty)) == s { \
             transition s { Seq::Empty -> Seq::Empty \
             Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: append_empty_right(tail) } } } \
             machine append_assoc(s: Seq, t: Seq, u: Seq) -> Seq terminates by s; \
             ensures (append(append(s, t), u)) == (append(s, append(t, u))) { \
             transition s { Seq::Empty -> Seq::Empty \
             Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: append_assoc(tail, t, u) } } }",
        )
        .expect("the append laws should prove by induction");
    }

    #[test]
    fn seq_length_append_proves() {
        // The Seq zoo's first COMPOSED law: length distributes over append,
        // by induction on the first sequence -- generic recursive data,
        // cross-machine unfolding (append inside length inside add), the
        // IH as a rewrite. No citations needed: both sides normalize.
        validate(
            "data Seq { case Empty; case Cons(head: Nat, tail: Seq); } \
             machine length(s: Seq) -> Nat terminates by s; { \
             transition s { Seq::Empty -> Nat::Zero \
             Seq::Cons { head, tail } -> Nat::Succ { prev: length(tail) } } } \
             machine append(s: Seq, t: Seq) -> Seq terminates by s; { \
             transition s { Seq::Empty -> (t) \
             Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: append(tail, t) } } } \
             machine length_append(s: Seq, t: Seq) -> Nat terminates by s; \
             ensures (length(append(s, t))) == (add(length(s), length(t))) { \
             transition s { Seq::Empty -> Nat::Zero \
             Seq::Cons { head, tail } -> Nat::Succ { prev: length_append(tail, t) } } }",
        )
        .expect("length_append should prove by induction");
    }

    #[test]
    fn citation_with_wrong_operands_does_not_prove_the_goal() {
        // The citation instantiates at ITS operands only -- citing the law
        // at `b` says nothing about the unrelated `c`. (Citing at `b` DOES
        // serve goals one compute step away, like `add(Succ(b), Zero) ==
        // Succ(b)` -- the Succ arm unfolds and exposes exactly the cited
        // instance; that is sound derivation, not leakage.)
        let error = validate(
            &(RIGHT_ID_LAW.to_owned()
                + " machine wrong_operand(b: Nat, c: Nat) \
               ensures (add(c, Nat::Zero)) == c { \
               right_id(b); }"),
        )
        .expect_err("a citation at the wrong operands should not discharge the goal");
        assert!(
            error.contains("no entailment tier judges yet"),
            "expected the N3 fence, got: {error}"
        );
    }
}

mod provider_plan {
    use omega_effects::build_boundary_provider_approval_registry;
    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema,
    };
    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    fn typed(source: &str) -> omega_typed_trees::TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("typed lowering")
    }

    #[test]
    fn service_schema_reifies_a_boundary_trait() {
        // PRV2: the schema derives from the typed boundary TraitDefinition
        // -- names, parameter counts (receiver excluded), result presence,
        // canonical service reach, and independent operational ceilings.
        let program = typed(
            "boundary trait Console {\n\
             machine write_line(text: &[u8])\n\
             effects\n\
                 Console\n\
             suspends;\n\
             blocks;\n\
             machine exit_process(return_code: i32);\n\
             }\n\
             data Main { console: Console; }\n\
             machine Main::main(&mut self) {}\n",
        );
        let console = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Console")
            .expect("Console trait");
        let schema =
            ServiceSchema::from_typed(&program, console).expect("boundary trait has a schema");
        assert_eq!(schema.trait_name, "Console");
        assert_eq!(schema.methods.len(), 2);
        assert_eq!(schema.methods[0].name, "write_line");
        assert_eq!(schema.methods[0].parameter_count, 1);
        assert!(!schema.methods[0].has_result);
        assert_eq!(schema.methods[0].service_reach, vec!["Console".to_owned()]);
        assert!(schema.methods[0].may_suspend);
        assert!(schema.methods[0].may_block);
        assert_eq!(schema.methods[1].name, "exit_process");
        assert_eq!(schema.methods[1].service_reach, vec!["Console".to_owned()]);
        assert!(!schema.methods[1].may_suspend);
        assert!(!schema.methods[1].may_block);
    }

    #[test]
    fn trait_parents_expand_requirements_without_laundering_policy_into_service_reach() {
        let program = typed(
            "trait Calling<C> {\n\
             machine policy_probe();\n\
             }\n\
             data X64Convention {}\n\
             boundary trait Device {\n\
             machine read()\n\
             effects\n\
                 Device;\n\
             }\n\
             boundary trait Timer: Device + Calling<X64Convention> {\n\
             machine tick();\n\
             }\n\
             data Main {}\n\
             machine Main::main(&mut self) {}\n",
        );
        let timer = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Timer")
            .expect("Timer trait");

        let parents = program.trait_requirements(timer);
        assert_eq!(parents.len(), 2);
        assert_eq!(
            program.trait_composition_kind(&parents[0]),
            Some(omega_typed_trees::trait_definition::TraitCompositionKind::ServiceReach)
        );
        assert_eq!(
            program.trait_composition_kind(&parents[1]),
            Some(omega_typed_trees::trait_definition::TraitCompositionKind::Policy)
        );

        let schema = ServiceSchema::from_typed(&program, timer).expect("boundary schema");
        assert_eq!(
            schema
                .methods
                .iter()
                .map(|method| method.name.as_str())
                .collect::<Vec<_>>(),
            ["read", "policy_probe", "tick"]
        );
        let method = |name: &str| {
            schema
                .methods
                .iter()
                .find(|method| method.name == name)
                .expect("schema method")
        };
        assert_eq!(method("read").service_reach, ["Device"]);
        assert!(
            method("policy_probe").service_reach.is_empty(),
            "ordinary policy parents must not mint service identities",
        );
        assert_eq!(method("tick").service_reach, ["Device", "Timer"]);

        let registry = build_boundary_provider_approval_registry(&program);
        let timer_provider = registry.provider(timer.symbol).expect("Timer provider");
        assert!(timer_provider.approved);
    }

    #[test]
    fn fingerprint_is_presentation_invariant() {
        // PRV2: identity excludes presentation -- reordering rows keeps the
        // fingerprint; changing a binding changes it.
        let schema = ServiceSchema {
            trait_name: "Console".to_owned(),
            methods: Vec::new(),
        };
        let row = |method: &str, number: u32| ProviderPlanRow {
            method: method.to_owned(),
            binding: ProviderBinding::Syscall { number },
        };
        let plan = |rows: Vec<ProviderPlanRow>| ProviderPlan {
            name: "p".to_owned(),
            provider_type: "P".to_owned(),
            target: "t".to_owned(),
            schema: schema.clone(),
            rows,
            origin_package: "omega::language::std".to_owned(),
        };
        let forward = plan(vec![row("a", 1), row("b", 2)]);
        let reversed = plan(vec![row("b", 2), row("a", 1)]);
        assert_eq!(
            forward.identity_fingerprint(),
            reversed.identity_fingerprint()
        );
        let changed = plan(vec![row("a", 9), row("b", 2)]);
        assert_ne!(
            forward.identity_fingerprint(),
            changed.identity_fingerprint()
        );
    }
}
