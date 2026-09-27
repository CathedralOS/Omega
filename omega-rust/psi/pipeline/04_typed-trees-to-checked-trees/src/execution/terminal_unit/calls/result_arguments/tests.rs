//! Shared call loans retain the original local's storage and ownership.

use crate::tests::front_end::{checked_program, checked_program_result};
use checked_trees::{CheckedStructuralAccess, CheckedUnitEffectOperationPlan};

fn source(mutable: bool) -> String {
    let binding = if mutable { "mut " } else { "" };
    let before = if mutable { "region.set_base(104);" } else { "" };
    let after = if mutable { "region.set_base(108);" } else { "" };
    format!(
        r#"
        data Region {{ base: u64; size: u64; }}
        machine Region::new(base: u64, size: u64) -> Region {{
            Region {{ base: base, size: size }}
        }}
        machine Region::clone(source: &Region) -> Region {{
            Region {{ base: source.base, size: source.size }}
        }}
        machine Region::set_base(&mut self, base: u64) {{ self.base = base; }}
        machine Region::get_base(&self) -> u64 {{ self.base }}
        data Main {{ copied_base: u64; current_base: u64; }}
        machine Main::main(&mut self) {{
            let {binding}region: Region = Region::new(100, 16);
            {before}
            let copied: Region = Region::clone(&region);
            {after}
            self.copied_base = copied.get_base();
            self.current_base = region.get_base();
        }}
        "#
    )
}

fn machine_named(checked: &checked_trees::CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with(name))
        .expect("source machine")
        .symbol
}

#[test]
fn shared_clone_observes_mutable_or_immutable_local_storage() {
    for mutable in [false, true] {
        let checked = checked_program(&source(mutable));
        let machine = machine_named(&checked, "main");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .unwrap_or_else(|| {
                panic!(
                    "shared clone plan, mutable={mutable}: {:?}",
                    checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .omission_for_machine(machine)
                )
            });
        let results: Vec<_> = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    structural_arguments,
                    result,
                    ..
                } => Some((structural_arguments, result)),
                _ => None,
            })
            .collect();
        let [
            (construction_arguments, constructed),
            (clone_arguments, copied),
        ] = results.as_slice()
        else {
            panic!("constructor and clone both retain structural results: {results:?}")
        };
        assert!(construction_arguments.is_empty());
        let [borrow] = clone_arguments.as_slice() else {
            panic!("clone has one structural source: {clone_arguments:?}")
        };
        assert_eq!(borrow.access, CheckedStructuralAccess::SharedBorrow);
        assert!(borrow.path.is_empty());
        assert_eq!(
            borrow.source_structural_result_binding_ordinal(),
            Some(constructed.binding_ordinal)
        );
        assert_ne!(constructed.binding_ordinal, copied.binding_ordinal);
    }
}

#[test]
fn mutable_local_shared_clone_requires_captured_read_access() {
    let checked = checked_program(&source(true));
    let machine = machine_named(&checked, "main");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("mutable shared clone plan");
    let coordinate = plan
        .operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                structural_arguments,
                ..
            } if structural_arguments
                .iter()
                .any(|argument| argument.access == CheckedStructuralAccess::SharedBorrow) =>
            {
                Some(*coordinate)
            }
            _ => None,
        })
        .expect("clone coordinate");
    let borrow_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == machine && state.state_symbol == plan.state)
        .expect("caller borrow state");
    let call = checked
        .facts
        .borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .find(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
        })
        .expect("captured clone access");
    let access = call.accesses.start();
    assert_eq!(
        checked.facts.borrow.argument_accesses.get(access).kind,
        checked_trees::BorrowAccessKind::Read
    );
    let mut facts = checked.facts.clone();
    facts.borrow.argument_accesses.get_mut(access).root_symbol = symbols::SymbolHandle::invalid();
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(rebuilt.for_machine(machine).is_none());
}

#[test]
fn shared_local_loan_does_not_authorize_conflicting_mutation() {
    let source = r#"
        data Region { base: u64; }
        machine Region::new() -> Region { Region { base: 100 } }
        machine Region::set_base(&mut self, base: u64) { self.base = base; }
        machine read(region: &Region) -> u64 { region.base }
        machine exercise() -> u64 {
            let mut region: Region = Region::new();
            let held: &Region = &region;
            region.set_base(104);
            read(held)
        }
    "#;
    let diagnostics = checked_program_result(source).expect_err("live shared loan conflicts");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("while local borrow `held` is still active")),
        "expected conflicting loan diagnostic: {diagnostics:?}"
    );
}
