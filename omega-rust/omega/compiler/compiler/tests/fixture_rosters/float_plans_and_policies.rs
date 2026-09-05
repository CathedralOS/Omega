//! Exact corpus cases used by selected float plans and policy tests.
//! The execution tables retain their adapter, diagnostic, and trap expectations.

pub(crate) const DOMAINS_DOMAIN_OPERATOR_PROVEN_FACT_SELECTS_MEANING: &str =
    "domains/domain_operator_proven_fact_selects_meaning";
pub(crate) const OPERATORS_FLOAT_OPERATOR_IDENTITIES: &str = "operators/float_operator_identities";
pub(crate) const FLOAT_RUNTIME_NAMED_FORMAT_CONVERSION_EXIT: &str =
    "float/runtime_named_format_conversion_exit";
pub(crate) const FLOAT_RUNTIME_NAMED_INTEGER_TO_FLOAT_CONVERSION_EXIT: &str =
    "float/runtime_named_integer_to_float_conversion_exit";
pub(crate) const FLOAT_RUNTIME_NAMED_FLOAT_TO_INTEGER_CONVERSION_EXIT: &str =
    "float/runtime_named_float_to_integer_conversion_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_MIN_MAX_SQRT_EXIT: &str =
    "float/named_provider_min_max_sqrt_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_NEGATE_IS_NAN_EXIT: &str =
    "float/named_provider_negate_is_nan_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_CLASSIFICATION_PREDICATES_EXIT: &str =
    "float/named_provider_classification_predicates_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_CLASSIFY_EXIT: &str = "float/named_provider_classify_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_MULTIPLY_THEN_ADD_EXIT: &str =
    "float/named_provider_multiply_then_add_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_FUSED_MULTIPLY_ADD_EXIT: &str =
    "float/named_provider_fused_multiply_add_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_DIRECTED_FUSED_MULTIPLY_ADD_EXIT: &str =
    "float/named_provider_directed_fused_multiply_add_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_DIRECTED_ADD_EXIT: &str =
    "float/named_provider_directed_add_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_DIRECTED_SUBTRACT_EXIT: &str =
    "float/named_provider_directed_subtract_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_DIRECTED_MULTIPLY_EXIT: &str =
    "float/named_provider_directed_multiply_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_DIRECTED_DIVIDE_EXIT: &str =
    "float/named_provider_directed_divide_exit";
pub(crate) const FLOAT_NAMED_PROVIDER_DIRECTED_SQUARE_ROOT_EXIT: &str =
    "float/named_provider_directed_square_root_exit";
pub(crate) const ARITHMETIC_FLOAT_SATURATING_OVERFLOW_EXIT: &str =
    "arithmetic/float_saturating_overflow_exit";
pub(crate) const DOMAINS_DOMAIN_OPERATOR_UNPROVEN_KEEPS_BUILTIN_MEANING: &str =
    "domains/domain_operator_unproven_keeps_builtin_meaning";
pub(crate) const DOMAINS_DOMAIN_OPERATOR_INACTIVE_SAME_CARRIER_COEXISTS: &str =
    "domains/domain_operator_inactive_same_carrier_coexists";
pub(crate) const DOMAINS_DOMAIN_OPERATOR_COMPETING_SPELLING_MEANINGS: &str =
    "domains/domain_operator_competing_spelling_meanings";
pub(crate) const FLOAT_NAMED_FLOAT_TO_INTEGER_EXACT_UNPROVEN: &str =
    "float/named_float_to_integer_exact_unproven";
pub(crate) const FLOAT_NAMED_FLOAT_TO_INTEGER_WRAPPING_REJECTED: &str =
    "float/named_float_to_integer_wrapping_rejected";
pub(crate) const FLOAT_NAMED_FLOAT_TO_INTEGER_NO_CONTEXT_UNPROVEN: &str =
    "float/named_float_to_integer_no_context_unproven";
pub(crate) const FLOAT_NAMED_FLOAT_TO_INTEGER_IMPLICIT_DISCARD_REJECTED: &str =
    "float/named_float_to_integer_implicit_discard_rejected";
pub(crate) const OPERATORS_NAMED_OPERATOR_RESULT_OVERLOAD_DUPLICATE_DISPATCH: &str =
    "operators/named_operator_result_overload_duplicate_dispatch";
pub(crate) const FLOAT_RUNTIME_NAMED_FLOAT_TO_INTEGER_TRAPPING_NAN_TRAPS: &str =
    "float/runtime_named_float_to_integer_trapping_nan_traps";
pub(crate) const FLOAT_RUNTIME_NAMED_FLOAT_TO_INTEGER_TRAPPING_OVERFLOW_TRAPS: &str =
    "float/runtime_named_float_to_integer_trapping_overflow_traps";
pub(crate) const FLOAT_FLOAT_SATURATING_ARITHMETIC_EXIT: &str =
    "float/float_saturating_arithmetic_exit";
pub(crate) const FLOAT_FLOAT_TRAPPING_OVERFLOW_TRAPS: &str = "float/float_trapping_overflow_traps";
pub(crate) const FLOAT_RUNTIME_POLICY_ADAPTER_MATRIX_EXIT: &str =
    "float/runtime_policy_adapter_matrix_exit";
pub(crate) const ARITHMETIC_FLOAT_TRAPPING_OVERFLOW_TRAPS: &str =
    "arithmetic/float_trapping_overflow_traps";
pub(crate) const ARITHMETIC_FLOAT_TRAPPING_DIVZERO_TRAPS: &str =
    "arithmetic/float_trapping_divzero_traps";
pub(crate) const ARITHMETIC_FLOAT_TRAPPING_INVALID_TRAPS: &str =
    "arithmetic/float_trapping_invalid_traps";
pub(crate) const FLOAT_FLOAT_TRAPPING_PROPAGATED_NAN_TRAPS: &str =
    "float/float_trapping_propagated_nan_traps";
pub(crate) const FLOAT_FLOAT_TRAPPING_PROPAGATED_INFINITY_TRAPS: &str =
    "float/float_trapping_propagated_infinity_traps";

pub(crate) const FLOAT_TO_INTEGER_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        FLOAT_NAMED_FLOAT_TO_INTEGER_EXACT_UNPROVEN,
        "cannot prove unqualified `I32::from_f64` operand",
    ),
    (
        FLOAT_NAMED_FLOAT_TO_INTEGER_WRAPPING_REJECTED,
        "has no overload for result dispatch set `arithmetic:Wrapping`",
    ),
    (
        FLOAT_NAMED_FLOAT_TO_INTEGER_NO_CONTEXT_UNPROVEN,
        "cannot prove unqualified `I32::from_f64` operand",
    ),
    (
        FLOAT_NAMED_FLOAT_TO_INTEGER_IMPLICIT_DISCARD_REJECTED,
        "discards its non-unit `i32` result",
    ),
    (
        OPERATORS_NAMED_OPERATOR_RESULT_OVERLOAD_DUPLICATE_DISPATCH,
        "duplicate named requirement overload `Convert::value`",
    ),
];

pub(crate) const FLOAT_TO_INTEGER_TRAP_PASS_CANARIES: &[&str] = &[
    FLOAT_RUNTIME_NAMED_FLOAT_TO_INTEGER_TRAPPING_NAN_TRAPS,
    FLOAT_RUNTIME_NAMED_FLOAT_TO_INTEGER_TRAPPING_OVERFLOW_TRAPS,
];

pub(crate) const POLICY_ADAPTER_PASS_CANARIES: &[(
    &str,
    checked_trees::CheckedArithmeticPolicyAdapter,
)] = &[
    (
        FLOAT_FLOAT_SATURATING_ARITHMETIC_EXIT,
        checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
            format: numerics::float_semantics::FloatFormat::BINARY32,
        },
    ),
    (
        FLOAT_FLOAT_TRAPPING_OVERFLOW_TRAPS,
        checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
            format: numerics::float_semantics::FloatFormat::BINARY32,
        },
    ),
];

pub(crate) const POLICY_DIFFERENTIAL_PASS_CANARIES: &[(&str, Option<i32>, Option<&str>)] = &[
    (FLOAT_RUNTIME_POLICY_ADAPTER_MATRIX_EXIT, Some(70), None),
    (ARITHMETIC_FLOAT_SATURATING_OVERFLOW_EXIT, Some(70), None),
    (
        ARITHMETIC_FLOAT_TRAPPING_OVERFLOW_TRAPS,
        None,
        Some("float overflow"),
    ),
    (
        ARITHMETIC_FLOAT_TRAPPING_DIVZERO_TRAPS,
        None,
        Some("division by zero"),
    ),
    (
        ARITHMETIC_FLOAT_TRAPPING_INVALID_TRAPS,
        None,
        Some("invalid float operation"),
    ),
    (
        FLOAT_FLOAT_TRAPPING_PROPAGATED_NAN_TRAPS,
        None,
        Some("non-finite NaN result"),
    ),
    (
        FLOAT_FLOAT_TRAPPING_PROPAGATED_INFINITY_TRAPS,
        None,
        Some("non-finite infinity result"),
    ),
];

pub(crate) const PASS_CANARIES: &[&str] = &[
    DOMAINS_DOMAIN_OPERATOR_PROVEN_FACT_SELECTS_MEANING,
    OPERATORS_FLOAT_OPERATOR_IDENTITIES,
    FLOAT_RUNTIME_NAMED_FORMAT_CONVERSION_EXIT,
    FLOAT_RUNTIME_NAMED_INTEGER_TO_FLOAT_CONVERSION_EXIT,
    FLOAT_RUNTIME_NAMED_FLOAT_TO_INTEGER_CONVERSION_EXIT,
    FLOAT_NAMED_PROVIDER_MIN_MAX_SQRT_EXIT,
    FLOAT_NAMED_PROVIDER_NEGATE_IS_NAN_EXIT,
    FLOAT_NAMED_PROVIDER_CLASSIFICATION_PREDICATES_EXIT,
    FLOAT_NAMED_PROVIDER_CLASSIFY_EXIT,
    FLOAT_NAMED_PROVIDER_MULTIPLY_THEN_ADD_EXIT,
    FLOAT_NAMED_PROVIDER_FUSED_MULTIPLY_ADD_EXIT,
    FLOAT_NAMED_PROVIDER_DIRECTED_FUSED_MULTIPLY_ADD_EXIT,
    FLOAT_NAMED_PROVIDER_DIRECTED_ADD_EXIT,
    FLOAT_NAMED_PROVIDER_DIRECTED_SUBTRACT_EXIT,
    FLOAT_NAMED_PROVIDER_DIRECTED_MULTIPLY_EXIT,
    FLOAT_NAMED_PROVIDER_DIRECTED_DIVIDE_EXIT,
    FLOAT_NAMED_PROVIDER_DIRECTED_SQUARE_ROOT_EXIT,
    DOMAINS_DOMAIN_OPERATOR_UNPROVEN_KEEPS_BUILTIN_MEANING,
    DOMAINS_DOMAIN_OPERATOR_INACTIVE_SAME_CARRIER_COEXISTS,
];

pub(crate) const FAIL_CANARIES: &[&str] = &[DOMAINS_DOMAIN_OPERATOR_COMPETING_SPELLING_MEANINGS];
