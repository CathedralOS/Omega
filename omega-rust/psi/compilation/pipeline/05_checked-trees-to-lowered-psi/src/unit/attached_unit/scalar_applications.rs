//! Assign one application namespace before types, contracts and bodies lower.
//!
//! Source coordinates exist only in this preparation object. Publication joins
//! its queried types and complete evaluation entries to the closure's ordinary
//! declaration namespaces. A nested helper is never a substitute for the root
//! expression, and an absent checked entry cannot become an executable recipe.

use crate::lowering_error::LoweringError;
use checked_trees::{CheckedScalarComputationKind, CheckedTrees};
use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId};
use terminal_psi::{LayoutMeasurement, ScalarApplication, ScalarApplications};
use typed_trees::constant_recipes::{
    ConstantEvaluationEntry, ConstantRecipeHandle, RetainedConstantPosition,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::type_identity::TypeIdentityId;

mod conditions;
mod proof_scope;
#[cfg(test)]
mod tests;

/// Invocation-local construction storage, shared by all bodies in one closure.
#[derive(Default)]
pub(crate) struct SourceScalarApplications {
    rows: Vec<Application>,
    expressions: Vec<(ExpressionHandle, symbols::SymbolHandle, ScalarApplicationId)>,
    recipes: Vec<(
        ConstantRecipeHandle,
        symbols::SymbolHandle,
        ScalarApplicationId,
    )>,
    conditions: Vec<conditions::Condition>,
}

struct Application {
    definition: Definition,
    scalar_type: ScalarType,
}

#[derive(PartialEq, Eq)]
enum Definition {
    Layout {
        subject: TypeIdentityId,
        measurement: LayoutMeasurement,
    },
    Evaluation {
        entry: symbols::SymbolHandle,
    },
}

impl SourceScalarApplications {
    pub(crate) const EMPTY: Self = Self {
        rows: Vec::new(),
        expressions: Vec::new(),
        recipes: Vec::new(),
        conditions: Vec::new(),
    };

    pub(crate) fn prepare(checked: &CheckedTrees) -> Result<Self, LoweringError> {
        let mut applications = Self::default();
        let references = typed_trees::references::collect_program_references(&checked.typed);
        let owners = typed_trees::references::collect_program_operand_owners(&checked.typed);
        // A closed source program still carries historical recipe coordinates.
        // Only genuinely pending values need a portable application.
        for (recipe, declaration) in checked.typed.constant_recipes.iter() {
            if checked.typed.closed_constant_recipe_value(recipe).is_some() {
                continue;
            }
            let retained = match declaration.position {
                RetainedConstantPosition::ArrayLength(reference)
                | RetainedConstantPosition::ConstApplication {
                    site: reference, ..
                } => references.type_references.contains(&reference),
                RetainedConstantPosition::RangeEndpoint(expression)
                | RetainedConstantPosition::ContractExpression(expression) => {
                    references.expressions.contains(&expression)
                }
                RetainedConstantPosition::None => false,
            };
            if !retained {
                continue;
            }
            // Preparation publishes entries only for closed, live contexts.
            // Open expression inputs retain descriptions, not executable entries.
            // A concrete type/contract still requires its exact application
            // when that consumer lowers; this does not supply a fallback.
            if !declaration.contexts.is_empty() {
                for context in checked
                    .typed
                    .constant_expression_contexts
                    .span_or_empty(declaration.contexts)
                {
                    if checked
                        .typed
                        .machine_by_symbol(context.evaluation_entry)
                        .is_some()
                    {
                        let application =
                            applications.retain_recipe_in(checked, recipe, context.owner)?;
                        let condition = conditions::completion(
                            checked,
                            &mut applications,
                            context.owner,
                            application,
                        )?;
                        applications.conditions.push(condition);
                    }
                }
            } else {
                let declaring_owners = match declaration.position {
                    RetainedConstantPosition::ArrayLength(reference)
                    | RetainedConstantPosition::ConstApplication {
                        site: reference, ..
                    } => owners.type_owners(reference).collect::<Vec<_>>(),
                    RetainedConstantPosition::RangeEndpoint(expression) => {
                        owners.expression_owners(expression).collect()
                    }
                    _ => Vec::new(),
                };
                if declaring_owners.is_empty() {
                    return Err(LoweringError::Unsupported(
                        "live constant formation has no exact declaring owner",
                    ));
                }
                for owner in declaring_owners {
                    let application = applications.retain_recipe_in(checked, recipe, owner)?;
                    let condition =
                        conditions::completion(checked, &mut applications, owner, application)?;
                    applications.conditions.push(condition);
                }
            }
        }
        // Initializer formation is owed by the declaration even when no
        // executable use reads its value. Retain its real complete entry.
        for declaration in checked.typed.const_declarations() {
            if declaration.evaluated_value.is_some() || !declaration.evaluation_entry.is_valid() {
                continue;
            }
            if !checked
                .typed
                .tables
                .machines
                .is_valid(declaration.evaluation_entry)
            {
                return Err(LoweringError::Unsupported(
                    "constant declaration has a stale evaluation entry",
                ));
            }
            let entry = checked
                .typed
                .tables
                .machines
                .get(declaration.evaluation_entry)
                .symbol;
            let entry = checked.typed.constant_evaluation_entry(entry).ok_or(
                LoweringError::Unsupported("constant declaration lost its evaluation entry"),
            )?;
            let application = applications.retain_entry(checked, entry)?;
            let condition = conditions::completion(
                checked,
                &mut applications,
                declaration.symbol,
                application,
            )?;
            applications.conditions.push(condition);
        }
        // Authored layout queries owe a defined observation in their lexical
        // scope independently of runtime reach. A measurement is not a fake
        // executable entry; its ordinary application kind remains explicit.
        for observation in &checked.facts.flow.terminal_unit_effects.layout_observations {
            if !references.expressions.contains(&observation.expression)
                || checked
                    .landed_integer_constant(observation.expression)
                    .is_some()
            {
                continue;
            }
            let application = applications.retain_expression_in(
                checked,
                observation.expression,
                observation.owner,
            )?;
            let condition =
                conditions::completion(checked, &mut applications, observation.owner, application)?;
            applications.conditions.push(condition);
        }
        for (_, computation) in checked.facts.values.scalar_computations.nodes.iter() {
            if let CheckedScalarComputationKind::RetainedConstant { expression } = computation.kind
                && references.expressions.contains(&expression)
                && checked.landed_integer_constant(expression).is_none()
                && !applications
                    .expressions
                    .iter()
                    .any(|(source, _, _)| *source == expression)
            {
                applications.retain_expression(checked, expression)?;
            }
        }
        let conditions = conditions::prepare(checked, &mut applications)?;
        applications.conditions.extend(conditions);
        Ok(applications)
    }

    fn retain_recipe_in(
        &mut self,
        checked: &CheckedTrees,
        recipe: ConstantRecipeHandle,
        owner: symbols::SymbolHandle,
    ) -> Result<ScalarApplicationId, LoweringError> {
        if let Some(application) = self
            .recipes
            .iter()
            .find(|(source, scope, _)| *source == recipe && *scope == owner)
            .map(|(_, _, application)| *application)
        {
            return Ok(application);
        }
        if !checked.typed.constant_recipes.is_valid(recipe) {
            return Err(LoweringError::Unsupported(
                "scalar condition lost its exact recipe",
            ));
        }
        let declaration = checked.typed.constant_recipes.get(recipe);
        let application = match declaration.position {
            RetainedConstantPosition::ArrayLength(reference) => {
                self.retain_array_length(checked, reference)?
            }
            RetainedConstantPosition::RangeEndpoint(expression)
            | RetainedConstantPosition::ContractExpression(expression)
            | RetainedConstantPosition::ConstApplication {
                root: expression, ..
            } => self.retain_expression_in(checked, expression, owner)?,
            RetainedConstantPosition::None => {
                return Err(LoweringError::Unsupported(
                    "pending constant catalog contains an absent source position",
                ));
            }
        };
        self.recipes.push((recipe, owner, application));
        Ok(application)
    }

    fn retain_array_length(
        &mut self,
        checked: &CheckedTrees,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Result<ScalarApplicationId, LoweringError> {
        let typed_trees::types::TypeReferenceNode::FixedArray {
            length: typed_trees::types::FixedArrayLength::ConstCall { machine, .. },
            ..
        } = checked.typed.type_reference_table.type_reference(reference)
        else {
            return Err(LoweringError::Unsupported(
                "pending array extent has no complete retained invocation",
            ));
        };
        let entry = checked
            .typed
            .machine_by_symbol(*machine)
            .ok_or(LoweringError::Unsupported(
                "pending array extent lost its exact invocation declaration",
            ))?;
        let state =
            checked
                .typed
                .machine_states(entry)
                .first()
                .ok_or(LoweringError::Unsupported(
                    "pending array extent invocation has no entry signature",
                ))?;
        if !checked.typed.state_parameters(state).is_empty() {
            return Err(LoweringError::Unsupported(
                "pending array extent invocation requires runtime arguments",
            ));
        }
        let primitive = checked
            .typed
            .primitive_type_reference(state.return_type)
            .ok_or(LoweringError::Unsupported(
                "pending array extent invocation has no scalar result",
            ))?;
        let scalar_type = crate::emission::scalar_types::terminal_scalar_type(primitive)?;
        if scalar_type.as_integer().is_none() {
            return Err(LoweringError::Unsupported(
                "pending array extent result is not an integer",
            ));
        }
        self.insert(Definition::Evaluation { entry: *machine }, scalar_type)
    }

    fn retain_expression(
        &mut self,
        checked: &CheckedTrees,
        expression: ExpressionHandle,
    ) -> Result<ScalarApplicationId, LoweringError> {
        self.retain_expression_in(checked, expression, symbols::SymbolHandle::invalid())
    }

    fn retain_expression_in(
        &mut self,
        checked: &CheckedTrees,
        expression: ExpressionHandle,
        owner: symbols::SymbolHandle,
    ) -> Result<ScalarApplicationId, LoweringError> {
        if let Some(application) = self.expression_in(expression, owner) {
            return Ok(application);
        }
        if let Some(entry) = checked
            .typed
            .constant_evaluation_for_expression(expression, owner)
            .map_err(LoweringError::Unsupported)?
        {
            let application = self.retain_entry(checked, entry)?;
            self.expressions.push((expression, owner, application));
            return Ok(application);
        }
        let (definition, scalar_type) = if let ExpressionNode::Call(call) =
            checked.typed.expression_table.expression(expression)
            && let Some(query) = &call.layout_query
        {
            let mut observations = checked
                .facts
                .flow
                .terminal_unit_effects
                .layout_observations
                .iter()
                .filter(|observation| {
                    observation.expression == expression
                        && (!owner.is_valid() || observation.owner == owner)
                });
            let observation = observations.next().ok_or(LoweringError::Unsupported(
                "pending layout query has no checked structural subject",
            ))?;
            if observation.measurement != query.kind
                || observations.any(|other| {
                    other.subject != observation.subject
                        || other.measurement != observation.measurement
                })
            {
                return Err(LoweringError::Unsupported(
                    "pending layout query has ambiguous lexical structural subjects",
                ));
            }
            (
                Definition::Layout {
                    subject: observation.subject,
                    measurement: match observation.measurement {
                        language_core::data_layout_queries::DataLayoutQueryKind::Size => {
                            LayoutMeasurement::Size
                        }
                        language_core::data_layout_queries::DataLayoutQueryKind::Alignment => {
                            LayoutMeasurement::Alignment
                        }
                    },
                },
                crate::emission::scalar_types::terminal_scalar_type(
                    typed_trees::types::PrimitiveType::U64,
                )?,
            )
        } else {
            let contexts = checked
                .typed
                .constant_recipes
                .iter()
                .filter(|(_, recipe)| recipe.position.expression() == expression)
                .flat_map(|(_, recipe)| {
                    checked
                        .typed
                        .constant_expression_contexts
                        .span_or_empty(recipe.contexts)
                })
                .map(|context| {
                    format!(
                        "{} (entry {:?})",
                        checked.typed.symbols.name(context.owner),
                        context.evaluation_entry
                    )
                })
                .collect::<Vec<_>>();
            return Err(LoweringError::UnpublishedScalarCondition {
                owner,
                expression,
                reason: format!(
                    "pending constant expression has no retained complete evaluation entry in `{}`; retained contexts: {}",
                    checked.typed.symbols.name(owner),
                    contexts.join(", "),
                ),
            });
        };
        let application = self.insert(definition, scalar_type)?;
        self.expressions.push((expression, owner, application));
        Ok(application)
    }

    fn retain_entry(
        &mut self,
        checked: &CheckedTrees,
        entry: ConstantEvaluationEntry,
    ) -> Result<ScalarApplicationId, LoweringError> {
        let primitive = checked
            .typed
            .primitive_type_reference(entry.result_type)
            .ok_or(LoweringError::Unsupported(
                "constant entry has no exact scalar result carrier",
            ))?;
        self.insert(
            Definition::Evaluation { entry: entry.entry },
            crate::emission::scalar_types::terminal_scalar_type(primitive)?,
        )
    }

    fn insert(
        &mut self,
        definition: Definition,
        scalar_type: ScalarType,
    ) -> Result<ScalarApplicationId, LoweringError> {
        if matches!(definition, Definition::Evaluation { entry } if !entry.is_valid()) {
            return Err(LoweringError::Unsupported(
                "pending scalar application has no exact evaluation entry",
            ));
        }
        if let Some(ordinal) = self
            .rows
            .iter()
            .position(|row| row.definition == definition && row.scalar_type == scalar_type)
        {
            return Self::identity(ordinal);
        }
        let application = Self::identity(self.rows.len())?;
        self.rows.push(Application {
            definition,
            scalar_type,
        });
        Ok(application)
    }

    fn identity(ordinal: usize) -> Result<ScalarApplicationId, LoweringError> {
        u64::try_from(ordinal)
            .ok()
            .and_then(|length| length.checked_add(1))
            .and_then(ScalarApplicationId::new)
            .ok_or(LoweringError::Unsupported(
                "scalar application namespace is exhausted",
            ))
    }

    pub(crate) fn expression(&self, expression: ExpressionHandle) -> Option<ScalarApplicationId> {
        let mut matching = self
            .expressions
            .iter()
            .filter(|(source, _, _)| *source == expression)
            .map(|(_, _, application)| *application);
        let first = matching.next()?;
        matching.all(|other| other == first).then_some(first)
    }

    pub(crate) fn expression_in(
        &self,
        expression: ExpressionHandle,
        owner: symbols::SymbolHandle,
    ) -> Option<ScalarApplicationId> {
        self.expressions
            .iter()
            .find(|(source, scope, _)| *source == expression && *scope == owner)
            .map(|(_, _, application)| *application)
            .or_else(|| {
                self.expressions
                    .iter()
                    .find(|(source, scope, _)| *source == expression && !scope.is_valid())
                    .map(|(_, _, application)| *application)
            })
            .or_else(|| {
                (!owner.is_valid())
                    .then(|| self.expression(expression))
                    .flatten()
            })
    }

    pub(crate) fn recipe(&self, recipe: ConstantRecipeHandle) -> Option<ScalarApplicationId> {
        let mut matching = self
            .recipes
            .iter()
            .filter(|(source, _, _)| *source == recipe)
            .map(|(_, _, application)| *application);
        let first = matching.next()?;
        matching.all(|other| other == first).then_some(first)
    }

    pub(crate) fn recipe_in(
        &self,
        recipe: ConstantRecipeHandle,
        owner: symbols::SymbolHandle,
    ) -> Option<ScalarApplicationId> {
        self.recipes
            .iter()
            .find(|(source, scope, _)| *source == recipe && *scope == owner)
            .map(|(_, _, application)| *application)
            .or_else(|| {
                self.recipes
                    .iter()
                    .find(|(source, scope, _)| *source == recipe && !scope.is_valid())
                    .map(|(_, _, application)| *application)
            })
            .or_else(|| (!owner.is_valid()).then(|| self.recipe(recipe)).flatten())
    }

    pub(crate) fn scalar_type(&self, application: ScalarApplicationId) -> Option<ScalarType> {
        let ordinal = usize::try_from(application.get().checked_sub(1)?).ok()?;
        self.rows.get(ordinal).map(|row| row.scalar_type)
    }

    pub(crate) fn array_extent(
        &self,
        checked: &CheckedTrees,
        extent: typed_trees::constant_recipes::DeclaredArrayExtent,
    ) -> Result<terminal_psi::ArrayExtent, LoweringError> {
        if let Some(length) = extent.closed(&checked.typed) {
            return Ok(terminal_psi::ArrayExtent::Literal(length));
        }
        let typed_trees::constant_recipes::DeclaredArrayExtent::Recipe(recipe) = extent else {
            return Err(LoweringError::Unsupported(
                "literal array extent failed to resolve",
            ));
        };
        let application = self.recipe(recipe).ok_or(LoweringError::Unsupported(
            "fixed array extent has no retained scalar application in this product",
        ))?;
        Ok(terminal_psi::ArrayExtent::Application(application))
    }

    pub(crate) fn evaluation_roots(&self) -> impl Iterator<Item = symbols::SymbolHandle> + '_ {
        self.rows.iter().filter_map(|row| match row.definition {
            Definition::Evaluation { entry } => Some(entry),
            _ => None,
        })
    }

    pub(crate) fn type_roots(&self) -> impl Iterator<Item = TypeIdentityId> + '_ {
        self.rows.iter().filter_map(|row| match row.definition {
            Definition::Layout { subject, .. } => Some(subject),
            _ => None,
        })
    }

    pub(crate) fn publish(
        &self,
        types: &[(TypeIdentityId, StructuralTypeId)],
        machines: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
        boundaries: &[super::operation_frame::BoundaryParameters],
        emitted: conditions::EmittedScalars<'_>,
    ) -> Result<ScalarApplications, LoweringError> {
        let mut result = ScalarApplications::default();
        let scope = proof_scope::publish(
            emitted.checked,
            emitted.identities,
            emitted.sources,
            machines,
            boundaries,
        )?;
        result.proof_scope = scope.scope;
        for row in &self.rows {
            let declaration = match row.definition {
                Definition::Layout {
                    subject,
                    measurement,
                } => ScalarApplication::LayoutObservation {
                    subject: super::lookup_type_id(types, &subject)?,
                    measurement,
                    semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
                    scalar_type: row.scalar_type,
                },
                Definition::Evaluation { entry } => ScalarApplication::Evaluate {
                    entry: super::target_families::callable(entry, machines, boundaries)?,
                    scalar_type: row.scalar_type,
                },
            };
            result
                .insert(declaration)
                .map_err(LoweringError::Unsupported)?;
        }
        result.conditions = conditions::publish(
            &self.conditions,
            machines,
            boundaries,
            &scope.coordinates,
            emitted,
        )?;
        Ok(result)
    }
}

pub(crate) use conditions::EmittedScalars;
