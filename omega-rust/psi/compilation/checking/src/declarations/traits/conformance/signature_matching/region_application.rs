//! Region comparison consumes the same simultaneous application scopes as
//! ordinary conformance formation. An applied operand's regions already live
//! in the provider; reinterpreting them through the trait telescope changes
//! their owner. Callable alpha-binders normalize on both sides of the join.

use super::{TraitTypeBinding, TraitTypeBindingTarget};
use crate::type_regions::TypeRegionScope;
use typed_trees::type_identity::{NormalizedTypeIdentity, TypeIdentityRequest};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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

    fn resolve(
        mut self,
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Self)> {
        let mut visited = Vec::new();
        loop {
            if !reference.is_valid() {
                return Some((reference, self));
            }
            if !program
                .type_reference_table
                .contains_type_reference(reference)
                || visited.contains(&reference)
            {
                return None;
            }
            visited.push(reference);
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return Some((reference, self));
            };
            if !self.provider
                && let Some(binding) = self
                    .bindings
                    .iter()
                    .rev()
                    .find(|binding| binding.parameter_symbol == *symbol)
                && let TraitTypeBindingTarget::Type(argument) = binding.target
            {
                reference = argument;
                self.bindings = &self.bindings[..binding.scope_start];
                self.provider = self.bindings.is_empty();
                continue;
            }
            if let Some(application) = program
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
            {
                reference = application;
                continue;
            }
            return Some((reference, self));
        }
    }
}
