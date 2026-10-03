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

const INLINE_VIEW_SOURCE: &str = r#"
    data Root { bytes: [u8; 8]; spare: [u8; 8]; offset: u32 [0..=4]; }
    machine Root::enter(root: &mut Root) -> u64 {
        root.offset = 3;
        transition root.offset < 4 {
            true -> write(root, &mut root.bytes[root.offset] as &mut [u8])
            false -> 0
        }
        state write(root: &mut Root, view: &mut [u8]) -> u64 {
            transition view.len > 4 {
                true -> patch(root, view)
                false -> 0
            }
        }
        state patch(root: &mut Root, view: &mut [u8]) -> u64 {
            view[4] = 9;
            transition { _ -> done(root) }
        }
        state done(root: &mut Root) -> u64 {
            transition root.bytes[7] == 9 { true -> 70 false -> 0 }
        }
    }
"#;

fn inline_view_establishment(
    plan: &mut CheckedComposedUnitControlMachinePlan,
) -> &mut CheckedUnitEffectOperationPlan {
    let edge = match &mut plan.states[0].terminator {
        CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { jump, .. } => jump,
        CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } => when_true,
        _ => panic!("the guarded recast belongs to the selected edge"),
    };
    edge.transfers
        .iter_mut()
        .find_map(|transfer| match &mut transfer.source {
            CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                establishment, ..
            } => Some(establishment.as_mut()),
            _ => None,
        })
        .expect("the inline descriptor has one ordinary establishment")
}

#[test]
fn inline_view_edge_preserves_affine_carrier_and_source_free_custody() {
    let checked = crate::front_end::checked_program(INLINE_VIEW_SOURCE);
    let mut original = plan(&checked);
    admission::admit(&checked, &original)
        .expect("the borrowed formal retains the affine descriptor's independent debt");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } =
        inline_view_establishment(&mut original)
    else {
        panic!("ordinary establishment")
    };
    assert_eq!(
        result.multiplicity,
        language_semantics::Multiplicity::Affine
    );
    let lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("inline view lowers on its selected edge");
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(&semantic_bytes).unwrap(),
        &terminal_codec::decode_proof_bundle(&proof_bytes).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("descriptor authority, region bounds and disposal survive serialization");
}

#[test]
fn inline_view_edge_rejects_changed_custody_source_and_region() {
    let checked = crate::front_end::checked_program(INLINE_VIEW_SOURCE);
    let original = plan(&checked);
    admission::admit(&checked, &original).unwrap();
    let root = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Root")
        .unwrap();
    let spare = checked
        .data_members(root)
        .iter()
        .find_map(|member| match member {
            checked_trees::data::DataMember::Field(field) if field.name.as_str() == "spare" => {
                Some(field.path_identity())
            }
            _ => None,
        })
        .unwrap();
    for mutation in 0..5 {
        let mut changed = checked.clone();
        let mut bad_plan = original.clone();
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. } =
            inline_view_establishment(&mut bad_plan)
        else {
            unreachable!()
        };
        match mutation {
            0 => result.multiplicity = language_semantics::Multiplicity::Unrestricted,
            1 => result.statement_index += 1,
            _ => {
                let checked_trees::CheckedStructuralValueKind::MutableSliceView { source, region } =
                    &mut changed
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get_mut(*value)
                        .kind
                else {
                    panic!("exact mutable view producer")
                };
                match mutation {
                    2 => source.access = checked_trees::CheckedStructuralAccess::SharedBorrow,
                    3 => {
                        let checked_trees::CheckedUnitStructuralPathSegment::Field(field) =
                            &mut source.path[0]
                        else {
                            panic!("the byte backing is the authored field")
                        };
                        *field = spare.clone();
                    }
                    4 => {
                        let checked_trees::CheckedElementViewRegion::RepresentationBytes {
                            end,
                            ..
                        } = region
                        else {
                            panic!("captured byte region")
                        };
                        *end -= 1;
                    }
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            admission::admit(&changed, &bad_plan).is_err(),
            "changed inline view custody {mutation}"
        );
    }
}

#[test]
fn inline_view_edge_indexed_record_observation_replays_each_coordinate() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
        CheckedStorageRoot, CheckedStructuralPredicatePathSegment,
    };
    let checked = crate::front_end::checked_program(
        r#"
        data Pair { low: u16; high: u16; }
        data Root {}
        machine Root::enter(view: &[Pair], other: &[Pair]) -> u16 {
            transition view.len > 1 {
                true -> read(view, other)
                _ -> 0
            }
            state read(view: &[Pair], other: &[Pair]) -> u16 {
                transition view[1].low == 7 { true -> 7 _ -> 0 }
            }
        }
        "#,
    );
    let (_, binding) = checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .find(|(_, binding)| {
            checked.symbols.display_path(binding.state, "::") == "Root::enter::read"
                && binding.role == CheckedScalarExpressionRole::Guard
        })
        .expect("the guarded observation retains its exact authored occurrence");
    let original = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(binding.state, binding.statement_ordinal, binding.role)
        .unwrap();
    let replay = |value: &CheckedScalarExpression| {
        let mut changed = checked.clone();
        let retained = changed
            .facts
            .values
            .scalar_expressions
            .expressions
            .iter_mut()
            .find(|retained| {
                retained.state == binding.state
                    && retained.statement_ordinal == binding.statement_ordinal
                    && retained.role == binding.role
            })
            .unwrap();
        retained.expression = value.clone();
        // Enter through the production owner: it authenticates the occurrence
        // and removes the checked guard's builtin `subject == true` wrapper
        // before replaying the retained Boolean subject.
        crate::expression_preparation::source_custody::validate_pure(
            &changed,
            binding,
            semantic_vocabulary::ScalarType::Boolean,
        )
    };
    replay(original).expect("a view read owns a live-bound obligation, not a fixed-array extent");
    crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("Terminal independently proves the dominating view bound");
    for mutation in 0..3 {
        let mut changed = original.clone();
        let CheckedScalarExpression::Boolean(boolean) = &mut changed else {
            panic!("Boolean guard")
        };
        let CheckedBooleanExpression::IntegerComparison { left, .. } = boolean.as_mut() else {
            panic!("field comparison")
        };
        let CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            index,
            element_path,
            ..
        } = left.as_mut()
        else {
            panic!("indexed record observation")
        };
        match mutation {
            0 => *root = CheckedStorageRoot::Parameter { index: 1 },
            1 => element_path[0] = CheckedStructuralPredicatePathSegment::Field("high".into()),
            2 => {
                // Keep a valid index carrier but change the authored selector.
                let CheckedScalarExpression::IntegerLiteral { literal } = index.as_mut() else {
                    panic!("literal selector")
                };
                *literal = numerics::literals::IntegerLiteral::from_value(0)
                    .with_landing(literal.landing().expect("typed index"));
            }
            _ => unreachable!(),
        }
        assert!(
            replay(&changed).is_err(),
            "changed view coordinate {mutation}"
        );
    }
}

// Preserve the public shared-view customer's preceding writes and forwarding:
// the region proof must discharge from its retained endpoints in this context.
const INLINE_SHARED_RECORD_SOURCE: &str = r#"
// Inline shared byte and record views retain their captured backing window.
// Scalar observations through the record slice replay their exact field and
// index while independent bounds proofs retain the live descriptor extent.

data Pair {
    low: u16;
    high: u16;
}

data Main {
    tail_bytes: [u8; 8];
    pair_bytes: [u8; 12];
    k: u32 [0..=4];
}

machine Main::main(main: &mut Main) -> u64 {
    main.k = 3;
    main.tail_bytes[7] = 9;
    main.pair_bytes[4] = 1;
    main.pair_bytes[5] = 2;
    main.pair_bytes[6] = 3;
    main.pair_bytes[7] = 4;
    main.pair_bytes[8] = 5;
    main.pair_bytes[9] = 6;
    main.pair_bytes[10] = 7;
    main.pair_bytes[11] = 8;

    transition main.k < 4 {
        true -> write_tail(main, &main.tail_bytes[main.k] as &[u8])
        _ -> bad_tail_length(main)
    }

    state write_tail(main: &mut Main, tail: &[u8]) -> u64 {
    transition tail.len > 4 {
        true -> write_tail_bytes(main, tail)
        _ -> bad_tail_length(main)
    }
    }

    state write_tail_bytes(main: &mut Main, tail: &[u8]) -> u64 {
    // Changing the selector cannot relocate the captured descriptor.
    main.k = 1;
    transition tail.len == 5 && tail[4] == 9 {
        true -> check_tail_backing(main)
        _ -> bad_tail_write(main)
    }
    }

    state check_tail_backing(main: &mut Main) -> u64 {
    transition main.tail_bytes[7] == 9 {
        true -> pair_view(main)
        _ -> bad_tail_write(main)
    }
    }

    state pair_view(main: &mut Main) -> u64 {
    transition main.k == 1 {
        true -> write_pairs(main, &main.pair_bytes[4] as &[Pair])
        _ -> bad_pair_length(main)
    }
    }

    state write_pairs(main: &mut Main, pairs: &[Pair]) -> u64 {
    transition pairs.len > 1 {
        true -> write_pair_fields(main, pairs)
        _ -> bad_pair_length(main)
    }
    }

    state write_pair_fields(main: &mut Main, pairs: &[Pair]) -> u64 {
    transition pairs.len == 2 && pairs[0].low == 0x0201 && pairs[0].high == 0x0403 && pairs[1].low == 0x0605 && pairs[1].high == 0x0807 {
        true -> check_pair_backing(main)
        _ -> bad_pair_write(main)
    }
    }

    state check_pair_backing(main: &mut Main) -> u64 {
    transition main.pair_bytes[4] == 1 && main.pair_bytes[5] == 2
        && main.pair_bytes[6] == 3 && main.pair_bytes[7] == 4
        && main.pair_bytes[8] == 5 && main.pair_bytes[9] == 6
        && main.pair_bytes[10] == 7 && main.pair_bytes[11] == 8 {
        true -> good(main)
        _ -> bad_pair_write(main)
    }
    }

    state good(main: &mut Main) -> u64 { transition { _ -> 70 } }
    state bad_tail_length(main: &mut Main) -> u64 { transition { _ -> 71 } }
    state bad_tail_write(main: &mut Main) -> u64 { transition { _ -> 72 } }
    state bad_pair_length(main: &mut Main) -> u64 { transition { _ -> 73 } }
    state bad_pair_write(main: &mut Main) -> u64 { transition { _ -> 74 } }
}
"#;

#[test]
fn inline_view_edge_shared_record_region_preserves_its_full_proof_context() {
    let checked = crate::front_end::checked_program(INLINE_SHARED_RECORD_SOURCE);
    let lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Main::main"),
    )
    .expect("the 4..12 Pair region and forwarded reads retain independently provable bounds");
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(&semantic_bytes).unwrap(),
        &terminal_codec::decode_proof_bundle(&proof_bytes).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the actual shared region's full context survives source-free replay");
}
