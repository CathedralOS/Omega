//! Attached calls through nested receiver fields and their cast arguments.
//!
//! The product lexer's `retain` state calls
//! `self.lexer.append_source_byte(value as u8)` with `value` bound from a
//! `ByteRead::Byte(value: i32 [0..=255])` payload. The nested receiver was
//! never the problem; the exact narrowing cast is a call-statement argument,
//! and validation only retained exact-cast evidence for expression, local,
//! assignment and transition statements, so the Unit call builder found
//! neither a computation root nor a bound row and omitted the machine.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

fn entry_source(entry: &str, parameter: &str, call: &str) -> String {
    format!(
        r#"
pub boundary trait Host {{
    machine exit(code: i32);
}}
pub data ByteRead {{ case Eof; case Byte(value: i32 [0..=255]); }}
pub data Lexer {{ last: u8; retained: bool; }}
pub machine Lexer::append_source_byte(&mut self, byte: u8) {{
    self.last = byte;
    self.retained = true;
}}
pub data Root {{ host: Host; lexer: Lexer; read: ByteRead; raw: i32; last: u8; }}
pub machine Root::append_direct(&mut self, byte: u8) {{
    self.last = byte;
}}
machine Root::run(&mut self) reaches Host {{
    {entry}
    state retain(&mut self, {parameter}) {{
        {call};
        transition self.lexer.retained {{
            true -> done()
            _ -> done()
        }}
    }}
    state done(&mut self) {{ self.host.exit(0); }}
}}
"#
    )
}

fn composed_plan_exists(
    source: &str,
) -> (bool, Option<checked_trees::CheckedUnitPlanOmissionStage>) {
    let checked = checked(source);
    let root = machine_named(&checked, "run");
    let plans = &checked.facts.flow.terminal_unit_effects;
    (
        plans
            .composed_machines
            .iter()
            .any(|plan| plan.machine == root),
        plans.omission_for_machine(root).map(|row| row.stage),
    )
}

#[test]
fn direct_and_nested_receiver_calls_with_plain_arguments_plan() {
    for call in [
        "self.append_direct(value)",
        "self.lexer.append_source_byte(value)",
    ] {
        assert_eq!(
            composed_plan_exists(&entry_source(
                "transition { _ -> retain(7) }",
                "value: u8",
                call
            )),
            (true, None),
            "{call}: a plain scalar argument plans through either receiver"
        );
    }
}

// The product shape: the payload bound `[0..=255]` travels into `retain`,
// validation retains the exact `i32 -> u8` cast fact for the call-statement
// argument, and the call plans with that computation.
#[test]
fn nested_receiver_call_with_payload_bounded_exact_cast_argument_plans() {
    for call in [
        "self.lexer.append_source_byte(value as u8)",
        "self.append_direct(value as u8)",
    ] {
        assert_eq!(
            composed_plan_exists(&entry_source(
                "transition self.read { ByteRead::Byte { value } -> retain(value) _ -> done() }",
                "value: i32",
                call
            )),
            (true, None),
            "{call}: the payload-bounded exact cast argument is retained evidence"
        );
    }
}

// Exact narrowing needs positive evidence that the value fits. An unbounded
// input must fail source checking, before Unit planning is considered.
#[test]
fn exact_cast_argument_without_positive_evidence_rejects_during_checking() {
    let source = entry_source(
        "transition { _ -> retain(self.raw) }",
        "value: i32",
        "self.lexer.append_source_byte(value as u8)",
    );
    let tokens = super::Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = super::parse_syntax_trees(&tokens).expect("parse");
    let resolved = super::resolve(super::ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = super::lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = crate::lower_typed_trees(typed)
        .expect_err("unproven narrowing is a checking error, not a Unit omission");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Exact integer cast")
                && diagnostic.message.contains("not provably representable")
        }),
        "{diagnostics:?}"
    );
}
