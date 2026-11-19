//! Record scalar results and check layout, range and predicate observations against their artifact.
use super::{
    AdmittedScalarApplications, ApplicationObservations, CallingAbiStorageDemand, EntryResult,
    ScalarApplicationObservations, ScalarApplicationValues, ScalarConditionScope,
    ScalarConditionSelection,
};
use arena::Arena;
use bignum::BigInt;
use numerics::scalar_predicates::ScalarValue;
use semantic_vocabulary::{IntegerValue, MachineId, ScalarApplicationId, ScalarType, ValueId};
use terminal_psi::{
    LayoutMeasurement, ScalarApplication, ScalarRangeObservation, ScalarRangeRequirement,
};
use terminal_semantics::{ReferenceStorageGeometry, StorageGeometryError};

use psi_provider_selections::AdmittedProviderSelections;
use psi_target_layouts::{AdmittedTargetLayouts, TerminalTargetContext};
use semantic_vocabulary::ScalarAddressModel;
use terminal_semantics::entry_values::TerminalScalarValue;
impl ScalarApplicationValues {
    pub fn value(&self, application: ScalarApplicationId) -> Option<TerminalScalarValue> {
        let position = usize::try_from(application.get() - 1).ok()?;
        self.0.storage_slice().get(position)?.value
    }

    pub fn observed(
        &self,
    ) -> impl Iterator<Item = (ScalarApplicationId, TerminalScalarValue)> + '_ {
        self.0
            .storage_slice()
            .iter()
            .enumerate()
            .filter_map(|(position, row)| {
                let value = row.value?;
                let application =
                    ScalarApplicationId::new(u64::try_from(position).ok()?.checked_add(1)?)?;
                Some((application, value))
            })
    }
}

impl<'artifact> ScalarApplicationObservations<'artifact> {
    pub fn new<'layout>(
        artifact: impl Into<terminal_codec::TerminalExecutionView<'artifact>>,
        targets: impl Into<Option<&'artifact TerminalTargetContext>>,
        providers: &'artifact AdmittedProviderSelections,
        target_layout: impl Into<Option<&'layout psi_target_layouts::LayoutTargetFacts>>,
    ) -> Result<Self, String> {
        let artifact = artifact.into();
        let targets = targets.into();
        let target_layout = target_layout.into();
        if targets.is_some_and(|targets| artifact.identity() != targets.terminal_psi())
            || artifact.custody_identity() != *providers.execution_identity()
        {
            return Err("scalar observations name a different Terminal artifact".into());
        }
        if let Some(target) = artifact.module().folded_constants.target.as_deref()
            && (targets.map(TerminalTargetContext::target) != Some(target)
                || providers.folded_constant_target() != Some(target))
        {
            return Err(
                "scalar observations differ from the artifact's folded target closure".into(),
            );
        }
        let layouts = match (targets, target_layout) {
            (Some(targets), Some(layout)) => Some(
                psi_target_layouts::admit_target_layouts(artifact, targets, layout)
                    .map_err(|error| error.to_string())?,
            ),
            (None, None) => None,
            _ => return Err("scalar layout facts and deployment selection disagree".into()),
        };
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
            layout_target: target_layout.cloned(),
            carrier_layout: None,
            applications: std::sync::Arc::new(applications),
            condition_scope: ScalarConditionScope::Source,
            inherited: None,
            reexecuted: Vec::new(),
        })
    }

    /// Extend a successful Product completion under its exact retained execution
    /// inputs. Root selection may grow; values and already discharged conditions
    /// still belong to the same code, providers and physical target.
    pub fn with_completed_scope(
        mut self,
        completed: &AdmittedScalarApplications,
        address_model: &Result<Option<ScalarAddressModel>, String>,
    ) -> Result<Self, String> {
        completed.restore_scope(&mut self, address_model)?;
        Ok(self)
    }

    pub fn extends_completed_application(&self, application: ScalarApplicationId) -> bool {
        self.position(application)
            .ok()
            .and_then(|position| self.reexecuted.get(position))
            == Some(&false)
            && self
                .inherited
                .as_ref()
                .is_some_and(|scope| scope.values.value(application).is_some())
    }

    /// Start an expanded output request in this new scope. The prior return
    /// value stays in immutable completion storage for comparison, but cannot
    /// authorize the new activation as its own already-completed result.
    pub fn request_additional_entry_results(
        &mut self,
        application: ScalarApplicationId,
        results: &[(ValueId, ScalarType)],
    ) -> Result<(), String> {
        let machine = self.entry(application)?;
        if !self.extends_completed_application(application)
            || self.value(application)?.is_none()
            || !results.iter().any(|(result, scalar)| {
                self.entry_result(application, machine, *result, *scalar)
                    .is_err()
            })
        {
            return Err("scalar output request has no additional local result".into());
        }
        let position = self.position(application)?;
        let applications = std::sync::Arc::make_mut(&mut self.applications);
        let handle = applications
            .handle_of(&applications.storage_slice()[position])
            .ok_or("scalar application storage lost its row")?;
        applications.get_mut(handle).value = None;
        Ok(())
    }

    pub fn require_completed_address_model(
        &self,
        model: Option<&ScalarAddressModel>,
    ) -> Result<(), String> {
        if self
            .inherited
            .as_ref()
            .is_some_and(|scope| scope.inputs.address_model() != model)
        {
            return Err("scalar completion changed its retained address model".into());
        }
        Ok(())
    }

    pub fn artifact(&self) -> terminal_codec::TerminalExecutionView<'artifact> {
        self.artifact
    }

    pub fn with_target_carriers(
        mut self,
        carriers: Option<semantic_vocabulary::TargetCarrierLayout>,
    ) -> Result<Self, String> {
        if self.inherited.is_some() && self.carrier_layout != carriers {
            return Err("scalar completion changed its retained carrier layout".into());
        }
        if carriers.is_some() && self.targets.is_none() {
            return Err("scalar carrier layout requires an explicit deployment selection".into());
        }
        self.carrier_layout = carriers;
        Ok(self)
    }

    pub fn carrier_layout(&self) -> Option<semantic_vocabulary::TargetCarrierLayout> {
        self.carrier_layout
    }
    pub fn targets(&self) -> Option<&TerminalTargetContext> {
        self.targets
    }
    pub fn target(&self) -> Option<&str> {
        self.targets.map(TerminalTargetContext::target)
    }
    pub fn providers(&self) -> &AdmittedProviderSelections {
        self.providers
    }
    pub fn layouts(&self) -> Option<&AdmittedTargetLayouts> {
        self.layouts.as_ref()
    }

    fn geometry(&self) -> Result<psi_target_layouts::TargetLayoutGeometry<'_>, String> {
        match &self.layouts {
            Some(layouts) => layouts
                .geometry(self.artifact)
                .map_err(|error| error.to_string()),
            None => Ok(psi_target_layouts::TargetLayoutGeometry::unscoped(
                self.artifact,
            )),
        }
    }

    pub fn with_application_roots(
        mut self,
        roots: Option<&[terminal_psi::ProofDeclarationId]>,
    ) -> Result<Self, String> {
        let Some(roots) = roots else {
            return Ok(self);
        };
        let scope = &self.artifact.module().scalar_applications.proof_scope;
        let mut declarations = Vec::with_capacity(roots.len());
        for &root in roots {
            if !matches!(
                scope.declaration(root),
                Some(terminal_psi::ProofScopeDeclaration::Callable(_))
            ) {
                return Err(
                    "Build scalar application root has no exact checked proof declaration".into(),
                );
            }
            if !declarations.contains(&root) {
                declarations.push(root);
            }
        }
        for callable in self
            .providers
            .providers()
            .map(|(_, body)| terminal_psi::TargetMachineCallable::Machine(body))
            .chain(self.providers.assumed_bindings().iter().map(|binding| {
                terminal_psi::TargetMachineCallable::Machine(binding.implementation)
            }))
        {
            let owner = scope
                .callable_declaration(callable)
                .ok_or("selected scalar provider has no retained proof declaration")?;
            if !declarations.contains(&owner) {
                declarations.push(owner);
            }
        }
        self.condition_scope = ScalarConditionScope::SourceRoots(declarations);
        Ok(self)
    }

    pub fn with_invocation_roots(
        mut self,
        roots: &[terminal_psi::ProofDeclarationId],
    ) -> Result<Self, String> {
        if self.inherited.is_some() {
            return Err("Product scalar completion cannot become an invocation scope".into());
        }
        if roots.is_empty() {
            return Err("scalar invocation has no checked entry root".into());
        }
        let scope = &self.artifact.module().scalar_applications.proof_scope;
        let mut declarations = Vec::with_capacity(roots.len());
        for &root in roots {
            if !matches!(
                scope.declaration(root),
                Some(terminal_psi::ProofScopeDeclaration::Callable(_))
            ) {
                return Err("scalar invocation root has no exact checked proof declaration".into());
            }
            if !declarations.contains(&root) {
                declarations.push(root);
            }
        }
        self.condition_scope = ScalarConditionScope::Invocation(declarations);
        Ok(self)
    }

    pub fn with_abi_storage(mut self, demand: &CallingAbiStorageDemand) -> Result<Self, String> {
        if self.inherited.is_some() {
            return Err("Product scalar completion cannot become an ABI storage scope".into());
        }
        demand.require_artifact(self.artifact)?;
        if Some(demand.target()) != self.target() {
            return Err("ABI storage demand names another selected target".into());
        }
        self.condition_scope = ScalarConditionScope::AbiStorage(demand.into());
        Ok(self)
    }

    pub fn values(&self) -> ScalarApplicationValues {
        ScalarApplicationValues(std::sync::Arc::clone(&self.applications))
    }

    pub fn conditions<'scope>(
        &self,
        value_scope: impl Into<Option<&'scope str>>,
        demanded: &[ScalarApplicationId],
    ) -> Result<ScalarConditionSelection<'artifact>, String> {
        let value_scope = value_scope.into();
        if let ScalarConditionScope::AbiStorage(scope) = &self.condition_scope
            && scope.applications != demanded
        {
            return Err("ABI scalar request differs from its canonical extent roster".into());
        }
        let catalog = &self.artifact.module().scalar_applications;
        let mut selected = ScalarConditionSelection::select(
            &catalog.proof_scope,
            &catalog.conditions,
            self.target(),
            self.target() == value_scope,
            self.providers.providers().chain(
                self.providers
                    .assumed_bindings()
                    .iter()
                    .map(|binding| (binding.required, binding.implementation)),
            ),
            demanded,
            &self.condition_scope,
        )?;
        if let Some(previous) = &self.inherited {
            selected.include_completed(&previous.conditions);
        }
        Ok(selected)
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

    pub fn entry(&self, application: ScalarApplicationId) -> Result<MachineId, String> {
        let Some(ScalarApplication::Evaluate { entry, scalar_type }) = self
            .artifact
            .module()
            .scalar_applications
            .declaration(application)
        else {
            return Err("scalar application is not a complete evaluation entry".into());
        };
        let machine = TerminalTargetContext::resolve_entry(
            self.targets,
            self.artifact.identity(),
            self.artifact.module(),
            *entry,
        )
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
        let extending = self.extends_completed_application(application);
        if extending
            && self
                .inherited
                .as_ref()
                .and_then(|scope| scope.values.value(application))
                != Some(value)
        {
            return Err("scalar completion extension changed a previously observed value".into());
        }
        let applications = std::sync::Arc::make_mut(&mut self.applications);
        let handle = applications
            .handle_of(&applications.storage_slice()[position])
            .ok_or("scalar application storage lost its row")?;
        let row = applications.get_mut(handle);
        if row.value.is_some() {
            return Err("scalar application completed more than once".into());
        }
        row.value = Some(value);
        if extending {
            self.reexecuted[position] = true;
        }
        Ok(())
    }

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
        let extending = self.extends_completed_application(application);
        let applications = std::sync::Arc::make_mut(&mut self.applications);
        let handle = applications
            .handle_of(&applications.storage_slice()[position])
            .ok_or("scalar application storage lost its row")?;
        let row = applications.get_mut(handle);
        if let Some(previous) = row
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
        {
            if extending && self.inherited.as_ref().is_some_and(|scope| scope.values.0.storage_slice()[position].results.storage_slice().iter().any(|row| matches!(row,
                EntryResult::Observed { result: observed, value } if *observed == result && *value == previous))) {
                return (previous == value).then_some(()).ok_or_else(||
                    "scalar completion extension changed a previously captured result".into());
            }
            return Err("scalar entry result was recorded more than once".into());
        }
        row.results.insert(EntryResult::Observed { result, value });
        Ok(())
    }

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
        let geometry = self.geometry()?;
        ReferenceStorageGeometry::new(geometry.declarations(), |application| {
            Err(StorageGeometryError::UnresolvedExtent(application))
        })
        .with_target_carriers(self.carrier_layout)
        .dependencies(*subject)
        .map_err(|error| error.to_string())
    }

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
            .geometry()
            .map_err(ScalarLayoutObservationError::Invalid)?;
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
        })
        .with_target_carriers(self.carrier_layout);
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

    pub(super) fn observation_ready(
        &self,
        observation: &ScalarRangeObservation,
    ) -> Result<bool, String> {
        let ready = match observation {
            ScalarRangeObservation::Literal(_) => return Ok(true),
            ScalarRangeObservation::Application(application) => {
                let scalar_type = self
                    .artifact
                    .module()
                    .scalar_applications
                    .declaration(*application)
                    .and_then(|row| row.scalar_type())
                    .ok_or("scalar condition application has no carrier")?;
                if !matches!(scalar_type, ScalarType::Integer(_)) {
                    return Err("scalar condition requires an integer observation".into());
                }
                self.value(*application)?.is_some()
            }
            ScalarRangeObservation::EntryResult {
                application,
                machine,
                result,
                scalar_type,
            } => {
                if self.entry(*application)? != *machine {
                    return Err("scalar condition names another entry activation".into());
                }
                if !matches!(scalar_type, ScalarType::Integer(_)) {
                    return Err("scalar condition requires an integer observation".into());
                }
                self.value(*application)?.is_some()
                    && (!self.extends_completed_application(*application)
                        || self
                            .entry_result(*application, *machine, *result, *scalar_type)
                            .is_ok())
            }
            ScalarRangeObservation::Missing => {
                return Err("scalar condition has an absent observation".into());
            }
        };
        if ready {
            self.integer(observation)?;
        }
        Ok(ready)
    }

    pub(super) fn integer(&self, observation: &ScalarRangeObservation) -> Result<BigInt, String> {
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

    pub fn discharge_range(
        &self,
        position: usize,
        address_model: Option<(&BigInt, &BigInt)>,
    ) -> Result<(), String> {
        if self
            .inherited
            .as_ref()
            .is_some_and(|scope| scope.conditions.ranges.binary_search(&position).is_ok())
        {
            return Ok(());
        }
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
                "scalar range condition {position}: bounds [{low}, {high}] lie outside [{minimum}, {maximum}]"
            ));
        }
        Ok(())
    }

    pub fn discharge_predicate(&self, position: usize) -> Result<(), String> {
        if self
            .inherited
            .as_ref()
            .is_some_and(|scope| scope.conditions.predicates.binary_search(&position).is_ok())
        {
            return Ok(());
        }
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
