// Source-evaluated import and syscall binding execution preparation for native
// admission.
//
// Direct and retained compilation share `prepare_source_evaluated_bindings`:
// join Terminal demand to exact selected import and syscall plans, then derive
// execution records or rejoin a complete supplied roster. Compact report
// coordinates remain diagnostics rather than authority; the independent
// settlement checker joins the strong plan commitment, retained locator,
// calling contract, target, and same-stack contribution before any native
// realization consumes them.
//
// Imported user-space callees run on the program's private receiver stack (a
// `.bss` region with no guard page whose size is the admitted peak), so
// unmeasured foreign entries receive the 256 KiB `SAME_STACK_BYTES` reserve
// unless a bodyless `via` leaf authored `stack N bytes`. Direct kernel entries
// have no user-space callee and mint no same-stack reserve.

use super::{NativeBoundaryRealization, NativeProviderSettlement};
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use installation_evidence::ProviderExecutionEvidence;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use task_plans::{
    AdmittedSameStackContribution, SameStackContributionAdmissionCandidate,
    SameStackContributionAdmissionReceiptId, SameStackProviderPlanCommitment,
    admit_same_stack_contribution,
};

#[derive(Debug, Clone, Copy)]
pub struct SourceEvaluatedImportSettlement<'evidence> {
    provider_execution: &'evidence dyn ProviderExecutionEvidence,
    same_stack: &'evidence AdmittedSameStackContribution,
}

impl<'evidence> SourceEvaluatedImportSettlement<'evidence> {
    pub const fn new(
        provider_execution: &'evidence dyn ProviderExecutionEvidence,
        same_stack: &'evidence AdmittedSameStackContribution,
    ) -> Self {
        Self {
            provider_execution,
            same_stack,
        }
    }

    pub const fn provider_execution(self) -> &'evidence dyn ProviderExecutionEvidence {
        self.provider_execution
    }

    pub const fn same_stack(self) -> &'evidence AdmittedSameStackContribution {
        self.same_stack
    }
}

#[derive(Debug)]
pub struct PreparedSourceEvaluatedBindings<'input> {
    evidence: SourceImportEvidence<'input>,
    syscalls: Vec<PreparedSystemCall<'input>>,
}

#[derive(Debug)]
enum SourceImportEvidence<'input> {
    Derived(Vec<MintedSourceEvaluatedImport<'input>>),
    Supplied(Vec<NativeProviderSettlement<'input>>),
}

#[derive(Debug)]
struct PreparedSystemCall<'plan> {
    plan: &'plan ProviderPlan,
    execution: SourceEvaluatedBindingExecution,
}

pub fn prepare_source_evaluated_bindings<'input>(
    plans: &'input [ProviderPlan],
    demanded: &BTreeSet<String>,
    supplied: &[SourceEvaluatedImportSettlement<'input>],
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<PreparedSourceEvaluatedBindings<'input>, Vec<Diagnostic>> {
    let exact_plans = exact_demanded_import_plans(plans, demanded)?;
    let evidence = if supplied.is_empty() {
        SourceImportEvidence::Derived(mint_source_evaluated_imports(plans, demanded, identities)?)
    } else {
        SourceImportEvidence::Supplied(rejoin_external_import_settlements(&exact_plans, supplied)?)
    };
    let syscalls = prepare_system_calls(plans, demanded)?;
    Ok(PreparedSourceEvaluatedBindings { evidence, syscalls })
}

impl PreparedSourceEvaluatedBindings<'_> {
    pub fn settlements(&self) -> Cow<'_, [NativeProviderSettlement<'_>]> {
        let mut settlements: Cow<'_, [NativeProviderSettlement<'_>]> = match &self.evidence {
            SourceImportEvidence::Derived(imports) => Cow::Owned(
                imports
                    .iter()
                    .map(|import| NativeProviderSettlement {
                        provider_execution: &import.execution,
                        provider_plan: import.plan,
                        realization: NativeBoundaryRealization::NormalizedForeignCall(
                            &import.same_stack,
                        ),
                    })
                    .collect(),
            ),
            SourceImportEvidence::Supplied(settlements) => Cow::Borrowed(settlements),
        };
        if !self.syscalls.is_empty() {
            settlements
                .to_mut()
                .extend(
                    self.syscalls
                        .iter()
                        .map(|syscall| NativeProviderSettlement {
                            provider_execution: &syscall.execution,
                            provider_plan: syscall.plan,
                            realization: NativeBoundaryRealization::SystemCall,
                        }),
                );
        }
        settlements
    }
}

const SAME_STACK_BYTES: u64 = 256 * 1024;
const SAME_STACK_ALIGNMENT: u64 = 16;

fn foreign_same_stack_bytes(authored: u64) -> u64 {
    if authored == 0 {
        SAME_STACK_BYTES
    } else {
        authored
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceEvaluatedBindingExecution {
    requirement_identity: String,
    provider_plan_report_identity: u64,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    boundary_contract_report_fingerprint: u64,
}

impl ProviderExecutionEvidence for SourceEvaluatedBindingExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    fn provider_execution_report_identity(&self) -> u64 {
        self.provider_execution_report_identity
    }

    fn provider_execution_report_fingerprint(&self) -> u64 {
        self.provider_execution_report_fingerprint
    }

    fn normalized_root_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.boundary_contract_report_fingerprint
    }
}

#[derive(Debug)]
struct MintedSourceEvaluatedImport<'plan> {
    plan: &'plan ProviderPlan,
    execution: SourceEvaluatedBindingExecution,
    same_stack: AdmittedSameStackContribution,
}

fn mint_source_evaluated_imports<'plan>(
    plans: &'plan [ProviderPlan],
    demanded: &BTreeSet<String>,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<MintedSourceEvaluatedImport<'plan>>, Vec<Diagnostic>> {
    let mut minted = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in plans {
        for row in &plan.rows {
            let mechanism_digest = match &row.binding {
                ProviderBinding::Import { evaluated } => {
                    evaluated.locator().identity_digest().as_bytes()
                }
                ProviderBinding::VtableField { .. } | ProviderBinding::TableFunction { .. } => {
                    *plan.identity_digest().as_bytes()
                }
                _ => continue,
            };
            let requirement = plan.identities.text(row.requirement_identity);
            if !demanded.contains(requirement) {
                continue;
            }
            let plan_report_identity = plan.report_fingerprint();
            let plan_commitment =
                SameStackProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes());
            let locator_digest = mechanism_digest;
            let execution = SourceEvaluatedBindingExecution {
                requirement_identity: requirement.to_owned(),
                provider_plan_report_identity: plan_report_identity,
                provider_execution_report_identity: nonzero(fnv1a(&[
                    b"source-evaluated-import-execution",
                    requirement.as_bytes(),
                    &plan_report_identity.to_le_bytes(),
                ])),
                provider_execution_report_fingerprint: nonzero(fnv1a(&[
                    b"source-evaluated-import-fingerprint",
                    requirement.as_bytes(),
                    &locator_digest,
                ])),
                boundary_contract_report_fingerprint: nonzero(fnv1a(&[
                    b"source-evaluated-import-contract",
                    &locator_digest,
                ])),
            };
            let receipt = SameStackContributionAdmissionReceiptId::from_normalized_identity(
                nonzero(fnv1a(&[
                    b"source-evaluated-import-receipt",
                    requirement.as_bytes(),
                    &plan_report_identity.to_le_bytes(),
                ])),
            );
            let receipt = match receipt {
                Ok(receipt) => receipt,
                Err(error) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "source-evaluated import `{requirement}` cannot mint its same-stack admission receipt: {error}"
                    )));
                    continue;
                }
            };
            let Some(requirement_index) = identities.index_of(requirement) else {
                diagnostics.push(Diagnostic::error(format!(
                    "source-evaluated import `{requirement}` is absent from the selection identity table"
                )));
                continue;
            };
            match admit_same_stack_contribution(
                SameStackContributionAdmissionCandidate {
                    provider_plan_report_identity: plan_report_identity,
                    provider_plan_commitment: plan_commitment,
                    requirement_identity: requirement_index,
                    receipt,
                    bytes: foreign_same_stack_bytes(row.admitted_same_stack_bytes),
                    alignment: SAME_STACK_ALIGNMENT,
                },
                plan_report_identity,
                plan_commitment,
                requirement_index,
            ) {
                Ok(same_stack) => minted.push(MintedSourceEvaluatedImport {
                    plan,
                    execution,
                    same_stack,
                }),
                Err(error) => diagnostics.push(Diagnostic::error(format!(
                    "source-evaluated import `{requirement}` has no admitted same-stack contribution: {error}"
                ))),
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(minted)
    } else {
        Err(diagnostics)
    }
}

fn prepare_system_calls<'plan>(
    plans: &'plan [ProviderPlan],
    demanded: &BTreeSet<String>,
) -> Result<Vec<PreparedSystemCall<'plan>>, Vec<Diagnostic>> {
    let mut prepared = Vec::new();
    for plan in plans {
        for row in &plan.rows {
            let ProviderBinding::Syscall { number } = row.binding else {
                continue;
            };
            let requirement = plan.identities.text(row.requirement_identity);
            if !demanded.contains(requirement) {
                continue;
            }
            let count = plans
                .iter()
                .flat_map(|plan| {
                    plan.rows.iter().filter(move |row| {
                        plan.identities.text(row.requirement_identity) == requirement
                    })
                })
                .count();
            if count != 1 {
                return Err(diagnostics::error_in_context(
                    "source-evaluated syscall",
                    format!(
                        "demanded requirement `{requirement}` resolves to {count} selected rows"
                    ),
                ));
            }
            let plan_report_identity = plan.report_fingerprint();
            let commitment = plan.identity_digest();
            let execution = SourceEvaluatedBindingExecution {
                requirement_identity: requirement.to_owned(),
                provider_plan_report_identity: plan_report_identity,
                provider_execution_report_identity: nonzero(fnv1a(&[
                    b"source-evaluated-syscall-execution",
                    requirement.as_bytes(),
                    commitment.as_bytes(),
                ])),
                provider_execution_report_fingerprint: nonzero(fnv1a(&[
                    b"source-evaluated-syscall-fingerprint",
                    requirement.as_bytes(),
                    commitment.as_bytes(),
                    &number.to_le_bytes(),
                ])),
                boundary_contract_report_fingerprint: nonzero(fnv1a(&[
                    b"source-evaluated-syscall-contract",
                    commitment.as_bytes(),
                    &number.to_le_bytes(),
                ])),
            };
            prepared.push(PreparedSystemCall { plan, execution });
        }
    }
    Ok(prepared)
}

fn fnv1a(parts: &[&[u8]]) -> u64 {
    let mut fingerprint = semantic_vocabulary::Fnv1a::new();
    for part in parts {
        fingerprint.bytes(part);
        fingerprint.byte(0xff);
    }
    fingerprint.finish()
}

fn nonzero(value: u64) -> u64 {
    if value == 0 { 1 } else { value }
}

fn exact_demanded_import_plans<'proposal>(
    plans: &'proposal [ProviderPlan],
    demanded: &BTreeSet<String>,
) -> Result<BTreeMap<String, &'proposal ProviderPlan>, Vec<Diagnostic>> {
    let mut exact = BTreeMap::new();
    for requirement in demanded {
        let matches = plans
            .iter()
            .flat_map(|plan| {
                plan.rows
                    .iter()
                    .filter(move |row| {
                        plan.identities.text(row.requirement_identity) == requirement.as_str()
                            && matches!(
                                row.binding,
                                ProviderBinding::Import { .. }
                                    | ProviderBinding::VtableField { .. }
                                    | ProviderBinding::TableFunction { .. }
                            )
                    })
                    .map(move |_| plan)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {}
            [plan] => {
                exact.insert(requirement.clone(), *plan);
            }
            _ => {
                return Err(diagnostics::error_in_context(
                    "Terminal native proposal",
                    format!(
                        "demanded source-evaluated import `{requirement}` resolves to {} selected import plans",
                        matches.len()
                    ),
                ));
            }
        }
    }
    Ok(exact)
}

fn rejoin_external_import_settlements<'proposal, 'evidence>(
    exact_plans: &BTreeMap<String, &'proposal ProviderPlan>,
    imports: &[SourceEvaluatedImportSettlement<'evidence>],
) -> Result<Vec<NativeProviderSettlement<'proposal>>, Vec<Diagnostic>>
where
    'evidence: 'proposal,
{
    let mut seen = BTreeSet::new();
    let mut settlements = Vec::with_capacity(imports.len());
    for import in imports {
        let requirement = import.provider_execution.requirement_identity();
        if !seen.insert(requirement) {
            return Err(diagnostics::error_in_context(
                "source-evaluated import settlement",
                format!("requirement `{requirement}` was supplied more than once"),
            ));
        }
        let provider_plan = exact_plans.get(requirement).copied().ok_or_else(|| {
            diagnostics::error_in_context(
                "source-evaluated import settlement",
                format!("requirement `{requirement}` is not a demanded selected import"),
            )
        })?;
        if provider_plan.report_fingerprint()
            != import.provider_execution.provider_plan_report_identity()
        {
            return Err(diagnostics::error_in_context(
                "source-evaluated import settlement",
                format!(
                    "execution evidence for `{requirement}` names a different provider-plan report coordinate"
                ),
            ));
        }
        settlements.push(NativeProviderSettlement {
            provider_execution: import.provider_execution,
            provider_plan,
            realization: NativeBoundaryRealization::NormalizedForeignCall(import.same_stack),
        });
    }
    if let Some(missing) = exact_plans
        .keys()
        .find(|requirement| !seen.contains(requirement.as_str()))
    {
        return Err(diagnostics::error_in_context(
            "source-evaluated import settlement",
            format!("demanded import `{missing}` has no supplied execution and stack custody"),
        ));
    }
    Ok(settlements)
}

#[cfg(test)]
mod tests {
    use super::{SAME_STACK_BYTES, foreign_same_stack_bytes, mint_source_evaluated_imports};
    use effects::provider_plan::{
        ProviderBinding, ProviderIdentityTable, ProviderPlan, ProviderPlanRow,
    };
    use std::collections::BTreeSet;

    fn leaf_plan(binding: ProviderBinding, bytes: u64) -> ProviderPlan {
        let mut identities = ProviderIdentityTable::default();
        let requirement_identity = identities.intern("Leaves::output_string");
        ProviderPlan {
            name: "leaves".to_owned(),
            identities,
            rows: vec![ProviderPlanRow {
                method: "output_string".to_owned(),
                requirement_identity,
                declared_requirement_identity: requirement_identity,
                calling_signature: None,
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: bytes,
                binding,
            }],
            ..ProviderPlan::default()
        }
    }

    #[test]
    fn authored_same_stack_bytes_replace_the_unmeasured_reserve() {
        let demanded = BTreeSet::from(["Leaves::output_string".to_owned()]);
        let identities = terminal_psi::TerminalIdentityTable::from(vec![
            String::new(),
            "Leaves::output_string".to_owned(),
        ]);
        for binding in [
            ProviderBinding::VtableField {
                table: "Protocol".to_owned(),
                field: "output_string".to_owned(),
                storage: Err("this control observes only same-stack demand".into()),
            },
            ProviderBinding::TableFunction {
                table: "BootServices".to_owned(),
                field: "output_string".to_owned(),
                storage: Err("this control observes only same-stack demand".into()),
            },
        ] {
            let authored_plan = leaf_plan(binding.clone(), 4096);
            let unmeasured_plan = leaf_plan(binding, 0);
            let authored = mint_source_evaluated_imports(
                std::slice::from_ref(&authored_plan),
                &demanded,
                &identities,
            )
            .expect("authored demand admits");
            let unmeasured = mint_source_evaluated_imports(
                std::slice::from_ref(&unmeasured_plan),
                &demanded,
                &identities,
            )
            .expect("unmeasured leaf keeps the reserve");
            assert_eq!(authored[0].same_stack.bytes(), 4096);
            assert_eq!(unmeasured[0].same_stack.bytes(), SAME_STACK_BYTES);
            assert_eq!(foreign_same_stack_bytes(0), SAME_STACK_BYTES);
            assert_eq!(foreign_same_stack_bytes(64), 64);
        }
    }
}
