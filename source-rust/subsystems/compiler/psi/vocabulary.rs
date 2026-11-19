/*
Root `TerminalModule` aggregate combining interned identities, structural and scalar domain
declarations, mathematical proof declarations, boundary requirements, executable `TerminalMachine`
bodies, folded constants, quotients, scalar applications, and task-activation descriptors.
*/

pub use crate::types_and_values::{types, values};
pub use crate::{boundary, control_flow, identity, ownership, proof};

pub use boundary::*;
#[path = "types/folded_constants.rs"]
pub(crate) mod folded_constants;
pub use folded_constants::*;
#[path = "evidence/proof_application_scope.rs"]
pub mod proof_application_scope;
#[path = "types/scalar_applications.rs"]
pub mod scalar_applications;
#[path = "types/scalar_conditions.rs"]
pub mod scalar_conditions;
pub use control_flow::{
    AtomicAccessEvent, AtomicReadModifyWrite, Block, CallableInvocation, CallableInvocationInputs,
    CrashCause, ElementViewRegion, FenceOrdering, FloatResultPolicy, MemoryOrdering,
    ModuleInvocation, ModuleInvocationInputs, Operation, OperationKind, OperationResult,
    RecordFieldInitializer, RecordFieldValue, ReferenceStorageView, RegisterAccessEvent,
    ScalarCaseField, StructuralCaseSuccessorEdge, StructuralOperationResult,
    StructuralResultClaimBinding, StructuralStoreDisposition, SuccessorEdge, TargetMachineCallable,
    TerminalBlockNaturalRank, TerminalIntegerRankValue, TerminalMachine, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc, Terminator,
    TrappingIntegerOperation, TrappingIntegerPrimitive, structural_parameter_position,
};
pub use identity::*;
pub use ownership::*;
pub use proof::*;
pub use proof_application_scope::{
    DeclaredSynchronousBoundaryTarget, ProofApplicationScope, ProofCallableAssociation,
    ProofCallableHeader, ProofCheckedRecursiveCall, ProofCheckedRecursiveComponent,
    ProofDeclarationId, ProofScopeDeclaration,
};
pub use scalar_applications::{
    ArrayExtent, LayoutMeasurement, ScalarApplication, ScalarApplications,
};
pub use scalar_conditions::{
    ScalarCompletionCondition, ScalarConditionSubject, ScalarConditions, ScalarPredicateCondition,
    ScalarPredicateNode, ScalarRangeCondition, ScalarRangeObservation, ScalarRangeRequirement,
};
#[path = "types/quotients.rs"]
pub mod quotients;
#[path = "machines/task_activation.rs"]
pub mod task_activation;
pub use task_activation::*;
pub use types::{
    ByteSequenceCarrier, ResultQualificationEstablishment, ScalarDomainDeclaration,
    ScalarDomainEstablishmentRoute, ScalarDomainPredicate, ScalarFieldQualification,
    ScalarFloatRange, ScalarIntegerRange, ScalarPrimitiveQualification, ScalarQualificationCatalog,
    ScalarQualificationCoercion, ScalarQualificationSet, ScalarReturnOrigin,
    ScalarReturnQualification, ScalarReturnSite, StructuralCaseDeclaration,
    StructuralContentProjection, StructuralDomainAccessPlan, StructuralDomainDeclaration,
    StructuralDomainPredicate, StructuralEstablishmentRoute, StructuralFieldDeclaration,
    StructuralFieldType, StructuralPathQualification, StructuralQualificationChange,
    StructuralReferentIdentity, StructuralTypeDeclaration, StructuralTypeShape, field_by_identity,
    find_structural_case, find_structural_type, live_field, live_field_by_id,
    live_field_by_identity, live_field_entry, structural_case_by_identity,
    structural_case_initializer_path, structural_case_position, structural_children,
    structural_type_by_identity, structural_type_contains_reference,
    structural_type_contains_reference_by, structural_type_has_borrowed_storage_by,
    unique_structural_declaration,
};
pub use values::*;


#[path = "evidence/theorems.rs"]
mod theorems;
pub use theorems::*;

pub use crate::Psi as TerminalModule;


#[path = "types/module_queries.rs"]
mod module_queries;
pub use module_queries::OpaqueStorageShapes;
