//! Prove a constant call's premises before running the operations they protect.
//!
//! Independent calculations run first. Checks on earlier results inside this
//! entry run when those results arrive; they cannot use the entry's final answer.

use super::{ScalarAddressModel, ScalarApplicationObservations};
use crate::TerminalScalarValue;
use numerics::bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::{PredicateNode, ScalarValue};
use semantic_vocabulary::{
    IntegerValue, MachineId, ScalarApplicationId, ScalarConditionRole, ScalarType, ValueId,
};
use terminal_psi::{
    ScalarConditionSubject, ScalarConditions, ScalarPredicateCondition, ScalarRangeCondition,
    ScalarRangeObservation, ScalarRangeRequirement, TargetMachineCallable,
};

#[derive(Clone, Copy)]
enum ConditionId {
    Range(usize),
    Predicate(usize),
}

/// One immutable selection shared by dependency discovery and execution.
pub struct ScalarEntryPrerequisites<'artifact> {
    conditions: &'artifact ScalarConditions,
    application: ScalarApplicationId,
    entry: MachineId,
    target: String,
    providers: crate::AdmittedProviderSelections,
    rows: Vec<ConditionId>,
}

/// Only successful prerequisite admission can create a provisional entry request.
pub struct AdmittedScalarEntry<'input, 'artifact> {
    observations: &'input ScalarApplicationObservations<'artifact>,
    application: ScalarApplicationId,
    checks: ScalarEntryChecks,
}

impl<'input, 'artifact> AdmittedScalarEntry<'input, 'artifact> {
    pub fn observations(&self) -> &'input ScalarApplicationObservations<'artifact> {
        self.observations
    }
    pub fn application(&self) -> ScalarApplicationId {
        self.application
    }
    pub fn checks(&self) -> ScalarEntryChecks {
        self.checks.clone()
    }
}

impl<'artifact> ScalarEntryPrerequisites<'artifact> {
    pub fn prepare(
        observations: &ScalarApplicationObservations<'artifact>,
        application: ScalarApplicationId,
    ) -> Result<Self, String> {
        let catalog = &observations.artifact().module().scalar_applications;
        let conditions = &catalog.conditions;
        let scope = &catalog.proof_scope;
        let entry = observations.entry(application)?;
        let mut rows = Vec::new();
        for (position, condition) in conditions.ranges().iter().enumerate() {
            if matches!(
                condition,
                ScalarRangeCondition::Range {
                    role: ScalarConditionRole::Prerequisite,
                    subject: ScalarConditionSubject::Declaration(_)
                        | ScalarConditionSubject::Application(_),
                    ..
                }
            ) {
                rows.push(ConditionId::Range(position));
            }
        }
        for (position, condition) in conditions.predicates().iter().enumerate() {
            if condition.role == ScalarConditionRole::Prerequisite
                && !matches!(condition.subject, ScalarConditionSubject::Module)
            {
                rows.push(ConditionId::Predicate(position));
            }
        }
        if !rows.is_empty() {
            let root = scope
                .callable_declaration(TargetMachineCallable::Machine(entry))
                .ok_or("scalar prerequisite entry has no checked declaration")?;
            let invocation = crate::compiler_evaluation::compiler_invocation_scope(
                scope,
                observations.targets().target(),
                observations.providers(),
                root,
            )?;
            let applies = |subject| -> Result<bool, String> {
                Ok(match subject {
                    ScalarConditionSubject::Declaration(owner) => {
                        invocation.dependencies.contains(&owner)
                    }
                    ScalarConditionSubject::Application(owner) if owner == application => true,
                    ScalarConditionSubject::Application(owner) => {
                        match catalog.declaration(owner) {
                            Some(terminal_psi::ScalarApplication::Evaluate { .. }) => {
                                let owner = scope.callable_declaration(TargetMachineCallable::Machine(observations.entry(owner)?))
                            .ok_or("scalar prerequisite owner has no checked entry declaration")?;
                                invocation.dependencies.contains(&owner)
                            }
                            Some(terminal_psi::ScalarApplication::LayoutObservation { .. }) => {
                                false
                            }
                            _ => {
                                return Err(
                                    "scalar prerequisite owner application is absent".into()
                                );
                            }
                        }
                    }
                    ScalarConditionSubject::Module => false,
                })
            };
            let mut retained = Vec::new();
            for row in rows {
                let subject = match row {
                    ConditionId::Range(position) => match conditions.ranges()[position] {
                        ScalarRangeCondition::Range { subject, .. } => subject,
                        _ => return Err("scalar prerequisite range is absent".into()),
                    },
                    ConditionId::Predicate(position) => conditions.predicates()[position].subject,
                };
                if applies(subject)? {
                    retained.push(row);
                }
            }
            rows = retained;
        }
        Ok(Self {
            conditions,
            application,
            entry,
            target: observations.targets().target().to_owned(),
            providers: observations.providers().clone(),
            rows,
        })
    }

    fn local(&self, observation: &ScalarRangeObservation) -> bool {
        matches!(observation, ScalarRangeObservation::EntryResult { application, machine, .. } if *application == self.application && *machine == self.entry)
    }

    fn visit(
        &self,
        row: ConditionId,
        mut visit: impl FnMut(&ScalarRangeObservation) -> Result<(), String>,
    ) -> Result<(), String> {
        match row {
            ConditionId::Range(position) => {
                for step in self.conditions.steps(&self.conditions.ranges()[position])? {
                    if let IntegerBoundStep::Observation(observation) = step {
                        visit(observation)?;
                    }
                }
            }
            ConditionId::Predicate(position) => {
                for (_, node) in self.conditions.predicates()[position].nodes().iter() {
                    if let PredicateNode::Leaf(observation) = &node.0 {
                        visit(observation)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn applications(&self) -> Result<Vec<ScalarApplicationId>, String> {
        let mut applications = Vec::new();
        for &row in &self.rows {
            self.visit(row, |observation| {
                if !self.local(observation)
                    && let ScalarRangeObservation::Application(application)
                    | ScalarRangeObservation::EntryResult { application, .. } = observation
                    && !applications.contains(application)
                {
                    applications.push(*application);
                }
                Ok(())
            })?;
        }
        Ok(applications)
    }

    /// Explain which retained check needs this unfinished calculation. These
    /// descriptions are diagnostics; dependency decisions use the typed rows.
    pub fn dependency_reason(
        &self,
        dependency: ScalarApplicationId,
    ) -> Result<Option<String>, String> {
        for &row in &self.rows {
            let mut needs_value = false;
            self.visit(row, |observation| {
                needs_value |= !self.local(observation)
                    && matches!(observation, ScalarRangeObservation::Application(application)
                        | ScalarRangeObservation::EntryResult { application, .. } if *application == dependency);
                Ok(())
            })?;
            if needs_value {
                return Ok(Some(match row {
                    ConditionId::Range(position) => format!("range condition {position}"),
                    ConditionId::Predicate(position) => {
                        self.conditions.predicates()[position].description.clone()
                    }
                }));
            }
        }
        Ok(None)
    }

    pub fn admit<'input>(
        &self,
        observations: &'input ScalarApplicationObservations<'artifact>,
        address_model: Option<&ScalarAddressModel>,
    ) -> Result<AdmittedScalarEntry<'input, 'artifact>, String> {
        if !std::ptr::eq(
            self.conditions,
            &observations
                .artifact()
                .module()
                .scalar_applications
                .conditions,
        ) || self.target != observations.targets().target()
            || self.providers != *observations.providers()
            || self.entry != observations.entry(self.application)?
        {
            return Err("scalar prerequisites name a different artifact, invocation, target or provider selection".into());
        }
        let scope = &observations
            .artifact()
            .module()
            .scalar_applications
            .proof_scope;
        let root = scope
            .callable_declaration(TargetMachineCallable::Machine(self.entry))
            .ok_or("scalar prerequisite entry has no checked declaration")?;
        crate::compiler_evaluation::require_root_failure_admission(
            observations.artifact().module(),
            root,
        )?;
        let mut checks = ScalarEntryChecks::default();
        for &row in &self.rows {
            let mut inputs = Vec::new();
            self.visit(row, |observation| {
                if inputs.iter().any(|(existing, _)| existing == observation) {
                    return Ok(());
                }
                let value = if self.local(observation) {
                    if let ScalarRangeObservation::EntryResult {
                        result,
                        scalar_type,
                        ..
                    } = observation
                    {
                        let capture = (*result, *scalar_type);
                        if !checks.results.contains(&capture) {
                            checks.results.push(capture);
                        }
                    }
                    None
                } else {
                    Some(observations.integer(observation)?)
                };
                inputs.push((observation.clone(), value));
                Ok(())
            })?;
            let condition = match row {
                ConditionId::Range(position) => {
                    let range = &self.conditions.ranges()[position];
                    let ScalarRangeCondition::Range { requirement, .. } = range else {
                        return Err("scalar prerequisite range is absent".into());
                    };
                    let (minimum, maximum) = match requirement {
                        ScalarRangeRequirement::Fixed { minimum, maximum } => {
                            (minimum.clone(), maximum.clone())
                        }
                        ScalarRangeRequirement::AddressModel => {
                            let model = address_model
                                .ok_or("scalar prerequisite needs an admitted address model")?;
                            if model.target != self.target
                                || !model.carrier.is_address()
                                || model.semantics_version == 0
                            {
                                return Err("scalar prerequisite address model differs from the selected target".into());
                            }
                            (
                                BigInt::zero(),
                                BigInt::from_u64(1)
                                    .shl_bits(usize::from(model.carrier.bits()))
                                    .sub(&BigInt::from_u64(1)),
                            )
                        }
                    };
                    EntryCondition::Range {
                        position,
                        steps: self.conditions.steps(range)?.to_vec(),
                        minimum,
                        maximum,
                    }
                }
                ConditionId::Predicate(position) => {
                    EntryCondition::Predicate(self.conditions.predicates()[position].clone())
                }
            };
            let mut check = EntryCheck {
                condition,
                inputs,
                complete: false,
            };
            check.discharge_ready(&mut |_, _| Ok(None))?;
            if !check.complete {
                checks.pending.push(check);
            }
        }
        Ok(AdmittedScalarEntry {
            observations,
            application: self.application,
            checks,
        })
    }
}

#[derive(Clone, Default)]
pub struct ScalarEntryChecks {
    results: Vec<(ValueId, ScalarType)>,
    pending: Vec<EntryCheck>,
}

#[derive(Clone)]
struct EntryCheck {
    condition: EntryCondition,
    inputs: Vec<(ScalarRangeObservation, Option<BigInt>)>,
    complete: bool,
}

#[derive(Clone)]
enum EntryCondition {
    Range {
        position: usize,
        steps: Vec<IntegerBoundStep<ScalarRangeObservation>>,
        minimum: BigInt,
        maximum: BigInt,
    },
    Predicate(ScalarPredicateCondition),
}

impl ScalarEntryChecks {
    pub fn results(&self) -> &[(ValueId, ScalarType)] {
        &self.results
    }

    pub fn discharge_ready(
        &mut self,
        mut observe: impl FnMut(ValueId, ScalarType) -> Result<Option<TerminalScalarValue>, String>,
    ) -> Result<(), String> {
        for check in &mut self.pending {
            if !check.complete {
                check.discharge_ready(&mut observe)?;
            }
        }
        Ok(())
    }

    pub fn require_complete(&self) -> Result<(), String> {
        if self.pending.iter().any(|check| !check.complete) {
            return Err("constant entry returned without establishing every prerequisite from its exact operand occurrences".into());
        }
        Ok(())
    }
}

impl EntryCheck {
    fn discharge_ready(
        &mut self,
        observe: &mut impl FnMut(ValueId, ScalarType) -> Result<Option<TerminalScalarValue>, String>,
    ) -> Result<(), String> {
        for (observation, value) in &mut self.inputs {
            if value.is_some() {
                continue;
            }
            let ScalarRangeObservation::EntryResult {
                result,
                scalar_type,
                ..
            } = observation
            else {
                return Err("scalar prerequisite is missing an independent observation".into());
            };
            if let Some(observed) = observe(*result, *scalar_type)? {
                if observed.scalar_type() != *scalar_type {
                    return Err("scalar prerequisite operand changed its carrier".into());
                }
                *value = Some(match observed {
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Signed(value),
                        ..
                    } => BigInt::from_i128(value),
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(value),
                        ..
                    } => BigInt::from_u128(value),
                    _ => return Err("scalar prerequisite operand is not an integer".into()),
                });
            }
        }
        if self.inputs.iter().any(|(_, value)| value.is_none()) {
            return Ok(());
        }
        let integer = |observation: &ScalarRangeObservation| {
            self.inputs
                .iter()
                .find(|(input, _)| input == observation)
                .and_then(|(_, value)| value.clone())
                .ok_or_else(|| "scalar prerequisite lost its captured operand".to_owned())
        };
        match &self.condition {
            EntryCondition::Range {
                position,
                steps,
                minimum,
                maximum,
            } => {
                let (low, high) = numerics::integer_bounds::evaluate(steps, &mut |observation| {
                    integer(observation)
                })?;
                if low < *minimum || high > *maximum {
                    return Err(format!(
                        "constant invocation prerequisite: scalar range condition {position}: bounds [{low}, {high}] lie outside [{minimum}, {maximum}]"
                    ));
                }
            }
            EntryCondition::Predicate(condition) => match condition
                .evaluate(&mut |observation| integer(&observation).ok().map(ScalarValue::Integer))
            {
                Some(ScalarValue::Boolean(true)) => (),
                Some(ScalarValue::Boolean(false)) => {
                    return Err(format!(
                        "constant invocation prerequisite: scalar predicate is false: {}",
                        condition.description
                    ));
                }
                _ => {
                    return Err(format!(
                        "constant invocation prerequisite could not be established: {}",
                        condition.description
                    ));
                }
            },
        }
        self.complete = true;
        Ok(())
    }
}
