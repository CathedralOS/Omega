//! Composite values supplied directly as internal state arguments: a call's
//! structural result established on the selected edge, and a copyable member
//! projected out of a live source parameter. The source-free artifact must
//! deliver the selected values, and the independent edge validation must
//! reject a plan whose establishment or projection no longer matches the
//! authored operand.
use super::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedUnitEffectOperationPlan, admission,
};
use checked_trees::CheckedStructuralControlTransferSourcePlan;

const SOURCE: &str = r#"
    data Pair [copy] { a: u32 in Wrapping; b: u32 in Wrapping; }
    data Holder [copy] { pair: Pair; }
    machine make(a: u32 in Wrapping, b: u32 in Wrapping) -> Pair {
        Pair { a: a, b: b }
    }
    data Root {}
    machine Root::enter() -> u32 in Wrapping {
        transition true {
            true -> hold(Holder { pair: make(30, 40) })
            false -> hold(Holder { pair: make(1, 2) })
        }
        state hold(holder: Holder) -> u32 in Wrapping {
            transition { _ -> take(holder.pair) }
        }
        state take(pair: Pair) -> u32 in Wrapping { pair.a + pair.b }
    }
"#;

fn plan(checked: &checked_trees::CheckedTrees) -> CheckedComposedUnitControlMachinePlan {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| checked.symbols.display_path(plan.machine, "::") == "Root::enter")
        .expect("composite state arguments keep the composed graph")
        .clone()
}

fn interpreted(checked: &checked_trees::CheckedTrees) -> u128 {
    let lowered = crate::lower_machine(
        checked,
        crate::TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("composite state arguments lower through verified Terminal Psi");
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(&semantic_bytes).unwrap(),
        &terminal_codec::decode_proof_bundle(&proof_bytes).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the source-free artifact verifies");
    let terminal_interpreter::TerminalExecutionResult::Scalar(
        terminal_interpreter::TerminalScalarValue::Integer {
            value: semantic_vocabulary::IntegerValue::Unsigned(value),
            ..
        },
    ) = terminal_interpreter::interpret_terminal_artifact(
        &semantic_bytes,
        &proof_bytes,
        &proof_admission::AdmissionProfile::default(),
        &[],
    )
    .expect("the source-free artifact executes")
    else {
        panic!("the machine completes with its unsigned scalar result")
    };
    value
}

#[test]
fn composite_edge_arguments_deliver_the_selected_values() {
    let checked = crate::front_end::checked_program(SOURCE);
    let plan = plan(&checked);
    admission::admit(&checked, &plan).expect("source-exact composite edges");
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &plan.states[0].terminator
    else {
        panic!("the entry selects between two call-bearing edges");
    };
    for edge in [when_true, when_false] {
        let [transfer] = edge.transfers.as_slice() else {
            panic!("one composite argument per edge");
        };
        let CheckedStructuralControlTransferSourcePlan::EstablishedValue { establishment, .. } =
            &transfer.source
        else {
            panic!("the record literal is established on its own edge");
        };
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, calls, .. } =
            establishment.as_ref()
        else {
            panic!("an ordinary structural establishment");
        };
        assert_eq!(result.statement_index, edge.statement_ordinal);
        assert_eq!(calls.len(), 1, "the member call belongs to its edge");
    }
    assert!(
        plan.states[0]
            .operations
            .iter()
            .all(|operation| operation.call().is_none()),
        "no edge call is hoisted ahead of the selection"
    );
    assert!(matches!(
        &plan.states[1].terminator,
        CheckedComposedUnitControlTerminatorPlan::Jump { successor }
            if matches!(
                successor.transfers[0].source,
                CheckedStructuralControlTransferSourcePlan::ProjectedCopy { .. }
            )
    ));
    assert_eq!(interpreted(&checked), 70);
}

#[test]
fn composite_edge_arguments_reject_drifted_custody() {
    let checked = crate::front_end::checked_program(SOURCE);
    let original = plan(&checked);
    for mutation in ["statement", "type", "value", "projection"] {
        let mut changed = original.clone();
        let (entry, rest) = changed.states.split_at_mut(1);
        let CheckedComposedUnitControlTerminatorPlan::Conditional {
            when_true,
            when_false,
            ..
        } = &mut entry[0].terminator
        else {
            panic!("conditional entry");
        };
        let false_source = when_false.transfers[0].source.clone();
        let CheckedStructuralControlTransferSourcePlan::EstablishedValue { establishment, .. } =
            &mut when_true.transfers[0].source
        else {
            panic!("established edge argument");
        };
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } =
            establishment.as_mut()
        else {
            panic!("structural establishment");
        };
        match mutation {
            "statement" => result.statement_index += 1,
            "type" => result.type_identity.push_str("::drifted"),
            // The unselected arm's value cannot stand in for the selected one.
            "value" => when_true.transfers[0].source = false_source,
            _ => {
                let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
                    &mut rest[0].terminator
                else {
                    panic!("projection edge");
                };
                let CheckedStructuralControlTransferSourcePlan::ProjectedCopy { subject } =
                    &mut successor.transfers[0].source
                else {
                    panic!("projected copy");
                };
                subject.path.clear();
            }
        }
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "{mutation} drift must not pass edge validation"
        );
    }
}

#[test]
fn indexed_composite_edge_arguments_reject_a_same_typed_selector() {
    let checked = crate::front_end::checked_program(
        r#"
        data Kind [copy] { case First; case Second; }
        data Item [copy] { kind: Kind; value: u64; }
        data Output { kind: Kind; value: u64; }
        data Root {}
        machine Root::enter(items: &[Item], index: u64, other: u64, output: &mut Output) {
            transition index < items.len {
                true -> write(items[index], output)
                false -> {}
            }
            state write(item: Item, output: &mut Output) {
                output.kind = item.kind;
                output.value = item.value;
            }
        }
        "#,
    );
    let original = plan(&checked);
    admission::admit(&checked, &original).expect("source-exact indexed state argument");
    let lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("the guarded indexed argument lowers before any mutation");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the indexed argument retains independently verified bounds and custody");

    let CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
        jump,
        return_when_true: false,
        return_arm: checked_trees::CheckedConditionalReturnArm::Unit { .. },
        ..
    } = &original.states[0].terminator
    else {
        panic!("the length guard selects the indexed transfer");
    };
    let CheckedStructuralControlTransferSourcePlan::EstablishedValue { establishment, .. } =
        &jump.transfers[0].source
    else {
        panic!("the indexed record is copied into a fresh edge value");
    };
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } =
        establishment.as_ref()
    else {
        panic!("ordinary structural establishment");
    };
    let mut changed = checked.clone();
    let checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } =
        &mut changed
            .facts
            .values
            .structural_values
            .nodes
            .get_mut(*value)
            .kind
    else {
        panic!("the mixed record uses the general structural copy route");
    };
    let [
        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
            checked_trees::CheckedRuntimeIndex::Parameter { position },
        ),
    ] = source.path.as_mut_slice()
    else {
        panic!("one exact runtime selector belongs to the copied source");
    };
    assert_eq!(*position, 0, "index is the first scalar formal");
    *position = 1; // `other` has the same u64 type, but is not this source occurrence.
    assert!(
        admission::admit(&changed, &original).is_err(),
        "a same-typed selector cannot replace the authored indexed source"
    );
}

/// A `[copy]` record whose member is a shared view, copied out of the
/// receiver into an owned successor parameter. The view keeps its source
/// loan; copy permission establishes no predicate of the member.
const VIEW_RECORD_SOURCE: &str = r#"
    data Room [copy] { label: &[u8]; }
    data Root { room: Room; }
    machine Root::enter(&mut self) -> u64 {
        self.room.label = "Gate";
        transition { _ -> take(self.room) }
        state take(&mut self, room: Room) -> u64 { (room.label.len) }
    }
"#;

#[test]
fn shared_view_records_cross_an_edge_as_projected_copies() {
    let checked = crate::front_end::checked_program(VIEW_RECORD_SOURCE);
    let original = plan(&checked);
    admission::admit(&checked, &original).expect("a shared-view record copies whole");
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &original.states[0].terminator
    else {
        panic!("the entry jumps to the copying successor");
    };
    // The receiver forwards as itself; the room is the one projected copy.
    let copies = successor
        .transfers
        .iter()
        .enumerate()
        .filter(|(_, transfer)| {
            matches!(
                transfer.source,
                CheckedStructuralControlTransferSourcePlan::ProjectedCopy { .. }
            )
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [copy] = copies.as_slice() else {
        panic!("exactly one projected copy");
    };
    let mut changed = original.clone();
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut changed.states[0].terminator
    else {
        panic!("projection edge");
    };
    let CheckedStructuralControlTransferSourcePlan::ProjectedCopy { subject } =
        &mut successor.transfers[*copy].source
    else {
        panic!("projected copy");
    };
    subject.path.clear();
    assert!(
        admission::admit(&checked, &changed).is_err(),
        "a copy that no longer names the authored member must not pass"
    );
}
