//! Denotation and its pending application travel together until caller facts
//! are installed. Normalizing a rank never proves its intermediate operations.

use super::super::arithmetic_judgment::IntegerApplicationInput;
use super::{BTreeMap, Engine, ExpressionHandle, Polynomial, identity_views};

pub(super) struct RankValue {
    pub value: Polynomial,
    application: Option<DeclaredApplication>,
}

struct DeclaredApplication {
    measure: symbols::SymbolHandle,
    input: IntegerApplicationInput,
}

impl RankValue {
    pub fn scalar(value: Polynomial) -> Self {
        Self {
            value,
            application: None,
        }
    }

    pub fn computed(
        program: &dyn crate::CheckingInput,
        engine: &mut Engine<'_>,
        subject: ExpressionHandle,
        parameter: symbols::SymbolHandle,
        body: ExpressionHandle,
    ) -> Option<Self> {
        let input = engine.normalize(subject)?;
        Self::with_input(program, engine, parameter, body, input.into())
    }

    pub(super) fn with_input(
        program: &dyn crate::CheckingInput,
        engine: &mut Engine<'_>,
        parameter: symbols::SymbolHandle,
        body: ExpressionHandle,
        input: IntegerApplicationInput,
    ) -> Option<Self> {
        let value =
            identity_views::computed_measure_value(program, engine, parameter, body, &input)?;
        Some(Self {
            value,
            application: Some(DeclaredApplication {
                measure: program.symbols.get(parameter).parent,
                input,
            }),
        })
    }

    pub fn after_arrival(&self, substitutions: &BTreeMap<String, Polynomial>) -> Option<Self> {
        let substitute = |value: &Polynomial| {
            super::inductive_judgment::apply_argument_map(value, substitutions)
        };
        Some(Self {
            value: substitute(&self.value)?,
            application: match &self.application {
                Some(application) => Some(DeclaredApplication {
                    measure: application.measure,
                    input: application.input.mapped(substitute)?,
                }),
                None => None,
            },
        })
    }

    pub fn forms(&self, engine: &mut Engine<'_>) -> bool {
        self.formation(engine).is_some()
    }

    pub fn formation(
        &self,
        engine: &mut Engine<'_>,
    ) -> Option<Option<super::super::IntegerApplicationFormation>> {
        let Some(application) = &self.application else {
            return Some(None);
        };
        let formed =
            engine.form_declared_integer_application(application.measure, &application.input)?;
        // Only completed formation can expose the declaration's value. A
        // reservation is not an evaluator and cannot lend facts to its own
        // partial children. Already-produced source invariants remain attached
        // to the reserved observation while this equality gives its meaning.
        engine
            .install_hypotheses(vec![(
                typed_trees::expression::BinaryOperator::Equal,
                self.value.clone(),
                formed.value,
            )])
            .then_some(formed.formation)
    }
}
