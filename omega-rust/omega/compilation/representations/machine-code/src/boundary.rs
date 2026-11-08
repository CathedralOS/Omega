/*
Boundary execution roles, completion custody, privileged port effects, and settlement records.

We record the physical custody of realized boundary settlements (`BoundarySettlementRecord`),
their admitted provider or compiler-builtin execution roles (`BoundaryExecutionRecord`),
non-authoritative completion-receipt joins (`CompletionProviderCustodyBinding`), and privileged
I/O port effects (`PortEffectRecord`). Object and installation verifiers rederive completion
custody from the enclosing settlement rather than trusting serialized report coordinates.
*/

use crate::ForeignCallScalarArgumentRecord;
use calling_conventions::{ConventionalSumLayout, ValuePlacement};
use semantic_vocabulary::{BoundaryMachineId, EdgeId, OperationId, ScalarType, ServiceId, ValueId};
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundaryScalarArgument,
    CompilerBuiltinExecution, CompletionClaimSource, ProviderExecutionBinding,
};
use terminal_psi::{
    CompletionReceipt, StructuralArgument, StructuralOperationResult, StructuralTypeDeclaration,
};

pub mod execution {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CompletionProviderCustodyBinding {
        pub source: CompletionClaimSource,
        pub receipt: CompletionReceipt,
        pub provider_execution: ProviderExecutionRecord,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BoundaryExecutionRecord {
        AdmittedProvider(ProviderExecutionRecord),
        CompilerBuiltin(CompilerBuiltinExecution),
    }

    impl From<BoundaryExecutionBinding> for BoundaryExecutionRecord {
        fn from(binding: BoundaryExecutionBinding) -> Self {
            match binding {
                BoundaryExecutionBinding::AdmittedProvider(execution) => {
                    Self::AdmittedProvider(execution.into())
                }
                BoundaryExecutionBinding::CompilerBuiltin(execution) => {
                    Self::CompilerBuiltin(execution)
                }
            }
        }
    }

    pub fn derive_completion_provider_custody(
        execution: BoundaryExecutionRecord,
        sources: &[CompletionClaimSource],
        receipts: &[CompletionReceipt],
    ) -> Option<Vec<CompletionProviderCustodyBinding>> {
        let BoundaryExecutionRecord::AdmittedProvider(provider_execution) = execution else {
            return (sources.is_empty() && receipts.is_empty()).then(Vec::new);
        };
        receipts
            .iter()
            .map(|receipt| {
                let source = sources
                    .iter()
                    .find(|source| source.claim() == receipt.claim)?;
                Some(CompletionProviderCustodyBinding {
                    source: source.clone(),
                    receipt: receipt.clone(),
                    provider_execution,
                })
            })
            .collect()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ProviderExecutionRecord {
        pub provider_plan_report_identity: u64,
        pub provider_execution_report_identity: u64,
        pub provider_execution_report_fingerprint: u64,
        pub normalized_root_report_identity: u64,
        pub boundary_contract_report_fingerprint: u64,
    }

    impl ProviderExecutionRecord {
        pub fn new(
            provider_plan_report_identity: u64,
            provider_execution_report_identity: u64,
            provider_execution_report_fingerprint: u64,
            normalized_root_report_identity: u64,
            boundary_contract_report_fingerprint: u64,
        ) -> Option<Self> {
            [
                provider_plan_report_identity,
                provider_execution_report_identity,
                provider_execution_report_fingerprint,
                normalized_root_report_identity,
                boundary_contract_report_fingerprint,
            ]
            .iter()
            .all(|identity| *identity != 0)
            .then_some(Self {
                provider_plan_report_identity,
                provider_execution_report_identity,
                provider_execution_report_fingerprint,
                normalized_root_report_identity,
                boundary_contract_report_fingerprint,
            })
        }
    }

    impl From<ProviderExecutionBinding> for ProviderExecutionRecord {
        fn from(binding: ProviderExecutionBinding) -> Self {
            Self {
                provider_plan_report_identity: binding.provider_plan_report_identity().get(),
                provider_execution_report_identity: binding.provider_execution_report_identity(),
                provider_execution_report_fingerprint: binding
                    .provider_execution_report_fingerprint(),
                normalized_root_report_identity: binding.normalized_root_report_identity(),
                boundary_contract_report_fingerprint: binding
                    .boundary_contract_report_fingerprint(),
            }
        }
    }
}

pub mod ports {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PortEffectRecord {
        pub psi_operation: OperationId,
        pub service: ServiceId,
        pub port: u16,
        pub value: u8,
        pub operation_ordinal: usize,
        pub code_offset: usize,
        pub byte_count: usize,
    }
}

pub mod settlements {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BoundaryScalarResultRecord {
        pub value: ValueId,
        pub scalar_type: ScalarType,
        pub placement: ValuePlacement,
        pub return_edge: EdgeId, // Exact terminal edge that returns this boundary-produced value.
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BoundaryStructuralResultRecord {
        pub defining_operation: OperationId,
        pub result: StructuralOperationResult,
        pub declaration: StructuralTypeDeclaration, // Retained for independent realization replay.
        pub layout: ConventionalSumLayout,
        pub home_byte_offset: u32, // Offset within the complete allocated Unit frame.
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum BoundaryResultRecord {
        Unit,
        Scalar(BoundaryScalarResultRecord),
        Structural(BoundaryStructuralResultRecord),
    }

    impl BoundaryResultRecord {
        pub const fn is_unit(&self) -> bool {
            matches!(self, Self::Unit)
        }

        pub const fn scalar(&self) -> Option<&BoundaryScalarResultRecord> {
            match self {
                Self::Scalar(result) => Some(result),
                Self::Unit | Self::Structural(_) => None,
            }
        }

        optimization_core::structural_variant_accessor!(BoundaryStructuralResultRecord; Self::Unit | Self::Scalar(_));
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BoundarySettlementRecord {
        pub invocation_site: semantic_vocabulary::InvocationSite,
        pub boundary: BoundaryMachineId,
        pub execution: BoundaryExecutionRecord,
        pub realization: BoundaryRealization,
        pub scalar_arguments: Vec<BoundaryScalarArgument>,
        pub runtime_scalar_arguments: Vec<ForeignCallScalarArgumentRecord>,
        pub arguments: Vec<StructuralArgument>,
        pub byte_sequence_arguments: Vec<BoundaryByteSequenceArgumentRecord>,
        pub completion_claim_sources: Vec<CompletionClaimSource>,
        pub completion_receipts: Vec<CompletionReceipt>,
        pub completion_provider_custody: Vec<CompletionProviderCustodyBinding>,
        pub native_result: BoundaryResultRecord,
        pub operation_ordinal: usize,
        pub code_offset: usize,
        pub byte_count: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BoundaryByteSequenceArgumentRecord {
        pub argument: StructuralArgument,
        pub literal_operation: OperationId,
        pub structural_type: StructuralTypeDeclaration,
        pub bytes: Vec<u8>,
        pub code_offset: usize,
        pub code_byte_count: usize,
        pub data_offset: usize,
        pub data_byte_count: usize,
    }
}

pub use execution::{
    BoundaryExecutionRecord, CompletionProviderCustodyBinding, ProviderExecutionRecord,
    derive_completion_provider_custody,
};
pub use ports::PortEffectRecord;
pub use settlements::{
    BoundaryByteSequenceArgumentRecord, BoundaryResultRecord, BoundaryScalarResultRecord,
    BoundarySettlementRecord, BoundaryStructuralResultRecord,
};
