//! Retained builtin proposals and their selected-plan custody.
//!
//! Derivation binds each proposal to its selected plan index and Terminal
//! boundary once; `settle_compiler_builtins` reads that binding and admits it
//! through the receiving policy and target catalog.

use super::super::NativeCompilerBuiltinSettlement;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderPlan, ServiceSchema};

/// A projected callable keeps its original semantic contract. Ordinary plans
/// already own that contract, so their proposals retain only the plan index.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CompilerIntrinsicSemanticSchema {
    SelectedPlan,
    Satisfied(ServiceSchema),
}

/// What derivation settled for one demanded intrinsic row. Derivation covers
/// the complete canonical program once and is shared by every entry (main and
/// each private callback), so it cannot know whether a demanded boundary is
/// inside any one entry's execution scope. A row it could not close is
/// therefore retained with its reason rather than refused: the entry whose
/// scope executes that boundary refuses it at admission with that reason, and
/// an entry whose scope never reaches it (a retained provider adapter nothing
/// calls) omits it with the rest of its out-of-scope occurrences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCompilerBuiltinSettlement {
    /// The catalog execution the local target lowerer admits.
    Execution(target_operations::CompilerBuiltinExecution),
    /// The selected row's execution has no closed native catalog identity.
    NoClosedCatalogIdentity,
    /// The closed identity has no native boundary realization in the catalog.
    NoNativeBoundaryRealization,
}

/// One compiler-owned builtin proposed beside a target-neutral Terminal
/// artifact: the demanded requirement, the selected plan holding its one
/// intrinsic row, the Terminal boundary it settles and what derivation
/// settled for it. The local lowerer still admits the execution through its
/// target catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCompilerBuiltinProposal {
    requirement_identity: String,
    provider_plan_index: usize,
    boundary: semantic_vocabulary::BoundaryMachineId,
    settlement: TerminalCompilerBuiltinSettlement,
    semantic_schema: CompilerIntrinsicSemanticSchema,
}

impl TerminalCompilerBuiltinProposal {
    pub(crate) fn new(
        requirement_identity: String,
        provider_plan_index: usize,
        boundary: semantic_vocabulary::BoundaryMachineId,
        settlement: TerminalCompilerBuiltinSettlement,
        semantic_schema: &ServiceSchema,
        selected_schema: &ServiceSchema,
    ) -> Self {
        let semantic_schema = if semantic_schema == selected_schema {
            CompilerIntrinsicSemanticSchema::SelectedPlan
        } else {
            CompilerIntrinsicSemanticSchema::Satisfied(semantic_schema.clone())
        };
        Self {
            requirement_identity,
            provider_plan_index,
            boundary,
            settlement,
            semantic_schema,
        }
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn provider_plan_index(&self) -> usize {
        self.provider_plan_index
    }

    pub const fn boundary(&self) -> semantic_vocabulary::BoundaryMachineId {
        self.boundary
    }

    pub const fn settlement(&self) -> TerminalCompilerBuiltinSettlement {
        self.settlement
    }

    fn semantic_schema<'request>(
        &'request self,
        provider_plan: &'request ProviderPlan,
    ) -> &'request ServiceSchema {
        match &self.semantic_schema {
            CompilerIntrinsicSemanticSchema::SelectedPlan => &provider_plan.schema,
            CompilerIntrinsicSemanticSchema::Satisfied(schema) => schema,
        }
    }
}

/// Borrow the exact selected plans for retained re-entry, preserving proposal
/// order, boundary binding and execution identity.
pub(crate) fn prepare_retained_compiler_builtins<'request>(
    builtins: &'request [TerminalCompilerBuiltinProposal],
    selected_plans: &'request [ProviderPlan],
) -> Result<Vec<NativeCompilerBuiltinSettlement<'request>>, Vec<Diagnostic>> {
    builtins
        .iter()
        .map(|builtin| {
            let provider_plan = selected_plans
                .get(builtin.provider_plan_index())
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "Terminal native proposal: compiler builtin `{}` names an absent selected provider plan",
                        builtin.requirement_identity(),
                    ))]
                })?;
            Ok(NativeCompilerBuiltinSettlement {
                requirement_identity: builtin.requirement_identity(),
                provider_plan,
                semantic_schema: builtin.semantic_schema(provider_plan),
                boundary: builtin.boundary(),
                settlement: builtin.settlement(),
            })
        })
        .collect()
}
