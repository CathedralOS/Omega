//! Runtime-sized activation claims — the custody and demand model behind
//! `wiki/spec/resources/activation_storage.md`.
//!
//! An activation names a bound at authoring time and commits an extent within
//! it at runtime. The claim is linear and activation-scoped: establishment is
//! a checked admission (`committed <= bound`, never a trap or an implicit
//! clamp), releases run in reverse establishment order, and every live claim
//! must retire before the activation unwinds. Its grant provenance names
//! activation storage — never provider backing — so no provider seam accepts
//! it back as freed extent.
//!
//! Bound composition charges the authored bound, never the committed extent:
//! simultaneously live claim bounds add with alignment, bounds on mutually
//! exclusive branch paths compose by maximum, and an unbounded claim leaves
//! the activation's demand open — a `None` bound therefore rejects the
//! composition instead of silently summing.

#[cfg(test)]
mod tests;

use crate::extent::diagnostic::ExtentDiagnostic;
use crate::extent::{Lineage, SplitBranch};
use crate::identities::{ExtentLineageId, normalized_extent_identity};

normalized_extent_identity!(
    /// The authored claim site — the per-site key publication and replay join on.
    ActivationClaimSiteId,
    "activation-claim-site"
);

/// One live claim's identity inside its activation ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActivationClaimId(u64);

impl ActivationClaimId {
    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// The branch path a claim site inhabits, expressed as branch indices from
/// the activation's unconditional trunk outward. `[]` is the trunk — the
/// claim may be live on every path. Divergent paths are mutually exclusive
/// branches: claims whose paths diverge are never simultaneously live, so
/// their bounds compose by maximum rather than sum.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivationClaimBranch(Vec<u32>);

impl ActivationClaimBranch {
    /// The unconditional trunk path: live under every deeper branch.
    pub fn trunk() -> Self {
        Self(Vec::new())
    }

    /// The path one branch deeper.
    pub fn extend(&self, branch: u32) -> Self {
        let mut path = self.0.clone();
        path.push(branch);
        Self(path)
    }

    /// Whether `self` is the trunk or a strict ancestor path of `other` — a
    /// claim at `self` may be live wherever a claim at `other` is live.
    pub fn covers(&self, other: &Self) -> bool {
        other.0.starts_with(&self.0)
    }

    /// Whether the two paths diverge — claims on them cannot be live
    /// together.
    pub fn mutually_exclusive(&self, other: &Self) -> bool {
        !self.covers(other) && !other.covers(self)
    }
}

/// Where the claim's backing comes from. Activation claims are committed
/// activation storage: they need no provider grant, receive no external
/// provenance, and are never returned to a provider seam as freed extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationClaimProvenance {
    ActivationStorage,
}

/// One site's demand contribution for bound composition. `bound: None` models
/// an unbounded claim — it leaves the activation's demand open and rejects
/// the composition.
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

/// Compose the worst-case bound over one roster of live claims: aligned sum
/// along each branch path, maximum across mutually exclusive paths. This is
/// the bound charge a claim contributes to the enclosing frame's closed
/// stack demand — committed extents never widen it.
pub fn compose_claim_bounds<'a>(
    rows: impl IntoIterator<Item = ClaimBoundRow<'a>>,
) -> Result<u64, ExtentDiagnostic> {
    let rows: Vec<ClaimBoundRow<'a>> = rows.into_iter().collect();
    let mut aligned = Vec::with_capacity(rows.len());
    for row in &rows {
        let Some(bound) = row.bound else {
            return Err(ExtentDiagnostic(
                "an unbounded activation claim leaves the activation's stack demand open".into(),
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

/// What an admitted claim surface supplies: the authored bound and the
/// runtime-chosen committed extent, plus the branch the site inhabits.
#[derive(Debug)]
pub struct ActivationClaimRequest {
    pub site: ActivationClaimSiteId,
    pub branch: ActivationClaimBranch,
    /// The authored static byte bound — the demand charge.
    pub bound: u64,
    /// Byte alignment of the claimed region; must be a nonzero power of two.
    pub alignment: u64,
    /// The runtime-chosen committed extent.
    pub committed: u64,
}

/// The checked admission rule: recomputed or runtime-chosen committed extent
/// within the authored bound. Rejection is the checked outcome — never a
/// trap and never an implicit clamp.
pub fn validate_committed_within_bound(bound: u64, committed: u64) -> Result<(), ExtentDiagnostic> {
    if committed > bound {
        return Err(ExtentDiagnostic(format!(
            "committed extent {committed} exceeds the claim's authored bound {bound}"
        )));
    }
    Ok(())
}

/// A live, linear activation claim. Deliberately non-`Clone`: the source-side
/// `[linear]` checker supplies the must-consume rule, and the model keeps
/// every consuming operation explicit so claim authority is never silently
/// dropped. Releasing, splitting and merging conserve claim coverage rather
/// than allocating fresh authority.
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

    /// The authored bound — the byte extent charged into demand.
    pub const fn bound(&self) -> u64 {
        self.bound
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    /// The runtime-chosen extent committed inside the bound.
    pub const fn committed(&self) -> u64 {
        self.committed
    }

    /// Activation-storage provenance; never provider backing.
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

/// Establishment failed — the request is returned so the admission decision
/// stays the caller's.
#[derive(Debug)]
pub struct ClaimEstablishmentError {
    pub request: ActivationClaimRequest,
    pub diagnostic: ExtentDiagnostic,
}

/// The activation tried to unwind with claims still live — every live claim
/// must release before the frame retires. The surviving claims are returned.
#[derive(Debug)]
pub struct ClaimDrainError {
    pub live: Vec<ActivationClaim>,
    pub diagnostic: ExtentDiagnostic,
}

/// The per-activation ledger of live claims in establishment order.
///
/// Release is the stack discipline the realization replays against, not an
/// allocator convention: a claim cannot release while a later-established
/// claim stays live. Split children and merge parents are established at the
/// transformation point, so they join the newest end of the ordering.
#[derive(Debug, Default)]
pub struct ActivationClaimLedger {
    next_identity: u64,
    live: Vec<ActivationClaim>,
}

impl ActivationClaimLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// The live-claim roster in establishment order — the same roster a
    /// suspension crossing joins with its exact storage roles.
    pub fn live_claims(&self) -> impl Iterator<Item = &ActivationClaim> {
        self.live.iter()
    }

    pub fn claim(&self, id: ActivationClaimId) -> Option<&ActivationClaim> {
        self.live.iter().find(|claim| claim.id == id)
    }

    /// Admit one claim: `committed <= bound` and a valid alignment, nothing
    /// else. The bound charge composes into the activation's closed demand
    /// before establishment — see [`compose_claim_bounds`].
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
        if let Err(diagnostic) = validate_committed_within_bound(request.bound, request.committed) {
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

    /// Release one claim — only while no later-established claim stays live.
    /// On success the released claim returns so its custody transfer stays
    /// explicit; on failure the ledger is unchanged.
    pub fn release(&mut self, id: ActivationClaimId) -> Result<ActivationClaim, ExtentDiagnostic> {
        match self.live.last() {
            Some(newest) if newest.id == id => Ok(self.live.pop().expect("newest claim is live")),
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

    /// Split one live claim into two children that cover the parent exactly:
    /// child committed extents sum to the parent's committed extent and child
    /// bounds sum to the parent's bound, each `committed <= bound`. Children
    /// inherit the parent's branch and establish at the split point — both
    /// must release before any sibling established before them.
    pub fn split_claim(
        &mut self,
        id: ActivationClaimId,
        committed_at: u64,
        bound_at: u64,
    ) -> Result<(ActivationClaimId, ActivationClaimId), ExtentDiagnostic> {
        let Some(position) = self.live.iter().position(|claim| claim.id == id) else {
            return Err(ExtentDiagnostic(format!(
                "claim {} is not live in this activation",
                id.0
            )));
        };
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

    /// Merge two children back into their exact parent: sibling lineage of
    /// one split, and the pair live and newest in release order.
    pub fn merge_claims(
        &mut self,
        lower_id: ActivationClaimId,
        upper_id: ActivationClaimId,
    ) -> Result<ActivationClaimId, ExtentDiagnostic> {
        let Some(lower_position) = self.live.iter().position(|claim| claim.id == lower_id) else {
            return Err(ExtentDiagnostic(format!(
                "claim {} is not live in this activation",
                lower_id.0
            )));
        };
        let Some(upper_position) = self.live.iter().position(|claim| claim.id == upper_id) else {
            return Err(ExtentDiagnostic(format!(
                "claim {} is not live in this activation",
                upper_id.0
            )));
        };
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

    /// Every live claim must release before the frame unwinds. On failure the
    /// surviving claims return so the caller can retire or reseat them.
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

    /// The claim bounds' contribution to the activation's closed stack
    /// demand: aligned sum along each branch path, maximum across mutually
    /// exclusive paths.
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
