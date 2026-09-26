//! Source-evaluated import settlements for a compile with no installed
//! provider behind them.
//!
//! A `via` leaf that evaluated to a normalized foreign import is realized as
//! a normalized foreign call, and native realization admits that call only
//! against a provider settlement: execution evidence naming the exact selected
//! plan and requirement, and a same-stack contribution the call site's stack
//! budget consumes. An installed provider supplies both from its installation
//! evidence. A source-evaluated import in an ordinary compile has no
//! installation; its evidence is the selected plan and the retained external
//! contract themselves, so this module mints the two records from exactly
//! those inputs. Nothing here grants authority: the plan report identity and
//! commitment are rejoined by `retained_native_product` and
//! `providers/settlements`, and the receiving terminal-authority policy still
//! classifies the mechanism.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use installation_evidence::ProviderExecutionEvidence;
use task_plans::{
    AdmittedSameStackContribution, SameStackContributionAdmissionCandidate,
    SameStackContributionAdmissionReceiptId, SameStackProviderPlanCommitment,
    admit_same_stack_contribution,
};

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

/// Execution evidence for one source-evaluated import: the requirement and
/// the exact selected plan, with report coordinates derived from them so the
/// same compile mints the same values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEvaluatedImportExecution {
    requirement_identity: String,
    provider_plan_report_identity: u64,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    boundary_contract_report_fingerprint: u64,
}

impl ProviderExecutionEvidence for SourceEvaluatedImportExecution {
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
pub struct MintedSourceEvaluatedImport<'plan> {
    pub plan: &'plan ProviderPlan,
    pub execution: SourceEvaluatedImportExecution,
    pub same_stack: AdmittedSameStackContribution,
}

/// Mint execution evidence and a same-stack contribution for every selected
/// provider row whose binding is an evaluated import and whose requirement
/// the Terminal module demands. Rows of any other binding kind are not this
/// module's: intrinsics settle through the compiler-builtin catalog and
/// syscalls through their own realization; an import the program never
/// calls settles nothing.
pub fn mint_source_evaluated_imports<'plan>(
    plans: &'plan [ProviderPlan],
    demanded: &std::collections::BTreeSet<String>,
) -> Result<Vec<MintedSourceEvaluatedImport<'plan>>, Vec<Diagnostic>> {
    let mut minted = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in plans {
        for row in &plan.rows {
            let ProviderBinding::Import { evaluated } = &row.binding else {
                continue;
            };
            if !demanded.contains(&row.requirement_identity) {
                continue;
            }
            let requirement = row.requirement_identity.as_str();
            let plan_report_identity = plan.report_fingerprint();
            let plan_commitment =
                SameStackProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes());
            let locator_digest = evaluated.locator().identity_digest().as_bytes();
            let execution = SourceEvaluatedImportExecution {
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
            match admit_same_stack_contribution(
                SameStackContributionAdmissionCandidate {
                    provider_plan_report_identity: plan_report_identity,
                    provider_plan_commitment: plan_commitment,
                    requirement_identity: requirement.to_owned(),
                    receipt,
                    bytes: SAME_STACK_BYTES,
                    alignment: SAME_STACK_ALIGNMENT,
                },
                plan_report_identity,
                plan_commitment,
                requirement,
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
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in *part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn nonzero(value: u64) -> u64 {
    if value == 0 { 1 } else { value }
}
