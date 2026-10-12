//! Region-bearing source types keep their exact simultaneous supplying prefix.
//! Provider matching and original call input projection share this read-only
//! scope walk. Resolving a static binder drops its receiving frame before the
//! actual is traversed; a foreign operand instead re-enters its retained owner.

use super::TypeRegionScope;
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::type_identity::{
    AppliedTypeIdentityOwner, AppliedTypeIdentityScope, IndexIdentityArgument,
    LifetimeParameterIdentity, NormalizedTypeIdentity, TypeIdentityRequest,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
pub(crate) struct ApplicationRegions<'scope> {
    identities: &'scope [AppliedTypeIdentityScope<'scope>],
    owner: AppliedTypeIdentityOwner<'scope>,
}

impl<'scope> ApplicationRegions<'scope> {
    pub(crate) fn new(
        application: &'scope AppliedTraitScope<'scope>,
        identities: &'scope [AppliedTypeIdentityScope<'scope>],
    ) -> Self {
        Self::source(application.argument_identity_owner(), identities)
    }

    pub(crate) fn source(
        owner: AppliedTypeIdentityOwner<'scope>,
        identities: &'scope [AppliedTypeIdentityScope<'scope>],
    ) -> Self {
        Self { identities, owner }
    }

    pub(crate) fn prefix(mut self, count: usize) -> Self {
        self.identities = &self.identities[..count];
        self
    }

    pub(crate) fn region(
        self,
        program: &dyn crate::CheckingInput,
        name: &str,
    ) -> Option<LifetimeParameterIdentity> {
        typed_trees::type_identity::scoped_lifetime_parameter_identity(
            program.typed_program(),
            self.owner,
            name,
            self.identities,
        )
    }
}

impl TypeRegionScope for ApplicationRegions<'_> {
    fn erased_identity(
        self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        let mut substitutions = Vec::new();
        let mut starts = Vec::with_capacity(self.identities.len());
        for scope in self.identities {
            starts.push(substitutions.len());
            substitutions.extend(scope.arguments.iter().filter_map(|(parameter, argument)| {
                match argument {
                    IndexIdentityArgument::TypeArgument(reference)
                    | IndexIdentityArgument::TypeReference(reference) => {
                        Some((*parameter, *reference))
                    }
                    _ => None,
                }
            }));
        }
        program.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: &substitutions,
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
            let coordinate = (reference, self.identities.len());
            if visited.contains(&coordinate)
                || !program
                    .type_reference_table
                    .contains_type_reference(reference)
            {
                return None;
            }
            visited.push(coordinate);
            let owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            if owner.is_valid() {
                let supplying = self.identities.iter().rposition(|scope| {
                    program.lifetime_declaration_owner(scope.declaration)
                        == program.lifetime_declaration_owner(owner)
                });
                self.identities = &self.identities[..supplying.map_or(0, |ordinal| ordinal + 1)];
                if supplying.is_none() {
                    self.owner = AppliedTypeIdentityOwner::Declaration(owner);
                }
            }
            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            {
                let supplied =
                    self.identities
                        .iter()
                        .enumerate()
                        .rev()
                        .find_map(|(ordinal, scope)| {
                            scope.arguments.iter().find_map(|(parameter, argument)| {
                                if parameter != symbol {
                                    return None;
                                }
                                match argument {
                                    IndexIdentityArgument::TypeArgument(reference)
                                    | IndexIdentityArgument::TypeReference(reference) => {
                                        Some((*reference, ordinal))
                                    }
                                    _ => None,
                                }
                            })
                        });
                if let Some((actual, ordinal)) = supplied {
                    reference = actual;
                    self.identities = &self.identities[..ordinal];
                    continue;
                }
            }
            let (source, _) = program.project_enclosing_data_type_reference(reference, &[])?;
            if source != reference {
                reference = source;
                continue;
            }
            return Some((reference, self));
        }
    }
}
