use super::result_relation;
use crate::{TerminalMachineSelection, lower_machine};
use lowered_psi::LoweredPsi;
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm, ScalarType};

fn lowered(source: &str, name: &str) -> LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    lower_machine(&checked, TerminalMachineSelection::Name(name)).unwrap()
}

fn relation(lowered: &LoweredPsi) -> Option<Proposition> {
    let validated = terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
    let machine = validated.machine(lowered.semantic_module.entry).unwrap();
    result_relation(
        machine,
        &validated.value_context(machine).unwrap(),
        &mut 4096,
    )
}

#[test]
fn return_relation_preserves_wrapping_at_the_carrier_boundary() {
    let lowered = lowered(
        "machine root(n: i8 in Wrapping) -> i8 { (n * 2) as i8 }",
        "root",
    );
    let machine = &lowered.semantic_module.machines[0];
    assert!(
        machine.contract.ensures.is_empty(),
        "no caller demands a guarantee"
    );
    let candidate = relation(&lowered).expect("scalar parameter-only relation");
    let parameter = &machine.parameters[0];
    let ScalarType::Integer(integer) = parameter.scalar_type else {
        panic!("integer parameter")
    };
    let equation = Proposition::Equal(
        ScalarTerm::value(parameter.id, parameter.scalar_type),
        ScalarTerm::integer(integer, IntegerValue::Signed(100)).unwrap(),
    );
    let validated = terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
    let specialized = proof_admission::check_value_equality_denotation(
        &validated.value_context(machine).unwrap(),
        &candidate,
        [&equation],
    )
    .unwrap();
    let result = machine.result.scalar().unwrap();
    let Proposition::Equal(returned, expression) = specialized else {
        panic!("return equality")
    };
    assert_eq!(returned, ScalarTerm::value(result.id, result.scalar_type));
    assert_eq!(
        expression.integer_value(),
        Some((integer, IntegerValue::Signed(-56)))
    );
}

#[test]
fn unrelated_mutation_does_not_hide_a_scalar_parameter_relation() {
    let lowered = lowered(
        "data Main { count: i32 in Wrapping; }
         machine Main::root(&mut self, n: i32 in Wrapping) -> i32 {
             self.count = self.count + 1;
             (n * 2) as i32
         }",
        "Main::root",
    );
    assert!(relation(&lowered).is_some());
}

#[test]
fn stored_narrowing_result_supplies_the_byte_store_guarantee() {
    let lowered = lowered(
        r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Main { bytes: [u8; 3] in Utf8; byte: u8; }
        machine narrow(value: u32) -> u8 {
            (value as u8 in Wrapping) as u8
        }
        machine Main::run(&mut self) {
            self.bytes = "XXX";
            self.byte = narrow(48);
            self.bytes[0] = self.byte;
        }
        "#,
        "Main::run",
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the stored call result retains independently proved ASCII bounds");
    let guarantee = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.contract.ensures)
        .next()
        .expect("the narrowing call publishes a proved relation")
        .obligation;
    let mut missing = lowered.proof_bundle.clone();
    missing
        .evidence
        .retain(|proof| proof.obligation != guarantee);
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "an inferred guarantee cannot certify itself"
    );
}

#[test]
fn variable_narrowing_result_preserves_the_proved_digit_bound() {
    let lowered = lowered(
        r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Main { bytes: [u8; 3] in Utf8; digit: u32 in Wrapping; byte: u8; }
        machine narrow(value: u32) -> u8 {
            (value as u8 in Wrapping) as u8
        }
        machine Main::run(&mut self, value: u32 in Wrapping) {
            self.bytes = "XXX";
            self.digit = value % 10;
            self.byte = narrow((self.digit + 48) as u32);
            self.bytes[0] = self.byte;
        }
        "#,
        "Main::run",
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the actual digit bound survives the narrowing call");
}

#[test]
fn trapping_conversion_result_is_proved_before_the_caller_uses_it() {
    let lowered = lowered(
        r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Main { bytes: [u8; 3] in Utf8; byte: u8; value: i32; digit: i32; }
        machine trap_if(invalid: bool) {
            let invalid_value: u32 in Wrapping = invalid as u32 in Wrapping;
            let count: u32 in Wrapping = invalid_value * 8;
            let probe: u8 in Trapping =
                (1 as u8 in Trapping) << (count as u32 in Trapping);
        }
        machine narrow(value: i32) -> u8 crashes Trap {
            let below: bool = value < 0;
            trap_if(below);
            let above: bool = value > 255;
            trap_if(above);
            (value as u8 in Trapping) as u8
        }
        machine Main::run(&mut self) {
            self.bytes = "XXX";
            self.value = 25;
            self.digit = self.value / 10;
            self.digit = self.digit + 48;
            self.byte = narrow(self.digit);
            self.bytes[0] = self.byte;
        }
        "#,
        "Main::run",
    );
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile)
        .expect("a normal trapping result retains its independently proved exact relation");
    let guarantee = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.contract.ensures)
        .next()
        .expect("the caller demands the conversion guarantee")
        .obligation;
    let mut missing = lowered.proof_bundle.clone();
    missing
        .evidence
        .retain(|proof| proof.obligation != guarantee);
    assert!(
        terminal_verifier::verify_module(&lowered.semantic_module, &missing, &profile).is_err(),
        "proposing an exact return relation cannot grant it without evidence"
    );
}

#[test]
fn trapping_return_candidate_retains_exact_not_wrapping_arithmetic() {
    let lowered = lowered(
        "machine root(value: u8 in Trapping) -> u8 { (value + 3) as u8 }",
        "root",
    );
    let candidate = relation(&lowered).expect("the normal return has an exact relation");
    let Proposition::Equal(_, ScalarTerm::ExactIntegerAdd { .. }) = candidate else {
        panic!("normal Trapping addition is exact, never a wrapped candidate: {candidate:?}")
    };
    assert!(
        lowered.semantic_module.machines[0]
            .contract
            .ensures
            .is_empty()
    );
}

#[test]
fn non_ascii_narrowing_store_is_rejected_before_lowering() {
    let diagnostics = crate::front_end::checked_program_result(
        r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Main { bytes: [u8; 3] in Utf8; byte: u8; }
        machine narrow(value: u32) -> u8 { (value as u8 in Wrapping) as u8 }
        machine Main::run(&mut self) {
            self.bytes = "XXX";
            self.byte = narrow(200);
            self.bytes[0] = self.byte;
        }
        "#,
    )
    .expect_err("the narrowing result cannot satisfy the UTF-8 field requirement");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove default-domain field requirement")
            && diagnostic.message.contains("Utf8")
    }));
}

#[test]
fn overwritten_or_unobserved_call_results_do_not_demand_guarantees() {
    for store in [
        "self.byte = 49; self.bytes[0] = self.byte;",
        "self.other = 49; self.bytes[0] = self.other;",
    ] {
        let source = format!(
            r#"
            domain [u8; 3]::Utf8 requires valid_utf8(self);
            data Main {{ bytes: [u8; 3] in Utf8; byte: u8; other: u8; }}
            machine narrow(value: u32) -> u8 {{ (value as u8 in Wrapping) as u8 }}
            machine Main::run(&mut self) {{
                self.bytes = "XXX";
                self.byte = narrow(48);
                {store}
            }}
        "#
        );
        let lowered = lowered(&source, "Main::run");
        assert!(
            lowered
                .semantic_module
                .machines
                .iter()
                .all(|machine| machine.contract.ensures.is_empty()),
            "a stale or sibling field equality must not select the call: {store}"
        );
    }
}

#[test]
fn differing_returns_and_storage_observations_do_not_become_parameter_guarantees() {
    let branched = lowered(
        "machine root(n: i32 in Wrapping, flag: bool) -> i32 {
             transition flag {
                 true -> ((n * 2) as i32)
                 false -> ((n * 3) as i32)
             }
         }",
        "root",
    );
    assert_eq!(relation(&branched), None);
    let observed = lowered(
        "data Main { value: i32; }
         machine Main::root(&self) -> i32 { self.value }",
        "Main::root",
    );
    assert_eq!(relation(&observed), None);
}

#[test]
fn unresolved_block_parameters_and_exhaustion_decline() {
    let lowered = lowered(
        "machine root(n: i32 in Wrapping, flag: bool) -> i32 {
             transition flag {
                 true -> ((n * 2) as i32)
                 false -> ((n * 2) as i32)
             }
         }",
        "root",
    );
    assert_eq!(
        relation(&lowered),
        None,
        "block-local identities cannot escape as machine parameters"
    );
    let validated = terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
    let machine = validated.machine(lowered.semantic_module.entry).unwrap();
    let mut remaining = 1;
    assert_eq!(
        result_relation(
            machine,
            &validated.value_context(machine).unwrap(),
            &mut remaining
        ),
        None
    );
    assert_eq!(remaining, 0);
}

#[test]
fn invalid_dynamic_result_proposal_preserves_the_complete_lowered_product() {
    use crate::proofs::ProofSearchDeclines;
    use semantic_vocabulary::{ObligationId, OperationId, ValueId};
    use terminal_psi::{
        ContractClause, Operation, OperationKind, OperationResult, TerminalAffineCleanupAction,
        TerminalMachineResult, Terminator,
    };

    // Lower the original stored-dispatch source before changing its Terminal
    // result producer. The source recognizer need not admit constant realizations.
    let mut lowered = lowered(
        r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] {
        value: bool;
    }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool {
            transition { _ -> self.value }
        }
    }

    data Holder<'item> {
        handler: &'item dyn Measure;
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run<'item>(&self) {
        let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
        let holder: Holder<'item> = Holder { handler: erased };
        let result: bool = holder.handler.measure();
    }
"#,
        "Main::run",
    );
    let [dispatch] = lowered
        .semantic_module
        .dynamic_dispatch
        .stored_dispatches
        .as_slice()
    else {
        panic!("one stored scalar dispatch")
    };
    let dispatch = dispatch.clone();
    let realization = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == dispatch.realization)
        .unwrap();
    let returned_values = realization
        .blocks
        .iter()
        .filter_map(|block| match block.terminator {
            Terminator::Return { value, .. } => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [returned] = returned_values.as_slice() else {
        panic!("the realization has one scalar return")
    };
    let producer = realization
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.id == *returned)
        })
        .expect("the return uses the field-read result directly");
    assert!(matches!(
        producer.kind,
        OperationKind::BooleanStructuralField { .. }
    ));
    producer.kind = OperationKind::BooleanConstant { value: true };
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the constant producer retains valid stored-dispatch custody");
    let [selection] = lowered
        .semantic_module
        .dynamic_dispatch
        .selections
        .as_slice()
    else {
        panic!("one source selection")
    };
    assert_eq!(selection.owner, dispatch.owner);
    let source = selection.source.clone();
    let operation_id = OperationId::new(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .map(|operation| operation.id.get())
            .max()
            .unwrap()
            .checked_add(1)
            .unwrap(),
    )
    .unwrap();
    let caller_obligation = ObligationId::new(
        terminal_verifier::maximum_registered_obligation_id(&lowered.semantic_module)
            .unwrap()
            .checked_add(1)
            .unwrap(),
    )
    .unwrap();
    let maximum_value = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|caller| {
            caller
                .parameters
                .iter()
                .chain(&caller.contract.erased_scalar_formals)
                .chain(caller.result.scalar_ref())
                .chain(caller.blocks.iter().flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .chain(&block.erased_scalar_formals)
                        .chain(
                            block
                                .operations
                                .iter()
                                .filter_map(|operation| operation.result.scalar_ref()),
                        )
                }))
        })
        .map(|value| value.id.get())
        .max()
        .unwrap();
    let caller = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == dispatch.owner)
        .unwrap();
    assert_eq!(caller.result, TerminalMachineResult::Unit);
    let block = caller
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .operations
                .iter()
                .any(|operation| operation.id == dispatch.operation)
        })
        .unwrap();
    let stored_call = block
        .operations
        .iter()
        .find(|operation| operation.id == dispatch.operation)
        .unwrap();
    assert!(matches!(
        stored_call.kind,
        OperationKind::CallDynamicScalar { .. }
    ));
    let mut call_result = stored_call.result.scalar().unwrap();
    call_result.id = ValueId::new(maximum_value.checked_add(1).unwrap()).unwrap();
    let mut caller_result = call_result;
    caller_result.id = ValueId::new(maximum_value.checked_add(2).unwrap()).unwrap();
    let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards,
    } = &block.terminator
    else {
        panic!("the stored call has one normal Unit return")
    };
    let terminator = Terminator::Return {
        edge: *edge,
        value: call_result.id,
        cleanup_actions: trivial_affine_discards
            .iter()
            .copied()
            .map(TerminalAffineCleanupAction::DiscardRoot)
            .collect(),
    };

    // An ordinary call creates proof demand for this same realization. The
    // original stored call and all of its descriptor custody remain intact.
    block.operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id,
        result: OperationResult::Scalar(call_result),
        kind: OperationKind::CallStructuralScalar {
            callee: dispatch.realization,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![source],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    block.terminator = terminator;
    caller.result = TerminalMachineResult::Scalar(caller_result);
    let result_term = ScalarTerm::value(caller_result.id, caller_result.scalar_type);
    caller.contract.ensures.push(ContractClause {
        proposition: Proposition::Equal(result_term.clone(), result_term),
        obligation: caller_obligation,
    });
    let original = terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module)
        .expect("the baseline with both calls is valid");
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .all(|proof| proof.obligation != caller_obligation),
        "the new reflexive caller guarantee remains an unsupplied demand"
    );
    let mut remaining = 4096;
    assert_eq!(
        super::demanded_callees(&lowered, &original, &mut remaining),
        std::collections::BTreeSet::from([dispatch.realization]),
        "the transaction must reach the stored realization, not decline before proposing"
    );
    let validated = terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
    let realization = validated.machine(dispatch.realization).unwrap();
    assert!(realization.contract.ensures.is_empty());
    let candidate = super::result_relation(
        realization,
        &validated.value_context(realization).unwrap(),
        &mut remaining,
    )
    .expect("the demanded constant realization produces a parameter-only relation");
    assert!(
        remaining > 0,
        "this witness does not exhaust inference capacity"
    );

    // Establish the exact failure boundary independently of the transaction.
    let mut proposal = lowered.semantic_module.clone();
    let proposal_obligation = ObligationId::new(
        terminal_verifier::maximum_registered_obligation_id(&proposal)
            .unwrap()
            .checked_add(1)
            .unwrap(),
    )
    .unwrap();
    proposal
        .machines
        .iter_mut()
        .find(|machine| machine.id == dispatch.realization)
        .unwrap()
        .contract
        .ensures
        .push(ContractClause {
            proposition: candidate,
            obligation: proposal_obligation,
        });
    assert!(matches!(
        terminal_verifier::validate_module(&proposal),
        Err(terminal_verifier::ModuleError::InvalidStoredDynamicDispatch { owner, operation })
            if owner == dispatch.owner && operation == dispatch.operation
    ));

    let baseline = lowered.clone();
    super::retain_provable(&mut lowered, &mut ProofSearchDeclines::default())
        .expect("an invalid optional proposal must roll back rather than reject the baseline");
    assert_eq!(
        lowered, baseline,
        "all semantic, proof and custody records survive rollback"
    );
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("rollback restores the valid baseline");
}
