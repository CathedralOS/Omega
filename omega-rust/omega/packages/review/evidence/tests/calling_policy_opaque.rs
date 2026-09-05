mod support;

use omega_package_evidence::project_checked_calling_policy;
use omega_package_evidence::record::{
    PackagePolicyCallingPlan, PackageReviewNominalOwner,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationMovementRole,
};
use support::*;

const TYPES: &str = r#"use omega::language::core::representation;
pub boundary data TransferToken;
pub data TransferCarrier { value: u64; }
pub TransferTokenRepresentation:
    TransferCarrier satisfies OpaqueRepresentation<TransferToken>;
pub boundary data UnusedToken [copy];
pub data UnusedCarrier [copy] { value: u64; }
pub UnusedTokenRepresentation:
    UnusedCarrier satisfies OpaqueRepresentation<UnusedToken>;
"#;

const CALLING: &str = r#"use calling;
data TransferPolicy { }
TransferPolicyCallingPolicy: TransferPolicy satisfies CallingPolicy;
machine TransferPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    let mut output: BoundaryEntryPlan;
    output.call.convention = CallingConvention::MicrosoftX64;
    output.call.parameter_count = 2;
    output.call.parameters[0].shape.class = AbiValueClass::Integer;
    output.call.parameters[0].shape.byte_size = signature.shapes[1].byte_size;
    output.call.parameters[0].shape.alignment = signature.shapes[1].alignment;
    output.call.parameters[0].location_count = 1;
    output.call.parameters[0].locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rcx, value_byte_offset: 0,
        byte_size: signature.shapes[1].byte_size,
    };
    output.call.parameters[1].shape.class = AbiValueClass::Integer;
    output.call.parameters[1].shape.byte_size = signature.shapes[3].byte_size;
    output.call.parameters[1].shape.alignment = signature.shapes[3].alignment;
    output.call.parameters[1].location_count = 1;
    output.call.parameters[1].locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rdx, value_byte_offset: 0,
        byte_size: signature.shapes[3].byte_size,
    };
    output.call.has_result = true;
    output.call.result.shape.class = AbiValueClass::Integer;
    output.call.result.shape.byte_size = signature.shapes[5].byte_size;
    output.call.result.shape.alignment = signature.shapes[5].alignment;
    output.call.result.location_count = 1;
    output.call.result.locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rax, value_byte_offset: 0,
        byte_size: signature.shapes[5].byte_size,
    };
    output.call.stack_alignment = 16;
    output.call.shadow_bytes = 32;
    output.call.entry_control = EntryControl::CallReturn;
    BoundaryPlanResult::Accepted { plan: output }
}
boundary trait TransferEntry: Calling<TransferPolicy> {
    machine transfer(first: TransferToken, second: TransferToken) -> TransferToken;
}
"#;

fn fixture(foreign_types: bool) -> (TempPackage, Option<TempPackage>, CheckedCompilation) {
    let package = TempPackage::new();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap();
    package.write(
        "calling.omg",
        &fs::read_to_string(repository.join("source/library/std/calling.omg")).unwrap(),
    );
    let dependency = foreign_types.then(TempPackage::new);
    let mut source_bindings = vec![PackageSourceBinding::new(
        package_identity(),
        "review-fixture",
        package.0.clone(),
    )];
    let mut dependency_bindings = Vec::new();
    let imports = if let Some(dependency) = &dependency {
        dependency.write(
            "types.omg",
            &TYPES
                .replace(
                    "pub boundary data TransferToken;",
                    "pub boundary data TransferToken [copy];",
                )
                .replace(
                    "pub data TransferCarrier {",
                    "pub data TransferCarrier [copy] {",
                ),
        );
        let identity = PackageKeyIdentity::from_digest([42; 32]).unwrap();
        source_bindings.push(PackageSourceBinding::new(
            identity,
            "carrier-package",
            dependency.0.clone(),
        ));
        dependency_bindings.push(PackageDependencyBinding::new(
            package_identity(),
            "carrier",
            identity,
        ));
        "use carrier::types;\n"
    } else {
        TYPES
    };
    package.write("main.omg", &format!("{imports}{CALLING}"));
    package.write(
        "build.omg",
        &format!(
            r#"{}
machine build(builder: &mut Build) {{
    builder.package("review-fixture");
    builder.select_representation<TransferToken, TransferTokenRepresentation>();
    builder.select_representation<UnusedToken, UnusedTokenRepresentation>();
}}
"#,
            if foreign_types {
                "use carrier::types;"
            } else {
                ""
            }
        ),
    );
    let inputs = PackageCompilationInputs::new_package(
        package_identity(),
        source_bindings,
        dependency_bindings,
    )
    .unwrap();
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        inputs,
    )
    .expect("opaque calling contract source should check");
    (package, dependency, checked)
}

fn project(checked: &CheckedCompilation) -> PackagePolicyCallingPlan {
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| checked.symbols.name(realization.boundary_trait) == "TransferEntry")
        .expect("source boundary calling realization");
    let policy =
        project_checked_calling_policy(checked, realization).expect("exact opaque calling policy");
    let bytes = policy
        .canonical_bytes()
        .expect("complete calling policy bytes");
    let recovered = PackagePolicyCallingPlan::recover_canonical(
        &bytes,
        omega_package_evidence::encoding::PackagePolicyRecoveryLimits::default(),
    )
    .expect("source-independent complete calling policy recovery");
    assert_eq!(recovered, policy);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    policy
}

#[test]
fn opaque_calling_policy_retains_exact_occurrences_and_excludes_unused_selections() {
    let (_package, _dependency, checked) = fixture(false);
    assert_eq!(checked.opaque_representation_selections().len(), 2);
    let policy = project(&checked);
    let [use_] = policy.opaque_uses() else {
        panic!("only the used selection contributes a calling row")
    };
    assert_eq!(use_.opaque().path(), "TransferToken");
    assert_eq!(use_.carrier().path(), "TransferCarrier");
    assert_eq!(
        use_.selection_owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        use_.application().declaration().path(),
        "TransferTokenRepresentation"
    );
    assert!(use_.application().subject().is_some());
    assert_eq!(use_.application().trait_arguments().len(), 1);
    assert_eq!(
        use_.copy_disposition(),
        PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly
    );
    assert_eq!(use_.occurrences().len(), 3);
    let roots: Vec<_> = use_
        .occurrences()
        .iter()
        .map(|occurrence| occurrence.carrier_shape_root())
        .collect();
    assert!(roots.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(
        roots
            .iter()
            .all(|root| usize::from(*root) < policy.shape_graph().shapes().len())
    );
    assert_eq!(
        use_.occurrences()[0].role(),
        PackageReviewOpaqueRepresentationMovementRole::Parameter {
            formal_ordinal: 0,
            native_ordinal: 0
        }
    );
    assert_eq!(
        use_.occurrences()[1].role(),
        PackageReviewOpaqueRepresentationMovementRole::Parameter {
            formal_ordinal: 1,
            native_ordinal: 1
        }
    );
    assert_eq!(
        use_.occurrences()[2].role(),
        PackageReviewOpaqueRepresentationMovementRole::Result
    );
    assert!(
        use_.occurrences()
            .iter()
            .all(|occurrence| occurrence.path().is_empty())
    );

    // Recheck against changed typed selection meaning, not just the cached
    // use's old compact or strong application coordinate.
    let mut changed = checked.clone();
    let selected = checked
        .opaque_representation_selections()
        .iter()
        .find(|selection| checked.symbols.name(selection.opaque()) == "TransferToken")
        .unwrap();
    let foreign_carrier = checked
        .opaque_representation_selections()
        .iter()
        .find(|selection| checked.symbols.name(selection.opaque()) == "UnusedToken")
        .unwrap()
        .carrier();
    let conformances = changed.typed.roots.conformances;
    changed
        .typed
        .tables
        .conformances
        .span_mut(conformances)
        .unwrap()
        .iter_mut()
        .find(|conformance| conformance.symbol == selected.application().declaration)
        .unwrap()
        .carrier_symbol = foreign_carrier;
    let realization = changed
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| changed.symbols.name(realization.boundary_trait) == "TransferEntry")
        .unwrap();
    assert!(project_checked_calling_policy(&changed, realization).is_err());
}

#[test]
fn foreign_opaque_declarations_keep_the_local_selection_owner() {
    let (_package, _dependency, checked) = fixture(true);
    let policy = project(&checked);
    let [use_] = policy.opaque_uses() else {
        panic!("one foreign used opaque")
    };
    let foreign =
        PackageReviewNominalOwner::Package(PackageKeyIdentity::from_digest([42; 32]).unwrap());
    assert_eq!(use_.opaque().owner(), foreign);
    assert_eq!(use_.carrier().owner(), foreign);
    assert_eq!(use_.application().declaration().owner(), foreign);
    assert_eq!(
        use_.selection_owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(use_.occurrences().len(), 3);
    assert_eq!(
        use_.copy_disposition(),
        PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    );
}
