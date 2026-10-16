//! Observable register and processor-control events with exact operand roles.

use language_core::inline_assembly::{AsmRegisterOperation, AsmSystemRegister};
use semantic_vocabulary::ValueId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAccessEvent<Operand = ValueId> {
    ProcessorHalt,
    Read {
        register: AsmSystemRegister,
    },
    Write {
        register: AsmSystemRegister,
        value: Operand,
    },
    PortRead {
        port: Operand,
    },
    PortWrite {
        port: Operand,
        value: Operand,
    },
    ModelSpecificRead {
        index: Operand,
    },
    ModelSpecificWrite {
        index: Operand,
        value: Operand,
    },
}

impl<Operand> RegisterAccessEvent<Operand> {
    pub const fn operation(&self) -> AsmRegisterOperation {
        match self {
            Self::ProcessorHalt => AsmRegisterOperation::ProcessorHalt,
            Self::Read { register } => AsmRegisterOperation::SystemRead(*register),
            Self::Write { register, .. } => AsmRegisterOperation::SystemWrite(*register),
            Self::PortRead { .. } => AsmRegisterOperation::PortRead,
            Self::PortWrite { .. } => AsmRegisterOperation::PortWrite,
            Self::ModelSpecificRead { .. } => AsmRegisterOperation::ModelSpecificRead,
            Self::ModelSpecificWrite { .. } => AsmRegisterOperation::ModelSpecificWrite,
        }
    }

    pub fn inputs(&self) -> impl Iterator<Item = &Operand> {
        match self {
            Self::Read { .. } | Self::ProcessorHalt => [None, None],
            Self::Write { value, .. } => [Some(value), None],
            Self::PortRead { port } => [Some(port), None],
            Self::PortWrite { port, value } => [Some(port), Some(value)],
            Self::ModelSpecificRead { index } => [Some(index), None],
            Self::ModelSpecificWrite { index, value } => [Some(index), Some(value)],
        }
        .into_iter()
        .flatten()
    }

    pub fn try_map_inputs<Other, Error>(
        &self,
        mut map: impl FnMut(&Operand) -> Result<Other, Error>,
    ) -> Result<RegisterAccessEvent<Other>, Error> {
        Ok(match self {
            Self::ProcessorHalt => RegisterAccessEvent::ProcessorHalt,
            Self::Read { register } => RegisterAccessEvent::Read {
                register: *register,
            },
            Self::Write { register, value } => RegisterAccessEvent::Write {
                register: *register,
                value: map(value)?,
            },
            Self::PortRead { port } => RegisterAccessEvent::PortRead { port: map(port)? },
            Self::PortWrite { port, value } => RegisterAccessEvent::PortWrite {
                port: map(port)?,
                value: map(value)?,
            },
            Self::ModelSpecificRead { index } => {
                RegisterAccessEvent::ModelSpecificRead { index: map(index)? }
            }
            Self::ModelSpecificWrite { index, value } => RegisterAccessEvent::ModelSpecificWrite {
                index: map(index)?,
                value: map(value)?,
            },
        })
    }
}

impl<Operand: Copy> RegisterAccessEvent<Operand> {
    pub fn map_operands(&mut self, mut map: impl FnMut(Operand) -> Operand) {
        *self = self
            .try_map_inputs::<Operand, std::convert::Infallible>(|value| Ok(map(*value)))
            .unwrap();
    }
}
