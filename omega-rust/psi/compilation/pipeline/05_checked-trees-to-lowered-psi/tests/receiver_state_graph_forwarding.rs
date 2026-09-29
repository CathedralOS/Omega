//! A multi-state scalar machine that retains borrowed `self` belongs to the
//! Unit state graph: only a single-state scalar graph retains a receiver. The
//! receiver stays on the entry roster while non-entry structural formals
//! forward across the edges that reach them. This is the
//! `SnapshotRegionFilter::get_element_count` shape: an Alignment formal rides
//! the jump into the divide state, which also reads the receiver.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::OperationKind;

#[test]
fn unread_multistate_receiver_retains_its_declared_call_signature() {
    for receiver in ["&self", "&mut self"] {
        let source = format!(
            r#"
            data Cell {{ value: i32; }}
            data Main {{ cell: Cell; }}
            machine Cell::get({receiver}) -> i32 {{
                transition {{ _ -> done() }}
                state done({receiver}) -> i32 {{ 70 }}
            }}
            machine Main::value(&mut self) -> i32 {{
                let result: i32 = self.cell.get();
                result
            }}
        "#
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Main::value"),
        )
        .unwrap_or_else(|error| {
            panic!("unread {receiver} must remain an explicit call operand: {error:?}")
        });
        let module = terminal_codec::decode_module(
            &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        )
        .unwrap();
        let proof = terminal_codec::decode_proof_bundle(
            &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
                .unwrap(),
        )
        .unwrap();
        terminal_verifier::verify_module(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("the retained receiver and its caller independently verify");
        let callee = module
            .machines
            .iter()
            .find(|machine| machine.id != module.entry)
            .expect("the caller retains a separate callee");
        let [parameter] = callee.structural_parameters.as_slice() else {
            panic!("the unread callee retains its receiver");
        };
        assert!(parameter.is_self);
        assert_eq!(
            parameter.access,
            if receiver == "&self" {
                terminal_psi::StructuralAccess::SharedBorrow
            } else {
                terminal_psi::StructuralAccess::MutableBorrow
            }
        );
        let mut forged = checked.clone();
        let callee_symbol = forged
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Cell::get")
            .unwrap()
            .symbol;
        let plan = forged
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| plan.machine == callee_symbol)
            .unwrap();
        plan.states[0].structural_parameters[0].access = if receiver == "&self" {
            checked_trees::CheckedStructuralAccess::MutableBorrow
        } else {
            checked_trees::CheckedStructuralAccess::SharedBorrow
        };
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &forged,
                TerminalMachineSelection::Name("Main::value"),
            )
            .is_err(),
            "unused receiver access must still match its declaration"
        );

        if receiver == "&mut self" {
            let invalid = source.replace("Main::value(&mut self)", "Main::value(&self)");
            assert!(
                crate::front_end::checked_program_result(&invalid).is_err(),
                "an unread mutable receiver still requires exclusive access"
            );
        }
    }
}

const SOURCE: &str = r#"
    data Alignment [copy] { bytes: u64; }
    data Filter [copy] { width: u64; }
    machine Filter::region_size(&self) -> u64 {
        self.width
    }
    // Interning `Filter` as an authored nominal type is what the real filter
    // gets from its enclosing scan types; without it the shared-receiver
    // argument cannot resolve the attachment's declared type.
    machine noop(other: Filter) -> u64 {
        42
    }
    machine Filter::count(&self, width: u64, alignment: Alignment) -> u64
    crashes Abort
    {
        transition alignment.bytes > 0 {
            true -> divide(alignment)
            false -> violated()
        }
        state violated(&self) -> u64 {
            crash Abort;
        }
        state divide(&self, alignment: Alignment) -> u64 {
            let size: u64 = self.region_size();
            transition {
                _ -> (size / alignment.bytes)
            }
        }
    }
"#;

#[test]
fn receiver_reading_state_graph_forwards_structural_formals_on_edges() {
    let checked = crate::front_end::checked_program(SOURCE);
    let count = checked
        .machines()
        .iter()
        .find(|machine| checked.symbols.display_path(machine.symbol, "::") == "Filter::count")
        .expect("count machine")
        .symbol;
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(count)
            .is_none()
            && checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_for_machine(count)
                .is_some(),
        "the Unit state graph owns the receiver-reading machine"
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Filter::count"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the receiver-reading state graph lowers its forwarded structural formals")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the forwarded state graph independently verifies");

    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("count entry machine");
    let [receiver, alignment] = entry.structural_parameters.as_slice() else {
        panic!("entry retains the receiver and the forwarded formal")
    };
    assert!(receiver.is_self);
    assert!(!alignment.is_self);
    let forwarded = alignment.place;
    let reads_alignment = entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| match &operation.kind {
                OperationKind::IntegerStructuralField { source, .. } => *source == forwarded,
                OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } => structural_arguments
                    .iter()
                    .any(|argument| argument.place == forwarded),
                _ => false,
            })
    });
    assert!(
        reads_alignment,
        "a block reads the forwarded Alignment formal"
    );
}
