//! A composed state's scalar call through a whole or projected receiver,
//! including a runtime array element: admission binds the receiver formal to the call's
//! receiver expression, which the authored argument list never holds
//! (`AuthoredCall::structural_argument`), and requires a parameter operand to
//! name that exact place (`composed_control/admission.rs`). The call then
//! emits through the shared operation frame (`operation_frame/calls.rs`) and
//! the module verifies. A receiver whose record holds a provider binding
//! stays a structural operand rather than a set of scalar field reads, which
//! is the shape the run canaries exercise.

use super::{checked_source_with_core_service, lower_machine};
use crate::TerminalMachineSelection;

const INDEXED_CALLS: &str = r#"
    data Cell { value: u64 in Wrapping; }
    data Holder { cells: [Cell; 4]; }
    machine Cell::read(&self) -> u64 in Wrapping { self.value }
    machine read_cell(cell: &Cell) -> u64 in Wrapping { cell.value }
    machine Holder::probe(&self, position: u64) -> bool {
        transition position < 4 {
            true -> read(position)
            false -> (false)
        }
        state read(&self, position: u64) -> bool {
            let receiver: u64 in Wrapping = self.cells[position].read() + 1;
            let argument: u64 in Wrapping = read_cell(&self.cells[position]) + 1;
            receiver == 70 && argument == 70
        }
    }
"#;

#[test]
fn indexed_receiver_and_explicit_borrow_compose_with_scalar_arithmetic() {
    let checked = checked_source_with_core_service(INDEXED_CALLS);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Holder::probe"))
        .expect("both projected loans retain their checked index and bound");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("runtime call paths carry independently verified bounds");
    let sites = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .flatten()
        .filter(|argument| {
            argument
                .path
                .iter()
                .any(|segment| segment.runtime_index().is_some())
        })
        .count();
    assert_eq!(sites, 2, "both authored calls carry runtime projections");
    let mut shrunk = lowered.semantic_module;
    let mut arrays = 0;
    for declaration in &mut shrunk.structural_types {
        if let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = &mut declaration.shape
            && *length == 4
        {
            *length = 1;
            arrays += 1;
        }
    }
    assert_eq!(arrays, 1);
    terminal_verifier::validate_module(&shrunk)
        .expect("shrinking keeps the path structurally valid");
    assert!(
        terminal_verifier::verify_module(
            &shrunk,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "the old bound cannot authorize a smaller array"
    );
}

#[test]
fn indexed_scalar_calls_retain_bounds_and_source_access_requirements() {
    for source in [
        INDEXED_CALLS.replace("position < 4", "position < 5"),
        INDEXED_CALLS.replace("position: u64", "position: i64"),
        INDEXED_CALLS.replace("Cell::read(&self)", "Cell::read(&mut self)"),
    ] {
        let Ok(checked) = crate::front_end::checked_program_result(&source) else {
            continue;
        };
        let Ok(lowered) = lower_machine(&checked, TerminalMachineSelection::Name("Holder::probe"))
        else {
            continue;
        };
        assert!(
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "an invalid bound or mutable loan from shared storage must reject"
        );
    }
}

#[test]
fn indexed_scalar_call_rejects_a_substituted_selector() {
    let mut checked = checked_source_with_core_service(INDEXED_CALLS);
    let mut substituted = false;
    let arguments = checked
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in arguments {
        let argument = checked
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(handle);
        let Some(argument) = argument.as_place_mut() else {
            continue;
        };
        for segment in &mut argument.path {
            if let checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::Parameter { position },
            ) = segment
            {
                *position = 99;
                substituted = true;
            }
        }
    }
    assert!(substituted, "fixture contains checked runtime selectors");
    assert!(lower_machine(&checked, TerminalMachineSelection::Name("Holder::probe")).is_err());
}

const FIELD_INDEXED_CALLS: &str = r#"
    data Cell { value: u64 in Wrapping; }
    data Holder { cells: [Cell; 4]; }
    data Cursor { position: u64[0..4]; other: u64[0..4]; }
    machine Cell::read(&self) -> u64 in Wrapping { self.value }
    machine read_cell(cell: &Cell) -> u64 in Wrapping { cell.value }
    machine probe(holder: &Holder, cursor: &Cursor, other: &Cursor) -> bool {
        let receiver: u64 in Wrapping = holder.cells[cursor.position].read() + 1;
        let argument: u64 in Wrapping = read_cell(&holder.cells[cursor.position]) + 1;
        receiver == 70 && argument == 70
    }
"#;

#[test]
fn field_indexed_scalar_calls_capture_verified_selectors() {
    let checked = checked_source_with_core_service(FIELD_INDEXED_CALLS);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("probe"))
        .expect("field selectors compose with receiver and explicit scalar calls");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("captured field values retain their declared bounds");
}

#[test]
fn field_indexed_scalar_calls_capture_each_nested_selector() {
    let source = FIELD_INDEXED_CALLS
        .replace(
            "data Holder { cells: [Cell; 4]; }",
            "data Row { cells: [Cell; 4]; } data Holder { rows: [Row; 2]; }",
        )
        .replace("other: u64[0..4]", "other: u64[0..2]")
        .replace(
            "holder.cells[cursor.position]",
            "holder.rows[cursor.other].cells[cursor.position]",
        );
    let checked = checked_source_with_core_service(&source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("probe"))
        .expect("nested array loans capture both field selectors");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("each captured selector has its own bounded array projection");
}

#[test]
fn field_indexed_scalar_calls_interleave_effectful_arguments() {
    let checked = checked_source_with_core_service(
        r#"
        data Cell { value: u64 in Wrapping; }
        data Holder { cells: [Cell; 4]; }
        data Cursor { position: u64[0..4]; }
        machine Cursor::advance(&mut self) -> u64 in Wrapping {
            self.position = 1;
            0
        }
        machine read_after(cell: &Cell, ignored: u64 in Wrapping) -> u64 in Wrapping {
            cell.value + ignored
        }
        machine read_before(ignored: u64 in Wrapping, cell: &Cell) -> u64 in Wrapping {
            cell.value + ignored
        }
        machine probe(holder: &Holder, cursor: &mut Cursor) -> bool {
            let first: u64 in Wrapping = read_after(&holder.cells[cursor.position], cursor.advance());
            cursor.position = 0;
            let second: u64 in Wrapping = read_before(cursor.advance(), &holder.cells[cursor.position]);
            first == 10 && second == 20
        }
    "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("probe"))
        .expect("selector captures interleave with scalar effects");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("snapshot bounds survive the later selector mutation");
}

#[test]
fn field_indexed_scalar_calls_keep_extent_and_access_checks() {
    for source in [
        FIELD_INDEXED_CALLS.replace("position: u64[0..4]", "position: u64[0..5]"),
        FIELD_INDEXED_CALLS.replace("Cell::read(&self)", "Cell::read(&mut self)"),
    ] {
        let Ok(checked) = crate::front_end::checked_program_result(&source) else {
            continue;
        };
        let Ok(lowered) = lower_machine(&checked, TerminalMachineSelection::Name("probe")) else {
            continue;
        };
        assert!(
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "capturing a field cannot widen its bound or loan authority"
        );
    }
}

#[test]
fn field_indexed_scalar_calls_reject_substituted_selector_sources() {
    for change_root in [false, true] {
        let mut checked = checked_source_with_core_service(FIELD_INDEXED_CALLS);
        let handles = checked
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        let mut changed = 0;
        for handle in handles {
            let Some(argument) = checked
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .get_mut(handle)
                .as_place_mut()
            else {
                continue;
            };
            for segment in &mut argument.path {
                if let checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::StructuralField(field),
                ) = segment
                {
                    if change_root {
                        field.parameter_position = 2;
                    } else {
                        let checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) =
                            &mut field.path[0]
                        else {
                            panic!("selector is a record field");
                        };
                        let replacement = identity.replace("position", "other");
                        assert_ne!(*identity, replacement);
                        *identity = replacement;
                    }
                    changed += 1;
                }
            }
        }
        assert_eq!(changed, 2);
        assert!(
            lower_machine(&checked, TerminalMachineSelection::Name("probe")).is_err(),
            "an equally typed field on the same or another root is not the authored selector"
        );
    }
}

const NESTED_INDEXED_CALLS: &str = r#"
    data Cell { value: u64 in Wrapping; }
    data Holder { cells: [Cell; 4]; position: u64[0..4]; }
    machine Cell::read(&self) -> u64 in Wrapping { self.value }
    machine read_cell(cell: &Cell) -> u64 in Wrapping { cell.value }
    machine combine(cell: &Cell, value: u64 in Wrapping) -> u64 in Wrapping {
        cell.value + value
    }
    machine Holder::probe(&self) -> bool {
        let explicit: u64 in Wrapping = combine(&self.cells[0], read_cell(&self.cells[self.position]));
        let receiver: u64 in Wrapping = combine(&self.cells[0], self.cells[self.position].read());
        explicit == 70 && receiver == 70
    }
"#;

#[test]
fn nested_indexed_calls_rejoin_place_and_selector_observations() {
    for source in [
        NESTED_INDEXED_CALLS.to_owned(),
        NESTED_INDEXED_CALLS.replace("[self.position]", "[2]"),
        NESTED_INDEXED_CALLS
            .replace("data Holder { cells: [Cell; 4]; position: u64[0..4]; }",
                "data Row { cells: [Cell; 4]; } data Holder { rows: [Row; 2]; row: u64[0..2]; position: u64[0..4]; }")
            .replace("self.cells[0]", "self.rows[0].cells[0]")
            .replace("self.cells[self.position]", "self.rows[self.row].cells[self.position]"),
        NESTED_INDEXED_CALLS
            .replace(
                "Holder::probe(&self)",
                "Holder::probe(&self, position: u64[0..4])",
            )
            .replace("[self.position]", "[position]"),
    ] {
        let checked = checked_source_with_core_service(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Holder::probe"))
            .expect("indexed calls are ordinary scalar operands beside structural actuals");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("nested indexed calls retain their bounds and borrowed places");
    }
}

#[test]
fn nested_indexed_calls_require_the_exact_ordered_observation_roster() {
    let original = checked_source_with_core_service(NESTED_INDEXED_CALLS);
    lower_machine(&original, TerminalMachineSelection::Name("Holder::probe"))
        .expect("authentic nested calls lower");
    let calls = original
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            let checked_trees::CheckedScalarComputationKind::Call {
                source_call,
                arguments,
                ..
            } = node.kind
            else {
                return None;
            };
            let plans = &original.facts.values.scalar_computations;
            plans
                .operands
                .span(arguments)?
                .iter()
                .any(|argument| {
                    matches!(
                        plans.nodes.get(*argument).kind,
                        checked_trees::CheckedScalarComputationKind::Call { .. }
                    )
                })
                .then(|| original.facts.flow.control.calls.get(source_call).accesses)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        2,
        "both outer calls observe their nested invocation"
    );
    for accesses in calls {
        assert_eq!(
            original
                .facts
                .borrow
                .argument_accesses
                .span_or_empty(accesses)
                .len(),
            3,
            "direct borrow, nested indexed place, then selector field"
        );
        for mutation in 0..4 {
            let mut changed = original.clone();
            let rows = changed
                .facts
                .borrow
                .argument_accesses
                .span_mut_or_empty(accesses);
            match mutation {
                0 => rows.swap(1, 2),
                1 => rows[2] = rows[1].clone(),
                2 => rows[1].segments = rows[0].segments,
                3 => rows[2].kind = checked_trees::BorrowAccessKind::Mutable,
                _ => unreachable!(),
            }
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name("Holder::probe")).is_err(),
                "substituted, missing, reordered or exclusive read evidence must reject"
            );
        }
    }
}

/// A multi-state caller calls a value method on its whole receiver and
/// dispatches on the result.
const WHOLE_RECEIVER: &str = r#"
    pub boundary trait Console {
        machine exit(code: i32) reaches Console;
    }
    data Main { console: Binding<Console>; sum: i32 in Wrapping; }
    machine Main::doubled(&self) -> i32 in Wrapping { self.sum + self.sum }
    machine Main::main(&mut self) reaches Console {
        self.sum = 3;
        let p: i32 in Wrapping = self.doubled();
        transition p == 6 { true -> ok() _ -> bad() }
        state ok(&mut self) { self.console.exit(70); }
        state bad(&mut self) { self.console.exit(71); }
    }
"#;

/// A substituted receiver operand still rejects: the checked row must name
/// the parameter place the authored receiver spells, here the whole `self`.
#[test]
fn a_receiver_operand_that_drifts_from_its_authored_place_rejects() {
    let mut checked = checked_source_with_core_service(WHOLE_RECEIVER);
    // The initializer's call arrives as a scalar computation: the receiver
    // operand is the Call node's first structural argument, a `Place` plan.
    let call_span = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|node| match &node.1.kind {
            checked_trees::CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } => Some(*structural_arguments),
            _ => None,
        })
        .expect("the receiver call computation");
    let receiver = checked
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .span_mut_or_empty(call_span)
        .first_mut()
        .and_then(|argument| argument.as_place_mut())
        .expect("the receiver operand place");
    receiver
        .path
        .push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            "sum".to_owned(),
        ));
    assert!(
        lower_machine(&checked, TerminalMachineSelection::Name("Main::main")).is_err(),
        "a receiver operand must name the authored receiver place"
    );
}

const MUTABLE_ARGUMENT_EVALUATION: &str = r#"
    data Counter { hits: u64 in Wrapping; }
    machine Counter::bump(&mut self, value: u64 in Wrapping) -> u64 in Wrapping {
        self.hits = self.hits + 1;
        value
    }
    machine Counter::nested(&mut self) {
        transition { _ -> finish(self.bump(self.bump(7))) }
        state finish(&mut self, value: u64 in Wrapping) { }
    }
    machine Counter::read_before_call(&mut self) {
        transition { _ -> finish(self.bump(self.hits + 1)) }
        state finish(&mut self, value: u64 in Wrapping) { }
    }
"#;

#[test]
fn mutable_receiver_call_accepts_completed_scalar_argument_evaluation() {
    let checked = checked_source_with_core_service(MUTABLE_ARGUMENT_EVALUATION);
    for machine in ["Counter::nested", "Counter::read_before_call"] {
        lower_machine(&checked, TerminalMachineSelection::Name(machine))
            .unwrap_or_else(|error| panic!("{machine}: {error:?}"));
    }
}

#[test]
fn completed_scalar_argument_observations_still_require_source_custody() {
    let original = checked_source_with_core_service(MUTABLE_ARGUMENT_EVALUATION);
    lower_machine(&original, TerminalMachineSelection::Name("Counter::nested"))
        .expect("original nested invocation");
    let accesses = original
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(_, node)| {
            let checked_trees::CheckedScalarComputationKind::Call {
                source_call,
                arguments,
                ..
            } = node.kind
            else {
                return None;
            };
            let nested = original
                .facts
                .values
                .scalar_computations
                .operands
                .span(arguments)?;
            nested
                .iter()
                .any(|argument| {
                    matches!(
                        original
                            .facts
                            .values
                            .scalar_computations
                            .nodes
                            .get(*argument)
                            .kind,
                        checked_trees::CheckedScalarComputationKind::Call { .. }
                    )
                })
                .then(|| original.facts.flow.control.calls.get(source_call).accesses)
        })
        .expect("the outer scalar call observes its nested receiver");
    for exclusive in [false, true] {
        let mut changed = original.clone();
        let row = &mut changed
            .facts
            .borrow
            .argument_accesses
            .span_mut_or_empty(accesses)[0];
        if exclusive {
            row.kind = checked_trees::BorrowAccessKind::Mutable;
        } else {
            row.root_symbol = symbols::SymbolHandle::invalid();
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Counter::nested")).is_err(),
            "completed argument evaluation still requires exact observation evidence"
        );
    }
}

const MIXED_ARGUMENT_EVALUATION: &str = r#"
    data Cell { value: u64 in Wrapping; }
    data Counter { hits: u64 in Wrapping; cell: Cell; }
    machine Counter::read(&mut self, value: u64 in Wrapping, zero: u64 in Wrapping, cell: &Cell)
        -> u64 in Wrapping {
        self.hits = self.hits + 1;
        value + zero + cell.value
    }
    machine Counter::enter(&mut self, other: &Cell) {
        transition { _ -> finish(self.read(self.hits + self.hits, 0, other)) }
        state finish(&mut self, value: u64 in Wrapping) { }
    }
"#;

/// Scalar actuals finish evaluating before the invocation, so the two reads of
/// `self.hits` hold no loan beside the `&mut self` receiver and never reach the
/// call as structural operands. A retained reference does: `&self.cell` stays
/// a structural operand of the call. Because it lies strictly inside the
/// receiver's extent, the loan table admits it as a shared child that freezes
/// the mutable parent (`wiki/spec/terminal-psi/loans.md`; the verifier's
/// `interior_member_borrow`). A mutable child of a shared parent has no such
/// cell in the table and still rejects.
#[test]
fn receiver_overlap_distinguishes_scalar_observations_from_retained_references() {
    let structural_operands = |lowered: &lowered_psi::LoweredPsi| {
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } => Some(structural_arguments.clone()),
                _ => None,
            })
            .expect("the receiver call lowers to a structural scalar call")
    };
    let original = checked_source_with_core_service(MIXED_ARGUMENT_EVALUATION);
    let lowered = lower_machine(&original, TerminalMachineSelection::Name("Counter::enter"))
        .expect("two scalar reads, a literal and an independent shared reference");
    let operands = structural_operands(&lowered);
    assert_eq!(operands.len(), 2, "the receiver and `other` only");
    assert_ne!(operands[0].place, operands[1].place);

    let member = checked_source_with_core_service(&MIXED_ARGUMENT_EVALUATION.replace(
        "self.hits + self.hits, 0, other",
        "self.hits + self.hits, 0, &self.cell",
    ));
    let lowered = lower_machine(&member, TerminalMachineSelection::Name("Counter::enter"))
        .expect("a shared member borrow inside the mutable receiver is a frozen subloan");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the member subloan verifies independently");
    let operands = structural_operands(&lowered);
    let [receiver, cell] = operands.as_slice() else {
        panic!("the receiver and the retained member reference: {operands:#?}")
    };
    assert_eq!(
        receiver.place, cell.place,
        "the reference stays live on the receiver's root"
    );
    assert!(receiver.path.is_empty());
    assert!(
        !cell.path.is_empty(),
        "the reference is strictly inside the receiver"
    );

    let mutable_child_of_shared_parent = MIXED_ARGUMENT_EVALUATION
        .replace(
            "machine Counter::read(&mut self, value: u64 in Wrapping, zero: u64 in Wrapping, cell: &Cell)",
            "machine Counter::read(&self, value: u64 in Wrapping, zero: u64 in Wrapping, cell: &mut Cell)",
        )
        .replace("self.hits = self.hits + 1;", "cell.value = cell.value + 1;")
        .replace("self.hits + self.hits, 0, other", "self.hits + self.hits, 0, &mut self.cell");
    let errors = crate::front_end::checked_program_result(&mutable_child_of_shared_parent)
        .expect_err("a mutable member loan beside a shared receiver has no loan-table cell");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("receives shared receiver overlapping another argument")),
        "{errors:#?}"
    );
}
