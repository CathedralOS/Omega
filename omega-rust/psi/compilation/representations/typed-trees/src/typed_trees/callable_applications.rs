//! Apply an authored callable telescope before typed trees are frozen.
//!
//! An application owns fresh lexical binders and complete substituted
//! contracts, not merely substituted runtime carriers. Declaration rows stay
//! immutable: sharing their binder symbols would make global queries depend on
//! which application happened to be prepared last.
//! Trait scopes own universal requirement views; static selection occurrences
//! own determined provider tuples. Neither view is an admission or an instance.

mod closed_operands;
mod conformance_scopes;
mod machine_selections;
mod payloads;
mod requirement_instances;
mod selected_operands;
mod signatures;
mod trait_scopes;

pub use requirement_instances::ClosedRequirementApplication;

use crate::{TypedTrees, data, signature, types};
use symbols::SymbolHandle;

impl TypedTrees {
    /// Read every prepared callable's declaration scope without replacing the
    /// authored roster. Contract occurrence readers need both: applied facts
    /// retain fresh formal symbols and their own substituted result carrier.
    pub fn prepared_callable_signatures(&self) -> impl Iterator<Item = &signature::StateSignature> {
        self.applied_callable_signatures
            .iter()
            .map(|(_, prepared)| &prepared.signature)
    }

    /// Project declaration-owned application rows with the declaration roster.
    /// Backing arenas are not checking roots: a private evaluation projection
    /// cannot retain an instance obligation after removing its known source
    /// trait. Unknown owners remain obligations, and the original input keeps
    /// its complete rows. Occurrence-owned machine selections are separate.
    pub fn remove_trait_declaration_applications(
        &mut self,
        removed: impl Fn(SymbolHandle) -> bool,
    ) {
        let applications = self
            .applied_callable_signatures
            .iter()
            .filter_map(|(handle, application)| match application.owner {
                AppliedCallableOwner::Trait { declaration, .. }
                | AppliedCallableOwner::RequirementInstance { declaration, .. }
                    if removed(declaration) =>
                {
                    Some(handle)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for application in applications {
            self.applied_callable_signatures.free(application);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AppliedCallableSignature {
    owner: AppliedCallableOwner,
    signature: signature::StateSignature,
}

impl AppliedCallableSignature {
    pub(super) fn requirement_instance_symbol(&self) -> Option<SymbolHandle> {
        matches!(self.owner, AppliedCallableOwner::RequirementInstance { .. })
            .then_some(self.signature.symbol)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum AppliedCallableOwner {
    #[default]
    Absent,
    Trait {
        declaration: SymbolHandle,
        application: crate::type_identity::NormalizedTypeIdentity,
    },
    RequirementInstance {
        declaration: SymbolHandle,
        template: SymbolHandle,
        trait_application_identity: crate::type_identity::NormalizedTypeIdentity,
        portable_trait_application_identity: crate::type_identity::NormalizedTypeIdentity,
        portable_local_arguments: arena::HandleSpan<crate::type_identity::NormalizedTypeIdentity>,
        trait_scope: crate::trait_definition::RetainedTraitScope,
        trait_arguments: arena::HandleSpan<types::TypeReferenceHandle>,
        local_arguments: arena::HandleSpan<types::TypeReferenceHandle>,
        application: crate::static_applications::StaticMachineApplicationHandle,
    },
    MachineSelection {
        invocation: crate::static_applications::StaticMachineApplicationHandle,
        ordinal: usize,
        demand: SymbolHandle,
        selected: SymbolHandle,
        provider: crate::static_applications::StaticMachineApplicationHandle,
        required: signature::StateSignature,
        source_required: signature::StateSignature,
        caller_operands: arena::HandleSpan<CallableSubstitution>,
        caller_lifetimes:
            arena::HandleSpan<crate::static_applications::StaticMachineLifetimeBinding>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallableSubstitution {
    pub parameter: SymbolHandle,
    pub actual: types::TypeReferenceHandle,
}

/// A symbolic application is a checking view, not an executable instance.
pub struct AppliedMachineCallable<'a> {
    pub lexical_owner: SymbolHandle,
    pub provider: crate::static_applications::StaticMachineApplicationHandle,
    pub required: &'a signature::StateSignature,
    pub selected: &'a signature::StateSignature,
    pub source_required: &'a signature::StateSignature,
    pub caller_operands: &'a [CallableSubstitution],
    pub caller_lifetimes: &'a [crate::static_applications::StaticMachineLifetimeBinding],
}

#[derive(Default)]
struct CallableApplication {
    region_scope: SymbolHandle,
    /// Complete captured application during materialization. Local callable
    /// binders remain in `lifetimes` and mask same-spelled captured regions.
    trait_region_application: Option<(crate::trait_definition::TraitApplicationEnvironment, usize)>,
    lifetime_declaration: SymbolHandle,
    lifetimes: Vec<(crate::name::Identifier, crate::name::Identifier)>,
    elided_lifetimes: Vec<crate::name::Identifier>,
    types: Vec<(SymbolHandle, types::TypeReferenceHandle)>,
    static_arguments: Vec<(SymbolHandle, crate::expression::StaticMachineArgument)>,
    symbols: Vec<(SymbolHandle, SymbolHandle)>,
    active_requirements: Vec<(SymbolHandle, SymbolHandle)>,
    source_binders: Vec<(crate::name::Identifier, SymbolHandle)>,
}

impl CallableApplication {
    fn scoped_lifetime(
        &self,
        program: &TypedTrees,
        reference: types::TypeReferenceHandle,
        original: &crate::name::Identifier,
    ) -> Result<
        (
            crate::name::Identifier,
            crate::type_identity::LifetimeParameterIdentity,
            bool,
        ),
        &'static str,
    > {
        if self
            .lifetimes
            .iter()
            .any(|(parameter, _)| parameter == original)
        {
            return Ok((self.lifetime(original), Default::default(), false));
        }
        let Some((environment, preceding_frames)) = &self.trait_region_application else {
            return Ok((original.clone(), Default::default(), false));
        };
        let (region, substituted) = environment
            .lifetime_identity(
                program,
                reference,
                *preceding_frames,
                original.as_str(),
                self.region_scope,
            )
            .ok_or("applied trait operand has no exact supplying region")?;
        let name = crate::type_identity::lifetime_parameter_name(program, region)
            .ok_or("applied trait operand lost its region declaration ordinal")?
            .clone();
        Ok((name, region, substituted))
    }

    fn lifetime(&self, original: &crate::name::Identifier) -> crate::name::Identifier {
        self.lifetimes
            .iter()
            .rev()
            .find(|(parameter, _)| parameter == original)
            .map_or_else(|| original.clone(), |(_, actual)| actual.clone())
    }

    fn symbol(&self, original: SymbolHandle) -> SymbolHandle {
        self.symbols
            .iter()
            .rev()
            .find(|(symbol, _)| *symbol == original)
            .map_or(original, |(_, replacement)| *replacement)
    }

    fn nominal(
        &mut self,
        program: &mut TypedTrees,
        trait_symbol: SymbolHandle,
        requirement: SymbolHandle,
        arguments: &[types::TypeReferenceHandle],
    ) -> Result<Option<signature::StateSignature>, &'static str> {
        let Some(definition) = program.trait_definition_by_symbol(trait_symbol) else {
            return Ok(None);
        };
        let parameters = program
            .trait_type_parameters(definition)
            .iter()
            .map(|parameter| (parameter.name.clone(), parameter.symbol))
            .collect::<Vec<_>>();
        if parameters.len() != arguments.len() {
            return Ok(None);
        }
        let Some(declared) = program
            .trait_machine_signature_by_symbol(definition, requirement)
            .cloned()
        else {
            return Ok(None);
        };
        // Eagerly expanding a recursive requirement has no finite telescope.
        // Refuse explicitly rather than publishing a residual generic subtree
        // or inventing an expansion limit. A graph-backed recursive application
        // representation must precede support for that language composition.
        if self
            .active_requirements
            .contains(&(trait_symbol, requirement))
        {
            return Err(
                "recursive nominal callable application is not yet represented by cycle-aware applied telescopes",
            );
        }
        let previous_types = self.types.len();
        let previous_binders = self.source_binders.len();
        self.source_binders.extend(parameters.iter().cloned());
        self.types.extend(
            parameters
                .into_iter()
                .map(|(_, symbol)| symbol)
                .zip(arguments.iter().copied()),
        );
        self.active_requirements.push((trait_symbol, requirement));
        let applied = self.signature(program, &declared);
        self.active_requirements.pop();
        self.types.truncate(previous_types);
        self.source_binders.truncate(previous_binders);
        applied.map(Some)
    }
}

impl TypedTrees {
    /// Instantiate an inferred result's reference regions in the caller.
    /// Elision retains the call's separate loan-source contract; it never
    /// rewrites the authored declaration or supplies an anonymous nominal tag.
    pub fn apply_callable_result_type(
        &mut self,
        original: types::TypeReferenceHandle,
        lifetimes: &[(crate::name::Identifier, Option<crate::name::Identifier>)],
        declaration: SymbolHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        let mut application = CallableApplication {
            region_scope: declaration,
            ..CallableApplication::default()
        };
        for (parameter, argument) in lifetimes {
            if let Some(argument) = argument {
                application
                    .lifetimes
                    .push((parameter.clone(), argument.clone()));
            } else {
                application.elided_lifetimes.push(parameter.clone());
            }
        }
        application.type_reference(self, original)
    }

    /// Apply an invocation's known operands to a callable carrier, including
    /// domain, extent and expression payloads. Arguments keep their caller
    /// scope: substitution is simultaneous, not a sequence of textual rewrites.
    /// This produces a view for inference; it admits no callable or bound.
    /// With no type substitutions, valid carrier and expression nodes are copied,
    /// so callers may remap lexical binders without reopening the declaration.
    pub fn apply_callable_type(
        &mut self,
        original: types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
        lifetimes: &[(crate::name::Identifier, crate::name::Identifier)],
        declaration: SymbolHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        CallableApplication {
            region_scope: declaration,
            types: substitutions.to_vec(),
            lifetimes: lifetimes.to_vec(),
            ..CallableApplication::default()
        }
        .type_reference(self, original)
    }

    /// Apply captured regions through their original simultaneous frames.
    /// A method's own region telescope masks equally spelled outer captures.
    pub fn apply_captured_callable_type(
        &mut self,
        original: types::TypeReferenceHandle,
        environment: &crate::trait_definition::TraitApplicationEnvironment,
        local_lifetimes: &[crate::name::Identifier],
        declaration: SymbolHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        CallableApplication {
            region_scope: declaration,
            trait_region_application: Some((environment.clone(), environment.frame_count())),
            lifetimes: local_lifetimes
                .iter()
                .map(|name| (name.clone(), name.clone()))
                .collect(),
            ..CallableApplication::default()
        }
        .type_reference(self, original)
    }

    /// Copy a retained callable binder contract with exact lexical remapping.
    /// Nested callable telescopes are freshened by the same application owner
    /// as ordinary selected signatures; copying establishes no admission.
    pub fn copy_callable_parameter_kind(
        &mut self,
        kind: data::TypeParameterKind,
        symbols: &[(SymbolHandle, SymbolHandle)],
        declaration: SymbolHandle,
    ) -> Result<data::TypeParameterKind, &'static str> {
        CallableApplication {
            region_scope: declaration,
            symbols: symbols.to_vec(),
            ..CallableApplication::default()
        }
        .parameter_kind(self, kind)
    }

    /// Prepare nominal callable applications after all declarations are lowered.
    /// Invalid identities/arity remain absent for ordinary declaration checking;
    /// an application that cannot be represented fails instead of looking applied.
    pub fn instantiate_nominal_machine_parameter_contracts(&mut self) -> Result<(), &'static str> {
        let pending = self
            .data_type_parameters
            .iter()
            .filter_map(|(handle, parameter)| match &parameter.kind {
                data::TypeParameterKind::Machine {
                    contract:
                        data::MachineParameterContract::Nominal {
                            trait_definition,
                            requirement,
                            trait_arguments,
                            instantiated: None,
                        },
                } if !trait_arguments.is_empty() => Some((
                    handle,
                    *trait_definition,
                    *requirement,
                    trait_arguments.clone(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (handle, trait_symbol, requirement, arguments) in pending {
            let applied = CallableApplication::default().nominal(
                self,
                trait_symbol,
                requirement,
                &arguments,
            )?;
            if let data::TypeParameterKind::Machine {
                contract: data::MachineParameterContract::Nominal { instantiated, .. },
            } = &mut self.data_type_parameters.get_mut(handle).kind
            {
                *instantiated = applied.map(Box::new);
            }
        }
        Ok(())
    }
}
