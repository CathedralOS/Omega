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
