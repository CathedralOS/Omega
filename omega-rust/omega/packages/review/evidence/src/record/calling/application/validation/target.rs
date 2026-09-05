//! Target coordinates are checked without consulting the recovering host.

use crate::record::{
    PackageReviewRepresentationArchitecture as Architecture,
    PackageReviewRepresentationObjectFormat as ObjectFormat,
    PackageReviewRepresentationTarget as Target,
    PackageReviewRepresentationTargetProfile as Profile,
};

pub(super) fn validate(target: Target) -> Result<(), &'static str> {
    let profile = match target.profile {
        Profile::LinuxArm64 => omega_target::TargetProfile::LinuxArm64,
        Profile::LinuxX64 => omega_target::TargetProfile::LinuxX64,
        Profile::MacosArm64 => omega_target::TargetProfile::MacosArm64,
        Profile::WindowsX64 => omega_target::TargetProfile::WindowsX64,
        Profile::UefiX64 => omega_target::TargetProfile::UefiX64,
        Profile::CrossPlatformCli | Profile::LocalUnchecked => {
            // These profiles capture the producer's host target. Comparing to
            // this process's host would reject portable historical policy.
            return if target.pointer_size == 8 && target.pointer_alignment == 8 {
                Ok(())
            } else {
                Err("calling host-profile pointer geometry is unsupported")
            };
        }
    };
    let expected = profile.native_target();
    let architecture = match target.architecture {
        Architecture::Aarch64 => omega_target::Architecture::Aarch64,
        Architecture::X86_64 => omega_target::Architecture::X86_64,
    };
    let object_format = match target.object_format {
        ObjectFormat::Elf => omega_target::ObjectFormat::Elf,
        ObjectFormat::MachO => omega_target::ObjectFormat::MachO,
        ObjectFormat::Coff => omega_target::ObjectFormat::Coff,
    };
    if architecture != expected.architecture
        || object_format != expected.object_format
        || usize::from(target.pointer_size) != expected.pointer_size
        || usize::from(target.pointer_alignment) != expected.pointer_alignment
    {
        return Err("calling target coordinates disagree with their exact profile");
    }
    Ok(())
}
