//! Region comparison consumes the same simultaneous application scopes as
//! ordinary conformance formation. An applied operand's regions already live
//! in the provider; reinterpreting them through the trait telescope changes
//! their owner. Callable alpha-binders normalize on both sides of the join.

use super::{TraitTypeBinding, TraitTypeBindingTarget};
use crate::type_regions::{ReferenceStep, TypeRegionScope};
use symbols::SymbolHandle;
use typed_trees::type_identity::{NormalizedTypeIdentity, TypeIdentityRequest};
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Copy)]
pub(super) struct RegionApplicationScope<'scope> {
    bindings: &'scope [TraitTypeBinding],
    provider: bool,
}

impl<'scope> RegionApplicationScope<'scope> {
    pub(super) fn requirement(bindings: &'scope [TraitTypeBinding]) -> Self {
        Self {
            bindings,
            provider: false,
        }
    }

    pub(super) fn provider(bindings: &'scope [TraitTypeBinding]) -> Self {
        Self {
            bindings,
            provider: true,
        }
    }

    pub(super) fn is_provider(self) -> bool {
        self.provider
    }
}

impl TypeRegionScope for RegionApplicationScope<'_> {
    fn erased_identity(
        self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        let mut substitutions = Vec::new();
        let mut starts = Vec::new();
        let mut binders = Vec::new();
        for (ordinal, binding) in self.bindings.iter().enumerate() {
            match binding.target {
                TraitTypeBindingTarget::Type(argument) if !self.provider => {
                    substitutions.push((binding.parameter_symbol, argument));
                    let start = self.bindings[..binding.scope_start]
                        .iter()
                        .filter(|binding| matches!(binding.target, TraitTypeBindingTarget::Type(_)))
                        .count();
                    if starts.last() != Some(&start) {
                        starts.push(start);
                    }
                }
                TraitTypeBindingTarget::Parameter(actual) => {
                    let label = format!("$conformance{ordinal}");
                    binders.push((binding.parameter_symbol, label.clone()));
                    binders.push((actual, label));
                }
                _ => {}
            }
        }
        program.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: &substitutions,
                binders: &binders,
                ..TypeIdentityRequest::ordinary(reference)
            },
            &starts,
        )
    }

    fn named_alias(
        mut self,
        _program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
        _reference: TypeReferenceHandle,
    ) -> Option<ReferenceStep<Self>> {
        if self.provider {
            return None;
        }
        let bindings = self.bindings;
        let binding = bindings
            .iter()
            .rev()
            .find(|binding| binding.parameter_symbol == symbol)?;
        let TraitTypeBindingTarget::Type(argument) = binding.target else {
            return None;
        };
        let bindings = &bindings[..binding.scope_start];
        self.bindings = bindings;
        self.provider = bindings.is_empty();
        Some(ReferenceStep::Chase(self, argument))
    }
}
