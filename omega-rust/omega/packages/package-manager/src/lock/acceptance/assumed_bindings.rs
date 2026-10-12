//! Receiving consent joins the complete current assumed-relationship risk row
//! to this occurrence's own lock. Provider consent and dependency locks cannot
//! answer it. Source/compiler authentication and selection of the consuming
//! root Product occurrence remain with the compilation owner.

use package_evidence::record::{
    PackagePolicyAssumedBindingContracts, PackagePolicyRowKind, PackagePolicyRowLimits,
};
use trust_model::{AcceptedAssumedBindingRelationship, accept_assumed_binding_relationship};

use super::{PackageAcceptanceRow, PackageLockError, PackagePolicyAcceptance};

impl PackagePolicyAcceptance {
    /// Consume current compiler-produced relationship obligations without
    /// reconstructing package analysis or turning assumptions into proofs.
    pub(crate) fn validate_assumed_binding_contracts(
        &self,
        contracts: &PackagePolicyAssumedBindingContracts,
    ) -> Result<Vec<AcceptedAssumedBindingRelationship>, PackageLockError> {
        let mismatch = || PackageLockError::AssumedBindingContractsMismatch {
            package: self.package,
        };
        if self.schema_version != package_evidence::record::PACKAGE_POLICY_ROW_VERSION
            || contracts.package() != self.package
            || contracts.target() != self.target
        {
            return Err(mismatch());
        }
        let (rows, _) = contracts
            .acceptance_rows_with_limits(PackagePolicyRowLimits::default())
            .map_err(PackageLockError::Encoding)?;
        let mut rows: Vec<_> = rows
            .into_iter()
            .map(PackageAcceptanceRow::from_row)
            .collect();
        rows.sort_unstable_by_key(|row| (row.kind, row.key));
        let accepted = self
            .rows
            .iter()
            .filter(|row| row.kind == PackagePolicyRowKind::AssumedBindingContract);
        if !accepted.eq(rows.iter()) {
            return Err(mismatch());
        }
        let mut receipts = Vec::new();
        receipts
            .try_reserve_exact(contracts.relationships().len())
            .map_err(|_| PackageLockError::AllocationFailed)?;
        for contract in contracts.relationships() {
            // Only the exact row join above supplies receiving consent. Its
            // relationship subject remains distinct from every provider grant.
            let admission = contract.relationship.trust_obligation();
            let receipt = accept_assumed_binding_relationship(&contract.relationship, &[admission])
                .map_err(|_| mismatch())?;
            receipts.push(receipt);
        }
        Ok(receipts)
    }
}
