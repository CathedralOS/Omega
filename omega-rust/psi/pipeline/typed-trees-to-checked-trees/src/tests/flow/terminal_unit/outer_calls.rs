//! A call consumed through the structural value walk (a record or case
//! initializer holding a call node) still owns the structural calls nested
//! inside its own argument expressions: the outer-call roster unions those
//! operand calls instead of reporting them unconsumed.
use crate::tests::flow::terminal_unit::{checked, machine_named};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitPlanOmissionStage,
};

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

fn omission(checked: &checked_trees::CheckedTrees, name: &str) -> &'static str {
    let row = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(machine_named(checked, name))
        .unwrap_or_else(|| panic!("{name} still reports a construction omission"));
    let CheckedUnitPlanOmissionStage::LocalConstruction { phase, .. } = row.stage else {
        panic!("{name} omits at a post-construction stage");
    };
    phase
}

#[test]
fn constructor_field_call_consumes_its_nested_call_arguments() {
    let checked = checked(SOURCE);
    // `Avs::new` is consumed by the structural walk of the `Asc` record
    // literal; `Fmt::default` and `Ct::default` are its structural argument
    // calls and must not read as unconsumed nested calls.
    assert!(
        !omission(&checked, "Asc::nested").starts_with("outer calls"),
        "constructor field calls own their nested argument calls: {:?}",
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine_named(&checked, "Asc::nested"))
    );
}

#[test]
fn local_initializer_field_call_consumes_its_nested_call_arguments() {
    let checked = checked(SOURCE);
    assert!(
        !omission(&checked, "Asc::nested_let").starts_with("outer calls"),
        "local initializer field calls own their nested argument calls: {:?}",
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine_named(&checked, "Asc::nested_let"))
    );
}

const TRANSITION_SOURCE: &str = r#"
    data Dtr { tag: u64 }
    data Srfc { dtr: Dtr; count: u64 }
    data Srsr { collections: [Srfc; 4]; collection_count: u64 }

    machine Dtr::default() -> Dtr { Dtr { tag: 0 } }
    machine Srfc::empty(dtr: Dtr, count: u64) -> Srfc {
        Srfc { dtr: dtr, count: count }
    }

    machine Srsr::get_filter_collection(&self, collection_index: u64) -> Srfc {
        transition collection_index < self.collection_count && collection_index < 4 {
            true -> (self.collections[collection_index])
            false -> (Srfc::empty(Dtr::default(), 0))
        }
    }
"#;

#[test]
fn transition_arm_values_own_their_calls() {
    // A bool-guarded transition mints one structural root per `(value)` arm;
    // the outer-call walk must visit every root at the statement, not only
    // the first, or arm calls read as unconsumed.
    let checked = checked(TRANSITION_SOURCE);
    assert!(
        !omission(&checked, "Srsr::get_filter_collection").starts_with("outer calls"),
        "transition arm values own their calls: {:?}",
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
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
    machine Srsr::get_filter_collection(&self, collection_index: u64) -> Srfc {
        transition collection_index < self.collection_count {
            true -> (Srfc::empty(Dtr::default(), 0))
            false -> (Srfc::empty(Dtr::default(), 1))
        }
    }
"#;

const TRANSITION_READ_ARMS: &str = r#"
    data Dtr { tag: u64 }
    data Srfc { dtr: Dtr; count: u64 }
    data Srsr { collections: [Srfc; 4]; collection_count: u64 }

    machine Dtr::default() -> Dtr { Dtr { tag: 0 } }
    machine Srfc::empty(dtr: Dtr, count: u64) -> Srfc {
        Srfc { dtr: dtr, count: count }
    }
    machine Srsr::get_filter_collection(&self, collection_index: u64) -> Srfc {
        transition collection_index < self.collection_count && collection_index < 4 {
            true -> (self.collections[collection_index])
            false -> (self.collections[0])
        }
    }
"#;

const TRANSITION_LITERAL_ARMS: &str = r#"
    data Dtr { tag: u64 }
    data Srfc { dtr: Dtr; count: u64 }
    data Srsr { collections: [Srfc; 4]; collection_count: u64 }

    machine Srsr::get_filter_collection(&self, collection_index: u64, fallback: Srfc) -> Srfc {
        transition collection_index < self.collection_count {
            true -> (Srfc { dtr: fallback.dtr, count: 1 })
            false -> (fallback)
        }
    }
"#;

const TRANSITION_ENUM_ARMS: &str = r#"
    data Region { case Empty; case Full(count: u64); }
    data Holder { flag: u64 }

    machine Holder::pick(&self, x: u64, r: Region) -> Region {
        transition x < self.flag {
            true -> (Region::Empty)
            false -> (r)
        }
    }
"#;

const TRANSITION_SIMPLE_CALL_ARMS: &str = r#"
    data Dtr { tag: u64 }
    data Srfc { dtr: Dtr; count: u64 }
    data Srsr { flag: u64 }

    machine Srfc::empty(dtr: Dtr, count: u64) -> Srfc {
        Srfc { dtr: dtr, count: count }
    }
    machine Srsr::pick(&self, x: u64, d: Dtr) -> Srfc {
        transition x < 4 {
            true -> (Srfc::empty(d, 1))
            false -> (Srfc::empty(d, 2))
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
            matches!(
                operation,
                checked_trees::CheckedUnitEffectOperationPlan::StructuralCall { .. }
            )
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

#[test]
fn transition_arm_value_shapes_still_compose() {
    // Arm shapes already admitted keep their plans: literal fields, enum
    // cases, and call arguments that are parameters mint no operand calls.
    for (label, source, name) in [
        (
            "literal-arms",
            TRANSITION_LITERAL_ARMS,
            "Srsr::get_filter_collection",
        ),
        ("enum-arms", TRANSITION_ENUM_ARMS, "Holder::pick"),
        (
            "simple-call-arms",
            TRANSITION_SIMPLE_CALL_ARMS,
            "Srsr::pick",
        ),
    ] {
        let checked = checked(source);
        let plans = &checked.facts.flow.terminal_unit_effects;
        assert!(
            plans
                .composed_for_machine(machine_named(&checked, name))
                .is_some(),
            "{label} declined: {:?}",
            plans.omission_for_machine(machine_named(&checked, name))
        );
    }
}

const CONDITIONAL_CALL_TAIL: &str = r#"
    data Text { len: u64 }

    machine Text::make(len: u64) -> Text { Text { len: len } }
    machine Text::matches(t: &Text, expected: &[u8], position: u64) -> bool {
        t.len == position
    }

    machine Text::consistent() -> bool {
        let a: Text = Text::make(0);
        let b: Text = Text::make(512);
        Text::matches(&a, "0B", 0) && Text::matches(&b, "1B", 0)
    }
"#;

const CASE_FIELD_STORE: &str = r#"
    data F [copy] { case A; case B; }
    data X { len: u64; fmt: F }
    machine X::empty() -> X { X { len: 0, fmt: F::A } }
    machine X::set_fmt(&mut self, f: F) {
        self.fmt = f;
    }
    machine X::clone_fmt(&mut self, source: &X) {
        self.fmt = source.fmt;
    }
    machine X::clone_from(&mut self, source: &X) {
        self.len = source.len;
        self.fmt = source.fmt;
    }
    machine X::clone(source: &X) -> X {
        let mut copy: X = X::empty();
        copy.clone_from(source);
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

#[test]
fn case_field_member_store_keeps_whole_parameter_stores() {
    let checked = checked(CASE_FIELD_STORE);
    let operations = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "X::set_fmt"))
        .expect("whole-parameter case field store")
        .operations
        .clone();
    let store = operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(store) => Some(store),
            _ => None,
        })
        .unwrap_or_else(|| panic!("whole-parameter case field store: {operations:#?}"));
    assert!(store.value.path.is_empty());
    assert_eq!(store.value.access, CheckedStructuralAccess::Owned);
}

#[test]
fn case_field_member_store_unblocks_the_clone_chain() {
    let checked = checked(CASE_FIELD_STORE);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let clone = machine_named(&checked, "X::clone");
    assert!(
        plans.composed_for_machine(clone).is_some(),
        "clone composes once clone_from plans: {:?}",
        plans.omission_for_machine(clone)
    );
}

const CONDITIONAL_SCALAR_TAIL: &str = r#"
    machine p(x: u64) -> bool { x == 0 }
    machine consistent_scalar() -> bool {
        p(0) && p(1)
    }
"#;

#[test]
fn conditional_tail_scalar_calls_still_plan() {
    // Pure scalar calls under a short-circuit tail were already admitted;
    // this guards that nothing about the literal-argument path disturbed it.
    let checked = checked(CONDITIONAL_SCALAR_TAIL);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let machine = machine_named(&checked, "consistent_scalar");
    assert!(
        plans.for_machine(machine).is_some(),
        "pure scalar conditional tail declined: {:?}",
        plans.omission_for_machine(machine)
    );
}

#[test]
fn conditional_tail_calls_compose_their_literal_arguments() {
    // `matches(&a, "0B", 0)` under `&&` is a nested call of the return
    // computation, not an outer statement call: its `"0B"` actual is a
    // byte-sequence literal the computation carries as a borrowed view, the
    // same plan the statement route builds for `write("...")` sites. Once
    // the call node mints, ordinary consumption retires its call row.
    let checked = checked(CONDITIONAL_CALL_TAIL);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let machine = machine_named(&checked, "Text::consistent");
    assert!(
        plans.for_machine(machine).is_some(),
        "conditional literal-argument calls declined: {:?}",
        plans.omission_for_machine(machine)
    );
    let matches = machine_named(&checked, "Text::matches");
    let call_nodes = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter(|(_, node)| {
            matches!(
                &node.kind,
                checked_trees::CheckedScalarComputationKind::Call { target_machine, .. }
                    if *target_machine == matches
            )
        })
        .count();
    assert_eq!(
        call_nodes, 2,
        "each `matches` operand is one computation call"
    );
}

const EXPRESSION_STATEMENT_VIEW_CALL: &str = r#"
    machine suffix(place: u64, text: &[u8]) -> &[u8] {
        text
    }

    machine format(rounded: u64, suffix: &[u8]) {
    }

    machine render() {
        format(0, suffix(1, "B"));
    }
"#;

#[test]
fn expression_statement_borrowed_view_calls_consume_the_nested_call() {
    // `format(rounded, suffix(place))` as an expression statement nests a
    // callee whose `&[u8]` result is the borrowed-view family: the
    // outer-call roster must plan the anonymous result so the nested call
    // counts as consumed rather than reading as an unconsumed nested call.
    let checked = checked(EXPRESSION_STATEMENT_VIEW_CALL);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let render = machine_named(&checked, "render");
    assert!(
        plans.for_machine(render).is_some(),
        "expression-statement nested `&[u8]` call declined: {:?}\n  suffix: {:?}\n  format: {:?}",
        plans.omission_for_machine(render),
        plans.omission_for_machine(machine_named(&checked, "suffix")),
        plans.omission_for_machine(machine_named(&checked, "format"))
    );
}

const LET_BOUND_VIEW_CALL: &str = r#"
    data Snap { bytes: [u8; 64]; count: u64; }

    machine Snap::read_snapshot_region_values(&self, i: u64) -> &[u8] {
        transition i <= self.bytes.len {
            true -> (self.bytes[0..i])
            false -> (self.bytes[0..64])
        }
    }

    machine format(rounded: u64, suffix: &[u8]) {
    }

    machine Snap::render(&self) {
        let region: &[u8] = self.read_snapshot_region_values(3);
        format(0, region);
    }
"#;

#[test]
fn let_bound_borrowed_view_result_names_the_call_binding() {
    // A `let` local binding the `&[u8]` result of an attached `&self` callee:
    // the local materializes the anonymous call binding under a symbol so the
    // forwarded argument reads as a live borrowed view rather than an owned
    // local — the read_snapshot_region_values family.
    let checked = checked(LET_BOUND_VIEW_CALL);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let render = machine_named(&checked, "Snap::render");
    assert!(
        plans.for_machine(render).is_some() || plans.composed_for_machine(render).is_some(),
        "let-bound `&[u8]` call result declined: {:?}\n  read: {:?}\n  format: {:?}",
        plans.omission_for_machine(render),
        plans.omission_for_machine(machine_named(&checked, "Snap::read_snapshot_region_values")),
        plans.omission_for_machine(machine_named(&checked, "format"))
    );
}

#[test]
fn transition_arm_runtime_index_reads_remain_declined() {
    // `self.collections[collection_index]` is a runtime-indexed read the
    // composed guard retains no place for — that residual is tracked
    // separately; a literal-index arm also declines at the same gate until
    // guard-derived bounds carry integer ranges.
    for (label, source) in [
        ("read-arms", TRANSITION_READ_ARMS),
        ("mixed", TRANSITION_SOURCE),
    ] {
        let checked = checked(source);
        let plans = &checked.facts.flow.terminal_unit_effects;
        assert!(
            plans
                .composed_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
                .is_none(),
            "{label} unexpectedly composed",
        );
    }
}
