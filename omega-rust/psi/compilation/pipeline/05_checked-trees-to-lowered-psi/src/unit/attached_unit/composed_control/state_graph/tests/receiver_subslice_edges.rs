//! A receiver-bearing state's edge names its view subslice after the implicit
//! receiver formal: the authored argument list holds it one slot before its
//! parameter position, and the edge rejoins it at that authored slot.

use super::super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::admission;

#[test]
fn receiver_self_edge_rejoins_its_subslice_at_the_authored_argument() {
    let checked = crate::front_end::checked_program(
        "data Root { count: u32 in Wrapping; }
         machine Root::walk(&mut self, bytes: &[u8])
         terminates by bytes -> Slice::Length;
         {
             self.count = self.count + 1;
             transition bytes.len > 0 {
                 true -> walk(bytes[1..])
                 false -> done()
             }
             state done(&mut self) {}
         }",
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.states.len() == 2)
        .expect("the walk graph");
    admission::admit(&checked, plan)
        .expect("the self edge's subslice rejoins its authored argument");

    // The same subslice claimed for the receiver's slot names no authored
    // argument there.
    let mut changed = plan.clone();
    let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &mut changed.states[0].terminator
    else {
        panic!("a guarded self edge: {:?}", changed.states[0].terminator);
    };
    let transfer = when_true
        .transfers
        .iter_mut()
        .find(|transfer| {
            matches!(
                transfer.source,
                checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { .. }
            )
        })
        .expect("the edge transfers its subslice");
    transfer.target_parameter_index = 0;
    assert!(
        admission::admit(&checked, &changed).is_err(),
        "a subslice moved onto the receiver slot rejects"
    );
}
