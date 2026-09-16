//! Evaluate the Linux target's authored arrival policy, not a Rust
//! reconstruction. The kernel arrives with the initial process-stack image in
//! rsp and completes through `exit_group`; the semantic crossing stays the
//! shared two-root `ProgramStorageEntry` contract.

use super::repository_root;
use calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, EntryStack, MachineRegime, MachineRegister,
    ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use compiler::CheckedCompileRequest;
use compiler::compile_to_checked;
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use provider_planning::calling_policy_plans::{BoundaryValueClass, evaluate_calling_policy_plan};
use semantic_vocabulary::PackageKeyIdentity;

/// Check the real bundled contract under its own standard-library package
/// custody: a copied fixture source would declare a second `LinuxX86_64`
/// beside the bundled target implementation.
fn checked_contract(_name: &str) -> compiler::CheckedCompilation {
    let standard_library_root = repository_root().join("source/library/std");
    let package =
        PackageKeyIdentity::from_digest([73; 32]).expect("nonzero entry fixture package identity");
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "omega-language-std",
            standard_library_root.clone(),
        )],
        Vec::new(),
    )
    .expect("standard-library entry fixture package graph");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(
            &standard_library_root.join("targets/linux_x86_64/entry.omg"),
            Some("linux_x86_64"),
        )
    })
    .expect("the real Linux entry contract and its calling applications check")
}

fn physical_signature() -> CallSignature {
    CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(4, 4)),
    }
}

fn semantic_signature() -> CallSignature {
    CallSignature {
        parameters: vec![ValueShape::integer(16, 8); 2],
        result: None,
    }
}

#[test]
fn linux_entry_policy_replays_physical_and_semantic_calling_plans() {
    let checked = checked_contract("linux-entry-call-plans");
    let realizations: Vec<_> = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| realization.policy_machine.ends_with("LinuxX86_64::plan"))
        .collect();
    assert_eq!(realizations.len(), 2);
    for realization in realizations {
        let (actual, report, commitment) = realization
            .replayed_validated_application()
            .expect("independent source-signature and policy-result replay");
        assert_eq!(report, realization.report_fingerprint);
        assert_eq!(commitment, realization.commitment);
        assert_eq!(actual.plan(), realization.exact_boundary_entry_plan());
        if actual.plan().call.result.is_some() {
            // The physical surface is the kernel arrival, not an ordinary
            // boundary call: the initial process-stack image arrives in rsp,
            // the completion status leaves in edi, and there is no return
            // continuation beyond exit_group.
            let [stack_image] = actual.plan().call.parameters.as_slice() else {
                panic!("kernel physical entry carries exactly the initial stack image")
            };
            assert_eq!(stack_image.shape, ValueShape::integer(8, 8));
            assert_eq!(
                stack_image.locations.as_slice(),
                [ValueLocation::Register {
                    register: MachineRegister::X86Rsp,
                    value_byte_offset: 0,
                    byte_size: 8,
                }]
            );
            let result = actual
                .plan()
                .call
                .result
                .as_ref()
                .expect("physical entry result");
            assert_eq!(result.shape, ValueShape::integer(4, 4));
            assert_eq!(
                result.locations.as_slice(),
                [ValueLocation::Register {
                    register: MachineRegister::X86Rdi,
                    value_byte_offset: 0,
                    byte_size: 4,
                }]
            );
            assert_eq!(
                actual.plan().call.entry_control,
                EntryControl::SupervisorCall {
                    number_register: MachineRegister::X86Rax,
                    immediate: 231,
                }
            );
        } else {
            let materialized = realization.materialized_signature();
            for root in materialized.parameters() {
                let shape = materialized.shapes()[usize::from(*root)];
                let BoundaryValueClass::Record {
                    first_field,
                    field_count,
                } = shape.class()
                else {
                    panic!("core Extent must retain its record shape, not an ABI-sized integer")
                };
                assert_eq!(field_count, 2);
                let fields = &materialized.fields()[usize::from(first_field)..][..2];
                assert_eq!([fields[0].byte_offset(), fields[1].byte_offset()], [0, 8]);
                for field in fields {
                    let word = materialized.shapes()[usize::from(field.shape())];
                    assert_eq!(word.class(), BoundaryValueClass::Integer);
                    assert_eq!((word.byte_size(), word.alignment()), (8, 8));
                }
            }
            // The semantic crossing is an ordinary System V boundary call the
            // generated bridge performs: the two Extent roots occupy rdi/rsi
            // and rdx/rcx.
            let expected = evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::SystemVAMD64,
                &semantic_signature(),
            )
            .expect("independent ABI plan");
            assert_eq!(actual.plan().call, expected.plan().call);
        }
    }
}

#[test]
fn linux_entry_policy_evaluates_both_entry_surfaces() {
    let checked = checked_contract("linux-entry-policy-eval");
    let physical =
        evaluate_calling_policy_plan(&checked.typed, "LinuxX86_64::plan", &physical_signature())
            .expect("kernel physical entry plans");
    assert_eq!(physical.plan().call.stack_alignment, 16);
    assert_eq!(
        physical.plan().call.entry_control,
        EntryControl::SupervisorCall {
            number_register: MachineRegister::X86Rax,
            immediate: 231,
        },
        "the kernel supplies no return continuation; completion is exit_group"
    );
    // The program-storage crossing is only expressible through its retained
    // application: the authored policy requires the exact Extent record
    // shapes, which a flat call signature cannot denote.
    let semantic = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| realization.policy_machine.ends_with("LinuxX86_64::plan"))
        .find(|realization| {
            realization
                .exact_boundary_entry_plan()
                .call
                .result
                .is_none()
        })
        .expect("retained semantic program-storage entry application");
    let (semantic_plan, _, _) = semantic
        .replayed_validated_application()
        .expect("semantic application replays");
    assert_eq!(
        semantic_plan.plan().call.entry_control,
        EntryControl::CallReturn
    );
    assert_eq!(
        semantic_plan.plan().call.parameters.len(),
        2,
        "the bridge passes the image and initial-storage roots"
    );
    assert_eq!(
        semantic_plan.plan().state.initial_regime,
        MachineRegime::X86Long64
    );
    assert_eq!(
        semantic_plan.plan().state.stack,
        EntryStack::ProviderSelected
    );
}

#[test]
fn linux_entry_policy_rejects_wrong_physical_or_storage_signatures() {
    let checked = checked_contract("linux-entry-invalid-call-plans");
    for mutation in 0..8 {
        let mut signature = physical_signature();
        match mutation {
            0 => {
                signature.parameters.push(ValueShape::integer(8, 8));
            }
            1 => signature.parameters[0] = ValueShape::integer(4, 4),
            2 => signature.result = None,
            3 => signature.result = Some(ValueShape::integer(8, 8)),
            4 => {
                signature = CallSignature {
                    parameters: vec![ValueShape::integer(8, 8); 2],
                    result: Some(ValueShape::integer(4, 4)),
                }
            }
            5 => {
                signature = CallSignature {
                    parameters: vec![ValueShape::integer(16, 16); 2],
                    result: None,
                }
            }
            6 => {
                signature = CallSignature {
                    parameters: vec![ValueShape::integer(8, 8); 3],
                    result: None,
                }
            }
            _ => {
                signature = CallSignature {
                    parameters: vec![],
                    result: None,
                }
            }
        }
        assert!(
            evaluate_calling_policy_plan(&checked.typed, "LinuxX86_64::plan", &signature).is_err(),
            "invalid entry shape {mutation} cannot acquire a calling plan"
        );
    }
}
