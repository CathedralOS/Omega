/*
We manage runtime-sized activation storage claims (`activation_claims`), loans
to external borrowers outside static checker visibility (`external_loans`), and
role-bound device ordering events (`ordering_events`) that gate authority
reacquisition across hardware boundaries.
*/

pub(crate) mod activation_claims {
    #[cfg(test)]
    mod tests {
        use super::{
            ActivationClaimBranch, ActivationClaimLedger, ActivationClaimProvenance,
            ActivationClaimRequest, ActivationClaimSiteId, ClaimBoundRow, ClaimEstablishmentError,
            compose_claim_bounds,
        };

        fn site(identity: u64) -> ActivationClaimSiteId {
            ActivationClaimSiteId::from_normalized_identity(identity).unwrap()
        }

        fn request(
            site_identity: u64,
            branch: ActivationClaimBranch,
            bound: u64,
            alignment: u64,
            committed: u64,
        ) -> ActivationClaimRequest {
            ActivationClaimRequest {
                site: site(site_identity),
                branch,
                bound,
                alignment,
                committed,
            }
        }

        #[test]
        fn establishment_admits_committed_within_bound() {
            let mut ledger = ActivationClaimLedger::new();
            let id = ledger
                .establish(request(1, ActivationClaimBranch::trunk(), 64, 8, 40))
                .unwrap();
            let claim = ledger.claim(id).unwrap();
            assert_eq!(claim.bound(), 64);
            assert_eq!(claim.committed(), 40);
            assert_eq!(
                claim.provenance(),
                ActivationClaimProvenance::ActivationStorage
            );
        }

        #[test]
        fn establishment_rejects_committed_beyond_bound_and_returns_the_request() {
            let mut ledger = ActivationClaimLedger::new();
            let error = ledger
                .establish(request(1, ActivationClaimBranch::trunk(), 64, 8, 65))
                .unwrap_err();
            let ClaimEstablishmentError {
                request,
                diagnostic,
            } = error;
            assert_eq!(request.committed, 65);
            assert!(diagnostic.0.contains("exceeds the claim's authored bound"));
            assert!(ledger.live_claims().next().is_none());
        }

        #[test]
        fn establishment_rejects_a_non_power_of_two_alignment() {
            let mut ledger = ActivationClaimLedger::new();
            let error = ledger
                .establish(request(1, ActivationClaimBranch::trunk(), 64, 3, 40))
                .unwrap_err();
            let ClaimEstablishmentError { diagnostic, .. } = error;
            assert!(diagnostic.0.contains("power of two"));
        }

        #[test]
        fn release_follows_reverse_establishment_order() {
            let mut ledger = ActivationClaimLedger::new();
            let first = ledger
                .establish(request(1, ActivationClaimBranch::trunk(), 64, 8, 40))
                .unwrap();
            let second = ledger
                .establish(request(2, ActivationClaimBranch::trunk(), 32, 8, 20))
                .unwrap();
            let diagnostic = ledger.release(first).unwrap_err();
            assert!(diagnostic.0.contains("later-established claim"));
            let released = ledger.release(second).unwrap();
            assert_eq!(released.id(), second);
            ledger.release(first).unwrap();
            ledger.drain().unwrap();
        }

        #[test]
        fn drain_rejects_while_any_claim_stays_live() {
            let mut ledger = ActivationClaimLedger::new();
            let claim = ledger
                .establish(request(1, ActivationClaimBranch::trunk(), 64, 8, 40))
                .unwrap();
            let error = ledger.drain().unwrap_err();
            assert_eq!(error.live.len(), 1);
            assert!(error.diagnostic.0.contains("cannot outlive"));
            assert_eq!(error.live[0].id(), claim);
        }

        #[test]
        fn split_children_cover_the_parent_exactly_and_merge_rejoins() {
            let mut ledger = ActivationClaimLedger::new();
            let parent = ledger
                .establish(request(1, ActivationClaimBranch::trunk(), 64, 8, 40))
                .unwrap();
            let (lower, upper) = ledger.split_claim(parent, 16, 24).unwrap();
            let lower_claim = ledger.claim(lower).unwrap();
            let upper_claim = ledger.claim(upper).unwrap();
            assert_eq!(lower_claim.committed() + upper_claim.committed(), 40);
            assert_eq!(lower_claim.bound() + upper_claim.bound(), 64);
            let merged = ledger.merge_claims(lower, upper).unwrap();
            let merged = ledger.claim(merged).unwrap();
            assert_eq!(merged.bound(), 64);
            assert_eq!(merged.committed(), 40);
        }

        #[test]
        fn split_rejects_children_that_lose_cover_or_violate_the_bound() {
            let mut ledger = ActivationClaimLedger::new();
            let parent = ledger
                .establish(request(1, ActivationClaimBranch::trunk(), 64, 8, 40))
                .unwrap();
            let diagnostic = ledger.split_claim(parent, 30, 10).unwrap_err();
            assert!(diagnostic.0.contains("committed <= bound"));
            assert_eq!(ledger.claim(parent).unwrap().bound(), 64);
            let _ = ledger.split_claim(parent, 0, 10).unwrap_err();
            let _ = ledger.split_claim(parent, 10, 0).unwrap_err();
            let _ = ledger.split_claim(parent, 10, 64).unwrap_err();
            assert_eq!(ledger.live_claims().count(), 1);
        }

        #[test]
        fn merge_requires_the_two_newest_live_claims() {
            let mut ledger = ActivationClaimLedger::new();
            let first = ledger
                .establish(request(1, ActivationClaimBranch::trunk(), 64, 8, 40))
                .unwrap();
            let (lower, _upper) = ledger.split_claim(first, 16, 24).unwrap();
            let second = ledger
                .establish(request(2, ActivationClaimBranch::trunk(), 32, 8, 20))
                .unwrap();
            let diagnostic = ledger.merge_claims(lower, second).unwrap_err();
            assert!(diagnostic.0.contains("two newest live claims"));
        }

        #[test]
        fn merge_rejects_independent_claim_roots() {
            let mut ledger = ActivationClaimLedger::new();
            let first = ledger
                .establish(request(1, ActivationClaimBranch::trunk(), 64, 8, 40))
                .unwrap();
            let second = ledger
                .establish(request(2, ActivationClaimBranch::trunk(), 32, 8, 20))
                .unwrap();
            let diagnostic = ledger.merge_claims(first, second).unwrap_err();
            assert!(diagnostic.0.contains("no merge sibling"));
        }

        #[test]
        fn composition_takes_the_maximum_across_mutually_exclusive_branches() {
            let trunk = ActivationClaimBranch::trunk();
            let left = trunk.extend(0);
            let right = trunk.extend(1);
            let rows = [
                ClaimBoundRow {
                    bound: Some(100),
                    alignment: 16,
                    branch: &trunk,
                },
                ClaimBoundRow {
                    bound: Some(300),
                    alignment: 16,
                    branch: &left,
                },
                ClaimBoundRow {
                    bound: Some(500),
                    alignment: 16,
                    branch: &right,
                },
            ];
            assert_eq!(compose_claim_bounds(rows).unwrap(), 624);
            assert!(left.mutually_exclusive(&right));
            assert!(!trunk.mutually_exclusive(&left));
        }

        #[test]
        fn composition_rejects_an_unbounded_claim_as_open_demand() {
            let trunk = ActivationClaimBranch::trunk();
            let rows = [ClaimBoundRow {
                bound: None,
                alignment: 8,
                branch: &trunk,
            }];
            let error = compose_claim_bounds(rows).unwrap_err();
            assert!(error.0.contains("demand open"));
        }

        #[test]
        fn ledger_composed_demand_matches_the_bound_rules() {
            let mut ledger = ActivationClaimLedger::new();
            let trunk = ActivationClaimBranch::trunk();
            ledger
                .establish(request(1, trunk.clone(), 10, 8, 10))
                .unwrap();
            ledger
                .establish(request(2, trunk.extend(0), 70, 16, 32))
                .unwrap();
            ledger
                .establish(request(3, trunk.extend(1), 30, 8, 30))
                .unwrap();
            assert_eq!(ledger.composed_demand().unwrap(), 96);
        }
    }

    use crate::extent::diagnostic::{ExtentDiagnostic, nonzero_identity};
    use crate::extent::{Lineage, SplitBranch};
    use crate::identities::ExtentLineageId;
    use crate::identities::normalized_extent_identity;

    normalized_extent_identity! {
        ActivationClaimSiteId, "activation-claim-site"
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ActivationClaimId(u64);

    impl ActivationClaimId {
        pub const fn normalized_identity(self) -> u64 {
            self.0
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ActivationClaimBranch(Vec<u32>);

    impl ActivationClaimBranch {
        pub fn trunk() -> Self {
            Self(Vec::new())
        }

        pub fn extend(&self, branch: u32) -> Self {
            let mut path = self.0.clone();
            path.push(branch);
            Self(path)
        }

        pub fn covers(&self, other: &Self) -> bool {
            other.0.starts_with(&self.0)
        }

        pub fn mutually_exclusive(&self, other: &Self) -> bool {
            !self.covers(other) && !other.covers(self)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ActivationClaimProvenance {
        ActivationStorage,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct ClaimBoundRow<'a> {
        pub bound: Option<u64>,
        pub alignment: u64,
        pub branch: &'a ActivationClaimBranch,
    }

    fn align_up(value: u64, alignment: u64) -> Result<u64, ExtentDiagnostic> {
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(ExtentDiagnostic(
                "activation claim alignment must be a nonzero power of two".into(),
            ));
        }
        value
            .checked_add(alignment - 1)
            .map(|sum| sum & !(alignment - 1))
            .ok_or_else(|| ExtentDiagnostic("aligned activation claim bound overflows".into()))
    }

    pub fn compose_claim_bounds<'a>(
        rows: impl IntoIterator<Item = ClaimBoundRow<'a>>,
    ) -> Result<u64, ExtentDiagnostic> {
        let rows: Vec<ClaimBoundRow<'a>> = rows.into_iter().collect();
        let mut aligned = Vec::with_capacity(rows.len());
        for row in &rows {
            let Some(bound) = row.bound else {
                return Err(ExtentDiagnostic(
                    "an unbounded activation claim leaves the activation's stack demand open"
                        .into(),
                ));
            };
            aligned.push(align_up(bound, row.alignment)?);
        }
        let mut worst = 0u64;
        for (row, _) in rows.iter().zip(aligned.iter()) {
            let mut sum = 0u64;
            for (inner, contribution) in rows.iter().zip(aligned.iter()) {
                if inner.branch.covers(row.branch) {
                    sum = sum.checked_add(*contribution).ok_or_else(|| {
                        ExtentDiagnostic("activation claim bound composition overflows".into())
                    })?;
                }
            }
            worst = worst.max(sum);
        }
        Ok(worst)
    }

    #[derive(Debug)]
    pub struct ActivationClaimRequest {
        pub site: ActivationClaimSiteId,
        pub branch: ActivationClaimBranch,
        pub bound: u64,
        pub alignment: u64,
        pub committed: u64,
    }

    pub fn validate_committed_within_bound(
        bound: u64,
        committed: u64,
    ) -> Result<(), ExtentDiagnostic> {
        if committed > bound {
            return Err(ExtentDiagnostic(format!(
                "committed extent {committed} exceeds the claim's authored bound {bound}"
            )));
        }
        Ok(())
    }

    #[derive(Debug)]
    pub struct ActivationClaim {
        id: ActivationClaimId,
        site: ActivationClaimSiteId,
        branch: ActivationClaimBranch,
        bound: u64,
        alignment: u64,
        committed: u64,
        provenance: ActivationClaimProvenance,
        lineage: Lineage,
    }

    impl ActivationClaim {
        pub const fn id(&self) -> ActivationClaimId {
            self.id
        }

        pub const fn site(&self) -> ActivationClaimSiteId {
            self.site
        }

        pub const fn branch(&self) -> &ActivationClaimBranch {
            &self.branch
        }

        pub const fn bound(&self) -> u64 {
            self.bound
        }

        pub const fn alignment(&self) -> u64 {
            self.alignment
        }

        pub const fn committed(&self) -> u64 {
            self.committed
        }

        pub const fn provenance(&self) -> ActivationClaimProvenance {
            self.provenance
        }

        fn bound_row(&self) -> ClaimBoundRow<'_> {
            ClaimBoundRow {
                bound: Some(self.bound),
                alignment: self.alignment,
                branch: &self.branch,
            }
        }
    }

    #[derive(Debug)]
    pub struct ClaimEstablishmentError {
        pub request: ActivationClaimRequest,
        pub diagnostic: ExtentDiagnostic,
    }

    #[derive(Debug)]
    pub struct ClaimDrainError {
        pub live: Vec<ActivationClaim>,
        pub diagnostic: ExtentDiagnostic,
    }

    #[derive(Debug, Default)]
    pub struct ActivationClaimLedger {
        next_identity: u64,
        live: Vec<ActivationClaim>,
    }

    impl ActivationClaimLedger {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn live_claims(&self) -> impl Iterator<Item = &ActivationClaim> {
            self.live.iter()
        }

        pub fn claim(&self, id: ActivationClaimId) -> Option<&ActivationClaim> {
            self.live.iter().find(|claim| claim.id == id)
        }

        fn live_position(&self, id: ActivationClaimId) -> Result<usize, ExtentDiagnostic> {
            let Some(position) = self.live.iter().position(|claim| claim.id == id) else {
                return Err(ExtentDiagnostic(format!(
                    "claim {} is not live in this activation",
                    id.0
                )));
            };
            Ok(position)
        }

        pub fn establish(
            &mut self,
            request: ActivationClaimRequest,
        ) -> Result<ActivationClaimId, ClaimEstablishmentError> {
            if let Err(diagnostic) = align_up(request.bound, request.alignment) {
                return Err(ClaimEstablishmentError {
                    request,
                    diagnostic,
                });
            }
            if let Err(diagnostic) =
                validate_committed_within_bound(request.bound, request.committed)
            {
                return Err(ClaimEstablishmentError {
                    request,
                    diagnostic,
                });
            }
            Ok(self.push_established(ActivationClaim {
                id: ActivationClaimId(0),
                site: request.site,
                branch: request.branch,
                bound: request.bound,
                alignment: request.alignment,
                committed: request.committed,
                provenance: ActivationClaimProvenance::ActivationStorage,
                lineage: Lineage {
                    root: ExtentLineageId::from_normalized_identity(self.next_identity + 1)
                        .expect("claim identities are nonzero"),
                    path: Vec::new(),
                },
            }))
        }

        pub fn release(
            &mut self,
            id: ActivationClaimId,
        ) -> Result<ActivationClaim, ExtentDiagnostic> {
            match self.live.last() {
                Some(newest) if newest.id == id => {
                    Ok(self.live.pop().expect("newest claim is live"))
                }
                Some(newest) => Err(ExtentDiagnostic(format!(
                    "claim {} cannot release while later-established claim {} stays live",
                    id.0, newest.id.0
                ))),
                None => Err(ExtentDiagnostic(format!(
                    "claim {} is not live in this activation",
                    id.0
                ))),
            }
        }

        pub fn split_claim(
            &mut self,
            id: ActivationClaimId,
            committed_at: u64,
            bound_at: u64,
        ) -> Result<(ActivationClaimId, ActivationClaimId), ExtentDiagnostic> {
            let position = self.live_position(id)?;
            let claim = &self.live[position];
            if committed_at == 0
                || committed_at >= claim.committed
                || bound_at == 0
                || bound_at >= claim.bound
            {
                return Err(ExtentDiagnostic(
                    "claim split must produce two nonempty children".into(),
                ));
            }
            if committed_at > bound_at || claim.committed - committed_at > claim.bound - bound_at {
                return Err(ExtentDiagnostic(
                    "claim split children must each satisfy committed <= bound".into(),
                ));
            }
            let claim = self.live.remove(position);
            let mut lower_path = claim.lineage.path.clone();
            lower_path.push(SplitBranch::Lower);
            let mut upper_path = claim.lineage.path;
            upper_path.push(SplitBranch::Upper);
            let lower_id = self.push_established(ActivationClaim {
                bound: bound_at,
                committed: committed_at,
                id: ActivationClaimId(0),
                site: claim.site,
                branch: claim.branch.clone(),
                alignment: claim.alignment,
                provenance: claim.provenance,
                lineage: Lineage {
                    root: claim.lineage.root,
                    path: lower_path,
                },
            });
            let upper_id = self.push_established(ActivationClaim {
                bound: claim.bound - bound_at,
                committed: claim.committed - committed_at,
                id: ActivationClaimId(0),
                site: claim.site,
                branch: claim.branch,
                alignment: claim.alignment,
                provenance: claim.provenance,
                lineage: Lineage {
                    root: claim.lineage.root,
                    path: upper_path,
                },
            });
            Ok((lower_id, upper_id))
        }

        pub fn merge_claims(
            &mut self,
            lower_id: ActivationClaimId,
            upper_id: ActivationClaimId,
        ) -> Result<ActivationClaimId, ExtentDiagnostic> {
            let lower_position = self.live_position(lower_id)?;
            let upper_position = self.live_position(upper_id)?;
            if lower_position != self.live.len() - 2 || upper_position != self.live.len() - 1 {
                return Err(ExtentDiagnostic(
                    "claim children merge only as the two newest live claims".into(),
                ));
            }
            let lower = &self.live[lower_position];
            let upper = &self.live[upper_position];
            let Some((lower_branch, lower_parent)) = lower.lineage.path.split_last() else {
                return Err(ExtentDiagnostic("a root claim has no merge sibling".into()));
            };
            let Some((upper_branch, upper_parent)) = upper.lineage.path.split_last() else {
                return Err(ExtentDiagnostic("a root claim has no merge sibling".into()));
            };
            if lower.lineage.root != upper.lineage.root
                || lower_branch != &SplitBranch::Lower
                || upper_branch != &SplitBranch::Upper
                || lower_parent != upper_parent
            {
                return Err(ExtentDiagnostic(
                    "claim merge requires exact split siblings of one lineage".into(),
                ));
            }
            if lower.site != upper.site || lower.branch != upper.branch {
                return Err(ExtentDiagnostic(
                    "claim merge requires identical site and branch".into(),
                ));
            }
            let merged = ActivationClaim {
                bound: lower.bound + upper.bound,
                committed: lower.committed + upper.committed,
                id: ActivationClaimId(0),
                site: lower.site,
                branch: lower.branch.clone(),
                alignment: lower.alignment,
                provenance: lower.provenance,
                lineage: Lineage {
                    root: lower.lineage.root,
                    path: lower_parent.to_vec(),
                },
            };
            self.live.pop();
            self.live.pop();
            Ok(self.push_established(merged))
        }

        pub fn drain(self) -> Result<(), ClaimDrainError> {
            if self.live.is_empty() {
                Ok(())
            } else {
                Err(ClaimDrainError {
                    diagnostic: ExtentDiagnostic(format!(
                        "{} live activation claim(s) cannot outlive the activation",
                        self.live.len()
                    )),
                    live: self.live,
                })
            }
        }

        pub fn composed_demand(&self) -> Result<u64, ExtentDiagnostic> {
            compose_claim_bounds(self.live.iter().map(|claim| claim.bound_row()))
        }

        fn push_established(&mut self, mut claim: ActivationClaim) -> ActivationClaimId {
            self.next_identity += 1;
            claim.id = ActivationClaimId(self.next_identity);
            let id = claim.id;
            self.live.push(claim);
            id
        }
    }
}

pub(crate) mod external_loans {
    use crate::extent::diagnostic::{ExtentDiagnostic, nonzero_identity};
    use crate::identities::normalized_extent_identity;
    use crate::identities::{
        AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRights, MappingEraId,
        NormalizedIdentitySet,
    };
    use crate::loans::{ExtentLoan, LoanPolarity};
    normalized_extent_identity! { ExternalBorrowerId, "external-borrower" }
    normalized_extent_identity! { ExternalLoanId, "external-loan" }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ExternalLoanDirection {
        DeviceReads,
        DeviceWrites,
    }

    normalized_extent_identity! { ExternalCompletionFactId, "external-completion-fact" }
    normalized_extent_identity! { ExternalReachReceiptId, "external-reach receipt" }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ExternalReachMechanism {
        AdmittedBorrowerContract,
        HardwareIsolation,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct ExternalReachReceipt {
        identity: ExternalReachReceiptId,
        loan: ExternalLoanId,
        borrower: ExternalBorrowerId,
        direction: ExternalLoanDirection,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        lineage: ExtentLineageId,
        rights: ExtentRights,
        base: u64,
        length: u64,
        mechanism: ExternalReachMechanism,
        confined_to_range: bool,
    }

    impl ExternalReachReceipt {
        pub fn from_admitted_provider(
            identity: ExternalReachReceiptId,
            loan_identity: ExternalLoanId,
            grant: &ExternalLoanGrant,
            loan: &ExtentLoan<'_>,
            mechanism: ExternalReachMechanism,
            confined_to_range: bool,
        ) -> Self {
            Self {
                identity,
                loan: loan_identity,
                borrower: grant.borrower,
                direction: grant.direction,
                address_space: loan.address_space(),
                provenance: loan.provenance(),
                era: loan.era(),
                lineage: loan.lineage_root(),
                rights: loan.rights().clone(),
                base: loan.base(),
                length: loan.length(),
                mechanism,
                confined_to_range,
            }
        }
    }

    pub type CompletionObligations = NormalizedIdentitySet<ExternalCompletionFactId>;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExternalLoanGrant {
        borrower: ExternalBorrowerId,
        direction: ExternalLoanDirection,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        required_rights: ExtentRights,
        completion: CompletionObligations,
    }

    impl ExternalLoanGrant {
        pub fn from_admitted_provider(
            borrower: ExternalBorrowerId,
            direction: ExternalLoanDirection,
            address_space: AddressSpaceId,
            provenance: ExtentProvenanceId,
            required_rights: ExtentRights,
            completion: CompletionObligations,
        ) -> Self {
            Self {
                borrower,
                direction,
                address_space,
                provenance,
                required_rights,
                completion,
            }
        }
    }

    #[derive(Debug)]
    pub struct ExternalLoan<'extent> {
        identity: ExternalLoanId,
        borrower: ExternalBorrowerId,
        direction: ExternalLoanDirection,
        completion: CompletionObligations,
        reach_receipt: ExternalReachReceiptId,
        reach_mechanism: ExternalReachMechanism,
        loan: ExtentLoan<'extent>,
    }

    impl<'extent> ExternalLoan<'extent> {
        pub const fn identity(&self) -> ExternalLoanId {
            self.identity
        }

        pub const fn borrower(&self) -> ExternalBorrowerId {
            self.borrower
        }

        pub const fn direction(&self) -> ExternalLoanDirection {
            self.direction
        }

        pub const fn reach_receipt(&self) -> ExternalReachReceiptId {
            self.reach_receipt
        }

        pub const fn reach_mechanism(&self) -> ExternalReachMechanism {
            self.reach_mechanism
        }

        pub const fn base(&self) -> u64 {
            self.loan.base()
        }

        pub const fn length(&self) -> u64 {
            self.loan.length()
        }

        pub fn complete(
            self,
            receipt: ExternalCompletionReceipt,
        ) -> Result<ExternalLoanCompletion, Box<ExternalCompletionError<'extent>>> {
            let mismatch = if receipt.loan != self.identity {
                Some("completion receipt names a different external loan")
            } else if receipt.borrower != self.borrower {
                Some("completion receipt names a different external borrower")
            } else if receipt.direction != self.direction {
                Some("completion receipt names a different transfer direction")
            } else if receipt.reach_receipt != self.reach_receipt {
                Some("completion receipt names different confinement evidence")
            } else if receipt.address_space != self.loan.address_space() {
                Some("completion receipt names a different address space")
            } else if receipt.provenance != self.loan.provenance() {
                Some("completion receipt names different extent provenance")
            } else if receipt.era != self.loan.era() {
                Some("completion receipt names a stale mapping era")
            } else if receipt.lineage != self.loan.lineage_root() {
                Some("completion receipt names a different extent authority lineage")
            } else if receipt.rights != *self.loan.rights() {
                Some("completion receipt names different attenuated extent rights")
            } else if receipt.base != self.loan.base() || receipt.length != self.loan.length() {
                Some("completion receipt names a different lent extent range")
            } else if !receipt.borrow_released {
                Some("completion receipt does not establish external-borrow release")
            } else if !receipt.established_facts.contains(&self.completion) {
                Some("completion receipt lacks facts required by the external-loan grant")
            } else {
                None
            };

            if let Some(message) = mismatch {
                return Err(Box::new(ExternalCompletionError {
                    loan: self,
                    receipt,
                    diagnostic: ExtentDiagnostic(message.into()),
                }));
            }

            Ok(ExternalLoanCompletion {
                loan: self.identity,
                borrower: self.borrower,
                reach_receipt: self.reach_receipt,
                base: self.loan.base(),
                length: self.loan.length(),
            })
        }
    }

    pub fn begin_external_loan<'extent>(
        loan: ExtentLoan<'extent>,
        identity: ExternalLoanId,
        grant: &ExternalLoanGrant,
        reach_receipt: Option<ExternalReachReceipt>,
    ) -> Result<ExternalLoan<'extent>, Box<ExternalLoanStartError<'extent>>> {
        let mismatch = if loan.address_space() != grant.address_space {
            Some("extent address space does not match external-loan grant")
        } else if loan.provenance() != grant.provenance {
            Some("extent provenance does not match external-loan grant")
        } else if !loan.rights().contains(&grant.required_rights) {
            Some("extent lacks rights required by external-loan grant")
        } else if grant.direction == ExternalLoanDirection::DeviceReads
            && loan.polarity() != LoanPolarity::Shared
        {
            Some("device-read lending requires a shared extent loan")
        } else if grant.direction == ExternalLoanDirection::DeviceWrites
            && loan.polarity() != LoanPolarity::Exclusive
        {
            Some("device-write lending requires an exclusive extent loan")
        } else {
            None
        };

        if let Some(message) = mismatch {
            return Err(Box::new(ExternalLoanStartError {
                loan,
                reach_receipt,
                diagnostic: ExtentDiagnostic(message.into()),
            }));
        }

        let reach_mismatch = match reach_receipt.as_ref() {
            None => {
                Some("external borrower requires exact-range reach evidence or hardware isolation")
            }
            Some(receipt) if receipt.loan != identity => {
                Some("external-reach receipt names a different external loan")
            }
            Some(receipt) if receipt.borrower != grant.borrower => {
                Some("external-reach receipt names a different borrower")
            }
            Some(receipt) if receipt.direction != grant.direction => {
                Some("external-reach receipt names a different transfer direction")
            }
            Some(receipt) if receipt.address_space != loan.address_space() => {
                Some("external-reach receipt names a different address space")
            }
            Some(receipt) if receipt.provenance != loan.provenance() => {
                Some("external-reach receipt names different extent provenance")
            }
            Some(receipt) if receipt.era != loan.era() => {
                Some("external-reach receipt names a stale mapping era")
            }
            Some(receipt) if receipt.lineage != loan.lineage_root() => {
                Some("external-reach receipt names a different extent authority lineage")
            }
            Some(receipt) if receipt.rights != *loan.rights() => {
                Some("external-reach receipt names different attenuated extent rights")
            }
            Some(receipt) if receipt.base != loan.base() || receipt.length != loan.length() => {
                Some("external borrower reach is not the exact lent extent range")
            }
            Some(receipt) if !receipt.confined_to_range => {
                Some("external-reach receipt does not establish range confinement")
            }
            Some(_) => None,
        };
        if let Some(message) = reach_mismatch {
            return Err(Box::new(ExternalLoanStartError {
                loan,
                reach_receipt,
                diagnostic: ExtentDiagnostic(message.into()),
            }));
        }
        let reach_receipt = reach_receipt.expect("validated external-reach receipt");

        Ok(ExternalLoan {
            identity,
            borrower: grant.borrower,
            direction: grant.direction,
            completion: grant.completion.clone(),
            reach_receipt: reach_receipt.identity,
            reach_mechanism: reach_receipt.mechanism,
            loan,
        })
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct ExternalCompletionReceipt {
        loan: ExternalLoanId,
        borrower: ExternalBorrowerId,
        direction: ExternalLoanDirection,
        reach_receipt: ExternalReachReceiptId,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        lineage: ExtentLineageId,
        rights: ExtentRights,
        base: u64,
        length: u64,
        borrow_released: bool,
        established_facts: CompletionObligations,
    }

    impl ExternalCompletionReceipt {
        pub fn from_admitted_provider(
            loan: &ExternalLoan<'_>,
            borrow_released: bool,
            established_facts: impl IntoIterator<Item = ExternalCompletionFactId>,
        ) -> Self {
            Self {
                loan: loan.identity,
                borrower: loan.borrower,
                direction: loan.direction,
                reach_receipt: loan.reach_receipt,
                address_space: loan.loan.address_space(),
                provenance: loan.loan.provenance(),
                era: loan.loan.era(),
                lineage: loan.loan.lineage_root(),
                rights: loan.loan.rights().clone(),
                base: loan.loan.base(),
                length: loan.loan.length(),
                borrow_released,
                established_facts: CompletionObligations::from_normalized_identities(
                    established_facts,
                ),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ExternalLoanCompletion {
        pub loan: ExternalLoanId,
        pub borrower: ExternalBorrowerId,
        pub reach_receipt: ExternalReachReceiptId,
        pub base: u64,
        pub length: u64,
    }

    #[derive(Debug)]
    pub struct ExternalLoanStartError<'extent> {
        loan: ExtentLoan<'extent>,
        reach_receipt: Option<ExternalReachReceipt>,
        diagnostic: ExtentDiagnostic,
    }

    impl<'extent> ExternalLoanStartError<'extent> {
        pub const fn diagnostic(&self) -> &ExtentDiagnostic {
            &self.diagnostic
        }

        pub fn into_loan(self) -> ExtentLoan<'extent> {
            self.loan
        }

        pub fn into_parts(self) -> (ExtentLoan<'extent>, Option<ExternalReachReceipt>) {
            (self.loan, self.reach_receipt)
        }
    }

    #[derive(Debug)]
    pub struct ExternalCompletionError<'extent> {
        loan: ExternalLoan<'extent>,
        receipt: ExternalCompletionReceipt,
        diagnostic: ExtentDiagnostic,
    }

    impl<'extent> ExternalCompletionError<'extent> {
        pub const fn diagnostic(&self) -> &ExtentDiagnostic {
            &self.diagnostic
        }

        pub fn into_parts(self) -> (ExternalLoan<'extent>, ExternalCompletionReceipt) {
            (self.loan, self.receipt)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            CompletionObligations, ExternalBorrowerId, ExternalCompletionFactId,
            ExternalCompletionReceipt, ExternalLoanDirection, ExternalLoanGrant, ExternalLoanId,
            ExternalReachMechanism, ExternalReachReceipt, ExternalReachReceiptId,
            begin_external_loan,
        };
        use crate::tests::{grant, id, rights};
        use crate::{AddressSpaceId, ExtentLoan, ExtentProvenanceId};

        fn external_grant(
            direction: ExternalLoanDirection,
            completion: CompletionObligations,
        ) -> ExternalLoanGrant {
            ExternalLoanGrant::from_admitted_provider(
                id(500, ExternalBorrowerId::from_normalized_identity),
                direction,
                id(10, AddressSpaceId::from_normalized_identity),
                id(20, ExtentProvenanceId::from_normalized_identity),
                rights(&[100]),
                completion,
            )
        }

        fn loan_id(identity: u64) -> ExternalLoanId {
            id(identity, ExternalLoanId::from_normalized_identity)
        }

        fn completion_fact(identity: u64) -> ExternalCompletionFactId {
            id(identity, ExternalCompletionFactId::from_normalized_identity)
        }

        fn reach_receipt(
            identity: ExternalLoanId,
            grant: &ExternalLoanGrant,
            loan: &ExtentLoan<'_>,
            mechanism: ExternalReachMechanism,
        ) -> ExternalReachReceipt {
            ExternalReachReceipt::from_admitted_provider(
                id(800, ExternalReachReceiptId::from_normalized_identity),
                identity,
                grant,
                loan,
                mechanism,
                true,
            )
        }

        #[test]
        fn external_read_loan_requires_completion_facts_before_releasing_borrow() {
            let extent = grant(1, 0x1000, 64);
            let loan = extent.loan(0, 32).expect("shared DMA source");
            let grant = external_grant(
                ExternalLoanDirection::DeviceReads,
                CompletionObligations::from_normalized_identities([
                    completion_fact(700),
                    completion_fact(701),
                ]),
            );
            let reach = reach_receipt(
                loan_id(600),
                &grant,
                &loan,
                ExternalReachMechanism::AdmittedBorrowerContract,
            );
            let transfer = begin_external_loan(loan, loan_id(600), &grant, Some(reach))
                .expect("admitted external read");
            assert_eq!(transfer.direction(), ExternalLoanDirection::DeviceReads);

            let incomplete = ExternalCompletionReceipt::from_admitted_provider(
                &transfer,
                true,
                [completion_fact(701)],
            );
            let error = transfer
                .complete(incomplete)
                .expect_err("required device fence is missing");
            assert!(error.diagnostic().0.contains("lacks facts"));
            let (transfer, _) = (*error).into_parts();

            let complete = ExternalCompletionReceipt::from_admitted_provider(
                &transfer,
                true,
                [completion_fact(700), completion_fact(701)],
            );
            let completion = transfer.complete(complete).expect("completed DMA read");
            assert_eq!((completion.base, completion.length), (0x1000, 32));
        }

        #[test]
        fn external_write_loan_derives_exclusive_cpu_exclusion() {
            let mut extent = grant(1, 0x2000, 64);
            let shared = extent.loan(0, 16).expect("shared loan");
            let write_grant = external_grant(
                ExternalLoanDirection::DeviceWrites,
                CompletionObligations::default(),
            );
            let shared_reach = reach_receipt(
                loan_id(601),
                &write_grant,
                &shared,
                ExternalReachMechanism::HardwareIsolation,
            );
            let error = begin_external_loan(shared, loan_id(601), &write_grant, Some(shared_reach))
                .expect_err("device mutation needs exclusive custody");
            assert!(error.diagnostic().0.contains("exclusive"));

            let exclusive = extent.loan_mut(0, 16).expect("exclusive loan");
            let exclusive_reach = reach_receipt(
                loan_id(602),
                &write_grant,
                &exclusive,
                ExternalReachMechanism::HardwareIsolation,
            );
            let transfer =
                begin_external_loan(exclusive, loan_id(602), &write_grant, Some(exclusive_reach))
                    .expect("admitted external write");
            let receipt = ExternalCompletionReceipt::from_admitted_provider(&transfer, true, []);
            let completion = transfer.complete(receipt).expect("completed DMA write");
            assert_eq!(completion.borrower.normalized_identity(), 500);
            assert_eq!(completion.reach_receipt.normalized_identity(), 800);
        }

        #[test]
        fn external_agent_reach_must_equal_the_lent_extent_and_fail_closed() {
            let extent = grant(1, 0x3000, 128);
            let loan = extent.loan(32, 32).expect("DMA subrange");
            let read_grant = external_grant(
                ExternalLoanDirection::DeviceReads,
                CompletionObligations::default(),
            );
            let missing = begin_external_loan(loan, loan_id(603), &read_grant, None)
                .expect_err("an invisible borrower without reach evidence must reject");
            assert!(missing.diagnostic().0.contains("reach evidence"));
            let loan = (*missing).into_loan();

            let mut overbroad_reach = reach_receipt(
                loan_id(603),
                &read_grant,
                &loan,
                ExternalReachMechanism::HardwareIsolation,
            );
            overbroad_reach.base = 0x3000;
            overbroad_reach.length = 128;
            let overbroad =
                begin_external_loan(loan, loan_id(603), &read_grant, Some(overbroad_reach))
                    .expect_err("whole-parent reach exceeds the exact lent subrange");
            assert!(overbroad.diagnostic().0.contains("exact lent extent range"));
            let loan = (*overbroad).into_loan();

            let exact_reach = reach_receipt(
                loan_id(603),
                &read_grant,
                &loan,
                ExternalReachMechanism::HardwareIsolation,
            );
            let transfer = begin_external_loan(loan, loan_id(603), &read_grant, Some(exact_reach))
                .expect("exact subrange isolation");
            assert_eq!((transfer.base(), transfer.length()), (0x3020, 32));
            assert_eq!(
                transfer.reach_mechanism(),
                ExternalReachMechanism::HardwareIsolation
            );
        }

        #[test]
        fn external_completion_cannot_replay_after_lent_authority_drift() {
            let first_extent = grant(1, 0x4000, 64);
            let read_grant = external_grant(
                ExternalLoanDirection::DeviceReads,
                CompletionObligations::default(),
            );
            let first_loan = first_extent.loan(0, 16).expect("first DMA range");
            let first_reach = reach_receipt(
                loan_id(604),
                &read_grant,
                &first_loan,
                ExternalReachMechanism::HardwareIsolation,
            );
            let first =
                begin_external_loan(first_loan, loan_id(604), &read_grant, Some(first_reach))
                    .expect("first external loan");
            let stale = ExternalCompletionReceipt::from_admitted_provider(&first, true, []);

            let second_extent = grant(2, 0x4000, 64);
            let second_loan = second_extent.loan(0, 16).expect("second DMA range");
            let second_reach = reach_receipt(
                loan_id(604),
                &read_grant,
                &second_loan,
                ExternalReachMechanism::HardwareIsolation,
            );
            let second =
                begin_external_loan(second_loan, loan_id(604), &read_grant, Some(second_reach))
                    .expect("second external loan");

            let error = second
                .complete(stale)
                .expect_err("completion for another authority lineage must not replay");
            assert!(error.diagnostic().0.contains("authority lineage"));
        }
    }
}

pub(crate) mod ordering_events {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::extent::diagnostic::{ExtentDiagnostic, nonzero_identity, validate_range};
    use crate::external_loans::ExternalLoanId;
    use crate::identities::normalized_extent_identity;
    use crate::identities::{AddressSpaceId, ExtentLineageId, ExtentProvenanceId, MappingEraId};
    use crate::mapping::MappingId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum DeviceOrderingRole {
        Publication,
        CacheMaintenance,
        Notification,
        Completion,
        Acquisition,
    }

    impl DeviceOrderingRole {
        pub const fn name(self) -> &'static str {
            match self {
                Self::Publication => "publication",
                Self::CacheMaintenance => "cache-maintenance",
                Self::Notification => "notification",
                Self::Completion => "completion",
                Self::Acquisition => "acquisition",
            }
        }
    }

    normalized_extent_identity! { OrderingEventId, "ordering-event" }
    normalized_extent_identity! {
        DeviceInstanceId, "device-instance"
    }
    normalized_extent_identity! {
        RuntimeScopeId, "runtime-scope"
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum OrderingCoordinateKind {
        Data,
        Descriptor,
        Doorbell,
        ReadBack,
        Request,
        Completion,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct OrderingRange {
        pub mapping: MappingId,
        pub base: u64,
        pub length: u64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OrderingAuthority {
        pub address_space: AddressSpaceId,
        pub provenance: ExtentProvenanceId,
        pub era: MappingEraId,
        pub lineage: ExtentLineageId,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct OrderingEvent {
        identity: OrderingEventId,
        role: DeviceOrderingRole,
        device: DeviceInstanceId,
        scope: RuntimeScopeId,
        request: ExternalLoanId,
        authority: OrderingAuthority,
        coordinates: BTreeMap<OrderingCoordinateKind, OrderingRange>,
    }

    impl OrderingEvent {
        pub fn from_admitted_provider(
            identity: OrderingEventId,
            role: DeviceOrderingRole,
            device: DeviceInstanceId,
            scope: RuntimeScopeId,
            request: ExternalLoanId,
            authority: OrderingAuthority,
            coordinates: impl IntoIterator<Item = (OrderingCoordinateKind, OrderingRange)>,
        ) -> Result<Self, ExtentDiagnostic> {
            let mut bound = BTreeMap::new();
            for (kind, range) in coordinates {
                validate_range(range.base, range.length)?;
                if bound.insert(kind, range).is_some() {
                    return Err(ExtentDiagnostic(format!(
                        "ordering event carries duplicate {:?} coordinate",
                        kind
                    )));
                }
            }
            if bound.is_empty() {
                return Err(ExtentDiagnostic(
                    "ordering event must bind at least one coordinate range".into(),
                ));
            }
            Ok(Self {
                identity,
                role,
                device,
                scope,
                request,
                authority,
                coordinates: bound,
            })
        }

        pub const fn identity(&self) -> OrderingEventId {
            self.identity
        }

        pub const fn role(&self) -> DeviceOrderingRole {
            self.role
        }

        pub const fn device(&self) -> DeviceInstanceId {
            self.device
        }

        pub const fn scope(&self) -> RuntimeScopeId {
            self.scope
        }

        pub const fn request(&self) -> ExternalLoanId {
            self.request
        }

        pub const fn authority(&self) -> OrderingAuthority {
            self.authority
        }

        pub fn coordinate(&self, kind: OrderingCoordinateKind) -> Option<OrderingRange> {
            self.coordinates.get(&kind).copied()
        }

        pub fn same_payload_different_role(&self, other: &Self) -> bool {
            self.role != other.role
                && self.device == other.device
                && self.scope == other.scope
                && self.request == other.request
                && self.authority == other.authority
                && self.coordinates == other.coordinates
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OrderingCoverageRequirement {
        pub device: DeviceInstanceId,
        pub scope: RuntimeScopeId,
        pub request: ExternalLoanId,
        pub authority: OrderingAuthority,
        pub roles: BTreeSet<DeviceOrderingRole>,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct OrderingCoverageAdmission {
        events: Vec<OrderingEvent>,
    }

    impl OrderingCoverageAdmission {
        pub fn events(&self) -> &[OrderingEvent] {
            &self.events
        }
    }

    #[derive(Debug)]
    pub struct OrderingCoverageRejection {
        events: Vec<OrderingEvent>,
        diagnostic: ExtentDiagnostic,
    }

    impl OrderingCoverageRejection {
        pub const fn diagnostic(&self) -> &ExtentDiagnostic {
            &self.diagnostic
        }

        pub fn into_events(self) -> Vec<OrderingEvent> {
            self.events
        }
    }

    pub fn admit_role_coverage(
        requirement: &OrderingCoverageRequirement,
        events: Vec<OrderingEvent>,
    ) -> Result<OrderingCoverageAdmission, OrderingCoverageRejection> {
        let reject = |events: Vec<OrderingEvent>, message: String| {
            Err(OrderingCoverageRejection {
                events,
                diagnostic: ExtentDiagnostic(message),
            })
        };

        let mut covered: BTreeSet<DeviceOrderingRole> = BTreeSet::new();
        let mut failure: Option<String> = None;
        for event in &events {
            if event.device != requirement.device {
                failure = Some("ordering event names a different stable device instance".into());
            } else if event.scope != requirement.scope {
                failure =
                    Some("ordering event names a different runtime queue/session scope".into());
            } else if event.request != requirement.request {
                failure = Some("ordering event names a different external-loan request".into());
            } else if event.authority != requirement.authority {
                failure = Some("ordering event binds drifted extent authority".into());
            } else if !requirement.roles.contains(&event.role) {
                failure = Some(format!(
                    "ordering event carries extra {:?} coverage",
                    event.role
                ));
            } else if !covered.insert(event.role) {
                failure = Some(format!(
                    "ordering event duplicates {:?} coverage",
                    event.role
                ));
            }
            if failure.is_some() {
                break;
            }
        }
        if let Some(message) = failure {
            return reject(events, message);
        }
        if covered != requirement.roles {
            let missing: Vec<&'static str> = requirement
                .roles
                .difference(&covered)
                .map(|role| role.name())
                .collect();
            return reject(
                events,
                format!(
                    "ordering coverage is missing {} role(s)",
                    missing.join(", ")
                ),
            );
        }
        Ok(OrderingCoverageAdmission { events })
    }

    pub fn admit_acquisition_after_completion(
        acquisition: &OrderingEvent,
        completion: &OrderingEvent,
    ) -> Result<(), ExtentDiagnostic> {
        let mismatch = if acquisition.role != DeviceOrderingRole::Acquisition {
            Some("event is not an acquisition")
        } else if completion.role != DeviceOrderingRole::Completion {
            Some("companion event is not a completion")
        } else if acquisition.request != completion.request {
            Some("acquisition names a different external-loan request")
        } else if acquisition.device != completion.device {
            Some("acquisition names a different stable device instance")
        } else if acquisition.scope != completion.scope {
            Some("acquisition names a different runtime queue/session scope")
        } else if acquisition.authority != completion.authority {
            Some("acquisition binds different extent authority")
        } else {
            None
        };
        mismatch.map_or(Ok(()), |message| Err(ExtentDiagnostic(message.into())))
    }

    #[cfg(test)]
    mod tests {
        use super::{
            DeviceInstanceId, DeviceOrderingRole, ExtentLineageId, ExtentProvenanceId,
            ExternalLoanId, MappingEraId, OrderingAuthority, OrderingCoordinateKind,
            OrderingCoverageRequirement, OrderingEvent, OrderingEventId, OrderingRange,
            RuntimeScopeId, admit_acquisition_after_completion, admit_role_coverage,
        };
        use crate::identities::AddressSpaceId;
        use crate::mapping::MappingId;

        fn authority() -> OrderingAuthority {
            OrderingAuthority {
                address_space: AddressSpaceId::from_normalized_identity(1).unwrap(),
                provenance: ExtentProvenanceId::from_normalized_identity(2).unwrap(),
                era: MappingEraId::from_normalized_identity(3).unwrap(),
                lineage: ExtentLineageId::from_normalized_identity(4).unwrap(),
            }
        }

        fn event(role: DeviceOrderingRole, identity: u64) -> OrderingEvent {
            OrderingEvent::from_admitted_provider(
                OrderingEventId::from_normalized_identity(identity).unwrap(),
                role,
                DeviceInstanceId::from_normalized_identity(10).unwrap(),
                RuntimeScopeId::from_normalized_identity(20).unwrap(),
                ExternalLoanId::from_normalized_identity(30).unwrap(),
                authority(),
                [(
                    OrderingCoordinateKind::Data,
                    OrderingRange {
                        mapping: MappingId::from_normalized_identity(40).unwrap(),
                        base: 0x1000,
                        length: 0x200,
                    },
                )],
            )
            .unwrap()
        }

        fn requirement(roles: &[DeviceOrderingRole]) -> OrderingCoverageRequirement {
            OrderingCoverageRequirement {
                device: DeviceInstanceId::from_normalized_identity(10).unwrap(),
                scope: RuntimeScopeId::from_normalized_identity(20).unwrap(),
                request: ExternalLoanId::from_normalized_identity(30).unwrap(),
                authority: authority(),
                roles: roles.iter().copied().collect(),
            }
        }

        #[test]
        fn role_discriminant_participates_in_identity() {
            let publication = event(DeviceOrderingRole::Publication, 100);
            let mut notification = event(DeviceOrderingRole::Notification, 100);
            notification.identity = publication.identity;
            assert!(publication.same_payload_different_role(&notification));
            assert_ne!(publication, notification);
        }

        #[test]
        fn complete_roster_admits() {
            let roles = [
                DeviceOrderingRole::Publication,
                DeviceOrderingRole::Notification,
                DeviceOrderingRole::Completion,
                DeviceOrderingRole::Acquisition,
            ];
            let events: Vec<_> = roles
                .iter()
                .enumerate()
                .map(|(index, role)| event(*role, index as u64 + 1))
                .collect();
            let admission = admit_role_coverage(&requirement(&roles), events).unwrap();
            assert_eq!(admission.events().len(), 4);
        }

        #[test]
        fn missing_role_rejects_and_returns_events() {
            let roles = [
                DeviceOrderingRole::Publication,
                DeviceOrderingRole::Completion,
            ];
            let events = vec![event(DeviceOrderingRole::Publication, 1)];
            let rejection = admit_role_coverage(&requirement(&roles), events).unwrap_err();
            assert!(rejection.diagnostic().to_string().contains("missing"));
            assert_eq!(rejection.into_events().len(), 1);
        }

        #[test]
        fn extra_role_rejects_and_returns_events() {
            let roles = [DeviceOrderingRole::Publication];
            let events = vec![
                event(DeviceOrderingRole::Publication, 1),
                event(DeviceOrderingRole::Completion, 2),
            ];
            let rejection = admit_role_coverage(&requirement(&roles), events).unwrap_err();
            assert!(rejection.diagnostic().to_string().contains("extra"));
            assert_eq!(rejection.into_events().len(), 2);
        }

        #[test]
        fn duplicate_role_rejects() {
            let roles = [DeviceOrderingRole::Publication];
            let events = vec![
                event(DeviceOrderingRole::Publication, 1),
                event(DeviceOrderingRole::Publication, 2),
            ];
            let rejection = admit_role_coverage(&requirement(&roles), events).unwrap_err();
            assert!(rejection.diagnostic().to_string().contains("duplicates"));
        }

        #[test]
        fn drifted_scope_rejects() {
            let roles = [DeviceOrderingRole::Publication];
            let mut drifted = event(DeviceOrderingRole::Publication, 1);
            drifted.scope = RuntimeScopeId::from_normalized_identity(99).unwrap();
            let rejection = admit_role_coverage(&requirement(&roles), vec![drifted]).unwrap_err();
            assert!(rejection.diagnostic().to_string().contains("scope"));
        }

        #[test]
        fn acquisition_requires_completion_on_same_request() {
            let completion = event(DeviceOrderingRole::Completion, 1);
            let acquisition = event(DeviceOrderingRole::Acquisition, 2);
            admit_acquisition_after_completion(&acquisition, &completion).unwrap();

            let mut wrong_scope = event(DeviceOrderingRole::Acquisition, 3);
            wrong_scope.scope = RuntimeScopeId::from_normalized_identity(77).unwrap();
            let error = admit_acquisition_after_completion(&wrong_scope, &completion).unwrap_err();
            assert!(error.to_string().contains("scope"));

            let notification = event(DeviceOrderingRole::Notification, 4);
            let error =
                admit_acquisition_after_completion(&acquisition, &notification).unwrap_err();
            assert!(error.to_string().contains("completion"));
        }

        #[test]
        fn duplicate_coordinate_kind_rejects() {
            let error = OrderingEvent::from_admitted_provider(
                OrderingEventId::from_normalized_identity(1).unwrap(),
                DeviceOrderingRole::Publication,
                DeviceInstanceId::from_normalized_identity(10).unwrap(),
                RuntimeScopeId::from_normalized_identity(20).unwrap(),
                ExternalLoanId::from_normalized_identity(30).unwrap(),
                authority(),
                [
                    (
                        OrderingCoordinateKind::Data,
                        OrderingRange {
                            mapping: MappingId::from_normalized_identity(40).unwrap(),
                            base: 0x1000,
                            length: 0x200,
                        },
                    ),
                    (
                        OrderingCoordinateKind::Data,
                        OrderingRange {
                            mapping: MappingId::from_normalized_identity(40).unwrap(),
                            base: 0x3000,
                            length: 0x100,
                        },
                    ),
                ],
            )
            .unwrap_err();
            assert!(error.to_string().contains("duplicate"));
        }

        #[test]
        fn empty_coordinates_reject() {
            let error = OrderingEvent::from_admitted_provider(
                OrderingEventId::from_normalized_identity(1).unwrap(),
                DeviceOrderingRole::Publication,
                DeviceInstanceId::from_normalized_identity(10).unwrap(),
                RuntimeScopeId::from_normalized_identity(20).unwrap(),
                ExternalLoanId::from_normalized_identity(30).unwrap(),
                authority(),
                [],
            )
            .unwrap_err();
            assert!(error.to_string().contains("at least one coordinate"));
        }
    }
}
