//! Values a receiver supplies at a Psi entry. Host addresses carry their
//! provenance, and structural inputs retain their exact path and qualifications.

use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue, ScalarType, StructuralDomainId,
    StructuralTypeId,
};
use terminal_psi::{StructuralPathSegment, TerminalAccessViewInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum TerminalScalarValue {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    IeeeFloat(IeeeFloatValue),
    Callable {
        contract: semantic_vocabulary::CallableContractId,
        descriptor: semantic_vocabulary::MachineDescriptorId,
    },
    /// A non-null `address` a native import returned. Its number is a real
    /// host pointer, not an interpreter storage identity, so it carries
    /// its provenance here: the interpreter may store, copy and compare
    /// it with other host addresses and pass it back to native imports,
    /// but refuses arithmetic, casts and comparison against its own
    /// addresses (spec: terminal-psi/boundary_calls.md#host-result-validation).
    HostAddress {
        scalar_type: IntegerType,
        address: u64,
    },
    /// A pure invocation computed NaN without determining a sign or payload.
    /// Copying the value preserves its meaning without creating observable bits.
    UndeterminedNaN(IeeeFloatFormat),
}

impl TerminalScalarValue {
    pub fn zero(scalar_type: ScalarType) -> Option<Self> {
        match scalar_type {
            ScalarType::Callable(_) => None,
            ScalarType::Boolean => Some(Self::Boolean(false)),
            ScalarType::Integer(integer_type) => Some(Self::Integer {
                scalar_type: integer_type,
                value: integer_type.zero(),
            }),
            ScalarType::IeeeFloat(format) => Some(Self::IeeeFloat(match format {
                IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
                IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
            })),
        }
    }

    pub fn satisfies_field(self, field: &terminal_psi::StructuralFieldType) -> bool {
        if let Self::Callable { contract, .. } = self {
            return matches!(field, terminal_psi::StructuralFieldType::Scalar(ScalarType::Callable(expected)) if *expected == contract);
        }
        if let Self::HostAddress { scalar_type, .. } = self {
            return matches!(field, terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(expected)) if *expected == scalar_type);
        }
        if let Self::UndeterminedNaN(format) = self {
            return matches!(field, terminal_psi::StructuralFieldType::IeeeFloat(expected)
                | terminal_psi::StructuralFieldType::Scalar(ScalarType::IeeeFloat(expected))
                if *expected == format);
        }
        self.literal()
            .is_some_and(|literal| crate::scalar_field_accepts_literal(field, &literal))
    }

    /// A literal is available only for scalars with an established representation.
    pub fn literal(self) -> Option<semantic_vocabulary::ScalarTerm> {
        Some(match self {
            Self::Boolean(value) => semantic_vocabulary::ScalarTerm::Boolean(value),
            Self::Integer { scalar_type, value } => {
                semantic_vocabulary::ScalarTerm::Integer { scalar_type, value }
            }
            Self::IeeeFloat(value) => semantic_vocabulary::ScalarTerm::IeeeFloatLiteral(value),
            Self::Callable { .. } | Self::HostAddress { .. } | Self::UndeterminedNaN(_) => {
                return None;
            }
        })
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::IeeeFloat(value) => ScalarType::IeeeFloat(value.format()),
            Self::UndeterminedNaN(format) => ScalarType::IeeeFloat(format),
            Self::Callable { contract, .. } => ScalarType::Callable(contract),
            Self::HostAddress { scalar_type, .. } => ScalarType::Integer(scalar_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct TerminalStructuralValue {
    pub opaque_identity: u64,
    pub structural_type: StructuralTypeId,
    pub qualifications: Vec<StructuralDomainId>,
    pub projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    pub path: Vec<StructuralPathSegment>,
}

impl TerminalStructuralValue {
    pub fn minted(
        opaque_identity: u64,
        structural_type: StructuralTypeId,
        qualifications: Vec<StructuralDomainId>,
    ) -> Self {
        Self {
            opaque_identity,
            structural_type,
            qualifications,
            projected_qualifications: Vec::new(),
            path: Vec::new(),
        }
    }

    pub fn absorb_qualifications(&mut self, source: &[StructuralDomainId]) {
        for domain in source {
            if !self.qualifications.contains(domain) {
                self.qualifications.push(*domain);
            }
        }
        self.qualifications.sort();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAccessViewEstablishment {
    pub input: TerminalAccessViewInput,
    pub referent: TerminalStructuralValue,
}
