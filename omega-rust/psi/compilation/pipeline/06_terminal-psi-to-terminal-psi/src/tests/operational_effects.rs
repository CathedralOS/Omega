//! Blocking is an interface promise and an independently acknowledged call.

use super::*;

const BLOCKING: &str = r#"
public machine may_wait() -> u32 blocks { 7 }
public machine wrapped() -> u32 blocks { block may_wait() }
machine main() crashes Trap {
    let answer: u32 = block wrapped();
    transition answer == 7 {
        true -> done()
        _ -> bad()
    }
    machine done() {}
    machine bad() crashes Trap { crash Trap; }
}
"#;

fn invocation(text: &str) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    elaborate(
        boundary_calls::typed(text),
        terminal_codec::TerminalPublication::Invocation,
    )
}

fn elaborate(
    typed: typed_trees::TypedTrees,
    publication: terminal_codec::TerminalPublication,
) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .unwrap()
        .symbol;
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    typed_trees_to_terminal_psi::elaborate_for(
        &frozen,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        publication,
    )
}

#[test]
fn an_acknowledged_blocking_call_preserves_its_returned_value() {
    let verdict = judge(invocation(BLOCKING)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

fn accepted(text: &str) -> Box<crate::AcceptedClosure> {
    match judge(invocation(text)).unwrap() {
        RouteVerdict::Accepted(accepted) => accepted,
        other => panic!("expected accepted: {other:?}"),
    }
}

fn violation(text: &str, expected: &str) {
    match judge(invocation(text)).unwrap() {
        RouteVerdict::Rejected { violations, .. } => assert!(
            violations
                .iter()
                .any(|violation| violation.message().contains(expected)),
            "{violations:?}"
        ),
        other => panic!("expected {expected}: {other:?}"),
    }
}

#[test]
fn permission_and_acknowledgement_are_independent() {
    violation(
        &BLOCKING.replace(
            "public machine wrapped() -> u32 blocks",
            "public machine wrapped() -> u32",
        ),
        "blocks ceiling is false",
    );
    violation(
        &BLOCKING.replace("{ block may_wait() }", "{ may_wait() }"),
        "requires a block acknowledgement",
    );
    violation(
        &BLOCKING.replace("{ block may_wait() }", "{ suspend may_wait() }"),
        "requires a block acknowledgement",
    );
    violation(
        &BLOCKING.replace(
            "machine main() crashes Trap",
            "machine main() blocks false crashes Trap",
        ),
        "blocks ceiling is false",
    );
}

#[test]
fn private_omission_infers_the_envelope() {
    let text = BLOCKING.replace(
        "public machine wrapped() -> u32 blocks",
        "machine wrapped() -> u32",
    );
    let accepted = accepted(&text);
    let root = &accepted.elaboration.module.machines[0];
    assert_eq!(root.declared_blocking, None);
    assert!(root.published_may_block);
    assert!(!root.published_may_suspend);
}

#[test]
fn redundant_markers_keep_the_result_and_create_no_crossing() {
    let text = BLOCKING.replace("block may_wait()", "suspend block may_wait()");
    let accepted = accepted(&text);
    assert!(
        accepted
            .elaboration
            .module
            .machines
            .iter()
            .all(|machine| !machine.published_may_suspend)
    );
    assert!(
        accepted
            .elaboration
            .module
            .operations()
            .all(|operation| operation.suspension_crossing.is_none())
    );
}

#[test]
fn a_real_suspension_requires_evidence_beyond_its_marker() {
    let text = BLOCKING
        .replace("blocks", "suspends")
        .replace("block ", "suspend ");
    match judge(invocation(&text)).unwrap() {
        RouteVerdict::FallBack(refused) => assert!(
            refused.detail.contains("continuation, carry and loan"),
            "{refused:?}"
        ),
        other => panic!("actual suspension remains unfinished: {other:?}"),
    }
    violation(
        &text.replace("{ suspend may_wait() }", "{ block may_wait() }"),
        "requires a suspend acknowledgement",
    );
}

#[test]
fn a_nested_suspend_marker_does_not_gain_a_position_from_argument_lifting() {
    violation(
        r#"
        machine identity(value: u32) -> u32 { value }
        machine main() -> u32 { identity(suspend identity(7)) }
    "#,
        "suspend-marked call",
    );
    accepted(
        r#"
        machine identity(value: u32) -> u32 { value }
        machine main() -> u32 { identity(block identity(7)) }
    "#,
    );
}

const LOOP: &str = r#"
public machine again(value: bool) blocks {
    transition value {
        true -> block again(false)
        _ -> done()
    }
    machine done() {}
}
machine main() { block again(true); }
"#;

#[test]
fn frame_reuse_keeps_the_called_interface() {
    let accepted = accepted(LOOP);
    assert!(accepted.elaboration.module.machines.iter().any(|machine| {
        machine
            .call_formations
            .iter()
            .any(|row| row.tail_callee.is_some())
    }));
    violation(
        &LOOP.replace("true -> block again(false)", "true -> again(false)"),
        "requires a block acknowledgement",
    );
}

#[test]
fn malformed_or_forged_operational_metadata_is_not_a_program_rejection() {
    let mut elaboration = invocation(BLOCKING).unwrap();
    let machine = &mut elaboration.module.machines[0];
    machine.call_formations.push(machine.call_formations[0]);
    let refused = crate::operational::judge(&elaboration).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
    assert!(refused.detail.contains("strictly ordered"));
    let mut elaboration = invocation(BLOCKING).unwrap();
    elaboration.module.machines[0].published_may_block = false;
    assert!(
        crate::operational::judge(&elaboration)
            .unwrap_err()
            .detail
            .contains("published operational contract")
    );
    let mut elaboration = invocation(BLOCKING).unwrap();
    let row = &mut elaboration.module.machines[0].call_formations;
    row.truncate(1);
    row[0].site = terminal_psi::CallSite::Operation(id(999));
    assert!(
        crate::operational::judge(&elaboration)
            .unwrap_err()
            .detail
            .contains("no canonical call")
    );
}

#[test]
fn the_kept_runtime_witness_checks_before_native_realization() {
    accepted(include_str!(
        "../../../../../../../tests/fixtures/run/calls/operational_contracts/main.omg"
    ));
}

#[test]
fn a_boundary_uses_its_published_envelope_even_without_a_waiting_body() {
    let source = r#"
        boundary trait Probe { machine wait() -> u32 reaches none blocks crashes none; }
        machine main() -> u32 { block Probe::wait() }
    "#;
    accepted(source);
    violation(
        &source.replace("block Probe", "Probe"),
        "requires a block acknowledgement",
    );
    violation(
        &source.replace("machine main() -> u32", "public machine main() -> u32"),
        "blocks ceiling is false",
    );
}

#[test]
fn an_implementation_keeps_its_own_bound_and_owes_the_requirement_bound() {
    fn witness(requirement: &str, supplied: &str, body: &str) -> RouteVerdict {
        let text = format!(
            r#"
            trait Probe {{ machine wait() -> u32 reaches none {requirement}; }}
            public machine helper() -> u32 blocks {{ 7 }}
            machine implementation() -> u32 satisfies Probe::wait {supplied} {{ {body} }}
        "#
        );
        let typed = boundary_calls::typed(&text);
        let owner = typed
            .traits()
            .iter()
            .find(|row| row.name.as_str() == "Probe")
            .unwrap()
            .symbol;
        let required = typed
            .trait_machine_signatures
            .iter()
            .find(|(_, row)| row.name.as_str() == "wait")
            .unwrap()
            .1
            .symbol;
        let implementation = typed
            .machines()
            .iter()
            .find(|row| row.name.as_str() == "implementation")
            .unwrap()
            .symbol;
        judge(typed_trees_to_terminal_psi::elaborate_witness(
            &typed,
            owner,
            required,
            &[],
            implementation,
        ))
        .unwrap()
    }
    assert!(matches!(
        witness("blocks", "", "7"),
        RouteVerdict::Accepted(_)
    ));
    for refused in [
        witness("", "blocks", "7"),
        witness("blocks", "blocks false", "block helper()"),
    ] {
        assert!(
            matches!(refused, RouteVerdict::Rejected { ref violations, .. }
            if violations.iter().any(|violation| violation.message().contains("blocks ceiling is false"))),
            "{refused:?}"
        );
    }
}

#[test]
fn a_call_with_no_normal_return_keeps_its_acknowledgement() {
    let text = LOOP.replace("_ -> done()", "_ -> block again(false)");
    let accepted = accepted(&text);
    assert!(
        accepted.elaboration.module.machines[0]
            .blocks
            .iter()
            .any(|block| matches!(
                block.terminator,
                terminal_psi::Terminator::InvokeNoNormal { .. }
            ))
    );
    violation(
        &text.replace("block again(true)", "again(true)"),
        "requires a block acknowledgement",
    );
}

#[test]
fn a_tail_call_keeps_its_conservative_service_interface_too() {
    let text = r#"
        boundary trait Output {}
        machine first(value: bool) reaches none {
            transition value { true -> second(false) _ -> done() }
            machine done() {}
        }
        public machine second(value: bool) reaches Output {
            transition value { true -> first(false) _ -> done() }
            machine done() {}
        }
        machine main() { first(true); }
    "#;
    match judge(invocation(text)).unwrap() {
        RouteVerdict::Rejected { exceeded, .. } => assert!(!exceeded.is_empty()),
        _ => panic!("tail jump must preserve the callee's service contract"),
    }
}

#[test]
fn canonical_roundtrip_keeps_bounds_and_source_acknowledgements() {
    let typed = super::build_purpose::frontier_of(BLOCKING, "machine build_entry() {}");
    let RouteVerdict::Accepted(accepted) = judge(elaborate(
        typed,
        terminal_codec::TerminalPublication::Portable,
    ))
    .unwrap() else {
        panic!("portable source must be accepted");
    };
    let module = &accepted.elaboration.module;
    let bytes = terminal_codec::encode_module(module).unwrap();
    let decoded = terminal_codec::decode_module(&bytes).unwrap();
    assert_eq!(&decoded, module);
    assert!(decoded.machines[0].published_may_block);
    assert!(
        decoded.machines[0]
            .call_formations
            .iter()
            .any(|row| row.acknowledgement.acknowledges_block)
    );
}
