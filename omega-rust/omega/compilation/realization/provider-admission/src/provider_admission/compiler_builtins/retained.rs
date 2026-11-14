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
// external provider declarations and admit closed numeric operations for an
// existing Terminal executor without granting host, device, or ISA feature
// authority.

use super::super::NativeCompilerBuiltinSettlement;
use super::{
    SettledCompilerBuiltin, boundary_contract_matches_selected_schema, compiler_builtin_completion,
    float_arithmetic_signature,
};
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::external_providers::{matches_external_selection, reconcile_numeric_meaning};
use effects::provider_plan::{ProviderPlan, ServiceSchema};
use provider_selection::selected_external_execution;
use std::collections::BTreeSet;
use target_operations::{CompilerBuiltinExecution, NearestIeeeFloatArithmeticRealization};

pub use provider_selection::CompilerIntrinsicPlanProvenance;

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

struct SelectedIntrinsicRow<'a> {
    requirement_identity: &'a str,
    plan_index: usize,
    row: &'a effects::provider_plan::ProviderPlanRow,
    semantic_schema: &'a ServiceSchema,
    execution: Option<CompilerIntrinsicExecutionIdentity>,
    external_realization: Option<&'a str>,
}

pub fn derive_selected_intrinsic_settlement_proposals<'a>(
    selected: &'a effects::SelectedProviderPlanFacts,
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'a>>,
    demanded_boundaries: &BTreeSet<String>,
    boundaries: &[terminal_psi::BoundaryMachineDeclaration],
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<TerminalCompilerBuiltinProposal>, Vec<Diagnostic>> {
    let plans = selected.plans();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected provider plans are not aligned with compiler-intrinsic settlement provenance",
        )]);
    }
    let mut selected_rows = Vec::new();
    for (plan_index, (plan, retained)) in plans.iter().zip(provenance).enumerate() {
        if retained.plan != plan
            || retained.row_executions.len() != plan.rows.len()
            || retained.row_numeric_realizations.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "selected compiler-intrinsic plan `{}` has misaligned retained settlement provenance",
                plan.name,
            ))]);
        }
        if demanded_boundaries.is_empty() {
            continue;
        }
        for ((row, execution), realization) in plan
            .rows
            .iter()
            .zip(retained.row_executions)
            .zip(retained.row_numeric_realizations)
        {
            if matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
            ) {
                selected_rows.push(SelectedIntrinsicRow {
                    requirement_identity: plan.identities.text(row.requirement_identity),
                    plan_index,
                    row,
                    semantic_schema: retained.semantic_schema,
                    execution: *execution,
                    external_realization: realization.as_deref(),
                });
            }
        }
    }
    selected_rows
        .sort_unstable_by(|left, right| left.requirement_identity.cmp(right.requirement_identity));

    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    for requirement in demanded_boundaries {
        let actual_requirement = selected.selected_requirement_identity(requirement);
        let first =
            selected_rows.partition_point(|row| row.requirement_identity < actual_requirement);
        let matches = selected_rows[first..]
            .iter()
            .take_while(|row| row.requirement_identity == actual_requirement)
            .filter(|row| {
                selected.selected_row_matches_requirement(
                    requirement,
                    row.requirement_identity,
                    plans[row.plan_index].report_fingerprint(),
                )
            })
            .collect::<Vec<_>>();
        let [selected_row] = matches.as_slice() else {
            if !matches.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` resolves to {} selected compiler-intrinsic rows", matches.len(),
                )));
            }
            continue;
        };
        let mut matching_boundaries = boundaries
            .iter()
            .filter(|boundary| identities.text(boundary.identity) == Some(requirement.as_str()));
        let (Some(boundary), None) = (matching_boundaries.next(), matching_boundaries.next())
        else {
            diagnostics.push(Diagnostic::error(format!(
                "compiler-builtin proposal for `{requirement}` does not rejoin one Terminal boundary"
            )));
            continue;
        };
        let Some(callable) = boundary.declaration.as_ref() else {
            diagnostics.push(Diagnostic::error(format!(
                "compiler-builtin `{requirement}` has no checked callable declaration"
            )));
            continue;
        };
        let identity = match selected_external_execution(
            external_providers,
            identities,
            &callable.source.machine,
            &plans[selected_row.plan_index],
            selected_row.row,
            selected_row.execution,
            selected_row.external_realization,
        ) {
            Ok(identity) => identity,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` {reason}"
                )));
                continue;
            }
        };
        if let Err(reason) = reconcile_numeric_meaning(boundary.numeric_intrinsic, identity) {
            diagnostics.push(Diagnostic::error(format!(
                "Terminal boundary `{requirement}` {reason}"
            )));
            continue;
        }
        let format = boundary
            .numeric_intrinsic
            .and_then(|meaning| meaning.builtin_format());
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
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}

pub fn validate_external_provider_owners(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
) -> Result<(), String> {
    validate_provider_owners(external_providers, identities, declarations, |_, _| true)
}

fn validate_retained_external_provider_owners(
    module: &terminal_psi::TerminalModule,
    selected: &effects::SelectedProviderPlanFacts,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
) -> Result<(), String> {
    backend_plan::require_selected_provider_declarations(selected, declarations)?;
    let mut retained = module
        .boundary_machines
        .iter()
        .map(|boundary| {
            module
                .identities
                .text(boundary.identity)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    format!(
                        "retained Terminal boundary {:?} has no callable identity",
                        boundary.id,
                    )
                })
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    for demand in &module.boundary_application_demands {
        let identity = demand.requirement().overload();
        if identity.is_empty() {
            return Err("retained boundary application has no callable identity".into());
        }
        retained.insert(identity);
    }
    validate_provider_owners(
        &module.external_providers,
        &module.identities,
        declarations,
        |plan, row| {
            retained.iter().any(|requirement| {
                selected.selected_row_matches_requirement(
                    requirement,
                    plan.identities.text(row.requirement_identity),
                    plan.report_fingerprint(),
                )
            })
        },
    )
}

fn validate_provider_owners(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
    requires_declaration: impl Fn(
        &effects::provider_plan::ProviderPlan,
        &effects::provider_plan::ProviderPlanRow,
    ) -> bool,
) -> Result<(), String> {
    for selected in declarations {
        if selected.row_numeric_realizations.len() != selected.plan.rows.len() {
            return Err("selected external declarations have incomplete row custody".into());
        }
        for (row, realization) in selected
            .plan
            .rows
            .iter()
            .zip(&selected.row_numeric_realizations)
        {
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } =
                &row.binding
            else {
                if realization.is_some() {
                    return Err("non-intrinsic row substituted an external realization".into());
                }
                continue;
            };
            if realization.as_ref().is_some_and(String::is_empty) {
                return Err("selected external realization has empty declaration identity".into());
            }
            let mut found = false;
            for candidate in external_providers.iter().filter(|candidate| {
                matches_external_selection(candidate, identities, &selected.plan, row, machine)
                    && realization.as_ref().is_none_or(|realization| {
                        identities.text(candidate.realization.machine.identity())
                            == Some(realization.as_str())
                    })
            }) {
                found = true;
                let owner = candidate
                    .provider
                    .as_ref()
                    .and_then(|provider| identities.text(provider.declaration.identity()));
                if owner != selected.provider_declaration.as_deref() {
                    return Err(
                        "selected external provider has a different exact declaring owner".into(),
                    );
                }
            }
            if realization.is_some() && !found && requires_declaration(&selected.plan, row) {
                return Err(format!(
                    "selected provider `{}` has no retained external implementation for `{}` (implementation `{}`)",
                    selected.plan.provider_type,
                    selected.plan.identities.text(row.requirement_identity),
                    realization.as_deref().unwrap_or_default(),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AdmittedTerminalIntrinsics {
    terminal: terminal_codec::TerminalExecutionIdentity,
    operations: Vec<(
        semantic_vocabulary::BoundaryMachineId,
        NearestIeeeFloatArithmeticRealization,
    )>,
}

impl AdmittedTerminalIntrinsics {
    pub fn terminal(&self) -> &terminal_codec::TerminalExecutionIdentity {
        &self.terminal
    }

    pub fn operations(
        &self,
    ) -> &[(
        semantic_vocabulary::BoundaryMachineId,
        NearestIeeeFloatArithmeticRealization,
    )] {
        &self.operations
    }
}

pub fn admit_terminal_intrinsics<'selection>(
    artifact: terminal_codec::TerminalExecutionView<'_>,
    selected: &'selection effects::SelectedProviderPlanFacts,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'selection>>,
    demanded: &std::collections::BTreeSet<String>,
) -> Result<AdmittedTerminalIntrinsics, Vec<Diagnostic>> {
    let module = artifact.module();
    validate_retained_external_provider_owners(module, selected, declarations)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let proposals = derive_selected_intrinsic_settlement_proposals(
        selected,
        provenance,
        demanded,
        &module.boundary_machines,
        &module.external_providers,
        &module.identities,
    )?;
    let retained = prepare_retained_compiler_builtins(&proposals, selected.plans())?;
    let mut operations = Vec::with_capacity(retained.len());
    for proposal in retained {
        let refuse = |reason: &str| {
            vec![Diagnostic::error(format!(
                "selected Terminal intrinsic `{}` {reason}",
                proposal.requirement_identity,
            ))]
        };
        let TerminalCompilerBuiltinSettlement::Execution {
            identity,
            execution,
        } = proposal.settlement
        else {
            return Err(refuse("has no closed catalog execution"));
        };
        let CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format } =
            execution.execution()
        else {
            return Err(refuse("requires separate host or physical admission"));
        };
        if !identity.is_numeric() {
            return Err(refuse("does not retain a numeric semantic identity"));
        }
        let boundary =
            terminal_psi::find_boundary_machine(&module.boundary_machines, proposal.boundary)
                .ok_or_else(|| refuse("names an absent Terminal boundary"))?;
        if !boundary_contract_matches_selected_schema(
            boundary,
            proposal.provider_plan,
            proposal.requirement_identity,
        ) || boundary.completion != compiler_builtin_completion(execution.execution())
        {
            return Err(refuse(
                "differs from its exact selected completion contract",
            ));
        }
        let realization = float_arithmetic_signature(
            NearestIeeeFloatArithmeticRealization {
                operation,
                format,
                x86_scalar_fma: None,
            },
            boundary,
            &module.structural_types,
        )
        .map_err(|reason| refuse(&reason))?;
        if operations.iter().any(|(slot, _)| *slot == boundary.id) {
            return Err(refuse("has competing selected executions"));
        }
        operations.push((boundary.id, realization));
    }
    Ok(AdmittedTerminalIntrinsics {
        terminal: artifact.custody_identity(),
        operations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_semantics::numeric_intrinsics::NumericIntrinsic;
    use target_operations::CompilerBuiltinExecution;

    #[test]
    fn portable_numeric_meaning_cannot_authorize_or_replace_provider_custody() {
        let meaning = NumericIntrinsic::Builtin {
            function: symbols::BuiltinFunction::FloatIsNan,
            format: numerics::literals::FloatFormat::F64,
        };
        let selected = CompilerIntrinsicExecutionIdentity::from(meaning);
        assert!(reconcile_numeric_meaning(Some(meaning), Some(selected)).is_ok());
        assert!(reconcile_numeric_meaning(Some(meaning), None).is_err());
        assert!(reconcile_numeric_meaning(None, Some(selected)).is_err());
        let other = NumericIntrinsic::Builtin {
            function: symbols::BuiltinFunction::FloatIsFinite,
            format: numerics::literals::FloatFormat::F64,
        };
        assert!(reconcile_numeric_meaning(Some(other), Some(selected)).is_err());
        assert!(
            reconcile_numeric_meaning(
                Some(meaning),
                Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
            )
            .is_err()
        );
        assert!(
            reconcile_numeric_meaning(
                None,
                Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
            )
            .is_ok()
        );
        assert!(reconcile_numeric_meaning(None, None).is_ok());
        let format = meaning.builtin_format().unwrap();
        assert_eq!(format, semantic_vocabulary::IeeeFloatFormat::Binary64);
        assert!(matches!(
            SettledCompilerBuiltin::from_identity(
                selected,
                semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
                Some(format)
            )
            .map(SettledCompilerBuiltin::execution),
            Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                operation: target_operations::IeeeFloatArithmeticOperation::IsNan,
                format: semantic_vocabulary::IeeeFloatFormat::Binary64,
            })
        ));
    }
}
