use super::{
    PackagePolicyChangeError as Error, PackagePolicyPackageChange, PackagePolicyRowChange,
};
use crate::declarations::PackageKey;
use crate::resolution::graph::CanonicalSourceClosureSubjectLimits;
use omega_package_evidence::record::{
    PackagePolicyRow, PackagePolicyRowLimits, PackagePolicyRowUsage,
};
use omega_package_source::SourceLineage;

/// Aggregate ceilings across both complete inputs, not a reset per package.
/// Context bytes count canonical source and fresh review inputs and requested
/// scratch/path/key storage. Source-subject construction has its own bounded
/// model and temporary storage; that storage is not charged as context bytes.
/// These limits do not measure allocator overhead or process memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyChangeLimits {
    pub maximum_packages: usize,
    pub maximum_rows: usize,
    pub maximum_projection_owned_bytes: usize,
    pub maximum_projection_elements: usize,
    /// Combined policy-row changes and source-replacement findings.
    pub maximum_changed_rows: usize,
    pub maximum_changed_owned_bytes: usize,
    /// Per diagnostic path. Combined retained path storage is context-bounded.
    pub maximum_dependency_path_steps: usize,
    pub maximum_context_bytes: usize,
}
impl Default for PackagePolicyChangeLimits {
    fn default() -> Self {
        Self {
            maximum_packages: 4096,
            maximum_rows: 131_072,
            maximum_projection_owned_bytes: 256 * 1024 * 1024,
            maximum_projection_elements: 8 * 1024 * 1024,
            maximum_changed_rows: 65_536,
            maximum_changed_owned_bytes: 64 * 1024 * 1024,
            maximum_dependency_path_steps: 1024,
            maximum_context_bytes: 128 * 1024 * 1024,
        }
    }
}
impl PackagePolicyChangeLimits {
    fn bounded(self) -> Self {
        let hard = Self::default();
        Self {
            maximum_packages: self.maximum_packages.min(hard.maximum_packages),
            maximum_rows: self.maximum_rows.min(hard.maximum_rows),
            maximum_projection_owned_bytes: self
                .maximum_projection_owned_bytes
                .min(hard.maximum_projection_owned_bytes),
            maximum_projection_elements: self
                .maximum_projection_elements
                .min(hard.maximum_projection_elements),
            maximum_changed_rows: self.maximum_changed_rows.min(hard.maximum_changed_rows),
            maximum_changed_owned_bytes: self
                .maximum_changed_owned_bytes
                .min(hard.maximum_changed_owned_bytes),
            maximum_dependency_path_steps: self
                .maximum_dependency_path_steps
                .min(hard.maximum_dependency_path_steps),
            maximum_context_bytes: self.maximum_context_bytes.min(hard.maximum_context_bytes),
        }
    }
}

pub(super) struct Budget {
    pub(super) limits: PackagePolicyChangeLimits,
    context: usize,
    rows: usize,
    owned: usize,
    elements: usize,
    changed_rows: usize,
    changed_owned: usize,
}
impl Budget {
    pub(super) fn new(limits: PackagePolicyChangeLimits) -> Self {
        Self {
            limits: limits.bounded(),
            context: 0,
            rows: 0,
            owned: 0,
            elements: 0,
            changed_rows: 0,
            changed_owned: 0,
        }
    }
    pub(super) fn context(&mut self, amount: usize) -> Result<(), Error> {
        charge(
            &mut self.context,
            amount,
            self.limits.maximum_context_bytes,
            "context bytes",
        )
    }
    pub(super) fn slots<T>(&mut self, count: usize) -> Result<(), Error> {
        self.context(
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(Error::AllocationFailed)?,
        )
    }
    pub(super) fn package_slots(&mut self, count: usize) -> Result<(), Error> {
        if count > self.limits.maximum_packages {
            return Err(Error::LimitExceeded {
                resource: "package slots",
                maximum: self.limits.maximum_packages,
            });
        }
        self.slots::<PackagePolicyPackageChange>(count)
    }
    pub(super) fn subject_limits(&self) -> CanonicalSourceClosureSubjectLimits {
        CanonicalSourceClosureSubjectLimits {
            maximum_record_bytes: self
                .limits
                .maximum_context_bytes
                .saturating_sub(self.context),
            maximum_packages: self.limits.maximum_packages,
            ..Default::default()
        }
    }
    pub(super) fn row_limits(&self) -> PackagePolicyRowLimits {
        PackagePolicyRowLimits {
            maximum_rows: self.limits.maximum_rows - self.rows,
            maximum_owned_bytes: self.limits.maximum_projection_owned_bytes - self.owned,
            maximum_sequence_elements: self.limits.maximum_projection_elements - self.elements,
            ..Default::default()
        }
    }
    pub(super) fn projected(&mut self, usage: PackagePolicyRowUsage) -> Result<(), Error> {
        charge(
            &mut self.rows,
            usage.rows(),
            self.limits.maximum_rows,
            "projected rows",
        )?;
        charge(
            &mut self.owned,
            usage.owned_bytes(),
            self.limits.maximum_projection_owned_bytes,
            "projection owned bytes",
        )?;
        charge(
            &mut self.elements,
            usage.sequence_elements(),
            self.limits.maximum_projection_elements,
            "projection elements",
        )
    }
    pub(super) fn changed(&mut self, rows: usize, bytes: usize) -> Result<(), Error> {
        charge(
            &mut self.changed_rows,
            rows,
            self.limits.maximum_changed_rows,
            "changed rows",
        )?;
        let slots = rows
            .checked_mul(std::mem::size_of::<PackagePolicyRowChange>())
            .ok_or(Error::AllocationFailed)?;
        charge(
            &mut self.changed_owned,
            bytes.checked_add(slots).ok_or(Error::AllocationFailed)?,
            self.limits.maximum_changed_owned_bytes,
            "changed owned bytes",
        )
    }
    pub(super) fn key(&mut self, key: &PackageKey) -> Result<(), Error> {
        self.context(std::mem::size_of::<PackageKey>())?;
        self.context(key.name().as_str().len())?;
        match key.source_lineage() {
            SourceLineage::GitHub(value) => {
                self.context(value.owner().len())?;
                self.context(value.repository().len())
            }
            SourceLineage::GitLab(value) => self.context(value.repository_path().len()),
            SourceLineage::Git(value) => {
                self.context(value.user().map_or(0, str::len))?;
                self.context(value.host().len())?;
                self.context(value.repository_path().len())
            }
            SourceLineage::Workspace(value) => self.context(value.member_path().as_str().len()),
            SourceLineage::ExternalLocal(value) => self.context(
                value
                    .canonical_absolute_path()
                    .as_os_str()
                    .as_encoded_bytes()
                    .len(),
            ),
        }
    }

    pub(super) fn source_replacements(&mut self, count: usize) -> Result<(), Error> {
        charge(
            &mut self.changed_rows,
            count,
            self.limits.maximum_changed_rows,
            "changed rows and source replacements",
        )
    }
}
pub(super) fn row_bytes(row: &PackagePolicyRow) -> Result<usize, Error> {
    row.key_bytes()
        .len()
        .checked_add(row.canonical_bytes().len())
        .and_then(|bytes| bytes.checked_add(row.canonical_text().len()))
        .ok_or(Error::AllocationFailed)
}
fn charge(
    current: &mut usize,
    amount: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), Error> {
    *current = current
        .checked_add(amount)
        .filter(|value| *value <= maximum)
        .ok_or(Error::LimitExceeded { resource, maximum })?;
    Ok(())
}
