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
use typed_trees::constant_recipes::{ConstantRecipeHandle, RetainedConstantPosition};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::type_identity::TypeIdentityId;

mod conditions;

/// Invocation-local construction storage, shared by all bodies in one closure.
#[derive(Default)]
pub(crate) struct SourceScalarApplications {
    rows: Vec<Application>,
    expressions: Vec<(ExpressionHandle, ScalarApplicationId)>,
    recipes: Vec<(ConstantRecipeHandle, ScalarApplicationId)>,
    conditions: Vec<conditions::Range>,
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
            let application = match declaration.position {
                RetainedConstantPosition::ArrayLength(reference) => {
                    applications.retain_array_length(checked, reference)?
                }
                RetainedConstantPosition::RangeEndpoint(expression)
                | RetainedConstantPosition::ContractExpression(expression)
                | RetainedConstantPosition::ConstApplication {
                    root: expression, ..
                } => applications.retain_expression(checked, expression)?,
                RetainedConstantPosition::None => {
                    return Err(LoweringError::Unsupported(
                        "pending constant catalog contains an absent source position",
                    ));
                }
            };
            applications.recipes.push((recipe, application));
        }
        for (_, computation) in checked.facts.values.scalar_computations.nodes.iter() {
            if let CheckedScalarComputationKind::RetainedConstant { expression } = computation.kind
                && references.expressions.contains(&expression)
                && checked.landed_integer_constant(expression).is_none()
            {
                applications.retain_expression(checked, expression)?;
            }
        }
        applications.conditions = conditions::prepare(checked, &mut applications)?;
        Ok(applications)
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
        if let Some(application) = self.expression(expression) {
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
                .filter(|observation| observation.expression == expression);
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
            let declarations = checked.typed.const_declarations();
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.materialized_initializer == expression)
                .ok_or(LoweringError::Unsupported(
                    "pending constant expression has no retained complete evaluation entry",
                ))?;
            if !checked
                .typed
                .tables
                .machines
                .is_valid(declaration.evaluation_entry)
            {
                return Err(LoweringError::Unsupported(
                    "pending constant declaration lost its complete evaluation entry",
                ));
            }
            let primitive = checked
                .typed
                .primitive_type_reference(declaration.declared_type)
                .ok_or(LoweringError::Unsupported(
                    "pending constant entry has no concrete scalar result carrier",
                ))?;
            (
                Definition::Evaluation {
                    entry: checked
                        .typed
                        .tables
                        .machines
                        .get(declaration.evaluation_entry)
                        .symbol,
                },
                crate::emission::scalar_types::terminal_scalar_type(primitive)?,
            )
        };
        let application = self.insert(definition, scalar_type)?;
        self.expressions.push((expression, application));
        Ok(application)
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
        self.expressions
            .iter()
            .find(|(source, _)| *source == expression)
            .map(|(_, application)| *application)
    }

    pub(crate) fn recipe(&self, recipe: ConstantRecipeHandle) -> Option<ScalarApplicationId> {
        self.recipes
            .iter()
            .find(|(source, _)| *source == recipe)
            .map(|(_, application)| *application)
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
        self.rows
            .iter()
            .filter_map(|row| match row.definition {
                Definition::Layout { subject, .. } => Some(subject),
                _ => None,
            })
            .chain(
                self.conditions
                    .iter()
                    .filter_map(conditions::Range::type_root),
            )
    }

    pub(crate) fn publish(
        &self,
        types: &[(TypeIdentityId, StructuralTypeId)],
        machines: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
        boundaries: &[super::operation_frame::BoundaryParameters],
        emitted: conditions::EmittedScalars<'_>,
    ) -> Result<ScalarApplications, LoweringError> {
        let mut result = ScalarApplications::default();
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
        result.conditions =
            conditions::publish(&self.conditions, types, machines, boundaries, emitted)?;
        Ok(result)
    }
}

pub(crate) use conditions::EmittedScalars;
