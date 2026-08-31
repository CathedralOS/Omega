use std::collections::BTreeSet;

/// Closed compiler-owned execution child retained independently of authored
/// realization spelling, provider identity, and service reach.
///
/// This vocabulary is intentionally finite. A checked compiler intrinsic that
/// cannot be represented here has no closed terminal-mechanism identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerIntrinsicExecutionIdentity {
    /// Exact toolchain-owned `Console::exit_process(i32) -> Unit` execution
    /// selected for one canonical Linux target.
    LinuxExitGroupI32,
    BuiltinFunction(psi_symbols::BuiltinFunction),
    PrimitiveFloatBinary {
        operation: CompilerPrimitiveFloatBinaryOperation,
        format: psi_numerics::literals::FloatFormat,
    },
    NamedFloatNegation(psi_numerics::literals::FloatFormat),
    NamedFloatConversion {
        source: CompilerNumericType,
        target: CompilerNumericType,
        domain: psi_numerics::arithmetic::ArithmeticDomain,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerNumericType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

impl CompilerNumericType {
    pub const ALL: [Self; 10] = [
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::F32,
        Self::F64,
    ];

    pub const fn from_primitive(primitive: psi_typed_trees::types::PrimitiveType) -> Option<Self> {
        use psi_typed_trees::types::PrimitiveType;

        match primitive {
            PrimitiveType::I8 => Some(Self::I8),
            PrimitiveType::I16 => Some(Self::I16),
            PrimitiveType::I32 => Some(Self::I32),
            PrimitiveType::I64 => Some(Self::I64),
            PrimitiveType::U8 => Some(Self::U8),
            PrimitiveType::U16 => Some(Self::U16),
            PrimitiveType::U32 => Some(Self::U32),
            PrimitiveType::U64 => Some(Self::U64),
            PrimitiveType::F32 => Some(Self::F32),
            PrimitiveType::F64 => Some(Self::F64),
            PrimitiveType::Bool | PrimitiveType::Addr => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    pub const fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerPrimitiveFloatBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl CompilerPrimitiveFloatBinaryOperation {
    pub const ALL: [Self; 10] = [
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
        Self::Equal,
        Self::NotEqual,
        Self::Less,
        Self::LessOrEqual,
        Self::Greater,
        Self::GreaterOrEqual,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Multiply => "multiply",
            Self::Divide => "divide",
            Self::Equal => "equal",
            Self::NotEqual => "not_equal",
            Self::Less => "less",
            Self::LessOrEqual => "less_or_equal",
            Self::Greater => "greater",
            Self::GreaterOrEqual => "greater_or_equal",
        }
    }
}

/// D45's closed physical terminal-authority vocabulary. Declaration order is
/// the canonical encoded order; additions require a target-policy migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalAuthorityClass {
    FilesystemContentRead,
    FilesystemContentWrite,
    FilesystemMetadataQuery,
    DirectoryEnumeration,
    FilesystemNamespaceMutation,
    FilesystemMetadataMutation,
    ProcessOutput,
    ProcessTermination,
    MachineControl,
    PortIo,
    InterruptControl,
    InterruptEntry,
    RootMemoryAccess,
}

impl TerminalAuthorityClass {
    pub const ALL: [Self; 13] = [
        Self::FilesystemContentRead,
        Self::FilesystemContentWrite,
        Self::FilesystemMetadataQuery,
        Self::DirectoryEnumeration,
        Self::FilesystemNamespaceMutation,
        Self::FilesystemMetadataMutation,
        Self::ProcessOutput,
        Self::ProcessTermination,
        Self::MachineControl,
        Self::PortIo,
        Self::InterruptControl,
        Self::InterruptEntry,
        Self::RootMemoryAccess,
    ];

    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::FilesystemContentRead => 0,
            Self::FilesystemContentWrite => 1,
            Self::FilesystemMetadataQuery => 2,
            Self::DirectoryEnumeration => 3,
            Self::FilesystemNamespaceMutation => 4,
            Self::FilesystemMetadataMutation => 5,
            Self::ProcessOutput => 6,
            Self::ProcessTermination => 7,
            Self::MachineControl => 8,
            Self::PortIo => 9,
            Self::InterruptControl => 10,
            Self::InterruptEntry => 11,
            Self::RootMemoryAccess => 12,
        }
    }
}

/// One target-policy disposition for one exact terminal mechanism.
///
/// Classes are always stored in canonical order without duplicates. An empty
/// set means only that the mechanism exercises none of D45's classes; it does
/// not claim purity, trustworthiness, or absence of general side effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityDisposition {
    classes: Vec<TerminalAuthorityClass>,
}

impl TerminalAuthorityDisposition {
    pub fn from_classes(classes: impl IntoIterator<Item = TerminalAuthorityClass>) -> Self {
        let classes = classes
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self { classes }
    }

    pub fn classes(&self) -> &[TerminalAuthorityClass] {
        &self.classes
    }

    pub fn is_authority_class_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

/// Version and strong commitment for one complete receiving target-policy
/// table. This carrier is evidence identity only; accepting it remains the
/// receiving realization authority's decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAuthorityPolicyIdentity {
    version: u32,
    commitment: [u8; 32],
}

impl TerminalAuthorityPolicyIdentity {
    pub const fn from_parts(version: u32, commitment: [u8; 32]) -> Self {
        Self {
            version,
            commitment,
        }
    }

    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn commitment(self) -> [u8; 32] {
        self.commitment
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_authority_class_order_matches_canonical_tags() {
        for (index, class) in TerminalAuthorityClass::ALL.into_iter().enumerate() {
            assert_eq!(class.canonical_tag(), index as u8);
        }
    }

    #[test]
    fn disposition_classes_are_canonical_and_unique() {
        let disposition = TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::PortIo,
            TerminalAuthorityClass::ProcessTermination,
            TerminalAuthorityClass::PortIo,
            TerminalAuthorityClass::MachineControl,
        ]);
        assert_eq!(
            disposition.classes(),
            &[
                TerminalAuthorityClass::ProcessTermination,
                TerminalAuthorityClass::MachineControl,
                TerminalAuthorityClass::PortIo,
            ]
        );
    }
}
