//! A call-free single-expression leaf whose loans all ride the structural
//! scalar-return owner's lanes stays with that owner and off the scalar
//! graph. `scalar_targets::available_target` refuses a machine registered in
//! both families, so a leaf in both would prune every ordinary caller.
use super::checked_program_result;
use checked_trees::CheckedTrees;

fn machine(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("machine {name}"))
        .symbol
}

#[test]
fn byte_view_leaves_keep_one_scalar_owner_for_their_callers() {
    for (callee, name, call) in [
        (
            "machine count(bytes: &[u8]) -> u64 { (bytes.len) }",
            "count",
            "count(&self.buf)",
        ),
        (
            "machine Probe::count(&self, bytes: &[u8]) -> u64 { (bytes.len) }",
            "Probe::count",
            "self.probe.count(&self.buf)",
        ),
    ] {
        let source = format!(
            r#"
            data Probe {{ base: u64; }}
            data Cell {{ value: u64; }}
            {callee}
            machine read(cell: &Cell) -> u64 {{ cell.value }}
            data Main {{ probe: Probe; cell: Cell; buf: [u8; 8]; length: u64; value: u64; }}
            machine Main::main(&mut self) {{
                self.buf[0] = 1;
                let counted: u64 = {call};
                let read_value: u64 = read(&self.cell);
                self.length = counted;
                self.value = read_value;
            }}
            "#
        );
        let checked = checked_program_result(&source)
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let flow = &checked.facts.flow;
        let count = machine(&checked, name);
        assert_eq!(
            flow.terminal_scalar_graphs.declined_reason(count),
            Some(
                "machine has no source-independent checked scalar control plan (structural scalar-return owner)"
            ),
            "{name}: the byte view keeps its structural return owner"
        );
        assert!(
            flow.terminal_structural_scalar_returns
                .for_machine(count)
                .is_some(),
            "{name}: one structural scalar return"
        );
        // A borrowed record is not one of that owner's lanes; 5cb02a9c2c
        // moved such leaves onto the scalar graph.
        assert!(
            flow.terminal_scalar_graphs
                .for_machine(machine(&checked, "read"))
                .is_some(),
            "{name}: the borrowed record leaf stays a scalar graph"
        );
        assert!(
            flow.terminal_unit_effects
                .for_machine(machine(&checked, "Main::main"))
                .is_some(),
            "{name}: the caller reaches both leaves: {:?}",
            flow.terminal_unit_effects
                .omission_for_machine(machine(&checked, "Main::main"))
        );
    }
}
