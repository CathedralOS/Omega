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
use arena::{Arena, Handle};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;

/// Which authored node a recipe keeps symbolic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RetainedConstantPosition {
    /// The arena's invalid sentinel only.
    #[default]
    None,
    /// `[T; call()]`: the exact `FixedArray` type-reference node.
    ArrayLength(TypeReferenceHandle),
    /// `T[lo..=call()]`: the endpoint call expression inside the range.
    RangeEndpoint(ExpressionHandle),
    /// `Data<call()>`: the `ConstExpression` argument node and its authored
    /// root expression.
    ConstApplication {
        site: TypeReferenceHandle,
        root: ExpressionHandle,
    },
}

/// One retained symbolic constant application.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetainedConstantRecipe {
    pub position: RetainedConstantPosition,
    /// The zero-argument machine the position applies, resolved in the
    /// authored scope; diagnostics and receipts name it.
    pub machine: SymbolHandle,
    pub name: Identifier,
    pub source_span: source::SourceSpan,
}

pub type ConstantRecipeHandle = Handle<RetainedConstantRecipe>;

/// The exact selected realization one closure evaluated a dependency under.
/// Equal values under different realizations are different closures.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedConstantRecipe {
    pub recipe: ConstantRecipeHandle,
    pub value: BigInt,
}

/// Every recipe's value under one target capsule, with the realization
/// receipts the evaluation consumed. Empty in the shared program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstantRecipeClosure {
    /// Primary value application; supplemental proof receipts keep own scopes.
    pub target_scope: String,
    pub values: Vec<ClosedConstantRecipe>,
    pub receipts: Vec<ConstantRecipeDependencyReceipt>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    /// Canonical nullary application of an authored array extent. Source
    /// positions remain distinct formation obligations; array type identity
    /// names this exact resolved application rather than its source occurrence.
    /// Generic argument recipes can contain larger expressions and do not share
    /// this equivalence, even when one dependency names the same machine.
    pub fn array_extent_application(&self, recipe: ConstantRecipeHandle) -> Option<SymbolHandle> {
        if !self.constant_recipes.is_valid(recipe) {
            return None;
        }
        let recipe = self.constant_recipes.get(recipe);
        let RetainedConstantPosition::ArrayLength(site) = recipe.position else {
            return None;
        };
        match self.type_reference_table.type_reference(site) {
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::ConstCall { machine, .. },
                ..
            } if machine.is_valid() && *machine == recipe.machine => Some(*machine),
            _ => None,
        }
    }

    /// Geometry of an equivalent array application in this admitted capsule.
    /// Only already-closed sites participate; this lookup grants no visibility,
    /// schedules no evaluation, and never combines values from other capsules.
    pub fn closed_array_extent_value(
        &self,
        recipe: ConstantRecipeHandle,
    ) -> Result<Option<&BigInt>, &'static str> {
        let application = self
            .array_extent_application(recipe)
            .ok_or("array extent lost its checked nullary application")?;
        let mut value = None;
        for closed in &self.constant_recipe_closure.values {
            if self.array_extent_application(closed.recipe) != Some(application) {
                continue;
            }
            if value.is_some_and(|previous| previous != &closed.value) {
                return Err("equivalent array extent applications disagree in one target capsule");
            }
            value = Some(&closed.value);
        }
        Ok(value)
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

    /// Read the selected recipes from `evaluated`, a copy of this program
    /// whose deferred continuation folded under selected rows. A
    /// position the fold left symbolic names the recipe that did not close.
    pub fn read_constant_recipe_closure(
        &self,
        evaluated: &crate::TypedTrees,
        recipes: &[ConstantRecipeHandle],
        receipts: Vec<ConstantRecipeDependencyReceipt>,
    ) -> Result<ConstantRecipeClosure, Vec<diagnostics::Diagnostic>> {
        let mut values = Vec::with_capacity(recipes.len());
        let mut unclosed = Vec::new();
        for &handle in recipes {
            let recipe = self.constant_recipes.get(handle);
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
                RetainedConstantPosition::RangeEndpoint(expression) => {
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
    roster: &mut Arena<RetainedConstantRecipe>,
    recipes: impl IntoIterator<Item = RetainedConstantRecipe>,
) {
    for recipe in recipes {
        let duplicate = roster
            .iter()
            .any(|(_, retained)| retained.position == recipe.position);
        if !duplicate {
            roster.insert(recipe);
        }
    }
}
