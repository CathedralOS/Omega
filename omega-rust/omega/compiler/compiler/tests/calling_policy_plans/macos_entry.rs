//! Evaluate the target's authored arrival policy, not a Rust reconstruction.

use super::standard_library_root;
use calling_conventions::{
    CallSignature, CallingPolicy, EntryStack, MachineRegime, Preemption, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};
use compiler::CheckedCompileRequest;
use compiler::compile_to_checked;
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use provider_planning::calling_policy_plans::{BoundaryValueClass, evaluate_calling_policy_plan};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};

/// Check the real bundled contract under its own standard-library package
/// custody: a copied fixture source would declare a second `MacosArm64` beside
/// the bundled target implementation. The macOS ARM64 program-entry slot owns a
/// closed physical-contract package, so every `macos_arm64` compilation already
/// seeds `targets/macos_arm64/entry.omg`; a copy of it duplicates `MacosArm64`,
/// `MacosPhysicalEntry`, `MacosApplication` and every `MacosArm64::*` policy
/// machine.
fn checked_contract(_name: &str) -> compiler::CheckedCompilation {
    let standard_library_root = standard_library_root();
    let package =
        PackageKeyIdentity::from_digest([76; 32]).expect("nonzero entry fixture package identity");
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
            &standard_library_root.join("targets/macos_arm64/entry.omg"),
            Some("macos_arm64"),
        )
    })
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

/// Copy the whole standard library and append `declarations` to its macOS
/// ARM64 entry contract, returning the copied contract path.
///
/// The wrong-shaped application has to be authored inside the standard library
/// itself. `MacosArm64` is package-private, so a consumer package cannot name
/// it, and the macOS ARM64 slot's closed physical-contract package seeds the
/// authored contract into every `macos_arm64` compilation, so a fixture that
/// carried its own copy of that contract would declare `MacosArm64`,
/// `MacosPhysicalEntry`, `MacosApplication` and every `MacosArm64::*` policy
/// machine twice. One copied standard library keeps exactly one of each and
/// still reaches the authored rejection.
fn standard_library_copy_with_entry_declarations(name: &str, declarations: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "omega-calling-policy-std-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    copy_source_tree(&standard_library_root(), &root);
    let contract = root.join("targets/macos_arm64/entry.omg");
    let authored = fs::read_to_string(&contract).expect("copied macOS entry contract");
    fs::write(&contract, format!("{authored}\n{declarations}"))
        .expect("append the wrong-shaped application to the copied contract");
    contract
}

fn copy_source_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create standard-library copy directory");
    for entry in fs::read_dir(source).expect("read standard-library directory") {
        let entry = entry.expect("standard-library directory entry");
        let copied = destination.join(entry.file_name());
        if entry
            .file_type()
            .expect("standard-library entry kind")
            .is_dir()
        {
            copy_source_tree(&entry.path(), &copied);
        } else {
            fs::copy(entry.path(), &copied).expect("copy standard-library source");
        }
    }
}

#[test]
fn macos_entry_policy_rejects_same_size_record_with_non_extent_fields() {
    let contract = standard_library_copy_with_entry_declarations(
        "macos-entry-record-rejection",
        r#"
data NotAnExtent { base: f64; length: f64; }
boundary trait WrongStorage {
    machine enter(image: NotAnExtent, initial_storage: NotAnExtent);
}
boundary trait WrongMacosApplication: WrongStorage + Calling<MacosArm64> {}
"#,
    );
    let root = contract
        .ancestors()
        .nth(3)
        .expect("copied standard-library root")
        .to_path_buf();
    let package = PackageKeyIdentity::from_digest([78; 32])
        .expect("nonzero copied standard-library package identity");
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "omega-language-std",
            root,
        )],
        Vec::new(),
    )
    .expect("copied standard-library package graph");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&contract, Some("macos_arm64"))
    })
    .expect_err("two floating fields have the same size but are not Extent's ABI shape");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("MacosApplication requires exact physical or program-storage entry shapes")),
        "the authored policy must reject the record: {diagnostics:?}"
    );
}
