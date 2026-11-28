//! Identify the exact built-in operation selected for a native boundary.
//! Receipts and lowering use the same bytes for that operation, including
//! its numeric formats or the field placement for a scalar accessor.
#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerIntrinsicExecutionIdentity {
    FixedActivation(language_core::sealed_intrinsics::ActivationOperation),
    StaticInterrupt(language_core::static_interrupts::StaticInterruptOperation),
    HostedExitProcessI32,
    TaskRegisterActivate,
    TaskRegisterRead,
    SegmentReload(terminal_psi::SegmentReload),
    SegmentRead(terminal_psi::SegmentRegister),
    HostedWriteByteI32,
    HostedReadByte,
    ProgramImageRetainedSuffixStart,
    ProgramStorageEntry,
    AccessLifecycle(terminal_psi::TerminalAccessLifecycleOperation),
    EmptyRunRelease,
    AccessorScalarAccess {
        contract: [u8; 32],
        placement_commitment: [u8; 32],
        access_slot: u32,
        operation: CompilerAccessorScalarOperation,
    },
    BuiltinFunction(language_core::BuiltinFunction),
    PrimitiveFloatBinary {
        operation: CompilerPrimitiveFloatBinaryOperation,
        format: numerics::intrinsics::FloatFormat,
    },
    PrimitiveIntegerComparison {
        operation: CompilerPrimitiveIntegerComparisonOperation,
        integer_type: CompilerNumericType,
    },
    NamedFloatNegation(numerics::intrinsics::FloatFormat),
    NamedFloatConversion {
        source: CompilerNumericType,
        target: CompilerNumericType,
        domain: numerics::arithmetic::ArithmeticDomain,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerAccessorScalarOperation {
    Read,
    Take,
    Write,
}

impl From<numerics::intrinsics::NumericIntrinsic> for CompilerIntrinsicExecutionIdentity {
    fn from(meaning: numerics::intrinsics::NumericIntrinsic) -> Self {
        use numerics::intrinsics::NumericIntrinsic as Meaning;
        match meaning {
            Meaning::Builtin { function, .. } => Self::BuiltinFunction(function),
            Meaning::FloatBinary { operation, format } => {
                Self::PrimitiveFloatBinary { operation, format }
            }
            Meaning::IntegerComparison {
                operation,
                integer_type,
            } => Self::PrimitiveIntegerComparison {
                operation,
                integer_type,
            },
            Meaning::Negation(format) => Self::NamedFloatNegation(format),
            Meaning::Conversion {
                source,
                target,
                domain,
            } => Self::NamedFloatConversion {
                source,
                target,
                domain,
            },
        }
    }
}

impl CompilerIntrinsicExecutionIdentity {
    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::PrimitiveFloatBinary { .. }
                | Self::PrimitiveIntegerComparison { .. }
                | Self::NamedFloatNegation(_)
                | Self::NamedFloatConversion { .. }
        ) || matches!(self, Self::BuiltinFunction(function) if function.is_float_family())
    }

    pub const fn checked_register(operation: terminal_psi::RegisterOperation) -> Option<Self> {
        use language_core::BuiltinFunction as B;
        use language_core::inline_assembly::AsmControlRegister as C;
        use terminal_psi::{RegisterOperation as O, SystemRegister as R};
        let function = match operation {
            O::TaskRegisterActivate => return Some(Self::TaskRegisterActivate),
            O::TaskRegisterRead => return Some(Self::TaskRegisterRead),
            O::SegmentReload(kind) => return Some(Self::SegmentReload(kind)),
            O::SegmentRead(register) => return Some(Self::SegmentRead(register)),
            O::ProcessorHalt => B::AsmHlt,
            O::MemoryFence(kind) => match kind {
                terminal_psi::MemoryFenceKind::Load => B::AsmLoadFence,
                terminal_psi::MemoryFenceKind::Store => B::AsmStoreFence,
                terminal_psi::MemoryFenceKind::Full => B::AsmFullFence,
                terminal_psi::MemoryFenceKind::Aarch64Load => B::AsmAarch64LoadFence,
                terminal_psi::MemoryFenceKind::Aarch64Store => B::AsmAarch64StoreFence,
                terminal_psi::MemoryFenceKind::Aarch64Full => B::AsmAarch64FullFence,
            },
            O::InterruptControl(
                language_core::inline_assembly::AsmInterruptControlKind::Disable,
            ) => B::AsmDisableInterrupts,
            O::InterruptControl(
                language_core::inline_assembly::AsmInterruptControlKind::Enable,
            ) => B::AsmEnableInterrupts,
            O::GlobalDescriptorTable(terminal_psi::GlobalDescriptorTableOperation::Store) => {
                B::AsmStoreGdtr
            }
            O::GlobalDescriptorTable(terminal_psi::GlobalDescriptorTableOperation::Load) => {
                B::AsmLoadGdtr
            }
            O::PortRead => B::AsmPortIn,
            O::PortWrite => B::AsmPortOut,
            O::ModelSpecificRead => B::AsmReadMsr,
            O::ModelSpecificWrite => B::AsmWriteMsr,
            O::SystemRead(register) => match register {
                R::Control(C::Cr0) => B::AsmReadCr0,
                R::Control(C::Cr2) => B::AsmReadCr2,
                R::Control(C::Cr3) => B::AsmReadCr3,
                R::Control(C::Cr4) => B::AsmReadCr4,
                R::SctlrEl1 => B::AsmReadSctlrEl1,
                R::TcrEl1 => B::AsmReadTcrEl1,
                R::Ttbr0El1 => B::AsmReadTtbr0El1,
                R::Ttbr1El1 => B::AsmReadTtbr1El1,
                R::MairEl1 => B::AsmReadMairEl1,
                R::VbarEl1 => B::AsmReadVbarEl1,
                R::TpidrEl1 => B::AsmReadTpidrEl1,
                R::EsrEl1 => B::AsmReadEsrEl1,
                R::FarEl1 => B::AsmReadFarEl1,
                R::Rflags => B::AsmSnapshotFlags,
            },
            O::SystemWrite(register) => match register {
                R::Control(C::Cr0) => B::AsmWriteCr0,
                R::Control(C::Cr3) => B::AsmWriteCr3,
                R::Control(C::Cr4) => B::AsmWriteCr4,
                R::Control(C::Cr2) => return None,
                R::SctlrEl1 => B::AsmWriteSctlrEl1,
                R::TcrEl1 => B::AsmWriteTcrEl1,
                R::Ttbr0El1 => B::AsmWriteTtbr0El1,
                R::Ttbr1El1 => B::AsmWriteTtbr1El1,
                R::MairEl1 => B::AsmWriteMairEl1,
                R::VbarEl1 => B::AsmWriteVbarEl1,
                R::TpidrEl1 => B::AsmWriteTpidrEl1,
                R::Rflags => B::AsmRestoreFlags,
                R::EsrEl1 | R::FarEl1 => return None,
            },
        };
        Some(Self::BuiltinFunction(function))
    }
}

pub fn compiler_accessor_scalar_contract_identity(
    schema: semantic_vocabulary::provider_identity::ServiceSchemaDigest,
    placement_commitment: [u8; 32],
    access_slot: u32,
    operation: CompilerAccessorScalarOperation,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"omega.sealed-placed-scalar-catalog.v1\0");
    hash.update(schema.as_bytes());
    hash.update(placement_commitment);
    hash.update(access_slot.to_be_bytes());
    hash.update([match operation {
        CompilerAccessorScalarOperation::Read => 0,
        CompilerAccessorScalarOperation::Take => 1,
        CompilerAccessorScalarOperation::Write => 2,
    }]);
    hash.finalize().into()
}

// The first byte identifies the operation family in both native receipt and
// boundary coverage identities. Keep published discriminants stable.
#[repr(u8)]
enum IntrinsicTag {
    HostedExitProcessI32 = 0,
    BuiltinFunction = 1,
    PrimitiveFloatBinary = 2,
    NamedFloatNegation = 3,
    NamedFloatConversion = 4,
    HostedWriteByteI32 = 5,
    HostedReadByte = 6,
    PrimitiveIntegerComparison = 7,
    AccessorScalarAccess = 8,
    ProgramImageRetainedSuffixStart = 9,
    ProgramStorageEntry = 10,
    AccessLifecycle = 11,
    TaskRegisterActivate = 12,
    TaskRegisterRead = 13,
    SegmentReload = 14,
    SegmentRead = 15,
    // Tag 16 formerly meant either interrupt installation or empty-run release.
    StaticInterrupt = 17,
    EmptyRunRelease = 18,
    FixedActivation = 19,
}

pub fn compiler_intrinsic_execution_identity_bytes(
    identity: CompilerIntrinsicExecutionIdentity,
) -> Vec<u8> {
    use CompilerIntrinsicExecutionIdentity as Execution;

    let mut bytes = vec![0_u8; 8];
    let tag = match identity {
        Execution::StaticInterrupt(operation) => {
            bytes[1] = operation.code();
            IntrinsicTag::StaticInterrupt
        }
        Execution::TaskRegisterActivate => IntrinsicTag::TaskRegisterActivate,
        Execution::TaskRegisterRead => IntrinsicTag::TaskRegisterRead,
        Execution::SegmentReload(kind) => {
            bytes[1] = kind.code();
            IntrinsicTag::SegmentReload
        }
        Execution::SegmentRead(register) => {
            bytes[1] = register.code();
            IntrinsicTag::SegmentRead
        }
        Execution::HostedExitProcessI32 => IntrinsicTag::HostedExitProcessI32,
        Execution::HostedWriteByteI32 => IntrinsicTag::HostedWriteByteI32,
        Execution::HostedReadByte => IntrinsicTag::HostedReadByte,
        Execution::ProgramImageRetainedSuffixStart => IntrinsicTag::ProgramImageRetainedSuffixStart,
        Execution::ProgramStorageEntry => IntrinsicTag::ProgramStorageEntry,
        Execution::EmptyRunRelease => IntrinsicTag::EmptyRunRelease,
        Execution::FixedActivation(operation) => {
            use language_core::sealed_intrinsics::ActivationOperation as Op;
            bytes[1] = match operation {
                Op::Prepare => 0,
                Op::StepFixed => 1,
                Op::Park => 2,
                Op::Resume => 3,
            };
            IntrinsicTag::FixedActivation
        }
        Execution::AccessLifecycle(operation) => {
            bytes[1] = match operation {
                terminal_psi::TerminalAccessLifecycleOperation::Initialize => 0,
                terminal_psi::TerminalAccessLifecycleOperation::MoveOut => 1,
                terminal_psi::TerminalAccessLifecycleOperation::View => 2,
                terminal_psi::TerminalAccessLifecycleOperation::Relocate => 3,
            };
            IntrinsicTag::AccessLifecycle
        }
        Execution::AccessorScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => {
            bytes[1] = match operation {
                CompilerAccessorScalarOperation::Read => 0,
                CompilerAccessorScalarOperation::Take => 1,
                CompilerAccessorScalarOperation::Write => 2,
            };
            bytes.extend_from_slice(&contract);
            bytes.extend_from_slice(&placement_commitment);
            bytes.extend_from_slice(&access_slot.to_be_bytes());
            IntrinsicTag::AccessorScalarAccess
        }
        Execution::BuiltinFunction(function) => {
            bytes[1..5].copy_from_slice(&(function.ordinal() as u32).to_be_bytes());
            IntrinsicTag::BuiltinFunction
        }
        Execution::PrimitiveFloatBinary { operation, format } => {
            bytes[1] = primitive_float_operation_tag(operation);
            bytes[2] = float_format_tag(format);
            IntrinsicTag::PrimitiveFloatBinary
        }
        Execution::PrimitiveIntegerComparison {
            operation,
            integer_type,
        } => {
            bytes[1] = primitive_integer_comparison_operation_tag(operation);
            bytes.extend_from_slice(&integer_type.canonical_encoding());
            IntrinsicTag::PrimitiveIntegerComparison
        }
        Execution::NamedFloatNegation(format) => {
            bytes[1] = float_format_tag(format);
            IntrinsicTag::NamedFloatNegation
        }
        Execution::NamedFloatConversion {
            source,
            target,
            domain,
        } => {
            bytes[3] = arithmetic_domain_tag(domain);
            bytes.extend_from_slice(&source.canonical_encoding());
            bytes.extend_from_slice(&target.canonical_encoding());
            IntrinsicTag::NamedFloatConversion
        }
    };
    bytes[0] = tag as u8;
    bytes
}

const fn float_format_tag(format: numerics::intrinsics::FloatFormat) -> u8 {
    match format {
        numerics::intrinsics::FloatFormat::F32 => 0,
        numerics::intrinsics::FloatFormat::F64 => 1,
    }
}

const fn arithmetic_domain_tag(domain: numerics::arithmetic::ArithmeticDomain) -> u8 {
    match domain {
        numerics::arithmetic::ArithmeticDomain::Exact => 0,
        numerics::arithmetic::ArithmeticDomain::Wrapping => 1,
        numerics::arithmetic::ArithmeticDomain::Saturating => 2,
        numerics::arithmetic::ArithmeticDomain::Trapping => 3,
    }
}

pub use numerics::intrinsics::NumericType as CompilerNumericType;
pub use numerics::intrinsics::PrimitiveFloatBinaryOperation as CompilerPrimitiveFloatBinaryOperation;
pub use numerics::intrinsics::PrimitiveIntegerComparisonOperation as CompilerPrimitiveIntegerComparisonOperation;
pub use numerics::intrinsics::primitive_float_operation_tag;
pub use numerics::intrinsics::primitive_integer_comparison_operation_tag;

#[cfg(test)]
mod tests {
    use super::{
        CompilerIntrinsicExecutionIdentity as Intrinsic,
        compiler_intrinsic_execution_identity_bytes,
    };
    use language_core::static_interrupts::StaticInterruptOperation;

    #[test]
    fn interrupt_and_release_identities_retire_the_ambiguous_encoding() {
        let retired = [16, 0, 0, 0, 0, 0, 0, 0];
        let operations = StaticInterruptOperation::ALL
            .into_iter()
            .map(Intrinsic::StaticInterrupt)
            .chain([Intrinsic::EmptyRunRelease]);
        let mut seen = std::collections::BTreeSet::new();
        for operation in operations {
            let encoded = compiler_intrinsic_execution_identity_bytes(operation);
            assert_ne!(encoded, retired);
            assert!(seen.insert(encoded), "duplicate identity for {operation:?}");
        }
    }
}
