//! Call loans retain the original local's storage and ownership.

use crate::tests::front_end::{checked_program, checked_program_result};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitCallResult, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralPathSegment,
};

#[test]
fn exclusive_whole_local_loans_use_captured_access_not_binding_mutability() {
    // Write-only formation separately requires a mutable-authority root.
    // A mutable loan of owned local storage does not require rebinding it.
    for (access, binding) in [("mut", ""), ("write", "mut ")] {
        let checked = checked_program(&format!(
            r#"
            data Record {{ value: i32; }}
            machine fill(output: &{access} Record) {{ output.value = 7; }}
            machine run() {{
                let {binding}record: Record;
                fill(&{access} record);
            }}
            "#
        ));
        let machine = machine_named(&checked, "run");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .unwrap_or_else(|| {
                panic!(
                    "&{access} whole local: {:?}",
                    checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .omission_for_machine(machine)
                )
            });
        let argument = plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::Call(call) => call.structural_arguments.first(),
                _ => None,
            })
            .expect("whole-local call argument");
        assert!(argument.path.is_empty());
        assert!(
            argument
                .source_structural_result_binding_ordinal()
                .is_some()
        );
        assert_eq!(
            argument.access,
            if access == "mut" {
                CheckedStructuralAccess::MutableBorrow
            } else {
                CheckedStructuralAccess::WriteOnlyBorrow
            }
        );

        let state = checked
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
            .span_or_empty(state.calls)
            .first()
            .expect("captured ordinary call");
        let mut facts = checked.facts.clone();
        facts
            .borrow
            .argument_accesses
            .get_mut(call.accesses.start())
            .root_symbol = symbols::SymbolHandle::invalid();
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(machine).is_none(),
            "&{access} accepted a changed loan root"
        );
    }
}

#[test]
fn qualified_record_view_element_loans_require_exact_captured_access() {
    let checked = checked_program(
        r#"
        domain [u8; 8]::Utf8 requires valid_utf8(self);
        data Tag [copy] { case First; case Second; }
        data Cell [copy] { label: [u8; 8] in Utf8; tag: Tag; value: u64; }
        data Storage { cells: [Cell; 2]; }
        machine fill(cell: &mut Cell, tag: Tag, value: u64) {
            cell.tag = tag;
            cell.value = value;
        }
        machine run(storage: &mut Storage) {
            let view: &mut [Cell] = storage.cells.as_mut_slice();
            fill(&mut view[0], Tag::First, 31);
            fill(&mut view[1], Tag::Second, 39);
        }
        "#,
    );
    let machine = machine_named(&checked, "run");
    let target = machine_named(&checked, "fill");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "qualified view element loans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let calls = plan
        .operations
        .iter()
        .filter_map(CheckedUnitEffectOperationPlan::call)
        .filter(|call| call.target.machine == target)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2, "successive loans retain the view binding");
    let mut binding = None;
    for (element, call) in calls.iter().enumerate() {
        let argument = &call.structural_arguments[0];
        assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
        assert_eq!(
            argument.path,
            [CheckedUnitStructuralPathSegment::FixedIndex(element as u64)]
        );
        let source = argument
            .source_structural_result_binding_ordinal()
            .expect("the projected loan retains its view result");
        if let Some(previous) = binding {
            assert_eq!(source, previous);
        }
        binding = Some(source);
    }
    let coordinate = calls[0].coordinate;
    let borrow_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == machine && state.state_symbol == plan.state)
        .expect("caller borrow state");
    let captured = checked
        .facts
        .borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .find(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
        })
        .expect("captured element loan");
    let access = captured.accesses.start();
    assert_eq!(
        checked.facts.borrow.argument_accesses.get(access).kind,
        checked_trees::BorrowAccessKind::Mutable
    );
    for changed_root in [false, true] {
        let mut facts = checked.facts.clone();
        let captured = facts.borrow.argument_accesses.get_mut(access);
        if changed_root {
            captured.root_symbol = symbols::SymbolHandle::invalid();
        } else {
            captured.kind = checked_trees::BorrowAccessKind::Read;
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(machine).is_none(),
            "qualified element loan accepted changed root={changed_root} custody"
        );
    }
}

#[test]
fn boundary_call_borrows_whole_local_buffer() {
    let checked = checked_program(
        r#"
        pub boundary trait Input { machine fill(buffer: &mut [u8]); }
        machine run() reaches Input {
            let mut buffer: [u8; 16];
            Input::fill(&mut buffer);
        }
    "#,
    );
    let machine = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "local buffer boundary call: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let argument = plan
        .operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::Call(call)
                if call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit =>
            {
                call.structural_arguments.first()
            }
            _ => None,
        })
        .expect("boundary buffer argument");
    assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
    assert!(argument.path.is_empty());
    assert!(
        argument
            .source_structural_result_binding_ordinal()
            .is_some()
    );
}

#[test]
fn local_buffer_loans_preserve_fixed_and_slice_presentations() {
    for (formal, borrow, expected) in [
        (
            "&mut [u8; 4]",
            "&mut",
            CheckedStructuralAccess::MutableBorrow,
        ),
        ("&[u8]", "&", CheckedStructuralAccess::SharedBorrow),
    ] {
        let source = format!(
            r#"
            pub boundary trait Input {{ machine observe(buffer: {formal}); }}
            machine run() reaches Input {{
                let mut buffer: [u8; 4];
                Input::observe({borrow} buffer);
            }}
        "#
        );
        let checked = checked_program(&source);
        let machine = machine_named(&checked, "run");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .unwrap_or_else(|| {
                panic!(
                    "{formal}: {:?}",
                    checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .omission_for_machine(machine)
                )
            });
        assert!(plan.operations.iter().any(|operation| matches!(operation,
            CheckedUnitEffectOperationPlan::Call(call)
                if call.is_boundary()
                    && *call.result() == CheckedUnitCallResult::Unit
                    && call.structural_arguments.len() == 1
                    && call.structural_arguments[0].access == expected
                    && call.structural_arguments[0].source_structural_result_binding_ordinal().is_some())));

        // Neither matching element types nor an established local substitute
        // for the exact access captured at the authored call.
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
            .first()
            .expect("captured boundary call");
        let access = call.accesses.start();
        let mut facts = checked.facts.clone();
        facts.borrow.argument_accesses.get_mut(access).root_symbol =
            symbols::SymbolHandle::invalid();
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(machine).is_none(),
            "{formal} accepted a changed loan root"
        );
    }
}

#[test]
fn state_local_zeroed_buffer_can_end_after_its_boundary_loan() {
    let checked = checked_program(
        r#"
        pub boundary trait Input { machine fill(buffer: &mut [u8]); }
        machine run() reaches Input {
            transition { _ -> read() }
            state read() {
                let mut buffer: [u8; 16];
                Input::fill(&mut buffer);
            }
        }
    "#,
    );
    let machine = machine_named(&checked, "run");
    let graph = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "zeroed state-local buffer: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    assert!(graph.states.iter().any(|state| state.operations.iter().any(|operation|
        matches!(operation, CheckedUnitEffectOperationPlan::Call(call)
            if call.is_boundary()
                && *call.result() == CheckedUnitCallResult::Unit
                && call.structural_arguments.len() == 1
                && call.structural_arguments[0].access == CheckedStructuralAccess::MutableBorrow
                && call.structural_arguments[0].source_structural_result_binding_ordinal().is_some()))));
}

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
                CheckedUnitEffectOperationPlan::Call(call) if !call.is_boundary() => call
                    .structural_result()
                    .map(|result| (&call.structural_arguments, result)),
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
            CheckedUnitEffectOperationPlan::Call(call)
                if !call.is_boundary()
                    && call.structural_result().is_some()
                    && call.structural_arguments.iter().any(|argument| {
                        argument.access == CheckedStructuralAccess::SharedBorrow
                    }) =>
            {
                Some(call.coordinate)
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

#[test]
fn domain_carrying_local_records_lend_whole_and_projected_loans() {
    // A field's numeric domain rides a loan of the local's own storage: the
    // callee sees the same declared record, so whole `&mut`/`&` loans and a
    // `&mut` field projection all name the local's result binding.
    let checked = checked_program(
        r#"
        data Banner { width: i32 in Wrapping; }
        data Screen { banner: Banner; height: i32 in Wrapping; }
        machine Banner::widen(banner: &mut Banner, by: i32 in Wrapping) {
            banner.width = banner.width + by;
        }
        machine Screen::grow(screen: &mut Screen, by: i32 in Wrapping) {
            screen.height = screen.height + by;
        }
        machine Screen::inspect(screen: &Screen) { }
        machine run() {
            let mut screen: Screen;
            Banner::widen(&mut screen.banner, 70);
            Screen::grow(&mut screen, 1);
            Screen::inspect(&screen);
        }
        "#,
    );
    let machine = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "domain-carrying local loans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let arguments = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::Call(call) => call.structural_arguments.first(),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [projected, whole_mutable, whole_shared] = arguments.as_slice() else {
        panic!("three local loans: {arguments:?}");
    };
    assert_eq!(projected.path.len(), 1);
    assert_eq!(projected.access, CheckedStructuralAccess::MutableBorrow);
    assert!(whole_mutable.path.is_empty());
    assert_eq!(whole_mutable.access, CheckedStructuralAccess::MutableBorrow);
    assert!(whole_shared.path.is_empty());
    assert_eq!(whole_shared.access, CheckedStructuralAccess::SharedBorrow);
    let binding = projected.source_structural_result_binding_ordinal();
    assert!(binding.is_some());
    assert_eq!(
        whole_mutable.source_structural_result_binding_ordinal(),
        binding
    );
    assert_eq!(
        whole_shared.source_structural_result_binding_ordinal(),
        binding
    );

    // The projected loan still rejoins its captured borrow: a changed root
    // on that access withdraws the whole plan rather than trusting the path.
    let state = checked
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
        .span_or_empty(state.calls)
        .first()
        .expect("captured projected call");
    let mut facts = checked.facts.clone();
    facts
        .borrow
        .argument_accesses
        .get_mut(call.accesses.start())
        .root_symbol = symbols::SymbolHandle::invalid();
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        rebuilt.for_machine(machine).is_none(),
        "a projected local loan accepted a changed loan root"
    );
}

#[test]
fn affine_local_records_lend_unrestricted_leaves_to_scalar_calls() {
    // `Pair` and `Words` are affine; their `u8` leaves are unrestricted. A
    // projected shared loan observes the leaf in the local's own storage, so
    // the leaf's multiplicity, not the root's, meets the formal, and both the
    // field and the array element name the local's result binding.
    let checked = checked_program(
        r#"
        data Pair { low: u8; high: u8; }
        data Words { count: u8; bytes: [u8; 4]; }
        machine is_seventy(leaf: &u8) -> bool {
            transition leaf == 70 { true -> (true) _ -> (false) }
        }
        machine run() {
            let pair: Pair = Pair { low: 3, high: 70 };
            let words: Words = Words { count: 4, bytes: [1, 70, 3, 4] };
            let field: bool = is_seventy(&pair.high);
            let element: bool = is_seventy(&words.bytes[1]);
        }
        "#,
    );
    let machine = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "affine local leaf loans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let arguments = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::Call(call) => call.structural_arguments.first(),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [field, element] = arguments.as_slice() else {
        panic!("two leaf loans: {arguments:?}");
    };
    assert!(matches!(
        field.path.as_slice(),
        [CheckedUnitStructuralPathSegment::Field(_)]
    ));
    assert!(matches!(
        element.path.as_slice(),
        [
            CheckedUnitStructuralPathSegment::Field(_),
            CheckedUnitStructuralPathSegment::FixedIndex(1)
        ]
    ));
    for argument in [field, element] {
        assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
        assert!(
            argument
                .source_structural_result_binding_ordinal()
                .is_some()
        );
    }
    assert_ne!(
        field.source_structural_result_binding_ordinal(),
        element.source_structural_result_binding_ordinal()
    );
}

#[test]
fn local_record_loans_reach_a_plain_record_result_call() {
    // A plain record result holds no reference, so an exclusive loan of a
    // local, whole or by field path, ends with the call: the structural
    // result call keeps each local's own binding as the argument source.
    let checked = checked_program(
        r#"
        data Tally { hits: i32 in Wrapping; misses: i32 in Wrapping; }
        data Outer { height: i32 in Wrapping; tally: Tally; }
        data Report { total: i32 in Wrapping; }
        machine Tally::record(tally: &mut Tally, by: i32 in Wrapping) -> Report {
            tally.hits = tally.hits + by;
            transition { _ -> (Report { total: tally.hits + tally.misses }) }
        }
        machine run() {
            let mut tally: Tally;
            let mut outer: Outer;
            let first: Report = Tally::record(&mut tally, 30);
            let second: Report = Tally::record(&mut outer.tally, 40);
        }
        "#,
    );
    let machine = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "local loans into a record result: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let calls = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [whole, projected] = calls.as_slice() else {
        panic!("two record-result calls: {calls:?}");
    };
    let mut sources = Vec::new();
    for (call, path_length) in [(whole, 0), (projected, 1)] {
        assert!(matches!(
            call.result(),
            CheckedUnitCallResult::Structural { .. }
        ));
        let [argument] = call.structural_arguments.as_slice() else {
            panic!("one lent local: {:?}", call.structural_arguments);
        };
        assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
        assert_eq!(argument.path.len(), path_length);
        sources.push(
            argument
                .source_structural_result_binding_ordinal()
                .expect("the local's own binding lends"),
        );
    }
    assert_ne!(sources[0], sources[1], "each call lends its own local");
}

#[test]
fn a_view_result_keeps_its_local_loan_out_of_the_plain_result_lane() {
    // A returned `&mut` view borrows the lent local itself: the call mints a
    // result loan, so the lent storage stays borrowed past the call. The
    // plain-result lane must not admit that `&mut` loan of a local.
    let checked = checked_program(
        r#"
        data Cell { value: i32 in Wrapping; }
        data Tally { cells: [Cell; 2]; hits: i32 in Wrapping; }
        machine Tally::pick(tally: &mut Tally) -> &mut Cell {
            transition { _ -> &mut tally.cells[1] }
        }
        machine run() {
            let mut tally: Tally;
            let cell: &mut Cell = Tally::pick(&mut tally);
        }
        "#,
    );
    let machine = machine_named(&checked, "run");
    // A plan may take the call only through reference-result custody: the
    // call mints a result loan over the lent local and a later release ends
    // it, so the storage stays borrowed past the call.
    let Some(plan) = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
    else {
        return;
    };
    let call = plan
        .operations
        .iter()
        .find_map(CheckedUnitEffectOperationPlan::call)
        .expect("the view call is planned");
    let loan = call
        .structural_custody()
        .map(|custody| custody.reference_loan)
        .filter(|loan| loan.is_valid())
        .unwrap_or_else(|| {
            panic!("a view result retained a local loan through the plain-result lane: {call:#?}")
        });
    assert!(
        plan.operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::ReleaseReference { loan: released, .. }
                if *released == loan
        )),
        "the view result keeps the lent local borrowed until its loan is released"
    );
}

#[test]
fn returned_mutable_record_lends_its_referent_to_an_implicit_receiver() {
    let checked = checked_program(
        r#"
        data Child { value: i32 in Wrapping; }
        data Holder { child: Child; sibling: i32 in Wrapping; }
        data Bag { holders: [Holder; 2]; }
        data Main { bag: Bag; }
        machine pick(bag: &mut Bag) -> &mut Holder {
            let holders: &mut [Holder] = bag.holders.as_mut_slice();
            transition { _ -> &mut holders[1] }
        }
        machine Holder::replace(&mut self) { self.child.value = 41; }
        machine observe(holder: &Holder) { }
        machine Main::run(&mut self) {
            let selected: &mut Holder = pick(&mut self.bag);
            selected.sibling = 29;
            selected.replace();
            selected.sibling = 31;
        }
        "#,
    );
    let machine = machine_named(&checked, "run");
    let pick = machine_named(&checked, "pick");
    let replace = machine_named(&checked, "replace");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "returned-record receiver plan: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let pick_call = plan
        .operations
        .iter()
        .filter_map(CheckedUnitEffectOperationPlan::call)
        .find(|call| call.target.machine == pick)
        .expect("reference producer call");
    let result = pick_call
        .structural_result()
        .expect("reference result binding");
    let replace_call = plan
        .operations
        .iter()
        .filter_map(CheckedUnitEffectOperationPlan::call)
        .find(|call| call.target.machine == replace)
        .expect("implicit receiver call");
    let [receiver] = replace_call.structural_arguments.as_slice() else {
        panic!(
            "replace receives one returned referent: {:?}",
            replace_call.structural_arguments
        );
    };
    assert_eq!(receiver.access, CheckedStructuralAccess::MutableBorrow);
    assert_eq!(receiver.path, [CheckedUnitStructuralPathSegment::Referent]);
    assert_eq!(
        receiver.source_structural_result_binding_ordinal(),
        Some(result.binding_ordinal)
    );

    // The returned permission must come from the exact initializer call.
    // Removing its authored-expression identity leaves no producer that can
    // authorize the receiver even though the result row is still present.
    let producer = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find_map(|(handle, call)| {
            (call.target_symbol == pick_call.target.state
                && call.statement_index == result.statement_index as usize
                && call.authored_expression.is_valid())
            .then_some(handle)
        })
        .expect("exact reference producer fact");
    let mut missing_producer = checked.facts.clone();
    missing_producer
        .flow
        .control
        .calls
        .get_mut(producer)
        .authored_expression = typed_trees::expression::ExpressionHandle::invalid();
    let missing_producer_plan = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &missing_producer,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &missing_producer.flow.terminal_boundary_scalar_returns,
            structural_returns: &missing_producer.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        missing_producer_plan.for_machine(machine).is_none(),
        "a result row without its exact producer authorized the receiver"
    );

    // The implicit receiver must remain inside the producer's exact returned
    // loan. Moving the retained last use without moving its weakening makes
    // that lifetime stale and withdraws the caller plan.
    let loan = pick_call
        .structural_custody()
        .map(|custody| custody.reference_loan)
        .filter(|loan| loan.is_valid())
        .expect("returned reference loan");
    let mut stale_facts = checked.facts.clone();
    let last_use = stale_facts.borrow.loans.get(loan).last_use_statement_index;
    stale_facts
        .borrow
        .loans
        .get_mut(loan)
        .last_use_statement_index = last_use.checked_sub(1).expect("receiver follows producer");
    let stale = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &stale_facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &stale_facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &stale_facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        stale.for_machine(machine).is_none(),
        "a stale returned-reference lifetime authorized the receiver"
    );

    // A shared permission with the same referent spelling cannot be retagged
    // as the mutable receiver. Keep all captured facts unchanged and corrupt
    // only the selected local's declared reference access.
    let observe = machine_named(&checked, "observe");
    let shared_reference = checked
        .typed
        .machine_by_symbol(observe)
        .and_then(|machine| checked.typed.machine_states(machine).first())
        .and_then(|state| checked.typed.state_parameters(state).first())
        .expect("shared Holder parameter")
        .type_reference;
    let mut shared = checked.typed.clone();
    let statements = shared
        .machine_by_symbol(machine)
        .and_then(|machine| shared.machine_states(machine).first())
        .expect("run state")
        .statement_nodes;
    let typed_trees::statement::StatementNode::LocalData(selected) =
        &mut shared.statement_table.statements_mut(statements)[result.statement_index as usize]
    else {
        panic!("reference result local")
    };
    selected.type_reference = shared_reference;
    let shared_as_mutable = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &shared,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        shared_as_mutable.for_machine(machine).is_none(),
        "a shared returned carrier was retagged as a mutable receiver"
    );
}
