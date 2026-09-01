use super::super::PackageReviewEncodingLimits;
use super::super::encoder::Encoder;
use super::super::review::{encode, encode_with_limits};
use super::super::rows::{encode_rows, encode_rows_with_limits};
use crate::record::package::PackageReviewCanonicalRowSources;
use crate::record::{
    CheckedPackageReviewProjection, PackageReviewCanonicalRowSource,
    PackageReviewCollectionViewOperation, PackageReviewCompilerIntrinsicExecution,
    PackageReviewConformanceBound, PackageReviewContractCallTarget,
    PackageReviewContractExpression, PackageReviewDataProperties, PackageReviewExternalBinding,
    PackageReviewExternalCallableSignature, PackageReviewExternalExecutableSupply,
    PackageReviewExternalRequirement, PackageReviewExternalStaticParameter,
    PackageReviewMachineParameterContract, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSyntheticSourceKind, PackageReviewTypeIdentity,
};
use omega_effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
    EvaluatedForeignImport, ProviderBinding,
};
use psi_core::PackageKeyIdentity;

use super::callables::{encode_external_callable_signature, encode_external_executable_supply_key};
use super::expressions::encode_contract_expression;
use super::identity::encode_nominal;
use super::providers::{encode_compiler_intrinsic_execution, encode_provider_row};

#[test]
fn external_requirement_encoding_appends_the_top_level_requirement_tag() {
    let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
    let nominal = |path: &str| PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package),
        path: path.to_owned(),
    };
    let unit_signature = || PackageReviewExternalCallableSignature {
        lifetime_parameter_count: 0,
        static_parameters: Vec::new(),
        conformance_bounds: Vec::new(),
        parameters: Vec::new(),
        return_type: PackageReviewTypeIdentity {
            canonical: "unit".to_owned(),
        },
    };
    let callable_path = "LinuxCompletion::complete";
    let requirement_path = "InterruptAcknowledgement::complete#exact";
    let supply = PackageReviewExternalExecutableSupply {
        callable: nominal(callable_path),
        signature: unit_signature(),
        requirement: PackageReviewExternalRequirement::TopLevelRequirement {
            identity: nominal(requirement_path),
            signature: unit_signature(),
        },
        binding: PackageReviewExternalBinding::Syscall { number: 60 },
    };
    let mut encoder = Encoder::bounded(256);
    encode_external_executable_supply_key(&mut encoder, &supply)
        .expect("encode top-level external requirement key");
    let encoded = encoder.finish().expect("bounded encoding");
    let mut prefix = Encoder::bounded(256);
    encode_nominal(&mut prefix, &supply.callable).expect("encode callable prefix");
    encode_external_callable_signature(&mut prefix, &supply.signature)
        .expect("encode callable signature prefix");
    let tag_offset = prefix.finish().expect("bounded prefix encoding").len();
    assert_eq!(encoded[tag_offset], 2);
}

#[test]
fn external_supply_key_retains_exact_return_carrier_and_static_telescope() {
    let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
    let nominal = |path: &str| PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package),
        path: path.to_owned(),
    };
    let empty_signature = || PackageReviewExternalCallableSignature {
        lifetime_parameter_count: 0,
        static_parameters: Vec::new(),
        conformance_bounds: Vec::new(),
        parameters: Vec::new(),
        return_type: PackageReviewTypeIdentity {
            canonical: "nominal(package:ReturnA)".to_owned(),
        },
    };
    let supply = PackageReviewExternalExecutableSupply {
        callable: nominal("Foreign::read"),
        signature: empty_signature(),
        requirement: PackageReviewExternalRequirement::TopLevelRequirement {
            identity: nominal("Read::read#exact"),
            signature: empty_signature(),
        },
        binding: PackageReviewExternalBinding::Syscall { number: 0 },
    };
    let encode_key = |supply: &PackageReviewExternalExecutableSupply| {
        let mut encoder = Encoder::bounded(512);
        encode_external_executable_supply_key(&mut encoder, supply)
            .expect("encode external supply key");
        encoder.finish().expect("bounded encoding")
    };

    let baseline = encode_key(&supply);
    let mut changed_return = supply.clone();
    changed_return.signature.return_type.canonical = "nominal(package:ReturnB)".to_owned();
    assert_ne!(baseline, encode_key(&changed_return));

    let mut changed_telescope = supply;
    changed_telescope.signature.static_parameters.push(
        PackageReviewExternalStaticParameter::Type {
            properties: PackageReviewDataProperties {
                multiplicity: psi_language_semantics::Multiplicity::Affine,
                carry: None,
            },
        },
    );
    let telescope_key = encode_key(&changed_telescope);
    assert_ne!(baseline, telescope_key);

    let mut changed_bounds = changed_telescope;
    {
        let PackageReviewExternalStaticParameter::Type { properties } =
            &mut changed_bounds.signature.static_parameters[0]
        else {
            unreachable!("type static parameter fixture")
        };
        properties.multiplicity = psi_language_semantics::Multiplicity::Unrestricted;
    }
    let multiplicity_key = encode_key(&changed_bounds);
    assert_ne!(telescope_key, multiplicity_key);

    {
        let PackageReviewExternalStaticParameter::Type { properties } =
            &mut changed_bounds.signature.static_parameters[0]
        else {
            unreachable!("type static parameter fixture")
        };
        properties.multiplicity = psi_language_semantics::Multiplicity::Affine;
        properties.carry = Some(psi_language_semantics::CarryPolicy::PERMISSIVE);
    }
    let carry_key = encode_key(&changed_bounds);
    assert_ne!(telescope_key, carry_key);

    let mut changed_requirement = changed_bounds;
    let PackageReviewExternalRequirement::TopLevelRequirement { signature, .. } =
        &mut changed_requirement.requirement
    else {
        unreachable!("top-level requirement fixture")
    };
    signature
        .static_parameters
        .push(PackageReviewExternalStaticParameter::Type {
            properties: PackageReviewDataProperties {
                multiplicity: psi_language_semantics::Multiplicity::Unrestricted,
                carry: None,
            },
        });
    let requirement_key = encode_key(&changed_requirement);
    assert_ne!(carry_key, requirement_key);

    let mut changed_const_telescope = changed_requirement;
    changed_const_telescope.signature.static_parameters.push(
        PackageReviewExternalStaticParameter::Const {
            type_identity: PackageReviewTypeIdentity {
                canonical: "nominal(toolchain:u64)".to_owned(),
            },
        },
    );
    let const_key = encode_key(&changed_const_telescope);
    assert_ne!(requirement_key, const_key);
    let PackageReviewExternalStaticParameter::Const { type_identity } = changed_const_telescope
        .signature
        .static_parameters
        .last_mut()
        .expect("const static parameter")
    else {
        unreachable!("const static parameter fixture")
    };
    type_identity.canonical = "nominal(toolchain:i32)".to_owned();
    let changed_const_key = encode_key(&changed_const_telescope);
    assert_ne!(const_key, changed_const_key);

    let mut changed_conformance_bounds = changed_const_telescope;
    changed_conformance_bounds
        .signature
        .conformance_bounds
        .push(PackageReviewConformanceBound {
            binder_ordinal: Some(0),
            subject_parameter: 0,
            selected_conformance: None,
            selected_lifetime_arguments: Vec::new(),
            selected_arguments: Vec::new(),
            selected_subject: None,
            trait_identity: nominal("Ranked"),
            trait_lifetime_arguments: Vec::new(),
            arguments: Vec::new(),
        });
    let conformance_key = encode_key(&changed_conformance_bounds);
    assert_ne!(changed_const_key, conformance_key);

    let mut changed_machine_telescope = changed_conformance_bounds;
    changed_machine_telescope.signature.static_parameters.push(
        PackageReviewExternalStaticParameter::Machine {
            contract: PackageReviewMachineParameterContract::RequirementIdentity,
        },
    );
    assert_ne!(conformance_key, encode_key(&changed_machine_telescope));
}

pub(crate) fn normalized_import_row(
    export: &[u8],
) -> omega_effects::provider_plan::ProviderPlanRow {
    let locator = omega_effects::normalize_foreign_locator(
        omega_effects::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: export.to_vec(),
        },
        omega_target::TargetProfile::WindowsX64,
    )
    .expect("normalized import fixture");
    omega_effects::provider_plan::ProviderPlanRow {
        method: "write".to_owned(),
        requirement_identity: "Console::write#exact".to_owned(),
        requirement_lifetime_partition: Vec::new(),
        binding: ProviderBinding::Import {
            evaluated: evaluated_import(locator, 11),
        },
    }
}

#[test]
fn provider_row_encoding_distinguishes_requirement_lifetime_partitions() {
    let mut distinct = normalized_import_row(b"WriteFile");
    distinct.requirement_lifetime_partition = vec![0, 1];
    let mut repeated = distinct.clone();
    repeated.requirement_lifetime_partition = vec![0, 0];

    let encode_row = |row| {
        let mut encoder = Encoder::bounded(4096);
        encode_provider_row(&mut encoder, row).expect("encode provider row");
        encoder.finish().expect("bounded provider row")
    };
    assert_ne!(encode_row(&distinct), encode_row(&repeated));
}

fn evaluated_import(
    locator: omega_effects::NormalizedForeignLocator,
    seed: u8,
) -> EvaluatedForeignImport {
    let usage = EvaluatedBindingUsage::from_evaluator(7, 1, 10, 1_000, 0, 0, 4, 12, 3, 0)
        .expect("valid fixture usage");
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        None,
        format!("fixture::producer::{seed}"),
        EvaluatedBindingProducerClosureDigest::from_bytes([seed; 32]).unwrap(),
        1,
        usage,
        EvaluatedBindingEvaluationDigest::from_bytes([seed.wrapping_add(1); 32]).unwrap(),
        1,
        EvaluatedBindingMaterializationDigest::from_bytes([seed.wrapping_add(2); 32]).unwrap(),
        locator.identity_digest(),
    )
    .expect("valid fixture receipt");
    EvaluatedForeignImport::from_retained_evidence(locator, receipt)
        .expect("receipt matches fixture locator")
}

#[test]
fn collection_view_encoding_has_one_closed_tag_per_operation() {
    for (tag, operation) in [
        PackageReviewCollectionViewOperation::SharedSlice,
        PackageReviewCollectionViewOperation::MutableSlice,
        PackageReviewCollectionViewOperation::TextView,
        PackageReviewCollectionViewOperation::Bytes,
    ]
    .into_iter()
    .enumerate()
    {
        let expression = PackageReviewContractExpression::Call {
            receiver: Some(Box::new(PackageReviewContractExpression::Boolean(false))),
            target: PackageReviewContractCallTarget::CollectionView(operation),
            static_arguments: Vec::new(),
            evidence_arguments: Vec::new(),
            arguments: Vec::new(),
        };
        let mut encoder = Encoder::bounded(64);
        encode_contract_expression(&mut encoder, &expression)
            .expect("encode collection-view expression");
        let encoded = encoder.finish().expect("bounded encoding");
        assert_eq!(&encoded[..5], &[11, 1, 0, 0, 3]);
        assert_eq!(encoded[5], tag as u8);
    }
}

#[test]
pub(crate) fn normalized_import_review_encoding_retains_exact_atomic_locator() {
    fn encoded(export: &[u8]) -> Vec<u8> {
        let mut encoder = Encoder::bounded(1024);
        encode_provider_row(&mut encoder, &normalized_import_row(export))
            .expect("encode normalized import");
        encoder.finish().expect("bounded encoding")
    }

    let write = encoded(b"WriteFile");
    let read = encoded(b"ReadFile");
    assert_ne!(write, read);
    assert!(
        write
            .windows(b"kernel32.dll".len())
            .any(|bytes| bytes == b"kernel32.dll")
    );
    assert!(
        write
            .windows(b"WriteFile".len())
            .any(|bytes| bytes == b"WriteFile")
    );
}

#[test]
fn macho_import_review_encoding_retains_raw_atomic_coordinates() {
    fn encoded(install_name: &[u8], symbol: &[u8]) -> Vec<u8> {
        let row = omega_effects::provider_plan::ProviderPlanRow {
            method: "write".to_owned(),
            requirement_identity: "Console::write#exact".to_owned(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Import {
                evaluated: evaluated_import(
                    omega_effects::normalize_foreign_locator(
                        omega_effects::ForeignLocatorCandidate::MachODylibSymbol {
                            install_name: install_name.to_vec(),
                            symbol: symbol.to_vec(),
                        },
                        omega_target::TargetProfile::MacosArm64,
                    )
                    .expect("normalized Mach-O import fixture"),
                    21,
                ),
            },
        };
        let mut encoder = Encoder::bounded(1024);
        encode_provider_row(&mut encoder, &row).expect("encode normalized Mach-O import");
        encoder.finish().expect("bounded encoding")
    }

    let baseline = encoded(b"/usr/lib/libSystem.B.dylib", b"_write");
    assert_ne!(baseline, encoded(b"/usr/lib/libobjc.A.dylib", b"_write"));
    assert_ne!(baseline, encoded(b"/usr/lib/libSystem.B.dylib", b"_read"));
    assert!(
        baseline
            .windows(b"/usr/lib/libSystem.B.dylib".len())
            .any(|bytes| bytes == b"/usr/lib/libSystem.B.dylib")
    );
    assert!(
        baseline
            .windows(b"_write".len())
            .any(|bytes| bytes == b"_write")
    );
}

#[test]
fn compiler_intrinsic_execution_encoding_is_closed_and_format_sensitive() {
    use omega_provider_planning::plans::CompilerPrimitiveFloatBinaryOperation;

    fn encoded(execution: PackageReviewCompilerIntrinsicExecution) -> Vec<u8> {
        let mut encoder = Encoder::bounded(16);
        encode_compiler_intrinsic_execution(&mut encoder, &execution)
            .expect("encode closed compiler execution");
        encoder.finish().expect("bounded encoding")
    }

    let builtin = encoded(PackageReviewCompilerIntrinsicExecution::BuiltinFunction(
        psi_symbols::BuiltinFunction::Min,
    ));
    let linux_exit = encoded(PackageReviewCompilerIntrinsicExecution::LinuxExitGroupI32);
    let negate_f32 = encoded(PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(
        psi_numerics::literals::FloatFormat::F32,
    ));
    let negate_f64 = encoded(PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(
        psi_numerics::literals::FloatFormat::F64,
    ));
    let primitive_add_f32 = encoded(
        PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary {
            operation: CompilerPrimitiveFloatBinaryOperation::Add,
            format: psi_numerics::literals::FloatFormat::F32,
        },
    );
    let primitive_subtract_f32 = encoded(
        PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary {
            operation: CompilerPrimitiveFloatBinaryOperation::Subtract,
            format: psi_numerics::literals::FloatFormat::F32,
        },
    );
    let primitive_add_f64 = encoded(
        PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary {
            operation: CompilerPrimitiveFloatBinaryOperation::Add,
            format: psi_numerics::literals::FloatFormat::F64,
        },
    );
    let conversion = encoded(
        PackageReviewCompilerIntrinsicExecution::NamedFloatConversion {
            source: omega_provider_planning::plans::CompilerNumericType::F64,
            target: omega_provider_planning::plans::CompilerNumericType::F32,
            domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
        },
    );
    assert_ne!(builtin, negate_f32);
    assert_ne!(linux_exit, builtin);
    assert_eq!(linux_exit, [4]);
    assert_ne!(negate_f32, negate_f64);
    assert_ne!(primitive_add_f32, primitive_subtract_f32);
    assert_ne!(primitive_add_f32, primitive_add_f64);
    assert_eq!(negate_f32, [1, 0]);
    assert_eq!(negate_f64, [1, 1]);
    assert_eq!(conversion, [2, 9, 8, 0]);
    assert_eq!(primitive_add_f32, [3, 0, 0]);
    assert_eq!(primitive_subtract_f32, [3, 1, 0]);
    assert_eq!(primitive_add_f64, [3, 0, 1]);
}

#[test]
fn primitive_float_binary_encoding_has_explicit_operation_tags() {
    use omega_provider_planning::plans::CompilerPrimitiveFloatBinaryOperation;

    let operations = [
        CompilerPrimitiveFloatBinaryOperation::Add,
        CompilerPrimitiveFloatBinaryOperation::Subtract,
        CompilerPrimitiveFloatBinaryOperation::Multiply,
        CompilerPrimitiveFloatBinaryOperation::Divide,
        CompilerPrimitiveFloatBinaryOperation::Equal,
        CompilerPrimitiveFloatBinaryOperation::NotEqual,
        CompilerPrimitiveFloatBinaryOperation::Less,
        CompilerPrimitiveFloatBinaryOperation::LessOrEqual,
        CompilerPrimitiveFloatBinaryOperation::Greater,
        CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual,
    ];
    for (tag, operation) in operations.into_iter().enumerate() {
        let mut encoder = Encoder::bounded(16);
        encode_compiler_intrinsic_execution(
            &mut encoder,
            &PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary {
                operation,
                format: psi_numerics::literals::FloatFormat::F32,
            },
        )
        .expect("encode primitive float binary execution");
        assert_eq!(
            encoder.finish().expect("bounded encoding"),
            [3, tag as u8, 0],
        );
    }
}

#[test]
fn compiler_conversion_encoding_has_explicit_numeric_and_domain_tags() {
    use omega_provider_planning::plans::CompilerNumericType;
    use psi_numerics::arithmetic::ArithmeticDomain;

    fn encoded(execution: PackageReviewCompilerIntrinsicExecution) -> Vec<u8> {
        let mut encoder = Encoder::bounded(16);
        encode_compiler_intrinsic_execution(&mut encoder, &execution)
            .expect("encode closed compiler conversion");
        encoder.finish().expect("bounded encoding")
    }

    let numeric_types = [
        CompilerNumericType::I8,
        CompilerNumericType::I16,
        CompilerNumericType::I32,
        CompilerNumericType::I64,
        CompilerNumericType::U8,
        CompilerNumericType::U16,
        CompilerNumericType::U32,
        CompilerNumericType::U64,
        CompilerNumericType::F32,
        CompilerNumericType::F64,
    ];
    for (tag, numeric_type) in numeric_types.into_iter().enumerate() {
        assert_eq!(
            encoded(
                PackageReviewCompilerIntrinsicExecution::NamedFloatConversion {
                    source: numeric_type,
                    target: CompilerNumericType::F32,
                    domain: ArithmeticDomain::Exact,
                }
            ),
            [2, tag as u8, 8, 0],
        );
    }

    for (tag, domain) in [
        ArithmeticDomain::Exact,
        ArithmeticDomain::Wrapping,
        ArithmeticDomain::Saturating,
        ArithmeticDomain::Trapping,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            encoded(
                PackageReviewCompilerIntrinsicExecution::NamedFloatConversion {
                    source: CompilerNumericType::F64,
                    target: CompilerNumericType::F32,
                    domain,
                }
            ),
            [2, 9, 8, tag as u8],
        );
    }
}

pub(crate) fn empty_review() -> CheckedPackageReviewProjection {
    CheckedPackageReviewProjection {
        package: PackageKeyIdentity::from_digest([1; 32]).expect("nonzero package identity"),
        target: omega_target::TargetProfile::WindowsX64,
        public_traits: Vec::new(),
        public_conformances: Vec::new(),
        public_domains: Vec::new(),
        public_propositions: Vec::new(),
        public_consts: Vec::new(),
        public_operators: Vec::new(),
        public_data: Vec::new(),
        representation_tcb: Vec::new(),
        semantic_dependencies: Vec::new(),
        callables: Vec::new(),
        external_executable_supply: Vec::new(),
        dangerous_authorities: Vec::new(),
        dangerous_authority_slack: Vec::new(),
        selected_providers: Vec::new(),
        selected_provider_families: Vec::new(),
        boundary_application_realizations: Vec::new(),
        row_sources: PackageReviewCanonicalRowSources {
            public_traits: Vec::new(),
            public_conformances: Vec::new(),
            public_domains: Vec::new(),
            public_propositions: Vec::new(),
            public_consts: Vec::new(),
            public_operators: Vec::new(),
            public_data: Vec::new(),
            representation_tcb: Vec::new(),
            semantic_dependencies: Vec::new(),
            callables: Vec::new(),
            external_executable_supply: Vec::new(),
            dangerous_authorities: Vec::new(),
            dangerous_authority_slack: Vec::new(),
            boundary_application_realizations: Vec::new(),
            selected_provider_set: PackageReviewCanonicalRowSource::compiler_derived(
                PackageReviewSyntheticSourceKind::EmptySelectedProviderSet,
            ),
        },
    }
}

#[test]
pub(crate) fn bounded_encoders_reject_instead_of_returning_partial_evidence() {
    let review = empty_review();
    assert!(encode(&review).is_ok());
    assert!(encode_rows(&review).is_ok());

    assert!(
        encode_with_limits(
            &review,
            PackageReviewEncodingLimits::new(1, 2, 64, 256, 512)
        )
        .is_err()
    );
    assert!(
        encode_rows_with_limits(
            &review,
            PackageReviewEncodingLimits::new(256, 1, 64, 256, 512),
        )
        .is_err()
    );
    assert!(
        encode_rows_with_limits(
            &review,
            PackageReviewEncodingLimits::new(256, 2, 64, 1, 512),
        )
        .is_err()
    );
    assert!(
        encode_rows_with_limits(
            &review,
            PackageReviewEncodingLimits::new(256, 2, 64, 256, 1),
        )
        .is_err()
    );
}

#[test]
pub(crate) fn canonical_encoding_rejects_unresolved_nominal_ownership() {
    let identity = PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Unresolved,
        path: "source_free::nominal".to_owned(),
    };
    let error = encode_nominal(&mut Encoder::bounded(1024), &identity)
        .expect_err("unresolved ownership must not enter canonical review bytes");
    assert_eq!(
        error.to_string(),
        "package review cannot encode unresolved nominal ownership"
    );
}
