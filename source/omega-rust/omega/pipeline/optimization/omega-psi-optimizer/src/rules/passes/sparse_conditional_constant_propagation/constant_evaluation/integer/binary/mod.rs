//! Optimizer module role: stage group. Binary integer constant-evaluation taxonomy.
//!
//! Exact rule definitions descend by operation semantics. Every rule enters
//! the same constant-fact traversal, operation-shape classifier, scalar
//! evaluator, and proof-witness seam. The SCCP pass entrance remains the sole
//! owner of exact rule enablement and order.

macro_rules! integer_binary_evaluation_rule {
    ($name:ident, $rule_name:literal, $kind:expr, $safety:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> omega_optimization_core::OptimizationRuleContract {
                super::binary_contract($rule_name, $safety)
            }
        }

        impl crate::PsiOptimizationRule for $name {
            fn contract(&self) -> omega_optimization_core::OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &omega_optimization_unit::PsiOptimizationUnit,
                analyses: crate::RuleAnalysisView<'_>,
            ) -> Result<Vec<omega_optimization_unit::PsiRewriteCandidate>, crate::RuleProposalError>
            {
                super::proposal::propose(unit, analyses, Self::contract(), $kind)
            }
        }
    };
}

mod arithmetic;
mod bitwise;
mod model;
mod proposal;
mod quotient;
mod shapes;
mod shifts;
mod witness;

pub use arithmetic::*;
pub use bitwise::*;
pub use quotient::*;
pub use shifts::*;

fn binary_contract(
    rule_name: &[u8],
    safety: omega_optimization_core::OptimizationSafetyClass,
) -> omega_optimization_core::OptimizationRuleContract {
    super::super::integer_evaluation_contract(rule_name, safety)
}
