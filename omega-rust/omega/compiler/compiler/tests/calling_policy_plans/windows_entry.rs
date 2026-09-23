//! Evaluate the Windows target's authored arrival policy, not a Rust
//! reconstruction. The loader enters a PE32+ executable at its
//! AddressOfEntryPoint under Microsoft x64 with no contractual register input
//! and completes through the eax process-completion status; the semantic
//! crossing stays the shared two-root `ProgramStorageEntry` contract.

use super::bundled_standard_library_root;
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
/// custody: a copied fixture source would declare a second `WindowsX86_64`
/// beside the bundled target implementation.
fn checked_contract(_name: &str) -> compiler::CheckedCompilation {
    let standard_library_root = bundled_standard_library_root();
    let package =
        PackageKeyIdentity::from_digest([75; 32]).expect("nonzero entry fixture package identity");
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "omega_language_std",
            standard_library_root.clone(),
        )],
        Vec::new(),
    )
    .expect("standard-library entry fixture package graph");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(
            &standard_library_root.join("targets/windows_x86_64/entry.omg"),
            Some("windows_x86_64"),
        )
    })
    .expect("the real Windows entry contract and its calling applications check")
}

fn physical_signature() -> CallSignature {
    CallSignature {
        parameters: Vec::new(),
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
fn windows_entry_policy_replays_physical_and_semantic_calling_plans() {
    let checked = checked_contract("windows-entry-call-plans");
    let realizations: Vec<_> = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| realization.policy_machine.ends_with("WindowsX86_64::plan"))
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
            // The physical surface is the loader arrival, not an ordinary
            // boundary call: no register carries contractual input, and the
            // completion status returns in eax as the process exit code.
            assert!(
                actual.plan().call.parameters.is_empty(),
                "loader physical entry carries no contractual inputs"
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
                    register: MachineRegister::X86Rax,
                    value_byte_offset: 0,
                    byte_size: 4,
                }]
            );
            assert_eq!(actual.plan().call.entry_control, EntryControl::CallReturn);
            assert_eq!(actual.plan().call.shadow_bytes, 32);
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
            // The semantic crossing is an ordinary Microsoft x64 boundary call
            // the generated bridge performs: the two Extent roots pass
            // indirectly through rcx and rdx.
            let expected = evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::MicrosoftX64,
                &semantic_signature(),
            )
            .expect("independent ABI plan");
            assert_eq!(actual.plan().call, expected.plan().call);
        }
    }
}

#[test]
fn windows_entry_policy_evaluates_both_entry_surfaces() {
    let checked = checked_contract("windows-entry-policy-eval");
    let physical =
        evaluate_calling_policy_plan(&checked.typed, "WindowsX86_64::plan", &physical_signature())
            .expect("loader physical entry plans");
    assert_eq!(physical.plan().call.stack_alignment, 16);
    assert_eq!(
        physical.plan().call.entry_control,
        EntryControl::CallReturn,
        "the loader maps the entry's returned status to the process exit code"
    );
    assert_eq!(physical.plan().call.shadow_bytes, 32);
    // The program-storage crossing is only expressible through its retained
    // application: the authored policy requires the exact Extent record
    // shapes, which a flat call signature cannot denote.
    let semantic = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| realization.policy_machine.ends_with("WindowsX86_64::plan"))
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
fn windows_entry_policy_rejects_wrong_physical_or_storage_signatures() {
    let checked = checked_contract("windows-entry-invalid-call-plans");
    for mutation in 0..8 {
        let mut signature = physical_signature();
        match mutation {
            0 => {
                signature.parameters.push(ValueShape::integer(8, 8));
            }
            1 => signature.result = Some(ValueShape::integer(8, 8)),
            2 => signature.result = None,
            3 => signature.result = Some(ValueShape::integer(4, 8)),
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
                    parameters: vec![ValueShape::integer(4, 4)],
                    result: Some(ValueShape::integer(4, 4)),
                }
            }
        }
        assert!(
            evaluate_calling_policy_plan(&checked.typed, "WindowsX86_64::plan", &signature)
                .is_err(),
            "invalid entry shape {mutation} cannot acquire a calling plan"
        );
    }
}
