//! A `collection[0..collection.len]` completion re-borrows the carrier itself:
//! the whole view *is* the shared `&` loan, so the result names the carrier
//! parameter — or the stored `&` field's referent leaf for member projections —
//! with no subslice producer to lower. Partial subslices keep the derived-place
//! `ByteSequenceSubslice`/`ElementViewSubslice` sources.
use crate::tests::flow::terminal_unit::{checked, machine_named};
use checked_trees::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};

fn establish<'a>(
    checked: &'a checked_trees::CheckedTrees,
    name: &str,
) -> &'a checked_trees::CheckedUnitStructuralArgumentPlan {
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(checked, name))
        .expect("whole view return composes a Unit plan");
    plan.operations
        .iter()
        .find_map(|operation| {
            let CheckedUnitEffectOperationPlan::EstablishReference { source, .. } = operation
            else {
                return None;
            };
            Some(source)
        })
        .expect("whole view return establishes one reference")
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
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = source.source
    else {
        panic!("whole view resolves to the carrier parameter itself");
    };
    // `&self` reads no receiver storage, so `x` is the only structural formal.
    assert_eq!(parameter_index, 0);
    assert!(source.path.is_empty());
    assert_eq!(
        source.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
}

#[test]
fn member_whole_view_return_names_the_stored_view_referent() {
    let checked = checked(
        r#"
        data Wv<'r> { view: &'r [u8]; tag: u64 }
        machine Wv::head_all(&self) -> &'r [u8] {
            self.view[0..self.view.len]
        }
    "#,
    );
    let source = establish(&checked, "Wv::head_all");
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = source.source
    else {
        panic!("member whole view resolves to the receiver parameter");
    };
    assert_eq!(parameter_index, 0);
    assert_eq!(
        source.path,
        vec![
            CheckedUnitStructuralPathSegment::Field("view".to_string()),
            CheckedUnitStructuralPathSegment::Referent,
        ]
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
    // An owned carrier's `[0..len]` is a real subslice of its storage, not a
    // loan pass-through — the `&` carrier route above stays declined.
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
