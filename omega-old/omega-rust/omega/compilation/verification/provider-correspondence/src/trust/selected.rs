//! Verify root grants against the immutable leaves, plans and identities of one selected target.
use diagnostics::Diagnostic;

pub struct SelectedTrustEvidence<'a> {
    pub program: terminal_psi::TerminalPsiIdentity,
    pub target: Option<&'a str>,
    pub plans: &'a effects::SelectedProviderPlanFacts,
    pub leaves: &'a [backend_plan::DirectBoundaryLeaf],
    pub bindings: &'a [effects::provider_plan::EvaluatedNativeBinding],
    pub identities: &'a terminal_psi::TerminalIdentityTable,
}

impl SelectedTrustEvidence<'_> {
    pub fn admit_root_grants(
        &self,
        declarations: &declaration_contracts::trust_declarations::TrustDeclarations,
        grants: &[declaration_contracts::selection::AuthoredRootGrant],
        policy: super::RootTrustPolicy,
        artifact: Option<terminal_codec::TerminalArtifactView<'_>>,
    ) -> Result<Vec<super::TrustAdmission>, Vec<Diagnostic>> {
        let plans = self.plans;
        let native = self.native_grant_evidence(grants, artifact)?;
        super::reconstruct_trust_obligations(
            declarations,
            grants,
            &plans.plans().iter().collect::<Vec<_>>(),
            plans,
            &native,
            policy,
        )
    }

    fn native_grant_evidence(
        &self,
        grants: &[declaration_contracts::selection::AuthoredRootGrant],
        artifact: Option<terminal_codec::TerminalArtifactView<'_>>,
    ) -> Result<Vec<Result<backend_plan::NativeRequirementTrust, String>>, Vec<Diagnostic>> {
        use declaration_contracts::selection::RootGrantSubject;
        let requests = grants
            .iter()
            .filter_map(|grant| {
                let RootGrantSubject::BoundaryRequirement {
                    declaration,
                    callable,
                } = &grant.subject
                else {
                    return None;
                };
                Some((grant, (declaration.as_str(), callable.as_str())))
            })
            .collect::<Vec<_>>();
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let mut leaves = self.leaves.iter().collect::<Vec<_>>();
        fn key(leaf: &backend_plan::DirectBoundaryLeaf) -> (&str, &str) {
            (&leaf.declaration_identity, &leaf.callable_identity)
        }
        leaves.sort_unstable_by(|left, right| key(left).cmp(&key(right)));
        let artifact = artifact
            .ok_or_else(|| vec![Diagnostic::error("native grant has no accepted program")])?;
        if artifact.identity() != self.program {
            return Err(vec![Diagnostic::error(
                "root grant evidence belongs to another published program",
            )]);
        }
        let module = artifact.module();
        Ok(requests.into_iter().map(|(grant, selected)| {
            let first = leaves.partition_point(|leaf| key(leaf) < selected);
            let last = leaves.partition_point(|leaf| key(leaf) <= selected);
            let [leaf] = &leaves[first..last] else {
                return Err(format!(
                    "root grant `{}` has no unique selected native requirement with its exact declaration and callable application",
                    grant.selector
                ));
            };
            leaf.prepare_native_trust(grant, self.target, module, self.bindings, self.identities)
        }).collect())
    }
}
