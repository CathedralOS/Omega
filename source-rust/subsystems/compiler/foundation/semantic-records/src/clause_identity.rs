//! Typed values on either side of a source Identity. Applications refer to
//! the selected declaration and, for a requirement, its supplying record.
use crate::{ScalarTerm, ScalarType, StructuralCaseSubject, StructuralTypeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum ClauseCarrier {
    Parameter(u32),
    Scalar(ScalarType),
    Structural(StructuralTypeId),
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
    pub fn validate(&self) -> Result<(), crate::PropositionError> {
        let invalid = || crate::PropositionError::InvalidClauseIdentity;
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
