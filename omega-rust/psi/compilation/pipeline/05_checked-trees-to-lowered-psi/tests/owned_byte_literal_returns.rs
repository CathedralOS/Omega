//! Literal completions construct bounded ownership with the declared byte
//! predicate; a return signature alone cannot manufacture those contents.
use checked_trees::{CheckedComposedUnitControlTerminatorPlan, CheckedUnitEffectOperationPlan};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};

#[test]
fn returned_record_reference_field_stores_preserve_the_borrowed_destination() {
    let checked = crate::front_end::checked_program(
        "domain [u8; 16]::Utf8 requires valid_utf8(self);
         data Holder { text: [u8; 16] in Utf8; sibling: u64; }
         data Bag { holders: [Holder; 2]; }
         data Context { bag: Bag; }
         machine pick(bag: &mut Bag) -> &mut Holder {
             let holders: &mut [Holder] = bag.holders.as_mut_slice();
             transition { _ -> &mut holders[1] }
         }
         machine label(short: bool) -> [u8; 16] in Utf8 {
             transition short { true -> \"Gate\" false -> \"Branch Room\" }
         }
         machine fill(bag: &mut Bag) {
             let selected: &mut Holder = pick(bag);
             selected.text = label(false);
             selected.text = label(true);
             selected.sibling = 29;
             transition { _ -> finish() }
             state finish() {}
         }
         machine Context::fill(&mut self) {
             let selected: &mut Holder = pick(&mut self.bag);
             selected.text = label(false);
             selected.text = label(true);
             selected.sibling = 29;
             transition self.bag.holders[1].text == \"Gate\"
                 && self.bag.holders[1].sibling == 29 {
                 true -> finish(70)
                 false -> finish(1)
             }
             state finish(code: u64) {}
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("fill"))
        .unwrap_or_else(|error| panic!("returned record store lowering: {error:?}"));
    let paths = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::StructuralByteSequenceFieldStore { path, .. } => {
                Some(path)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 2);
    assert!(
        paths
            .iter()
            .all(|path| path.first() == Some(&terminal_psi::StructuralPathSegment::Referent))
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("returned reference stores retain exact origin and release");

    let projected = lower_machine(&checked, TerminalMachineSelection::Name("Context::fill"))
        .unwrap_or_else(|error| panic!("explicit projected argument lowering: {error:?}"));
    terminal_verifier::verify_module(
        &projected.semantic_module,
        &projected.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("an explicitly borrowed member keeps its exact returned-reference loan");
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Context::fill"),
    )
    .produce(
        terminal_production::TerminalProductionCustody::artifact_only(
            &mut terminal_production::TerminalProductionTimings::default(),
        ),
    )
    .expect("reference field stores survive optimization and canonical publication")
    .into_artifact();

    let mut changed = checked.clone();
    let store = changed
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|machine| &mut machine.states)
        .flat_map(|state| &mut state.operations)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ByteSequenceStore(store)
                if matches!(store.root, checked_trees::CheckedStoreRoot::Local { .. }) =>
            {
                Some(store)
            }
            _ => None,
        })
        .expect("local destination stays distinct from a parameter");
    store.root = checked_trees::CheckedStoreRoot::Parameter { position: 0 };
    assert!(
        lower_machine(&changed, TerminalMachineSelection::Name("fill")).is_err(),
        "the original bag parameter cannot replace the returned holder identity"
    );
}

#[test]
fn owned_byte_literal_returns_keep_exact_source_and_result_custody() {
    for guarded in [false, true] {
        let body = if guarded {
            "transition short { true -> \"Gate\" false -> \"Branch Room\" }"
        } else {
            "\"Gate\""
        };
        let checked = crate::front_end::checked_program(&format!(
            "domain [u8; 16]::Utf8 requires valid_utf8(self);
             machine label(short: bool) -> [u8; 16] in Utf8 {{ {body} }}"
        ));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "label")
            .unwrap()
            .symbol;
        let plan_index = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .position(|plan| plan.machine == machine)
            .expect("owned literal completion retains an ordinary graph");
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("label"))
            .expect("literal completion lowers with exact bounded ownership");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("owned result predicate and capacity verify independently");
        let results = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishByteSequenceConcatenation { .. }
                )
            })
            .count();
        assert_eq!(results, if guarded { 2 } else { 1 });
        for corruption in ["bytes", "type", "qualification", "source"] {
            let mut changed = checked.clone();
            let state = &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index]
                .states[0];
            let operation = if guarded {
                let CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } =
                    &mut state.terminator
                else {
                    panic!("ordered guarded returns");
                };
                &mut return_values[0]
            } else {
                state
                    .operations
                    .last_mut()
                    .expect("direct literal producer")
            };
            let CheckedUnitEffectOperationPlan::EstablishByteSequenceLiteral {
                result,
                bytes,
                qualifications,
            } = operation
            else {
                panic!("ordinary literal establishment");
            };
            match corruption {
                "bytes" => *bytes = b"Fake".to_vec(),
                "type" => result.type_identity.push_str("-substituted"),
                "qualification" => qualifications.clear(),
                "source" => result.statement_index += 1,
                _ => unreachable!(),
            }
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name("label")).is_err(),
                "guarded={guarded}, corruption={corruption}"
            );
        }
    }
}

#[test]
fn owned_byte_call_result_field_store_rejoins_the_exact_completed_call() {
    let checked = crate::front_end::checked_program(
        "domain [u8; 16]::Utf8 requires valid_utf8(self);
         data Holder { text: [u8; 16] in Utf8; }
         machine label(short: bool) -> [u8; 16] in Utf8 {
             transition short { true -> \"Gate\" false -> \"Branch Room\" }
         }
         machine fill(holder: &mut Holder) {
             holder.text = label(false);
             holder.text = label(true);
             transition { _ -> finish() }
             state finish() {}
         }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "fill")
        .unwrap()
        .symbol;
    let plans = &checked.facts.flow.terminal_unit_effects.composed_machines;
    let plan_index = plans
        .iter()
        .position(|plan| plan.machine == machine)
        .unwrap_or_else(|| {
            panic!(
                "caller keeps its effect graph: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    let plan = &plans[plan_index];
    let (state_index, state) = plan
        .states
        .iter()
        .enumerate()
        .find(|(_, state)| {
            state
                .operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
                    )
                })
                .count()
                == 2
        })
        .expect("both assignments retain their byte-store source");
    let stores = state
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            let CheckedUnitEffectOperationPlan::ByteSequenceStore(store) = operation else {
                return None;
            };
            let checked_trees::CheckedByteSequenceValue::CallResult {
                binding_ordinal, ..
            } = store.value.nodes.get(store.value.root)
            else {
                panic!("completed call source");
            };
            Some((index, *binding_ordinal))
        })
        .collect::<Vec<_>>();
    assert_ne!(
        stores[0].1, stores[1].1,
        "same-shaped calls have distinct results"
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("fill"))
        .expect("qualified owned helper results replace a byte field");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("callee predicates and exact receiving field bounds verify");
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::StructuralByteSequenceFieldStore { .. }
            ))
            .count(),
        2
    );
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .any(|machine| matches!(
                &machine.result, terminal_psi::TerminalMachineResult::Structural(result)
                    if !result.qualifications.is_empty()
            )),
        "the helper retains its declared predicate"
    );
    for corruption in [
        "state",
        "statement",
        "earlier result",
        "capacity",
        "earlier call",
    ] {
        let mut changed = checked.clone();
        let changed_store = &mut changed.facts.flow.terminal_unit_effects.composed_machines
            [plan_index]
            .states[state_index]
            .operations[stores[1].0];
        let CheckedUnitEffectOperationPlan::ByteSequenceStore(store) = changed_store else {
            panic!("second byte-field replacement");
        };
        let checked_trees::CheckedByteSequenceValue::CallResult {
            state,
            statement_index,
            binding_ordinal,
            capacity,
        } = store.value.nodes.get_mut(store.value.root)
        else {
            panic!("completed result source");
        };
        match corruption {
            "state" => {
                *state = plans
                    .iter()
                    .find(|plan| plan.machine != machine)
                    .expect("distinct helper")
                    .states[0]
                    .state
            }
            "statement" => *statement_index = 0,
            "earlier result" => *binding_ordinal = stores[0].1,
            "capacity" => *capacity -= 1,
            "earlier call" => {
                *statement_index = 0;
                *binding_ordinal = stores[0].1;
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("fill")).is_err(),
            "substituted call-result custody rejects: {corruption}"
        );
    }
}
