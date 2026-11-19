//! Static composition laws for activation storage bounds.
//! The same laws govern checker proofs and live ledger admission. This owner
//! has no mutable ledger, source program, provider grant or host execution.
#![forbid(unsafe_code)]
use extent_contracts::ExtentDiagnostic;
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

#[derive(Debug, Clone, Copy)]
pub struct ClaimBoundRow<'a> {
    pub bound: Option<u64>,
    pub alignment: u64,
    pub branch: &'a ActivationClaimBranch,
}

pub fn aligned_claim_bound(value: u64, alignment: u64) -> Result<u64, ExtentDiagnostic> {
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
                "an unbounded activation claim leaves the activation's stack demand open".into(),
            ));
        };
        aligned.push(aligned_claim_bound(bound, row.alignment)?);
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

pub fn validate_committed_within_bound(bound: u64, committed: u64) -> Result<(), ExtentDiagnostic> {
    if committed > bound {
        return Err(ExtentDiagnostic(format!(
            "committed extent {committed} exceeds the claim's authored bound {bound}"
        )));
    }
    Ok(())
}
