//! Explicit custody records for foreign arguments retained beyond one call.
//!
//! Call-scoped arguments remain borrows minted through
//! [`ProgramLocalExtentRegistry::loan_under_activation`]/
//! [`ProgramLocalExtentRegistry::loan_mut_under_activation`] under the exact
//! establishing activation. Longer-lived dispositions are explicit so the
//! registry can retain the program-local account, while the `Extent` remains
//! passive authority. Unknown ambient backing and substituted mapping
//! revisions reject.

use extents::{
    AddressSpaceId, Extent, ExtentLineageId, ExtentProgramLocalOrigin, ExtentProvenanceId,
    ExtentRights, MappingEraId,
};

use super::{ExternalRootDiagnostic, ProgramLocalExtentRegistry};

/// Foreign access polarity over the retained range; mirrors [`extents::LoanPolarity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedForeignAccess {
    Shared,
    Exclusive,
}

/// How the foreign side keeps the argument after the call returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedForeignArgumentDisposition {
    LifetimeBorrowed,
    Moved,
    Snapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RetainedForeignArgumentId(u64);

impl RetainedForeignArgumentId {
    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedForeignArgumentRequest {
    offset: u64,
    length: u64,
    access: RetainedForeignAccess,
    rights: ExtentRights,
}

impl RetainedForeignArgumentRequest {
    pub fn new(
        offset: u64,
        length: u64,
        access: RetainedForeignAccess,
        rights: ExtentRights,
    ) -> Result<Self, ExternalRootDiagnostic> {
        if length == 0 {
            return Err(ExternalRootDiagnostic(
                "retained foreign argument range must be nonempty".into(),
            ));
        }
        Ok(Self {
            offset,
            length,
            access,
            rights,
        })
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn access(&self) -> RetainedForeignAccess {
        self.access
    }

    pub const fn rights(&self) -> &ExtentRights {
        &self.rights
    }
}

#[derive(Debug)]
enum RetainedForeignCustody {
    LifetimeBorrowed,
    Moved(Extent),
    Snapshot(Extent),
}

/// Live retained foreign argument. The registry holds account lifetime custody.
#[derive(Debug)]
#[must_use = "live retained foreign argument custody must be released"]
pub struct RetainedForeignArgument {
    identity: RetainedForeignArgumentId,
    origin: ExtentProgramLocalOrigin,
    lineage: ExtentLineageId,
    address_space: AddressSpaceId,
    base: u64,
    length: u64,
    access: RetainedForeignAccess,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    custody: RetainedForeignCustody,
}

impl RetainedForeignArgument {
    pub const fn identity(&self) -> RetainedForeignArgumentId {
        self.identity
    }

    pub const fn origin(&self) -> ExtentProgramLocalOrigin {
        self.origin
    }

    pub const fn lineage(&self) -> ExtentLineageId {
        self.lineage
    }

    pub const fn address_space(&self) -> AddressSpaceId {
        self.address_space
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn access(&self) -> RetainedForeignAccess {
        self.access
    }

    pub const fn rights(&self) -> &ExtentRights {
        &self.rights
    }

    pub const fn provenance(&self) -> ExtentProvenanceId {
        self.provenance
    }

    pub const fn era(&self) -> MappingEraId {
        self.era
    }

    pub const fn disposition(&self) -> RetainedForeignArgumentDisposition {
        match self.custody {
            RetainedForeignCustody::LifetimeBorrowed => {
                RetainedForeignArgumentDisposition::LifetimeBorrowed
            }
            RetainedForeignCustody::Moved(_) => RetainedForeignArgumentDisposition::Moved,
            RetainedForeignCustody::Snapshot(_) => RetainedForeignArgumentDisposition::Snapshot,
        }
    }
}

#[derive(Debug)]
pub struct ReleasedRetainedForeignArgument {
    identity: RetainedForeignArgumentId,
    disposition: RetainedForeignArgumentDisposition,
    returned: Option<Extent>,
}

impl ReleasedRetainedForeignArgument {
    pub const fn identity(&self) -> RetainedForeignArgumentId {
        self.identity
    }

    pub const fn disposition(&self) -> RetainedForeignArgumentDisposition {
        self.disposition
    }

    pub fn returned(&self) -> Option<&Extent> {
        self.returned.as_ref()
    }

    pub fn into_returned(self) -> Option<Extent> {
        self.returned
    }
}

#[derive(Debug)]
pub struct RetainedForeignArgumentError<T> {
    subject: T,
    diagnostic: ExternalRootDiagnostic,
}

impl<T> RetainedForeignArgumentError<T> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_subject(self) -> T {
        self.subject
    }
}

impl RetainedForeignArgumentError<Extent> {
    pub fn into_extent(self) -> Extent {
        self.subject
    }
}

impl RetainedForeignArgumentError<RetainedForeignArgument> {
    pub fn into_retained(self) -> RetainedForeignArgument {
        self.subject
    }
}

impl<'root, 'code> ProgramLocalExtentRegistry<'root, 'code> {
    pub fn retain_foreign_argument_borrowed(
        &mut self,
        extent: &Extent,
        request: RetainedForeignArgumentRequest,
    ) -> Result<RetainedForeignArgument, ExternalRootDiagnostic> {
        let (origin, base) = self.validate_requested_range(extent, &request)?;
        self.check_retention_conflict(origin, base, request.length, request.access)?;
        let identity = self.next_retention_id()?;
        let held = self
            .held
            .get_mut(&origin)
            .expect("validated program-local origin remains held");
        held.retained.insert(
            identity,
            super::LiveRetention {
                base,
                length: request.length,
                access: request.access,
            },
        );
        Ok(Self::build_retained(
            identity,
            origin,
            extent.lineage_root(),
            extent.address_space(),
            base,
            request.length,
            request.access,
            request.rights,
            extent.provenance(),
            extent.era(),
            RetainedForeignCustody::LifetimeBorrowed,
        ))
    }

    pub fn retain_foreign_argument_moved(
        &mut self,
        extent: Extent,
        access: RetainedForeignAccess,
        rights: ExtentRights,
    ) -> Result<RetainedForeignArgument, Box<RetainedForeignArgumentError<Extent>>> {
        let (_, origin) = match self.validate_backing(&extent) {
            Ok(value) => value,
            Err(diagnostic) => return Err(Box::new(error(extent, diagnostic.0))),
        };
        if !extent.rights().contains(&rights) {
            return Err(Box::new(error(
                extent,
                "retained foreign argument rights exceed the Extent's authority",
            )));
        }
        if let Err(diagnostic) =
            self.check_retention_conflict(origin, extent.base(), extent.length(), access)
        {
            return Err(Box::new(error(extent, diagnostic.0)));
        }
        let identity = match self.next_retention_id() {
            Ok(identity) => identity,
            Err(diagnostic) => return Err(Box::new(error(extent, diagnostic.0))),
        };
        let held = self
            .held
            .get_mut(&origin)
            .expect("validated program-local origin remains held");
        held.retained.insert(
            identity,
            super::LiveRetention {
                base: extent.base(),
                length: extent.length(),
                access,
            },
        );
        let base = extent.base();
        let length = extent.length();
        let lineage = extent.lineage_root();
        let address_space = extent.address_space();
        let provenance = extent.provenance();
        let era = extent.era();
        Ok(Self::build_retained(
            identity,
            origin,
            lineage,
            address_space,
            base,
            length,
            access,
            rights,
            provenance,
            era,
            RetainedForeignCustody::Moved(extent),
        ))
    }

    pub fn retain_foreign_argument_snapshot(
        &mut self,
        source: &Extent,
        request: RetainedForeignArgumentRequest,
        backing: Extent,
    ) -> Result<RetainedForeignArgument, Box<RetainedForeignArgumentError<Extent>>> {
        if let Err(diagnostic) = self.validate_requested_range(source, &request) {
            return Err(Box::new(error(backing, diagnostic.0)));
        }
        let (_, origin) = match self.validate_backing(&backing) {
            Ok(value) => value,
            Err(diagnostic) => return Err(Box::new(error(backing, diagnostic.0))),
        };
        if backing.length() != request.length {
            return Err(Box::new(error(
                backing,
                "retained foreign argument snapshot backing length does not match the requested range",
            )));
        }
        if backing.program_local_origin() == source.program_local_origin() {
            return Err(Box::new(error(
                backing,
                "retained foreign argument snapshot backing must be a private copy with a distinct origin",
            )));
        }
        if !backing.rights().contains(&request.rights) {
            return Err(Box::new(error(
                backing,
                "retained foreign argument rights exceed the snapshot backing's authority",
            )));
        }
        if let Err(diagnostic) =
            self.check_retention_conflict(origin, backing.base(), backing.length(), request.access)
        {
            return Err(Box::new(error(backing, diagnostic.0)));
        }
        let identity = match self.next_retention_id() {
            Ok(identity) => identity,
            Err(diagnostic) => return Err(Box::new(error(backing, diagnostic.0))),
        };
        let held = self
            .held
            .get_mut(&origin)
            .expect("validated program-local origin remains held");
        held.retained.insert(
            identity,
            super::LiveRetention {
                base: backing.base(),
                length: backing.length(),
                access: request.access,
            },
        );
        let base = backing.base();
        let length = backing.length();
        let lineage = backing.lineage_root();
        let address_space = backing.address_space();
        let provenance = backing.provenance();
        let era = backing.era();
        Ok(Self::build_retained(
            identity,
            origin,
            lineage,
            address_space,
            base,
            length,
            request.access,
            request.rights,
            provenance,
            era,
            RetainedForeignCustody::Snapshot(backing),
        ))
    }

    pub fn release_retained_foreign_argument(
        &mut self,
        retained: RetainedForeignArgument,
    ) -> Result<
        ReleasedRetainedForeignArgument,
        Box<RetainedForeignArgumentError<RetainedForeignArgument>>,
    > {
        let origin = retained.origin;
        let Some(held) = self.held.get_mut(&origin) else {
            return Err(Box::new(error(
                retained,
                "retained foreign argument names no held program-local account",
            )));
        };
        let Some(live) = held.retained.get(&retained.identity) else {
            return Err(Box::new(error(
                retained,
                "retained foreign argument identity is not live on its held account",
            )));
        };
        if live.base != retained.base
            || live.length != retained.length
            || live.access != retained.access
        {
            return Err(Box::new(error(
                retained,
                "retained foreign argument live range or access does not match its account record",
            )));
        }
        held.retained.remove(&retained.identity);
        let disposition = retained.disposition();
        let returned = match retained.custody {
            RetainedForeignCustody::LifetimeBorrowed => None,
            RetainedForeignCustody::Moved(extent) | RetainedForeignCustody::Snapshot(extent) => {
                Some(extent)
            }
        };
        Ok(ReleasedRetainedForeignArgument {
            identity: retained.identity,
            disposition,
            returned,
        })
    }

    fn validate_requested_range(
        &self,
        extent: &Extent,
        request: &RetainedForeignArgumentRequest,
    ) -> Result<(ExtentProgramLocalOrigin, u64), ExternalRootDiagnostic> {
        let (_, origin) = self.validate_backing(extent)?;
        let end = request.offset.checked_add(request.length).ok_or_else(|| {
            ExternalRootDiagnostic("retained foreign argument range overflows".into())
        })?;
        if end > extent.length() {
            return Err(ExternalRootDiagnostic(
                "retained foreign argument range exceeds the Extent".into(),
            ));
        }
        if !extent.rights().contains(&request.rights) {
            return Err(ExternalRootDiagnostic(
                "retained foreign argument rights exceed the Extent's authority".into(),
            ));
        }
        let base = extent.base().checked_add(request.offset).ok_or_else(|| {
            ExternalRootDiagnostic("retained foreign argument absolute base overflows".into())
        })?;
        Ok((origin, base))
    }

    fn check_retention_conflict(
        &self,
        origin: ExtentProgramLocalOrigin,
        base: u64,
        length: u64,
        access: RetainedForeignAccess,
    ) -> Result<(), ExternalRootDiagnostic> {
        let Some(held) = self.held.get(&origin) else {
            return Err(ExternalRootDiagnostic(
                "retained foreign argument has no held program-local account".into(),
            ));
        };
        let end = base
            .checked_add(length)
            .expect("validated Extent geometry and nonempty range cannot overflow");
        if held.retained.values().any(|live| {
            let live_end = live
                .base
                .checked_add(live.length)
                .expect("validated live retention cannot overflow");
            base < live_end
                && live.base < end
                && (access == RetainedForeignAccess::Exclusive
                    || live.access == RetainedForeignAccess::Exclusive)
        }) {
            return Err(ExternalRootDiagnostic(
                "retained foreign argument overlaps a live retained foreign argument with exclusive access"
                    .into(),
            ));
        }
        Ok(())
    }

    fn next_retention_id(&mut self) -> Result<RetainedForeignArgumentId, ExternalRootDiagnostic> {
        let identity = RetainedForeignArgumentId(self.next_retention);
        self.next_retention = self.next_retention.checked_add(1).ok_or_else(|| {
            ExternalRootDiagnostic("retained foreign argument identity space is exhausted".into())
        })?;
        Ok(identity)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_retained(
        identity: RetainedForeignArgumentId,
        origin: ExtentProgramLocalOrigin,
        lineage: ExtentLineageId,
        address_space: AddressSpaceId,
        base: u64,
        length: u64,
        access: RetainedForeignAccess,
        rights: ExtentRights,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        custody: RetainedForeignCustody,
    ) -> RetainedForeignArgument {
        RetainedForeignArgument {
            identity,
            origin,
            lineage,
            address_space,
            base,
            length,
            access,
            rights,
            provenance,
            era,
            custody,
        }
    }
}

fn error<T>(subject: T, diagnostic: impl Into<String>) -> RetainedForeignArgumentError<T> {
    RetainedForeignArgumentError {
        subject,
        diagnostic: ExternalRootDiagnostic(diagnostic.into()),
    }
}
