// Compiler-builtin proposal derivation, external provider owner validation,
// retained proposal custody, and Terminal reference intrinsic admission.
//
// `derive_selected_intrinsic_settlement_proposals` binds each called intrinsic
// requirement once to its selected plan index and Terminal boundary. Because
// derivation covers the complete canonical program once and is shared by main
// and each private callback, a row it cannot close is retained with its reason
// (`TerminalCompilerBuiltinSettlement`) so only an entry whose scope actually
// executes that boundary refuses it at admission.
//
// `validate_external_provider_owners` and `admit_terminal_intrinsics` rejoin
// the external provider declarations of retained requirements and admit closed
// numeric operations for an existing Terminal executor without granting host, device, or ISA feature
// authority.

use super::super::NativeCompilerBuiltinSettlement;
use super::SettledCompilerBuiltin;
use diagnostics::Diagnostic;
use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use provider_correspondence::external_providers::reconcile_numeric_meaning;
use provider_correspondence::selected_external_execution;
use provider_plans::{ProviderPlan, ServiceSchema};
use std::collections::BTreeSet;

pub use provider_correspondence::CompilerIntrinsicPlanProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompilerIntrinsicSemanticSchema {
    SelectedPlan,
    Satisfied(ServiceSchema),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCompilerBuiltinSettlement {
    Execution {
        identity: CompilerIntrinsicExecutionIdentity,
        execution: SettledCompilerBuiltin,
    },
    NoClosedCatalogIdentity,
    NoNativeBoundaryRealization,
}

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

pub fn prepare_retained_compiler_builtins<'request>(
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

pub fn derive_selected_intrinsic_settlement_proposals<'a>(
    selected: &'a selected_provider_plans::SelectedProviderPlanFacts,
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'a>>,
    demanded_boundaries: &BTreeSet<String>,
    module: &terminal_psi::TerminalModule,
    direct_leaves: &[direct_boundary_bindings::DirectBoundaryLeaf],
) -> Result<Vec<TerminalCompilerBuiltinProposal>, Vec<Diagnostic>> {
    let plans = selected.plans();
    let mut evidence = Vec::new();
    numeric_intrinsic_admission::selection::visit_selected_intrinsics(
        selected,
        provenance,
        demanded_boundaries,
        module,
        |requirement, selected_row, boundary, callable| {
            // The selected direct call already records its declaration and target
            // implementation. Validate that evidence here instead of asking an
            // external provider to supply it again.
            let mut direct = direct_leaves
                .iter()
                .filter(|leaf| leaf.plan == plans[selected_row.plan_index]);
            let direct = match (direct.next(), direct.next()) {
                (leaf, None) => leaf,
                _ => {
                    return Err(Diagnostic::error(
                        "compiler-builtin proposal has competing direct callable custody",
                    ));
                }
            };
            let identity = if let Some(leaf) = direct {
                leaf.validate_bound_calling_custody(module).and_then(|()| {
                    if &leaf.satisfied_schema != selected_row.semantic_schema
                        || leaf.execution
                            != direct_boundary_bindings::DirectBoundaryExecution::Intrinsic(
                                selected_row.execution,
                            )
                    {
                        return Err(
                            "direct callable settlement changes its retained schema or execution"
                                .into(),
                        );
                    }
                    Ok(selected_row.execution)
                })
            } else {
                selected_external_execution(
                    &module.external_providers,
                    &module.identities,
                    &callable.source.machine,
                    &plans[selected_row.plan_index],
                    selected_row.row,
                    selected_row.execution,
                    selected_row.external_realization,
                )
            }
            .map_err(|reason| {
                Diagnostic::error(format!("Terminal boundary `{requirement}` {reason}"))
            })?;
            reconcile_numeric_meaning(boundary.numeric_intrinsic, identity).map_err(|reason| {
                Diagnostic::error(format!("Terminal boundary `{requirement}` {reason}"))
            })?;
            let format = boundary
                .numeric_intrinsic
                .and_then(|meaning| meaning.builtin_format())
                .map(Into::into);
            let settlement = match identity {
                None => TerminalCompilerBuiltinSettlement::NoClosedCatalogIdentity,
                Some(
                    CompilerIntrinsicExecutionIdentity::AccessLifecycle(_)
                    | CompilerIntrinsicExecutionIdentity::EmptyRunRelease
                    | CompilerIntrinsicExecutionIdentity::StaticInterrupt(_),
                ) => TerminalCompilerBuiltinSettlement::NoNativeBoundaryRealization,
                Some(identity) => {
                    match SettledCompilerBuiltin::from_identity(identity, boundary.id, format) {
                        Some(execution) => TerminalCompilerBuiltinSettlement::Execution {
                            identity,
                            execution,
                        },
                        None => TerminalCompilerBuiltinSettlement::NoNativeBoundaryRealization,
                    }
                }
            };
            evidence.push(TerminalCompilerBuiltinProposal::new(
                selected_row.requirement_identity.to_owned(),
                selected_row.plan_index,
                boundary.id,
                settlement,
                selected_row.semantic_schema,
                &plans[selected_row.plan_index].schema,
            ));
            Ok(())
        },
    )?;
    Ok(evidence)
}

pub use numeric_intrinsic_admission::{
    AdmittedTerminalIntrinsics, admit_terminal_intrinsics,
    selection::validate_external_provider_owners,
};
