//! Whole and partial slice returns use the same descriptor producer. The
//! resulting value retains exact backing and bounds rather than referring to
//! a private descriptor carrier. Local backing escape is checked downstream.
use crate::tests::flow::terminal_unit::{checked, machine_named};
use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan};

fn establish<'a>(
    checked: &'a checked_trees::CheckedTrees,
    name: &str,
) -> &'a checked_trees::CheckedUnitStructuralArgumentPlan {
    let machine = machine_named(checked, name);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let ordinary = plans
        .for_machine(machine)
        .into_iter()
        .flat_map(|plan| &plan.operations);
    let composed = plans
        .composed_for_machine(machine)
        .into_iter()
        .flat_map(|plan| &plan.states)
        .flat_map(|state| &state.operations);
    ordinary
        .chain(composed)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishViewSubslice { source, result } => {
                assert_eq!(
                    result.multiplicity,
                    language_semantics::Multiplicity::Unrestricted
                );
                Some(source)
            }
            CheckedUnitEffectOperationPlan::EstablishReference { .. } => {
                panic!("a slice descriptor must not become a pointer reference")
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "descriptor producer missing: {:?}",
                plans.omission_for_machine(machine)
            )
        })
}

#[test]
fn parameter_whole_view_return_names_the_carrier_parameter() {
    let checked = checked(
        r#"
        data Nv { tag: u64 }
        machine Nv::head_all(&self, x: &[u8]) -> &[u8] {
            x[0..x.len]
        }
    "#,
    );
    let source = establish(&checked, "Nv::head_all");
    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        root:
            checked_trees::CheckedStorageRoot::Parameter {
                index: parameter_index,
            },
        ..
    } = source.source
    else {
        panic!("whole view retains its exact parameter backing");
    };
    // The composed state retains its explicit receiver before the view.
    assert_eq!(parameter_index, 1);
    assert!(source.path.is_empty());
    assert_eq!(
        source.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
}

#[test]
fn owned_parameter_whole_view_keeps_the_derived_subslice_source() {
    let checked = checked(
        r#"
        data Nv { tag: u64 }
        machine Nv::head_owned(&self, x: [u8; 64]) -> &[u8] {
            x[0..x.len]
        }
    "#,
    );
    let source = establish(&checked, "Nv::head_owned");
    // A descriptor can be formed over owned storage within an activation;
    // returning it still owes the independent nonlocal-backing judgment.
    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. } = source.source else {
        panic!("owned carrier keeps its derived subslice source");
    };
    assert!(source.path.is_empty());
}

#[test]
fn partial_subslice_keeps_the_derived_subslice_source() {
    let checked = checked(
        r#"
        data Nv { tag: u64 }
        machine Nv::head_part(&self, x: [u8; 64]) -> &[u8] {
            x[0..32]
        }
    "#,
    );
    let source = establish(&checked, "Nv::head_part");
    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. } = source.source else {
        panic!("partial subslice keeps its derived subslice source");
    };
}
