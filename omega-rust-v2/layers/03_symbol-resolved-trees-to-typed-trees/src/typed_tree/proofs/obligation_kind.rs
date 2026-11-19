use crate::{Access, CallableType, CrashCause, DomainApplication, RequirementSlot, Site, ValueType};
use omega_resolver::DeclarationId;

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
        contract: Box<CallableType>,
        machine: DeclarationId,
        arguments: Vec<ValueType>,
    },
    PositiveWidth(ValueType),
    /// A recursive type edge changes the arguments of a declaration with static formation requirements.
    FormRecursiveApplication {
        declaration: DeclarationId,
        arguments: Vec<ValueType>,
    },
    Termination(DeclarationId),
    TotalCrashGuard,
    AdmitMathematicalAssumption(DeclarationId),
    /// Linking a library machine requires a checked implementation of its published contract.
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
        place: omega_resolver::TreeNodeId,
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
