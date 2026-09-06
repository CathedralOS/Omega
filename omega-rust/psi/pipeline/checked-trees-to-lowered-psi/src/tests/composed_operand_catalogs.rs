//! Operand helpers coexist with dynamic realizations and closed-sum payloads.

use super::*;

fn roundtrip(checked: &CheckedTrees) -> LoweredPsi {
    let lowered = lower_machine(checked, "Main::main").expect("computed leaves lower");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("encode module");
    let evidence =
        terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    let module = terminal_codec::decode_module(&semantic).expect("decode module");
    let proof = terminal_codec::decode_proof_bundle(&evidence).expect("decode proof");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification of decoded module and evidence");
    let mut identities = module
        .machines
        .iter()
        .map(|machine| machine.id)
        .collect::<Vec<_>>();
    identities.sort();
    identities.dedup();
    assert_eq!(
        identities.len(),
        module.machines.len(),
        "machine identities remain disjoint"
    );
    lowered
}

#[test]
fn dynamic_continuation_operands_preserve_forwarding_and_helper_identities() {
    let checked = checked_source(
        r#"
        boundary trait Console {
            machine exit_process(return_code: i32) reaches Console;
        }
        trait Measure { machine measure(&self) -> i32; }
        data Item [copy] { value: i32; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 { transition { _ -> self.value } }
        }
        data Main { console: Console; selected: Item; }
        machine Main::main(&mut self) reaches Console {
            let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
            transition result == 0 { true -> good() _ -> bad() }
            state good(&mut self) { self.console.exit_process(helper(70i32)); }
            state bad(&mut self) { self.console.exit_process(helper(71i32)); }
        }
        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = finish(erased);
            transition { _ -> result }
        }
        machine finish(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
        machine helper(value: i32) -> i32 { identity(value) }
        machine identity(value: i32) -> i32
        requires 0i32 == 0i32
        ensures 0i32 == 0i32
        { value }
    "#,
    );
    let [plan] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls
        .as_slice()
    else {
        panic!("one authored dynamic scalar continuation");
    };
    assert_eq!(plan.forwarding_transfers.len(), 1);
    assert!(plan.unit_continuation.is_some());
    let lowered = roundtrip(&checked);
    assert_eq!(lowered.semantic_module.machines.len(), 6);
    assert_eq!(lowered.source_call_occurrences.len(), 8);
    let dynamic = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(dynamic.parameters.len(), 2);
    assert_eq!(dynamic.arguments.len(), 2);
    assert_eq!(dynamic.parameter_dispatches.len(), 1);
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
            .count(),
        2
    );
    assert!(
        caller.blocks.len() > 3,
        "operand evaluation retains private blocks"
    );
}

#[test]
fn closed_sum_computed_operand_keeps_payload_for_the_following_call() {
    let checked = checked_source(
        r#"
        machine identity(value: i32) -> i32 { value }
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
            machine write_byte(value: i32) reaches Console;
            machine exit_process(value: i32) reaches Console;
        }
        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            let result: ByteRead = self.console.read_byte();
            transition result {
                ByteRead::Byte { value } -> byte(value)
                ByteRead::Eof -> eof()
            }
            state byte(&mut self, value: i32 [0..=255]) {
                self.console.write_byte(identity(identity(value)));
                self.console.exit_process(value);
            }
            state eof(&mut self) { self.console.exit_process(70); }
        }
    "#,
    );
    // StructuralCase payload execution is not implemented by the interpreter;
    // codec roundtrips and independent verification cover this representation.
    let lowered = roundtrip(&checked);
    assert_eq!(lowered.semantic_module.machines.len(), 2);
    assert_eq!(lowered.source_call_occurrences.len(), 6);
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert!(matches!(
        caller.blocks[0].terminator,
        Terminator::StructuralCase { .. }
    ));
    let operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
            .count(),
        4
    );
    let (completion, arguments) = caller
        .blocks
        .iter()
        .find_map(|block| {
            let arguments = block
                .operations
                .iter()
                .filter_map(|operation| {
                    if let OperationKind::BoundaryCall { arguments, .. } = &operation.kind {
                        Some(arguments)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            (arguments.len() == 2).then_some((block, arguments))
        })
        .expect("computed write and subsequent exit share the leaf completion");
    assert_eq!(arguments[1].as_slice(), [completion.parameters[0].id]);
    assert_ne!(
        arguments[0], arguments[1],
        "later use selects retained payload, not computed operand slot"
    );
}
