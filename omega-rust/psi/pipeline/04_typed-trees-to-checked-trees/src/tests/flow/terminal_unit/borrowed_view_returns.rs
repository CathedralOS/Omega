//! Borrowed `&[u8]`/`&[T]` view results establish their reference from a
//! parameter-carrier subslice instead of a retained structural root.
use crate::tests::flow::terminal_unit::{checked, machine_named};
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

fn view_plan<'a>(
    checked: &'a checked_trees::CheckedTrees,
    name: &str,
) -> &'a CheckedUnitEffectOperationPlan {
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(checked, name))
        .expect("borrowed view return composes a Unit plan");
    let CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } =
        &plan.states[0].terminator
    else {
        panic!("guarded return tail");
    };
    &return_values[0]
}

#[test]
fn member_subslice_return_establishes_borrowed_element_view() {
    let checked = checked(
        r#"
        data Cand [copy] { addr: u64; }
        data V { cands: [Cand; 8]; heap_count: u64; }
        machine V::get_heap_candidates(&self, n: u64) -> &[Cand] {
            transition (n <= self.cands.len) {
                true -> (self.cands[0..n])
                false -> (self.cands[0..8])
            }
        }
    "#,
    );
    let CheckedUnitEffectOperationPlan::EstablishReference { result, source } =
        view_plan(&checked, "V::get_heap_candidates")
    else {
        panic!("borrowed view return is an EstablishReference producer");
    };
    assert_eq!(result.statement_index, 0);
    assert_eq!(result.binding_ordinal, 0);
    let CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
        root:
            checked_trees::CheckedStorageRoot::Parameter {
                index: parameter_index,
            },
        expression: _,
        start: Some(_),
        end: Some(_),
    } = source.source
    else {
        panic!("member subslice keeps its exclusive evaluated endpoints");
    };
    // The owning carrier is `self`, the record parameter, not the field.
    assert_eq!(parameter_index, 0);
    assert_eq!(
        source.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    assert!(source.path.is_empty());
}

#[test]
fn member_subslice_return_establishes_borrowed_byte_view() {
    let checked = checked(
        r#"
        data V { name: [u8; 64]; name_len: u64; }
        machine V::get_name(&self, n: u64) -> &[u8] {
            transition (n <= self.name.len) {
                true -> (self.name[0..n])
                false -> (self.name[0..64])
            }
        }
    "#,
    );
    let CheckedUnitEffectOperationPlan::EstablishReference { source, .. } =
        view_plan(&checked, "V::get_name")
    else {
        panic!("borrowed byte view return is an EstablishReference producer");
    };
    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
        start: Some(_),
        end: Some(_),
        ..
    } = source.source
    else {
        panic!("member byte subslice keeps its exclusive evaluated endpoints");
    };
}

#[test]
fn mutable_view_results_stay_outside_borrowed_view_custody() {
    let checked = checked(
        r#"
        data Cand [copy] { addr: u64; }
        data V { cands: [Cand; 8]; }
        machine V::get_mut(&mut self, n: u64) -> &mut [Cand] {
            transition (n <= self.cands.len) {
                true -> (self.cands[0..n])
                false -> (self.cands[0..8])
            }
        }
    "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine_named(&checked, "V::get_mut"))
            .is_none(),
        "a `&mut [T]` result stays with the owned reference-result family"
    );
}

#[test]
fn member_read_endpoints_lower_in_the_structural_namespace() {
    // `self.heap_count`/`self.cands.len` endpoints lower as structural
    // parameter reads — the same vocabulary the guard itself uses.
    let checked = checked(
        r#"
        data Cand [copy] { addr: u64; }
        data V { cands: [Cand; 8]; heap_count: u64; }
        machine V::get_member_end(&self) -> &[Cand] {
            transition (self.heap_count <= self.cands.len) {
                true -> (self.cands[0..self.heap_count])
                false -> (self.cands[0..self.cands.len])
            }
        }
    "#,
    );
    let CheckedUnitEffectOperationPlan::EstablishReference { source, .. } =
        view_plan(&checked, "V::get_member_end")
    else {
        panic!("member-read endpoints still mint the borrowed reference");
    };
    let CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
        start: Some(_),
        end: Some(_),
        ..
    } = source.source
    else {
        panic!("member-read endpoints retain evaluated expressions");
    };
}
