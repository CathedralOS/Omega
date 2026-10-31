//! Region identity refines the complete type, not just its physical loans.
//! Reuse conformance's structural query for phantom nominal arguments and
//! reference descendants. Simultaneous operands re-enter their supplying
//! scope; source names never equate a captured region with a fresh binder.

use super::callable_refinement::{CallableBorrowContract, captured_lifetimes};
use super::callable_regions::CallableRegions;
use super::carried_lifetimes::{CarriedLifetime, RegionIdentity};
use super::region_inference::InputRegionApplication;
use crate::type_regions::{
    ReferenceStep, TypeRegionRelationship, TypeRegionScope, type_reference_region_applications,
    type_reference_regions_match,
};
use symbols::SymbolHandle;
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Copy)]
enum RegionEnvironment<'scope> {
    Resolved(&'scope [(String, RegionIdentity)]),
    Captured(SymbolHandle),
}

impl RegionEnvironment<'_> {
    fn identity(
        self,
        program: &dyn crate::CheckingInput,
        name: &Identifier,
    ) -> Option<RegionIdentity> {
        match self {
            Self::Resolved(regions) => regions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == name.as_str())
                .map(|(_, identity)| identity.clone()),
            Self::Captured(owner) => captured_lifetimes(program, owner)
                .into_iter()
                .rev()
                .find(|(parameter, _)| parameter == name.as_str())
                .map(|(_, identity)| identity),
        }
    }
}

#[derive(Clone, Copy)]
struct RegionScope<'scope> {
    declaration: SymbolHandle,
    substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
    starts: &'scope [usize],
    applications: &'scope [Vec<(String, RegionIdentity)>],
    binders: &'scope [(SymbolHandle, String)],
    regions: RegionEnvironment<'scope>,
    arguments: RegionEnvironment<'scope>,
    output_elisions: &'scope [(TypeReferenceHandle, RegionIdentity)],
}

impl TypeRegionScope for RegionScope<'_> {
    fn erased_identity(
        self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> typed_trees::type_identity::NormalizedTypeIdentity {
        program.scoped_type_identity(
            typed_trees::type_identity::TypeIdentityRequest {
                binders: self.binders,
                substitutions: self.substitutions,
                ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
            },
            self.starts,
        )
    }

    fn enter_operand(
        mut self,
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
    ) -> Self {
        self.regions = RegionEnvironment::Captured(owner);
        self.output_elisions = &[];
        if program.lifetime_declaration_owner(owner)
            != program.lifetime_declaration_owner(self.declaration)
        {
            self.substitutions = &[];
            self.starts = &[];
            self.applications = &[];
        }
        self
    }

    fn named_alias(
        mut self,
        _program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
        reference: TypeReferenceHandle,
    ) -> Option<ReferenceStep<Self>> {
        let substitutions = self.substitutions;
        let starts = self.starts;
        let applications = self.applications;
        let (binding_ordinal, operand) = substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (parameter, _))| *parameter == symbol)
            .map(|(ordinal, (_, operand))| (ordinal, *operand))?;
        if operand == reference {
            return Some(ReferenceStep::Stop);
        }
        let Some((scope_ordinal, start)) = starts
            .iter()
            .enumerate()
            .rev()
            .find(|(_, start)| **start <= binding_ordinal)
        else {
            return Some(ReferenceStep::Refuse);
        };
        self.regions = scope_ordinal
            .checked_sub(1)
            .and_then(|ordinal| applications.get(ordinal))
            .map_or(self.arguments, |regions| {
                RegionEnvironment::Resolved(regions)
            });
        self.substitutions = &substitutions[..*start];
        self.starts = &starts[..scope_ordinal];
        self.applications = &applications[..applications.len().min(scope_ordinal)];
        self.output_elisions = &[];
        Some(ReferenceStep::Chase(self, operand))
    }
}

fn contract_scope<'scope>(
    contract: &'scope CallableBorrowContract<'_>,
    regions: &'scope CallableRegions,
) -> RegionScope<'scope> {
    RegionScope {
        declaration: contract.lexical_owner,
        substitutions: contract.substitutions,
        starts: contract.substitution_scope_starts,
        applications: &regions.applications,
        binders: contract.binders,
        regions: RegionEnvironment::Resolved(&regions.names),
        arguments: RegionEnvironment::Resolved(&regions.arguments),
        output_elisions: &[],
    }
}

pub(super) fn infer_inputs(
    program: &dyn crate::CheckingInput,
    required: &CallableBorrowContract<'_>,
    actual: &CallableBorrowContract<'_>,
    required_regions: &CallableRegions,
    actual_regions: &CallableRegions,
    actual_inputs: &[Vec<CarriedLifetime>],
) -> Result<InputRegionApplication, &'static str> {
    if required.parameters.len() != actual.parameters.len() {
        return Err("callable input arity does not match its requirement");
    }
    let mut applications = vec![InputRegionApplication::default()];
    for (ordinal, (required_parameter, actual_parameter)) in required
        .parameters
        .iter()
        .zip(actual.parameters)
        .enumerate()
    {
        let mut failure = "callable input does not preserve its region identity";
        let mut constrain = |application: &mut InputRegionApplication,
                             actual_scope: RegionScope<'_>,
                             required_scope: RegionScope<'_>,
                             reference,
                             actual: Option<&Identifier>,
                             required: Option<&Identifier>| {
            let Some(required) = required else {
                return true;
            };
            let Some(required) = required_scope.regions.identity(program, required) else {
                return false;
            };
            let actual = match actual {
                Some(name) => {
                    let Some(region) = actual_scope.regions.identity(program, name) else {
                        return false;
                    };
                    Some(region)
                }
                None => {
                    // Nonphysical operands cannot supply an input region. Keep
                    // elision on this parameter's enumerated physical frontier.
                    if !actual_inputs.get(ordinal).is_some_and(|leaves| {
                        leaves
                            .iter()
                            .any(|leaf| leaf.reference == reference && leaf.lifetime.is_none())
                    }) {
                        return false;
                    }
                    None
                }
            };
            let mut instantiated = actual.clone();
            if let Some(region) = &mut instantiated {
                application.apply(region);
            }
            if application.constrain(ordinal, reference, actual.clone(), required.clone()) {
                return true;
            }
            // Preserve the semantic axis of the failed application. A rigid
            // captured input cannot supply a universally fresh invocation;
            // conflicting inferred binders cannot merge captured storage with
            // that invocation either. Nonphysical operands carry identity but
            // never acquire a physical loan diagnostic.
            failure = if matches!(actual, Some(RegionIdentity::Captured(_, _)))
                && matches!(required, RegionIdentity::Fresh(_))
            {
                "fresh invocation lifetime telescope is not universally refined"
            } else if actual_inputs
                .get(ordinal)
                .is_some_and(|leaves| leaves.iter().any(|leaf| leaf.reference == reference))
                && (matches!(instantiated, Some(RegionIdentity::Captured(_, _)))
                    || matches!(required, RegionIdentity::Captured(_, _)))
            {
                "callable input does not preserve its captured region identity"
            } else {
                "callable type does not preserve its region identity"
            };
            false
        };
        if !type_reference_region_applications(
            program,
            actual_parameter.type_reference,
            required_parameter.type_reference,
            contract_scope(actual, actual_regions),
            contract_scope(required, required_regions),
            TypeRegionRelationship::CallableRefinement,
            &mut applications,
            &mut constrain,
        ) {
            return Err(failure);
        }
    }
    // Only the complete input tuple and universal telescope settle inference.
    // A domain alternative may look ambiguous before a later input fixes a
    // binder. Results consume this application; they never choose among it.
    let mut completed = Vec::new();
    let mut failure = "callable input regions do not determine one application";
    for mut application in applications {
        match application.finish(actual_regions.fresh_count, required_regions.fresh_count) {
            Ok(()) if !completed.contains(&application) => completed.push(application),
            Ok(()) => {}
            Err(reason) => failure = reason,
        }
    }
    if completed.len() != 1 {
        return Err(if completed.is_empty() {
            failure
        } else {
            "callable input regions do not determine one application"
        });
    }
    completed.pop().ok_or(failure)
}

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    required: &CallableBorrowContract<'_>,
    actual: &CallableBorrowContract<'_>,
    required_regions: &CallableRegions,
    actual_regions: &CallableRegions,
    input_elisions: &[(usize, TypeReferenceHandle, RegionIdentity)],
    output_elisions: &[(TypeReferenceHandle, RegionIdentity)],
) -> Result<(), &'static str> {
    let required_scope = contract_scope(required, required_regions);
    let actual_scope = contract_scope(actual, actual_regions);
    let mut matches = |_: &mut (),
                       actual_scope: RegionScope<'_>,
                       required_scope: RegionScope<'_>,
                       reference: TypeReferenceHandle,
                       actual: Option<&Identifier>,
                       required: Option<&Identifier>| {
        let Some(required) = required else {
            return true;
        };
        required_scope
            .regions
            .identity(program, required)
            .zip(match actual {
                Some(actual) => actual_scope.regions.identity(program, actual),
                None => actual_scope
                    .output_elisions
                    .iter()
                    .find(|(output, _)| *output == reference)
                    .map(|(_, region)| region.clone()),
            })
            .is_some_and(|(required, actual)| required == actual)
    };
    for (ordinal, (required, actual)) in required
        .parameters
        .iter()
        .map(|parameter| parameter.type_reference)
        .zip(
            actual
                .parameters
                .iter()
                .map(|parameter| parameter.type_reference),
        )
        .enumerate()
    {
        let elisions = input_elisions
            .iter()
            .filter(|(parameter, _, _)| *parameter == ordinal)
            .map(|(_, reference, region)| (*reference, region.clone()))
            .collect::<Vec<_>>();
        if !type_reference_regions_match(
            program,
            actual,
            required,
            RegionScope {
                output_elisions: &elisions,
                ..actual_scope
            },
            required_scope,
            &mut (),
            &mut matches,
        ) {
            return Err("callable type does not preserve its region identity");
        }
    }
    if !type_reference_regions_match(
        program,
        actual.return_type,
        required.return_type,
        RegionScope {
            output_elisions,
            ..actual_scope
        },
        required_scope,
        &mut (),
        &mut matches,
    ) {
        return Err("callable type does not preserve its region identity");
    }
    Ok(())
}
