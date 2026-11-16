// Leaf records target operations carry: call origins, float result policies,
// installation calls, runtime-index selectors and atomic storage widths.

use crate::{TargetStructuralArgument, TargetUnitScalarArgumentSource};
use calling_conventions::{CallPlan, ValueShape};
use native_vocabulary::completion_claims::CompletionClaimSource;
use semantic_vocabulary::{
    BoundaryMachineId, ClosedScalarType, IntegerSign, IntegerType, MachineId, ScalarType,
};
use terminal_psi::{AtomicAccessEvent, CompletionReceipt, ProviderCandidateConformance};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCallOrigin {
    Authored,
    ProgramEntry {
        boundary: BoundaryMachineId,
        entry: MachineId,
        source_signature: [u8; 32],
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    InstalledProvider {
        boundary: BoundaryMachineId,
        provider: ProviderCandidateConformance,
        assumption: Option<crate::InstalledAssumedBindingEvidence>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFloatResultPolicy {
    Trapping {
        raw_result: TargetUnitScalarArgumentSource,
    },
    Saturating {
        raw_result: TargetUnitScalarArgumentSource,
        eligible: TargetUnitScalarArgumentSource,
    },
}

impl TargetFloatResultPolicy {
    pub fn operands(self) -> impl Iterator<Item = TargetUnitScalarArgumentSource> {
        match self {
            Self::Trapping { raw_result } => [None, Some(raw_result)],
            Self::Saturating {
                eligible,
                raw_result,
            } => [Some(eligible), Some(raw_result)],
        }
        .into_iter()
        .flatten()
    }

    pub fn semantic_policy(self) -> terminal_psi::FloatResultPolicy {
        match self {
            Self::Trapping { raw_result } => terminal_psi::FloatResultPolicy::Trapping {
                raw_result: raw_result.source_value(),
            },
            Self::Saturating {
                eligible,
                raw_result,
            } => terminal_psi::FloatResultPolicy::Saturating {
                eligible: eligible.source_value(),
                raw_result: raw_result.source_value(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetInstallationCall {
    pub role: crate::InstallationCallRole,
    pub callee: MachineId,
    pub call_plan: CallPlan,
    pub argument: TargetStructuralArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralRuntimeIndex {
    pub operand: TargetUnitScalarArgumentSource,
    pub stride: u32,
    /// A fixed source collection bound, or None for a descriptor-carried bound.
    pub extent: Option<u64>,
    /// Together with the operation owner, identifies the admitted index proof.
    pub obligation: semantic_vocabulary::ObligationId,
}

pub fn atomic_storage_integer_type(
    event: AtomicAccessEvent,
    logical_type: ScalarType,
) -> Option<IntegerType> {
    atomic_storage_integer_type_closed(event, logical_type.try_into().ok()?)
        .map(semantic_vocabulary::ClosedIntegerType::as_checking)
}

pub fn atomic_storage_integer_type_closed(
    event: AtomicAccessEvent,
    logical_type: ClosedScalarType,
) -> Option<semantic_vocabulary::ClosedIntegerType> {
    if !event.ordering_is_legal() {
        return None;
    }
    // Artifact-backed consumers supply their retained closed descriptor;
    // independent metadata callers close once in the wrapper above.
    match logical_type {
        ClosedScalarType::Boolean
            if matches!(
                event,
                AtomicAccessEvent::Load { .. } | AtomicAccessEvent::Store { .. }
            ) =>
        {
            semantic_vocabulary::ClosedIntegerType::new(IntegerSign::Unsigned, 8).ok()
        }
        ClosedScalarType::Integer(integer)
            if !integer.is_address() && ValueShape::for_closed_integer(integer).is_some() =>
        {
            Some(integer)
        }
        _ => None,
    }
}

#[cfg(test)]
mod width_closure_tests {
    use super::*;
    use semantic_vocabulary::{ClosedIntegerType, WidthParameter};
    use semantic_vocabulary::{
        ClosedScalarType, IntegerSign, IntegerType, MachineId, ScalarType, ValueId,
    };
    use terminal_psi::AtomicAccessEvent;
    use terminal_psi::MemoryOrdering;

    #[test]
    fn atomic_metadata_requires_closed_widths_and_keeps_ordering_and_kind() {
        let load = AtomicAccessEvent::Load {
            ordering: MemoryOrdering::Receive,
        };
        let parameter = WidthParameter {
            owner: MachineId::new(1).unwrap(),
            value: ValueId::new(2).unwrap(),
        };
        let symbolic =
            ScalarType::Integer(IntegerType::parameterized(IntegerSign::Unsigned, parameter));
        assert_eq!(atomic_storage_integer_type(load, symbolic), None);
        for bits in [1, 8, 13, 16, 32, 64, 128] {
            let integer = ClosedIntegerType::new(IntegerSign::Signed, bits).unwrap();
            let expected = matches!(bits, 8 | 16 | 32 | 64).then_some(integer);
            assert_eq!(
                atomic_storage_integer_type_closed(load, ClosedScalarType::Integer(integer)),
                expected
            );
            assert_eq!(
                atomic_storage_integer_type(load, ScalarType::Integer(integer.as_checking())),
                expected.map(ClosedIntegerType::as_checking)
            );
        }
        assert_eq!(
            atomic_storage_integer_type_closed(
                load,
                ClosedScalarType::Integer(ClosedIntegerType::address(64).unwrap())
            ),
            None
        );
        assert!(atomic_storage_integer_type(load, ScalarType::Boolean).is_some());
        assert_eq!(
            atomic_storage_integer_type(
                AtomicAccessEvent::Load {
                    ordering: MemoryOrdering::Publish
                },
                ScalarType::Boolean
            ),
            None
        );
        assert_eq!(
            atomic_storage_integer_type(
                AtomicAccessEvent::Swap {
                    ordering: MemoryOrdering::NoOrdering,
                    value: ValueId::FIRST
                },
                ScalarType::Boolean
            ),
            None
        );
    }
}
