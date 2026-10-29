//! Target-dependent constant recipes retained through the shared program.
//!
//! A constant position whose evaluation needs a selected realization (a
//! boundary provider body, a sealed primitive Float meaning, a target
//! observation) cannot fold while the program is still target-neutral. The
//! position keeps its authored symbolic node (`FixedArrayLength::ConstCall`,
//! a marked range-endpoint call, a `ConstExpression` const-generic argument)
//! and this roster names it as a recipe: the exact application and the
//! declaration it applies. Checking judges the shared program once against
//! the symbolic nodes and records what it needs from each recipe.
//!
//! Closure happens under one target capsule: realization evaluates every
//! recipe under its selected rows and publishes the values here, beside the
//! realization receipts that make two closures distinct even when their
//! values agree. A closed copy of the program carries the closure; the
//! shared program carries none. Numeric consumers ask
//! `TypedTrees::closed_fixed_array_length` and the sibling queries instead
//! of matching the literal node, so one query answers the shared program
//! (unknown) and every closed copy (its own value) alike.

use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use arena::Handle;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;

mod evaluation_entries;
pub use evaluation_entries::{ConstantEvaluationEntry, create_evaluation_entry};

/// Which authored node a recipe keeps symbolic.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum RetainedConstantPosition {
    /// The arena's invalid sentinel only.
    #[default]
    None,
    /// `[T; call()]`: the exact `FixedArray` type-reference node.
    ArrayLength(TypeReferenceHandle),
    /// `T[lo..=call()]`: the endpoint call expression inside the range.
    RangeEndpoint(ExpressionHandle),
    /// A closed integer expression occurring in a source proof clause.
    ContractExpression(ExpressionHandle),
    /// `Data<call()>`: the `ConstExpression` argument node and its authored
    /// root expression.
    ConstApplication {
        site: TypeReferenceHandle,
        root: ExpressionHandle,
    },
}

/// One retained symbolic constant application.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct RetainedConstantRecipe {
    pub position: RetainedConstantPosition,
    /// Every lexical use of this complete expression, with its exact typed
    /// result carrier. Shared expression storage does not imply shared scope.
    pub contexts: arena::HandleSpan<ConstantExpressionContext>,
    /// Exact nested value calls whose admission the contract root owns.
    pub invocations: arena::HandleSpan<ExpressionHandle>,
    /// The first authored callable dependency, when present. A primitive
    /// observation has no authored callee and keeps the invalid handle.
    /// This is a reachability/debug anchor, not the complete root's entry.
    pub machine: SymbolHandle,
    pub name: Identifier,
    pub source_span: source::SourceSpan,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ConstantExpressionContext {
    pub owner: SymbolHandle,
    pub result_type: TypeReferenceHandle,
    /// Produced by executable preparation, never inherited by a specialization.
    pub evaluation_entry: SymbolHandle,
    pub evaluation_expression: ExpressionHandle,
}

impl ConstantExpressionContext {
    pub const fn new(owner: SymbolHandle, result_type: TypeReferenceHandle) -> Self {
        Self {
            owner,
            result_type,
            evaluation_entry: SymbolHandle::invalid(),
            evaluation_expression: ExpressionHandle::invalid(),
        }
    }

    /// Entry materialization does not create another lexical use of a root.
    pub fn same_scope(&self, other: &Self) -> bool {
        self.owner == other.owner && self.result_type == other.result_type
    }
}

pub type ConstantRecipeHandle = Handle<RetainedConstantRecipe>;

/// The exact selected realization one closure evaluated a dependency under.
/// Equal values under different realizations are different closures.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ConstantRecipeDependencyReceipt {
    pub requirement: SymbolHandle,
    pub plan_identity: [u8; 32],
    pub target_scope: String,
    /// Exact choosing declaration and body, independently of code ownership.
    /// Invalid only for the sealed primitive-intrinsic execution route.
    pub provider_declaration: SymbolHandle,
    pub provider_machine: SymbolHandle,
}

/// One recipe's value under a closure.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ClosedConstantRecipe {
    pub recipe: ConstantRecipeHandle,
    pub value: BigInt,
}

/// Exact scoped body used while a portable family was evaluated privately.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ConstantTargetBodySelection {
    pub head: SymbolHandle,
    pub member: SymbolHandle,
    pub target_scope: String,
}

/// Every recipe's value under one target capsule, with the realization
/// receipts the evaluation consumed. Empty in the shared program.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ConstantRecipeClosure {
    /// Primary value application; supplemental proof receipts keep own scopes.
    pub target_scope: String,
    pub values: Vec<ClosedConstantRecipe>,
    pub receipts: Vec<ConstantRecipeDependencyReceipt>,
    pub target_bodies: Vec<ConstantTargetBodySelection>,
}

impl ConstantRecipeClosure {
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The closed integer of `recipe`, when this closure evaluated it.
    pub fn value(&self, recipe: ConstantRecipeHandle) -> Option<&BigInt> {
        self.values
            .iter()
            .find(|closed| closed.recipe == recipe)
            .map(|closed| &closed.value)
    }
}

/// A fixed array's declared extent as every consumer may read it: the
/// literal, or the retained recipe whose value only a target closure fixes.
/// An open const parameter and an unrostered pending call are neither.
/// Checked plans carry this same vocabulary, so one closure query answers
/// the type and the plan built from it alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
pub enum DeclaredArrayExtent {
    Literal(u64),
    Recipe(ConstantRecipeHandle),
}

impl DeclaredArrayExtent {
    /// The literal extent; a recipe has none before closure.
    pub fn literal(self) -> Option<u64> {
        match self {
            Self::Literal(length) => Some(length),
            Self::Recipe(_) => None,
        }
    }

    /// The extent under `program`'s closure: a literal always, a recipe only
    /// in a closed copy.
    pub fn closed(self, program: &crate::TypedTrees) -> Option<u64> {
        match self {
            Self::Literal(length) => Some(length),
            Self::Recipe(recipe) => program
                .closed_constant_recipe_value(recipe)
                .and_then(BigInt::to_u64),
        }
    }

    /// Whether a fixed index is inside the extent when it is known; a recipe
    /// decides nothing here, its obligation is the range check's recorded row.
    pub fn admits_fixed_index(self, index: u64) -> bool {
        match self {
            Self::Literal(length) => index < length,
            Self::Recipe(_) => true,
        }
    }
}

impl crate::TypedTrees {
    /// The declared extent of the `FixedArray` node at `type_reference`, when
    /// it is a literal or a rostered recipe.
    pub fn declared_array_extent(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> Option<DeclaredArrayExtent> {
        let TypeReferenceNode::FixedArray { length, .. } =
            self.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        match length {
            FixedArrayLength::Literal(length) => u64::try_from(*length)
                .ok()
                .map(DeclaredArrayExtent::Literal),
            FixedArrayLength::ConstParameter { .. } | FixedArrayLength::Expression(_) => None,
            FixedArrayLength::ConstCall { .. } => {
                let recipe = self.constant_recipe_for_array_length(type_reference);
                recipe
                    .is_valid()
                    .then_some(DeclaredArrayExtent::Recipe(recipe))
            }
        }
    }

    /// The recipe retained for the exact `FixedArray` node, or the invalid handle.
    pub fn constant_recipe_for_array_length(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> ConstantRecipeHandle {
        self.constant_recipes
            .iter()
            .find(|(_, recipe)| {
                recipe.position == RetainedConstantPosition::ArrayLength(type_reference)
            })
            .map(|(handle, _)| handle)
            .unwrap_or_default()
    }

    /// The recipe retained for a range-endpoint call expression, or the invalid handle.
    pub fn constant_recipe_for_range_endpoint(
        &self,
        expression: ExpressionHandle,
    ) -> ConstantRecipeHandle {
        self.constant_recipes
            .iter()
            .find(|(_, recipe)| {
                recipe.position == RetainedConstantPosition::RangeEndpoint(expression)
            })
            .map(|(handle, _)| handle)
            .unwrap_or_default()
    }

    /// The retained integer root in an ordinary contract clause.
    pub fn constant_recipe_for_contract_expression(
        &self,
        expression: ExpressionHandle,
    ) -> ConstantRecipeHandle {
        self.constant_recipes
            .iter()
            .find(|(_, recipe)| {
                recipe.position == RetainedConstantPosition::ContractExpression(expression)
            })
            .map(|(handle, _)| handle)
            .unwrap_or_default()
    }

    /// A contract root's exact invocation roster defers formation to ordinary
    /// selected evaluation. This does not identify the invocation's value with
    /// the surrounding arithmetic root's completed integer.
    pub fn is_deferred_contract_invocation(&self, expression: ExpressionHandle) -> bool {
        self.constant_recipes.iter().any(|(_, recipe)| {
            matches!(
                recipe.position,
                RetainedConstantPosition::ContractExpression(_)
            ) && self
                .expression_table
                .expression_handles(recipe.invocations)
                .contains(&expression)
        })
    }

    /// The recipe retained for a const-generic argument site, or the invalid handle.
    pub fn constant_recipe_for_const_application(
        &self,
        site: TypeReferenceHandle,
    ) -> ConstantRecipeHandle {
        self.constant_recipes
            .iter()
            .find(|(_, recipe)| {
                matches!(
                    recipe.position,
                    RetainedConstantPosition::ConstApplication { site: candidate, .. }
                        if candidate == site
                )
            })
            .map(|(handle, _)| handle)
            .unwrap_or_default()
    }

    /// Whether `expression` is the authored root of a rostered const-generic argument.
    pub fn constant_recipe_root_for_const_application(&self, expression: ExpressionHandle) -> bool {
        self.constant_recipes.iter().any(|(_, recipe)| {
            matches!(
                recipe.position,
                RetainedConstantPosition::ConstApplication { root, .. } if root == expression
            )
        })
    }

    /// The closed value of `recipe` in this program copy, if it carries a closure.
    pub fn closed_constant_recipe_value(&self, recipe: ConstantRecipeHandle) -> Option<&BigInt> {
        if !recipe.is_valid() {
            return None;
        }
        self.constant_recipe_closure.value(recipe)
    }

    /// The numeric extent of the `FixedArray` node at `type_reference`: a
    /// literal always, a retained recipe only in a closed copy. `None` is a
    /// symbolic extent (an open const parameter, or a recipe before closure).
    pub fn closed_fixed_array_length(&self, type_reference: TypeReferenceHandle) -> Option<usize> {
        let TypeReferenceNode::FixedArray { length, .. } =
            self.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        match length {
            FixedArrayLength::Literal(length) => Some(*length),
            FixedArrayLength::ConstParameter { .. } | FixedArrayLength::Expression(_) => None,
            FixedArrayLength::ConstCall { .. } => self
                .closed_constant_recipe_value(self.constant_recipe_for_array_length(type_reference))
                .and_then(BigInt::to_u64)
                .and_then(|value| usize::try_from(value).ok()),
        }
    }

    /// Read every retained recipe's value from `evaluated`, a copy of this
    /// program whose deferred continuation folded under selected rows. A
    /// position the fold left symbolic names the recipe that did not close.
    pub fn read_constant_recipe_closure(
        &self,
        evaluated: &crate::TypedTrees,
        receipts: Vec<ConstantRecipeDependencyReceipt>,
    ) -> Result<ConstantRecipeClosure, Vec<diagnostics::Diagnostic>> {
        let mut values = Vec::with_capacity(self.constant_recipes.len());
        let mut unclosed = Vec::new();
        for (handle, recipe) in self.constant_recipes.iter() {
            let value = match recipe.position {
                RetainedConstantPosition::None => continue,
                RetainedConstantPosition::ArrayLength(type_reference) => {
                    match evaluated
                        .type_reference_table
                        .type_reference(type_reference)
                    {
                        TypeReferenceNode::FixedArray {
                            length: FixedArrayLength::Literal(length),
                            ..
                        } => u64::try_from(*length).ok().map(BigInt::from_u64),
                        _ => None,
                    }
                }
                RetainedConstantPosition::RangeEndpoint(expression) => evaluated
                    .expression_table
                    .constant_integer_value(expression)
                    .map(BigInt::from_i64),
                RetainedConstantPosition::ContractExpression(expression) => {
                    evaluated.closed_integer_expression_value(expression)
                }
                // The fold rewrites the argument node to the literal's
                // canonical spelling; the authored root expression stays.
                RetainedConstantPosition::ConstApplication { site, root } => {
                    match evaluated.type_reference_table.type_reference(site) {
                        TypeReferenceNode::Named { name, .. } => {
                            BigInt::from_str_radix(name.as_str(), 10)
                        }
                        TypeReferenceNode::ConstExpression(expression) => evaluated
                            .expression_table
                            .constant_integer_value(*expression)
                            .map(BigInt::from_i64),
                        _ => evaluated
                            .expression_table
                            .constant_integer_value(root)
                            .map(BigInt::from_i64),
                    }
                }
            };
            match value {
                Some(value) => values.push(ClosedConstantRecipe {
                    recipe: handle,
                    value,
                }),
                None => unclosed.push(
                    diagnostics::Diagnostic::error(format!(
                        "constant recipe `{}` did not close under the selected target realization",
                        recipe.name.as_str()
                    ))
                    .with_source_span(recipe.source_span),
                ),
            }
        }
        if unclosed.is_empty() {
            Ok(ConstantRecipeClosure {
                target_bodies: Vec::new(),
                values,
                receipts,
                ..Default::default()
            })
        } else {
            Err(unclosed)
        }
    }

    /// Drop every roster entry whose position has since folded to a literal.
    /// An admitted Build evaluates its own recipes in place; the roster then
    /// names only what is still symbolic, so a shared program with no live
    /// recipe needs no closure and admits no pending position.
    pub fn retain_symbolic_constant_recipes(&mut self) {
        let folded: Vec<ConstantRecipeHandle> = self
            .constant_recipes
            .iter()
            .filter(|(_, recipe)| match recipe.position {
                RetainedConstantPosition::None => false,
                RetainedConstantPosition::ArrayLength(type_reference) => !matches!(
                    self.type_reference_table.type_reference(type_reference),
                    TypeReferenceNode::FixedArray {
                        length: FixedArrayLength::ConstCall { .. },
                        ..
                    }
                ),
                RetainedConstantPosition::RangeEndpoint(expression)
                | RetainedConstantPosition::ContractExpression(expression) => {
                    !self.pending_const_range_endpoints.contains(&expression)
                }
                RetainedConstantPosition::ConstApplication { site, .. } => !matches!(
                    self.type_reference_table.type_reference(site),
                    TypeReferenceNode::ConstExpression(_)
                ),
            })
            .map(|(handle, _)| handle)
            .collect();
        for handle in folded {
            self.constant_recipes.free(handle);
        }
    }

    /// This program with `closure` installed: the closed copy a target's
    /// Terminal production and realization consume.
    pub fn with_constant_recipe_closure(mut self, closure: ConstantRecipeClosure) -> Self {
        self.constant_recipe_closure = closure;
        self
    }
}

/// The roster entries deferred evaluation retains; the evaluator appends them
/// as it marks each position pending.
pub fn retain_constant_recipes(
    program: &mut crate::TypedTrees,
    recipes: impl IntoIterator<Item = RetainedConstantRecipe>,
) {
    for recipe in recipes {
        let retained = program
            .constant_recipes
            .iter()
            .find(|(_, retained)| retained.position == recipe.position)
            .map(|(handle, _)| handle);
        if let Some(handle) = retained {
            let retained = program.constant_recipes.get(handle);
            let mut contexts = program
                .constant_expression_contexts
                .span_or_empty(retained.contexts)
                .to_vec();
            let previous_count = contexts.len();
            for context in program
                .constant_expression_contexts
                .span_or_empty(recipe.contexts)
            {
                if !contexts.iter().any(|retained| retained.same_scope(context)) {
                    contexts.push(*context);
                }
            }
            if contexts.len() != previous_count {
                program.constant_recipes.get_mut(handle).contexts =
                    program.constant_expression_contexts.insert_many(contexts);
            }
        } else {
            program.constant_recipes.insert(recipe);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_expression_contexts_preserve_all_owners_and_carriers() {
        let mut program = crate::TypedTrees::default();
        let expression = ExpressionHandle::from_parts(17, 1);
        let first = ConstantExpressionContext {
            owner: SymbolHandle::from_parts(23, 1),
            result_type: TypeReferenceHandle::from_parts(31, 1),
            ..Default::default()
        };
        let second = ConstantExpressionContext {
            owner: SymbolHandle::from_parts(29, 1),
            result_type: TypeReferenceHandle::from_parts(37, 1),
            ..Default::default()
        };
        let rebound = ConstantExpressionContext {
            result_type: TypeReferenceHandle::from_parts(41, 1),
            ..first
        };
        for context in [first, second, first, rebound] {
            let recipe = RetainedConstantRecipe {
                position: RetainedConstantPosition::ContractExpression(expression),
                contexts: program.constant_expression_contexts.insert_many([context]),
                ..Default::default()
            };
            retain_constant_recipes(&mut program, [recipe]);
        }
        assert_eq!(program.constant_recipes.len(), 1);
        let recipe = program.constant_recipe_for_contract_expression(expression);
        let contexts = program
            .constant_expression_contexts
            .span_or_empty(program.constant_recipes.get(recipe).contexts);
        assert_eq!(contexts, &[first, second, rebound]);
    }
}
