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
    captured_argument_owner:
        Option<crate::typed_trees::product_applications::CapturedStaticLexicalScope>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RetainedTraitScope {
    declaration: SymbolHandle,
    argument_owner: SymbolHandle,
    frames: HandleSpan<RetainedTraitApplicationFrame>,
    captured_argument_owner:
        arena::Handle<crate::typed_trees::product_applications::CapturedStaticLexicalScopeSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetainedTraitApplicationFrame {
    declaration: SymbolHandle,
    arguments: HandleSpan<CallableSubstitution>,
    lifetimes: HandleSpan<StaticMachineLifetimeBinding>,
    role: crate::type_identity::AppliedTypeIdentityRegionRole,
    binding_application: crate::static_applications::StaticMachineApplicationHandle,
    binding_parameters: HandleSpan<SymbolHandle>,
}

impl Default for RetainedTraitApplicationFrame {
    fn default() -> Self {
        Self {
            declaration: Default::default(),
            arguments: Default::default(),
            lifetimes: Default::default(),
            role: crate::type_identity::AppliedTypeIdentityRegionRole::Supplied,
            binding_application: Default::default(),
            binding_parameters: Default::default(),
        }
    }
}

impl AppliedTraitScope<'_> {
    pub fn environment(&self) -> TraitApplicationEnvironment {
        TraitApplicationEnvironment {
            declaration: self.declaration.symbol,
            argument_owner: self.argument_owner,
            substitutions: self.substitutions.clone(),
            scope_starts: self.substitution_scope_starts.clone(),
            region_scopes: self.region_scopes.clone(),
            captured_argument_owner: self.captured_argument_owner.clone(),
        }
    }
}

/// Relocate only owned graph/application payloads. Declaration coordinates,
/// simultaneous boundaries and captured namespaces remain with the original
/// environment. The copier supplies the actual source and destination owners.
pub trait TraitApplicationPayloadCopier {
    type Error;

    fn type_reference(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<TypeReferenceHandle, Self::Error>;

    fn static_machine_application(
        &mut self,
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Result<crate::static_applications::StaticMachineApplicationHandle, Self::Error>;
}

impl TraitApplicationEnvironment {
    /// Copy original payloads before a mutable consumer reads their handles.
    /// The original binder roster, frame roles and captured source identity are
    /// preserved; equal names or values never create a replacement frame.
    pub fn copy_payloads<Copier: TraitApplicationPayloadCopier>(
        &self,
        copier: &mut Copier,
    ) -> Result<Self, Copier::Error> {
        let mut copied = self.clone();
        for (_, actual) in &mut copied.substitutions {
            *actual = copier.type_reference(*actual)?;
        }
        for frame in &mut copied.region_scopes {
            frame.binding_application =
                copier.static_machine_application(frame.binding_application)?;
        }
        Ok(copied)
    }

    /// Resolve an operand before the simultaneous frame whose binder it
    /// replaces. Already-owned operands retain their own lexical prefix.
    pub(crate) fn lifetime_identity(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        preceding_frames: usize,
        name: &str,
    ) -> Option<crate::type_identity::LifetimeParameterIdentity> {
        let scope = self.scope(program)?;
        scope.with_identity_scopes(program, |frames| {
            let mut visible = frames.get(..preceding_frames)?;
            let retained_owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            let owner = if retained_owner.is_valid() {
                visible = visible
                    .iter()
                    .rposition(|frame| {
                        program.lifetime_declaration_owner(frame.declaration)
                            == program.lifetime_declaration_owner(retained_owner)
                    })
                    .map_or(&[][..], |ordinal| &visible[..=ordinal]);
                retained_owner
            } else {
                self.argument_owner
            };
            crate::type_identity::applied_lifetime_parameter_identity(program, owner, name, visible)
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
            captured_argument_owner: self.captured_argument_owner.clone(),
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
                            argument:
                                crate::static_applications::StaticMachineLifetimeArgument::inferred(
                                    actual.as_deref().map(Identifier::generated).as_ref(),
                                ),
                        }
                    }));
            frames.push(RetainedTraitApplicationFrame {
                declaration: region.declaration,
                arguments,
                lifetimes,
                role: region.role,
                binding_application: region.binding_application,
                binding_parameters: program
                    .trait_application_binding_parameters
                    .insert_many(region.binding_parameters.iter().copied()),
            });
        }
        Ok(RetainedTraitScope {
            declaration: self.declaration,
            argument_owner: self.argument_owner,
            frames: program.trait_application_frames.insert_many(frames),
            captured_argument_owner: self
                .captured_argument_owner
                .as_ref()
                .map(|scope| program.retain_captured_static_lexical_scope(scope))
                .unwrap_or_default(),
        })
    }
}

impl RetainedTraitScope {
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
                            binding
                                .argument
                                .named()
                                .map(|argument| argument.as_str().to_owned()),
                        )
                    })
                    .collect(),
                role: frame.role,
                binding_application: frame.binding_application,
                binding_parameters: program
                    .trait_application_binding_parameters
                    .span_or_empty(frame.binding_parameters)
                    .to_vec(),
            });
        }
        Some(AppliedTraitScope {
            declaration: program.trait_definition_by_symbol(self.declaration)?,
            argument_owner: self.argument_owner,
            substitutions,
            substitution_scope_starts: scope_starts,
            region_scopes,
            captured_argument_owner: if self.captured_argument_owner.is_valid() {
                Some(
                    program
                        .captured_static_lexical_scope(self.captured_argument_owner)?
                        .clone(),
                )
            } else {
                None
            },
        })
    }
}
