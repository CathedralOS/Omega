//! Ordinary native-input vocabulary. Field and variant coverage is exhaustive.

use super::record_fields;

record_variants!(target::Architecture {
    0 => Aarch64,
    1 => X86_64,
});

record_variants!(target::ForeignLocatorCandidate {
    0 => PeByName { library, export },
    1 => PeByOrdinal { library, ordinal },
    2 => ElfVersioned { object, symbol, version },
    3 => MachODylibSymbol { install_name, symbol },
});

record_variants!(target::HostedApplicationIntent {
    0 => Console,
    1 => Gui,
});

record_fields!(target::NativeApplicationMetadata {
    name,
    intent,
    identifier,
    subsystem,
    elf_interpreter
});

record_fields!(target::NativeTarget {
    architecture,
    object_format,
    pointer_size,
    pointer_alignment
});

record_variants!(target::ObjectFormat {
    0 => Elf,
    1 => MachO,
    2 => Coff,
});

record_variants!(target::TargetProfile {
    0 => LinuxArm64,
    1 => LinuxX64,
    2 => MacosArm64,
    3 => MacosX64,
    4 => WindowsX64,
    5 => UefiX64,
    6 => CrossPlatformCli,
    7 => LocalUnchecked,
    8 => AlphaBootstrap,
});

record_variants!(target::X86TargetFeature {
    0 => Avx,
    1 => Fma3,
});

record_variants!(target::X86ScalarFmaSlot {
    0 => Binary32,
    1 => Binary64,
});

record_variants!(target::ProgramEntryPhysicalContractPackage {
    0 => UefiX64,
    1 => MacosArm64,
    2 => MacosX64,
    3 => LinuxX86_64,
    4 => LinuxArm64,
    5 => WindowsX64,
});
