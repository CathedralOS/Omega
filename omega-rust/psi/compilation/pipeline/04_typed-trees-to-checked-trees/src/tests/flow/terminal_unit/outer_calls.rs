//! A call consumed through the structural value walk (a record or case
//! initializer holding a call node) still owns the structural calls nested
//! inside its own argument expressions: the outer-call roster unions those
//! operand calls instead of reporting them unconsumed.
use crate::tests::flow::terminal_unit::{checked, machine_named};
use checked_trees::{CheckedStructuralAccess, CheckedUnitEffectOperationPlan};

const SOURCE: &str = r#"
    data Fmt { v: u64 }
    data Ct { w: u64 }
    data Avs { fmt: Fmt; ct: Ct }
    data Asc { has_value: bool; avs: Avs }

    machine Fmt::default() -> Fmt { Fmt { v: 0 } }
    machine Ct::default() -> Ct { Ct { w: 0 } }
    machine Avs::new(fmt: Fmt, ct: Ct) -> Avs {
        Avs { fmt: fmt, ct: ct }
    }

    machine Asc::nested() -> Asc {
        Asc { has_value: false, avs: Avs::new(Fmt::default(), Ct::default()) }
    }

    machine Asc::nested_let() -> Asc {
        let c: Asc = Asc { has_value: true, avs: Avs::new(Fmt::default(), Ct::default()) };
        c
    }
"#;

#[test]
fn constructor_field_call_consumes_its_nested_call_arguments() {
    let checked = checked(SOURCE);
    // `Avs::new` is consumed by the structural walk of the `Asc` record
    // literal; `Fmt::default` and `Ct::default` are its structural argument
    // calls and must not read as unconsumed nested calls.
    let plans = &checked.facts.flow.terminal_unit_effects;
    let nested = machine_named(&checked, "Asc::nested");
    assert!(
        plans.for_machine(nested).is_some() || plans.composed_for_machine(nested).is_some(),
        "constructor field calls own their nested argument calls: {:?}",
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine_named(&checked, "Asc::nested"))
    );
}

const TRANSITION_CALL_ARMS: &str = r#"
    data Dtr { tag: u64 }
    data Srfc { dtr: Dtr; count: u64 }
    data Srsr { collections: [Srfc; 4]; collection_count: u64 }

    machine Dtr::default() -> Dtr { Dtr { tag: 0 } }
    machine Srfc::empty(dtr: Dtr, count: u64) -> Srfc {
        Srfc { dtr: dtr, count: count }
    }
    machine Srsr::get_filter_collection(collection: &Srsr, collection_index: u64) -> Srfc {
        transition collection_index < collection.collection_count {
            true -> (Srfc::empty(Dtr::default(), 0))
            false -> (Srfc::empty(Dtr::default(), 1))
        }
    }
"#;

#[test]
fn transition_arm_call_arguments_establish_nested_calls() {
    // `Srfc::empty(Dtr::default(), 0)` inside a `(value)` arm plans the nested
    // `Dtr::default` as an ordinary structural call operation in the state's
    // operation stream — the same shape a `let` initializer produces — and
    // keeps the arm's own call on its value node.
    let checked = checked(TRANSITION_CALL_ARMS);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let composed = plans
        .composed_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
        .unwrap_or_else(|| {
            panic!(
                "nested-call arm declined: {:?}",
                plans.omission_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
            )
        });
    let [state] = composed.states.as_slice() else {
        panic!("expected a single composed state");
    };
    let established = state
        .operations
        .iter()
        .filter(|operation| {
            operation
                .call()
                .is_some_and(|call| !call.is_boundary() && call.structural_result().is_some())
        })
        .count();
    assert_eq!(
        established, 2,
        "each arm's `Dtr::default` operand is one established structural call"
    );
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } =
        &state.terminator
    else {
        panic!("both `(value)` arms check as a guarded return terminator");
    };
    assert_eq!(return_values.len(), 2);
    for operation in return_values {
        let checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            calls, ..
        } = operation
        else {
            panic!("arm values lower through their structural value producers");
        };
        assert_eq!(calls.len(), 1, "each arm keeps its `Srfc::empty` call");
    }
}

const CASE_FIELD_STORE: &str = r#"
    data F [copy] { case A; case B; }
    data X { len: u64; fmt: F }
    machine X::empty() -> X { X { len: 0, fmt: F::A } }
    machine X::set_fmt(target: &mut X, f: F) {
        target.fmt = f;
    }
    machine X::clone_fmt(target: &mut X, source: &X) {
        target.fmt = source.fmt;
    }
    machine X::clone_from(target: &mut X, source: &X) {
        target.len = source.len;
        target.fmt = source.fmt;
    }
    machine X::clone(source: &X) -> X {
        let mut copy: X = X::empty();
        X::clone_from(&mut copy, source);
        copy
    }
"#;

#[test]
fn case_field_member_store_carries_the_member_read() {
    let checked = checked(CASE_FIELD_STORE);
    let operations = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "X::clone_fmt"))
        .unwrap_or_else(|| {
            panic!(
                "clone_fmt omission: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine_named(&checked, "X::clone_fmt"))
            )
        })
        .operations
        .clone();
    let store = operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(store) => Some(store),
            _ => None,
        })
        .unwrap_or_else(|| panic!("member-read case field store: {operations:#?}"));
    assert!(matches!(
        store.value.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
    ));
    assert!(matches!(store.value.path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::Field(identity)]
            if identity == "fmt"));
    assert_eq!(store.value.access, CheckedStructuralAccess::SharedBorrow);
}

const EXPRESSION_STATEMENT_STATIC_VIEW_CALL: &str = r#"
    data Sizer { name: [u8; 64]; name_len: u64; }

    machine Sizer::suffix(sizer: &Sizer, place: u64) -> &[u8] {
        transition place <= sizer.name.len {
            true -> (sizer.name[0..place])
            false -> (sizer.name[0..64])
        }
    }

    machine format(rounded: u64, suffix: &[u8]) {
    }

    machine Sizer::render(sizer: &Sizer) {
        format(0, Sizer::suffix(sizer, 1));
    }
"#;

#[test]
fn expression_statement_static_borrowed_view_calls_consume_the_nested_call() {
    // The outer call must consume its nested static call's returned view;
    // declaration qualification contributes no implicit argument.
    let checked = checked(EXPRESSION_STATEMENT_STATIC_VIEW_CALL);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let render = machine_named(&checked, "Sizer::render");
    assert!(
        plans.for_machine(render).is_some() || plans.composed_for_machine(render).is_some(),
        "expression-statement nested static view call declined: {:?}\n  suffix: {:?}\n  format: {:?}",
        plans.omission_for_machine(render),
        plans.omission_for_machine(machine_named(&checked, "Sizer::suffix")),
        plans.omission_for_machine(machine_named(&checked, "format"))
    );
}

#[test]
fn mutable_parameter_can_supply_a_shared_record_result() {
    let checked = checked(
        r#"
        data Nv { tag: u64 }

        machine Nv::view(value: &mut Nv) -> &Nv {
            value
        }
    "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let view = machine_named(&checked, "Nv::view");
    assert!(
        plans.for_machine(view).is_some() || plans.composed_for_machine(view).is_some(),
        "shared result from mutable parameter declined: {:?}",
        plans.omission_for_machine(view),
    );
}

#[test]
fn generic_static_parameter_return_named_view_composes() {
    // The generic application and lifetime belong to an ordinary parameter,
    // not to an attached-machine alias.
    let checked = checked(
        r#"
        data Nv<'r> { tag: &'r u64 }

        machine Nv::view<'r>(value: &'r Nv<'r>) -> &'r Nv<'r> {
            value
        }
    "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let view = machine_named(&checked, "Nv::view");
    assert!(
        plans.for_machine(view).is_some() || plans.composed_for_machine(view).is_some(),
        "generic static named-view return declined: {:?}",
        plans.omission_for_machine(view)
    );
}

const CALLER_FORWARDED_NAMED_VIEW: &str = r#"
    data Nv { tag: u64 }

    machine Nv::view(value: &Nv) -> &Nv {
        value
    }

    machine consume(v: &Nv) {
    }

    machine Nv::render(value: &Nv) {
        let v: &Nv = Nv::view(value);
        consume(v);
    }
"#;

#[test]
fn caller_forwarded_named_view_result_names_the_call_binding() {
    // Forwarding a call-produced reference retains the local's exact
    // reference identity and backing rather than reacquiring its source.
    let checked = checked(CALLER_FORWARDED_NAMED_VIEW);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let render = machine_named(&checked, "Nv::render");
    assert!(
        plans.for_machine(render).is_some() || plans.composed_for_machine(render).is_some(),
        "caller-forwarded `&'a V` named view declined: {:?}\n  view: {:?}\n  consume: {:?}",
        plans.omission_for_machine(render),
        plans.omission_for_machine(machine_named(&checked, "Nv::view")),
        plans.omission_for_machine(machine_named(&checked, "consume"))
    );
}
