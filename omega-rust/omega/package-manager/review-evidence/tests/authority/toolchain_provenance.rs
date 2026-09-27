use crate::support::*;
use build_declarations::DependencyPurpose;
use compiler::CheckedCompileRequest;

#[test]
fn copyable_opaque_retains_its_exact_selected_property_receipt() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::core::representation;

pub boundary data CopyToken [copy];
pub data CopyLeaf [copy] { value: u64; }
pub data CopyPayload [copy] { case Empty; case Value(value: CopyLeaf); }
pub data CopyCarrier [copy] { payloads: [CopyPayload; 2]; }
pub CopyTokenRepresentation:
    CopyCarrier satisfies OpaqueRepresentation<CopyToken>;
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.package("review_fixture");
    builder.select_representation<CopyToken, CopyTokenRepresentation>();
}
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("selected copyable opaque fixture should check");
    let [selection] = checked.custody.opaque_representation_selections() else {
        panic!("one selected copyable opaque representation")
    };
    assert_eq!(
        selection.copy_disposition(),
        representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    );
    assert!(
        checked
            .custody
            .boundary_calling_plan_realizations()
            .iter()
            .all(|realization| realization
                .materialized_signature()
                .opaque_representation_uses()
                .is_empty()),
        "an unused property receipt is not a D26 consumer demand"
    );

    let review = project_checked_package_review(&checked)
        .expect("selected copy receipt should project canonically");
    let rows = review
        .representation_tcb()
        .iter()
        .filter(|row| row.declaration().path() == "CopyToken")
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    let receipt = rows
        .iter()
        .find_map(|row| match row.kind() {
            PackageReviewRepresentationTcbKind::SelectedCopyReceipt {
                conformance,
                carrier,
                representation_schema_version,
                origin,
                lifecycle,
                copy_disposition,
                conformance_application_commitment,
                selected_application_commitment,
            } => Some((
                conformance,
                carrier,
                representation_schema_version,
                origin,
                lifecycle,
                copy_disposition,
                conformance_application_commitment,
                selected_application_commitment,
            )),
            PackageReviewRepresentationTcbKind::Unbound
            | PackageReviewRepresentationTcbKind::ProducerAvailability { .. }
            | PackageReviewRepresentationTcbKind::ConsumerDemand { .. } => None,
        })
        .expect("one selected opaque copy receipt row");
    assert_eq!(receipt.0.path(), "CopyTokenRepresentation");
    assert_eq!(receipt.1.path(), "CopyCarrier");
    assert_eq!(*receipt.2, selection.schema_version());
    assert_eq!(
        *receipt.3,
        PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance
    );
    assert_eq!(
        *receipt.4,
        PackageReviewOpaqueRepresentationLifecycleDisposition::Inert
    );
    assert_eq!(
        *receipt.5,
        PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    );
    assert_eq!(*receipt.6, selection.application().commitment.as_bytes());
    assert_eq!(*receipt.7, selection.selected_application_commitment());

    let canonical = review
        .canonical_rows()
        .expect("copy receipt canonical rows");
    let representation_rows = canonical
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::RepresentationTcb)
        .collect::<Vec<_>>();
    assert_eq!(representation_rows.len(), 3);
    let receipt_row = representation_rows
        .iter()
        .find(|row| {
            row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::RepresentationSelection
                })
            })
        })
        .expect("copy receipt row retains exact build selection source");
    assert_eq!(
        receipt_row.risk(),
        PackageReviewCanonicalRowRisk::AuditRecommended
    );
    let envelope = encode_package_review_canonical_row(receipt_row)
        .expect("selected copy receipt canonical row should encode");
    let recovered = decode_package_review_canonical_row(&envelope)
        .expect("selected copy receipt canonical row should recover");
    assert_eq!(
        recovered.kind(),
        PackageReviewCanonicalRowKind::RepresentationTcb
    );
    assert!(
        recovered
            .source()
            .authored_locations()
            .is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::RepresentationSelection
                })
            })
    );
}

#[test]
fn by_value_opaque_use_retains_exact_consumer_demand() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega_language_std::calling;
use omega::language::core::representation;

pub boundary data TransferToken;
pub data TransferCarrier { value: u64; }
pub TransferTokenRepresentation:
    TransferCarrier satisfies OpaqueRepresentation<TransferToken>;

data TwoParameterPolicy { }
TwoParameterPolicyCallingPolicy: TwoParameterPolicy satisfies CallingPolicy;

machine TwoParameterPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
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
        register: MachineRegister::X86Rcx,
        value_byte_offset: 0,
        byte_size: signature.shapes[1].byte_size,
    };
    output.call.parameters[1].shape.class = AbiValueClass::Integer;
    output.call.parameters[1].shape.byte_size = signature.shapes[3].byte_size;
    output.call.parameters[1].shape.alignment = signature.shapes[3].alignment;
    output.call.parameters[1].location_count = 1;
    output.call.parameters[1].locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rdx,
        value_byte_offset: 0,
        byte_size: signature.shapes[3].byte_size,
    };
    output.call.stack_alignment = 16;
    output.call.shadow_bytes = 32;
    output.call.entry_control = EntryControl::CallReturn;
    BoundaryPlanResult::Accepted { plan: output }
}

boundary trait TransferEntry: Calling<TwoParameterPolicy> {
    machine transfer(first: TransferToken, second: TransferToken);
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.package("review_fixture");
    builder.select_representation<TransferToken, TransferTokenRepresentation>();
}
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs_with_std(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("by-value opaque representation fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("by-value opaque representation demand should project");
    let demand = review
        .representation_tcb()
        .iter()
        .find_map(|row| match row.kind() {
            PackageReviewRepresentationTcbKind::ConsumerDemand {
                target,
                conformance,
                carrier,
                copy_disposition,
                shape_graph,
                occurrences,
                calling_policy,
                conformance_application_commitment,
                selected_application_commitment,
                boundary_plan_commitment,
                ..
            } if row.declaration().path() == "TransferToken" => Some((
                target,
                conformance,
                carrier,
                copy_disposition,
                shape_graph,
                occurrences,
                calling_policy,
                conformance_application_commitment,
                selected_application_commitment,
                boundary_plan_commitment,
            )),
            _ => None,
        })
        .expect("one actual consumer-demand row");
    assert_eq!(
        demand.0.profile(),
        PackageReviewRepresentationTargetProfile::WindowsX64
    );
    assert_eq!(demand.1.path(), "TransferTokenRepresentation");
    assert_eq!(demand.2.path(), "TransferCarrier");
    assert_eq!(
        *demand.3,
        PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly
    );
    assert!(!demand.4.shapes().is_empty());
    let [first_occurrence, second_occurrence] = demand.5.as_slice() else {
        panic!("two exact opaque occurrences in two boundary parameters")
    };
    assert_ne!(
        first_occurrence.carrier_shape_root(),
        second_occurrence.carrier_shape_root()
    );
    assert!(
        usize::from(first_occurrence.carrier_shape_root()) < demand.4.shapes().len()
            && usize::from(second_occurrence.carrier_shape_root()) < demand.4.shapes().len()
    );
    assert!(matches!(
        first_occurrence.role(),
        PackageReviewOpaqueRepresentationMovementRole::Parameter {
            formal_ordinal: 0,
            native_ordinal: 0,
        }
    ));
    assert!(matches!(
        second_occurrence.role(),
        PackageReviewOpaqueRepresentationMovementRole::Parameter {
            formal_ordinal: 1,
            native_ordinal: 1,
        }
    ));
    assert_eq!(
        first_occurrence.placement().locations(),
        &[
            package_evidence::record::PackageReviewBoundaryValueLocation::Register {
                register: PackageReviewMachineRegister::X86Rcx,
                value_byte_offset: 0,
                byte_size: 8,
            }
        ]
    );
    assert_eq!(
        second_occurrence.placement().locations(),
        &[
            package_evidence::record::PackageReviewBoundaryValueLocation::Register {
                register: PackageReviewMachineRegister::X86Rdx,
                value_byte_offset: 0,
                byte_size: 8,
            }
        ]
    );
    assert_eq!(*demand.6, PackageReviewBoundaryCallingPolicy::MicrosoftX64);
    assert_ne!(*demand.7, [0; 32]);
    assert_ne!(*demand.8, [0; 32]);
    assert_ne!(*demand.9, [0; 32]);

    let canonical = review
        .canonical_rows()
        .expect("consumer-demand canonical rows");
    let demand_row = canonical
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::RepresentationTcb
                && row.source().authored_locations().is_some_and(|locations| {
                    locations.iter().any(|location| {
                        location.role() == PackageReviewSourceLocationRole::RepresentationSelection
                    }) && locations.iter().any(|location| {
                        location.role() == PackageReviewSourceLocationRole::TraitParent
                    })
                })
                && row
                    .canonical_bytes()
                    .windows(32)
                    .any(|window| window == demand.9)
        })
        .expect("consumer-demand canonical row with selection and boundary source custody");
    assert_eq!(
        demand_row.risk(),
        PackageReviewCanonicalRowRisk::AuditRecommended
    );
    let envelope =
        encode_package_review_canonical_row(demand_row).expect("consumer-demand row should encode");
    let recovered =
        decode_package_review_canonical_row(&envelope).expect("consumer-demand row should recover");
    assert_eq!(recovered.key_bytes(), demand_row.key_bytes());
    assert_eq!(recovered.canonical_bytes(), demand_row.canonical_bytes());
}

#[test]
fn dependency_owned_opaque_copy_receipt_belongs_to_the_selecting_package() {
    let Some(target) = host_target_name() else {
        return;
    };
    let root = TempPackage::new();
    let dependency = TempPackage::new();
    root.write("main.omg", "machine root() {}\n");
    root.write(
        "build.omg",
        r#"use carrier::types;
machine build(builder: &mut Build) {
    builder.package("review_fixture");
    builder.select_representation<CopyToken, CopyTokenRepresentation>();
}
"#,
    );
    dependency.write(
        "types.omg",
        r#"use omega::language::core::representation;

pub boundary data CopyToken [copy];
pub data CopyCarrier [copy] { value: u64; }
pub CopyTokenRepresentation:
    CopyCarrier satisfies OpaqueRepresentation<CopyToken>;
"#,
    );

    let root_identity = package_identity();
    let dependency_identity =
        PackageKeyIdentity::from_digest([42; 32]).expect("dependency package identity");
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![
            PackageSourceBinding::new(root_identity, "review_fixture", root.0.clone()),
            PackageSourceBinding::new(dependency_identity, "carrier_package", dependency.0.clone()),
        ],
        vec![
            PackageDependencyBinding::new(root_identity, "carrier", dependency_identity),
            // The build entry imports the carrier; build imports select build edges.
            PackageDependencyBinding::for_purpose(
                root_identity,
                "carrier",
                dependency_identity,
                DependencyPurpose::Build,
            ),
        ],
    )
    .expect("root and representation dependency graph should validate");
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.0.join("main.omg"), Some(target))
    })
    .expect("root should select a dependency-owned copyable opaque");
    let review = project_checked_package_review(&checked)
        .expect("the selecting package should retain the copy receipt");
    let receipt = review
        .representation_tcb()
        .iter()
        .find(|row| {
            row.declaration().owner() == PackageReviewNominalOwner::Package(dependency_identity)
                && matches!(
                    row.kind(),
                    PackageReviewRepresentationTcbKind::SelectedCopyReceipt { .. }
                )
        })
        .expect("dependency-owned opaque copy receipt in root review");
    assert_eq!(receipt.declaration().path(), "CopyToken");

    let canonical = review
        .canonical_rows()
        .expect("cross-package copy receipt canonical rows");
    let receipt_source = canonical
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::RepresentationTcb
                && row.source().authored_locations().is_some_and(|locations| {
                    locations.iter().any(|location| {
                        location.role() == PackageReviewSourceLocationRole::RepresentationSelection
                            && location.owner()
                                == PackageReviewSourceLocationOwner::Package(root_identity)
                    })
                })
        })
        .expect("root-owned selecting source for dependency representation receipt");
    assert!(
        receipt_source
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::Declaration
                    && location.owner()
                        == PackageReviewSourceLocationOwner::Package(dependency_identity)
            }))
    );
}
