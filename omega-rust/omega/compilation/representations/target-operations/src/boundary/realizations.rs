// Closed target boundary realizations and normalized foreign/system-call bindings.
// `NormalizedForeignCallBinding` pairs the evaluated `BoundaryEntryPlan` with mechanism-specific custody (`Import`, `Component`, `SystemCall`, or `Table`), keeping dispatch-only table pointers out of the wire ABI plan while exposing `operand_placements` in source transport order.

use crate::{TargetStructuralArgument, TargetUnitScalarCallArgument};
use calling_conventions::BoundaryEntryPlan;
use semantic_vocabulary::{OperationId, ServiceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataOnlyPortRealization {
    pub effect_operation: OperationId,
    pub service: ServiceId,
    pub port: u16,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectPortReadU8Realization {
    pub service: ServiceId,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostedExitProcessI32Realization;

impl HostedExitProcessI32Realization {
    pub fn supports_target(target: target::NativeTarget) -> bool {
        target::HostedIntrinsicBundle::from_native_target(target).is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostedReadByteRealization;

impl HostedReadByteRealization {
    pub fn supports_target(target: target::NativeTarget) -> bool {
        target::HostedIntrinsicBundle::from_native_target(target).is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostedWriteByteI32Realization;

impl HostedWriteByteI32Realization {
    pub fn supports_target(target: target::NativeTarget) -> bool {
        target::HostedIntrinsicBundle::from_native_target(target).is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NearestIeeeFloatArithmeticRealization {
    pub operation: crate::IeeeFloatArithmeticOperation,
    pub format: semantic_vocabulary::IeeeFloatFormat,
    pub x86_scalar_fma: Option<target::AdmittedX86ScalarFmaProvider>, // Build-admitted AVX+FMA3 deployment custody on x86_64.
}

impl NearestIeeeFloatArithmeticRealization {
    pub fn supports_target(&self, target: target::NativeTarget) -> bool {
        let fused = self.operation == crate::IeeeFloatArithmeticOperation::FusedMultiplyAdd;
        match (target.architecture, self.x86_scalar_fma) {
            (target::Architecture::Aarch64, None) => true,
            (target::Architecture::X86_64, None) => !fused,
            (target::Architecture::X86_64, Some(provider)) => {
                fused && provider.profile().native_target() == target
            }
            (target::Architecture::Aarch64, Some(_)) => false,
        }
    }

    pub fn signature_matches(
        &self,
        parameters: &[semantic_vocabulary::ScalarType],
        result: semantic_vocabulary::ScalarType,
    ) -> bool {
        self.operation
            .signature_matches(self.format, parameters, result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxWriteLineRealization;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClaimCompletionOnlyRealization;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedForeignCallBinding {
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub mechanism: NormalizedForeignCallMechanism,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedForeignCallMechanism {
    /// Hosted calls use the receiving operating system stack, not a finite
    /// claim about the opaque callee's demand.
    HostedImport {
        locator: target::NormalizedForeignLocator,
        receipt: [u8; 32],
        requirement_identity: String,
    },
    Import {
        locator: target::NormalizedForeignLocator,
        same_stack_contribution: task_plans::AdmittedSameStackContribution,
    },
    Component {
        locator: target::NormalizedComponentLocator,
        provider: terminal_psi::TerminalPsiIdentity,
        candidate: semantic_vocabulary::MachineId,
        application_commitment: [u8; 32],
        same_stack_contribution: task_plans::AdmittedSameStackContribution,
    },
    SystemCall(NormalizedSystemCallBinding),
    Table {
        target: target::TargetProfile,
        byte_offset: u32,
        dispatch_only: bool,
        same_stack_contribution: task_plans::AdmittedSameStackContribution,
    },
}

impl NormalizedForeignCallMechanism {
    fn same_stack_contribution(&self) -> Option<&task_plans::AdmittedSameStackContribution> {
        match self {
            Self::Import {
                same_stack_contribution,
                ..
            }
            | Self::Component {
                same_stack_contribution,
                ..
            }
            | Self::Table {
                same_stack_contribution,
                ..
            } => Some(same_stack_contribution),
            Self::SystemCall(_) | Self::HostedImport { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSystemCallBinding {
    pub target: target::TargetProfile,
    pub number: i64,
    pub requirement_identity: String,
    pub execution: NormalizedForeignExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedForeignExecution {
    ProviderPlan { report: u64, commitment: [u8; 32] },
    NativeBinding([u8; 32]),
}

impl NormalizedForeignExecution {
    pub fn is_complete(self) -> bool {
        match self {
            Self::ProviderPlan { report, commitment } => report != 0 && commitment != [0; 32],
            Self::NativeBinding(receipt) => receipt != [0; 32],
        }
    }
    pub const fn provider_report(self) -> Option<u64> {
        match self {
            Self::ProviderPlan { report, .. } => Some(report),
            Self::NativeBinding(_) => None,
        }
    }
}

impl NormalizedForeignCallBinding {
    /// Match the retained execution source. Admission separately authenticates
    /// its full receipt, calling application and declared contract.
    pub fn native_binding_receipt(&self) -> Option<[u8; 32]> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::HostedImport { receipt, .. } => Some(*receipt),
            NormalizedForeignCallMechanism::SystemCall(binding) => match binding.execution {
                NormalizedForeignExecution::NativeBinding(receipt) => Some(receipt),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn matches_execution(&self, execution: super::ExternalExecutionBinding) -> bool {
        match execution {
            super::ExternalExecutionBinding::Provider(provider) => match self.mechanism.same_stack_contribution() {
                Some(contribution) => contribution.provider_plan_report_identity() == provider.provider_plan_report_identity().get(),
                None => self.system_call().is_some_and(|binding| matches!(binding.execution,
                    NormalizedForeignExecution::ProviderPlan { report, .. } if report == provider.provider_plan_report_identity().get())),
            },
            super::ExternalExecutionBinding::NativeBinding(native) => self.native_binding_receipt() == Some(native.receipt()),
        }
    }

    pub fn operand_placements(&self) -> ForeignCallOperands<'_> {
        use calling_conventions::{MachineRegister, ValueLocation, ValuePlacement, ValueShape};
        let dispatch = match &self.mechanism {
            NormalizedForeignCallMechanism::Table {
                target,
                dispatch_only: true,
                ..
            } => {
                let register = match target.native_target().architecture {
                    target::Architecture::X86_64 => MachineRegister::X86R11,
                    target::Architecture::Aarch64 => MachineRegister::Aarch64X(16),
                };
                Some(ValuePlacement {
                    shape: ValueShape::integer(8, 8),
                    locations: vec![ValueLocation::Register {
                        register,
                        byte_size: 8,
                        value_byte_offset: 0,
                    }],
                })
            }
            _ => None,
        };
        ForeignCallOperands {
            dispatch,
            parameters: &self.boundary_entry_plan.call.parameters,
        }
    }

    pub fn dispatch_operand_count(&self) -> usize {
        usize::from(matches!(self.table_dispatch(), Some((_, true))))
    }

    pub fn native_target(&self) -> Option<target::NativeTarget> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import { locator, .. }
            | NormalizedForeignCallMechanism::HostedImport { locator, .. } => {
                Some(locator.target().native_target())
            }
            NormalizedForeignCallMechanism::Component { locator, .. } => {
                Some(locator.target().native_target())
            }
            NormalizedForeignCallMechanism::Table { target, .. } => Some(target.native_target()),
            NormalizedForeignCallMechanism::SystemCall(binding) => match binding.target {
                target::TargetProfile::LinuxX64 | target::TargetProfile::LinuxArm64 => {
                    Some(binding.target.native_target())
                }
                _ => None,
            },
        }
    }

    pub fn requirement_identity<'identities>(
        &'identities self,
        identities: &'identities terminal_psi::TerminalIdentityTable,
    ) -> Option<&'identities str> {
        if let NormalizedForeignCallMechanism::HostedImport {
            requirement_identity,
            ..
        } = &self.mechanism
        {
            return Some(requirement_identity);
        }
        match self.mechanism.same_stack_contribution() {
            Some(contribution) => identities.text(contribution.requirement_identity()),
            None => self
                .system_call()
                .map(|binding| binding.requirement_identity.as_str()),
        }
    }

    pub fn provider_plan_report_identity(&self) -> u64 {
        match self.mechanism.same_stack_contribution() {
            Some(contribution) => contribution.provider_plan_report_identity(),
            None => self.system_call().map_or(0, |binding| {
                binding.execution.provider_report().unwrap_or(0)
            }),
        }
    }

    pub fn has_valid_entry_mechanism(&self) -> bool {
        use calling_conventions::{CallSignature, CallingPolicy, EntryControl};
        let Some(target) = self.native_target() else {
            return false;
        };
        let call = &self.boundary_entry_plan.call;
        match &self.mechanism {
            NormalizedForeignCallMechanism::Component {
                locator,
                provider,
                application_commitment,
                ..
            } => {
                call.policy == CallingPolicy::native_for_target(target)
                    && call.entry_control == EntryControl::CallReturn
                    && *application_commitment != [0; 32]
                    && *locator.slot_commitment()
                        == target::component_export_slot(
                            provider.vocabulary_marker.get(),
                            provider.program_fingerprint.as_bytes(),
                            application_commitment,
                        )
            }
            NormalizedForeignCallMechanism::HostedImport {
                locator,
                receipt,
                requirement_identity,
            } => {
                *receipt != [0; 32]
                    && !requirement_identity.is_empty()
                    && locator.target().program_entry_slot().schema
                        == target::ProgramEntrySchema::HostedApplication
                    && call.policy == CallingPolicy::native_for_target(target)
                    && call.entry_control == EntryControl::CallReturn
            }
            NormalizedForeignCallMechanism::Import { .. }
            | NormalizedForeignCallMechanism::Table { .. } => {
                call.policy == CallingPolicy::native_for_target(target)
                    && call.entry_control == EntryControl::CallReturn
            }
            NormalizedForeignCallMechanism::SystemCall(binding) => {
                let Some(policy) = CallingPolicy::native_syscall_for_target(target) else {
                    return false;
                };
                let signature = CallSignature {
                    parameters: call
                        .parameters
                        .iter()
                        .map(|parameter| parameter.shape)
                        .collect(),
                    result: call.result.as_ref().map(|result| result.shape),
                };
                binding.number >= 0
                    && !binding.requirement_identity.is_empty()
                    && binding.execution.is_complete()
                    && calling_conventions::evaluate_call_plan(policy, &signature).as_ref()
                        == Ok(call)
            }
        }
    }

    pub fn system_call(&self) -> Option<&NormalizedSystemCallBinding> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::SystemCall(binding) => Some(binding),
            NormalizedForeignCallMechanism::Import { .. }
            | NormalizedForeignCallMechanism::HostedImport { .. }
            | NormalizedForeignCallMechanism::Component { .. }
            | NormalizedForeignCallMechanism::Table { .. } => None,
        }
    }

    pub fn import(
        &self,
    ) -> Option<(
        &target::NormalizedForeignLocator,
        &task_plans::AdmittedSameStackContribution,
    )> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution,
            } => Some((locator, same_stack_contribution)),
            NormalizedForeignCallMechanism::SystemCall(_)
            | NormalizedForeignCallMechanism::HostedImport { .. }
            | NormalizedForeignCallMechanism::Component { .. }
            | NormalizedForeignCallMechanism::Table { .. } => None,
        }
    }

    pub fn symbolic_import(
        &self,
    ) -> Option<(
        target::NormalizedImportLocatorRef<'_>,
        &task_plans::AdmittedSameStackContribution,
    )> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution,
            } => Some((
                target::NormalizedImportLocatorRef::Foreign(locator),
                same_stack_contribution,
            )),
            NormalizedForeignCallMechanism::Component {
                locator,
                same_stack_contribution,
                ..
            } => Some((
                target::NormalizedImportLocatorRef::Component(locator),
                same_stack_contribution,
            )),
            NormalizedForeignCallMechanism::SystemCall(_)
            | NormalizedForeignCallMechanism::HostedImport { .. }
            | NormalizedForeignCallMechanism::Table { .. } => None,
        }
    }

    pub fn import_locator(&self) -> Option<target::NormalizedImportLocatorRef<'_>> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import { locator, .. }
            | NormalizedForeignCallMechanism::HostedImport { locator, .. } => {
                Some(target::NormalizedImportLocatorRef::Foreign(locator))
            }
            NormalizedForeignCallMechanism::Component { locator, .. } => {
                Some(target::NormalizedImportLocatorRef::Component(locator))
            }
            _ => None,
        }
    }

    pub fn foreign_import_locator(&self) -> Option<&target::NormalizedForeignLocator> {
        match self.import_locator()? {
            target::NormalizedImportLocatorRef::Foreign(locator) => Some(locator),
            target::NormalizedImportLocatorRef::Component(_) => None,
        }
    }

    pub fn hosted_import(&self) -> Option<&target::NormalizedForeignLocator> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::HostedImport { locator, .. } => Some(locator),
            _ => None,
        }
    }

    pub fn same_stack_contribution(&self) -> Option<&task_plans::AdmittedSameStackContribution> {
        self.mechanism.same_stack_contribution()
    }

    /// On a normal return, user-space calls must preserve floating controls
    /// regardless of the callee's stack bound. Callers decide whether the
    /// invocation has a return continuation; direct syscalls need no envelope.
    pub fn requires_floating_control_preservation(&self) -> bool {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import { .. }
            | NormalizedForeignCallMechanism::HostedImport { .. }
            | NormalizedForeignCallMechanism::Component { .. }
            | NormalizedForeignCallMechanism::Table { .. } => true,
            NormalizedForeignCallMechanism::SystemCall(_) => false,
        }
    }

    pub fn table_dispatch(&self) -> Option<(u32, bool)> {
        match self.mechanism {
            NormalizedForeignCallMechanism::Table {
                byte_offset,
                dispatch_only,
                ..
            } => Some((byte_offset, dispatch_only)),
            _ => None,
        }
    }
}

pub struct ForeignCallOperands<'plan> {
    dispatch: Option<calling_conventions::ValuePlacement>,
    parameters: &'plan [calling_conventions::ValuePlacement],
}

impl ForeignCallOperands<'_> {
    pub fn len(&self) -> usize {
        self.parameters.len() + usize::from(self.dispatch.is_some())
    }

    pub fn is_empty(&self) -> bool {
        self.dispatch.is_none() && self.parameters.is_empty()
    }

    pub fn get(&self, operand_index: usize) -> Option<&calling_conventions::ValuePlacement> {
        match (&self.dispatch, operand_index) {
            (Some(dispatch), 0) => Some(dispatch),
            (Some(_), operand_index) => self.parameters.get(operand_index - 1),
            (None, operand_index) => self.parameters.get(operand_index),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &calling_conventions::ValuePlacement> {
        self.dispatch.iter().chain(self.parameters.iter())
    }
}

pub type NormalizedForeignScalarArgument = TargetUnitScalarCallArgument;

pub type NormalizedForeignStructuralArgument = TargetStructuralArgument;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundarySettlementRealization {
    Builtin(BoundaryRealization),
    NormalizedForeignCall(NormalizedForeignCallBinding),
}

impl From<BoundaryRealization> for BoundarySettlementRealization {
    fn from(realization: BoundaryRealization) -> Self {
        Self::Builtin(realization)
    }
}

macro_rules! boundary_realization_conversion {
    ($variant:ident, $realization:ty) => {
        impl From<$realization> for BoundaryRealization {
            fn from(realization: $realization) -> Self {
                Self::$variant(realization)
            }
        }

        impl From<$realization> for BoundarySettlementRealization {
            fn from(realization: $realization) -> Self {
                Self::Builtin(realization.into())
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryRealization {
    EmptyRunRelease,
    StaticInterrupt {
        boundary: semantic_vocabulary::BoundaryMachineId,
        operation: language_core::static_interrupts::StaticInterruptOperation,
    },
    AccessLifecycle {
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    ProgramImageRetainedSuffixStart,
    AccessorScalarAccess(AccessorScalarAccessRealization),
    MetadataOnlyPort(MetadataOnlyPortRealization),
    DirectPortReadU8(DirectPortReadU8Realization),
    LinuxWriteLine(LinuxWriteLineRealization),
    HostedExitProcessI32(HostedExitProcessI32Realization),
    HostedReadByte(HostedReadByteRealization),
    HostedWriteByteI32(HostedWriteByteI32Realization),
    ClaimCompletionOnly(ClaimCompletionOnlyRealization),
    NearestIeeeFloatArithmetic(NearestIeeeFloatArithmeticRealization),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessorScalarAccessRealization {
    pub contract: [u8; 32],
    pub demand: terminal_psi::AccessorScalarBoundaryDemand,
}

boundary_realization_conversion!(
    NearestIeeeFloatArithmetic,
    NearestIeeeFloatArithmeticRealization
);
boundary_realization_conversion!(MetadataOnlyPort, MetadataOnlyPortRealization);
boundary_realization_conversion!(DirectPortReadU8, DirectPortReadU8Realization);
boundary_realization_conversion!(HostedExitProcessI32, HostedExitProcessI32Realization);
boundary_realization_conversion!(HostedReadByte, HostedReadByteRealization);
boundary_realization_conversion!(HostedWriteByteI32, HostedWriteByteI32Realization);
boundary_realization_conversion!(LinuxWriteLine, LinuxWriteLineRealization);
boundary_realization_conversion!(ClaimCompletionOnly, ClaimCompletionOnlyRealization);
boundary_realization_conversion!(AccessorScalarAccess, AccessorScalarAccessRealization);
