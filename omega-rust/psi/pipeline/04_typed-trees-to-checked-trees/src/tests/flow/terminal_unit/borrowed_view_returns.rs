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
