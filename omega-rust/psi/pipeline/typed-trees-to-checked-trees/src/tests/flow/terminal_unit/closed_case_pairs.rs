//! `transition self.<enum-field> { CaseA {..} -> t(..) CaseB {..} -> u() }`
//! where both cases of a closed two-variant sum are authored and no `_` arm
//! remains. The authored pair is an exact complement — exactly one variant
//! holds — so the second arm is the false fallback the source legitimately
//! omits and the tail composes as a conditional dispatch. Transferring a
//! `[copy]` payload subtree out of the selected case (`t(info)`) stays
//! unbound: no edge-local structural payload channel exists yet.
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
fn state_graph_case_pair_payload_field_transfer_is_the_residual() {
    // `have(info)` asks the edge to transfer `self.result.Opened.info` — a
    // `[copy]` subtree inside the selected case of a borrowed field sum. The
    // complement admits the dispatch; the transfer stays unbound until a
    // structural case-payload channel exists.
    let checked = checked(
        r#"
        pub boundary trait Host { machine exit(code: i32); }
        pub data Info [copy] { pid: u32; }
        pub data OpenResult { case Opened(info: Info); case Failed(error: Info); }
        machine OpenResult::make() -> OpenResult {
            OpenResult::Opened { info: Info { pid: 7 } }
        }
        pub data Root { result: OpenResult; }
        machine Root::run(&mut self) reaches Host {
            self.result = OpenResult::make();
            transition self.result {
                OpenResult::Opened { info } -> have(info)
                OpenResult::Failed { error } -> failed()
            }
            state have(&mut self, info: Info) { Host::exit(70); }
            state failed(&mut self) { Host::exit(71); }
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
                "the payload transfer is the residual wall: {phase}"
            );
            assert_eq!(*state_index, Some(0));
            assert_eq!(*statement_index, Some(3));
        }
        other => panic!("the payload transfer reports a local-construction omission: {other:?}"),
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
