//! `transition self.<enum-field> { CaseA {..} -> t(..) CaseB {..} -> u() }`
//! where both cases of a closed two-variant sum are authored and no `_` arm
//! remains. The authored pair is an exact complement — exactly one variant
//! holds — so the second arm is the false fallback the source legitimately
//! omits and the tail composes as a conditional dispatch. A `[copy]` payload
//! subtree transfers out of the selected case (`t(info)`) through the edge's
//! proven membership: the minted transfer carries the tested subject's own
//! structural argument plan plus the selected case and payload field
//! identities.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

#[test]
fn state_graph_composes_a_case_pair_inside_a_named_state() {
    // The z6 open_target shape: `transition self.<field>` lives inside a
    // declared `state` block, so `self` is authored on that state — its
    // receiver parameter is a different declaration than the entry state's.
    // The self rejoin scopes to whichever state owns the parameter list.
    let checked = checked(
        r#"
        pub data Info [copy] { pid: u32; }
        pub data OpenResult { case Opened(info: Info); case Failed(error: Info); }
        pub data Root { result: OpenResult; hit: i32; }
        machine Root::run(&mut self) {
            state open(&mut self) {
                transition self.result {
                    OpenResult::Opened { info } -> have(info)
                    OpenResult::Failed { error } -> failed()
                }
            }
            state have(&mut self, info: Info) { self.hit = 70; }
            state failed(&mut self) { self.hit = 71; }
        }
        "#,
    );
    let machine = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the named-state case pair composes: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    let [entry, open, have, failed] = plan.states.as_slice() else {
        panic!(
            "the named-state case pair keeps the entry, its state, and both leaves: {:?}",
            plan.states.len()
        )
    };
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &open.terminator
    else {
        panic!(
            "the named-state two-case complement mints a conditional terminator: {:?}",
            open.terminator
        )
    };
    assert_eq!(when_true.target_state, have.state);
    assert_eq!(when_false.target_state, failed.state);
    let [receiver, transfer] = when_true.transfers.as_slice() else {
        panic!(
            "the payload edge mints the receiver plus payload transfers: {:?}",
            when_true.transfers
        )
    };
    assert!(matches!(
        receiver.source,
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
    ));
    assert_eq!(transfer.target_parameter_index, 1);
    let checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
        subject,
        case_identity,
        field_identity,
        ..
    } = &transfer.source
    else {
        panic!(
            "the payload argument mints a CasePayload transfer: {:?}",
            transfer.source
        )
    };
    assert_eq!(case_identity, "Opened");
    assert_eq!(field_identity, "info");
    assert_eq!(
        subject.path,
        vec![checked_trees::CheckedUnitStructuralPathSegment::Field(
            "result".to_owned()
        )]
    );
    let _ = entry;
}
