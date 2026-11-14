//! Keep the interfaces a program must satisfy with the program being judged.
//! Abstract interfaces have no executable body. These rows retain their full
//! logical namespace and operational bounds without reopening typed source.

use semantic_vocabulary::{
    BoundaryMachineId, CallableCrashPolicy, IdTable, MachineId, PackageKeyIdentity, Proposition,
    ScalarType, ServiceId, StructuralTypeId, ValueId,
};
use symbols::SymbolHandle;
use terminal_psi::{
    CrashRouteBucket, ExternalProviderConformance, TerminalCallableEnvelope,
    TerminalCallableInvocation, TerminalCallableTermination,
};

use crate::values::RigidType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ceiling {
    Inferred,
    Authored(Vec<ServiceId>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementCeilings {
    pub reaches: Ceiling,
    pub invokes: Ceiling,
}

#[derive(Debug, Clone, Default)]
pub struct ServiceReach {
    pub ceilings: IdTable<MachineId, Option<Vec<ServiceId>>>,
    /// Diagnostic names in the module's service order; never semantic keys.
    pub names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashCeiling {
    Authored(Vec<CrashRouteBucket>),
    Inferred,
    DeclaredNone,
}

#[derive(Debug, Clone)]
pub struct CallableHeader {
    pub scalar_parameters: Vec<terminal_psi::ValueDeclaration>,
    pub parameter_order: Vec<terminal_psi::BoundaryParameterKind>,
    pub structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    pub result: terminal_psi::TerminalMachineResult,
    pub proof: terminal_psi::MachineContract,
    pub crash_policy: CallableCrashPolicy,
}

/// A native declaration remains a header, never a fabricated executable body.
/// The source/state handles only associate questions from the same preparation;
/// every semantic check reads the canonical headers and conformance below.
#[derive(Debug, Clone)]
pub struct ExternalProviderCandidate {
    pub source: SymbolHandle,
    pub state: SymbolHandle,
    pub requirement: BoundaryMachineId,
    pub required: CallableHeader,
    pub supplied: CallableHeader,
    pub required_envelope: TerminalCallableEnvelope,
    pub supplied_envelope: TerminalCallableEnvelope,
    pub conformance: ExternalProviderConformance,
    pub completion: semantic_vocabulary::BoundaryCompletion,
    pub native_boundary: Option<BoundaryMachineId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticServiceCeiling {
    Open,
    Closed(Vec<ServiceId>),
}

#[derive(Debug, Clone)]
pub struct StaticCallableEnvelope {
    pub service_reach: StaticServiceCeiling,
    pub invocations: Vec<TerminalCallableInvocation>,
    pub suspends: bool,
    pub blocks: bool,
    pub termination: TerminalCallableTermination,
}

/// The carrier of one already-formed static interface slot. This describes the
/// logical proof namespace, not a runtime ABI. Conformance headers retain their
/// full canonical parameter/result qualifications separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticValueType {
    Scalar(ScalarType),
    Record(StructuralTypeId),
    Rigid(RigidType),
}

impl StaticValueType {
    pub fn scalar(self) -> Option<ScalarType> {
        match self {
            Self::Scalar(scalar) => Some(scalar),
            Self::Record(_) | Self::Rigid(_) => None,
        }
    }

    pub fn is_value(self) -> bool {
        matches!(self, Self::Scalar(_) | Self::Rigid(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticBorrowed {
    pub position: usize,
    pub access: language_core::ReferenceAccess,
}

#[derive(Debug, Clone)]
pub struct StaticSignature {
    pub parameters: Vec<StaticValueType>,
    pub erased: Vec<usize>,
    pub borrowed: Vec<StaticBorrowed>,
    pub result: Option<StaticValueType>,
    pub requires: Vec<Proposition>,
    pub ensures: Vec<Proposition>,
    pub crashes: CrashCeiling,
}

impl StaticSignature {
    pub fn is_erased(&self, position: usize) -> bool {
        self.erased.binary_search(&position).is_ok()
    }

    pub fn borrowed(&self, position: usize) -> Option<&StaticBorrowed> {
        self.borrowed
            .binary_search_by_key(&position, |borrowed| borrowed.position)
            .ok()
            .and_then(|position| self.borrowed.get(position))
    }

    pub fn is_place(&self, position: usize) -> bool {
        self.borrowed(position).is_some()
            || matches!(
                self.parameters.get(position),
                Some(StaticValueType::Record(_))
            )
    }

    pub fn formal_values(&self) -> Vec<Option<ValueId>> {
        formal_value_ids(
            self.parameters.iter().enumerate().map(|(position, value)| {
                (
                    value.is_value(),
                    self.is_erased(position),
                    self.borrowed(position).is_some(),
                )
            }),
            self.result.is_some_and(StaticValueType::is_value),
        )
    }
}

/// One value numbering for source preparation and static interfaces. Runtime
/// value formals come first, then the value result, then erased value formals.
/// Borrowed and structural formals belong to the place namespace instead.
pub fn formal_value_ids(
    formals: impl Iterator<Item = (bool, bool, bool)> + Clone,
    value_result: bool,
) -> Vec<Option<ValueId>> {
    let relevant = formals
        .clone()
        .filter(|(value, erased, borrowed)| *value && !erased && !borrowed)
        .count() as u64;
    let (mut next_relevant, mut next_erased) = (0, relevant + u64::from(value_result));
    formals
        .map(|(value, erased, borrowed)| {
            if borrowed || !value {
                return None;
            }
            let next = if erased {
                &mut next_erased
            } else {
                &mut next_relevant
            };
            *next += 1;
            ValueId::new(*next)
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct StaticRequirementSchema {
    pub declaration: SymbolHandle,
    pub abstract_types: u32,
    pub signature: StaticSignature,
    pub envelope: StaticCallableEnvelope,
}

#[derive(Debug, Clone)]
pub struct StaticSchemaQuestion {
    pub declaration: SymbolHandle,
    pub requirements: Vec<StaticRequirementSchema>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticImplementation {
    Machine(MachineId),
    Native {
        source: SymbolHandle,
        candidate: usize,
    },
}

#[derive(Debug, Clone)]
pub struct StaticConformanceQuestion {
    pub declaration: SymbolHandle,
    pub requirement: SymbolHandle,
    pub application: String,
    pub implementation: StaticImplementation,
    pub required: CallableHeader,
    pub envelope: StaticCallableEnvelope,
    pub provider: Option<StaticProviderCandidate>,
}

#[derive(Debug, Clone)]
pub struct StaticProviderCandidate<Declaration = terminal_psi::NominalDeclarationIdentity> {
    pub boundary: BoundaryMachineId,
    pub declaration: Declaration,
    pub provider_identity: String,
    pub source_requirement: String,
    pub source_candidate: String,
    pub package: Option<PackageKeyIdentity>,
}
