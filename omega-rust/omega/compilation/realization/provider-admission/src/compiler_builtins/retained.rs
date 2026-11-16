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
    module: &terminal_psi::TerminalModule,
    direct_leaves: &[backend_plan::DirectBoundaryLeaf],
) -> Result<Vec<TerminalCompilerBuiltinProposal>, Vec<Diagnostic>> {
    let boundaries = &module.boundary_machines;
    let external_providers = &module.external_providers;
    let identities = &module.identities;
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
            || plan.row_numeric_realizations.len() != plan.rows.len()
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
            .zip(&plan.row_numeric_realizations)
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
        // The selected direct call already records its declaration and target
        // implementation. Validate that evidence here instead of asking an
        // external provider to supply it again.
        let mut direct = direct_leaves
            .iter()
            .filter(|leaf| leaf.plan == plans[selected_row.plan_index]);
        let direct = match (direct.next(), direct.next()) {
            (leaf, None) => leaf,
            _ => {
                diagnostics.push(Diagnostic::error(
                    "compiler-builtin proposal has competing direct callable custody",
                ));
                continue;
            }
        };
        let identity = match if let Some(leaf) = direct {
            leaf.validate_bound_calling_custody(module).and_then(|()| {
                if &leaf.satisfied_schema != selected_row.semantic_schema
                    || leaf.execution
                        != backend_plan::DirectBoundaryExecution::Intrinsic(selected_row.execution)
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
                external_providers,
                identities,
                &callable.source.machine,
                &plans[selected_row.plan_index],
                selected_row.row,
                selected_row.execution,
                selected_row.external_realization,
            )
        } {
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
                | CompilerIntrinsicExecutionIdentity::RunCustody(_)
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

// A selected external realization needs its exact declaring owner only where
// the module retains the requirement as a boundary or boundary application:
// native realization and intrinsic admission both execute selected rows by that
// demand, so a selected row nothing retains is never realized. Every consumer
// shares this one demand contract rather than asking for declarations of rows
// the program never executes.
pub fn validate_external_provider_owners(
    module: &terminal_psi::TerminalModule,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<(), String> {
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
        .collect::<Result<BTreeSet<_>, _>>()?;
    for demand in &module.boundary_application_demands {
        let identity = demand.requirement().overload();
        if identity.is_empty() {
            return Err("retained boundary application has no callable identity".into());
        }
        retained.insert(identity);
    }
    validate_retained_provider_owners(
        &module.external_providers,
        &module.identities,
        selected,
        &retained,
    )
}

fn validate_retained_provider_owners(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    facts: &effects::SelectedProviderPlanFacts,
    retained: &BTreeSet<&str>,
) -> Result<(), String> {
    let requires_declaration =
        |plan: &ProviderPlan, row: &effects::provider_plan::ProviderPlanRow| {
            retained.iter().any(|requirement| {
                facts.selected_row_matches_requirement(
                    requirement,
                    plan.identities.text(row.requirement_identity),
                    plan.report_fingerprint(),
                )
            })
        };
    for selected in facts.plans() {
        if selected.row_numeric_realizations.len() != selected.rows.len() {
            return Err("selected external declarations have incomplete row custody".into());
        }
        for (row, realization) in selected.rows.iter().zip(&selected.row_numeric_realizations) {
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
                matches_external_selection(candidate, identities, selected, row, machine)
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
            if realization.is_some() && !found && requires_declaration(selected, row) {
                return Err(format!(
                    "selected provider `{}` has no retained external implementation for `{}` (implementation `{}`)",
                    selected.provider_type,
                    selected.identities.text(row.requirement_identity),
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
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'selection>>,
    demanded: &std::collections::BTreeSet<String>,
) -> Result<AdmittedTerminalIntrinsics, Vec<Diagnostic>> {
    let module = artifact.module();
    validate_external_provider_owners(module, selected)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let proposals = derive_selected_intrinsic_settlement_proposals(
        selected,
        provenance,
        demanded,
        module,
        &[],
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
            module,
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

    fn intrinsic_plan(realization: &str) -> effects::SelectedProviderPlanFacts {
        let mut plan = ProviderPlan {
            name: "selected".into(),
            provider_type: "NativeFloats".into(),
            provider_declaration: Some("NativeFloats".into()),
            row_numeric_realizations: vec![Some(realization.into())],
            target: "linux_x86_64".into(),
            ..Default::default()
        };
        let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
        declaration.trait_name = declaration.identities.intern("Float");
        declaration
            .methods
            .push(effects::provider_plan::ServiceMethod {
                name: "add".into(),
                requirement_owner: "Float".into(),
                requirement_identity: declaration.identities.intern("Float::add"),
                ..Default::default()
            });
        let requirement = plan.identities.intern("Float::add");
        plan.rows.push(effects::provider_plan::ProviderPlanRow {
            method: "add".into(),
            requirement_identity: requirement,
            declared_requirement_identity: requirement,
            calling_signature: None,
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                machine: "float_add".into(),
            },
        });
        effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan]).unwrap()
    }

    #[test]
    fn external_realization_owner_is_demanded_only_for_retained_requirements() {
        let facts = intrinsic_plan("NativeFloatAdd");
        let identities = terminal_psi::TerminalIdentityTable::default();
        // A selected float row the program never retains needs no declaration:
        // a scalar-only module must not be refused for metadata it never runs.
        assert_eq!(
            validate_retained_provider_owners(&[], &identities, &facts, &BTreeSet::new()),
            Ok(())
        );
        let unrelated = BTreeSet::from(["Integer::add"]);
        assert_eq!(
            validate_retained_provider_owners(&[], &identities, &facts, &unrelated),
            Ok(())
        );
        // Once the module retains the requirement, its declaration is required.
        let retained = BTreeSet::from(["Float::add"]);
        let refusal =
            validate_retained_provider_owners(&[], &identities, &facts, &retained).unwrap_err();
        assert!(
            refusal.contains("has no retained external implementation for `Float::add`"),
            "{refusal}"
        );
    }
}
