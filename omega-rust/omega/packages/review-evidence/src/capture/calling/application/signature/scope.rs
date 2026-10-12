//! Keep the supplying declaration's regions separate from receiving binders.
//! Explicit trait lifetime arguments and regions inside type operands use the
//! same supplying coordinates, so repeated arguments retain their equality.

use super::instantiation;
use crate::capture::PackageReviewInput;
use crate::capture::calling::rejected;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::trait_definition::{AppliedTraitScope, TraitDefinition};
use typed_trees::types::TypeReferenceHandle;

pub(super) struct ApplicationScope {
    pub binders: Vec<Identifier>,
    pub trait_arguments: Vec<Identifier>,
    pub supplying_count: usize,
    owner: SymbolHandle,
}

impl ApplicationScope {
    pub fn new(
        compilation: &PackageReviewInput<'_>,
        root: &TraitDefinition,
        arguments: &[TypeReferenceHandle],
        owner: SymbolHandle,
    ) -> Result<Self, Vec<Diagnostic>> {
        if owner == root.symbol {
            return Ok(Self {
                binders: root.lifetime_parameters.clone(),
                trait_arguments: root.lifetime_parameters.clone(),
                supplying_count: 0,
                owner,
            });
        }
        let conformance = compilation
            .conformance_by_symbol(owner)
            .ok_or_else(|| rejected("calling application lost its supplying conformance"))?;
        if conformance.trait_symbol != root.symbol
            || compilation
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                != arguments
            || AppliedTraitScope::from_conformance(compilation, conformance).is_none()
        {
            return Err(rejected(
                "calling application differs from its supplying conformance",
            ));
        }
        let supplying = conformance
            .lifetime_parameters
            .iter()
            .enumerate()
            .map(|(ordinal, _)| {
                Identifier::generated(format!("$calling-application-region-{ordinal}"))
            })
            .collect::<Vec<_>>();
        let trait_arguments = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| {
                supplying
                    .get(*ordinal as usize)
                    .cloned()
                    .ok_or_else(|| rejected("calling trait lifetime lost its supplying coordinate"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let supplying_count = supplying.len();
        let mut binders = root.lifetime_parameters.clone();
        binders.extend(supplying);
        Ok(Self {
            binders,
            trait_arguments,
            supplying_count,
            owner,
        })
    }

    pub fn instantiate_argument(
        &self,
        compilation: &PackageReviewInput<'_>,
        scratch: &mut TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
        let offset = self.binders.len() - self.supplying_count;
        let binders = if self.supplying_count == 0 {
            self.binders.as_slice()
        } else {
            &self.binders[offset..]
        };
        instantiation::instantiate_application(scratch, compilation, reference, self.owner, binders)
    }

    pub fn coordinate(&self, name: &Identifier) -> Result<u32, Vec<Diagnostic>> {
        self.binders
            .iter()
            .position(|binder| binder == name)
            .and_then(|ordinal| u32::try_from(ordinal).ok())
            .ok_or_else(|| rejected("calling lifetime has no containing telescope coordinate"))
    }
}
