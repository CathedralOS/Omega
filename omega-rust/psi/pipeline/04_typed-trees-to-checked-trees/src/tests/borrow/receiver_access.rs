use crate::tests::front_end::checked_program_result;

fn reject_source(source: &str) -> Vec<diagnostics::Diagnostic> {
    // Stop at source checking: absence of a Terminal executable plan cannot
    // establish rejection of an unauthorized receiver access.
    let diagnostics = match checked_program_result(source) {
        Ok(_) => panic!("receiver access must fail source checking"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        !diagnostics.is_empty(),
        "rejection must report a diagnostic"
    );
    diagnostics
}

#[test]
fn shared_receiver_rejects_overlapping_exclusive_argument() {
    for caller in [
        "machine invoke(value: &mut Pair) -> u64 { value.inspect(&mut value) }",
        "machine invoke(input: u64) -> u64 {
            let value: Pair = Pair { left: input, right: 7 };
            value.inspect(&mut value)
        }",
    ] {
        let diagnostics = reject_source(&format!(
            "data Pair {{ left: u64; right: u64; }}
         machine Pair::inspect(&self, other: &mut Pair) -> u64 {{ self.right }}
         {caller}",
        ));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "receives shared receiver overlapping another argument in the same call"
                )),
            "exact receiver/argument incompatibility: {diagnostics:#?}"
        );
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("LET-bound"))
        );
    }
}

#[test]
fn local_record_receiver_cannot_acquire_unauthorized_mutation() {
    for body in [
        "let cell: Cell = Cell { value: 17 }; cell.replace();",
        "let mut cell: Cell = Cell { value: 17 }; let view: &Cell = &cell; view.replace();",
    ] {
        let source = format!(
            "data Cell {{ value: u64; }}
             machine Cell::replace(&mut self) {{ self.value = 29; }}
             machine observe() {{ {body} }}"
        );
        reject_source(&source);
    }
}

fn machine_named(checked: &checked_trees::CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == name || machine.name.as_str().ends_with(&format!("::{name}"))
        })
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

#[test]
fn dynamic_indexed_parameter_receiver_can_supply_shared_receiver() {
    let checked = checked_program_result(
        "data Cell { value: u64; }
         machine Cell::get(&self) -> u64 { self.value }
         machine run(cells: &[Cell; 2], i: u64 [0..=1]) -> u64 { cells[i].get() }",
    )
    .expect("a bounded runtime-indexed parameter receiver must check");
    let run = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(run)
        .unwrap_or_else(|| panic!("the caller keeps a Unit plan for a runtime-indexed receiver"));
    let calls = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    let [receiver] = calls.as_slice() else {
        panic!(
            "the runtime-indexed receiver call passes exactly one structural argument: {calls:#?}"
        )
    };
    // `i` is the first dense scalar parameter of `run` (the borrowed `cells`
    // array occupies no scalar position), so the retained selector is 0. Its
    // published `0..=1` entry range admits the receiver; the segment restates
    // no interval, since Terminal re-proves the bound at the call.
    assert!(
        receiver.access == checked_trees::CheckedStructuralAccess::SharedBorrow
            && receiver.path.as_slice()
                == [
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::Parameter { position: 0 }
                    )
                ],
        "the receiver argument lends the bounded runtime-indexed element shared: {receiver:#?}"
    );
}

#[test]
fn explicit_shared_dynamic_indexed_argument_lends_the_element_in_call_operation() {
    let checked = checked_program_result(
        "data Cell { value: u64; }
         data Rack { cells: [Cell; 2]; }
         machine take(view: &Cell) -> u64 { view.value }
         machine Rack::run(&mut self, i: u64 [0..=1]) -> u64 { take(&self.cells[i]) }",
    )
    .unwrap_or_else(|diagnostics| panic!("source still checks: {diagnostics:#?}"));
    let run = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(run)
        .unwrap_or_else(|| {
            panic!(
                "a shared subloan of a runtime-indexed element is planned: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(run)
            )
        });
    let calls = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    let [argument] = calls.as_slice() else {
        panic!("the indexed element call passes one structural argument: {calls:#?}")
    };
    // `i` is the first dense scalar parameter of `run`, so the segment
    // selects by parameter position 0; Terminal re-proves its bound.
    assert!(
        argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow
            && argument.path.as_slice()
                == [
                    checked_trees::CheckedUnitStructuralPathSegment::Field("cells".into()),
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::Parameter { position: 0 }
                    ),
                ],
        "the affine receiver lends its runtime-indexed element shared: {argument:#?}"
    );
}
