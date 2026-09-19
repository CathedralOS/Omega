//! Scalar return plans for structural, trait-operator and boundary machines,
//! with their integer bounds and parameter plans.

use crate::CheckedScalarExpression;
use crate::checked_trees::flow::terminal::{
    CheckedBoundaryMachinePlan, CheckedScalarBinding, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitNominalAffineCallerRequirementPlan,
    CheckedUnitNominalAffineCleanupPlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypePlan,
};
use language_semantics::{ServiceReachPlan, ServiceReachSummary};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

/// Complete checked input for the first scalar-returning structural cleanup
/// producer. The runtime value plan remains in `CheckedScalarExpressionPlans`;
/// this row binds it to the exact affine structural entry frontier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralScalarReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub machines: Vec<CheckedStructuralScalarReturnMachinePlan>,
    /// Boundary-operator applications whose checked realization consumes an
    /// exact structural frontier and returns one scalar. Selection and the
    /// authored application stay explicit; this is not rediscovered from the
    /// rewritten ordinary call expression.
    pub selected_operator_machines: Vec<CheckedSelectedOperatorStructuralScalarReturnMachinePlan>,
    /// Direct trait-backed fixed-token returns. These stay separate from the
    /// builtin scalar-expression lane because the selected realization is an
    /// executable structural call, not a primitive comparison.
    pub trait_operator_machines: Vec<CheckedTraitOperatorScalarReturnMachinePlan>,
}

impl CheckedStructuralScalarReturnPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedStructuralScalarReturnMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn trait_operator_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedTraitOperatorScalarReturnMachinePlan> {
        self.trait_operator_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }

    pub fn selected_operator_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedSelectedOperatorStructuralScalarReturnMachinePlan> {
        self.selected_operator_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }
}

/// One exact selected boundary-operator return over whole structural
/// parameters. The provider plan and concrete checked realization cross the
/// checked boundary explicitly so Terminal can retain the authored D29 use
/// while emitting an ordinary structural scalar call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSelectedOperatorStructuralScalarReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub result_type: PrimitiveType,
    pub return_statement_ordinal: u32,
    pub requirement_operator: SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: crate::CheckedProviderPlanCommitment,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_contract_report_fingerprint: u64,
    pub realization_contract_commitment: crate::MachineContractCommitment,
    pub service_reach: ServiceReachSummary,
    /// Authored source-parameter positions in fixed-token operand order.
    pub argument_source_positions: Vec<u32>,
}

/// One exact trait-backed fixed-token return over whole structural parameters.
/// The selected closed application and realization row cross the checked
/// boundary explicitly; Terminal lowering must not rediscover either from
/// names or visible conformances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTraitOperatorScalarReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: Option<String>,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub result_type: PrimitiveType,
    pub return_statement_ordinal: u32,
    pub conformance: SymbolHandle,
    /// Compact report coordinate; authority uses the adjacent commitment.
    pub conformance_application_report_fingerprint: u64,
    pub conformance_application_commitment:
        typed_trees::typed_trees::ClosedConformanceApplicationCommitment,
    pub requirement: SymbolHandle,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    /// Source-independent checked body of the exact selected realization.
    /// This is retained here because an unselected conformance member is not
    /// otherwise part of the ordinary terminal scalar-expression roots.
    pub realization_return_expression: CheckedScalarExpression,
    /// Authored source-parameter positions in fixed-token operand order.
    pub argument_source_positions: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralScalarReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// Free bodies have no attachment; attached bodies retain their exact owner.
    pub attachment_type_identity: Option<String>,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Dense scalar input order with each entry's original source position.
    /// Together with `structural_parameters`, this must exactly partition the
    /// authored state-parameter positions.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Proof-only erased scalar formals in authored order, retaining their
    /// authored parameter positions. They own no runtime argument lane.
    pub erased_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Immutable primitive bindings evaluated in source order. Initializer
    /// expressions remain in `CheckedScalarExpressionPlans` at the binding's
    /// exact statement coordinate.
    pub bindings: Vec<CheckedScalarBinding>,
    /// Ordered effects after the binding prefix and before return evaluation.
    /// Each effect retains its exact authored statement coordinate.
    pub effects: Vec<CheckedUnitEffectOperationPlan>,
    pub result_type: PrimitiveType,
    pub return_statement_ordinal: u32,
    /// One bounded actual CFG convergence: a single finite `!`/`&&`/`||`
    /// binding over a finite nonempty set of runtime Boolean inputs has typed
    /// value leaves entering one shared direct return/cleanup block. Boolean
    /// equality with a constant is normalized to identity/negation. One direct
    /// relevant Boolean field identity on one nominal-cleanup root is also
    /// admitted. Integer-comparison leaves separately accept scalar parameters
    /// and landed constants beneath up to two total binary, bitwise-not, or
    /// integer-widening shells, or one proof-bearing exact-cast, exact-add,
    /// exact-subtract, exact-multiply, exact shift, exact-divide, or
    /// exact-remainder computation shell. Proof-bearing parameter bounds remain
    /// explicit.
    /// Nested or multiple field identities, member/comparison mixtures, wider
    /// integer computations, and richer leaves retain the source-distributed
    /// fallback and publish `None`.
    pub shared_boolean_convergence: Option<CheckedStructuralBooleanConvergencePlan>,
    /// Complete canonical direct-Boolean caller facts preserved at the closed
    /// scalar return edge. Nominal cleanup actions select root-local subsets
    /// by `source_parameter_index`; no-code actions consume no premise.
    pub caller_requirements: Vec<CheckedUnitNominalAffineCallerRequirementPlan>,
    /// Bounded scalar premises retained from the authored contract. This slice
    /// admits direct fixed-width integer parameter bounds and pairwise
    /// parameter relations so proof-bearing exact arithmetic can be
    /// reconstructed terminally.
    pub scalar_requirements: Vec<CheckedStructuralScalarIntegerBoundRequirementPlan>,
    /// Complete post-result cleanup stream in reverse authored parameter
    /// order. Keeping trivial and nominal actions in one list prevents either
    /// representation or a later producer from losing their relative order.
    pub cleanup_actions: Vec<CheckedStructuralScalarReturnCleanupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralScalarIntegerBoundRequirementPlan {
    /// Dense position in this plan's scalar parameter namespace.
    pub parameter_position: u32,
    pub primitive_type: PrimitiveType,
    pub kind: CheckedStructuralScalarIntegerBoundKind,
    pub bound: CheckedStructuralScalarIntegerBoundPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralScalarIntegerBoundPlan {
    Literal(numerics::literals::IntegerLiteral),
    /// Dense position in the same scalar parameter namespace.
    Parameter(u32),
    /// The maximum of this fixed-width carrier minus the named dense parameter.
    MaximumMinusParameter(u32),
    /// The minimum of this signed carrier minus the named dense parameter.
    SignedMinimumMinusParameter(u32),
    /// The minimum of this signed carrier plus the named dense parameter.
    SignedMinimumPlusParameter(u32),
    /// The maximum of this signed carrier plus the named dense parameter.
    SignedMaximumPlusParameter(u32),
    /// The maximum of this fixed-width carrier divided by the named dense parameter.
    MaximumDivideParameter(u32),
    /// The minimum of this signed carrier divided by the named dense parameter.
    SignedMinimumDivideParameter(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralScalarIntegerBoundKind {
    Lower,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralBooleanConvergencePlan {
    pub binding_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralScalarReturnCleanupAction {
    DiscardRoot(u32),
    InvokeNominal(CheckedUnitNominalAffineCleanupPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralScalarParameterPlan {
    pub source_position: u32,
    pub primitive_type: PrimitiveType,
}

/// Source-handle-free checked plans for the first result-bearing bodyless
/// boundary slice. One successful boundary invocation returns a primitive
/// scalar. When structural custody is present, the invocation also consumes
/// the complete claim frontier carried by its arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedBoundaryScalarReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub boundary_machines: Vec<CheckedBoundaryMachinePlan>,
    pub machines: Vec<CheckedBoundaryScalarReturnMachinePlan>,
}

impl CheckedBoundaryScalarReturnPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedBoundaryScalarReturnMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryScalarReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Dense scalar order, with each parameter retaining its authored position.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Proof-only erased scalar formals in authored order, retaining their
    /// authored parameter positions. They own no runtime argument lane.
    pub erased_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    pub boundary_call: CheckedUnitEffectOperationPlan,
    pub result_type: PrimitiveType,
    pub return_statement_ordinal: u32,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
}
