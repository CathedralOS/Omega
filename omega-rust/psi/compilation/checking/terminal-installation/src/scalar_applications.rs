//! Target-bound observations used while closing portable scalar applications.
//!
//! These are provisional execution results, not a license to run a conditional
//! program. The compile-time owner supplies successful complete-entry results;
//! this owner checks their catalog coordinates and carriers, computes geometry
//! through the shared storage semantics, and discharges captured conditions.
//! Applicability and complete coverage still belong to the application closure.
//! In particular, possessing this ledger cannot bypass an executor's pending
//! condition check or turn an unvisited operation into a proved observation.

use arena::Arena;
use numerics::bignum::BigInt;
use numerics::scalar_predicates::ScalarValue;
use semantic_vocabulary::{IntegerValue, MachineId, ScalarApplicationId, ScalarType, ValueId};
use terminal_codec::TerminalArtifactView;
use terminal_psi::{
    LayoutMeasurement, ScalarApplication, ScalarRangeObservation, ScalarRangeRequirement,
};
use terminal_semantics::{ReferenceStorageGeometry, StorageGeometryError};

use crate::{
    AdmittedProviderSelections, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
    TerminalScalarValue,
};

mod conditions;
pub use conditions::ScalarConditionSelection;
mod closure;
pub use closure::{
    AdmittedScalarApplications, ScalarAddressModel, ScalarApplicationClosure, ScalarScopeInputs,
};

#[derive(Debug, Clone, Default)]
struct ApplicationObservations {
    value: Option<TerminalScalarValue>,
    results: Arena<EntryResult>,
}

#[derive(Debug, Clone, Default)]
enum EntryResult {
    #[default]
    Missing,
    Observed {
        result: ValueId,
        value: TerminalScalarValue,
    },
}

/// Invocation-owned scratch, borrowing the exact canonical input. Results from
/// another artifact, target or provider selection never share this namespace.
pub struct ScalarApplicationObservations<'artifact> {
    artifact: TerminalArtifactView<'artifact>,
    targets: &'artifact AdmittedTargetMachineSelections,
    providers: &'artifact AdmittedProviderSelections,
    layouts: AdmittedTargetLayouts,
    applications: std::sync::Arc<Arena<ApplicationObservations>>,
}

/// Immutable provisional inputs to one evaluation. Cloning shares the retained
/// rows; recording the next completed job cannot mutate a suspended invocation.
#[derive(Clone)]
pub struct ScalarApplicationValues(std::sync::Arc<Arena<ApplicationObservations>>);

impl ScalarApplicationValues {
    pub fn value(&self, application: ScalarApplicationId) -> Option<TerminalScalarValue> {
        let position = usize::try_from(application.get() - 1).ok()?;
        self.0.storage_slice().get(position)?.value
    }
}

impl<'artifact> ScalarApplicationObservations<'artifact> {
    pub fn new(
        artifact: TerminalArtifactView<'artifact>,
        targets: &'artifact AdmittedTargetMachineSelections,
        providers: &'artifact AdmittedProviderSelections,
    ) -> Result<Self, String> {
        if artifact.identity() != targets.terminal_psi()
            || artifact.identity() != providers.terminal_psi()
        {
            return Err("scalar observations name a different Terminal artifact".into());
        }
        if let Some(target) = artifact.module().folded_constants.target.as_deref()
            && (targets.target() != target || providers.folded_constant_target() != Some(target))
        {
            return Err(
                "scalar observations differ from the artifact's folded target closure".into(),
            );
        }
        let layouts =
            crate::admit_target_layouts(artifact, targets).map_err(|error| error.to_string())?;
        let mut applications = Arena::new();
        applications.insert_many(
            artifact
                .module()
                .scalar_applications
                .declarations()
                .iter()
                .map(|_| ApplicationObservations::default()),
        );
        Ok(Self {
            artifact,
            targets,
            providers,
            layouts,
            applications: std::sync::Arc::new(applications),
        })
    }

    pub fn artifact(&self) -> TerminalArtifactView<'artifact> {
        self.artifact
    }
    pub fn targets(&self) -> &AdmittedTargetMachineSelections {
        self.targets
    }
    pub fn providers(&self) -> &AdmittedProviderSelections {
        self.providers
    }
    pub fn layouts(&self) -> &AdmittedTargetLayouts {
        &self.layouts
    }

    pub fn values(&self) -> ScalarApplicationValues {
        ScalarApplicationValues(std::sync::Arc::clone(&self.applications))
    }

    /// Select this target's debt and the explicitly demanded evaluation jobs.
    /// The primary value scope additionally owes ground declaration formation.
    pub fn conditions(
        &self,
        value_scope: &str,
        demanded: &[ScalarApplicationId],
    ) -> Result<ScalarConditionSelection<'artifact>, String> {
        let catalog = &self.artifact.module().scalar_applications;
        ScalarConditionSelection::select(
            &catalog.proof_scope,
            &catalog.conditions,
            self.targets.target(),
            self.targets.target() == value_scope,
            self.providers.providers().chain(
                self.providers
                    .assumed_bindings()
                    .iter()
                    .map(|binding| (binding.required, binding.implementation)),
            ),
            demanded,
        )
    }

    fn position(&self, application: ScalarApplicationId) -> Result<usize, String> {
        let position = usize::try_from(application.get() - 1)
            .map_err(|_| "scalar application coordinate exceeds the host range")?;
        if position >= self.applications.len() {
            return Err(format!("unknown scalar application {}", application.get()));
        }
        Ok(position)
    }

    pub fn value(
        &self,
        application: ScalarApplicationId,
    ) -> Result<Option<TerminalScalarValue>, String> {
        Ok(self.applications.storage_slice()[self.position(application)?].value)
    }

    /// Only the catalog's complete, zero-argument scalar entry can answer this
    /// job. Selected provider execution remains the ordinary interpreter's job.
    pub fn entry(&self, application: ScalarApplicationId) -> Result<MachineId, String> {
        let Some(ScalarApplication::Evaluate { entry, scalar_type }) = self
            .artifact
            .module()
            .scalar_applications
            .declaration(application)
        else {
            return Err("scalar application is not a complete evaluation entry".into());
        };
        let machine = self
            .targets
            .entry(self.artifact.identity(), self.artifact.module(), *entry)
            .map_err(|error| error.to_string())?;
        let declaration = self
            .artifact
            .module()
            .machine(machine)
            .ok_or("scalar evaluation entry is absent")?;
        if !declaration.parameters.is_empty()
            || !declaration.structural_parameters.is_empty()
            || !matches!(&declaration.result, terminal_psi::TerminalMachineResult::Scalar(result) if result.scalar_type == *scalar_type)
        {
            return Err(
                "scalar evaluation entry has runtime inputs or a different result carrier".into(),
            );
        }
        Ok(machine)
    }

    /// Record the normal return of an executed complete entry, never a literal
    /// supplied in place of executing its body. This is still provisional.
    pub fn record_value(
        &mut self,
        application: ScalarApplicationId,
        value: TerminalScalarValue,
    ) -> Result<(), String> {
        self.entry(application)?;
        self.store_value(application, value)
    }

    fn store_value(
        &mut self,
        application: ScalarApplicationId,
        value: TerminalScalarValue,
    ) -> Result<(), String> {
        let position = self.position(application)?;
        let expected = self
            .artifact
            .module()
            .scalar_applications
            .declaration(application)
            .and_then(|row| row.scalar_type())
            .ok_or("scalar application has no carrier")?;
        validate_value(value, expected)?;
        let applications = std::sync::Arc::make_mut(&mut self.applications);
        let handle = applications
            .handle_of(&applications.storage_slice()[position])
            .ok_or("scalar application storage lost its row")?;
        let row = applications.get_mut(handle);
        if row.value.is_some() {
            return Err("scalar application completed more than once".into());
        }
        row.value = Some(value);
        Ok(())
    }

    /// Captures must come from the application's successful root activation.
    /// The interpreter owns that occurrence rule; this join refuses substitute
    /// machines, unrelated operation IDs, carrier changes and repeated records.
    pub fn record_entry_result(
        &mut self,
        application: ScalarApplicationId,
        machine: MachineId,
        result: ValueId,
        value: TerminalScalarValue,
    ) -> Result<(), String> {
        if machine != self.entry(application)? {
            return Err("scalar observation names another entry activation".into());
        }
        let expected = self
            .artifact
            .module()
            .scalar_applications
            .conditions
            .observations()
            .find_map(|row| match row {
                ScalarRangeObservation::EntryResult {
                    application: owner,
                    machine: entry,
                    result: observed,
                    scalar_type,
                } if *owner == application && *entry == machine && *observed == result => {
                    Some(*scalar_type)
                }
                _ => None,
            })
            .ok_or("scalar observation answers no retained entry result")?;
        validate_value(value, expected)?;
        let position = self.position(application)?;
        let applications = std::sync::Arc::make_mut(&mut self.applications);
        let handle = applications
            .handle_of(&applications.storage_slice()[position])
            .ok_or("scalar application storage lost its row")?;
        let row = applications.get_mut(handle);
        if row.results.storage_slice().iter().any(|row| matches!(row, EntryResult::Observed { result: observed, .. } if *observed == result)) {
            return Err("scalar entry result was recorded more than once".into());
        }
        row.results.insert(EntryResult::Observed { result, value });
        Ok(())
    }

    /// Read an exact capture after its complete entry returned normally. A
    /// completed job without this capture cannot be replayed to fill it later.
    pub fn entry_result(
        &self,
        application: ScalarApplicationId,
        machine: MachineId,
        result: ValueId,
        scalar_type: ScalarType,
    ) -> Result<TerminalScalarValue, String> {
        if self.entry(application)? != machine {
            return Err("scalar condition names another entry activation".into());
        }
        let row = &self.applications.storage_slice()[self.position(application)?];
        if row.value.is_none() {
            return Err("scalar entry has not completed normally".into());
        }
        let value = row
            .results
            .storage_slice()
            .iter()
            .find_map(|row| match row {
                EntryResult::Observed {
                    result: observed,
                    value,
                } if *observed == result => Some(*value),
                _ => None,
            })
            .ok_or("scalar condition has an unobserved entry result")?;
        validate_value(value, scalar_type)?;
        Ok(value)
    }

    /// Geometry demand follows the same selected authored declarations and
    /// recursive shape rules as concrete measurement, without evaluating jobs.
    pub fn layout_dependencies(
        &self,
        application: ScalarApplicationId,
    ) -> Result<Vec<ScalarApplicationId>, String> {
        let Some(ScalarApplication::LayoutObservation { subject, .. }) = self
            .artifact
            .module()
            .scalar_applications
            .declaration(application)
        else {
            return Err("application is not a layout observation".into());
        };
        let geometry = self
            .layouts
            .geometry(self.artifact)
            .map_err(|error| error.to_string())?;
        terminal_semantics::reference_storage_dependencies(geometry.declarations(), *subject)
            .map_err(|error| error.to_string())
    }

    /// Resolve through the ordinary byte-geometry owner. An outstanding array
    /// extent returns its exact dependency before any layout result is stored.
    pub fn observe_layout(
        &mut self,
        application: ScalarApplicationId,
    ) -> Result<TerminalScalarValue, ScalarLayoutObservationError> {
        let Some(ScalarApplication::LayoutObservation {
            subject,
            measurement,
            semantics_version,
            scalar_type,
        }) = self
            .artifact
            .module()
            .scalar_applications
            .declaration(application)
            .copied()
        else {
            return Err(ScalarLayoutObservationError::Invalid(
                "application is not a layout observation".into(),
            ));
        };
        if semantics_version != LayoutMeasurement::SEMANTICS_VERSION {
            return Err(ScalarLayoutObservationError::Invalid(
                "unsupported scalar layout semantics".into(),
            ));
        }
        let geometry = self
            .layouts
            .geometry(self.artifact)
            .map_err(|error| ScalarLayoutObservationError::Invalid(error.to_string()))?;
        let mut input_error = None;
        let mut geometry = ReferenceStorageGeometry::new(geometry.declarations(), |dependency| {
            match self.extent(dependency) {
                Ok(Some(length)) => Ok(length),
                Ok(None) => Err(StorageGeometryError::UnresolvedExtent(dependency)),
                Err(reason) => {
                    input_error = Some(reason);
                    Err(StorageGeometryError::UnresolvedExtent(dependency))
                }
            }
        });
        let layout = geometry.layout(subject);
        drop(geometry);
        if let Some(reason) = input_error {
            return Err(ScalarLayoutObservationError::Invalid(reason));
        }
        let (size, alignment) = layout.map_err(ScalarLayoutObservationError::Geometry)?;
        let ScalarType::Integer(integer_type) = scalar_type else {
            return Err(ScalarLayoutObservationError::Invalid(
                "layout observation has a non-integer carrier".into(),
            ));
        };
        let value = TerminalScalarValue::Integer {
            scalar_type: integer_type,
            value: IntegerValue::Unsigned(u128::from(match measurement {
                LayoutMeasurement::Size => size,
                LayoutMeasurement::Alignment => alignment,
            })),
        };
        self.store_value(application, value)
            .map_err(ScalarLayoutObservationError::Invalid)?;
        Ok(value)
    }

    pub fn extent(&self, application: ScalarApplicationId) -> Result<Option<u64>, String> {
        match self.value(application)? {
            None => Ok(None),
            Some(TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(value),
                ..
            }) => u64::try_from(value)
                .map(Some)
                .map_err(|_| "array extent exceeds u64".into()),
            Some(TerminalScalarValue::Integer {
                value: IntegerValue::Signed(value),
                ..
            }) => u64::try_from(value)
                .map(Some)
                .map_err(|_| "array extent is negative or exceeds u64".into()),
            Some(_) => Err("array extent requires an integer result".into()),
        }
    }

    fn integer(&self, observation: &ScalarRangeObservation) -> Result<BigInt, String> {
        let value = match observation {
            ScalarRangeObservation::Literal(value) => return Ok(value.clone()),
            ScalarRangeObservation::Application(application) => self
                .value(*application)?
                .ok_or("scalar condition has an unevaluated application")?,
            ScalarRangeObservation::EntryResult {
                application,
                machine,
                result,
                scalar_type,
            } => self.entry_result(*application, *machine, *result, *scalar_type)?,
            ScalarRangeObservation::Missing => {
                return Err("scalar condition has an absent observation".into());
            }
        };
        match value {
            TerminalScalarValue::Integer {
                value: IntegerValue::Signed(value),
                ..
            } => Ok(BigInt::from_i128(value)),
            TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(value),
                ..
            } => Ok(BigInt::from_u128(value)),
            _ => Err("scalar condition requires an integer observation".into()),
        }
    }

    /// Check one applicable retained range. Selecting the condition is separate
    /// from proving it; callers must account for every applicable row at closure.
    pub fn discharge_range(
        &self,
        position: usize,
        address_model: Option<(&BigInt, &BigInt)>,
    ) -> Result<(), String> {
        let conditions = &self.artifact.module().scalar_applications.conditions;
        let condition = conditions
            .ranges()
            .get(position)
            .ok_or("scalar range condition is absent")?;
        let terminal_psi::ScalarRangeCondition::Range { requirement, .. } = condition else {
            return Err("scalar range condition is absent".into());
        };
        let (minimum, maximum) = match requirement {
            ScalarRangeRequirement::Fixed { minimum, maximum } => (minimum, maximum),
            ScalarRangeRequirement::AddressModel => {
                address_model.ok_or("scalar range condition needs an admitted address model")?
            }
        };
        if minimum > maximum {
            return Err("scalar range condition has inverted required bounds".into());
        }
        let (low, high) =
            numerics::integer_bounds::evaluate(conditions.steps(condition)?, &mut |observation| {
                self.integer(observation)
            })?;
        if low < *minimum || high > *maximum {
            return Err(format!(
                "scalar range condition {position} exceeds its required bounds"
            ));
        }
        Ok(())
    }

    pub fn discharge_predicate(&self, position: usize) -> Result<(), String> {
        let condition = self
            .artifact
            .module()
            .scalar_applications
            .conditions
            .predicates()
            .get(position)
            .ok_or("scalar predicate condition is absent")?;
        let mut missing = None;
        let result = condition.evaluate(&mut |observation| match self.integer(&observation) {
            Ok(value) => Some(ScalarValue::Integer(value)),
            Err(reason) => {
                missing = Some(reason);
                None
            }
        });
        if let Some(reason) = missing {
            return Err(reason);
        }
        match result {
            Some(ScalarValue::Boolean(true)) => Ok(()),
            Some(ScalarValue::Boolean(false)) => Err(format!(
                "scalar predicate is false: {}",
                condition.description
            )),
            _ => Err(format!(
                "scalar predicate could not be established: {}",
                condition.description
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarLayoutObservationError {
    Geometry(StorageGeometryError),
    Invalid(String),
}

impl std::fmt::Display for ScalarLayoutObservationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Geometry(error) => error.fmt(formatter),
            Self::Invalid(reason) => formatter.write_str(reason),
        }
    }
}
impl std::error::Error for ScalarLayoutObservationError {}

fn validate_value(value: TerminalScalarValue, expected: ScalarType) -> Result<(), String> {
    if value.scalar_type() != expected {
        return Err("scalar observation has a different carrier".into());
    }
    if let TerminalScalarValue::Integer { scalar_type, value } = value
        && !scalar_type.admits(value)
    {
        return Err("scalar observation does not fit its integer carrier".into());
    }
    Ok(())
}
