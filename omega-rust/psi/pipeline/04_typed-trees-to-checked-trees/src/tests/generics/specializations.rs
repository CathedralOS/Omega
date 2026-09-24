use super::specialized_machine;
use crate::CheckingRequest;
use crate::tests::front_end::{checked_program, checked_program_result, typed_program};
use crate::tests::lower_typed_trees;

#[test]
fn higher_order_machine_schema_specializes_nested_selection_to_fixed_point() {
    let source = r#"
        pub data Index {
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

        data Main {}
        machine Main::run(&mut self) {}
    "#;

    let checked = checked_program(source);

    for name in ["forward_schema", "identity_schema"] {
        assert!(
            checked
                .machine_specializations
                .iter()
                .any(|specialization| {
                    checked.machines().iter().any(|machine| {
                        machine.symbol == specialization.template && machine.name.as_str() == name
                    })
                }),
            "{name} should have a concrete specialization"
        );
    }
    let forward = specialized_machine(&checked, "forward_schema");
    let identity = specialized_machine(&checked, "identity_schema");
    let identity_entry = checked.machine_states(identity)[0].symbol;
    let mut expressions = Vec::new();
    for state in checked.machine_states(forward) {
        for statement in checked.statement_table.statements(state.statement_nodes) {
            crate::monomorphization::collect_statement_expression_trees(
                &checked.typed,
                statement,
                &mut expressions,
            );
        }
    }
    let calls = expressions
        .iter()
        .filter_map(
            |expression| match checked.expression_table.expression(*expression) {
                typed_trees::expression::ExpressionNode::Call(call) => Some(call),
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    assert!(!calls.is_empty());
    assert!(
        calls
            .iter()
            .all(|call| call.machine_arguments.is_empty() && call.target_symbol == identity_entry)
    );
    let mut wrong_template = checked.typed.clone();
    wrong_template
        .machine_specializations
        .iter_mut()
        .find(|specialization| specialization.instance == identity.symbol)
        .expect("identity receipt")
        .template = forward.symbol;
    assert!(
        validation::validate_static_machine_call_contracts(
            &wrong_template,
            &validation::infer_operational_may(&wrong_template)
        )
        .is_err(),
        "a same-signature instance of another schema cannot replace the selected declaration"
    );
    let mut missing_receipt = checked.typed.clone();
    missing_receipt
        .machine_specializations
        .retain(|specialization| specialization.instance != identity.symbol);
    assert!(
        validation::validate_static_machine_call_contracts(
            &missing_receipt,
            &validation::infer_operational_may(&missing_receipt)
        )
        .is_err(),
        "a higher-order target needs its exact schema application receipt"
    );
    let mut duplicate_receipt = checked.typed.clone();
    let receipt = duplicate_receipt
        .machine_specializations
        .iter()
        .find(|specialization| specialization.instance == identity.symbol)
        .expect("identity receipt")
        .clone();
    duplicate_receipt.machine_specializations.push(receipt);
    assert!(
        validation::validate_static_machine_call_contracts(
            &duplicate_receipt,
            &validation::infer_operational_may(&duplicate_receipt)
        )
        .is_err(),
        "duplicate higher-order receipts cannot establish a unique selected schema"
    );
}

#[test]
fn generic_body_must_discharge_machine_parameter_preconditions() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>(value: i32)
        where machine F(item: i32)
            requires item > 0
        {
            F(value);
        }
    "#;

    let diagnostics = checked_program_result(source)
        .expect_err("an unconstrained generic body must not assume F's precondition");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("item > 0")
            || diagnostic.message.contains("contract")
            || diagnostic.message.contains("proof")
    }));
}

#[test]
fn generic_body_can_discharge_machine_parameter_precondition_from_own_contract() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>(value: i32)
        where machine F(item: i32)
            requires item > 0;
        requires value > 0
        {
            F(value);
        }
    "#;

    checked_program(source);
}

#[test]
fn generic_body_can_discharge_machine_parameter_precondition_from_call_value() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>()
        where machine F(item: i32)
            requires item > 0
        {
            F(1);
        }
    "#;

    checked_program(source);
}

#[test]
fn generic_body_inherits_machine_parameter_service_ceiling() {
    let source = r#"
        boundary trait DeviceIo {
            machine touch();
        }

        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>()
        where machine F()
            reaches DeviceIo
        {
            F();
        }
    "#;

    let typed = typed_program(source);
    let apply = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("apply machine");
    let operations = validation::infer_operational_may(&typed);
    let service_reaches = validation::infer_service_reaches(&typed, &operations);
    let apply_reach = service_reaches
        .for_machine(apply.symbol)
        .expect("apply service-reach summary");
    let device_io = typed
        .service_reaches
        .id_for_name("DeviceIo")
        .expect("DeviceIo service identity");
    assert!(
        service_reaches
            .services(apply_reach.effective)
            .contains(&device_io)
    );
}

#[test]
fn static_machine_selection_respects_guarded_crash_ceiling() {
    let source = r#"
            machine selected(flag: bool)
            crashes Abort
                flag
            {}

            machine apply<machine Selected>(flag: bool)
            where machine Selected(value: bool)
                crashes Abort
                    value;
            {
                Selected(flag);
            }

            machine caller(flag: bool) {
                apply<selected>(flag);
            }
            "#;
    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("an identical crash route should refine the machine slot");
}

#[test]
fn generic_body_can_consume_machine_parameter_ensures() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        domain i32::Positive
        requires
            self > 0

        machine pipeline<machine Establish, machine Consume>(value: &mut i32)
        where machine Establish(item: &mut i32)
            ensures item in i32::Positive;
        where machine Consume(item: &i32)
            requires item in i32::Positive
        {
            Establish(value);
            Consume(value);
        }
    "#;

    checked_program(source);
}

#[test]
fn static_machine_argument_specializes_body_calls_to_direct_symbols() {
    let source = r#"
        data Card {}
        data Main {}

        machine Card::power(value: &Card) -> u64 {
            7
        }

        machine apply<T, machine F>(value: &T) -> u64
        where machine F(item: &T) -> u64
        {
            F(value)
        }

        machine caller(card: &Card) {
            let score: u64 = apply<Card::power>(card);
        }

        machine Main::run(&mut self) {}
    "#;

    let typed = typed_program(source);
    let power_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Card::power")
        .and_then(|machine| typed.machine_states(machine).first())
        .map(|state| state.symbol)
        .expect("power entry symbol");

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("static specialization should check");
    let apply = specialized_machine(&checked, "apply");
    assert!(checked.machine_type_parameters(apply).is_empty());
    assert_eq!(checked.machine_specializations.len(), 1);
    assert_eq!(checked.machine_specializations[0].instance, apply.symbol);
    assert_eq!(
        checked.machine_specializations[0].machine_arguments,
        vec![power_symbol]
    );
    assert_eq!(
        checked.machine_specializations[0].type_arguments,
        vec!["Card"]
    );
    assert_ne!(checked.machine_specializations[0].report_fingerprint, 0);

    let direct_call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call)
                if call.target_symbol == power_symbol =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("F(value) should become a direct Card::power call");
    assert_eq!(direct_call.target.as_str(), "power");
    assert!(direct_call.machine_arguments.is_empty());

    assert!(
        checked
            .expression_table
            .iter_expressions()
            .filter_map(|(_, expression)| match expression {
                typed_trees::expression::ExpressionNode::Call(call) => Some(call),
                _ => None,
            })
            .all(|call| call.machine_arguments.is_empty())
    );
}

#[test]
fn free_static_machine_specialization_preserves_authored_target_name() {
    let source = r#"
        data Main {}

        machine chosen(value: u16) -> u16 {
            value
        }

        machine apply<machine F>(value: u16) -> u16
        where machine F(item: u16) -> u16
        {
            F(value)
        }

        machine caller() -> u16 {
            apply<chosen>(70)
        }

        machine Main::run(&mut self) {}
    "#;

    let typed = typed_program(source);
    let chosen_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "chosen")
        .and_then(|machine| typed.machine_states(machine).first())
        .map(|state| state.symbol)
        .expect("chosen entry symbol");

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("free static selection should specialize");
    let direct_call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call)
                if call.target_symbol == chosen_symbol =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("F(value) should become a direct chosen call");
    assert_eq!(direct_call.target.as_str(), "chosen");
    assert_ne!(direct_call.target.as_str(), "entry");
    assert!(direct_call.machine_arguments.is_empty());
}

#[test]
fn static_machine_specialization_identity_is_reproducible() {
    fn report_fingerprint(source: &str) -> u64 {
        checked_program(source).machine_specializations[0].report_fingerprint
    }

    let source = r#"
        data Card {}
        data Main {}
        machine Card::power(value: &Card) {}
        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        { F(value); }
        machine caller(card: &Card) {
            apply<Card::power>(card);
        }
        machine Main::run(&mut self) {}
    "#;
    assert_eq!(report_fingerprint(source), report_fingerprint(source));
}

#[test]
fn specialization_commitment_replays_and_rejects_compact_equal_substitution() {
    let source = r#"
        data Card {}
        data Main {}
        machine Card::power(value: &Card) {}
        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        { F(value); }
        machine caller(card: &Card) {
            apply<Card::power>(card);
        }
        machine Main::run(&mut self) {}
    "#;
    let typed = typed_program(source);
    let checked =
        lower_typed_trees(typed, &CheckingRequest::settled()).expect("specialization should check");
    let specialization = &checked.machine_specializations[0];
    assert!(!specialization.commitment.is_zero());
    assert_eq!(
        specialization.machine_argument_contract_commitments.len(),
        1
    );
    assert_eq!(
        crate::monomorphization::identities::recompute_machine_specialization_commitment(
            &checked.typed,
            &checked.facts.contract_plans,
            specialization,
        )
        .expect("exact specialization custody should replay"),
        specialization.commitment
    );

    let mut substituted = checked.clone();
    let compact_report = substituted.typed.machine_specializations[0].report_fingerprint;
    substituted.typed.machine_specializations[0].type_argument_identities[0]
        .push_str("|compact-equal-substitute");
    assert_eq!(
        substituted.typed.machine_specializations[0].report_fingerprint, compact_report,
        "the adversary deliberately retains the compact report coordinate"
    );
    let contracts = substituted.facts.contract_plans.clone();
    let diagnostics = crate::monomorphization::bind_specialization_contract_identities(
        &mut substituted.typed,
        &contracts,
    )
    .expect_err("a compact-equal exact specialization substitution must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stale authoritative commitment")
    }));

    let mut stale_template_commitment = checked.clone();
    let template_report = stale_template_commitment.typed.machine_specializations[0]
        .template_contract_report_fingerprint;
    stale_template_commitment.typed.machine_specializations[0].template_contract_commitment =
        typed_trees::typed_trees::MachineTemplateCommitment::from_digest([0xa5; 32]);
    assert_eq!(
        stale_template_commitment.typed.machine_specializations[0]
            .template_contract_report_fingerprint,
        template_report,
        "the adversary deliberately retains the compact template coordinate"
    );
    let contracts = stale_template_commitment.facts.contract_plans.clone();
    let diagnostics = crate::monomorphization::bind_specialization_contract_identities(
        &mut stale_template_commitment.typed,
        &contracts,
    )
    .expect_err("a stale template commitment must fail despite compact equality");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("authoritative template commitment")
    }));

    let mut stale_report_row = checked.clone();
    stale_report_row.typed.machine_specializations[0]
        .machine_argument_contract_report_fingerprints[0] ^= 1;
    let contracts = stale_report_row.facts.contract_plans.clone();
    assert!(
        crate::monomorphization::bind_specialization_contract_identities(
            &mut stale_report_row.typed,
            &contracts,
        )
        .is_err(),
        "a stale nonzero compact contract row must not be silently refreshed"
    );

    let mut stale_commitment_row = checked.clone();
    stale_commitment_row.typed.machine_specializations[0].machine_argument_contract_commitments
        [0][0] ^= 1;
    let contracts = stale_commitment_row.facts.contract_plans.clone();
    assert!(
        crate::monomorphization::bind_specialization_contract_identities(
            &mut stale_commitment_row.typed,
            &contracts,
        )
        .is_err(),
        "a stale nonzero strong contract row must not be silently refreshed"
    );
}

#[test]
fn value_machine_type_parameter_is_inferred_through_a_borrowed_place() {
    let source = r#"
        data Light [copy] { weight: i32 in Wrapping; }
        data Main { light: Light; }

        machine Main::weigh<T [copy]>(&self, value: &T) -> i32 {
            70
        }

        machine Main::run(&mut self) {
            let result: i32 in Wrapping = self.weigh(&self.light);
        }
    "#;

    let checked = checked_program(source);

    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "Main::weigh"
            })
        })
        .expect("weigh specialization");
    assert_eq!(specialization.type_arguments, ["Light"]);
    assert_eq!(
        specialization.type_argument_identities,
        ["named(name(Light))"]
    );
}

#[test]
fn public_visibility_survives_value_type_specialization() {
    let source = r#"
        pub data Light [copy] { weight: i32 in Wrapping; }
        pub data Main { light: Light; }

        pub machine Main::weigh<T [copy]>(&self, value: &T) -> i32 {
            70
        }

        machine Main::run(&mut self) {
            let result: i32 in Wrapping = self.weigh(&self.light);
        }
    "#;

    let checked = checked_program(source);
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "Main::weigh"
            })
        })
        .expect("public weigh specialization");

    for (symbol, is_public) in [
        (specialization.template, true),
        (specialization.instance, false),
    ] {
        assert_eq!(
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == symbol)
                .expect("public machine retained through specialization")
                .is_public,
            is_public,
        );
    }
}

#[test]
fn distinct_static_machine_specializations_clone_the_template() {
    let source = r#"
        boundary trait Clock {}
        data Card {}
        data Main {}
        machine Card::power(value: &Card) {}
        machine Card::rank(value: &Card) {}
        machine apply<T, machine F>(value: &T)
        where machine F(item: &T);
        reaches Clock
        suspends;
        blocks;
        { F(value); }
        machine caller(card: &Card) {
            apply<Card::power>(card);
            apply<Card::rank>(card);
        }
        machine Main::run(&mut self) {}
    "#;
    let typed = typed_program(source);
    let typed_apply = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("typed apply template");
    assert_eq!(
        typed
            .authored_service_reach_rows_for(typed_apply.symbol)
            .count(),
        1,
        "typed template retains authored reach before specialization"
    );
    assert_eq!(typed_apply.suspends_keyword_source_spans.len(), 1);
    assert_eq!(typed_apply.blocks_keyword_source_spans.len(), 1);
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("each concrete machine tuple should receive its own specialization");
    let apply_specializations: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "apply"
            })
        })
        .collect();
    assert_eq!(apply_specializations.len(), 2);
    assert_ne!(
        apply_specializations[0].machine_arguments,
        apply_specializations[1].machine_arguments
    );
    assert_eq!(
        checked
            .machines()
            .iter()
            .filter(|machine| {
                apply_specializations
                    .iter()
                    .any(|specialization| specialization.instance == machine.symbol)
            })
            .count(),
        2
    );
    let clock = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Clock")
        .expect("Clock boundary trait");
    for specialization in apply_specializations {
        let instance = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
            .expect("specialized machine");
        assert_eq!(instance.suspends_keyword_source_spans.len(), 1);
        assert_eq!(instance.blocks_keyword_source_spans.len(), 1);
        let rows = checked
            .authored_service_reach_rows_for(specialization.instance)
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            panic!(
                "specialization {:?} from {:?} retains {} authored reach rows",
                specialization.instance,
                specialization.template,
                rows.len()
            )
        };
        let [target] = row.targets.as_slice() else {
            panic!("each concrete specialization retains the authored Clock occurrence")
        };
        assert_eq!(target.service, clock.symbol);
        assert!(target.source_span.span.end > target.source_span.span.start);
    }
}

#[test]
fn attached_machine_specialization_clones_inherited_field_symbols() {
    let source = r#"
        data Console {}
        data Light [copy] { weight: i32; }
        data Main { console: Console; light: Light; number: i32; }

        machine Main::pick<T [copy]>(&self, value: &T) -> i32 { 7 }
        machine Main::run(&mut self) {
            let from_light: i32 = self.pick(&self.light);
            let from_number: i32 = self.pick(&self.number);
        }
    "#;
    let checked = checked_program(source);

    let pick = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::pick")
        .expect("pick template instance");
    let instances = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == pick.symbol)
        .map(|specialization| specialization.instance)
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    let main_symbol = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Main")
        .expect("Main data")
        .symbol;
    for instance in instances {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == instance)
            .expect("specialized pick machine");
        assert_eq!(
            machine.attached_data_symbol, main_symbol,
            "both the reused template and cloned specialization retain exact attached identity"
        );
        let fields = checked
            .symbols
            .child_handles(machine.symbol)
            .into_iter()
            .flatten()
            .filter(|symbol| checked.symbols.get(*symbol).kind == symbols::SymbolKind::Field)
            .map(|symbol| checked.symbols.name(symbol))
            .collect::<Vec<_>>();
        assert_eq!(fields, ["console", "light", "number"]);
    }
}

#[test]
fn forwarded_generic_calls_specialize_after_their_caller() {
    let source = r#"
        data Light [copy] { weight: i32; }
        data Main { light: Light; number: i32; }

        machine Main::copy_it<T [copy]>(&self, value: &T) {}
        machine Main::wrap<U [copy]>(&self, value: &U) {
            self.copy_it(value);
        }
        machine Main::run(&mut self) {
            self.copy_it(&self.light);
            self.copy_it(&self.number);
            self.wrap(&self.light);
        }
    "#;

    let checked = checked_program(source);

    let specialization_count = |name: &str| {
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template && machine.name.as_str() == name
                })
            })
            .count()
    };
    assert_eq!(specialization_count("Main::copy_it"), 2);
    assert_eq!(specialization_count("Main::wrap"), 1);
}

#[test]
fn concrete_specialization_must_satisfy_nominal_conformance_bound() {
    let source = r#"
        trait Marker {}
        data Good {}
        data Bad {}
        GoodMarker: Good satisfies Marker;

        machine accept<T>(value: &T)
        where T satisfies Marker
        {}

        machine caller(good: &Good, bad: &Bad) {
            accept(good);
            accept(bad);
        }
    "#;

    let diagnostics = checked_program_result(source)
        .expect_err("the Bad specialization has no authored nominal conformance");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("binds `T` to `Bad`, which has no nominal conformance to `Marker`")
    }));
}

#[test]
fn bounded_generic_call_specializes_to_concrete_attached_state() {
    let source = r#"
        trait Incrementable {
            machine increment(&mut self);
        }
        data Counter { value: i32 in Wrapping; }
        machine Counter::increment(&mut self) satisfies Incrementable::increment {
            self.value = self.value + 1;
        }
        CounterIncrementable: Counter satisfies Incrementable;

        machine step<T>(subject: &mut T)
        where T satisfies Incrementable
        {
            subject.increment();
        }
        data Main { counter: Counter; }
        machine Main::run(&mut self) {
            step(&mut self.counter);
        }
    "#;

    let checked = checked_program(source);

    let step = specialized_machine(&checked, "step");
    assert!(checked.machine_type_parameters(step).is_empty());
    assert!(step.conformance_bounds.is_empty());
    let state = checked
        .machine_states(step)
        .first()
        .expect("step entry state");
    let call = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            let typed_trees::statement::StatementNode::Call(call) = statement else {
                return None;
            };
            (call.target.as_str() == "increment").then_some(call)
        })
        .expect("bounded requirement call");
    assert_eq!(
        checked
            .statement_table
            .name_path_members(call.receiver)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["subject"]
    );

    let concrete_target = crate::lookup::resolve_state_call_target(
        &checked,
        step,
        state,
        call.receiver_symbol,
        call.target_symbol,
        crate::lookup::statement_call_receiver_members(&checked, call),
        &call.target,
    );
    let increment = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Counter::increment")
        .and_then(|machine| checked.machine_states(machine).first())
        .expect("Counter increment state");
    assert_eq!(call.target_symbol, increment.symbol);
    assert_eq!(concrete_target, increment.symbol);
}

#[test]
fn named_conformance_bound_rejects_a_different_concrete_carrier() {
    let source = r#"
        trait Marker {}
        data Good {}
        data Bad {}
        Primary: Good satisfies Marker;

        machine accept<T>(value: &T)
        where T satisfies Good::Primary
        {}

        machine caller(bad: &Bad) {
            accept(bad);
        }
    "#;

    let diagnostics =
        checked_program_result(source).expect_err("the selected conformance belongs only to Good");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("binds `T` to `Bad`, but named conformance `Good::Primary` belongs to `Good`")
    }));
}

#[test]
fn named_conformance_bound_rejects_a_name_owned_by_another_carrier() {
    let source = r#"
        trait Marker {}
        data Good {}
        data Bad {}
        Primary: Good satisfies Marker;

        machine accept<T>(value: &T)
        where T satisfies Bad::Primary
        {}

        machine caller(bad: &Bad) {
            accept(bad);
        }
    "#;

    let diagnostics = checked_program_result(source)
        .expect_err("the package-scoped conformance name still retains its carrier");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("names conformance `Bad::Primary`, but that declaration belongs to `Good`")
    }));
}

/// A concrete caller that resolves to a generic machine but cannot derive its
/// complete specialization tuple must reject: the statement-position call has
/// no result slot for the validation fence to inspect, so an underivable or
/// partial tuple would otherwise remain an unspecialized call to the generic
/// template. Zero-binding calls, partial explicit applications and conflicted
/// tuples all take the same honest rejection.
#[test]
fn underivable_generic_statement_calls_reject_instead_of_passing_unspecialized() {
    for source in [
        // No derivable evidence at all: the anonymous literal proposes
        // nothing, so `T` stays open.
        "machine pick<T>(value: T) {}
         machine main() -> u64 { pick(7); 7 }",
        // A partial explicit static tuple: `T` binds, `N` never does.
        "machine pair<T, const N: u64>(value: T) {}
         machine main() -> u64 { pair<i32>(7i32); 7 }",
        // Conflicting argument evidence can never complete one tuple.
        "machine choose<T>(first: T, second: T) {}
         machine main() -> u64 { choose(1i32, 2i64); 7 }",
        // A derived type beside an underivable sibling still cannot complete.
        "machine pair<T, U>(first: T, second: U) {}
         machine main() -> u64 { pair(7i32, 2); 7 }",
    ] {
        let diagnostics = checked_program_result(source)
            .expect_err("an underivable generic call must not pass unspecialized");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("cannot be derived") }),
            "{source}: {diagnostics:?}"
        );
    }
}

/// Statement-position calls that DO derive a complete tuple keep specializing:
/// a concrete argument carrier or a destination annotation still supplies the
/// tuple the selection gate requires.
#[test]
fn derivable_generic_statement_calls_still_specialize() {
    let source = r#"
        machine consume<T>(value: T) {}
        machine ident<T>(value: T) -> T { value }
        machine main() -> u64 {
            consume(7i32);
            let chosen: u64 = ident(9);
            chosen
        }
    "#;

    let checked = checked_program(source);
    assert_eq!(checked.machine_specializations.len(), 2);
}

/// A statement call whose only evidence lands through another specialization
/// in the same fixed point must not reject early: `consume`'s argument is the
/// still-generic `ident<u64>` call, so the first round proposes nothing for
/// `T`; once `ident<u64>` has specialized and its rewritten result type is
/// concrete, the tuple completes in the next round. The underivable-tuple
/// gate lives at the quiet point precisely so this late completion is not
/// condemned in the round where it was still symbolic.
#[test]
fn statement_call_completing_in_a_later_round_still_specializes() {
    let source = r#"
        machine consume<T>(value: T) {}
        machine ident<T>(value: T) -> T { value }
        machine main() -> u64 {
            consume(ident<u64>(9));
            7
        }
    "#;

    let checked = checked_program(source);
    assert_eq!(checked.machine_specializations.len(), 2);
}

/// An explicit builtin-type machine argument is complete static evidence on
/// its own: `consume<u64>` binds `T` even though `u64` never appears anywhere
/// else in the program — the unsuffixed `7` proposes no carrier and `i32` is
/// the only authored return. The static-argument materialization must intern
/// the builtin's named reference the same way an authored `-> u64` would, or
/// the tuple stays underivable.
#[test]
fn explicit_builtin_type_arguments_bind_without_an_ambient_carrier() {
    let source = r#"
        machine consume<T>(value: T) {}
        machine main() -> i32 {
            consume<u64>(7);
            7
        }
    "#;

    let checked = checked_program(source);
    assert_eq!(checked.machine_specializations.len(), 1);
}

/// The same late-round completion inside an attached `&mut self` machine:
/// `ident<u64>` is the only `u64` mention in the program, so this also pins
/// builtin-type argument materialization feeding the next round.
#[test]
fn attached_statement_call_completing_in_a_later_round_still_specializes() {
    let source = r#"
        machine consume<T>(value: T) {}
        machine ident<T>(value: T) -> T { value }
        data Main {}
        machine Main::run(&mut self) {
            consume(ident<u64>(9));
        }
    "#;

    let checked = checked_program(source);
    assert_eq!(checked.machine_specializations.len(), 2);
}

/// An attached generic method's `self` formal carries the machine's `Self`
/// alias, which expands to the retained owner application `Box<T>` for
/// binding. UFCS form pairs it with an ordinary argument; receiver syntax
/// excludes it from the argument list and the receiver place supplies the
/// evidence instead. Both must specialize rather than leave an
/// underivable-looking incomplete selection for the statement gate.
#[test]
fn attached_generic_self_evidence_specializes_statement_calls() {
    let source = r#"
        data Box<T> {
            value: T;
        }

        machine Box::settle<T>(self) {}
        machine Box::poke<T>(&mut self) {}

        machine main() -> u64 {
            let b: Box<i32> = Box { value: 1 };
            Box::settle(b);
            let mut c: Box<bool> = Box { value: true };
            c.poke();
            7
        }
    "#;

    let checked = checked_program(source);
    assert_eq!(checked.machine_specializations.len(), 2);
}
