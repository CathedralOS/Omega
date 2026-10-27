//! Join a provider declaration's public region slots to one receiving application.
//!
//! Build chooses a declaration, not its caller's regions. This query compares
//! every outer operand, including phantom regions and unused public arguments,
//! under the original simultaneous scopes. Only that named declaration's slots
//! may substitute; captured foreign coordinates remain rigid. Callable-local
//! lifetime/loan refinement is a separate judgment and is not inferred here.
//! A correspondence is derivation data, not provider admission. Execution must
//! retain it with the checked application through the receiving installation.

use crate::type_regions::{
    ReferenceStep, TypeRegionRelationship, TypeRegionScope, type_reference_region_applications,
};
use symbols::SymbolHandle;
use typed_trees::trait_definition::{AppliedTraitScope, Conformance};
use typed_trees::type_identity::{
    AppliedTypeIdentityScope, LifetimeParameterIdentity, NormalizedTypeIdentity,
    TypeIdentityRequest, applied_lifetime_parameter_identity,
};
use typed_trees::types::TypeReferenceHandle;

/// Receiving coordinates in the selecting declaration's public binder order.
/// This invocation-local result grants neither a loan nor selection authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRegionCorrespondence {
    pub selecting_declaration: SymbolHandle,
    pub receiving_regions: Vec<LifetimeParameterIdentity>,
}

/// Compare one complete receiving view with a named provider's checked nominal
/// application. Repeated slots must agree, and every provider slot must have an
/// actual; unmentioned slots are not filled from a caller or a rendered name.
pub fn provider_region_correspondence(
    program: &dyn crate::CheckingInput,
    provider: &Conformance,
    receiver: &AppliedTraitScope<'_>,
) -> Option<ProviderRegionCorrespondence> {
    // Type/const generic coverage is independent of region-slot substitution.
    // No symbolic non-region provider operand is admitted by this query.
    if !program.conformance_type_parameters(provider).is_empty() {
        return None;
    }
    let root = AppliedTraitScope::from_conformance(program.typed_program(), provider)?;
    let supplied = root.declaring_scope(program.typed_program(), receiver.declaration.symbol)?;
    let mut applications = vec![vec![None; provider.lifetime_parameters.len()]];
    let matched = supplied.with_identity_scopes(program.typed_program(), |supplied_scopes| {
        receiver.with_identity_scopes(program.typed_program(), |receiving_scopes| {
            let supplied_environment = ApplicationRegions::new(&supplied, supplied_scopes);
            let receiving_environment = ApplicationRegions::new(receiver, receiving_scopes);
            for parameter in &receiver.declaration.lifetime_parameters {
                let Some(actual) = supplied_environment.region(program, parameter.as_str()) else {
                    return false;
                };
                let Some(required) = receiving_environment.region(program, parameter.as_str())
                else {
                    return false;
                };
                applications.retain_mut(|regions| {
                    constrain_slot(provider.symbol, regions, actual, required)
                });
                if applications.is_empty() {
                    return false;
                }
            }
            let mut constrain =
                |regions: &mut Vec<Option<LifetimeParameterIdentity>>,
                 actual: ApplicationRegions<'_>,
                 required: ApplicationRegions<'_>,
                 _reference,
                 actual_name: Option<&typed_trees::name::Identifier>,
                 required_name: Option<&typed_trees::name::Identifier>| {
                    match (actual_name, required_name) {
                        (None, None) => true,
                        (Some(actual_name), Some(required_name)) => actual
                            .region(program, actual_name.as_str())
                            .zip(required.region(program, required_name.as_str()))
                            .is_some_and(|(actual, required)| {
                                constrain_slot(provider.symbol, regions, actual, required)
                            }),
                        _ => false,
                    }
                };
            for parameter in program.trait_type_parameters(receiver.declaration) {
                // Region comparison cannot make different static carriers,
                // domain applications or const values equal. Exact erased
                // identity separates static operands first. The exact region
                // walk also covers both qualification rosters: erasure may
                // collapse terms whose regional arguments remain distinct.
                if supplied.parameter_identity(
                    program.typed_program(),
                    parameter.symbol,
                    typed_trees::type_identity::TypeIdentityQualification::Ordinary,
                ) != receiver.parameter_identity(
                    program.typed_program(),
                    parameter.symbol,
                    typed_trees::type_identity::TypeIdentityQualification::Ordinary,
                ) {
                    return false;
                }
                let Some((actual, actual_scopes)) =
                    supplied.parameter_application(parameter.symbol)
                else {
                    return false;
                };
                let Some((required, required_scopes)) =
                    receiver.parameter_application(parameter.symbol)
                else {
                    return false;
                };
                if !type_reference_region_applications(
                    program,
                    actual,
                    required,
                    supplied_environment.prefix(actual_scopes),
                    receiving_environment.prefix(required_scopes),
                    TypeRegionRelationship::ExactApplication,
                    &mut applications,
                    &mut constrain,
                ) {
                    return false;
                }
            }
            true
        })
    });
    matched.then_some(())?;
    // Unmapped slots cannot be supplied by losing domain alternatives. Settle
    // only after the entire inherited application has constrained every slot.
    let mut complete = Vec::new();
    for regions in applications {
        if let Some(regions) = regions.into_iter().collect::<Option<Vec<_>>>()
            && !complete.contains(&regions)
        {
            complete.push(regions);
        }
    }
    let [regions] = complete.as_slice() else {
        return None;
    };
    Some(ProviderRegionCorrespondence {
        selecting_declaration: provider.symbol,
        receiving_regions: regions.clone(),
    })
}

fn constrain_slot(
    selecting_declaration: SymbolHandle,
    receiving: &mut [Option<LifetimeParameterIdentity>],
    supplied: LifetimeParameterIdentity,
    required: LifetimeParameterIdentity,
) -> bool {
    if supplied.declaration != selecting_declaration {
        return supplied == required;
    }
    let Some(slot) = receiving.get_mut(supplied.ordinal) else {
        return false;
    };
    match slot {
        Some(previous) => *previous == required,
        None => {
            *slot = Some(required);
            true
        }
    }
}

#[derive(Clone, Copy)]
struct ApplicationRegions<'scope> {
    application: &'scope AppliedTraitScope<'scope>,
    identities: &'scope [AppliedTypeIdentityScope<'scope>],
    owner: SymbolHandle,
    visible: usize,
}

impl<'scope> ApplicationRegions<'scope> {
    fn new(
        application: &'scope AppliedTraitScope<'scope>,
        identities: &'scope [AppliedTypeIdentityScope<'scope>],
    ) -> Self {
        Self {
            application,
            identities,
            owner: application.argument_owner,
            visible: application.substitutions.len(),
        }
    }

    fn prefix(mut self, count: usize) -> Self {
        self.visible = self
            .application
            .substitution_scope_starts
            .get(count)
            .copied()
            .unwrap_or(self.application.substitutions.len());
        self.identities = &self.identities[..count];
        self
    }

    fn region(
        self,
        program: &dyn crate::CheckingInput,
        name: &str,
    ) -> Option<LifetimeParameterIdentity> {
        applied_lifetime_parameter_identity(
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
        program.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: &self.application.substitutions[..self.visible],
                ..TypeIdentityRequest::ordinary(reference)
            },
            &self.application.substitution_scope_starts[..self.identities.len()],
        )
    }

    fn enter_operand(mut self, program: &dyn crate::CheckingInput, owner: SymbolHandle) -> Self {
        let count = self
            .identities
            .iter()
            .rposition(|identity| {
                program.lifetime_declaration_owner(identity.declaration)
                    == program.lifetime_declaration_owner(owner)
            })
            .map_or(0, |ordinal| ordinal + 1);
        self = self.prefix(count);
        self.owner = owner;
        self
    }

    fn named_alias(
        mut self,
        _program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
        reference: TypeReferenceHandle,
    ) -> Option<ReferenceStep<Self>> {
        let substitutions = &self.application.substitutions;
        let visible = self.visible;
        let (ordinal, argument) = substitutions[..visible]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (parameter, _))| *parameter == symbol)
            .map(|(ordinal, (_, argument))| (ordinal, *argument))?;
        if argument == reference {
            return Some(ReferenceStep::Stop);
        }
        let Some(count) = self
            .application
            .substitution_scope_starts
            .partition_point(|start| *start <= ordinal)
            .checked_sub(1)
        else {
            return Some(ReferenceStep::Refuse);
        };
        self = self.prefix(count);
        Some(ReferenceStep::Chase(self, argument))
    }
}
