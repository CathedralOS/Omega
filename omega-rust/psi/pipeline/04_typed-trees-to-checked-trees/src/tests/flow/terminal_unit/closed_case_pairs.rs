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

fn entry_terminator(
    checked: &checked_trees::CheckedTrees,
) -> &checked_trees::CheckedComposedUnitControlTerminatorPlan {
    let machine = machine_named(checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the exact two-case field subject composes: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let [entry, ..] = plan.states.as_slice() else {
        panic!("the case pair keeps the entry dispatch")
    };
    &entry.terminator
}

fn omission_phase(
    checked: &checked_trees::CheckedTrees,
) -> &checked_trees::CheckedUnitPlanOmission {
    let machine = machine_named(checked, "run");
    checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(machine)
        .unwrap_or_else(|| panic!("the fixture has an omission"))
}

#[test]
fn state_graph_composes_a_field_subject_case_pair_without_an_authored_fallback() {
    // The exact z6 shape: `transition self.result` over the two cases of a
    // closed sum with no authored `_` arm — the discriminant complement IS
    // the false fallback.
    let checked = checked(
        r#"
        pub data OpenResult { case Opened(pid: u32); case Failed(error: i32); }
        pub data Root { result: OpenResult; hit: i32; }
        machine Root::run(&mut self) {
            transition self.result {
                OpenResult::Opened { pid } -> have(pid)
                OpenResult::Failed { error } -> failed(error)
            }
            state have(&mut self, pid: u32) { self.hit = 1; }
            state failed(&mut self, error: i32) { self.hit = 2; }
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
                "the exact two-case field subject composes: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    let [entry, have, failed] = plan.states.as_slice() else {
        panic!("the case pair keeps the entry dispatch and both leaves")
    };
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!(
            "the two-case complement mints a conditional terminator: {:?}",
            entry.terminator
        )
    };
    assert_eq!(when_true.target_state, have.state);
    assert_eq!(when_false.target_state, failed.state);
}

#[test]
fn state_graph_composes_a_case_pair_when_payload_bindings_are_unused() {
    let checked = checked(
        r#"
        pub data OpenResult { case Opened; case Failed; }
        machine OpenResult::make() -> OpenResult { OpenResult::Opened }
        pub data Root { result: OpenResult; hit: i32; }
        machine Root::run(&mut self) {
            self.result = OpenResult::make();
            transition self.result {
                OpenResult::Opened -> have()
                OpenResult::Failed -> failed()
            }
            state have(&mut self) { self.hit = 70; }
            state failed(&mut self) { self.hit = 71; }
        }
        "#,
    );
    assert!(matches!(
        entry_terminator(&checked),
        checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
    ));
}

#[test]
fn state_graph_transfers_a_case_payload_field_into_the_target_state() {
    // `have(info)` asks the edge to transfer `self.result.Opened.info` — a
    // `[copy]` subtree inside the selected case of a borrowed field sum. The
    // edge's proven membership authorizes the projection, so the transfer
    // mints a CasePayload row carrying the tested subject's own plan.
    let checked = checked(
        r#"
        pub data Info [copy] { pid: u32; }
        pub data OpenResult { case Opened(info: Info); case Failed(error: Info); }
        pub data Root { result: OpenResult; hit: i32; }
        machine Root::run(&mut self) {
            transition self.result {
                OpenResult::Opened { info } -> have(info)
                OpenResult::Failed { error } -> failed()
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
                "the payload-bearing case pair composes: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    let [entry, have, failed] = plan.states.as_slice() else {
        panic!("the case pair keeps the entry dispatch and both leaves")
    };
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!("the two-case complement mints a conditional terminator")
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
        path,
    } = &transfer.source
    else {
        panic!(
            "the payload argument mints a CasePayload transfer: {:?}",
            transfer.source
        )
    };
    assert_eq!(case_identity, "Opened");
    assert_eq!(field_identity, "info");
    assert!(path.is_empty());
    assert!(matches!(
        subject.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    ));
    assert_eq!(
        subject.path,
        vec![checked_trees::CheckedUnitStructuralPathSegment::Field(
            "result".to_owned()
        )]
    );
    assert_eq!(
        subject.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    let [receiver] = when_false.transfers.as_slice() else {
        panic!("the empty-target edge mints only the receiver transfer")
    };
    assert!(matches!(
        receiver.source,
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
    ));
}

#[test]
fn state_graph_nested_case_payload_path_stays_a_transfer_residual() {
    // `have(info.inner)` reaches inside the payload field's own subtree:
    // only the whole declared field transfers today.
    let checked = checked(
        r#"
        pub data Inner [copy] { value: u32; }
        pub data Info [copy] { inner: Inner; }
        pub data OpenResult { case Opened(info: Info); case Failed(error: Info); }
        pub data Root { result: OpenResult; hit: i32; }
        machine Root::run(&mut self) {
            transition self.result {
                OpenResult::Opened { info } -> have(info.inner)
                OpenResult::Failed { error } -> failed()
            }
            state have(&mut self, inner: Inner) { self.hit = 70; }
            state failed(&mut self) { self.hit = 71; }
        }
        "#,
    );
    match &omission_phase(&checked).stage {
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
            phase,
            state_index,
            statement_index,
        } => {
            assert!(
                phase.contains("parameter transfer"),
                "a nested payload path still has no custody channel: {phase}"
            );
            assert_eq!(*state_index, Some(0));
            assert_eq!(*statement_index, Some(2));
        }
        other => panic!("the nested payload reports a local-construction omission: {other:?}"),
    }
}

#[test]
fn state_graph_keeps_an_authored_false_fallback_on_the_guarded_jumps_path() {
    // An authored `_` arm is real fallback evidence, not the synthesized
    // pair: three arms stay on the guarded-jumps terminator.
    let checked = checked(
        r#"
        pub boundary trait Host { machine exit(code: i32); }
        pub data OpenResult { case Opened; case Failed; }
        pub data Root { result: OpenResult; }
        machine Root::run(&mut self) reaches Host {
            transition self.result {
                OpenResult::Opened -> have()
                OpenResult::Failed -> failed()
                _ -> other()
            }
            state have(&mut self) { Host::exit(70); }
            state failed(&mut self) { Host::exit(71); }
            state other(&mut self) { Host::exit(72); }
        }
        "#,
    );
    assert!(matches!(
        entry_terminator(&checked),
        checked_trees::CheckedComposedUnitControlTerminatorPlan::GuardedJumps { .. }
    ));
}

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
