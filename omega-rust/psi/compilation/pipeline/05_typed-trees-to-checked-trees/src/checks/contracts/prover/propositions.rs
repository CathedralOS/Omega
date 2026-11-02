//! Substitute ordinary proposition arguments into selected scalar plans. State
//! arguments read live flow observations; alias arguments read their immutable
//! parameter telescope. Both use the shared checked arithmetic evaluator.
use super::{ScalarValue, live_scalar_value};
use typed_trees::proposition::{
    PropositionApplication, PropositionBody, PropositionDefinition, PropositionFormula,
};
use typed_trees::types::PrimitiveType;

struct PropositionProof<'program> {
    program: &'program dyn validation::CheckingInput,
    facts: &'program checked_trees::CheckFacts,
    state: &'program typed_trees::state::State,
    statement: usize,
    contexts: &'program [facts::FactContextHandle],
    active: Vec<symbols::SymbolHandle>,
}
struct PropositionArguments<'program> {
    declaration: &'program PropositionDefinition,
    values: Vec<(symbols::SymbolHandle, Option<ScalarValue>)>,
}
pub(in crate::checks::contracts) fn live_contexts_prove_proposition_application(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state: &typed_trees::state::State,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    application: &PropositionApplication,
) -> bool {
    PropositionProof {
        program,
        facts,
        state,
        statement,
        contexts,
        active: Vec::new(),
    }
    .application(application, None)
        == Some(true)
}
impl<'program> PropositionProof<'program> {
    fn application(
        &mut self,
        application: &PropositionApplication,
        environment: Option<&PropositionArguments<'program>>,
    ) -> Option<bool> {
        if self.active.contains(&application.proposition) {
            return None;
        }
        let declaration = self
            .program
            .proposition_definition_by_symbol(application.proposition)?;
        let PropositionBody::Transparent { proposition } = &declaration.body else {
            return None;
        };
        let parameters = self.program.proposition_parameters(declaration);
        let arguments = self
            .program
            .expression_table
            .expression_handles(application.arguments);
        if parameters.len() != arguments.len() {
            return None;
        }
        let mut bound = PropositionArguments {
            declaration,
            values: Vec::with_capacity(parameters.len()),
        };
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let value = self
                .program
                .primitive_type_reference(parameter.type_reference)
                .and_then(|primitive| match environment {
                    Some(environment) => self.parameter_value(environment, *argument, primitive),
                    None => live_scalar_value(
                        self.program,
                        self.facts,
                        self.state,
                        self.statement,
                        self.contexts,
                        *argument,
                        primitive,
                    ),
                });
            bound.values.push((parameter.symbol, value));
        }
        self.active.push(application.proposition);
        let result = match proposition {
            PropositionFormula::Application(application) => {
                self.application(application, Some(&bound))
            }
            PropositionFormula::BooleanExpression(expression) => {
                match self.parameter_value(&bound, *expression, PrimitiveType::Bool) {
                    Some(ScalarValue::Boolean(value)) => Some(value),
                    _ => None,
                }
            }
        };
        self.active.pop();
        result
    }
    fn parameter_value(
        &self,
        environment: &PropositionArguments<'program>,
        expression: typed_trees::expression::ExpressionHandle,
        primitive: PrimitiveType,
    ) -> Option<ScalarValue> {
        let parameters = self.program.proposition_parameters(environment.declaration);
        let plan = crate::values::lower_parameter_scalar_expression(
            self.program,
            &self.facts.operators,
            environment.declaration.symbol,
            parameters,
            expression,
            primitive,
        )?;
        let symbols = parameters
            .iter()
            .filter(|parameter| {
                crate::values::scalar::occupies_scalar_position(self.program, parameter)
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        crate::values::evaluate_checked_scalar(
            &plan,
            &mut crate::values::BoundScalarValues {
                symbols: &symbols,
                value_at_symbol: |symbol| {
                    environment
                        .values
                        .iter()
                        .find(|(parameter, _)| *parameter == symbol)
                        .and_then(|(_, value)| value.clone())
                },
            },
        )
    }
}
