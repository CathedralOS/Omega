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
    /// Both closed operands must independently form and agree in this target.
    EqualRecipe(ConstantRecipeHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantRecipeObligation {
    owner: symbols::SymbolHandle,
    recipe: ConstantRecipeHandle,
    requirement: ConstantRecipeRequirement,
    source_span: source::SourceSpan,
    /// The checked occurrence, for the refusal a closure reports.
    description: String,
    discharged: bool,
}

impl ConstantRecipeObligation {
    /// Retain an unproved target condition. Only successful discharge can
    /// mark it publishable, and its subjects cannot mutate afterward.
    pub fn retain(
        owner: symbols::SymbolHandle,
        recipe: ConstantRecipeHandle,
        requirement: ConstantRecipeRequirement,
        source_span: source::SourceSpan,
        description: String,
    ) -> Self {
        Self {
            owner,
            recipe,
            requirement,
            source_span,
            description,
            discharged: false,
        }
    }

    pub fn owner(&self) -> symbols::SymbolHandle {
        self.owner
    }
    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn is_pending(&self) -> bool {
        !self.discharged
    }

    /// Discharge every obligation against the closure `program` carries.
    /// Rows are independent; every failure is reported.
    pub fn discharge_all(
        obligations: &mut [Self],
        program: &TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        for obligation in obligations.iter_mut() {
            obligation.discharged = false;
        }
        let mut diagnostics = Vec::new();
        for obligation in obligations.iter() {
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
            if let ConstantRecipeRequirement::EqualRecipe(other) = &obligation.requirement {
                let other_value = program.closed_constant_recipe_value(*other);
                if other_value != Some(value) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{}: contract endpoint equality requires both admitted recipe values to agree; selected values are {} and {:?}",
                        obligation.description, value, other_value,
                    )).with_source_span(obligation.source_span));
                }
                continue;
            }
            let (holds, relation) = match &obligation.requirement {
                ConstantRecipeRequirement::ExceedsBound(bound) => (bound < value, "<"),
                ConstantRecipeRequirement::AtLeast(bound) => (bound <= value, "<="),
                ConstantRecipeRequirement::AtMost(bound) => (value <= bound, ">="),
                ConstantRecipeRequirement::EqualRecipe(_) => {
                    unreachable!("handled both-recipe relation above")
                }
            };
            if !holds {
                let bound = match &obligation.requirement {
                    ConstantRecipeRequirement::ExceedsBound(bound)
                    | ConstantRecipeRequirement::AtLeast(bound)
                    | ConstantRecipeRequirement::AtMost(bound) => bound,
                    ConstantRecipeRequirement::EqualRecipe(_) => {
                        unreachable!("handled both-recipe relation above")
                    }
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
            for obligation in obligations {
                obligation.discharged = true;
            }
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}
