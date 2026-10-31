//! Denotation and its pending application travel together until caller facts
//! are installed. Normalizing a rank never proves its intermediate operations.

use super::{BTreeMap, Engine, ExpressionHandle, Machine, Polynomial, identity_views};

pub(super) struct RankValue {
    pub value: Polynomial,
    application: Option<DeclaredApplication>,
}

struct DeclaredApplication {
    measure: symbols::SymbolHandle,
    input: Polynomial,
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
        machine: &Machine,
        engine: &mut Engine<'_>,
        subject: ExpressionHandle,
        parameter: symbols::SymbolHandle,
        body: ExpressionHandle,
    ) -> Option<Self> {
        let input = engine.normalize(subject)?;
        let body = identity_views::computed_measure_polynomial(program, machine, parameter, body)?;
        let value = super::inductive_judgment::apply_argument_map(
            &body,
            &BTreeMap::from([(
                identity_views::COMPUTATION_PARAMETER_ATOM.to_owned(),
                input.clone(),
            )]),
        )?;
        Some(Self {
            value,
            application: Some(DeclaredApplication {
                measure: program.symbols.get(parameter).parent,
                input,
            }),
        })
    }

    pub fn after_arrival(&self, substitutions: &BTreeMap<String, Polynomial>) -> Option<Self> {
        let substitute =
            |value| super::inductive_judgment::apply_argument_map(value, substitutions);
        Some(Self {
            value: substitute(&self.value)?,
            application: match &self.application {
                Some(application) => Some(DeclaredApplication {
                    measure: application.measure,
                    input: substitute(&application.input)?,
                }),
                None => None,
            },
        })
    }

    pub fn forms(&self, engine: &mut Engine<'_>) -> bool {
        let Some(application) = &self.application else {
            return true;
        };
        engine
            .form_declared_integer_application(application.measure, &application.input)
            .is_some_and(|formed| engine.substituted(&formed) == engine.substituted(&self.value))
    }
}
