use super::*;

pub(super) fn parameter_location_cases() -> [(
    NativeTarget,
    ScalarParameterLocation,
    ScalarParameterLocation,
); 5] {
    [
        (
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
        ),
        (
            NativeTarget::windows_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::uefi_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
        (
            NativeTarget::macos_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ]
}

pub(super) fn boolean_equal_location_cases() -> [(
    NativeTarget,
    [ScalarParameterLocation; 2],
    [ScalarParameterLocation; 2],
); 5] {
    [
        (
            NativeTarget::linux_x64(),
            [
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 16 },
                ScalarParameterLocation::IncomingStack { byte_offset: 24 },
            ],
        ),
        (
            NativeTarget::windows_x64(),
            [
                ScalarParameterLocation::Register(MachineRegister::X86Rcx),
                ScalarParameterLocation::Register(MachineRegister::X86Rdx),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 64 },
                ScalarParameterLocation::IncomingStack { byte_offset: 72 },
            ],
        ),
        (
            NativeTarget::uefi_x64(),
            [
                ScalarParameterLocation::Register(MachineRegister::X86Rcx),
                ScalarParameterLocation::Register(MachineRegister::X86Rdx),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 64 },
                ScalarParameterLocation::IncomingStack { byte_offset: 72 },
            ],
        ),
        (
            NativeTarget::linux_arm64(),
            [
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 0 },
                ScalarParameterLocation::IncomingStack { byte_offset: 8 },
            ],
        ),
        (
            NativeTarget::macos_arm64(),
            [
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 0 },
                ScalarParameterLocation::IncomingStack { byte_offset: 8 },
            ],
        ),
    ]
}
