//! Publish captured arithmetic requirements through the emitter's exact joins.
//!
//! Preparation retains the required applications before closure discovery.
//! Publication consumes the source-call associations emission already owns;
//! source handles stop here. A helper's final result cannot stand in for a
//! different intermediate call, and a missing owner is never module authority.

use super::SourceScalarApplications;
use crate::lowering_error::LoweringError;
use crate::unit::attached_unit::{BoundaryParameters, callables};
use checked_trees::{CheckedTrees, NominalMachineUseSite};
use lowered_psi::{LoweredSourceCallOccurrence, LoweredSourceCallSite};
use numerics::bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::{PredicateNode, PredicateOperator};
use semantic_vocabulary::{MachineId, ScalarApplicationId};
use symbols::SymbolHandle;
use terminal_psi::{
    ScalarConditionSubject, ScalarConditions, ScalarRangeObservation, ScalarRangeRequirement,
    TerminalCallable, TerminalMachine,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) struct Condition {
    owner: SymbolHandle,
    expression: ExpressionHandle,
    subject: Subject,
    requirement: Requirement,
}

enum Requirement {
    Completion(ScalarApplicationId),
    Range {
        requirement: ScalarRangeRequirement,
        steps: Vec<IntegerBoundStep<Observation>>,
    },
    Predicate {
        description: String,
        nodes: Vec<PredicateNode<usize, Observation>>,
        root: usize,
    },
}

pub(super) fn completion(
    checked: &CheckedTrees,
    applications: &mut SourceScalarApplications,
    owner: SymbolHandle,
    application: ScalarApplicationId,
) -> Result<Condition, LoweringError> {
    let expression = ExpressionHandle::invalid();
    let subject = subject(checked, applications, owner).map_err(|reason| {
        LoweringError::UnpublishedScalarCondition {
            owner,
            expression,
            reason,
        }
    })?;
    Ok(Condition {
        owner,
        expression,
        subject,
        requirement: Requirement::Completion(application),
    })
}

enum Subject {
    Module,
    Declaration { owner: SymbolHandle },
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

impl Condition {
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
) -> Result<Vec<Condition>, LoweringError> {
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
        ranges.push(Condition {
            owner,
            expression,
            subject,
            requirement: Requirement::Range { requirement, steps },
        });
    }
    for obligation in &checked.facts.constant_predicate_obligations {
        if !obligation.is_pending() {
            continue;
        }
        let owner = obligation.caller();
        let expression = ExpressionHandle::invalid();
        let failure = |reason| LoweringError::UnpublishedScalarCondition {
            owner,
            expression,
            reason,
        };
        let subject = subject(checked, applications, owner).map_err(&failure)?;
        let mut nodes = Vec::new();
        let root = obligation
            .map_nodes(
                &mut |expression, owner| observation(checked, applications, expression, owner),
                &mut |node| {
                    let ordinal = nodes.len();
                    nodes.push(node);
                    Ok(ordinal)
                },
            )
            .map_err(failure)?;
        ranges.push(Condition {
            owner,
            expression,
            subject,
            requirement: Requirement::Predicate {
                description: obligation.description().to_owned(),
                nodes,
                root,
            },
        });
    }
    for obligation in &checked.facts.constant_recipe_obligations {
        if !obligation.is_pending() {
            continue;
        }
        let owner = obligation.owner();
        let expression = ExpressionHandle::invalid();
        let failure = |reason| LoweringError::UnpublishedScalarCondition {
            owner,
            expression,
            reason,
        };
        // This producer explicitly records unscoped recipe formation. Missing
        // lookup for any valid owner is still an error, never module authority.
        let subject = if owner.is_valid() {
            subject(checked, applications, owner).map_err(&failure)?
        } else {
            Subject::Module
        };
        let left = recipe_observation(
            checked,
            applications,
            obligation.recipe(),
            obligation.recipe_owner(),
        )
        .map_err(&failure)?;
        use checked_trees::ConstantRecipeRequirement;
        let (operator, right) = match obligation.requirement() {
            ConstantRecipeRequirement::ExceedsBound(bound) => (
                PredicateOperator::Greater,
                Observation::Literal(bound.clone()),
            ),
            ConstantRecipeRequirement::AtLeast(bound) => (
                PredicateOperator::GreaterOrEqual,
                Observation::Literal(bound.clone()),
            ),
            ConstantRecipeRequirement::AtMost(bound) => (
                PredicateOperator::LessOrEqual,
                Observation::Literal(bound.clone()),
            ),
            ConstantRecipeRequirement::EqualRecipe(other) => (
                PredicateOperator::Equal,
                recipe_observation(checked, applications, *other, obligation.recipe_owner())
                    .map_err(failure)?,
            ),
        };
        ranges.push(Condition {
            owner,
            expression,
            subject,
            requirement: Requirement::Predicate {
                description: obligation.description().to_owned(),
                nodes: vec![
                    PredicateNode::Leaf(left),
                    PredicateNode::Leaf(right),
                    PredicateNode::Binary {
                        operator,
                        left: 0,
                        right: 1,
                    },
                ],
                root: 2,
            },
        });
    }
    Ok(ranges)
}

fn recipe_observation(
    checked: &CheckedTrees,
    applications: &mut SourceScalarApplications,
    recipe: typed_trees::constant_recipes::ConstantRecipeHandle,
    owner: SymbolHandle,
) -> Result<Observation, String> {
    if let Some(value) = checked.typed.closed_constant_recipe_value(recipe) {
        return Ok(Observation::Literal(value.clone()));
    }
    applications
        .retain_recipe_in(checked, recipe, owner)
        .map(Observation::Application)
        .map_err(|error| error.to_string())
}

fn subject(
    checked: &CheckedTrees,
    applications: &mut SourceScalarApplications,
    owner: SymbolHandle,
) -> Result<Subject, String> {
    let program = &checked.typed;
    if let Some(entry) = program
        .machine_by_own_or_state_symbol(owner)
        .and_then(|machine| program.constant_evaluation_entry(machine.symbol))
    {
        return applications
            .retain_entry(checked, entry)
            .map(Subject::Application)
            .map_err(|error| error.to_string());
    }
    if checked
        .facts
        .proof_applications
        .machines
        .iter()
        .any(|(_, machine)| {
            machine.machine == owner
                || checked
                    .facts
                    .proof_applications
                    .symbols
                    .span_or_empty(machine.states)
                    .contains(&owner)
        })
    {
        return Ok(Subject::Declaration { owner });
    }
    if program.const_declaration_by_symbol(owner).is_some()
        || program.data_definition_by_symbol(owner).is_some()
    {
        // Logical ownership does not request physical storage. A measurement
        // roots its actual queried type through its LayoutObservation row.
        return Ok(Subject::Declaration { owner });
    }
    // Bodyless declarations still owe formation of their contract operands.
    // The checked producer retained this exact owner; no spelling or missing
    // lookup can turn it into a module-wide requirement.
    let plan = &checked.facts.proof_applications;
    if plan
        .symbols
        .span_or_empty(plan.declarations)
        .contains(&owner)
    {
        return Ok(Subject::Declaration { owner });
    }
    Err("scalar condition has no retained declaration or evaluation owner".to_owned())
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
    if matches!(program.expression_table.expression(expression),
        ExpressionNode::Call(call) if call.layout_query.is_some())
    {
        return applications
            .retain_expression_in(checked, expression, owner)
            .map(Observation::Application)
            .map_err(|error| error.to_string());
    }
    if let Some(entry) = program
        .machine_by_own_or_state_symbol(owner)
        .and_then(|machine| program.constant_evaluation_entry(machine.symbol))
    {
        let application = applications
            .retain_entry(checked, entry)
            .map_err(|error| error.to_string())?;
        return Ok(Observation::EntryCall {
            application,
            entry: entry.entry,
            owner,
            expression,
        });
    }
    applications
        .retain_expression_in(checked, expression, owner)
        .map(Observation::Application)
        .map_err(|error| error.to_string())
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
    ranges: &[Condition],
    machines: &[(SymbolHandle, MachineId)],
    boundaries: &[BoundaryParameters],
    declarations: &[(SymbolHandle, terminal_psi::ProofDeclarationId)],
    emitted: EmittedScalars<'_>,
) -> Result<ScalarConditions, LoweringError> {
    let mut result = ScalarConditions::default();
    for range in ranges {
        let subject = match range.subject {
            Subject::Module => ScalarConditionSubject::Module,
            Subject::Declaration { owner: source, .. } => ScalarConditionSubject::Declaration(
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
            Subject::Application(application) => ScalarConditionSubject::Application(application),
        };
        match &range.requirement {
            Requirement::Completion(application) => result.retain_completion(subject, *application),
            Requirement::Range { requirement, steps } => {
                let steps = steps
                    .iter()
                    .map(|step| {
                        step.map_observation(&mut |observation| {
                            publish_observation(observation, machines, boundaries, &emitted)
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|reason| range.error(reason))?;
                result
                    .retain_range(subject, requirement.clone(), steps)
                    .map_err(|reason| range.error(reason.to_owned()))?;
            }
            Requirement::Predicate {
                description,
                nodes,
                root,
            } => {
                let mut published = arena::Arena::new();
                let mut coordinates = Vec::new();
                for node in nodes {
                    let mapped = node
                        .try_map(
                            &mut |reference| {
                                coordinates.get(reference).copied().ok_or_else(|| {
                                    "scalar predicate lost a preceding node".to_owned()
                                })
                            },
                            &mut |observation| {
                                publish_observation(observation, machines, boundaries, &emitted)
                            },
                        )
                        .map_err(|reason| range.error(reason))?;
                    coordinates.push(published.insert(terminal_psi::ScalarPredicateNode(mapped)));
                }
                let root = coordinates
                    .get(*root)
                    .copied()
                    .ok_or_else(|| range.error("scalar predicate lost its root".to_owned()))?;
                result
                    .retain_predicate(subject, description.clone(), published, root)
                    .map_err(|reason| range.error(reason.to_owned()))?;
            }
        }
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
    let callable =
        callables::callable(entry, machines, boundaries).map_err(|error| error.to_string())?;
    let TerminalCallable::Machine(machine) = callable else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{CheckFacts, ConstantRecipeObligation, ConstantRecipeRequirement};
    use numerics::scalar_predicates::ScalarValue;
    use typed_trees::constant_recipes::RetainedConstantRecipe;

    #[test]
    fn scalar_condition_publication_keeps_the_exact_state_owner() {
        let machine = SymbolHandle::from_parts(1, 1);
        let state = SymbolHandle::from_parts(2, 1);
        let mut facts = CheckFacts::default();
        let states = facts.proof_applications.symbols.insert_many([state]);
        facts.proof_applications.machines.insert(
            semantic_vocabulary::proof_applications::ProofApplicationMachine {
                machine,
                states,
                ..Default::default()
            },
        );
        let checked = CheckedTrees::with_roots(typed_trees::TypedTrees::default(), facts);
        let mut applications = SourceScalarApplications::default();
        assert!(
            matches!(subject(&checked, &mut applications, state).unwrap(),
            Subject::Declaration { owner, .. } if owner == state)
        );
    }

    #[test]
    fn scalar_condition_publication_retains_recipe_relations_and_explicit_module_scope() {
        let mut typed = typed_trees::TypedTrees::default();
        let first = typed
            .constant_recipes
            .insert(RetainedConstantRecipe::default());
        let second = typed
            .constant_recipes
            .insert(RetainedConstantRecipe::default());
        let first_application = ScalarApplicationId::new(1).unwrap();
        let second_application = ScalarApplicationId::new(2).unwrap();
        let mut facts = CheckFacts::default();
        for requirement in [
            ConstantRecipeRequirement::EqualRecipe(second),
            ConstantRecipeRequirement::ExceedsBound(BigInt::from_u64(23)),
            ConstantRecipeRequirement::AtLeast(BigInt::from_u64(24)),
            ConstantRecipeRequirement::AtMost(BigInt::from_u64(24)),
        ] {
            facts
                .constant_recipe_obligations
                .push(ConstantRecipeObligation::retain(
                    SymbolHandle::invalid(),
                    first,
                    requirement,
                    source::SourceSpan::default(),
                    "test formation".into(),
                ));
        }
        let checked = CheckedTrees::with_roots(typed, facts);
        let mut applications = SourceScalarApplications::default();
        applications.recipes.extend([
            (first, SymbolHandle::invalid(), first_application),
            (second, SymbolHandle::invalid(), second_application),
        ]);
        let prepared = prepare(&checked, &mut applications).unwrap();
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let conditions = publish(
            &prepared,
            &[],
            &[],
            &[],
            EmittedScalars {
                checked: &checked,
                identities: &mut identities,
                sources: &[],
                calls: &[],
                machines: &[],
            },
        )
        .unwrap();
        assert_eq!(conditions.predicates().len(), 4);
        for condition in conditions.predicates() {
            assert_eq!(condition.subject, ScalarConditionSubject::Module);
        }
        for (value, expected) in [
            (23, [true, false, false, true]),
            (24, [true, true, true, true]),
            (25, [true, true, true, false]),
        ] {
            for (condition, expected) in conditions.predicates().iter().zip(expected) {
                let result = condition.evaluate(&mut |observation| match observation {
                    ScalarRangeObservation::Application(_) => {
                        Some(ScalarValue::Integer(BigInt::from_u64(value)))
                    }
                    _ => panic!("literals retain their own value"),
                });
                assert_eq!(result, Some(ScalarValue::Boolean(expected)));
            }
        }
        for (second_value, expected) in [
            (Some(24), Some(true)),
            (Some(16), Some(false)),
            (None, None),
        ] {
            let mut observed = Vec::new();
            let value = conditions.predicates()[0].evaluate(&mut |observation| {
                let ScalarRangeObservation::Application(application) = observation else {
                    panic!("expected application");
                };
                observed.push(application);
                let value = if application == first_application {
                    Some(24)
                } else {
                    second_value
                }?;
                Some(ScalarValue::Integer(BigInt::from_u64(value)))
            });
            assert_eq!(observed, vec![first_application, second_application]);
            assert_eq!(value, expected.map(ScalarValue::Boolean));
        }
        assert!(subject(&checked, &mut applications, SymbolHandle::from_parts(99, 1)).is_err());
        assert!(subject(&checked, &mut applications, SymbolHandle::invalid()).is_err());
    }
}
