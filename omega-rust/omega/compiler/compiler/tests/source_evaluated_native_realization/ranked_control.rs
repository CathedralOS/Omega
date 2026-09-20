//! A normalized foreign call inside a ranked machine (`terminates by`):
//! physical-evidence derivation itself is cycle-agnostic — its
//! survivor/physical-child bijection is occurrence-coordinate keyed — and
//! legalization now replays the call's source custody, so the surviving
//! boundary occurrence reaches native image emission. There the hosted
//! receiver bridge still refuses the ranked boundary call's contract, storage,
//! or entry custody, upstream of physical evidence. This test pins that exact
//! frontier: once the hosted entry preparation replays boundary calls inside
//! ranked control, this test must flip to the full survivor/child assertions
//! the acyclic cases already carry.

use super::{Fixture, realize_linux_dynamic_outcome};

/// `Main::spin` is a measured self cycle: `terminates by` admits it and the
/// Terminal machine retains its ranked SCC decomposition. The foreign call in
/// its leading block is a surviving boundary occurrence inside ranked
/// control.
#[test]
fn ranked_machine_foreign_call_stops_at_hosted_entry_preparation() {
    let fixture = Fixture::with_source(
        "ranked-foreign-caller",
        "linux_x86_64",
        r#"use omega::language::core::external_binding;

boundary trait Trace {
    machine record(value: u64);
}

linux_x86_64 machine record_binding() -> Binding<9, 6, 11> {
    Binding::DllImport {
        import: DllImport::ElfVersioned {
            object: "libc.so.6",
            symbol: "getpid",
            version: "GLIBC_2.2.5",
        },
    }
}

machine record_leaf(value: u64) satisfies Trace::record via record_binding();

data Main { }
machine Main::spin(&mut self, n: u64) terminates by n; reaches Trace {
    Trace::record(7);
    transition n == 0 {
        true -> done()
        _ -> self.spin(n - 1)
    }
    state done(&mut self) { }
}
machine Main::main(&mut self) {
    self.spin(3);
}
"#,
        r#"machine build(builder: &mut Build) {
    builder.application("ranked-foreign-caller");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
    );
    let retained = fixture.compile_terminal();
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("decode terminal module");
    let ranked_machines = module
        .machines
        .iter()
        .filter(|machine| machine.ranked_scc.is_some())
        .collect::<Vec<_>>();
    let [ranked_machine] = ranked_machines.as_slice() else {
        panic!("the fixture retains exactly one ranked machine")
    };
    let ranked_call = ranked_machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::BoundaryCall { .. } => Some(operation.id),
            _ => None,
        })
        .expect("the ranked machine carries the foreign boundary call");

    let (_, diagnostics) = realize_linux_dynamic_outcome(retained, 0x5241_4e4b_0001).expect_err(
        "a foreign call inside a ranked machine must still stop upstream of physical evidence",
    );
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("hosted receiver bridge lost exact contract, storage, or entry custody"),
        "the foreign call at machine {} operation {ranked_call} must stop at hosted entry preparation, not deeper: {rendered}",
        ranked_machine.id
    );
}
