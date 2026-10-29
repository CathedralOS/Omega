//! Finish conditional checking across authored scopes without changing code.
//!
//! The compile-time producer supplies normally completed canonical observations.
//! This owner discharges retained conditions and accounts for their scope coverage.
//! It does not certify arbitrary host scalars as interpreter executions: the
//! provisional recording API is a trusted compiler-producer contract, not an
//! artifact-ingress or guest-accessible operation.

use super::{ScalarApplicationObservations, ScalarApplicationValues, ScalarConditionSelection};
use crate::{AdmittedProviderSelections, AdmittedTargetLayouts, AdmittedTargetMachineSelections};
use numerics::bignum::BigInt;
use semantic_vocabulary::IntegerType;
use terminal_codec::TerminalArtifactView;
use terminal_psi::{
    ScalarCompletionCondition, ScalarConditionSubject, ScalarRangeCondition, TerminalPsiIdentity,
};

/// Exact target-owned model supplied by the application from its selected
/// profile. Bounds are derived from this carrier, never from the host pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarAddressModel {
    pub target: String,
    pub carrier: IntegerType,
    pub semantics_version: u32,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarScopeInputs {
    targets: AdmittedTargetMachineSelections,
    providers: AdmittedProviderSelections,
    layouts: AdmittedTargetLayouts,
    address_model: Option<ScalarAddressModel>,
}

impl ScalarScopeInputs {
    pub fn targets(&self) -> &AdmittedTargetMachineSelections {
        &self.targets
    }
    pub fn providers(&self) -> &AdmittedProviderSelections {
        &self.providers
    }
    pub fn layouts(&self) -> &AdmittedTargetLayouts {
        &self.layouts
    }
    pub fn address_model(&self) -> Option<&ScalarAddressModel> {
        self.address_model.as_ref()
    }
}

/// Completed conditional judgment and primary values for one exact deployment.
/// Supplemental scopes retain their dependencies, never replace primary values.
/// No public constructor or codec can promote an unfinished provisional ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedScalarApplications {
    terminal: TerminalPsiIdentity,
    primary: CompletedScalarScope,
    supplemental: Vec<ScalarScopeInputs>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletedScalarScope {
    inputs: ScalarScopeInputs,
    values: ScalarApplicationValues,
    geometry: std::sync::Arc<[terminal_psi::StructuralTypeDeclaration]>,
}

impl AdmittedScalarApplications {
    pub fn terminal(&self) -> TerminalPsiIdentity {
        self.terminal
    }
    pub fn primary(&self) -> &ScalarScopeInputs {
        &self.primary.inputs
    }
    pub fn supplemental(&self) -> &[ScalarScopeInputs] {
        &self.supplemental
    }
    pub fn values(&self) -> &ScalarApplicationValues {
        &self.primary.values
    }
    /// Selected record placements and evaluated extents only. Canonical code
    /// and declarations remain in their original immutable allocation.
    pub fn geometry(&self) -> &std::sync::Arc<[terminal_psi::StructuralTypeDeclaration]> {
        &self.primary.geometry
    }
}

/// One writer for the complete authored-condition roster. Successful individual
/// evaluations or individual predicates do not constitute this completion.
pub struct ScalarApplicationClosure<'artifact> {
    artifact: TerminalArtifactView<'artifact>,
    primary: String,
    scopes: Vec<(String, bool)>,
    ranges: Vec<bool>,
    predicates: Vec<bool>,
    completions: Vec<bool>,
    primary_result: Option<CompletedScalarScope>,
    supplemental: Vec<ScalarScopeInputs>,
}

impl<'artifact> ScalarApplicationClosure<'artifact> {
    pub fn new(artifact: TerminalArtifactView<'artifact>, primary: &str) -> Self {
        let catalog = &artifact.module().scalar_applications;
        let applications = catalog.proof_scope.applications();
        let mut owners = Vec::new();
        let mut unscoped = false;
        let subjects = catalog
            .conditions
            .ranges()
            .iter()
            .filter_map(|condition| match condition {
                ScalarRangeCondition::Range { subject, .. } => Some(*subject),
                _ => None,
            })
            .chain(
                catalog
                    .conditions
                    .predicates()
                    .iter()
                    .map(|condition| condition.subject),
            )
            .chain(catalog.conditions.completions().iter().filter_map(
                |condition| match condition {
                    ScalarCompletionCondition::Complete { subject, .. } => Some(*subject),
                    ScalarCompletionCondition::Missing => None,
                },
            ));
        for subject in subjects {
            match subject {
                ScalarConditionSubject::Module => unscoped = true,
                ScalarConditionSubject::Declaration(owner) if !owners.contains(&owner) => {
                    owners.push(owner)
                }
                _ => {}
            }
        }
        let scopes = applications
            .required_scopes(primary, &owners, unscoped)
            .into_iter()
            .map(|scope| (scope.to_owned(), false))
            .collect();
        Self {
            artifact,
            primary: primary.to_owned(),
            scopes,
            ranges: vec![false; catalog.conditions.ranges().len()],
            predicates: vec![false; catalog.conditions.predicates().len()],
            completions: vec![false; catalog.conditions.completions().len()],
            primary_result: None,
            supplemental: Vec::new(),
        }
    }

    pub fn scopes(&self) -> impl Iterator<Item = &str> {
        self.scopes.iter().map(|(scope, _)| scope.as_str())
    }

    /// Consume the exact evaluated selection. Failure publishes neither coverage
    /// nor values, and predicate diagnostics precede address-model/range failures.
    pub fn close_scope(
        &mut self,
        observations: ScalarApplicationObservations<'artifact>,
        conditions: ScalarConditionSelection<'artifact>,
        address_model: Result<Option<ScalarAddressModel>, String>,
    ) -> Result<(), String> {
        if observations.artifact().identity() != self.artifact.identity()
            || !conditions.matches(&observations, &self.primary)
        {
            return Err(
                "scalar closure selection differs from its artifact or scope inputs".into(),
            );
        }
        let target = observations.targets().target();
        let scope = self
            .scopes
            .iter()
            .position(|(scope, _)| scope == target)
            .ok_or("scalar closure received an unrequested authored scope")?;
        if self.scopes[scope].1 {
            return Err("scalar closure scope completed more than once".into());
        }
        let catalog = &self.artifact.module().scalar_applications.conditions;
        for (position, condition) in catalog.completions().iter().enumerate() {
            if let ScalarCompletionCondition::Complete {
                subject: ScalarConditionSubject::Application(application),
                ..
            } = condition
                && observations.value(*application)?.is_some()
                && !conditions.completions().contains(&position)
            {
                return Err("completed scalar application has unselected completion debt".into());
            }
        }
        for &position in conditions.completions() {
            let ScalarCompletionCondition::Complete { application, .. } =
                catalog.completions()[position]
            else {
                return Err("scalar closure has an absent completion condition".into());
            };
            if observations.value(application)?.is_none() {
                return Err(format!(
                    "scalar application {application} has not completed normally"
                ));
            }
        }
        for (position, condition) in catalog.ranges().iter().enumerate() {
            if let ScalarRangeCondition::Range {
                subject: ScalarConditionSubject::Application(application),
                ..
            } = condition
                && observations.value(*application)?.is_some()
                && !conditions.ranges().contains(&position)
            {
                return Err("completed scalar application has unselected range debt".into());
            }
        }
        for (position, condition) in catalog.predicates().iter().enumerate() {
            if let ScalarConditionSubject::Application(application) = condition.subject
                && observations.value(application)?.is_some()
                && !conditions.predicates().contains(&position)
            {
                return Err("completed scalar application has unselected predicate debt".into());
            }
        }
        for &position in conditions.predicates() {
            observations.discharge_predicate(position)?;
        }
        let address_model = address_model?;
        let bounds = if let Some(model) = &address_model {
            if model.target != target || !model.carrier.is_address() || model.semantics_version == 0
            {
                return Err(
                    "scalar closure address model differs from its selected target or carrier"
                        .into(),
                );
            }
            Some((
                BigInt::zero(),
                BigInt::from_u64(1)
                    .shl_bits(usize::from(model.carrier.bits()))
                    .sub(&BigInt::from_u64(1)),
            ))
        } else {
            None
        };
        for &position in conditions.ranges() {
            observations.discharge_range(
                position,
                bounds.as_ref().map(|(minimum, maximum)| (minimum, maximum)),
            )?;
        }
        let mut geometry = observations.layouts().resolved_records().to_vec();
        for declaration in &observations.artifact().module().structural_types {
            if let terminal_psi::StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Application(application),
            } = declaration.shape
                && let Some(length) = observations.extent(application)?
            {
                let mut resolved = declaration.clone();
                resolved.shape = terminal_psi::StructuralTypeShape::FixedArray {
                    element,
                    length: length.into(),
                };
                geometry.push(resolved);
            }
        }
        geometry.sort_unstable_by_key(|declaration| declaration.id);
        for &position in conditions.predicates() {
            self.predicates[position] = true;
        }
        for &position in conditions.completions() {
            self.completions[position] = true;
        }
        for &position in conditions.ranges() {
            self.ranges[position] = true;
        }
        let inputs = ScalarScopeInputs {
            targets: observations.targets().clone(),
            providers: observations.providers().clone(),
            layouts: observations.layouts().clone(),
            address_model,
        };
        if target == self.primary {
            self.primary_result = Some(CompletedScalarScope {
                inputs,
                values: observations.values(),
                geometry: geometry.into(),
            });
        } else {
            self.supplemental.push(inputs);
        }
        self.scopes[scope].1 = true;
        Ok(())
    }

    pub fn finish(self) -> Result<AdmittedScalarApplications, String> {
        if self.scopes.iter().any(|(_, complete)| !complete) {
            return Err("scalar closure has an unfinished authored scope".into());
        }
        let conditions = &self.artifact.module().scalar_applications.conditions;
        for (position, condition) in conditions.completions().iter().enumerate() {
            let ScalarCompletionCondition::Complete { subject, .. } = condition else {
                return Err("scalar closure has an absent completion condition".into());
            };
            if !matches!(subject, ScalarConditionSubject::Application(_))
                && !self.completions[position]
            {
                return Err("retained scalar completion has no target application".into());
            }
        }
        // Generated entry debt exists only when that job is demanded. Every
        // authored or module obligation still needs an actual applicable scope.
        for (position, condition) in conditions.ranges().iter().enumerate() {
            let ScalarRangeCondition::Range { subject, .. } = condition else {
                return Err("scalar closure has an absent range condition".into());
            };
            if !matches!(subject, ScalarConditionSubject::Application(_)) && !self.ranges[position]
            {
                return Err("retained scalar range has no target application".into());
            }
        }
        for (position, condition) in conditions.predicates().iter().enumerate() {
            if !matches!(condition.subject, ScalarConditionSubject::Application(_))
                && !self.predicates[position]
            {
                return Err("retained scalar predicate has no target application".into());
            }
        }
        let primary = self
            .primary_result
            .ok_or("scalar closure has no primary value scope")?;
        Ok(AdmittedScalarApplications {
            terminal: self.artifact.identity(),
            primary,
            supplemental: self.supplemental,
        })
    }
}
