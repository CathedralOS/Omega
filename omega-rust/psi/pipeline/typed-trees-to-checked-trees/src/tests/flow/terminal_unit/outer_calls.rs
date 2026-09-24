//! A call consumed through the structural value walk (a record or case
//! initializer holding a call node) still owns the structural calls nested
//! inside its own argument expressions: the outer-call roster unions those
//! operand calls instead of reporting them unconsumed.
use crate::tests::flow::terminal_unit::{checked, machine_named};
use checked_trees::CheckedUnitPlanOmissionStage;

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
    // the first, or arm calls read as unconsumed. The mixed fixture — a
    // runtime-index read arm beside a nested-call arm — composes, which is
    // itself the proof the call stayed owned.
    let checked = checked(TRANSITION_SOURCE);
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .composed_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
            .is_some(),
        "mixed arm machine declined: {:?}",
        plans.omission_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
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

const TRANSITION_LITERAL_INDEX_ARMS: &str = r#"
    data Dtr { tag: u64 }
    data Srfc { dtr: Dtr; count: u64 }
    data Srsr { collections: [Srfc; 4]; collection_count: u64 }

    machine Srsr::get_filter_collection(&self, collection_index: u64) -> Srfc {
        transition collection_index < self.collection_count {
            true -> (self.collections[0])
            false -> (self.collections[1])
        }
    }
"#;

const TRANSITION_BOUNDED_INDEX_ARMS: &str = r#"
    data Dtr { tag: u64 }
    data Srfc { dtr: Dtr; count: u64 }
    data Srsr { collections: [Srfc; 4]; collection_count: u64 }

    machine Srsr::get_filter_collection(&self, collection_index: u64 [0..=3]) -> Srfc {
        transition collection_index < self.collection_count {
            true -> (self.collections[collection_index])
            false -> (self.collections[0])
        }
    }
"#;

#[test]
fn transition_arm_literal_index_reads_compose() {
    // Literal fixed-index arms mint `Projection` values — the sibling shape
    // the runtime-index read joins at the checked tree.
    let checked = checked(TRANSITION_LITERAL_INDEX_ARMS);
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .composed_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
            .is_some(),
        "literal arms declined: {:?}",
        plans.omission_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
    );
}

#[test]
fn transition_arm_bounded_index_reads_compose() {
    // A declared `[min..=max]` parameter range does not change the admission:
    // the runtime index still mints `IndexedProjection` under the same role.
    let checked = checked(TRANSITION_BOUNDED_INDEX_ARMS);
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .composed_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
            .is_some(),
        "bounded arms declined: {:?}",
        plans.omission_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
    );
}

#[test]
fn transition_arm_runtime_index_reads_compose() {
    // `self.collections[collection_index]` mints an `IndexedProjection`
    // structural value — the runtime-index sibling of the `Projection` a
    // literal `[0]` arm already produced — and the guarded terminator
    // composes with it at the checked tree.
    for (label, source) in [
        ("read-arms", TRANSITION_READ_ARMS),
        ("mixed", TRANSITION_SOURCE),
    ] {
        let checked = checked(source);
        let plans = &checked.facts.flow.terminal_unit_effects;
        let composed = plans
            .composed_for_machine(machine_named(&checked, "Srsr::get_filter_collection"))
            .unwrap_or_else(|| {
                panic!(
                    "{label} declined: {:?}",
                    plans.omission_for_machine(machine_named(
                        &checked,
                        "Srsr::get_filter_collection"
                    ))
                )
            });
        let [state] = composed.states.as_slice() else {
            panic!("expected a single composed state");
        };
        let checked_trees::CheckedComposedUnitControlTerminatorPlan::Guarded {
            return_values, ..
        } = &state.terminator
        else {
            panic!("both `(value)` arms check as a guarded return terminator");
        };
        assert_eq!(return_values.len(), 2);
        let indexed = return_values
            .iter()
            .filter_map(|operation| {
                let checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    value,
                    ..
                } = operation
                else {
                    return None;
                };
                let root = checked.facts.values.structural_values.nodes.get(*value);
                matches!(
                    root.kind,
                    checked_trees::CheckedStructuralValueKind::IndexedProjection { .. }
                )
                .then_some(())
            })
            .count();
        assert_eq!(
            indexed, 1,
            "{label}: the runtime-index arm establishes an IndexedProjection"
        );
    }
}
