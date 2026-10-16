//! Source-evaluated binding execution inputs for native admission.
//!
//! Direct and retained compilation share `prepare_source_evaluated_bindings`:
//! join Terminal demand to exact selected import and syscall plans, then derive execution
//! records or rejoin a complete supplied roster. The compact report coordinates
//! remain diagnostics, not authority. The independent settlement checker joins
//! the strong plan commitment, retained locator, calling contract, target and
//! same-stack contribution before any native realization can consume them.
//! Direct kernel entries have no user-space callee and mint no same-stack
//! reserve. Their independent settlement rejoins the exact syscall and plan;
//! preparation never grants receiving authority or recognizes service names.

mod syscalls;

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

/// Externally admitted custody for one demanded source-evaluated import.
///
/// Construction does not admit either input. Native realization independently
/// rejoins both values to the exact retained provider plan and import row.
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

/// Prepared source-binding execution inputs for native provider admission.
/// Owns derived evidence or borrows the caller's admitted evidence. Neither
/// form admits a locator, calling contract, stack bound or receiving authority:
/// `admit_native_providers` independently rejoins those inputs.
#[derive(Debug)]
pub struct PreparedSourceEvaluatedBindings<'input> {
    evidence: SourceImportEvidence<'input>,
    syscalls: Vec<syscalls::PreparedSystemCall<'input>>,
}

#[derive(Debug)]
enum SourceImportEvidence<'input> {
    Derived(Vec<MintedSourceEvaluatedImport<'input>>),
    Supplied(Vec<NativeProviderSettlement<'input>>),
}

/// Prepare exactly the demanded selected bindings. An empty supplied import roster
/// derives execution and stack records from the selected plans; a nonempty
/// roster must cover every demanded import exactly once and never falls back
/// to derived evidence for missing rows. Syscalls are always derived from
/// selected plans: they neither consume nor substitute supplied import custody.
pub(crate) fn prepare_source_evaluated_bindings<'input>(
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
    let syscalls = syscalls::prepare_system_calls(plans, demanded)?;
    Ok(PreparedSourceEvaluatedBindings { evidence, syscalls })
}

impl PreparedSourceEvaluatedBindings<'_> {
    /// Borrow supplied settlements directly, or project owned execution/stack
    /// records into the native admission request without cloning their evidence.
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

/// The stack a normalized foreign call may consume from its call site
/// onward. The callee runs on the program's private receiver stack, a
/// `.bss` region with no guard page whose size is exactly the admitted
/// peak, so this contribution must cover everything the OS or runtime
/// entry does before it returns or terminates: kernel32 `ExitProcess`
/// walks loader notifications through ntdll and faulted with the 184-byte
/// region a 64-byte contribution left it (Windows x86-64, 2026-09-23).
/// 256 KiB is the reserve a foreign entry gets; it only enlarges the region
/// and never admits a call the ABI plan refused.
const SAME_STACK_BYTES: u64 = 256 * 1024;
const SAME_STACK_ALIGNMENT: u64 = 16;

/// Execution evidence for one source-evaluated binding: the requirement and
/// the exact selected plan, with report coordinates derived from them so the
/// same compile mints the same values.
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
        // A source-evaluated import roots in the program itself; the plan
        // report coordinate is the only root it has.
        self.provider_plan_report_identity
    }

    fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.boundary_contract_report_fingerprint
    }
}

/// One demanded import row with the two records native realization joins.
#[derive(Debug)]
struct MintedSourceEvaluatedImport<'plan> {
    plan: &'plan ProviderPlan,
    execution: SourceEvaluatedBindingExecution,
    same_stack: AdmittedSameStackContribution,
}

/// Mint execution evidence and a same-stack contribution for every selected
/// provider row whose binding is an evaluated import and whose requirement
/// the Terminal module demands. Rows of any other binding kind are not this
/// module's: intrinsics settle through the compiler-builtin catalog and
/// syscalls through their own realization; an import the program never
/// calls settles nothing.
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
                    bytes: SAME_STACK_BYTES,
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
