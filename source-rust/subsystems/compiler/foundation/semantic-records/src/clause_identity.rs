//! Typed values on either side of a source Identity. Applications refer to
//! the selected declaration and, for a requirement, its supplying record.
use crate::{
    FloatMeaningClauseValue, IeeeFloatFormat, ScalarTerm, ScalarType, StructuralCaseSubject,
    StructuralTypeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum ClauseCarrier {
    Parameter(u32),
    Scalar(ScalarType),
    Structural(StructuralTypeId),
    /// The meaning of an IEEE value of this format, kept distinct from raw
    /// representation identity and from IEEE comparison.
    FloatMeaning(IeeeFloatFormat),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct ClauseDeclaration {
    /// Slot and generation in the elaboration's checked declaration arena.
    pub supplier: Option<(u32, u32)>,
    pub member: (u32, u32),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum ClauseOperand {
    Scalar(ScalarTerm),
    Input(StructuralCaseSubject),
    Formal(u32),
    Application {
        declaration: ClauseDeclaration,
        arguments: Vec<u32>,
    },
    /// A meaning projection of a float, or a float-semantics catalog
    /// application whose meaning operands name earlier values.
    FloatMeaning(FloatMeaningClauseValue),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct ClauseValue {
    pub carrier: ClauseCarrier,
    pub operand: ClauseOperand,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct ClauseIdentity {
    pub values: Vec<ClauseValue>,
    pub left: u32,
    pub right: u32,
}

impl ClauseIdentity {
    /// Whether a Psi module may publish this Identity: a whole input, or a
    /// field of one, at its data type, a scalar term at its scalar type, and a
    /// float meaning.
    /// A type parameter, a schema formal and an applied declaration name
    /// something only the source checker's signature defines, so they stay
    /// inside the compiler.
    pub fn is_portable(&self) -> bool {
        self.values.iter().all(|value| {
            matches!(
                (value.carrier, &value.operand),
                (ClauseCarrier::Structural(_), ClauseOperand::Input(_))
                    | (ClauseCarrier::Scalar(_), ClauseOperand::Scalar(_))
                    | (
                        ClauseCarrier::FloatMeaning(_),
                        ClauseOperand::FloatMeaning(_)
                    )
            )
        })
    }

    /// Whether the two sides are identical, for an Identity whose sides are
    /// both float literals at one raw IEEE carrier. Raw float identity is
    /// equality of the exact same-format representation: one NaN encoding is
    /// identical to itself, NaNs with different payloads or signs are not, and
    /// neither are positive and negative zero (wiki/spec/terminal-psi/mathematical_values.md,
    /// "Float equality and source identity": "Raw float identity compares
    /// exact same-format representations, preserving all bits observable by
    /// a legal bit operation").
    pub fn raw_float_literal_identity(&self) -> Option<bool> {
        let literal = |index: u32| match self.values.get(index as usize)? {
            ClauseValue {
                carrier: ClauseCarrier::Scalar(ScalarType::IeeeFloat(format)),
                operand: ClauseOperand::Scalar(ScalarTerm::IeeeFloatLiteral(value)),
            } if value.format() == *format => Some(*value),
            _ => None,
        };
        let (left, right) = (literal(self.left)?, literal(self.right)?);
        (left.format() == right.format()).then_some(left == right)
    }

    pub fn validate(&self) -> Result<(), crate::PropositionError> {
        let invalid = || crate::PropositionError::InvalidClauseIdentity;
        // A FloatMeaning Identity is entirely float meanings, under their own
        // catalog, ordering and connectivity rules.
        if self.is_float_meaning() {
            return self
                .validate_float_meaning()
                .map_err(crate::PropositionError::InvalidFloatMeaningClause);
        }
        if self.values.iter().any(|value| {
            matches!(value.carrier, ClauseCarrier::FloatMeaning(_))
                || matches!(value.operand, ClauseOperand::FloatMeaning(_))
        }) {
            return Err(invalid());
        }
        let left = self.values.get(self.left as usize).ok_or_else(invalid)?;
        let right = self.values.get(self.right as usize).ok_or_else(invalid)?;
        if left.carrier != right.carrier {
            return Err(invalid());
        }
        for (position, value) in self.values.iter().enumerate() {
            match &value.operand {
                ClauseOperand::Scalar(term) => {
                    term.validate()?;
                    if value.carrier != ClauseCarrier::Scalar(term.scalar_type()) {
                        return Err(invalid());
                    }
                }
                ClauseOperand::Application { arguments, .. }
                    if arguments.iter().any(|&arg| arg as usize >= position) =>
                {
                    return Err(invalid());
                }
                _ => {}
            }
        }
        Ok(())
    }
}
