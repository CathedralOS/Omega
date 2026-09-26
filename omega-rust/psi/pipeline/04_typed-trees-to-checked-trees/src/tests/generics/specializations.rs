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
