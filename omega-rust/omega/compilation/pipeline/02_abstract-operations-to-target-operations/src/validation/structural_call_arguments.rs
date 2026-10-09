//! Reconcile retained embedded calls against source operations and callee identity.
//!
//! Every retained row in the in-module and installed-provider call families is
//! collected and replayed against the callee's independently reconstructed
//! signature: the embedded plan, scalar and structural argument rows, result
//! identity, and result home are all re-derived from the abstract
//! declarations. The retained target plan, its
//! self-reported placements, and any producer-side digest are never admission
//! authority. A retained row bound to no source call operation, or a second
//! row under an operation key that already has one, is forged by construction.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{
    AbstractBoundaryResult, AbstractCallSignature, AbstractCallTarget, AbstractFunction,
    AbstractFunctionResult, AbstractOperation, AbstractResult, AbstractStructuralCallResult,
    CompletionClaimSource,
};
use calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, NativePlace, ValueClass, ValueShape, evaluate_call_plan,
};
use semantic_vocabulary::{
    BoundaryMachineId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations::{
    BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, CompilerBuiltinExecution, NativeCallOrigin, NativeCallTarget,
    NormalizedForeignCallBinding, ProviderExecutionBinding, ScalarAbiValue, ScalarFunctionAbi,
    TargetBoundaryResult, TargetFunction, TargetNativeCallbackArgument, TargetReferenceResult,
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralHomeRequirement,
    TargetUnitOperation, TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument,
    TargetUnitScalarHomeRequirement,
};
use terminal_psi::{
    BoundaryMachineDeclaration, ClaimTransfer, CrashRouteBucket, StructuralAccess,
    StructuralArgument, StructuralFieldType, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalDynamicDispatchCatalog,
    find_structural_type,
};

mod canonical_roots;

use super::reference_results;
use super::structural_shapes;








