//! Keep the source-owned application route across mutable preparation.
//!
//! Materialized operands alone cannot reconstruct an enclosing data or inherited
//! trait telescope. The checking view and provider correspondence therefore borrow
//! the same retained simultaneous frames. This storage is derivation data, not
//! closure, conformance, selection or permission evidence.

use super::{AppliedDeclarationRegionScope, AppliedTraitScope};
use crate::TypedTrees;
use crate::name::Identifier;
use crate::static_applications::StaticMachineLifetimeBinding;
use crate::typed_trees::CallableSubstitution;
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;
use symbols::SymbolHandle;

/// Construction storage for handing a borrowed scope across a mutable pass.
/// Durable rows use `RetainedTraitScope` and arena spans instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitApplicationEnvironment {
    declaration: SymbolHandle,
    argument_owner: SymbolHandle,
    substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    scope_starts: Vec<usize>,
    region_scopes: Vec<AppliedDeclarationRegionScope>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RetainedTraitScope {
    declaration: SymbolHandle,
    argument_owner: SymbolHandle,
    frames: HandleSpan<RetainedTraitApplicationFrame>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RetainedTraitApplicationFrame {
    declaration: SymbolHandle,
    arguments: HandleSpan<CallableSubstitution>,
    lifetimes: HandleSpan<StaticMachineLifetimeBinding>,
}

impl AppliedTraitScope<'_> {
    pub fn environment(&self) -> TraitApplicationEnvironment {
        TraitApplicationEnvironment {
            declaration: self.declaration.symbol,
            argument_owner: self.argument_owner,
            substitutions: self.substitutions.clone(),
            scope_starts: self.substitution_scope_starts.clone(),
            region_scopes: self.region_scopes.clone(),
        }
    }
}

impl TraitApplicationEnvironment {
    /// Resolve an operand before the simultaneous frame whose binder it
    /// replaces. Already-owned operands retain their own lexical prefix.
    pub(crate) fn lifetime_identity(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        preceding_frames: usize,
        name: &str,
        lexical_owner: SymbolHandle,
    ) -> Option<(crate::type_identity::LifetimeParameterIdentity, bool)> {
        let scope = self.scope(program)?;
        scope.with_identity_scopes(program, |frames| {
            let mut visible = frames.get(..preceding_frames)?;
            let recorded_owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            let retained_owner = if recorded_owner.is_valid() {
                recorded_owner
            } else {
                lexical_owner
            };
            let owner = if retained_owner.is_valid() {
                // A reference in an owned row can name its map's region.
                // Match the actual declaring telescope, respecting any local
                // shadow, before choosing the visible application prefix.
                let region_owner = crate::type_identity::lifetime_parameter_identity(
                    program,
                    retained_owner,
                    name,
                )?
                .declaration;
                let frame = visible.iter().rposition(|frame| {
                    program.lifetime_declaration_owner(frame.declaration)
                        == program.lifetime_declaration_owner(region_owner)
                });
                if let Some(ordinal) = frame {
                    visible = &visible[..=ordinal];
                    // The retained owner selects the receiving telescope.
                    // Its actual region operands still belong to the original
                    // supplier, rather than to the map being instantiated.
                    return crate::type_identity::applied_lifetime_parameter_identity_with_supplier(
                        program,
                        retained_owner,
                        self.argument_owner,
                        name,
                        visible,
                    );
                } else {
                    visible = &[];
                    retained_owner
                }
            } else {
                self.argument_owner
            };
            crate::type_identity::applied_lifetime_parameter_identity_with_supplier(
                program,
                owner,
                self.argument_owner,
                name,
                visible,
            )
        })
    }

    pub(crate) fn frame_count(&self) -> usize {
        self.scope_starts.len()
    }

    pub(crate) fn argument_owner(&self) -> SymbolHandle {
        self.argument_owner
    }

    pub fn scope<'program>(
        &self,
        program: &'program TypedTrees,
    ) -> Option<AppliedTraitScope<'program>> {
        Some(AppliedTraitScope {
            declaration: program.trait_definition_by_symbol(self.declaration)?,
            argument_owner: self.argument_owner,
            substitutions: self.substitutions.clone(),
            substitution_scope_starts: self.scope_starts.clone(),
            region_scopes: self.region_scopes.clone(),
        })
    }

    pub fn retain(&self, program: &mut TypedTrees) -> Result<RetainedTraitScope, &'static str> {
        if self.scope_starts.len() != self.region_scopes.len() {
            return Err("trait application lost its simultaneous region frames");
        }
        let mut frames = Vec::with_capacity(self.scope_starts.len());
        for (ordinal, region) in self.region_scopes.iter().enumerate() {
            let start = self.scope_starts[ordinal];
            let end = self
                .scope_starts
                .get(ordinal + 1)
                .copied()
                .unwrap_or(self.substitutions.len());
            let arguments = self
                .substitutions
                .get(start..end)
                .ok_or("trait application has an invalid substitution frame")?;
            let arguments =
                program
                    .callable_substitutions
                    .insert_many(arguments.iter().map(|(parameter, actual)| {
                        CallableSubstitution {
                            parameter: *parameter,
                            actual: *actual,
                        }
                    }));
            let lifetimes =
                program
                    .callable_lifetime_substitutions
                    .insert_many(region.lifetimes.iter().map(|(parameter, actual)| {
                        StaticMachineLifetimeBinding {
                            parameter: Identifier::generated(parameter),
                            argument: Identifier::generated(actual),
                        }
                    }));
            frames.push(RetainedTraitApplicationFrame {
                declaration: region.declaration,
                arguments,
                lifetimes,
            });
        }
        Ok(RetainedTraitScope {
            declaration: self.declaration,
            argument_owner: self.argument_owner,
            frames: program.trait_application_frames.insert_many(frames),
        })
    }
}

impl RetainedTraitScope {
    pub fn supplying_owner(self) -> SymbolHandle {
        self.argument_owner
    }

    /// Supplying declarations describe the telescope; actual operands remain
    /// live dependencies. Retaining a scope must not turn its owner's unrelated
    /// body into an execution root, nor drop a captured const/type declaration.
    pub(crate) fn visit_references(
        self,
        program: &TypedTrees,
        retained: &mut impl crate::references::TypedReferences,
    ) {
        retained.declaration_description_dependency(self.argument_owner);
        for frame in program.trait_application_frames.span_or_empty(self.frames) {
            retained.declaration_description_dependency(frame.declaration);
            for binding in program
                .callable_substitutions
                .span_or_empty(frame.arguments)
            {
                retained.declaration_description_dependency(binding.parameter);
                crate::references::visit_type_reference(program, retained, binding.actual);
            }
        }
    }

    pub fn scope<'program>(
        self,
        program: &'program TypedTrees,
    ) -> Option<AppliedTraitScope<'program>> {
        let mut substitutions = Vec::new();
        let mut scope_starts = Vec::new();
        let mut region_scopes = Vec::new();
        for frame in program.trait_application_frames.span_or_empty(self.frames) {
            scope_starts.push(substitutions.len());
            substitutions.extend(
                program
                    .callable_substitutions
                    .span_or_empty(frame.arguments)
                    .iter()
                    .map(|binding| (binding.parameter, binding.actual)),
            );
            region_scopes.push(AppliedDeclarationRegionScope {
                declaration: frame.declaration,
                lifetimes: program
                    .callable_lifetime_substitutions
                    .span_or_empty(frame.lifetimes)
                    .iter()
                    .map(|binding| {
                        (
                            binding.parameter.as_str().to_owned(),
                            binding.argument.as_str().to_owned(),
                        )
                    })
                    .collect(),
            });
        }
        Some(AppliedTraitScope {
            declaration: program.trait_definition_by_symbol(self.declaration)?,
            argument_owner: self.argument_owner,
            substitutions,
            substitution_scope_starts: scope_starts,
            region_scopes,
        })
    }
}
