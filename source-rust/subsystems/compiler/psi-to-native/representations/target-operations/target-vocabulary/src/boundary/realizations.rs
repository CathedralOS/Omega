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
    pub fn supports_target(_target: target::NativeTarget) -> bool {
        // This retired intrinsic has no input-error case. Both provider admission
        // and target lowering must refuse it rather than trap on ordinary errors.
        false
    }
}

#[cfg(test)]
mod hosted_read_retirement_tests {
    use super::HostedReadByteRealization;
    use target::NativeTarget;

    #[test]
    fn obsolete_two_case_read_cannot_enter_any_native_provider() {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_x64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            assert!(
                !HostedReadByteRealization::supports_target(target),
                "{target:?}"
            );
        }
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
        use crate::IeeeFloatArithmeticOperation::{DirectedFusedMultiplyAdd, FusedMultiplyAdd};
        match (target.architecture, self.x86_scalar_fma) {
            (target::Architecture::Aarch64, None) => true,
            (target::Architecture::X86_64, None) => !matches!(
                self.operation,
                FusedMultiplyAdd | DirectedFusedMultiplyAdd { .. }
            ),
            (target::Architecture::X86_64, Some(provider)) => {
                self.operation == FusedMultiplyAdd && provider.profile().native_target() == target
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

/// Fixed operating-system imports used to implement a sealed compiler primitive.
/// A Build-issued import carries its own independent receipt. Each Windows
/// target imports `kernel32!ExitProcess` under its own locator and C ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerImport {
    WindowsProcessExit,
    WindowsArm64ProcessExit,
}

impl CompilerImport {
    pub const fn tag(self) -> u8 {
        match self {
            Self::WindowsProcessExit => 0,
            Self::WindowsArm64ProcessExit => 1,
        }
    }

    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::WindowsProcessExit),
            1 => Some(Self::WindowsArm64ProcessExit),
            _ => None,
        }
    }

    /// The import that realizes hosted process exit on `profile`, if any.
    pub const fn process_exit(profile: target::TargetProfile) -> Option<Self> {
        match profile {
            target::TargetProfile::WindowsX64 => Some(Self::WindowsProcessExit),
            target::TargetProfile::WindowsArm64 => Some(Self::WindowsArm64ProcessExit),
            _ => None,
        }
    }

    pub const fn target(self) -> target::TargetProfile {
        match self {
            Self::WindowsProcessExit => target::TargetProfile::WindowsX64,
            Self::WindowsArm64ProcessExit => target::TargetProfile::WindowsArm64,
        }
    }

    pub fn locator(self) -> &'static target::NormalizedForeignLocator {
        use std::sync::OnceLock;
        static EXIT: [OnceLock<target::NormalizedForeignLocator>; 2] =
            [OnceLock::new(), OnceLock::new()];
        EXIT[usize::from(self.tag())].get_or_init(|| {
            target::normalize_foreign_locator(
                target::ForeignLocatorCandidate::PeByName {
                    library: b"kernel32.dll".to_vec(),
                    export: b"ExitProcess".to_vec(),
                },
                self.target(),
            )
            .expect("kernel32 ExitProcess is a Windows import")
        })
    }

    pub fn entry_plan(self) -> &'static BoundaryEntryPlan {
        use calling_conventions::{CallSignature, CallingPolicy, ValueShape};
        use std::sync::OnceLock;
        static PLAN: [OnceLock<BoundaryEntryPlan>; 2] = [OnceLock::new(), OnceLock::new()];
        PLAN[usize::from(self.tag())].get_or_init(|| {
            calling_conventions::evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::native_for_target(self.target().native_target()),
                &CallSignature {
                    parameters: vec![ValueShape::integer(4, 4)],
                    result: None,
                },
            )
            .expect("ExitProcess takes one Windows DWORD and never returns")
            .plan()
            .clone()
        })
    }

    pub fn binding(
        self,
        requirement_identity: String,
        selected_plan: [u8; 32],
    ) -> NormalizedForeignCallBinding {
        NormalizedForeignCallBinding {
            boundary_entry_plan: self.entry_plan().clone(),
            mechanism: NormalizedForeignCallMechanism::CompilerImport {
                import: self,
                requirement_identity,
                selected_plan,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedForeignCallBinding {
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub mechanism: NormalizedForeignCallMechanism,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedForeignCallMechanism {
    CompilerImport {
        import: CompilerImport,
        requirement_identity: String,
        selected_plan: [u8; 32],
    },
    /// Imported through an admitted Build binding and its requirement contract.
    BuildImport {
        stack: NativeBindingStack,
        locator: target::NormalizedForeignLocator,
        receipt: [u8; 32],
        requirement_identity: String,
    },
    Import {
        locator: target::NormalizedForeignLocator,
        same_stack_contribution: provider_stack_contribution::AdmittedSameStackContribution,
    },
    Component {
        locator: target::NormalizedComponentLocator,
        provider: terminal_psi::TerminalPsiIdentity,
        candidate: semantic_vocabulary::MachineId,
        application_commitment: [u8; 32],
        same_stack_contribution: provider_stack_contribution::AdmittedSameStackContribution,
    },
    SystemCall(NormalizedSystemCallBinding),
    Table {
        target: target::TargetProfile,
        byte_offset: u32,
        /// The supplied table's position among all operands, including private callbacks.
        table_operand: u32,
        dispatch_only: bool,
        source: TableBindingSource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableBindingSource {
    Provider(provider_stack_contribution::AdmittedSameStackContribution),
    BuildBinding {
        stack: NativeBindingStack,
        receipt: [u8; 32],
        requirement_identity: String,
    },
}

/// Caller-stack policy of an admitted native Build binding. A finite promise
/// belongs to this exact execution, including its contract and calling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBindingStack {
    Hosted,
    Bounded {
        execution: super::NativeBindingExecution,
        bytes: std::num::NonZeroU64,
    },
}

impl NativeBindingStack {
    pub fn for_requirement(
        target: target::TargetProfile,
        execution: super::NativeBindingExecution,
        bytes: u64,
    ) -> Result<Self, String> {
        if let Some(bytes) = std::num::NonZeroU64::new(bytes) {
            return Ok(Self::Bounded { execution, bytes });
        }
        if target.program_entry_slot().schema == target::ProgramEntrySchema::HostedApplication {
            return Ok(Self::Hosted);
        }
        Err("native Build binding needs admitted same-stack demand on a fixed or freestanding stack".into())
    }

    fn permits(self, profile: target::TargetProfile, receipt: &[u8; 32]) -> bool {
        match self {
            Self::Hosted => {
                profile.program_entry_slot().schema == target::ProgramEntrySchema::HostedApplication
            }
            Self::Bounded { execution, .. } => &execution.receipt() == receipt,
        }
    }

    pub fn commitment(self) -> [u8; 32] {
        let mut digest = wire::digest_encoder::DomainDigestEncoder::new(
            b"omega.native-binding-caller-stack.v1\0",
        );
        match self {
            Self::Hosted => digest.byte(0),
            Self::Bounded { execution, bytes } => {
                digest.byte(1);
                digest.raw(&execution.receipt());
                digest.raw(&execution.contract());
                digest.raw(&execution.calling());
                digest.u64(bytes.get());
            }
        }
        digest.finish()
    }
}

impl TableBindingSource {
    pub fn commitment(&self) -> [u8; 32] {
        let mut digest =
            wire::digest_encoder::DomainDigestEncoder::new(b"omega.table-call-source.v1\0");
        match self {
            Self::Provider(contribution) => {
                digest.byte(0);
                digest.raw(&contribution.commitment().as_bytes());
            }
            Self::BuildBinding {
                stack,
                receipt,
                requirement_identity,
            } => {
                digest.byte(1);
                digest.raw(&stack.commitment());
                digest.raw(receipt);
                digest.bytes(requirement_identity.as_bytes());
            }
        }
        digest.finish()
    }
}

impl NormalizedForeignCallMechanism {
    fn same_stack_contribution(
        &self,
    ) -> Option<&provider_stack_contribution::AdmittedSameStackContribution> {
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
                source: TableBindingSource::Provider(same_stack_contribution),
                ..
            } => Some(same_stack_contribution),
            Self::Table { source: TableBindingSource::BuildBinding { .. }, .. } => None,
            Self::SystemCall(_) | Self::BuildImport { .. } | Self::CompilerImport { .. } => None,
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
            NormalizedForeignCallMechanism::BuildImport { receipt, .. }
            | NormalizedForeignCallMechanism::Table { source: TableBindingSource::BuildBinding { receipt, .. }, .. } => Some(*receipt),
            NormalizedForeignCallMechanism::SystemCall(binding) => match binding.execution {
                NormalizedForeignExecution::NativeBinding(receipt) => Some(receipt),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn external_execution(
        &self,
        execution: super::BoundaryExecutionBinding,
    ) -> Option<super::ExternalExecutionBinding> {
        if let (
            super::BoundaryExecutionBinding::CompilerBuiltin(
                super::CompilerBuiltinExecution::HostedExitProcessI32,
            ),
            NormalizedForeignCallMechanism::CompilerImport { import, .. },
        ) = (execution, &self.mechanism)
        {
            return Some(super::ExternalExecutionBinding::CompilerImport(*import));
        }
        execution.external()
    }

    pub fn matches_execution(&self, execution: super::ExternalExecutionBinding) -> bool {
        match execution {
            super::ExternalExecutionBinding::CompilerImport(expected) => matches!(self.mechanism,
                NormalizedForeignCallMechanism::CompilerImport { import, .. } if import == expected) && self.has_valid_entry_mechanism(),
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
                table_operand,
                dispatch_only: true,
                ..
            } => {
                let register = match target.native_target().architecture {
                    target::Architecture::X86_64 => MachineRegister::X86R11,
                    target::Architecture::Aarch64 => MachineRegister::Aarch64X(16),
                };
                Some((
                    *table_operand as usize,
                    ValuePlacement {
                        shape: ValueShape::integer(8, 8),
                        locations: vec![ValueLocation::Register {
                            register,
                            byte_size: 8,
                            value_byte_offset: 0,
                        }],
                    },
                ))
            }
            _ => None,
        };
        ForeignCallOperands {
            dispatch,
            parameters: &self.boundary_entry_plan.call.parameters,
        }
    }

    pub fn table_operand_index(&self) -> Option<usize> {
        match self.mechanism {
            NormalizedForeignCallMechanism::Table { table_operand, .. } => {
                Some(table_operand as usize)
            }
            _ => None,
        }
    }

    fn dispatch_only_operand(&self) -> Option<usize> {
        match self.mechanism {
            NormalizedForeignCallMechanism::Table {
                table_operand,
                dispatch_only: true,
                ..
            } => Some(table_operand as usize),
            _ => None,
        }
    }

    pub fn operand_for_native_ordinal(&self, ordinal: usize) -> usize {
        ordinal.saturating_add(usize::from(
            self.dispatch_only_operand()
                .is_some_and(|table| table <= ordinal),
        ))
    }

    pub fn passed_operands<T>(
        &self,
        operands: impl IntoIterator<Item = T>,
    ) -> impl Iterator<Item = T> {
        let dispatch = self.dispatch_only_operand();
        operands
            .into_iter()
            .enumerate()
            .filter_map(move |(index, operand)| (Some(index) != dispatch).then_some(operand))
    }

    pub fn native_target(&self) -> Option<target::NativeTarget> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import { locator, .. }
            | NormalizedForeignCallMechanism::BuildImport { locator, .. } => {
                Some(locator.target().native_target())
            }
            NormalizedForeignCallMechanism::CompilerImport { import, .. } => {
                Some(import.locator().target().native_target())
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
        if let NormalizedForeignCallMechanism::BuildImport {
            requirement_identity,
            ..
        }
        | NormalizedForeignCallMechanism::CompilerImport {
            requirement_identity,
            ..
        }
        | NormalizedForeignCallMechanism::Table {
            source: TableBindingSource::BuildBinding { requirement_identity, .. }, ..
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
            NormalizedForeignCallMechanism::CompilerImport {
                import,
                requirement_identity,
                selected_plan,
            } => {
                *selected_plan != [0; 32]
                    && !requirement_identity.is_empty()
                    && self.boundary_entry_plan == *import.entry_plan()
            }
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
            NormalizedForeignCallMechanism::BuildImport {
                stack,
                locator,
                receipt,
                requirement_identity,
            } => {
                *receipt != [0; 32]
                    && !requirement_identity.is_empty()
                    && stack.permits(locator.target(), receipt)
                    && call.policy == CallingPolicy::native_for_target(target)
                    && call.entry_control == EntryControl::CallReturn
            }
            NormalizedForeignCallMechanism::Table {
                target, source: TableBindingSource::BuildBinding { stack, receipt, requirement_identity }, ..
            } if !stack.permits(*target, receipt)
                || *receipt == [0;32] || requirement_identity.is_empty() => false,
            NormalizedForeignCallMechanism::Import { .. }
            | NormalizedForeignCallMechanism::Table { .. } => {
                call.policy == CallingPolicy::native_for_target(target)
                    && call.entry_control == EntryControl::CallReturn
                    && self
                        .table_operand_index()
                        .is_none_or(|index| index < self.operand_placements().len())
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
            | NormalizedForeignCallMechanism::BuildImport { .. }
            | NormalizedForeignCallMechanism::CompilerImport { .. }
            | NormalizedForeignCallMechanism::Component { .. }
            | NormalizedForeignCallMechanism::Table { .. } => None,
        }
    }

    pub fn import(
        &self,
    ) -> Option<(
        &target::NormalizedForeignLocator,
        &provider_stack_contribution::AdmittedSameStackContribution,
    )> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution,
            } => Some((locator, same_stack_contribution)),
            NormalizedForeignCallMechanism::SystemCall(_)
            | NormalizedForeignCallMechanism::BuildImport { .. }
            | NormalizedForeignCallMechanism::CompilerImport { .. }
            | NormalizedForeignCallMechanism::Component { .. }
            | NormalizedForeignCallMechanism::Table { .. } => None,
        }
    }

    pub fn import_locator(&self) -> Option<target::NormalizedImportLocatorRef<'_>> {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import { locator, .. }
            | NormalizedForeignCallMechanism::BuildImport { locator, .. } => {
                Some(target::NormalizedImportLocatorRef::Foreign(locator))
            }
            NormalizedForeignCallMechanism::CompilerImport { import, .. } => Some(
                target::NormalizedImportLocatorRef::Foreign(import.locator()),
            ),
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

    pub fn same_stack_contribution(
        &self,
    ) -> Option<&provider_stack_contribution::AdmittedSameStackContribution> {
        self.mechanism.same_stack_contribution()
    }

    /// On a normal return, user-space calls must preserve floating controls
    /// regardless of the callee's stack bound. Callers decide whether the
    /// invocation has a return continuation; direct syscalls need no envelope.
    pub fn requires_floating_control_preservation(&self) -> bool {
        match &self.mechanism {
            NormalizedForeignCallMechanism::Import { .. }
            | NormalizedForeignCallMechanism::BuildImport { .. }
            | NormalizedForeignCallMechanism::CompilerImport { .. }
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
    dispatch: Option<(usize, calling_conventions::ValuePlacement)>,
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
        match &self.dispatch {
            Some((index, dispatch)) if operand_index == *index => Some(dispatch),
            Some((index, _)) if operand_index > *index => self.parameters.get(operand_index - 1),
            _ => self.parameters.get(operand_index),
        }
    }

    /// Map semantic formals to their full operand positions. Private callback
    /// positions already include all earlier callbacks and any dispatch slot.
    pub fn semantic_positions(&self, private: &[usize]) -> Option<Vec<usize>> {
        if private.windows(2).any(|pair| pair[0] >= pair[1])
            || private
                .iter()
                .any(|&index| index >= self.len() || self.native_ordinal(index).is_none())
        {
            return None;
        }
        Some(
            (0..self.len())
                .filter(|index| private.binary_search(index).is_err())
                .collect(),
        )
    }

    pub fn native_ordinal(&self, operand_index: usize) -> Option<usize> {
        match self.dispatch.as_ref().map(|(index, _)| *index) {
            Some(index) if index == operand_index => None,
            Some(index) if index < operand_index => Some(operand_index - 1),
            _ => Some(operand_index),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &calling_conventions::ValuePlacement> {
        (0..self.len()).filter_map(|index| self.get(index))
    }
}

#[cfg(test)]
mod operand_tests {
    use super::ForeignCallOperands;
    use calling_conventions::{ValuePlacement, ValueShape};

    #[test]
    fn the_table_keeps_its_position_among_semantic_and_private_arguments() {
        let placement = |width| ValuePlacement {
            shape: ValueShape::integer(width, 1),
            locations: Vec::new(),
        };
        let parameters = [placement(1), placement(2), placement(4), placement(8)];
        let dispatch = placement(8);
        let placements = ForeignCallOperands {
            dispatch: Some((2, dispatch.clone())),
            parameters: &parameters,
        };
        assert_eq!(
            placements.iter().collect::<Vec<_>>(),
            [
                &parameters[0],
                &parameters[1],
                &dispatch,
                &parameters[2],
                &parameters[3]
            ]
        );
        assert_eq!(placements.native_ordinal(2), None);
        assert_eq!(placements.native_ordinal(3), Some(2));
        assert_eq!(placements.semantic_positions(&[0, 1, 4]), Some(vec![2, 3]));
        assert!(placements.semantic_positions(&[2]).is_none());
        assert!(placements.semantic_positions(&[0, 0]).is_none());
        assert!(placements.semantic_positions(&[5]).is_none());

        let passed = ForeignCallOperands {
            dispatch: None,
            parameters: &parameters,
        };
        assert_eq!(passed.semantic_positions(&[0, 1]), Some(vec![2, 3]));
        assert_eq!(passed.native_ordinal(2), Some(2));
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
