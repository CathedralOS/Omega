//! A record argument is its typed, authenticated read roster, never a numeric
//! atom standing for the record. Scalar and record arguments share denotation
//! transport; neither shape carries application formation by itself.

use super::Polynomial;
use typed_trees::{expression::ExpressionHandle, types::TypeReferenceHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::contract_entailment) enum IntegerApplicationInput {
    Scalar(Polynomial),
    Record {
        type_reference: TypeReferenceHandle,
        reads: Vec<(ExpressionHandle, Polynomial)>,
    },
}

impl From<Polynomial> for IntegerApplicationInput {
    fn from(value: Polynomial) -> Self {
        Self::Scalar(value)
    }
}

impl From<&Polynomial> for IntegerApplicationInput {
    fn from(value: &Polynomial) -> Self {
        Self::Scalar(value.clone())
    }
}

impl From<&IntegerApplicationInput> for IntegerApplicationInput {
    fn from(value: &IntegerApplicationInput) -> Self {
        value.clone()
    }
}

impl IntegerApplicationInput {
    /// The flow owner has already authenticated each read and its observation
    /// frontier. Consume the complete occurrence roster without normalizing a
    /// record as an integer or reopening its source in the caller namespace.
    pub(in crate::proof_contracts::contract_entailment) fn captured(
        engine: &super::Engine<'_>,
        measure: symbols::SymbolHandle,
        input_type: TypeReferenceHandle,
    ) -> Option<Self> {
        let declaration = engine
            .program
            .measures()
            .iter()
            .find(|declaration| declaration.symbol == measure)?;
        let shape = super::super::ranking_range::declared_input_shape(engine.program, declaration)?;
        if !super::super::ranking_range::declared_input_type_matches(
            engine.program,
            input_type,
            shape.type_reference,
        ) {
            return None;
        }
        if engine
            .program
            .primitive_type_reference(shape.type_reference)
            .is_some()
        {
            return Some(Self::Scalar(engine.strict_symbol_value(shape.parameter)?));
        }
        let reads = shape
            .reads
            .iter()
            .flat_map(|read| &read.occurrences)
            .map(|source| Some((*source, engine.strict_occurrence_value(*source)?)))
            .collect::<Option<Vec<_>>>()?;
        Some(Self::Record {
            type_reference: input_type,
            reads,
        })
    }

    pub(super) fn visit_values(&self, mut visit: impl FnMut(&Polynomial)) {
        match self {
            Self::Scalar(value) => visit(value),
            Self::Record { reads, .. } => reads.iter().for_each(|(_, value)| visit(value)),
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn mapped(
        &self,
        mut map: impl FnMut(&Polynomial) -> Option<Polynomial>,
    ) -> Option<Self> {
        Some(match self {
            Self::Scalar(value) => Self::Scalar(map(value)?),
            Self::Record {
                type_reference,
                reads,
            } => Self::Record {
                type_reference: *type_reference,
                reads: reads
                    .iter()
                    .map(|(source, value)| Some((*source, map(value)?)))
                    .collect::<Option<Vec<_>>>()?,
            },
        })
    }
}
