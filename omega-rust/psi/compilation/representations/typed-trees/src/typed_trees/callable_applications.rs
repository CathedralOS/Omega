//! Apply an authored callable telescope before typed trees are frozen.
//!
//! A nominal application owns fresh lexical binders and complete substituted
//! contracts, not merely substituted runtime carriers. Declaration rows stay
//! immutable: sharing their binder symbols would make global queries depend on
//! which application happened to be prepared last.

mod payloads;
mod signatures;
mod trait_scopes;

use crate::{TypedTrees, data, signature, types};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AppliedCallableSignature {
    trait_definition: SymbolHandle,
    arguments: arena::HandleSpan<types::TypeReferenceHandle>,
    signature: signature::StateSignature,
}

#[derive(Default)]
struct CallableApplication {
    types: Vec<(SymbolHandle, types::TypeReferenceHandle)>,
    symbols: Vec<(SymbolHandle, SymbolHandle)>,
    active_requirements: Vec<(SymbolHandle, SymbolHandle)>,
}

impl CallableApplication {
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
            .map(|parameter| parameter.symbol)
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
        self.types
            .extend(parameters.into_iter().zip(arguments.iter().copied()));
        self.active_requirements.push((trait_symbol, requirement));
        let applied = self.signature(program, &declared);
        self.active_requirements.pop();
        self.types.truncate(previous_types);
        applied.map(Some)
    }
}

impl TypedTrees {
    /// Apply an invocation's known operands to a callable carrier, including
    /// domain, extent and expression payloads. Arguments keep their caller
    /// scope: substitution is simultaneous, not a sequence of textual rewrites.
    /// This produces a view for inference; it admits no callable or bound.
    pub fn apply_callable_type(
        &mut self,
        original: types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        CallableApplication {
            types: substitutions.to_vec(),
            ..CallableApplication::default()
        }
        .type_reference(self, original)
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
