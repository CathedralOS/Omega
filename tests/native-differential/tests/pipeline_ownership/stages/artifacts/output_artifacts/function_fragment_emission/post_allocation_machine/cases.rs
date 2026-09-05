use crate::tests::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum FixtureKind {
    ExactBinary,
    Movn,
    XorZero,
    MovR32,
    MovR64,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ExpectedActions {
    Zero,
    NonZero,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HostedCase {
    pub(super) rule: Optimization,
    pub(super) target: NativeTarget,
    pub(super) fixture: FixtureKind,
    pub(super) actions: ExpectedActions,
    pub(super) object_format: target::ObjectFormat,
    pub(super) text_section: &'static str,
    pub(super) calling_policy: calling_conventions::CallingPolicy,
    pub(super) exit_policy: WholeFunctionExitPolicy,
}

pub(super) fn hosted_cases() -> Vec<HostedCase> {
    let mut cases = Vec::new();
    for (rule, fixture, actions) in [
        (
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            FixtureKind::ExactBinary,
            ExpectedActions::NonZero,
        ),
        (
            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
            FixtureKind::Movn,
            ExpectedActions::NonZero,
        ),
        (
            Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
            FixtureKind::ExactBinary,
            ExpectedActions::Zero,
        ),
        (
            Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
            FixtureKind::ExactBinary,
            ExpectedActions::Zero,
        ),
        (
            Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1,
            FixtureKind::ExactBinary,
            ExpectedActions::Zero,
        ),
        (
            Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1,
            FixtureKind::ExactBinary,
            ExpectedActions::Zero,
        ),
    ] {
        cases.extend([
            arm64_case(rule, NativeTarget::linux_arm64(), fixture, actions),
            arm64_case(rule, NativeTarget::macos_arm64(), fixture, actions),
        ]);
    }
    for (rule, fixture) in [
        (
            Optimization::X86SelectXorZeroI64MaterializationV1,
            FixtureKind::XorZero,
        ),
        (
            Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
            FixtureKind::MovR32,
        ),
        (
            Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
            FixtureKind::MovR64,
        ),
    ] {
        cases.extend([
            x64_case(rule, NativeTarget::linux_x64(), fixture),
            x64_case(rule, NativeTarget::windows_x64(), fixture),
        ]);
    }
    cases
}

fn arm64_case(
    rule: Optimization,
    target: NativeTarget,
    fixture: FixtureKind,
    actions: ExpectedActions,
) -> HostedCase {
    let (object_format, text_section, exit_policy) = match target.object_format {
        target::ObjectFormat::Elf => (
            target::ObjectFormat::Elf,
            ".text",
            WholeFunctionExitPolicy::Aapcs64FramelessLeafV1,
        ),
        target::ObjectFormat::MachO => (
            target::ObjectFormat::MachO,
            "__TEXT,__text",
            WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1,
        ),
        _ => unreachable!(),
    };
    HostedCase {
        rule,
        target,
        fixture,
        actions,
        object_format,
        text_section,
        calling_policy: calling_conventions::CallingPolicy::Aapcs64,
        exit_policy,
    }
}

fn x64_case(rule: Optimization, target: NativeTarget, fixture: FixtureKind) -> HostedCase {
    let (object_format, calling_policy, exit_policy) = match target.object_format {
        target::ObjectFormat::Elf => (
            target::ObjectFormat::Elf,
            calling_conventions::CallingPolicy::SystemVAMD64,
            WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
        ),
        target::ObjectFormat::Coff => (
            target::ObjectFormat::Coff,
            calling_conventions::CallingPolicy::MicrosoftX64,
            WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1,
        ),
        _ => unreachable!(),
    };
    HostedCase {
        rule,
        target,
        fixture,
        actions: ExpectedActions::NonZero,
        object_format,
        text_section: ".text",
        calling_policy,
        exit_policy,
    }
}
