//! Descriptor custody and result role of one dynamic call.
//!
//! A dynamic call is one operation, `TargetUnitOperation::CallDynamic`. Its
//! dispatch kind names how the callee is reached and which descriptor words
//! travel; its result role names whether the invoked row defines a scalar.
//! The two axes are independent data, so neither a new dispatch kind nor a
//! new result role adds another call operation.

use crate::{
    TargetDynamicDescriptorArgument, TargetDynamicDescriptorParameterAbi, TargetStructuralArgument,
    TargetUnitScalarHomeRequirement,
};
use abstract_operations::{
    AbstractParameterDynamicDispatch, AbstractReboundDynamicDispatch, AbstractStoredDynamicDispatch,
};
use semantic_vocabulary::MachineId;
use terminal_psi::{ClaimTransfer, TerminalDynamicRequirement};

/// How one dynamic call reaches its callee and which descriptor custody it
/// retains. The stored, rebound and parameter kinds mirror
/// `AbstractDynamicDispatch`; `DescriptorArguments` is the direct call that
/// forwards descriptors. These are different ownership facts: consumers that
/// differ by custody match on the kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetDynamicDispatch {
    /// A direct call to `callee` whose authored parameter roster is one or
    /// more existential descriptors. It stays apart from an ordinary direct
    /// call because each descriptor expands to an ordered `{data, table}`
    /// pointer pair and requires adapter-table custody.
    DescriptorArguments {
        callee: MachineId,
        arguments: Vec<TargetDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
    },
    /// Reload and invoke a descriptor established by the unique preceding
    /// `StoreDynamicDescriptor` with the same target-neutral custody. Only a
    /// scalar result is admitted.
    Stored {
        custody: AbstractStoredDynamicDispatch,
        source_argument: TargetStructuralArgument,
    },
    /// One exact rebound invocation. Both source versions are lowered against
    /// the selected realization ABI, but only `rebound_argument` supplies the
    /// runtime instance. The retained dispatch row is private table content;
    /// later assignment and emission must call through that table.
    Rebound {
        custody: AbstractReboundDynamicDispatch,
        initial_argument: TargetStructuralArgument,
        rebound_argument: TargetStructuralArgument,
    },
    /// One indirect call through a requirement slot of this function's own
    /// borrowed descriptor parameter. `parameter_abi` binds the incoming
    /// `{instance, table}` pair to the graph's call plan, `requirement` is the
    /// closed interface row the call invokes, and `table_slot_byte_offset` is
    /// the byte offset of its entry in the incoming table. The semantic
    /// dispatch row never names a concrete realization.
    Parameter {
        custody: AbstractParameterDynamicDispatch,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        requirement: TerminalDynamicRequirement,
        table_slot_byte_offset: u32,
    },
}

/// Closed result role of one dynamic call. Unit is an authored role rather
/// than an absent scalar: the call plan has no result placement and no home
/// exists. A scalar result retains the durable home later Unit operations
/// read; the home names the defining operation, value, type and shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetDynamicCallResult {
    Unit,
    Scalar(TargetUnitScalarHomeRequirement),
}

impl TargetDynamicCallResult {
    pub const fn scalar_home(&self) -> Option<&TargetUnitScalarHomeRequirement> {
        match self {
            Self::Scalar(home) => Some(home),
            Self::Unit => None,
        }
    }
}
