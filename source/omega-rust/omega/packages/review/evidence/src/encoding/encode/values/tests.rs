use super::super::PackageReviewEncodingLimits;
use super::super::encoder::Encoder;
use super::super::review::{encode, encode_with_limits};
use super::super::rows::{encode_rows, encode_rows_with_limits};
use crate::record::package::PackageReviewCanonicalRowSources;
use crate::record::{
    CheckedPackageReviewProjection, PackageReviewCanonicalRowSource,
    PackageReviewCollectionViewOperation, PackageReviewCompilerIntrinsicExecution,
    PackageReviewContractCallTarget, PackageReviewContractExpression, PackageReviewDataProperties,
    PackageReviewExternalBinding, PackageReviewExternalCallableSignature,
    PackageReviewExternalExecutableSupply, PackageReviewExternalRequirement,
    PackageReviewExternalStaticParameter, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSyntheticSourceKind, PackageReviewTypeIdentity,
};
use omega_effects::provider_plan::ProviderBinding;
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
            &mut changed_bounds.signature.static_parameters[0];
        properties.multiplicity = psi_language_semantics::Multiplicity::Unrestricted;
    }
    let multiplicity_key = encode_key(&changed_bounds);
    assert_ne!(telescope_key, multiplicity_key);

    {
        let PackageReviewExternalStaticParameter::Type { properties } =
            &mut changed_bounds.signature.static_parameters[0];
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
    assert_ne!(carry_key, encode_key(&changed_requirement));
}

pub(crate) fn normalized_import_row(
    export: &[u8],
) -> omega_effects::provider_plan::ProviderPlanRow {
    omega_effects::provider_plan::ProviderPlanRow {
        method: "write".to_owned(),
        requirement_identity: "Console::write#exact".to_owned(),
        binding: ProviderBinding::Import {
            locator: omega_effects::normalize_foreign_locator(
                omega_effects::ForeignLocatorCandidate::PeByName {
                    library: b"kernel32.dll".to_vec(),
                    export: export.to_vec(),
                },
                omega_target::TargetProfile::WindowsX64,
            )
            .expect("normalized import fixture"),
        },
    }
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
