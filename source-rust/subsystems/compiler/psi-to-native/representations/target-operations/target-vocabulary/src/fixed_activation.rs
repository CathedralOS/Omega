//! Native field assignments for an authenticated core activation call.
use language_core::sealed_intrinsics::ActivationOperation;
use semantic_vocabulary::{BoundaryMachineId, MachineId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedField {
    pub offset: u32,
    pub bytes: u32,
}

/// A case tag is an ordinal in this checked sum. Each field has its actual
/// storage position, including the overlaid case payload offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedCase<const N: usize> {
    pub tag: u32,
    pub fields: [FixedField; N],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedActivationCall {
    pub boundary: BoundaryMachineId,
    pub action: FixedActivationAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedActivationAction {
    Prepare {
        storage: terminal_psi::TerminalActivationStorage,
        worker: MachineId,
        tag: FixedField,
        /// Owner.
        ready: FixedCase<1>,
        /// Original arguments and backing loan.
        rejected: FixedCase<2>,
    },
    Step {
        worker: MachineId,
        tag: FixedField,
        /// Returned value and complete original backing loan.
        returned: FixedCase<2>,
        /// Suspended owner.
        parked: FixedCase<1>,
    },
    Park {
        anchor: FixedField,
        reply_type: terminal_psi::TerminalIdentityIndex,
        reply_bytes: u32,
    },
    Resume {
        worker: MachineId,
        anchor: FixedField,
        reply_type: terminal_psi::TerminalIdentityIndex,
        reply_bytes: u32,
        tag: FixedField,
        /// Ready owner.
        ready: FixedCase<1>,
        /// The unaffected parked owner, permit and proposed value.
        mismatch: FixedCase<3>,
    },
}

impl FixedActivationAction {
    pub const fn prepares_split(self) -> bool {
        matches!(
            self,
            Self::Prepare {
                storage: terminal_psi::TerminalActivationStorage::Split,
                ..
            }
        )
    }

    pub const fn operation(self) -> ActivationOperation {
        match self {
            Self::Prepare { .. } => ActivationOperation::Prepare,
            Self::Step { .. } => ActivationOperation::StepFixed,
            Self::Park { .. } => ActivationOperation::Park,
            Self::Resume { .. } => ActivationOperation::Resume,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedActivationBinding {
    pub boundary: BoundaryMachineId,
    pub operation: ActivationOperation,
}

impl FixedActivationBinding {
    pub fn encode(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.boundary.get().to_le_bytes());
        bytes.push(match self.operation {
            ActivationOperation::Prepare => 0,
            ActivationOperation::StepFixed => 1,
            ActivationOperation::Park => 2,
            ActivationOperation::Resume => 3,
        });
    }
}

impl FixedActivationCall {
    pub const fn binding(self) -> FixedActivationBinding {
        FixedActivationBinding {
            boundary: self.boundary,
            operation: self.action.operation(),
        }
    }

    pub fn encode(&self, bytes: &mut Vec<u8>) {
        use FixedActivationAction as Action;
        fn field(bytes: &mut Vec<u8>, field: FixedField) {
            bytes.extend_from_slice(&field.offset.to_le_bytes());
            bytes.extend_from_slice(&field.bytes.to_le_bytes());
        }
        fn case<const N: usize>(bytes: &mut Vec<u8>, case: FixedCase<N>) {
            bytes.extend_from_slice(&case.tag.to_le_bytes());
            for member in case.fields {
                field(bytes, member);
            }
        }
        bytes.extend_from_slice(&self.boundary.get().to_le_bytes());
        match self.action {
            Action::Prepare {
                storage,
                worker,
                tag,
                ready,
                rejected,
            } => {
                bytes.push(0);
                bytes.push(match storage {
                    terminal_psi::TerminalActivationStorage::Fixed => 0,
                    terminal_psi::TerminalActivationStorage::Split => 1,
                });
                bytes.extend_from_slice(&worker.get().to_le_bytes());
                field(bytes, tag);
                case(bytes, ready);
                case(bytes, rejected);
            }
            Action::Step {
                worker,
                tag,
                returned,
                parked,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&worker.get().to_le_bytes());
                field(bytes, tag);
                case(bytes, returned);
                case(bytes, parked);
            }
            Action::Park {
                anchor,
                reply_type,
                reply_bytes,
            } => {
                bytes.push(2);
                field(bytes, anchor);
                bytes.extend_from_slice(&reply_type.0.to_le_bytes());
                bytes.extend_from_slice(&reply_bytes.to_le_bytes());
            }
            Action::Resume {
                worker,
                anchor,
                reply_type,
                reply_bytes,
                tag,
                ready,
                mismatch,
            } => {
                bytes.push(3);
                bytes.extend_from_slice(&worker.get().to_le_bytes());
                field(bytes, anchor);
                bytes.extend_from_slice(&reply_type.0.to_le_bytes());
                bytes.extend_from_slice(&reply_bytes.to_le_bytes());
                field(bytes, tag);
                case(bytes, ready);
                case(bytes, mismatch);
            }
        }
    }
}

/// Scratch homes for the public SysV wrapper. Every source operand is saved
/// before any private helper or parallel result transfer reuses its register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedCallFrame {
    pub inputs: Vec<(u32, u32)>,
    pub result: (u32, u32),
    pub frame_bytes: u32,
}
impl FixedCallFrame {
    pub const OUTPUT_POINTER: u32 = 0;
    pub const INPUT_POINTER: u32 = 8;
    pub const BACKING: u32 = 16; // original mutable byte view, two words

    pub fn for_binding(binding: &crate::NormalizedForeignCallBinding) -> Option<Self> {
        binding.fixed_activation()?;
        let call = &binding.boundary_entry_plan.call;
        if call.policy != calling_conventions::CallingPolicy::SystemVAMD64
            || call.stack_alignment != 16
            || call.shadow_bytes != 0
            || call.entry_control != calling_conventions::EntryControl::CallReturn
            || !call.callback_materializations.is_empty()
        {
            return None;
        }
        let mut cursor = 32u32;
        let mut home = |shape: calling_conventions::ValueShape| {
            let width = match shape.class {
                calling_conventions::ValueClass::BorrowedReference
                | calling_conventions::ValueClass::BorrowedScalarReference => 8,
                _ => shape.byte_size,
            };
            let start = cursor.checked_next_multiple_of(16)?;
            cursor = start.checked_add(width)?;
            Some((start, width))
        };
        let inputs = call
            .parameters
            .iter()
            .map(|p| home(p.shape))
            .collect::<Option<Vec<_>>>()?;
        let result = match &call.result {
            Some(r) => home(r.shape)?,
            None => (cursor, 0),
        };
        let frame_bytes = cursor.checked_next_multiple_of(16)?.checked_add(8)?;
        (frame_bytes <= i32::MAX as u32 - 16).then_some(Self {
            inputs,
            result,
            frame_bytes,
        })
    }

    pub fn caller_stack_bytes(&self) -> u64 {
        // The ordinary call-site measurement already includes entry linkage.
        // The helper call adds one return address below the wrapper frame.
        u64::from(self.frame_bytes) + 8
    }
}
impl crate::NormalizedForeignCallBinding {
    pub fn fixed_activation_stack_bytes(&self) -> Option<u64> {
        Some(FixedCallFrame::for_binding(self)?.caller_stack_bytes())
    }
}
