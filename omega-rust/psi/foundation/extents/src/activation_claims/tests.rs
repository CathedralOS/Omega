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
fn split_children_can_split_further_within_the_parent_cover() {
    let mut ledger = ActivationClaimLedger::new();
    let parent = ledger
        .establish(request(1, ActivationClaimBranch::trunk(), 64, 8, 40))
        .unwrap();
    let (lower, _upper) = ledger.split_claim(parent, 16, 24).unwrap();
    let (lower_lower, lower_upper) = ledger.split_claim(lower, 8, 12).unwrap();
    let lower_lower = ledger.claim(lower_lower).unwrap();
    assert_eq!(
        lower_lower.committed() + ledger.claim(lower_upper).unwrap().committed(),
        16
    );
}

#[test]
fn composition_adds_simultaneously_live_bounds_with_alignment() {
    let trunk = ActivationClaimBranch::trunk();
    let rows = [
        ClaimBoundRow {
            bound: Some(65),
            alignment: 64,
            branch: &trunk,
        },
        ClaimBoundRow {
            bound: Some(33),
            alignment: 32,
            branch: &trunk,
        },
    ];
    assert_eq!(compose_claim_bounds(rows).unwrap(), 192);
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
fn composition_counts_ancestor_claims_inside_a_deeper_branch() {
    let trunk = ActivationClaimBranch::trunk();
    let outer = trunk.extend(0);
    let inner = outer.extend(3);
    let rows = [
        ClaimBoundRow {
            bound: Some(16),
            alignment: 8,
            branch: &outer,
        },
        ClaimBoundRow {
            bound: Some(24),
            alignment: 8,
            branch: &inner,
        },
    ];
    assert_eq!(compose_claim_bounds(rows).unwrap(), 40);
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
    // trunk claim (aligned 16) + max(branch0 aligned 80, branch1 aligned 32)
    assert_eq!(ledger.composed_demand().unwrap(), 96);
}
