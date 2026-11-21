use crate::{Access, CallableType, CrashCause, DomainApplication, MachineApplication, RequirementSlot, Site, ValueType};
use omega_resolved::DeclarationId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObligationKind {
    RefineRequirement {
        requirement: RequirementSlot,
        implementation: DeclarationId,
    },
    EstablishDomain(DomainApplication),
    RefineMathematicalCitation {
        machine: DeclarationId,
        expected: ValueType,
    },
    RefineCallableValue {
        source: Site,
        destination: Site,
    },
    RefineCallable {
        /// Destination static formal, when the selection binds a generic argument.
        formal: Option<(DeclarationId, usize)>,
        contract: Box<CallableType>,
        selection: MachineApplication,
    },
    PositiveWidth(ValueType),
    /// A recursive type edge changes the arguments of a declaration with static formation requirements.
    FormRecursiveApplication {
        declaration: DeclarationId,
        /// The earlier occurrence of this declaration on the active formation path.
        active_arguments: Vec<ValueType>,
        arguments: Vec<ValueType>,
    },
    Termination(DeclarationId),
    TotalCrashGuard,
    AdmitMathematicalAssumption(DeclarationId),
    /// The toolchain's implementation of this bodyless library machine must establish its
    /// published guarantees. L3 records it at the library declaration, and every importing
    /// package receives the row unchanged.
    VerifyLibraryImplementation(DeclarationId),
    ProveGuarantee {
        machine: DeclarationId,
        predicate: Site,
    },
    FormSquash {
        source: ValueType,
        level: ValueType,
    },
    MathematicalFact {
        sort: ValueType,
    },
    UniverseMembership {
        subject: ValueType,
        sort: ValueType,
    },
    SupplyHypothesis {
        requirement: Site,
        evidence: ValueType,
        substitutions: Vec<ValueType>,
    },
    ApplyCallbackGuarantee {
        binder: ValueType,
        contract: Site,
        operands: Vec<Site>,
    },
    ApplyHypothesis {
        requirement: Site,
        operands: Vec<Site>,
    },
    CleanupContract(DeclarationId),
    EvaluateAccessPlan {
        domain: DeclarationId,
        machine: DeclarationId,
    },
    RefineEvidence {
        binder: RequirementSlot,
        implementation: DeclarationId,
    },
    CrashCoverage {
        machine: DeclarationId,
        cause: CrashCause,
    },
    OperationContract {
        token: String,
        operands: Vec<ValueType>,
    },
    ExactCast {
        source: ValueType,
        destination: ValueType,
    },
    EstablishData(DeclarationId),
    ZeroValue(ValueType),
    CopyRead(ValueType),
    /// Re-establish this exact place before its exclusive loan is released.
    RestoreBorrowed {
        place: omega_resolved::TreeNodeId,
        ty: ValueType,
    },
    Move {
        place: Site,
        ty: ValueType,
    },
    MathematicalApplication {
        definition: Option<DeclarationId>,
        generic_arguments: Vec<ValueType>,
        argument_count: usize,
    },
    Recast {
        source: ValueType,
        destination: ValueType,
    },
    Borrow {
        access: Access,
        place: Site,
    },
    ReturnBorrow {
        machine: DeclarationId,
    },
    BindLifetime {
        reference: ValueType,
        destination: ValueType,
    },
    CallPrecondition(DeclarationId),
    ZeroField {
        field: DeclarationId,
        ty: ValueType,
    },
}
