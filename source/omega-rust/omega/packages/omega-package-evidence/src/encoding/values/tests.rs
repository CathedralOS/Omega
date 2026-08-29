use crate::encoding::encode::PackageReviewEncodingLimits;
use crate::encoding::encode::encoder::Encoder;
use crate::encoding::encode::review::{encode, encode_with_limits};
use crate::encoding::encode::rows::{encode_rows, encode_rows_with_limits};
use crate::evidence::package::PackageReviewCanonicalRowSources;
use crate::evidence::{
    CheckedPackageReviewProjection, PackageReviewCanonicalRowSource,
    PackageReviewCompilerIntrinsicExecution, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewSyntheticSourceKind,
};
use omega_effects::provider_plan::ProviderBinding;
use psi_core::PackageKeyIdentity;

use super::identity::encode_nominal;
use super::providers::{encode_compiler_intrinsic_execution, encode_provider_row};

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
