// Omega plan adapters over Psi's source-owned intrinsic requirement view.
// Source declaration classification lives in typed trees, while provider schema
// and plan association remain here. Review's token-only float conformance lane
// (`closed_float_execution`) is narrower than the complete numeric requirement
// catalog (`execution_identity`).

use effects::provider_plan::{ProviderPlan, ProviderPlanRow, ServiceMethod};
use typed_trees::TypedTrees;
use typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement as SourceRequirement;
pub use typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirementKind;
pub use typed_trees::typed_trees::declarations::numeric_intrinsics::primitive_numeric_type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrinsicRequirement<'typed>(SourceRequirement<'typed>);

impl<'typed> std::ops::Deref for IntrinsicRequirement<'typed> {
    type Target = SourceRequirement<'typed>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'typed> IntrinsicRequirement<'typed> {
    pub fn into_source(self) -> SourceRequirement<'typed> {
        self.0
    }

    pub fn closed_float_execution(
        &self,
        typed: &TypedTrees,
    ) -> Option<effects::CompilerIntrinsicExecutionIdentity> {
        let meaning = typed.numeric_intrinsic_execution(self.symbol)?;
        matches!(
            meaning,
            language_semantics::numeric_intrinsics::NumericIntrinsic::FloatBinary { .. }
        )
        .then(|| meaning.into())
    }

    pub fn execution_identity(
        &self,
        typed: &TypedTrees,
    ) -> Option<effects::CompilerIntrinsicExecutionIdentity> {
        typed
            .numeric_intrinsic_execution(self.symbol)
            .map(Into::into)
    }

    pub fn from_operator(
        typed: &'typed TypedTrees,
        operator: &'typed typed_trees::operator::OperatorDefinition,
    ) -> Option<Self> {
        SourceRequirement::from_operator(typed, operator).map(Self)
    }

    pub fn by_symbol(typed: &'typed TypedTrees, symbol: symbols::SymbolHandle) -> Option<Self> {
        SourceRequirement::by_symbol(typed, symbol).map(Self)
    }

    pub fn plan_row_binds(
        &self,
        plan: &ProviderPlan,
        method: &ServiceMethod,
        row: &ProviderPlanRow,
    ) -> bool {
        effects::external_providers::RequirementPlanIdentity {
            overload: &self.requirement_identity,
            package: self.package_identity,
            schema: &self.schema_identity(),
            method: self.method_name(),
            owner: self.method_owner(),
        }
        .matches(plan, method, row)
    }
}
