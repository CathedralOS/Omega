//! Names shared with resolution, typing and lowering.

use crate::{callables, definitions, expressions, proofs, storage, type_records};
pub(crate) use callables::{
    callable_applications, callable_copies, calls_and_control_flow, machine_specializations,
};
pub use callables::{
    callable_lifetimes, callable_signature_shapes, callable_values, runtime_callable_identity,
    source_call_occurrences, static_operand_binding,
};
pub use definitions::{
    compiler_intrinsics, declarations, native_locators, service_declarations, source_names,
    source_subjects,
};
pub(crate) use definitions::{
    declaration_reflection, declared_bindings, numeric_declarations, operator_declarations,
};
pub(crate) use expressions::access_views_and_commitments;
pub use expressions::{references, values};
pub(crate) use proofs::evidence_and_inspection;
pub use proofs::{contract_structure, deferred_refusals};
pub use storage::declaration_projection;
pub(crate) use storage::{
    declaration_tables_and_index, frozen, selection_exclusions, source_review, symbol_rebinding,
};
pub(crate) use type_records::source_type_applications;
pub use type_records::{boundary_values, data_applications, opaque_representations, type_identity, type_system};

pub use access_views_and_commitments::constant_recipes;
pub(crate) use access_views_and_commitments::{access_views, commitments};
pub use callable_applications::{
    AppliedMachineCallable, AppliedMachineCallableId, AppliedStaticCallbackRequirement,
    CallableApplication, CallableContractScope, CallableSubstitution, ClosedRequirementApplication,
    CopiedCallableBody, MachineInstantiation, NamedCaptureError, NamedConformanceCaptureIndex,
    RequirementLocalArgument,
};
pub use calls_and_control_flow::{calls, control_flow, names};
pub use data_definitions::DataDefinitionTable;
pub use declaration_reflection::reflection;
pub use declaration_tables_and_index::declaration_index;
pub(crate) use declaration_tables_and_index::{
    data_definitions, declaration_tables, machine_declarations, machine_parameter_signatures,
    spelled_declarations, type_parameters,
};
pub use evidence_and_inspection::evidence;
pub use expressions::constant_values::*;
pub use frozen::{FrozenTypedTrees, ProductSourceIdentity};
pub use machine_declarations::MachineDeclarationTable;
pub use machine_parameter_signatures::MachineParameterSignatureQueries;
pub use machine_specializations::MachineSpecializations;
pub use selection_exclusions::AuthoredSelectionExclusions;
pub use service_declarations::provider_declarations;
pub use source_review::{
    SourceCallCorrespondence, SourceFieldCorrespondence, SourceLayoutPlanCoordinate,
    SourceLayoutRealizationCoordinate, SourceNode, SourceRecordCleanup, SourceRecordCorrespondence,
    SourceServiceCorrespondence, SourceTypeApplication, SourceTypeScope,
};
pub use spelled_declarations::SpelledDeclarationTable;
pub use storage::{TypedTreeRoots, TypedTreeTables};
pub use symbol_rebinding::{PreparedSymbolRebinding, SymbolRebinding};
pub use type_parameters::TypeParameterTable;

pub use crate::declaration_selection::AuthoredDeclarationSelectionExposure;
pub use crate::declaration_selection::AuthoredDeclarationSelectionKind;
pub use crate::declaration_selection::AuthoredDeclarationSelectionLateBinding;
pub use crate::declaration_selection::AuthoredDeclarationSelectionTarget;
pub use crate::declaration_selection::ResolvedAuthoredDeclarationSelection;
pub use crate::source_selections::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionOccurrenceId,
    AuthoredDeclarationSelectionRecordError, AuthoredDeclarationSelections,
};
pub use access_views::{
    AccessAtomicObservingResultContract, AccessAtomicResidentContract, AccessBackingOutcome,
    AccessBackingTransfer, AccessContentShape, AccessExecutionDeclaration, AccessFieldPlan,
    AccessLifecycleOperation, AccessLifecycleOwner, AccessLifecyclePlan, AccessLifecycleStorage,
    AccessViewPlan, AccessorPrimitive, AccessorTarget, PlanLaidFieldLayoutSource, PlanLaidGeometry,
    PlanLaidLayout, PlanLaidRealizations, PlanLaidRetiredFieldIdentity, PlanLaidSchemaArgument,
    PlanLaidSchemaField, PlanLaidSchemaFieldKind, PlanLaidTargetLayout,
    PrivateCallbackSlotApplication, ScalarAccessorDeclaration, ScalarAccessorOperation,
    access_atomic_observing_results,
};
pub use calls::{signature, static_applications};
pub use commitments::{
    CapturedMachineApplication, CapturedMachineDeclaration, CapturedMachineState,
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceApplicationSite, ClosedConformanceConstArgument, ClosedConformanceRowIdentity,
    DataImplementationApplication, MachineSpecialization, StaticRequirementDispatch,
    TraitImplementationApplication,
};
pub use control_flow::{machine, state, statement};
pub use declarations::{
    boundary_completions, calling_vocabulary, constant, data, domain, invocations, layout_storage,
    measure, operator, product_operand, trait_definition, type_aliases, visibility, wire,
};
pub use evidence::{byte_predicates, mathematical, proposition, ranking};
pub use names::{identity, name};
pub use source_type_applications::SourceSliceApplication;
pub use type_system::{closed_numeric, types};
pub use values::const_integer;
pub use values::expression;
pub use values::subtrees::{SubtreeEdges, SubtreeVisit};
