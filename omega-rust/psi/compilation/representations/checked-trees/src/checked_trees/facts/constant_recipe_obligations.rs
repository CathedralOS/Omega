//! What the shared judgment needs from each retained constant recipe.
//!
//! Checking proves an index, a window or a membership against a fixed
//! extent. When the extent is a recipe the shared program cannot fix, the
//! proof keeps its dependency: the comparison it needs is recorded here with
//! the bound checking did establish, and the target closure discharges it
//! against the recipe's closed value. A closure that cannot discharge every
//! row refuses the target; nothing accepts a folded scalar on trust.

use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::constant_recipes::ConstantRecipeHandle;

/// The relation a closed recipe value must satisfy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstantRecipeRequirement {
    /// `bound < recipe`: an index whose proven upper bound is `bound` reads
    /// inside the recipe extent.
    ExceedsBound(BigInt),
    /// `bound <= recipe`: a window end or a membership upper bound.
    AtLeast(BigInt),
    /// `recipe <= bound`: a membership lower bound against an upper endpoint.
    AtMost(BigInt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantRecipeObligation {
    pub recipe: ConstantRecipeHandle,
    /// The exact consuming body, independent of a shared recipe's evaluator.
    pub owner: symbols::SymbolHandle,
    pub requirement: ConstantRecipeRequirement,
    pub source_span: source::SourceSpan,
    /// The checked occurrence, for the refusal a closure reports.
    pub description: String,
}

impl ConstantRecipeObligation {
    /// Discharge every obligation against the closure `program` carries.
    /// Rows are independent; every failure is reported.
    pub fn discharge_all<'obligation>(
        obligations: impl IntoIterator<Item = &'obligation Self>,
        program: &TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        for obligation in obligations {
            let recipe = program.constant_recipes.get(obligation.recipe);
            let Some(value) = program.closed_constant_recipe_value(obligation.recipe) else {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "{}: constant recipe `{}` has no closed value under the selected target realization",
                        obligation.description,
                        recipe.name.as_str(),
                    ))
                    .with_source_span(obligation.source_span),
                );
                continue;
            };
            let (holds, relation) = match &obligation.requirement {
                ConstantRecipeRequirement::ExceedsBound(bound) => (bound < value, "<"),
                ConstantRecipeRequirement::AtLeast(bound) => (bound <= value, "<="),
                ConstantRecipeRequirement::AtMost(bound) => (value <= bound, ">="),
            };
            if !holds {
                let bound = match &obligation.requirement {
                    ConstantRecipeRequirement::ExceedsBound(bound)
                    | ConstantRecipeRequirement::AtLeast(bound)
                    | ConstantRecipeRequirement::AtMost(bound) => bound,
                };
                diagnostics.push(
                    Diagnostic::error(format!(
                        "{}: requires {} {} `{}`, but the selected target realization closes `{}` to {}",
                        obligation.description,
                        bound,
                        relation,
                        recipe.name.as_str(),
                        recipe.name.as_str(),
                        value,
                    ))
                    .with_source_span(obligation.source_span),
                );
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}
