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
        if matches!(
            target.profile,
            Profile::CrossPlatformCli | Profile::LocalUnchecked
        ) {
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
        let expected = target.profile.backend_profile().native_target();
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
