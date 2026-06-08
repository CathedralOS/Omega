use omega_target::{Architecture, NativeTarget, ObjectFormat};

pub fn can_emit_executable_image(target: NativeTarget) -> bool {
    matches!(
        (target.object_format, target.architecture),
        (ObjectFormat::Elf, Architecture::Aarch64)
            | (ObjectFormat::Elf, Architecture::X86_64)
            | (ObjectFormat::MachO, Architecture::Aarch64)
            | (ObjectFormat::Coff, Architecture::X86_64)
    )
}

#[cfg(test)]
mod tests {
    use super::can_emit_executable_image;
    use omega_target::NativeTarget;

    #[test]
    fn reports_direct_image_writer_support_by_target() {
        assert!(can_emit_executable_image(NativeTarget::linux_arm64()));
        assert!(can_emit_executable_image(NativeTarget::macos_arm64()));
        assert!(can_emit_executable_image(NativeTarget::windows_x64()));
        assert!(can_emit_executable_image(NativeTarget::linux_x64()));
    }
}
