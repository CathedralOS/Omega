//! Compiler-owned native settlements for closed intrinsic catalog entries.
//!
//! Provider selection and checked intrinsic review establish the exact
//! declaration/target identity. This module performs the later executable
//! projection: only compiler-intrinsic requirements actually called by the
//! canonical Terminal artifact receive native execution evidence.

use crate::pipeline::CheckedCompilation;
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_provider_planning::plans::CompilerIntrinsicExecutionIdentity;
use psi_diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug)]
pub(super) struct CompilerIntrinsicSettlementEvidence {
    requirement_identity: String,
    provider_plan_report_identity: u64,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    normalized_root_report_identity: u64,
    boundary_contract_report_fingerprint: u64,
    pub(super) plan_index: usize,
    pub(super) execution: CompilerIntrinsicExecutionIdentity,
}

impl ProviderExecutionEvidence for CompilerIntrinsicSettlementEvidence {
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
        self.normalized_root_report_identity
    }

    fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.boundary_contract_report_fingerprint
    }
}

pub(super) fn demanded_boundary_identities(
    module: &psi_terminal::TerminalModule,
) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    let declarations = module
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut demanded = BTreeSet::new();
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let psi_terminal::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
            continue;
        };
        let Some(identity) = declarations.get(boundary).copied() else {
            return Err(vec![Diagnostic::error(format!(
                "Terminal intrinsic settlement demand cites absent boundary {:?}",
                boundary,
            ))]);
        };
        demanded.insert(identity.to_owned());
    }
    Ok(demanded)
}

pub(super) fn derive_compiler_intrinsic_settlement_evidence(
    checked: &CheckedCompilation,
    demanded_boundaries: &BTreeSet<String>,
) -> Result<Vec<CompilerIntrinsicSettlementEvidence>, Vec<Diagnostic>> {
    let plans = checked.selected_provider_plans().plans();
    let provenance = checked.selected_provider_provenance();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected provider plans are not aligned with compiler-intrinsic settlement provenance",
        )]);
    }
    for (plan, retained) in plans.iter().zip(provenance) {
        if retained.plan != *plan
            || retained.row_compiler_intrinsic_executions.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "selected compiler-intrinsic plan `{}` has misaligned retained settlement provenance",
                plan.name,
            ))]);
        }
    }

    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    for requirement in demanded_boundaries {
        let matches = plans
            .iter()
            .zip(provenance)
            .enumerate()
            .flat_map(|(plan_index, (plan, retained))| {
                plan.rows
                    .iter()
                    .zip(&retained.row_compiler_intrinsic_executions)
                    .filter_map(move |(row, execution)| {
                        (row.requirement_identity == *requirement
                            && matches!(
                                row.binding,
                                omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                                    ..
                                }
                            ))
                        .then_some((plan_index, plan, retained, row, *execution))
                    })
            })
            .collect::<Vec<_>>();
        let [(plan_index, plan, retained, row, execution)] = matches.as_slice() else {
            if !matches.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` resolves to {} selected compiler-intrinsic rows",
                    matches.len(),
                )));
            }
            continue;
        };
        debug_assert_eq!(retained.plan, **plan);
        let Some(execution) = execution else {
            diagnostics.push(Diagnostic::error(format!(
                "selected compiler intrinsic `{}` for Terminal boundary `{requirement}` has no closed native catalog identity",
                plan.name,
            )));
            continue;
        };
        if !matches!(
            *execution,
            CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "selected compiler intrinsic `{}` for Terminal boundary `{requirement}` has no native boundary realization",
                plan.name,
            )));
            continue;
        }
        let coordinates = settlement_report_coordinates(
            plan.identity_digest().as_bytes(),
            requirement,
            match &row.binding {
                omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } => {
                    machine
                }
                _ => unreachable!("filtered compiler-intrinsic row"),
            },
            *execution,
        );
        evidence.push(CompilerIntrinsicSettlementEvidence {
            requirement_identity: requirement.clone(),
            provider_plan_report_identity: plan.report_fingerprint(),
            provider_execution_report_identity: coordinates[0],
            provider_execution_report_fingerprint: coordinates[1],
            normalized_root_report_identity: coordinates[2],
            boundary_contract_report_fingerprint: coordinates[3],
            plan_index: *plan_index,
            execution: *execution,
        });
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}

fn settlement_report_coordinates(
    plan_digest: &[u8; 32],
    requirement: &str,
    realization: &str,
    execution: CompilerIntrinsicExecutionIdentity,
) -> [u64; 4] {
    std::array::from_fn(|coordinate| {
        let mut hash = Sha256::new();
        hash.update(b"omega.compiler-intrinsic-provider-execution.v1\0");
        hash.update([coordinate as u8]);
        hash.update(plan_digest);
        hash.update((requirement.len() as u64).to_le_bytes());
        hash.update(requirement.as_bytes());
        hash.update((realization.len() as u64).to_le_bytes());
        hash.update(realization.as_bytes());
        hash.update([match execution {
            CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32 => 0,
            CompilerIntrinsicExecutionIdentity::BuiltinFunction(_) => 1,
            CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { .. } => 2,
            CompilerIntrinsicExecutionIdentity::NamedFloatNegation(_) => 3,
            CompilerIntrinsicExecutionIdentity::NamedFloatConversion { .. } => 4,
        }]);
        let digest: [u8; 32] = hash.finalize().into();
        u64::from_le_bytes(
            digest[..8]
                .try_into()
                .expect("SHA-256 prefix is eight bytes"),
        )
    })
}
