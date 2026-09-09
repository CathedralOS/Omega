//! Evaluate the target's authored arrival policy, not a Rust reconstruction.

use super::{repository_root, write_callback_package};
use calling_conventions::{
    CallSignature, CallingPolicy, EntryStack, MachineRegime, Preemption, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};
use compiler::compile_to_checked_with_packages;
use provider_planning::calling_policy_plans::{BoundaryValueClass, evaluate_calling_policy_plan};
use std::fs;

fn checked_contract(name: &str) -> compiler::CheckedCompilation {
    let source = fs::read_to_string(
        repository_root().join("source/library/std/targets/macos_arm64/entry.omg"),
    )
    .expect("real authored target contract");
    let (path, inputs) = write_callback_package(name, &source);
    compile_to_checked_with_packages(&path, Some("macos_arm64"), inputs)
        .expect("the real macOS entry contract and its calling applications check")
}

fn physical_signature() -> CallSignature {
    CallSignature {
        parameters: vec![
            ValueShape::integer(4, 4),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
        ],
        result: Some(ValueShape::integer(4, 4)),
    }
}

#[test]
fn macos_entry_policy_replays_physical_and_internal_storage_calling_plans() {
    let checked = checked_contract("macos-entry-call-plans");
    let realizations: Vec<_> = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| realization.policy_machine.ends_with("MacosArm64::plan"))
        .collect();
    assert_eq!(realizations.len(), 2);
    for realization in realizations {
        let (actual, report, commitment) = realization
            .replayed_validated_application()
            .expect("independent source-signature and policy-result replay");
        assert_eq!(report, realization.report_fingerprint);
        assert_eq!(commitment, realization.commitment);
        assert_eq!(actual.plan(), realization.exact_boundary_entry_plan());
        let signature = if actual.plan().call.result.is_some() {
            physical_signature()
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
            CallSignature {
                parameters: vec![ValueShape::integer(16, 8); 2],
                result: None,
            }
        };
        let expected = evaluate_ordinary_boundary_entry_plan(CallingPolicy::Aapcs64, &signature)
            .expect("independent ABI plan");
        assert_eq!(actual.plan().call, expected.plan().call);
        assert_eq!(
            actual.plan().state.initial_regime,
            MachineRegime::Aarch64A64 { exception_level: 0 }
        );
        assert_eq!(actual.plan().state.stack, EntryStack::ProviderSelected);
        assert_eq!(actual.plan().state.preemption, Preemption::NotApplicable);
        let mut missing_placement = realization.clone();
        missing_placement.boundary_entry_plan.call.parameters[0]
            .locations
            .clear();
        assert!(missing_placement.replayed_validated_application().is_err());
    }
    let declaration = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "MacosApplication")
        .expect("target-owned combined boundary schema");
    let schema = provider_planning::service_schema::from_typed(&checked.typed, declaration)
        .expect("retained target schema");
    assert_eq!(schema.methods.len(), 2);
    let semantic = schema
        .methods
        .iter()
        .find(|method| method.requirement_owner == "ProgramStorageEntry")
        .expect("the existing core storage crossing remains distinct");
    let physical = schema
        .methods
        .iter()
        .find(|method| method.requirement_owner == "MacosPhysicalEntry")
        .expect("exact dyld arrival");
    assert_eq!(semantic.parameter_type_identities.len(), 2);
    assert!(!semantic.has_result);
    assert_eq!(physical.parameter_type_identities.len(), 4);
    assert!(physical.has_result);
    assert_ne!(semantic.requirement_identity, physical.requirement_identity);
    for method in [semantic, physical] {
        let realization = checked
            .boundary_calling_plan_realizations()
            .iter()
            .find(|realization| {
                realization
                    .materialized_signature()
                    .owner_requirement_identity()
                    == method.requirement_identity
            })
            .expect("schema requirement rejoins its exact retained application");
        assert_eq!(method.calling_plan_commitment, Some(realization.commitment));
        assert_eq!(
            method.calling_plan_report_fingerprint,
            Some(realization.report_fingerprint)
        );
    }
    assert_eq!(
        target::TargetProfile::MacosArm64
            .program_entry_slot()
            .visible_parameters,
        target::ProgramEntryVisibleParameters::None
    );
}

#[test]
fn macos_entry_policy_rejects_wrong_physical_or_storage_signatures() {
    let checked = checked_contract("macos-entry-invalid-call-plans");
    for mutation in 0..8 {
        let mut signature = physical_signature();
        match mutation {
            0 => {
                signature.parameters.pop();
            }
            1 => signature.parameters[0] = ValueShape::integer(8, 8),
            2 => signature.parameters[1] = ValueShape::integer(4, 4),
            3 => signature.result = None,
            4 => signature.result = Some(ValueShape::integer(8, 8)),
            5 => {
                signature = CallSignature {
                    parameters: vec![ValueShape::integer(8, 8); 2],
                    result: None,
                }
            }
            6 => {
                signature = CallSignature {
                    parameters: vec![ValueShape::integer(16, 16); 2],
                    result: None,
                }
            }
            _ => {
                signature = CallSignature {
                    parameters: vec![ValueShape::integer(16, 8); 2],
                    result: None,
                }
            }
        }
        assert!(
            evaluate_calling_policy_plan(&checked.typed, "MacosArm64::plan", &signature).is_err(),
            "invalid entry shape {mutation} cannot acquire a calling plan"
        );
    }
}

#[test]
fn macos_entry_policy_rejects_same_size_record_with_non_extent_fields() {
    let contract = fs::read_to_string(
        repository_root().join("source/library/std/targets/macos_arm64/entry.omg"),
    )
    .expect("real authored target contract");
    let source = format!(
        "{contract}\n{}",
        r#"
data NotAnExtent { base: f64; length: f64; }
boundary trait WrongStorage {
    machine enter(image: NotAnExtent, initial_storage: NotAnExtent);
}
boundary trait WrongMacosApplication: WrongStorage + Calling<MacosArm64> {}
"#
    );
    let (path, inputs) = write_callback_package("macos-entry-record-rejection", &source);
    let diagnostics = compile_to_checked_with_packages(&path, Some("macos_arm64"), inputs)
        .expect_err("two floating fields have the same size but are not Extent's ABI shape");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("MacosApplication requires exact physical or program-storage entry shapes")),
        "the authored policy must reject the record: {diagnostics:?}"
    );
}
