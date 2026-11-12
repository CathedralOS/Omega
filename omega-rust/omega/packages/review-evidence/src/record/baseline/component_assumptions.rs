// Environmental premises of the independent components this root deploys
// (`PackagePolicyComponentAssumption`).
//
// Each row is one assumption from a verified component's roster, together with
// the component's package and the exact description rows bound by it. The
// consuming project's lock retains the reviewed row; root Product admission
// rejects a fresh row the lock does not hold
// (wiki/spec/build/component_publication.md#consumer-acceptance-and-build-attenuation).

use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyComponentAssumption {
    pub component: PackageKeyIdentity,
    pub assumption: [u8; 32],
    pub bound_rows: Vec<String>,
}

impl PackagePolicyComponentAssumption {
    // One canonical row per settled demand, in key order, for both the review
    // baseline and root Product admission.
    pub fn project<Rows>(settlement: &provider_planning::ProviderSettlement<Rows>) -> Vec<Self> {
        let mut rows: Vec<_> = settlement
            .component_assumption_demands()
            .into_iter()
            .map(|demand| Self {
                component: demand.component,
                assumption: demand.assumption,
                bound_rows: demand.bound_rows,
            })
            .collect();
        rows.sort();
        rows.dedup();
        rows
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.bound_rows.is_empty() {
            return Err("component assumption binds no description row");
        }
        if self.bound_rows.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("component assumption repeats or reorders its bound rows");
        }
        Ok(())
    }
}
