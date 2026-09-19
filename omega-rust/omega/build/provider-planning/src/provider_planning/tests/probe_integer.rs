//! Coverage for the primitive integer comparison execution identity: what
//! `boundary machine ==` plus a `satisfies ... via Binding::CompilerIntrinsic`
//! provider derives, and that the closed identity commits to the authored
//! spelling and exact operand type.
use super::{derive_satisfies_plans, typed_fixture};
use crate::ProviderPlanDerivation;
use crate::provider_planning::ProviderBinding;

/// A `satisfies` provider over a boundary `==` derives one single-row plan
/// that binds the exact operator requirement under a compiler-intrinsic
/// binding, and the row's execution identity is the authored `equal` on the
/// exact operand primitive.
#[test]
fn integer_comparison_satisfies_provider_binds_the_exact_requirement() {
    let typed = typed_fixture(
        r#"
        boundary machine == Comparison::equal(left: i32, right: i32) -> bool;

        machine IntProvider::equal_impl(left: i32, right: i32) -> bool
        satisfies Comparison::equal
        via Binding::CompilerIntrinsic;

        machine use_it(left: i32, right: i32) -> bool {
            left == right
        }
        "#,
    );
    let [operator] = typed.operators() else {
        panic!("one boundary operator")
    };
    let derived_plans = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None));
    let [derived] = derived_plans.as_slice() else {
        panic!("one derived provider plan")
    };
    let [row] = derived.plan.rows.as_slice() else {
        panic!("one realization row")
    };
    assert!(
        matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }),
        "expected a compiler-intrinsic binding, got {:?}",
        row.binding,
    );
    assert_eq!(derived.provenance.row_requirements, [operator.symbol]);
    assert_eq!(
        crate::primitive_integer_comparison_intrinsic_execution_identity(&typed, operator),
        Some(
            crate::CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                operation: crate::CompilerPrimitiveIntegerComparisonOperation::Equal,
                integer_type: crate::CompilerNumericType::I32,
            }
        ),
    );
    assert_eq!(
        crate::compiler_intrinsic_diagnostic_label(&typed, operator).as_deref(),
        Some("integer-comparison.equal.i32"),
    );
}

/// A boundary comparison operator with no provider derives no plan, so a
/// selected use can never reach an unplanned comparison execution.
#[test]
fn integer_comparison_without_provider_derives_no_plan() {
    let typed = typed_fixture(
        r#"
        boundary machine == Comparison::equal(left: i32, right: i32) -> bool;

        machine use_it(left: i32, right: i32) -> bool {
            left == right
        }
        "#,
    );
    assert!(
        derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None)).is_empty()
    );
}

/// The authored token is the integer comparison's operation identity: a
/// `boundary machine <` declaration commits to the primitive `less` on the
/// exact operand primitive, and the diagnostic label names that closed shape.
#[test]
fn integer_comparison_identity_commits_to_authored_spelling_and_type() {
    for (spelling, expected, label) in [
        (
            "==",
            crate::CompilerPrimitiveIntegerComparisonOperation::Equal,
            "integer-comparison.equal.u64",
        ),
        (
            "!=",
            crate::CompilerPrimitiveIntegerComparisonOperation::NotEqual,
            "integer-comparison.not_equal.u64",
        ),
        (
            "<",
            crate::CompilerPrimitiveIntegerComparisonOperation::Less,
            "integer-comparison.less.u64",
        ),
        (
            "<=",
            crate::CompilerPrimitiveIntegerComparisonOperation::LessOrEqual,
            "integer-comparison.less_or_equal.u64",
        ),
        (
            ">",
            crate::CompilerPrimitiveIntegerComparisonOperation::Greater,
            "integer-comparison.greater.u64",
        ),
        (
            ">=",
            crate::CompilerPrimitiveIntegerComparisonOperation::GreaterOrEqual,
            "integer-comparison.greater_or_equal.u64",
        ),
    ] {
        let typed = typed_fixture(&format!(
            r#"
            boundary machine {spelling} Comparison::order(left: u64, right: u64) -> bool;
            "#,
        ));
        let [operator] = typed.operators() else {
            panic!("one boundary operator")
        };
        assert_eq!(
            crate::primitive_integer_comparison_intrinsic_execution_identity(&typed, operator),
            Some(
                crate::CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                    operation: expected,
                    integer_type: crate::CompilerNumericType::U64,
                }
            ),
            "spelling {spelling}",
        );
        assert_eq!(
            crate::compiler_intrinsic_diagnostic_label(&typed, operator).as_deref(),
            Some(label),
        );
    }
}

/// Operands outside the fixed-width integer roster, a non-Boolean result, or
/// mismatched operand types have no closed comparison identity.
#[test]
fn integer_comparison_identity_fails_closed_outside_the_roster() {
    for source in [
        // Address carrier has no `CompilerNumericType`.
        "boundary machine == Comparison::equal(left: addr, right: addr) -> bool;",
        // Boolean operands are not integer comparisons.
        "boundary machine == Comparison::equal(left: bool, right: bool) -> bool;",
        // Float operands belong to the IEEE comparison lane.
        "boundary machine == Comparison::equal(left: f32, right: f32) -> bool;",
        // A non-Boolean result is not a comparison.
        "boundary machine == Comparison::equal(left: i32, right: i32) -> i32;",
        // Mismatched operand types have no shared operand primitive.
        "boundary machine == Comparison::equal(left: i32, right: i64) -> bool;",
        // A single operand cannot spell a comparison.
        "boundary machine == Comparison::equal(left: i32) -> bool;",
        // Non-comparison spellings have no integer comparison meaning.
        "boundary machine + Comparison::add(left: i32, right: i32) -> i32;",
    ] {
        let typed = typed_fixture(source);
        let [operator] = typed.operators() else {
            panic!("one boundary operator in `{source}`")
        };
        assert_eq!(
            crate::primitive_integer_comparison_intrinsic_execution_identity(&typed, operator),
            None,
            "source `{source}`",
        );
    }
}
