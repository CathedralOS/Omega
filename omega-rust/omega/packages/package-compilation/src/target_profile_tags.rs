//! Canonical one-byte tags for target profiles in persisted records.

pub(crate) fn target_profile_tag(profile: target::TargetProfile) -> u8 {
    match profile {
        target::TargetProfile::LinuxArm64 => 1,
        target::TargetProfile::LinuxX64 => 2,
        target::TargetProfile::MacosArm64 => 3,
        target::TargetProfile::WindowsX64 => 4,
        target::TargetProfile::UefiX64 => 5,
        target::TargetProfile::CrossPlatformCli => 6,
        target::TargetProfile::LocalUnchecked => 7,
        target::TargetProfile::AlphaBootstrap => 8,
        target::TargetProfile::MacosX64 => 9,
    }
}

/// Recover a target profile from its canonical evidence tag.
pub(crate) fn recover_target_profile_tag(tag: u8) -> Option<target::TargetProfile> {
    use target::TargetProfile::*;
    match tag {
        1 => Some(LinuxArm64),
        2 => Some(LinuxX64),
        3 => Some(MacosArm64),
        4 => Some(WindowsX64),
        5 => Some(UefiX64),
        6 => Some(CrossPlatformCli),
        7 => Some(LocalUnchecked),
        8 => Some(AlphaBootstrap),
        9 => Some(MacosX64),
        _ => None,
    }
}
