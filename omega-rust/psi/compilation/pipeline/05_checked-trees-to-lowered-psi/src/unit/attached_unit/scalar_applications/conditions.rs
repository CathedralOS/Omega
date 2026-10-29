//! Publish captured arithmetic requirements through the emitter's exact joins.
//!
//! Preparation retains the required applications before closure discovery.
//! Publication consumes the source-call associations emission already owns;
//! source handles stop here. A helper's final result cannot stand in for a
//! different intermediate call, and a missing owner is never module authority.

use super::SourceScalarApplications;
use crate::lowering_error::LoweringError;
use crate::unit::attached_unit::{BoundaryParameters, lookup_type_id, target_families};
use checked_trees::{CheckedTrees, NominalMachineUseSite};
use lowered_psi::{LoweredSourceCallOccurrence, LoweredSourceCallSite};
use numerics::bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use semantic_vocabulary::{MachineId, ScalarApplicationId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_psi::{
    ScalarConditionSubject, ScalarConditions, ScalarRangeObservation, ScalarRangeRequirement,
    TargetMachineCallable, TerminalMachine,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::type_identity::TypeIdentityId;

pub(super) struct Range {
    owner: SymbolHandle,
    expression: ExpressionHandle,
    subject: Subject,
    requirement: ScalarRangeRequirement,
    steps: Vec<IntegerBoundStep<Observation>>,
}

enum Subject {
    Callable(SymbolHandle),
    StructuralType(TypeIdentityId),
    Application(ScalarApplicationId),
}

#[derive(Clone)]
enum Observation {
    Literal(BigInt),
    Application(ScalarApplicationId),
    EntryCall {
        application: ScalarApplicationId,
        entry: SymbolHandle,
        owner: SymbolHandle,
        expression: ExpressionHandle,
    },
}

impl Range {
    pub(super) fn type_root(&self) -> Option<TypeIdentityId> {
        match self.subject {
            Subject::StructuralType(subject) => Some(subject),
            _ => None,
        }
    }

    fn error(&self, reason: String) -> LoweringError {
        LoweringError::UnpublishedScalarCondition {
            owner: self.owner,
            expression: self.expression,
            reason,
        }
    }
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    applications: &mut SourceScalarApplications,
) -> Result<Vec<Range>, LoweringError> {
    let mut ranges = Vec::new();
    for obligation in &checked.facts.integer_range_obligations {
        if !obligation.is_pending() {
            continue;
        }
        let owner = obligation.owner();
        let expression = obligation.expression();
        let failure = |reason| LoweringError::UnpublishedScalarCondition {
            owner,
            expression,
            reason,
        };
        let subject = subject(checked, applications, owner).map_err(&failure)?;
        let steps = obligation
            .map_steps(&mut |expression, owner| {
                observation(checked, applications, expression, owner)
            })
            .map_err(failure)?;
        let requirement = match obligation.fixed_limits() {
            Some((minimum, maximum)) => ScalarRangeRequirement::Fixed {
                minimum: minimum.clone(),
                maximum: maximum.clone(),
            },
            None => ScalarRangeRequirement::AddressModel,
        };
        ranges.push(Range {
            owner,
            expression,
            subject,
            requirement,
            steps,
        });
    }
    Ok(ranges)
}

fn subject(
    checked: &CheckedTrees,
    applications: &mut SourceScalarApplications,
    owner: SymbolHandle,
) -> Result<Subject, String> {
    let program = &checked.typed;
    if let Some((_, machine)) =
        checked
            .facts
            .proof_applications
            .machines
            .iter()
            .find(|(_, machine)| {
                machine.machine == owner
                    || checked
                        .facts
                        .proof_applications
                        .symbols
                        .span_or_empty(machine.states)
                        .contains(&owner)
            })
    {
        return Ok(Subject::Callable(machine.machine));
    }
    if let Some(declaration) = program.const_declaration_by_symbol(owner) {
        return applications
            .retain_expression(checked, declaration.materialized_initializer)
            .map(Subject::Application)
            .map_err(|error| error.to_string());
    }
    if program.data_definition_by_symbol(owner).is_some() {
        return program
            .normalized_nominal_type_identity(owner)
            .map(Subject::StructuralType)
            .ok_or_else(|| "scalar condition's data owner has no exact type identity".to_owned());
    }
    Err("scalar condition has no retained callable, constant or data owner".to_owned())
}

fn observation(
    checked: &CheckedTrees,
    applications: &mut SourceScalarApplications,
    expression: ExpressionHandle,
    owner: SymbolHandle,
) -> Result<Observation, String> {
    let program = &checked.typed;
    if let Some(value) = program.closed_integer_expression_value_in(expression, owner) {
        return Ok(Observation::Literal(value));
    }
    if let Some(application) = applications.expression(expression) {
        return Ok(Observation::Application(application));
    }
    if matches!(program.expression_table.expression(expression),
        ExpressionNode::Call(call) if call.layout_query.is_some())
    {
        return applications
            .retain_expression(checked, expression)
            .map(Observation::Application)
            .map_err(|error| error.to_string());
    }
    let entry = program
        .machine_by_own_or_state_symbol(owner)
        .ok_or_else(|| "scalar observation has no exact evaluation owner".to_owned())?;
    let declaration = program
        .const_declaration_for_evaluation_entry(entry.symbol)
        .ok_or_else(|| {
            "scalar observation does not belong to a complete constant entry".to_owned()
        })?;
    let application = applications
        .retain_expression(checked, declaration.materialized_initializer)
        .map_err(|error| error.to_string())?;
    Ok(Observation::EntryCall {
        application,
        entry: entry.symbol,
        owner,
        expression,
    })
}

/// Borrow the emitter's associations before source coordinates are discarded.
pub(crate) struct EmittedScalars<'a> {
    pub(crate) checked: &'a CheckedTrees,
    pub(crate) identities: &'a mut terminal_psi::TerminalIdentityTableBuilder,
    pub(crate) sources: &'a [(source::SourceId, [u8; 32])],
    pub(crate) calls: &'a [LoweredSourceCallOccurrence],
    pub(crate) machines: &'a [TerminalMachine],
}

pub(super) fn publish(
    ranges: &[Range],
    types: &[(TypeIdentityId, StructuralTypeId)],
    machines: &[(SymbolHandle, MachineId)],
    boundaries: &[BoundaryParameters],
    declarations: &[(SymbolHandle, terminal_psi::ProofDeclarationId)],
    emitted: EmittedScalars<'_>,
) -> Result<ScalarConditions, LoweringError> {
    let mut result = ScalarConditions::default();
    for range in ranges {
        let subject = match range.subject {
            Subject::Callable(source) => ScalarConditionSubject::Declaration(
                declarations
                    .iter()
                    .find(|(owner, _)| *owner == source)
                    .map(|(_, identity)| *identity)
                    .ok_or_else(|| {
                        range.error(
                            "scalar condition lost its retained declaration coordinate".into(),
                        )
                    })?,
            ),
            Subject::StructuralType(source) => {
                ScalarConditionSubject::StructuralType(lookup_type_id(types, &source)?)
            }
            Subject::Application(application) => ScalarConditionSubject::Application(application),
        };
        let steps = range
            .steps
            .iter()
            .map(|step| {
                step.map_observation(&mut |observation| {
                    publish_observation(observation, machines, boundaries, &emitted)
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|reason| range.error(reason))?;
        result
            .retain_range(subject, range.requirement.clone(), steps)
            .map_err(|reason| range.error(reason.to_owned()))?;
    }
    Ok(result)
}

fn publish_observation(
    observation: &Observation,
    machines: &[(SymbolHandle, MachineId)],
    boundaries: &[BoundaryParameters],
    emitted: &EmittedScalars<'_>,
) -> Result<ScalarRangeObservation, String> {
    let (application, entry, owner, expression) = match *observation {
        Observation::Literal(ref value) => {
            return Ok(ScalarRangeObservation::Literal(value.clone()));
        }
        Observation::Application(application) => {
            return Ok(ScalarRangeObservation::Application(application));
        }
        Observation::EntryCall {
            application,
            entry,
            owner,
            expression,
        } => (application, entry, owner, expression),
    };
    let callable = target_families::callable(entry, machines, boundaries)
        .map_err(|error| error.to_string())?;
    let TargetMachineCallable::Machine(machine) = callable else {
        return Err(
            "scalar observation's complete evaluation entry has no ordinary body".to_owned(),
        );
    };
    let body = emitted
        .machines
        .iter()
        .find(|body| body.id == machine)
        .ok_or_else(|| "scalar observation lost its emitted entry body".to_owned())?;
    let mut occurrences = emitted.calls.iter().filter(|call| {
        call.source_site == Some(NominalMachineUseSite::Expression(expression))
            && (call.source_state == owner
                || (owner == entry
                    && emitted
                        .checked
                        .typed
                        .machine_by_own_or_state_symbol(call.source_state)
                        .is_some_and(|machine| machine.symbol == entry)))
    });
    let occurrence = occurrences
        .next()
        .ok_or_else(|| "scalar observation lost its exact emitted call occurrence".to_owned())?;
    if occurrences.next().is_some() {
        return Err("scalar observation maps to multiple emitted call occurrences".to_owned());
    }
    let LoweredSourceCallSite::Operation(operation) = occurrence.terminal_site else {
        return Err("scalar observation names a call without normal completion".to_owned());
    };
    let value = body
        .operations()
        .find(|emitted| emitted.id == operation)
        .and_then(|operation| operation.result.as_scalar())
        .ok_or_else(|| {
            "scalar observation's call has no scalar result in its entry body".to_owned()
        })?;
    if value.scalar_type.as_integer().is_none() {
        return Err("scalar observation's exact call result is not an integer".to_owned());
    }
    Ok(ScalarRangeObservation::EntryResult {
        application,
        machine,
        result: value.id,
        scalar_type: value.scalar_type,
    })
}
