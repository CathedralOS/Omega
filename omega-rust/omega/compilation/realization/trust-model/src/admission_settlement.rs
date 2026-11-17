// Join authored root subjects to their selected provider applications, native
// requirements, or accepted-machine evidence. Native requirements retain the
// complete checked promise together with the selected Build locator receipt.

use crate::TrustAdmission;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{AuthoredRootGrant, RootGrantSubject};
use language_semantics::trust_declarations::{
    AcceptedContractDeclaration, AcceptedTemplateDeclaration, TrustDeclarationCardinality,
    TrustDeclarations,
};
use std::collections::BTreeMap;

enum PreparedTrustIdentity {
    ProviderPlan {
        report_identity: u64,
        digest: effects::provider_plan::ProviderPlanDigest,
    },
    NativeRequirement {
        report_identity: u64,
        contract: [u8; 32],
        plan: effects::provider_plan::ProviderPlanDigest,
    },
    MachineTemplate {
        report_identity: u64,
        declaration: AcceptedTemplateDeclaration,
    },
    AcceptedMachine(Result<AcceptedContractDeclaration, TrustDeclarationCardinality>),
}

struct PreparedTrustObligation {
    commitment: String,
    identity: PreparedTrustIdentity,
}

/// The receiving product's package role is authenticated by source custody.
/// Application roots can grant their own claims; package review is the
/// acceptance channel for dependency claims.
#[derive(Clone, Copy)]
pub enum RootTrustPolicy {
    Unpackaged,
    Application(semantic_vocabulary::PackageKeyIdentity),
    PackageReview,
}

impl RootTrustPolicy {
    fn permits_individual_claim(
        self,
        owner: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) -> bool {
        match self {
            Self::Unpackaged => true,
            Self::Application(root) => owner == Some(root),
            Self::PackageReview => false,
        }
    }
}

pub struct NativeTrustInputs<'a> {
    pub module: &'a terminal_psi::TerminalModule,
    pub target: Option<&'a str>,
    pub leaves: &'a [backend_plan::DirectBoundaryLeaf],
    pub evaluated: &'a [effects::provider_plan::EvaluatedNativeBinding],
    pub identities: &'a terminal_psi::TerminalIdentityTable,
}

pub fn reconstruct_trust_obligations(
    declarations: &TrustDeclarations,
    root_grants: &[AuthoredRootGrant],
    provider_plans: &[&effects::provider_plan::ProviderPlan],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    native: NativeTrustInputs<'_>,
    policy: RootTrustPolicy,
) -> Result<Vec<TrustAdmission>, Vec<Diagnostic>> {
    let mut rows = Vec::<PreparedTrustObligation>::new();
    let mut provider_grants = crate::resolve_authored_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|error| vec![error])?
    .into_iter();
    for grant in root_grants {
        let refuse =
            |reason: String| vec![Diagnostic::error(reason).with_source_span(grant.source_span)];
        let (commitment, identity) = match &grant.subject {
            RootGrantSubject::Provider { .. } => {
                // The provider resolver retains the authored row order. Read
                // its associated result directly; display names are not keys.
                let provider = provider_grants.next().ok_or_else(|| {
                    refuse("root provider grant lost its exact selected application".into())
                })?;
                (
                    provider.grant.commitment(),
                    PreparedTrustIdentity::ProviderPlan {
                        report_identity: provider.grant.selected_plan_report_identity,
                        digest: provider.grant.selected_plan_digest,
                    },
                )
            }
            RootGrantSubject::BoundaryRequirement {
                declaration,
                callable,
            } => {
                if !policy.permits_individual_claim(grant.subject_package) {
                    return Err(refuse(format!(
                        "package-aware compilation cannot admit individual native requirement `{}`; package claims require complete package-level review",
                        grant.selector
                    )));
                }
                let mut matches = native.leaves.iter().filter(|leaf| {
                    leaf.declaration_identity == *declaration && leaf.callable_identity == *callable
                });
                let (Some(leaf), None) = (matches.next(), matches.next()) else {
                    return Err(refuse(format!(
                        "root grant `{}` has no unique selected native requirement with its exact declaration and callable application",
                        grant.selector
                    )));
                };
                if Some(leaf.plan.target.as_str()) != native.target {
                    return Err(refuse(
                        "root native grant changes its selected deployment target".into(),
                    ));
                }
                if leaf.plan.origin_package_identity != grant.subject_package {
                    return Err(refuse(
                        "root native grant changes its binding's package ownership".into(),
                    ));
                }
                let boundary = leaf
                    .validate_plain_execution(native.module, native.evaluated, native.identities)
                    .map_err(refuse)?;
                let contract = boundary
                    .declaration
                    .as_ref()
                    .ok_or_else(|| {
                        refuse("root native grant lost its checked requirement contract".into())
                    })?
                    .source
                    .contract;
                (
                    format!(
                        "native requirement: {declaration} {callable} on {}",
                        leaf.plan.target
                    ),
                    PreparedTrustIdentity::NativeRequirement {
                        report_identity: leaf.plan.report_fingerprint(),
                        contract,
                        plan: leaf.plan.identity_digest(),
                    },
                )
            }
            RootGrantSubject::AcceptedMachine {
                declaration,
                machine,
            } => {
                if declaration.is_empty() {
                    return Err(refuse(
                        "root accepted-machine grant lost its exact declaration identity".into(),
                    ));
                }
                if !policy.permits_individual_claim(grant.subject_package) {
                    return Err(refuse(format!(
                        "package-aware compilation cannot admit individual accepted machine `{}`; package claims require complete package-level review",
                        grant.selector
                    )));
                }
                let machine =
                    crate::accepted_machine_grants::accepted_machine(declarations, *machine)
                        .map_err(|error| vec![error])?;
                if machine.declaration_identity != *declaration
                    || machine.package_identity != grant.subject_package
                {
                    return Err(refuse(
                        "root accepted-machine grant changes its exact declaration identity".into(),
                    ));
                }
                let identity = match machine.template {
                    Err(cardinality) => {
                        let reason = match cardinality {
                            TrustDeclarationCardinality::Missing => {
                                "no pre-lowering template classification"
                            }
                            TrustDeclarationCardinality::Duplicate => {
                                "duplicate pre-lowering template classifications"
                            }
                        };
                        return Err(refuse(format!(
                            "accepted machine `{}` has {reason}",
                            machine.name
                        )));
                    }
                    Ok(Some(declaration)) => PreparedTrustIdentity::MachineTemplate {
                        report_identity: declaration.report_fingerprint,
                        declaration,
                    },
                    Ok(None) => PreparedTrustIdentity::AcceptedMachine(machine.contract),
                };
                (format!("accepted fact: {declaration}"), identity)
            }
        };
        rows.push(PreparedTrustObligation {
            commitment,
            identity,
        });
    }

    let mut resolved = BTreeMap::new();
    for row in rows {
        let admission = match row.identity {
            PreparedTrustIdentity::ProviderPlan {
                report_identity,
                digest,
            } => TrustAdmission::for_provider_plan(row.commitment.clone(), report_identity, digest),
            PreparedTrustIdentity::NativeRequirement {
                report_identity,
                contract,
                plan,
            } => TrustAdmission::for_native_requirement(
                row.commitment.clone(),
                report_identity,
                &contract,
                plan,
            ),
            PreparedTrustIdentity::MachineTemplate {
                report_identity,
                declaration,
            } => TrustAdmission::for_machine_template(
                row.commitment.clone(),
                report_identity,
                declaration,
            ),
            PreparedTrustIdentity::AcceptedMachine(contract) => {
                let contract = contract.map_err(|cardinality| {
                    let reason = match cardinality {
                        TrustDeclarationCardinality::Missing => {
                            "no exact checked machine contract plan"
                        }
                        TrustDeclarationCardinality::Duplicate => {
                            "duplicate exact checked machine contract plans"
                        }
                    };
                    vec![Diagnostic::error(format!(
                        "accepted trust obligation `{}` has {reason}",
                        row.commitment
                    ))]
                })?;
                TrustAdmission::for_machine_contract(
                    row.commitment.clone(),
                    contract.report_fingerprint,
                    contract,
                )
            }
        }
        .map_err(|error| vec![Diagnostic::error(error)])?;
        if let Some(previous) = resolved.get(&row.commitment) {
            if previous == &admission {
                continue;
            }
            return Err(vec![Diagnostic::error(format!(
                "root grants have different exact subjects for commitment `{}`",
                row.commitment
            ))]);
        }
        resolved.insert(row.commitment, admission);
    }
    Ok(resolved.into_values().collect())
}
