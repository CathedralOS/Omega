use super::{
    CheckedScalarDispatchPattern, CheckedStructuralValueKind, CheckedTrees,
    CheckedUnitEffectOperationPlan, SymbolHandle, validate,
};

#[test]
fn nested_owned_byte_literals_replay_bytes_carrier_and_access() {
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralPathSegment,
    };

    for literal in ["", "Gate"] {
        let original = crate::front_end::checked_program(&format!(
            "domain [u8; 8]::Utf8 requires valid_utf8(self);
             data Label [copy] {{ text: [u8; 8] in Utf8; sibling: u64; }}
             data Entry [copy] {{ label: Label; sibling: u64; }}
             machine make() -> Entry {{
                 Entry {{ label: Label {{ text: \"{literal}\", sibling: 29 }}, sibling: 31 }}
             }}"
        ));
        let lowered =
            crate::lower_machine(&original, crate::TerminalMachineSelection::Name("make"))
                .expect("nested byte literal constructs owned backing");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("record byte qualifications and capacity verify independently");
        assert_eq!(
            lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishByteSequenceConcatenation { .. }
                ))
                .count(),
            1,
            "literal constructs one fresh bounded owner"
        );

        let (value, _) = original
            .facts
            .values
            .structural_values
            .nodes
            .iter()
            .find(|(_, node)| {
                matches!(&node.kind,
                CheckedStructuralValueKind::Place(argument) if matches!(argument.source,
                    CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }))
            })
            .expect("retained byte literal source");
        for mutation in 0..4 {
            let mut forged = original.clone();
            let CheckedStructuralValueKind::Place(argument) = &mut forged
                .facts
                .values
                .structural_values
                .nodes
                .get_mut(value)
                .kind
            else {
                unreachable!()
            };
            match mutation {
                0 => {
                    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } =
                        &mut argument.source
                    else {
                        unreachable!()
                    };
                    if bytes.is_empty() {
                        bytes.push(b'x');
                    } else {
                        bytes[0] = b'B';
                    }
                }
                1 => argument.access = CheckedStructuralAccess::SharedBorrow,
                2 => argument.type_identity.push_str("::different_capacity"),
                3 => argument
                    .path
                    .push(CheckedUnitStructuralPathSegment::FixedIndex(0)),
                _ => unreachable!(),
            }
            assert!(
                crate::lower_machine(&forged, crate::TerminalMachineSelection::Name("make"))
                    .is_err(),
                "literal source mutation {mutation} must not become trusted construction"
            );
        }
    }
}

#[test]
fn zeroed_local_array_supplies_slice_view_without_an_initializer_expression() {
    let checked = crate::front_end::checked_program(
        "data Entry { value: u8; }
         machine run() {
             let mut entries: [Entry; 2];
             let view: &[Entry] = entries.as_slice();
         }",
    );
    let values = &checked.facts.values.structural_values;
    let (_, root) = values
        .roots
        .iter()
        .find(|(_, root)| {
            matches!(
                values.nodes.get(root.root).kind,
                CheckedStructuralValueKind::BorrowedSliceView { .. }
            )
        })
        .expect("slice view root");
    let CheckedStructuralValueKind::BorrowedSliceView {
        source: argument, ..
    } = &values.nodes.get(root.root).kind
    else {
        unreachable!()
    };
    let (machine, state) = super::super::authored_state(&checked, root.state).unwrap();
    let replay = |statement, argument| {
        super::slice_view::validate(
            &checked,
            machine,
            state,
            statement,
            root.expression,
            argument,
            crate::expression_preparation::bindings::LoanAccess::Shared,
            checked_trees::CheckedElementViewRegion::WholeCollection,
        )
    };
    replay(root.statement_ordinal, argument).expect("declaration establishes zeroed backing");
    assert!(
        replay(0, argument).is_err(),
        "backing must precede the view"
    );
    let mut wrong_access = argument.clone();
    wrong_access.access = checked_trees::CheckedStructuralAccess::MutableBorrow;
    assert!(
        replay(root.statement_ordinal, &wrong_access).is_err(),
        "cannot widen access"
    );
}

fn checked() -> CheckedTrees {
    let source = "data Tag { case First; case Second; }
        machine choose(selector: u64) -> Tag {
            match selector { 0 -> Tag::First, _ -> Tag::Second }
        }";
    crate::front_end::checked_program(source)
}

#[test]
fn qualified_collection_views_replay_exact_source_type_path_and_access() {
    for mutable in [false, true] {
        let method = if mutable { "as_mut_slice" } else { "as_slice" };
        let access = if mutable { "mut " } else { "" };
        let checked = crate::front_end::checked_program(&format!(
            "domain [u8; 8]::Utf8 requires valid_utf8(self);
             data Cell [copy] {{ label: [u8; 8] in Utf8; }}
             data Storage {{ cells: [Cell; 2]; }}
             machine run(storage: &mut Storage) {{
                 let view: &{access}[Cell] = storage.cells.{method}();
             }}"
        ));
        let values = &checked.facts.values.structural_values;
        let (root, argument) = values
            .roots
            .iter()
            .find_map(|(_, root)| match &values.nodes.get(root.root).kind {
                CheckedStructuralValueKind::BorrowedSliceView { source, .. }
                | CheckedStructuralValueKind::MutableSliceView { source, .. } => {
                    Some((root, source))
                }
                _ => None,
            })
            .expect("retained collection view");
        let (machine, state) = super::super::authored_state(&checked, root.state).unwrap();
        let loan = if mutable {
            crate::expression_preparation::bindings::LoanAccess::Mutable
        } else {
            crate::expression_preparation::bindings::LoanAccess::Shared
        };
        let replay = |argument| {
            super::slice_view::validate(
                &checked,
                machine,
                state,
                root.statement_ordinal,
                root.expression,
                argument,
                loan,
                checked_trees::CheckedElementViewRegion::WholeCollection,
            )
        };
        replay(argument).expect("qualified storage can be lent without constructing its fields");
        let mut changed = argument.clone();
        changed.path.clear();
        assert!(
            replay(&changed).is_err(),
            "the array field cannot become the whole root"
        );
        let mut changed = argument.clone();
        changed.type_identity.push_str("::Other");
        assert!(
            replay(&changed).is_err(),
            "the collection type must remain exact"
        );
        let mut changed = argument.clone();
        changed.access = if mutable {
            checked_trees::CheckedStructuralAccess::SharedBorrow
        } else {
            checked_trees::CheckedStructuralAccess::MutableBorrow
        };
        assert!(replay(&changed).is_err(), "the loan access cannot change");
    }
}

#[test]
fn projected_array_reads_require_their_dominating_case_selection() {
    let checked = crate::front_end::checked_program(
        "data Packet { case Empty; case Samples(values: [[i32; 2]; 2]); }
        data Root { packet: Packet; }
        machine read(root: &Root) -> i32 {
            transition root.packet {
                Packet::Empty -> finish(0)
                Packet::Samples { values } -> finish(values[1][0])
            }
            state finish(value: i32) -> i32 { value }
        }",
    );
    let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("read"))
        .expect("nested arrays compose with payload selection");
    let verify = |module: &terminal_psi::TerminalModule| {
        terminal_verifier::verify_module(
            module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .map(|_| ())
    };
    verify(&lowered.semantic_module).expect("selected payload copy and primitive read verify");
    let mut hoisted = lowered.semantic_module.clone();
    let machine = hoisted
        .machines
        .iter_mut()
        .find(|machine| machine.id == hoisted.entry)
        .unwrap();
    let (block, ordinal) = machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, candidate)| {
            candidate
                .operations
                .iter()
                .position(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::StructuralCaseLeafCopy { .. }
                    )
                })
                .map(|ordinal| (block, ordinal))
        })
        .expect("case-authorized carrier copy");
    let copy = machine.blocks[block].operations.remove(ordinal);
    let entry = machine
        .blocks
        .iter_mut()
        .find(|block| block.id == machine.entry)
        .unwrap();
    entry.operations.insert(0, copy);
    assert!(
        verify(&hoisted).is_err(),
        "copying above the case test cannot grant payload authority"
    );
}

#[test]
fn case_array_operands_rejoin_their_authored_declaration_and_leaf_order() {
    let original = crate::front_end::checked_program(
        "data Packet { case Empty; case Samples(values: [[i32; 2]; 2]); }
        machine make(seed: i32) -> Packet {
            Packet::Samples { values: [[seed, 20], [30, 40]] }
        }",
    );
    let lowered = crate::lower_machine(&original, crate::TerminalMachineSelection::Name("make"))
        .expect("array construction does not depend on its enclosing constructor");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("case array verifies independently");

    let mut forged = original.clone();
    let (array, _) = forged
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .find(|(_, node)| matches!(node.kind, CheckedStructuralValueKind::ScalarArray { .. }))
        .expect("scalar array producer");
    let CheckedStructuralValueKind::ScalarArray { elements } = &mut forged
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(array)
        .kind
    else {
        unreachable!()
    };
    elements.swap(0, 3);
    assert!(
        crate::lower_machine(&forged, crate::TerminalMachineSelection::Name("make")).is_err(),
        "same-type leaf substitution must not pass replay"
    );
}

#[test]
fn assignment_replay_rejects_erased_arithmetic_policy() {
    let original = crate::front_end::checked_program(
        "data Buffers {
            source: [i32; 3] in Wrapping;
            destination: [i32; 3] in Wrapping;
        }
        machine copy(buffers: &mut Buffers) {
            buffers.destination = buffers.source;
        }",
    );
    let (machine, state, _) = operation(&original);
    let mut operation = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("array assignment has a Unit plan")
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            )
        })
        .expect("assignment establishes a copied array")
        .clone();
    validate(&original, machine, state, &operation).expect("exact assignment destination");
    let (root_handle, root) = original
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .next()
        .expect("assignment value root");
    let erased = validation::unwrapped_type_reference(&original.typed, root.type_reference)
        .expect("array carrier beneath the policy");
    assert_ne!(
        original.normalized_type_identity(root.type_reference),
        original.normalized_type_identity(erased),
        "the negative control must change semantic type, not merely its handle"
    );
    let mut forged = original.clone();
    forged
        .facts
        .values
        .structural_values
        .roots
        .get_mut(root_handle)
        .type_reference = erased;
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } = &mut operation
    else {
        panic!("expected structural value establishment");
    };
    result.type_identity = original.normalized_type_identity(erased).into_string();
    let error = validate(&forged, machine, state, &operation)
        .expect_err("internally matching forged identities cannot replace the authored type");
    assert!(
        error
            .to_string()
            .contains("substituted its owner or result type"),
        "independent destination reconstruction must reject the substitution: {error}"
    );
}

#[test]
fn record_replay_rejects_swapped_same_carrier_fields_and_operands() {
    let original = crate::front_end::checked_program(
        "data Pair[copy] { first: u64; second: u64; }
        machine make(first: u64, second: u64) -> Pair {
            Pair { second: second, first: first }
        }",
    );
    let (machine, state, operation) = operation(&original);
    validate(&original, machine, state, &operation).expect("authored record");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        panic!("construction");
    };
    let CheckedStructuralValueKind::Record { fields, .. } = original
        .facts
        .values
        .structural_values
        .nodes
        .get(value)
        .kind
    else {
        panic!("record");
    };
    let first = fields.start();
    let second = arena::Handle::from_parts(first.arena_index() + 1, first.generation());
    for exchange_field_identity in [false, true] {
        let mut forged = original.clone();
        let left = forged
            .facts
            .values
            .structural_values
            .record_fields
            .get(first)
            .clone();
        let right = forged
            .facts
            .values
            .structural_values
            .record_fields
            .get(second)
            .clone();
        if exchange_field_identity {
            forged
                .facts
                .values
                .structural_values
                .record_fields
                .get_mut(first)
                .field = right.field;
            forged
                .facts
                .values
                .structural_values
                .record_fields
                .get_mut(second)
                .field = left.field;
        } else {
            forged
                .facts
                .values
                .structural_values
                .record_fields
                .get_mut(first)
                .value = right.value;
            forged
                .facts
                .values
                .structural_values
                .record_fields
                .get_mut(second)
                .value = left.value;
        }
        assert!(validate(&forged, machine, state, &operation).is_err());
    }
}

fn operation(
    checked: &CheckedTrees,
) -> (SymbolHandle, SymbolHandle, CheckedUnitEffectOperationPlan) {
    let (_, root) = checked
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .next()
        .expect("fresh value root");
    (
        root.machine,
        root.state,
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: checked_trees::CheckedUnitStructuralResultBindingPlan {
                statement_index: root.statement_ordinal,
                binding_ordinal: 0,
                type_identity: checked
                    .normalized_type_identity(root.type_reference)
                    .into_string(),
                multiplicity: checked.type_multiplicity(root.type_reference),
            },
            value: root.root,
            calls: Vec::new(),
            operand_source: None,
            discard_result_on_return: false,
        },
    )
}

#[test]
fn structural_replay_rejects_substituted_integer_pattern_payload() {
    let mut checked = checked();
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).unwrap();
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        panic!("structural value");
    };
    let CheckedStructuralValueKind::Dispatch { arms, .. } =
        checked.facts.values.structural_values.nodes.get(value).kind
    else {
        panic!("dispatch");
    };
    let CheckedScalarDispatchPattern::Value(pattern) = checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span(arms)
        .unwrap()[0]
        .pattern
    else {
        panic!("value pattern");
    };
    let checked_trees::CheckedScalarComputationKind::Value(
        checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
    ) = &mut checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(pattern)
        .kind
    else {
        panic!("retained integer pattern");
    };
    *literal =
        numerics::literals::IntegerLiteral::from_value(17).with_landing(literal.landing().unwrap());
    assert!(validate(&checked, machine, state, &operation).is_err());
}

#[test]
fn structural_replay_rejects_swapped_case_results_and_missing_coverage() {
    let mut checked = checked();
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).expect("original source correspondence");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        panic!("value operation");
    };
    let CheckedStructuralValueKind::Dispatch { arms, .. } =
        checked.facts.values.structural_values.nodes.get(value).kind
    else {
        panic!("match");
    };
    let original = checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span(arms)
        .unwrap()
        .to_vec();
    let retained = checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span_mut(arms)
        .unwrap();
    retained[0].value = original[1].value;
    retained[1].value = original[0].value;
    assert!(validate(&checked, machine, state, &operation).is_err());
    checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span_mut(arms)
        .unwrap()
        .clone_from_slice(&original);
    let CheckedStructuralValueKind::Dispatch { arms, .. } = &mut checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(value)
        .kind
    else {
        panic!("match");
    };
    *arms = arena::HandleSpan::from_parts(arms.start(), 1);
    assert!(validate(&checked, machine, state, &operation).is_err());
}

#[test]
fn nested_call_replay_rejects_an_equal_row_outside_the_owning_state() {
    let mut checked = crate::front_end::checked_program(
        "data Inner { value: u64; } data Outer { child: Inner; }
        machine make_child(value: u64) -> Inner { Inner { value: value } }
        machine make() -> u64 { let outer: Outer = Outer { child: make_child(7) }; outer.child.value }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "make")
        .unwrap();
    let symbol = machine.symbol;
    let state = checked.machine_states(machine)[0].symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .expect("ordinary nested constructor");
    let operation = plan
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            )
        })
        .unwrap()
        .clone();
    validate(&checked, symbol, state, &operation).expect("actual nested call occurrence");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { calls, .. } = &operation else {
        unreachable!()
    };
    let [call] = calls.as_slice() else {
        panic!("one nested call")
    };
    let handle = call.value;
    let CheckedStructuralValueKind::Call { source_call } = checked
        .facts
        .values
        .structural_values
        .nodes
        .get(handle)
        .kind
    else {
        panic!("call value")
    };
    let equal_row = checked.facts.flow.control.calls.get(source_call).clone();
    let outside_state = checked.facts.flow.control.calls.append(equal_row);
    checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(handle)
        .kind = CheckedStructuralValueKind::Call {
        source_call: outside_state,
    };
    assert!(
        validate(&checked, symbol, state, &operation)
            .unwrap_err()
            .to_string()
            .contains("outside its source state")
    );
}

#[test]
fn owned_record_child_replay_rejects_same_carrier_parameter_substitution() {
    let mut checked = crate::front_end::checked_program(
        "data Inner { value: u64; } data Outer { child: Inner; }
        machine wrap(first: Inner, second: Inner) -> Outer { Outer { child: first } }",
    );
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).unwrap();
    let value = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .find_map(|(handle, value)| {
            matches!(value.kind, CheckedStructuralValueKind::Place(_)).then_some(handle)
        })
        .unwrap();
    let CheckedStructuralValueKind::Place(argument) = &mut checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(value)
        .kind
    else {
        unreachable!()
    };
    argument.source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    assert!(
        validate(&checked, machine, state, &operation)
            .unwrap_err()
            .to_string()
            .contains("substituted its parameter")
    );
}

/// A whole affine carrier carrying `[linear]` children joins through the same
/// ownership receipt: the transfer's claim set discharges every frontier
/// child's exact claim on its edge. The recorded receipt replays cleanly, and
/// a rewritten claim path, a swapped provenance, a dropped claim, or a
/// duplicated claim row all disagree with the independently re-derived
/// frontier. Claim identity itself is minted at checking, so the replayable
/// contract here is path composition, count, and provenance — the checker's
/// own replay owns identity comparison.
#[test]
fn carrier_selection_receipt_replay_rejects_mutated_claim_sets() {
    use language_semantics::PermissionProvenance;
    let checked = crate::front_end::checked_program(
        "data Token [linear] { code: u64; }
         data Holder { left: Token; right: Token; }
         boundary requirement Token::retire(self);
         machine Token::settle(self) { Token::retire(self); }
         machine choose(selected: bool, x: Holder) -> u64 {
             let picked: Holder = match selected { true -> x, false -> x };
             Token::settle(picked.left);
             Token::settle(picked.right);
             0
         }",
    );
    let receipt = checked
        .facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .next()
        .map(|(_, receipt)| receipt.clone())
        .expect("carrier selection receipt");
    super::owned_selection::validate_receipt(
        &checked,
        receipt.machine,
        receipt.state,
        receipt.statement_ordinal,
        &receipt,
    )
    .expect("the recorded carrier receipt replays");
    for mutation in 0..4 {
        let mut changed = checked.clone();
        let ownership = &mut changed.facts.flow.ownership;
        let transfer = receipt.transfers.start();
        let claims = ownership.selection_transfers.get(transfer).claims;
        match mutation {
            // A claim's consumed path is exact source-place identity.
            0 => {
                let path = ownership.selection_transfer_claims.get(claims.start()).path;
                *ownership.segments.get_mut(path.start()) =
                    facts::PlaceSegment::FixedIndex { index: 0 };
            }
            // A duplicated row can never match the distinct second frontier
            // path the leaf type requires.
            1 => {
                let first = ownership.selection_transfer_claims.get(claims.start()).path;
                ownership
                    .selection_transfer_claims
                    .get_mut(arena::Handle::from_parts(
                        claims.start().arena_index() + 1,
                        claims.start().generation(),
                    ))
                    .path = first;
            }
            // The consumed place's establishment provenance rides the row.
            2 => {
                ownership
                    .selection_transfer_claims
                    .get_mut(claims.start())
                    .provenance = PermissionProvenance::Unknown;
            }
            // Dropping a claim silently keeps one child live to scope exit.
            _ => {
                ownership.selection_transfers.get_mut(transfer).claims = arena::HandleSpan::empty();
            }
        }
        assert!(
            super::owned_selection::validate_receipt(
                &changed,
                receipt.machine,
                receipt.state,
                receipt.statement_ordinal,
                &receipt,
            )
            .is_err(),
            "claim mutation {mutation} accepted"
        );
    }
}

/// Retain the actual call-argument establishments: their coordinates belong to
/// the checked caller, not to a test-manufactured construction operation.
fn sibling_argument_constructions() -> (
    CheckedTrees,
    SymbolHandle,
    SymbolHandle,
    Vec<CheckedUnitEffectOperationPlan>,
) {
    let source = "data Input { bytes: [u8; 4]; value: u64; }
         data Inner [copy] { case Yes; case No; }
         data Choice [copy] { case Selected(inner: Inner); case Empty; }
         data Output { value: u64; }
         machine combine(input: Input, choice: Choice) -> Output {
             Output { value: input.value }
         }
         machine make() {
             let output: Output = combine(
                 Input { value: 7 },
                 Choice::Selected { inner: Inner::Yes }
             );
             transition output.value == 7 {
                 true -> done()
                 false -> done()
             }
             state done() {}
         }";
    let checked = crate::front_end::checked_program(source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "make")
        .expect("authored caller");
    let symbol = machine.symbol;
    let state = checked.machine_states(machine)[0].symbol;
    let plans = &checked.facts.flow.terminal_unit_effects;
    let operations = if let Some(plan) = plans.for_machine(symbol) {
        &plan.operations
    } else {
        &plans
            .composed_for_machine(symbol)
            .unwrap_or_else(|| panic!("retained caller graph missing: {:?}", plans.omissions))
            .states
            .iter()
            .find(|plan| plan.state == state)
            .expect("retained caller entry")
            .operations
    };
    let arguments = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    operand_source: Some(
                        checked_trees::CheckedArrayConstructionSource::CallArgument { .. }
                    ),
                    ..
                }
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(arguments.len(), 2, "both authored argument constructors");
    let coordinates = arguments
        .iter()
        .map(|operation| {
            let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                operand_source:
                    Some(checked_trees::CheckedArrayConstructionSource::CallArgument {
                        call_ordinal,
                        parameter_position,
                    }),
                ..
            } = operation
            else {
                unreachable!()
            };
            (result.statement_index, *call_ordinal, *parameter_position)
        })
        .collect::<Vec<_>>();
    assert!(
        coordinates
            .iter()
            .all(|coordinate| coordinate.0 == coordinates[0].0)
    );
    assert_eq!(coordinates[0].1, coordinates[1].1);
    assert_eq!((coordinates[0].2, coordinates[1].2), (0, 1));
    (checked, symbol, state, arguments)
}

#[test]
fn sibling_argument_constructors_keep_disjoint_scalar_root_scopes() {
    let (checked, machine, state, arguments) = sibling_argument_constructions();
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, result, .. } =
        &arguments[0]
    else {
        unreachable!()
    };
    let constructor = checked.facts.values.structural_values.nodes.get(*value);
    assert!(matches!(
        constructor.kind,
        CheckedStructuralValueKind::Record { .. }
    ));
    let roots = checked.facts.values.scalar_computations.roots.iter()
        .map(|(_, root)| root)
        .filter(|root| root.state == state && root.statement_ordinal == result.statement_index
            && matches!(root.role, checked_trees::CheckedScalarExpressionRole::RecordField { expression, .. }
                if expression == constructor.expression))
        .collect::<Vec<_>>();
    assert_eq!(
        roots.len(),
        1,
        "only the explicit value has an authored root"
    );
    assert!(
        checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(roots[0].root)
            .authored_root
            .is_valid()
    );
    for argument in &arguments {
        validate(&checked, machine, state, argument)
            .expect("each constructor owns its scalar roots, with siblings independently retained");
    }
}

#[test]
fn omitted_array_zero_remains_owned_and_validated_by_its_construction() {
    let (checked, machine, state, arguments) = sibling_argument_constructions();
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = arguments[0]
    else {
        unreachable!()
    };
    let plans = &checked.facts.values.structural_values;
    let CheckedStructuralValueKind::Record { fields, .. } = plans.nodes.get(value).kind else {
        panic!("record argument");
    };
    let (array, element) = plans
        .record_fields
        .span(fields)
        .unwrap()
        .iter()
        .find_map(|field| {
            let checked_trees::CheckedStructuralRecordFieldValue::Structural(array) = field.value
            else {
                return None;
            };
            let CheckedStructuralValueKind::ZeroedScalarArray { element, .. } =
                plans.nodes.get(array).kind
            else {
                return None;
            };
            Some((array, element))
        })
        .expect("omitted byte array");
    assert!(
        !checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, root)| root.root == element),
        "a synthesized literal is not an authored scalar root"
    );
    validate(&checked, machine, state, &arguments[0]).expect("declared zeroed array");
    for mutation in 0..3 {
        let mut forged = checked.clone();
        match mutation {
            0 => {
                let checked_trees::CheckedScalarComputationKind::Value(
                    checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                ) = &mut forged
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(element)
                    .kind
                else {
                    panic!("synthesized literal");
                };
                *literal = numerics::literals::IntegerLiteral::from_value(1)
                    .with_landing(literal.landing().unwrap());
            }
            1 => {
                let CheckedStructuralValueKind::ZeroedScalarArray { element_count, .. } =
                    &mut forged
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get_mut(array)
                        .kind
                else {
                    unreachable!()
                };
                *element_count += 1;
            }
            _ => {
                forged
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(element)
                    .primitive_type = checked_trees::types::PrimitiveType::U64
            }
        }
        assert!(
            validate(&forged, machine, state, &arguments[0]).is_err(),
            "zeroed array accepted mutation {mutation}"
        );
    }
}

#[test]
fn sibling_argument_root_partition_rejects_injected_and_duplicate_roles() {
    let (checked, machine, state, arguments) = sibling_argument_constructions();
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, result, .. } =
        &arguments[0]
    else {
        unreachable!()
    };
    let constructor = checked
        .facts
        .values
        .structural_values
        .nodes
        .get(*value)
        .expression;
    let retained = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| {
            root.state == state
                && root.statement_ordinal == result.statement_index
                && matches!(root.role, checked_trees::CheckedScalarExpressionRole::RecordField {
                expression, field_ordinal: 0,
            } if expression == constructor)
        })
        .expect("retained authored scalar field")
        .clone();
    for mutation in 0..4 {
        let mut forged = checked.clone();
        let mut extra = retained.clone();
        extra.role = match mutation {
            0 => checked_trees::CheckedScalarExpressionRole::RecordField {
                expression: constructor,
                field_ordinal: 99,
            },
            1 => checked_trees::CheckedScalarExpressionRole::RecordField {
                expression: arena::Handle::invalid(),
                field_ordinal: 0,
            },
            2 => retained.role,
            _ => checked_trees::CheckedScalarExpressionRole::RecordField {
                expression: constructor,
                field_ordinal: 1,
            },
        };
        forged.facts.values.scalar_computations.roots.append(extra);
        assert!(
            validate(&forged, machine, state, &arguments[0]).is_err(),
            "current constructor accepted root mutation {mutation}"
        );
        if mutation == 1 || mutation == 2 {
            assert!(
                validate(&forged, machine, state, &arguments[1]).is_err(),
                "sibling scope hid unauthored or duplicated root mutation {mutation}"
            );
        } else {
            validate(&forged, machine, state, &arguments[1])
                .expect("a delegated field ordinal is checked by its owning constructor replay");
        }
    }
}

#[test]
fn representation_view_region_replays_exact_offset_occurrence_and_extent() {
    use checked_trees::{CheckedElementViewRegion, CheckedScalarExpressionRole};
    let checked = crate::front_end::checked_program(
        "data Storage where offset <= 3 { bytes: [u8; 8]; offset: u32; }
         machine run(storage: &mut Storage) {
             storage.offset = 2;
             let view: &mut [u16] = &mut storage.bytes[storage.offset * 2] as &mut [u16];
         }",
    );
    let values = &checked.facts.values.structural_values;
    let (root, argument, region) = values
        .roots
        .iter()
        .find_map(|(_, root)| match &values.nodes.get(root.root).kind {
            CheckedStructuralValueKind::MutableSliceView {
                source,
                region: region @ CheckedElementViewRegion::RepresentationBytes { .. },
            } => Some((root.clone(), source.clone(), *region)),
            _ => None,
        })
        .expect("retained representation region");
    let replay = |checked: &CheckedTrees, region| {
        let (machine, state) = super::super::authored_state(checked, root.state).unwrap();
        super::slice_view::validate(
            checked,
            machine,
            state,
            root.statement_ordinal,
            root.expression,
            &argument,
            crate::expression_preparation::bindings::LoanAccess::Mutable,
            region,
        )
    };
    replay(&checked, region).expect("authored composite offset and collection extent replay");
    let CheckedElementViewRegion::RepresentationBytes { start, end } = region else {
        unreachable!()
    };
    assert!(
        replay(
            &checked,
            CheckedElementViewRegion::RepresentationBytes {
                start,
                end: end - 1
            }
        )
        .is_err()
    );
    assert!(replay(&checked, CheckedElementViewRegion::WholeCollection).is_err());
    assert!(
        replay(
            &checked,
            CheckedElementViewRegion::RepresentationBytes {
                start: arena::Handle::invalid(),
                end
            }
        )
        .is_err()
    );
    let mut substituted = checked.clone();
    substituted
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(start)
        .authored_root = root.expression;
    assert!(
        replay(&substituted, region).is_err(),
        "offset computation cannot name the enclosing view"
    );
    let role = CheckedScalarExpressionRole::RepresentationViewOffset {
        expression: root.expression,
    };
    assert!(
        super::operand_source(&checked, root.state, root.statement_ordinal - 1, role).is_err(),
        "a valid arena handle from another statement supplies no source occurrence"
    );
    assert!(
        super::operand_source(
            &checked,
            root.state,
            root.statement_ordinal,
            CheckedScalarExpressionRole::RepresentationViewOffset {
                expression: checked
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(start)
                    .authored_root
            }
        )
        .is_err(),
        "only the authored recast owns this operand role"
    );
}

#[test]
fn mutable_byte_region_establishment_replays_carrier_and_affine_custody() {
    let checked = crate::front_end::checked_program(
        "data Storage where offset <= 4 { bytes: [u8; 8]; offset: u32; }
         machine run(storage: &mut Storage) {
             let view: &mut [u8] = &mut storage.bytes[storage.offset] as &mut [u8];
             transition view.len > 0 {
                 true -> finish(view)
                 _ -> empty()
             }
             state finish(view: &mut [u8]) {}
             state empty() {}
         }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let state = checked.machine_states(machine)[0].symbol;
    let plans = &checked.facts.flow.terminal_unit_effects;
    let graph = plans
        .composed_for_machine(machine.symbol)
        .expect("retained view forwarding graph");
    let operation = graph
        .states
        .iter()
        .find(|row| row.state == state)
        .unwrap()
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            )
        })
        .expect("ordinary descriptor establishment");
    validate(&checked, machine.symbol, state, operation)
        .expect("mutable byte descriptor retains its slice carrier");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        unreachable!()
    };
    let root = checked
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .find(|(_, root)| root.root == *value)
        .map(|(_, root)| root)
        .unwrap();
    for mutation in 0..2 {
        let mut forged = operation.clone();
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } = &mut forged
        else {
            unreachable!()
        };
        if mutation == 0 {
            result.type_identity = checked
                .normalized_type_identity(root.type_reference)
                .into_string();
        } else {
            result.multiplicity = language_semantics::Multiplicity::Unrestricted;
        }
        assert!(
            validate(&checked, machine.symbol, state, &forged).is_err(),
            "the authored reference shell cannot replace descriptor storage or erase affine custody"
        );
    }
    let mut forged = checked.clone();
    let CheckedStructuralValueKind::MutableSliceView { source, .. } = &mut forged
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(*value)
        .kind
    else {
        panic!("mutable region")
    };
    source.access = checked_trees::CheckedStructuralAccess::SharedBorrow;
    assert!(
        validate(&forged, machine.symbol, state, operation).is_err(),
        "the captured source access remains exact"
    );
}

#[test]
fn erased_record_initializers_retain_independent_semantic_replay() {
    let original = crate::front_end::checked_program(
        "data Evidence { tag: i32; }
         data Carrier { payload: i32; proof [erased]: Evidence; }
         machine make() -> Carrier {
             Carrier { payload: 1, proof: Evidence { tag: 17 } }
         }",
    );
    let (machine, state, operation) = operation(&original);
    validate(&original, machine, state, &operation)
        .expect("erased initializer retains its authored semantic computation");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = &operation else {
        panic!("record construction");
    };
    let plans = &original.facts.values.structural_values;
    let CheckedStructuralValueKind::Record { fields, .. } = plans.nodes.get(*value).kind else {
        panic!("outer record");
    };
    let proof_field = arena::Handle::from_parts(
        fields.start().arena_index() + 1,
        fields.start().generation(),
    );
    let proof = plans.record_fields.get(proof_field);
    let checked_trees::CheckedStructuralRecordFieldValue::Structural(evidence) = proof.value else {
        panic!("erased evidence value");
    };
    let CheckedStructuralValueKind::Record { fields, .. } = plans.nodes.get(evidence).kind else {
        panic!("evidence constructor");
    };
    let tag = plans.record_fields.get(fields.start());
    let checked_trees::CheckedStructuralRecordFieldValue::Scalar(computation) = tag.value else {
        panic!("retained proof computation");
    };
    let (computation_root, retained_root) = original
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .find(|(_, root)| root.root == computation)
        .expect("semantic scalar root exists independently of runtime emission");
    assert_eq!(
        super::operand_source(
            &original,
            state,
            retained_root.statement_ordinal,
            retained_root.role,
        )
        .expect("semantic scope reaches nested erased initializers"),
        (tag.expression, checked_trees::types::PrimitiveType::I32),
    );
    assert!(
        super::operand_source(
            &original,
            state,
            retained_root.statement_ordinal,
            checked_trees::CheckedScalarExpressionRole::RecordField {
                expression: tag.expression,
                field_ordinal: 0,
            },
        )
        .is_err(),
        "an existing literal is not a constructor scope"
    );
    for mutation in [
        "subject",
        "type",
        "cycle",
        "occurrence",
        "literal",
        "primitive",
        "root",
    ] {
        let mut changed = original.clone();
        match mutation {
            "subject" => {
                changed
                    .facts
                    .values
                    .structural_values
                    .record_fields
                    .get_mut(proof_field)
                    .expression = tag.expression;
            }
            "type" => {
                changed
                    .facts
                    .values
                    .structural_values
                    .record_fields
                    .get_mut(proof_field)
                    .type_reference = tag.type_reference;
            }
            "cycle" => {
                changed
                    .facts
                    .values
                    .structural_values
                    .record_fields
                    .get_mut(proof_field)
                    .value = checked_trees::CheckedStructuralRecordFieldValue::Structural(*value);
            }
            "occurrence" => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(computation)
                    .authored_root = proof.expression;
            }
            "literal" => {
                let checked_trees::CheckedScalarComputationKind::Value(
                    checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                ) = &mut changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(computation)
                    .kind
                else {
                    panic!("evidence integer literal");
                };
                *literal = numerics::literals::IntegerLiteral::from_value(99)
                    .with_landing(literal.landing().unwrap());
            }
            "primitive" => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(computation)
                    .primitive_type = checked_trees::types::PrimitiveType::U64;
            }
            "root" => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(computation_root)
                    .root = arena::Handle::invalid();
            }
            _ => unreachable!(),
        }
        assert!(
            validate(&changed, machine, state, &operation).is_err(),
            "erasure must not waive {mutation} custody"
        );
    }
}
