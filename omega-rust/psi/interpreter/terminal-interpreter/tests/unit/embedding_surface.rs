//! The public embedding surface: one reusable admitted program, per-instance
//! import bindings, invocation, and close. Host objects here are checked
//! adapters bound to exact slots — not the raw effect hook exercised by
//! `embedding_lifetimes.rs`, which keeps covering the fuel and custody
//! mechanics beneath this layer.

use super::{
    AdmissionProfile, IntegerSign, IntegerType, IntegerValue, OperationKind, OperationResult,
    ProofBundle, TerminalAdapterError, TerminalBindingRejection, TerminalEffectRejection,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter, TerminalImportSignature,
    TerminalInstanceCloseError, TerminalInstanceError, TerminalInterpretError,
    TerminalMachineResult, TerminalProgram, TerminalScalarImport, TerminalScalarValue,
    TerminalStructuralInputs, TerminalUnitImport, Terminator, ValueDeclaration, boundary_id,
    edge_id, encode_module, encode_proof_section, operation_id, scalar_boundary_effect_module,
    value_id,
};
use semantic_vocabulary::ScalarType;
use terminal_psi::BoundaryMachineResult;

fn load(module: &terminal_psi::TerminalModule) -> TerminalProgram {
    let semantic = encode_module(module).expect("fixture semantics encode");
    let proof =
        encode_proof_section(module, &ProofBundle::default()).expect("empty evidence is bound");
    // The module, the two byte buffers and the profile all die here: the
    // program retains the admitted content, not the provenance inputs.
    TerminalProgram::load(&semantic, &proof, &AdmissionProfile::default())
        .expect("source-free fixture is independently checked")
}

/// The scalar boundary fixture plus a second demanded slot carrying the same
/// signature shape under a different identity — the occurrence roster where
/// "same requirement" does not mean "same binding".
fn two_import_sections() -> (Vec<u8>, Vec<u8>) {
    let mut module = scalar_boundary_effect_module();
    let mut second = module.boundary_machines[0].clone();
    second.id = boundary_id(2);
    second.identity = "test::other".into();
    module.boundary_machines.push(second);
    let mut call = module.machines[0].blocks[0].operations[2].clone();
    call.id = operation_id(4);
    let OperationKind::BoundaryCall { boundary, .. } = &mut call.kind else {
        unreachable!("scalar boundary fixture operation");
    };
    *boundary = boundary_id(2);
    module.machines[0].blocks[0].operations.push(call);
    (
        encode_module(&module).expect("fixture semantics encode"),
        encode_proof_section(&module, &ProofBundle::default()).expect("empty evidence is bound"),
    )
}

#[test]
fn program_admits_once_and_describes_demanded_imports_exactly() {
    let program = load(&scalar_boundary_effect_module());
    let imports = program.imports().collect::<Vec<_>>();
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0].id, boundary_id(1));
    assert_eq!(imports[0].identity, "test::observe");
    assert_eq!(imports[0].scalar_parameters, [ScalarType::Boolean; 2]);
    assert_eq!(
        program.import(boundary_id(1)).map(|d| d.id),
        Some(boundary_id(1))
    );
    assert!(program.import(boundary_id(9)).is_none());
    assert_eq!(
        program.import_by_identity("test::observe"),
        Some(boundary_id(1))
    );
    assert_eq!(program.import_by_identity("test::missing"), None);
}

#[test]
fn unbound_demanded_import_rejects_invocation_before_any_effect() {
    let program = load(&scalar_boundary_effect_module());
    // Host state an adapter borrows must be declared before the instance
    // that holds the borrow.
    let mut observed = Vec::new();
    let mut instance = program
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    assert_eq!(
        instance.unbound_imports().collect::<Vec<_>>(),
        [boundary_id(1)]
    );
    assert_eq!(
        instance.invoke(&mut TerminalFuelMeter::unbounded()),
        Err(TerminalInstanceError::UnboundImports {
            missing: vec![boundary_id(1)],
        })
    );
    assert!(
        instance.effects().is_empty(),
        "no guest operation ran for the unbound roster"
    );
    // Binding the demanded import then lets the same invocation proceed —
    // the failed invoke committed nothing.
    let import = program.import(boundary_id(1)).unwrap().clone();
    instance
        .bind_import(
            boundary_id(1),
            TerminalUnitImport::for_import(&import, |arguments| {
                observed.push(arguments.to_vec());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
    assert!(instance.unbound_imports().next().is_none());
    instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .unwrap();
    instance.close().unwrap();
    // Closing consumed the roster; dropping the instance ends the adapter's
    // borrow of host state so the recording is readable again.
    drop(instance);
    assert_eq!(
        observed,
        [vec![
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false)
        ]]
    );
}

#[test]
fn wrong_unknown_and_duplicate_bindings_reject_without_effects() {
    let program = load(&scalar_boundary_effect_module());
    let mut observed = Vec::new();
    let mut instance = program
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    let import = program.import(boundary_id(1)).unwrap().clone();

    // A slot this program never declared is unknown, not merely unbound.
    assert_eq!(
        instance.bind_import(
            boundary_id(77),
            TerminalUnitImport::for_import(&import, |_| Ok(())).unwrap(),
        ),
        Err(TerminalBindingRejection::UnknownImport {
            boundary: boundary_id(77),
        })
    );

    // An adapter built against a different contract cannot bind this slot.
    let mut incompatible = import.clone();
    incompatible.scalar_parameters.pop();
    assert_eq!(
        instance.bind_import(
            boundary_id(1),
            TerminalUnitImport::for_import(&incompatible, |_| Ok(())).unwrap(),
        ),
        Err(TerminalBindingRejection::SignatureMismatch {
            boundary: boundary_id(1),
        })
    );
    assert_eq!(
        instance.bound_imports().collect::<Vec<_>>(),
        [],
        "rejected bindings never enter the roster"
    );

    instance
        .bind_import(
            boundary_id(1),
            TerminalUnitImport::for_import(&import, |arguments| {
                observed.push(arguments.len());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
    // The roster retains the exact contract each established binding was
    // checked against — the description the embedder can read back.
    assert_eq!(
        instance.bound_signature(boundary_id(1)),
        Some(&TerminalImportSignature::from(&import))
    );
    assert_eq!(instance.bound_signature(boundary_id(2)), None);
    // The binding carrier is affine: a second supply for the same slot is a
    // duplicate, not a replacement.
    assert_eq!(
        instance.bind_import(
            boundary_id(1),
            TerminalUnitImport::for_import(&import, |_| Ok(())).unwrap(),
        ),
        Err(TerminalBindingRejection::DuplicateBinding {
            boundary: boundary_id(1),
        })
    );
    instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .unwrap();
    instance.close().unwrap();
    drop(instance);
    assert_eq!(observed, [2], "the first binding served the invocation");
}

#[test]
fn same_requirement_different_slots_reach_different_hosts() {
    let (semantic, proof) = two_import_sections();
    let program = TerminalProgram::load(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    assert_eq!(
        program.imports().map(|d| d.id).collect::<Vec<_>>(),
        [boundary_id(1), boundary_id(2)]
    );
    let mut first_calls = Vec::new();
    let mut second_calls = Vec::new();
    let mut instance = program
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    let first = program.import(boundary_id(1)).unwrap().clone();
    let second = program.import(boundary_id(2)).unwrap().clone();
    instance
        .bind_import(
            boundary_id(1),
            TerminalUnitImport::for_import(&first, |arguments| {
                first_calls.push(arguments.to_vec());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
    instance
        .bind_import(
            boundary_id(2),
            TerminalUnitImport::for_import(&second, |arguments| {
                second_calls.push(arguments.to_vec());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
    instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .unwrap();
    assert_eq!(
        instance.effects().len(),
        2,
        "both boundary calls committed before close"
    );
    instance.close().unwrap();
    assert!(
        instance.effects().is_empty(),
        "close releases the effect record"
    );
    drop(instance);
    assert_eq!(first_calls.len(), 1, "slot 1 reached only its own adapter");
    assert_eq!(second_calls.len(), 1, "slot 2 reached only its own adapter");
}

#[test]
fn scalar_result_import_returns_through_the_checked_adapter() {
    let mut module = scalar_boundary_effect_module();
    module.boundary_machines[0].result = BoundaryMachineResult::Scalar(ScalarType::Boolean);
    let returned = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(3),
        scalar_type: ScalarType::Boolean,
    };
    module.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(4),
        ..returned
    });
    module.machines[0].blocks[0].operations[2].result = OperationResult::Scalar(returned);
    module.machines[0].blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(3),
        cleanup_actions: Vec::new(),
    };
    let program = load(&module);
    let mut calls = 0usize;
    let mut instance = program
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    let import = program.import(boundary_id(1)).unwrap().clone();
    instance
        .bind_import(
            boundary_id(1),
            TerminalScalarImport::for_import(&import, |arguments| {
                calls += 1;
                let [
                    TerminalScalarValue::Boolean(first),
                    TerminalScalarValue::Boolean(second),
                ] = arguments
                else {
                    return Err(TerminalEffectRejection::new("wrong argument schema"));
                };
                Ok(TerminalScalarValue::Boolean(*first != *second))
            })
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        instance.invoke(&mut TerminalFuelMeter::unbounded()),
        Ok(TerminalExecutionStatus::Complete(
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        ))
    );
    // A finished invocation re-reports its retained result rather than
    // re-entering the guest or the host.
    assert_eq!(
        instance.invoke(&mut TerminalFuelMeter::unbounded()),
        Ok(TerminalExecutionStatus::Complete(
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        ))
    );
    instance.close().unwrap();
    drop(instance);
    assert_eq!(calls, 1, "completion is not re-entry");
}

#[test]
fn adapter_result_schema_mismatch_rejects_before_commit() {
    let mut module = scalar_boundary_effect_module();
    module.boundary_machines[0].result = BoundaryMachineResult::Scalar(ScalarType::Boolean);
    module.machines[0].blocks[0].operations[2].result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(3),
        scalar_type: ScalarType::Boolean,
    });
    module.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(4),
        scalar_type: ScalarType::Boolean,
    });
    module.machines[0].blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(3),
        cleanup_actions: Vec::new(),
    };
    let program = load(&module);
    let mut instance = program
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    let import = program.import(boundary_id(1)).unwrap().clone();
    // The closure's declared signature matches the slot, but the value it
    // returns does not: the checked adapter rejects its own result before the
    // interpreter ever commits it.
    instance
        .bind_import(
            boundary_id(1),
            TerminalScalarImport::for_import(&import, |_| {
                Ok(TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    value: IntegerValue::Unsigned(7),
                })
            })
            .unwrap(),
        )
        .unwrap();
    assert!(matches!(
        instance.invoke(&mut TerminalFuelMeter::unbounded()),
        Err(TerminalInstanceError::Execution(
            TerminalInterpretError::EffectRejected { .. }
        ))
    ));
    assert!(instance.effects().is_empty());
}

#[test]
fn adapter_construction_rejects_out_of_contract_slots() {
    let program = load(&scalar_boundary_effect_module());
    let unit_import = program.import(boundary_id(1)).unwrap().clone();
    let mut scalar_import = unit_import.clone();
    scalar_import.result = BoundaryMachineResult::Scalar(ScalarType::Boolean);
    assert_eq!(
        TerminalScalarImport::for_import(&unit_import, |_| Ok(TerminalScalarValue::Boolean(true)))
            .map(|_| ()),
        Err(TerminalAdapterError::ResultKindUnsupported)
    );
    assert_eq!(
        TerminalUnitImport::for_import(&scalar_import, |_| Ok(())).map(|_| ()),
        Err(TerminalAdapterError::ResultKindUnsupported)
    );
}

#[test]
fn close_reclaims_finished_and_never_started_instances() {
    let program = load(&scalar_boundary_effect_module());
    let mut instance = program
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    assert!(instance.close().is_ok(), "never-invoked instance closes");
    assert!(instance.is_closed());
    assert_eq!(
        instance.invoke(&mut TerminalFuelMeter::unbounded()),
        Err(TerminalInstanceError::Closed)
    );
    let import = program.import(boundary_id(1)).unwrap().clone();
    assert_eq!(
        instance.bind_import(
            boundary_id(1),
            TerminalUnitImport::for_import(&import, |_| Ok(())).unwrap(),
        ),
        Err(TerminalBindingRejection::InstanceClosed)
    );
}

#[test]
fn close_rejects_a_live_activation_and_resume_completes_it() {
    let program = load(&scalar_boundary_effect_module());
    let mut observed = 0usize;
    let mut instance = program
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    let import = program.import(boundary_id(1)).unwrap().clone();
    instance
        .bind_import(
            boundary_id(1),
            TerminalUnitImport::for_import(&import, |_| {
                observed += 1;
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        instance.invoke(&mut meter),
        Ok(TerminalExecutionStatus::SponsorExhausted(_))
    ));
    assert_eq!(
        instance.close(),
        Err(TerminalInstanceCloseError::Busy),
        "a live activation's custody is not discarded"
    );
    meter.replenish(1).unwrap();
    loop {
        match instance.invoke(&mut meter).unwrap() {
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => break,
            other => panic!("unexpected fixture outcome: {other:?}"),
        }
    }
    assert!(instance.is_finished());
    assert!(instance.close().is_ok());
    drop(instance);
    assert_eq!(observed, 1);
}

#[test]
fn independent_instances_share_one_admitted_program() {
    let (semantic, proof) = two_import_sections();
    let program = TerminalProgram::load(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    let mut second_calls = Vec::new();
    let mut first_instance = program
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    let mut second_instance = program
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    // Rosters are per-instance: the first can invoke while the second is
    // still unbound, and each slot's host object is its own.
    let import = program.import(boundary_id(1)).unwrap().clone();
    first_instance
        .bind_import(
            boundary_id(1),
            TerminalUnitImport::for_import(&import, |_| Ok(())).unwrap(),
        )
        .unwrap();
    first_instance
        .bind_import(
            boundary_id(2),
            TerminalUnitImport::for_import(
                &program.import(boundary_id(2)).unwrap().clone(),
                |_| Ok(()),
            )
            .unwrap(),
        )
        .unwrap();
    second_instance
        .bind_import(
            boundary_id(1),
            TerminalUnitImport::for_import(&import, |_| Ok(())).unwrap(),
        )
        .unwrap();
    second_instance
        .bind_import(
            boundary_id(2),
            TerminalUnitImport::for_import(
                &program.import(boundary_id(2)).unwrap().clone(),
                |arguments| {
                    second_calls.push(arguments.to_vec());
                    Ok(())
                },
            )
            .unwrap(),
        )
        .unwrap();
    first_instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .unwrap();
    second_instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .unwrap();
    first_instance.close().unwrap();
    second_instance.close().unwrap();
    drop(second_instance);
    drop(first_instance);
    assert_eq!(second_calls.len(), 1);
}
