//! Target coordinates are checked without consulting the recovering host.

use crate::record::{
    PackageReviewRepresentationArchitecture as Architecture,
    PackageReviewRepresentationObjectFormat as ObjectFormat,
    PackageReviewRepresentationTarget as Target,
    PackageReviewRepresentationTargetProfile as Profile,
};

impl Target {
    pub(crate) fn validate_canonical_structure(self) -> Result<(), &'static str> {
        let target = self;
        let profile = match target.profile {
            Profile::LinuxArm64 => target::TargetProfile::LinuxArm64,
            Profile::LinuxX64 => target::TargetProfile::LinuxX64,
            Profile::MacosArm64 => target::TargetProfile::MacosArm64,
            Profile::MacosX64 => target::TargetProfile::MacosX64,
            Profile::WindowsX64 => target::TargetProfile::WindowsX64,
            Profile::UefiX64 => target::TargetProfile::UefiX64,
            Profile::CrossPlatformCli | Profile::LocalUnchecked => {
                // These profiles capture the producer's host target. Comparing to
                // this process's host would reject portable historical policy, so
                // the record keeps only what `size_of`/`align_of::<usize>` can
                // produce: a nonzero power-of-two alignment and a nonzero size
                // that is a multiple of it. Narrower or wider real hosts (32-bit
                // or capability-sized pointers) stay canonical; fabricated
                // geometry does not.
                return if target.pointer_alignment.is_power_of_two()
                    && target.pointer_size != 0
                    && target.pointer_size.is_multiple_of(target.pointer_alignment)
                {
                    Ok(())
                } else {
                    Err("calling host-profile pointer geometry is unrealizable")
                };
            }
        };
        let expected = profile.native_target();
        let architecture = match target.architecture {
            Architecture::Aarch64 => target::Architecture::Aarch64,
            Architecture::X86_64 => target::Architecture::X86_64,
        };
        let object_format = match target.object_format {
            ObjectFormat::Elf => target::ObjectFormat::Elf,
            ObjectFormat::MachO => target::ObjectFormat::MachO,
            ObjectFormat::Coff => target::ObjectFormat::Coff,
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
}

#[cfg(test)]
mod tests {
    use super::{Architecture, ObjectFormat, Profile, Target};

    fn host_target(pointer_size: u16, pointer_alignment: u16) -> Target {
        Target {
            profile: Profile::CrossPlatformCli,
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Elf,
            pointer_size,
            pointer_alignment,
        }
    }

    #[test]
    fn host_profiles_admit_realizable_pointer_geometry() {
        for (pointer_size, pointer_alignment) in [(4, 4), (8, 8), (8, 4), (16, 16)] {
            for profile in [Profile::CrossPlatformCli, Profile::LocalUnchecked] {
                let target = Target {
                    profile,
                    ..host_target(pointer_size, pointer_alignment)
                };
                assert_eq!(target.validate_canonical_structure(), Ok(()));
            }
        }
    }

    #[test]
    fn host_profiles_reject_unrealizable_pointer_geometry() {
        // Zero size, zero or non-power-of-two alignment, alignment exceeding
        // size, and a size that is not a multiple of alignment are all outside
        // what `size_of`/`align_of::<usize>` can produce on any host.
        for (pointer_size, pointer_alignment) in [(0, 8), (8, 0), (8, 3), (4, 8), (6, 4)] {
            for profile in [Profile::CrossPlatformCli, Profile::LocalUnchecked] {
                let target = Target {
                    profile,
                    ..host_target(pointer_size, pointer_alignment)
                };
                assert!(target.validate_canonical_structure().is_err());
            }
        }
    }

    #[test]
    fn exact_profiles_still_pin_pointer_geometry() {
        let target = Target {
            profile: Profile::LinuxX64,
            ..host_target(8, 8)
        };
        assert_eq!(target.validate_canonical_structure(), Ok(()));
        for (pointer_size, pointer_alignment) in [(4, 4), (8, 4), (16, 16)] {
            let narrowed = Target {
                pointer_size,
                pointer_alignment,
                ..target
            };
            assert!(narrowed.validate_canonical_structure().is_err());
        }
    }
}
