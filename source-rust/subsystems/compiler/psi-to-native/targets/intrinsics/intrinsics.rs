//! Identify the exact built-in operation selected for a native boundary.
//! Receipts and lowering use the same bytes for that operation, including
//! its numeric formats or the field placement for a scalar accessor.
#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerIntrinsicExecutionIdentity {
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

pub fn compiler_intrinsic_execution_identity_bytes(
    identity: CompilerIntrinsicExecutionIdentity,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; 8];
    match identity {
        CompilerIntrinsicExecutionIdentity::StaticInterrupt(operation) => {
            bytes[0] = 16;
            bytes[1] = operation.code();
        }
        CompilerIntrinsicExecutionIdentity::TaskRegisterActivate => bytes[0] = 12,
        CompilerIntrinsicExecutionIdentity::TaskRegisterRead => bytes[0] = 13,
        CompilerIntrinsicExecutionIdentity::SegmentReload(kind) => {
            bytes[0] = 14;
            bytes[1] = kind.code();
        }
        CompilerIntrinsicExecutionIdentity::SegmentRead(register) => {
            bytes[0] = 15;
            bytes[1] = register.code();
        }
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => bytes[0] = 0,
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => bytes[0] = 5,
        CompilerIntrinsicExecutionIdentity::HostedReadByte => bytes[0] = 6,
        CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart => bytes[0] = 9,
        CompilerIntrinsicExecutionIdentity::ProgramStorageEntry => bytes[0] = 10,
        CompilerIntrinsicExecutionIdentity::EmptyRunRelease => bytes[0] = 16,
        CompilerIntrinsicExecutionIdentity::AccessLifecycle(operation) => {
            bytes[0] = 11;
            bytes[1] = match operation {
                terminal_psi::TerminalAccessLifecycleOperation::Initialize => 0,
                terminal_psi::TerminalAccessLifecycleOperation::MoveOut => 1,
                terminal_psi::TerminalAccessLifecycleOperation::View => 2,
                terminal_psi::TerminalAccessLifecycleOperation::Relocate => 3,
            };
        }
        CompilerIntrinsicExecutionIdentity::AccessorScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => {
            bytes[0] = 8;
            bytes[1] = match operation {
                CompilerAccessorScalarOperation::Read => 0,
                CompilerAccessorScalarOperation::Take => 1,
                CompilerAccessorScalarOperation::Write => 2,
            };
            bytes.extend_from_slice(&contract);
            bytes.extend_from_slice(&placement_commitment);
            bytes.extend_from_slice(&access_slot.to_be_bytes());
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            bytes[0] = 1;
            bytes[1..5].copy_from_slice(&(function.ordinal() as u32).to_be_bytes());
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            bytes[0] = 2;
            bytes[1] = primitive_float_operation_tag(operation);
            bytes[2] = float_format_tag(format);
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
            operation,
            integer_type,
        } => {
            bytes[0] = 7;
            bytes[1] = primitive_integer_comparison_operation_tag(operation);
            bytes.extend_from_slice(&integer_type.canonical_encoding());
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            bytes[0] = 3;
            bytes[1] = float_format_tag(format);
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } => {
            bytes[0] = 4;
            bytes[3] = arithmetic_domain_tag(domain);
            bytes.extend_from_slice(&source.canonical_encoding());
            bytes.extend_from_slice(&target.canonical_encoding());
        }
    }
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
