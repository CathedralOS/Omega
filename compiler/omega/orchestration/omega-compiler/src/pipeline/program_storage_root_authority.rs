//! Owned runtime authority disposition after program-storage installation.
//!
//! This carrier makes the installation's nullable internal representation
//! total without manufacturing an outbound source value. A receiver-free
//! installation may release its two whole roots. An attached installation
//! retains its receiver partition intact and exposes its separated residuals
//! only by borrow; it cannot turn them into one `Extent`.

use super::program_storage_entry::{
    InstalledProgramStorageRoots, ProgramStorageEntryDiagnostic, ProgramStorageEntryPlanBinding,
    ProgramStorageEntryProviderInvocation, ProgramStorageInstalledExtentRecord,
    ReservedProgramEntryReceiverStorage,
};
use psi_extents::Extent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramStorageEntryInitialStorageAuthorityKind {
    Whole,
    ReceiverPartitioned,
}

#[derive(Debug)]
pub(super) enum ProgramStorageEntryInitialStorageAuthority {
    Whole {
        initial_storage: Extent,
        zero_sized_receiver: Option<ReservedProgramEntryReceiverStorage>,
    },
    ReceiverPartitioned {
        receiver_storage: ReservedProgramEntryReceiverStorage,
    },
}

/// One installed image root plus the exact conserved disposition of initial
/// storage. This value owns authority; its observations do not duplicate it.
///
/// `ReceiverPartitioned` deliberately is not a source argument. Its `before`
/// and `after` residuals can be noncontiguous, while the selected receiver
/// extent must remain inside the same private partition owner.
#[derive(Debug)]
pub struct ProgramStorageEntryRootAuthorityDisposition {
    pub(super) binding: ProgramStorageEntryPlanBinding,
    pub(super) provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    pub(super) image: Extent,
    pub(super) initial_storage: ProgramStorageEntryInitialStorageAuthority,
}

impl InstalledProgramStorageRoots {
    /// Consume the installation into a total authority disposition suitable
    /// for later wrapper planning. The conversion replays the private storage
    /// invariants before exposing either the receiver-free whole roots or an
    /// intact receiver partition.
    pub fn into_root_authority_disposition(
        self,
    ) -> Result<
        ProgramStorageEntryRootAuthorityDisposition,
        ProgramStorageEntryRootAuthorityDispositionError,
    > {
        if let Err(diagnostic) = validate_root_authority_disposition(&self) {
            return Err(ProgramStorageEntryRootAuthorityDispositionError {
                roots: self,
                diagnostic,
            });
        }
        let (binding, provider_invocation, image, initial_storage, receiver_storage) =
            self.into_root_authority_parts();
        let initial_storage = match (initial_storage, receiver_storage) {
            (Some(initial_storage), zero_sized_receiver) => {
                ProgramStorageEntryInitialStorageAuthority::Whole {
                    initial_storage,
                    zero_sized_receiver,
                }
            }
            (None, Some(receiver_storage)) => {
                ProgramStorageEntryInitialStorageAuthority::ReceiverPartitioned { receiver_storage }
            }
            (None, None) => unreachable!("validated disposition retains initial-storage authority"),
        };
        Ok(ProgramStorageEntryRootAuthorityDisposition {
            binding,
            provider_invocation,
            image,
            initial_storage,
        })
    }
}

pub(super) fn validate_root_authority_disposition(
    roots: &InstalledProgramStorageRoots,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let record = roots.installation_record();
    let record = record.initial_storage();
    match (roots.initial_storage(), roots.receiver_storage()) {
        (Some(initial_storage), None) => validate_whole_initial_storage(initial_storage, record),
        (Some(initial_storage), Some(receiver))
            if receiver.storage().is_none() && receiver.placement().length() == 0 =>
        {
            validate_whole_initial_storage(initial_storage, record)?;
            if receiver.placement().lineage_root() != record.lineage_root() {
                return Err(ProgramStorageEntryDiagnostic(
                    "zero-sized receiver reservation drifted from initial-storage lineage".into(),
                ));
            }
            Ok(())
        }
        (None, Some(receiver)) if receiver.placement().length() != 0 => {
            let Some(selected) = receiver.storage() else {
                return Err(ProgramStorageEntryDiagnostic(
                    "nonempty receiver reservation lost its owned initial-storage partition".into(),
                ));
            };
            if selected.base() != receiver.placement().base()
                || selected.length() != receiver.placement().length()
                || selected.lineage_root() != receiver.placement().lineage_root()
            {
                return Err(ProgramStorageEntryDiagnostic(
                    "receiver reservation drifted from its selected initial-storage authority"
                        .into(),
                ));
            }
            validate_partitioned_initial_storage(
                [receiver.before(), Some(selected), receiver.after()],
                record,
            )
        }
        _ => Err(ProgramStorageEntryDiagnostic(
            "installed initial-storage authority has an inconsistent receiver disposition".into(),
        )),
    }
}

fn validate_whole_initial_storage(
    extent: &Extent,
    record: &ProgramStorageInstalledExtentRecord,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if extent.base() != record.base()
        || extent.length() != record.length()
        || !extent_matches_installed_storage_authority(extent, record)
    {
        return Err(ProgramStorageEntryDiagnostic(
            "whole initial-storage authority drifted from its installed root record".into(),
        ));
    }
    Ok(())
}

fn validate_partitioned_initial_storage(
    extents: [Option<&Extent>; 3],
    record: &ProgramStorageInstalledExtentRecord,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let Some(record_end) = record.base().checked_add(record.length()) else {
        return Err(ProgramStorageEntryDiagnostic(
            "installed initial-storage record overflows its address space".into(),
        ));
    };
    let mut next_base = record.base();
    for extent in extents.into_iter().flatten() {
        let Some(end) = extent.base().checked_add(extent.length()) else {
            return Err(ProgramStorageEntryDiagnostic(
                "partitioned initial-storage authority overflows its address space".into(),
            ));
        };
        if extent.length() == 0
            || extent.base() != next_base
            || !extent_matches_installed_storage_authority(extent, record)
        {
            return Err(ProgramStorageEntryDiagnostic(
                "receiver partition does not exactly conserve installed initial-storage authority"
                    .into(),
            ));
        }
        next_base = end;
    }
    if next_base != record_end {
        return Err(ProgramStorageEntryDiagnostic(
            "receiver partition does not exactly cover the installed initial-storage root".into(),
        ));
    }
    Ok(())
}

fn extent_matches_installed_storage_authority(
    extent: &Extent,
    record: &ProgramStorageInstalledExtentRecord,
) -> bool {
    extent.address_space() == record.address_space()
        && extent
            .rights()
            .identities()
            .eq(record.rights().iter().copied())
        && extent.provenance() == record.provenance()
        && extent.era() == record.mapping_era()
        && extent.origin() == record.origin()
        && extent.lineage_root() == record.lineage_root()
}

impl ProgramStorageEntryRootAuthorityDisposition {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.provider_invocation
    }

    pub const fn image(&self) -> &Extent {
        &self.image
    }

    pub const fn initial_storage_kind(&self) -> ProgramStorageEntryInitialStorageAuthorityKind {
        match self.initial_storage {
            ProgramStorageEntryInitialStorageAuthority::Whole { .. } => {
                ProgramStorageEntryInitialStorageAuthorityKind::Whole
            }
            ProgramStorageEntryInitialStorageAuthority::ReceiverPartitioned { .. } => {
                ProgramStorageEntryInitialStorageAuthorityKind::ReceiverPartitioned
            }
        }
    }

    pub const fn whole_initial_storage(&self) -> Option<&Extent> {
        match &self.initial_storage {
            ProgramStorageEntryInitialStorageAuthority::Whole {
                initial_storage, ..
            } => Some(initial_storage),
            ProgramStorageEntryInitialStorageAuthority::ReceiverPartitioned { .. } => None,
        }
    }

    pub const fn receiver_storage(&self) -> Option<&ReservedProgramEntryReceiverStorage> {
        match &self.initial_storage {
            ProgramStorageEntryInitialStorageAuthority::Whole {
                zero_sized_receiver,
                ..
            } => zero_sized_receiver.as_ref(),
            ProgramStorageEntryInitialStorageAuthority::ReceiverPartitioned {
                receiver_storage,
            } => Some(receiver_storage),
        }
    }

    /// The lower conserved residual. It remains owned by this carrier.
    pub fn residual_before(&self) -> Option<&Extent> {
        self.receiver_storage()
            .and_then(ReservedProgramEntryReceiverStorage::before)
    }

    /// The upper conserved residual. It remains owned by this carrier.
    pub fn residual_after(&self) -> Option<&Extent> {
        self.receiver_storage()
            .and_then(ReservedProgramEntryReceiverStorage::after)
    }

    /// Release two whole roots only when no receiver occurrence is attached.
    /// A partitioned or zero-sized attached receiver returns this disposition
    /// intact instead of fabricating source-call compatibility.
    pub fn try_into_receiver_free_whole_roots(
        self,
    ) -> Result<ProgramStorageEntryWholeRootAuthority, ProgramStorageEntryWholeRootAuthorityError>
    {
        let Self {
            binding,
            provider_invocation,
            image,
            initial_storage,
        } = self;
        match initial_storage {
            ProgramStorageEntryInitialStorageAuthority::Whole {
                initial_storage,
                zero_sized_receiver: None,
            } => Ok(ProgramStorageEntryWholeRootAuthority {
                binding,
                provider_invocation,
                image,
                initial_storage,
            }),
            initial_storage => Err(ProgramStorageEntryWholeRootAuthorityError {
                disposition: ProgramStorageEntryRootAuthorityDisposition {
                    binding,
                    provider_invocation,
                    image,
                    initial_storage,
                },
                diagnostic: ProgramStorageEntryDiagnostic(
                    "attached program storage cannot be released as two whole root authorities"
                        .into(),
                ),
            }),
        }
    }
}

/// The exact two whole authorities available from a receiver-free installed
/// entry. This is not an ABI value, emitted argument, or callee realization.
#[derive(Debug)]
pub struct ProgramStorageEntryWholeRootAuthority {
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    image: Extent,
    initial_storage: Extent,
}

impl ProgramStorageEntryWholeRootAuthority {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.provider_invocation
    }

    pub const fn image(&self) -> &Extent {
        &self.image
    }

    pub const fn initial_storage(&self) -> &Extent {
        &self.initial_storage
    }

    pub fn into_parts(
        self,
    ) -> (
        ProgramStorageEntryPlanBinding,
        Option<ProgramStorageEntryProviderInvocation>,
        Extent,
        Extent,
    ) {
        (
            self.binding,
            self.provider_invocation,
            self.image,
            self.initial_storage,
        )
    }
}

#[derive(Debug)]
pub struct ProgramStorageEntryWholeRootAuthorityError {
    disposition: ProgramStorageEntryRootAuthorityDisposition,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageEntryWholeRootAuthorityError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_disposition(self) -> ProgramStorageEntryRootAuthorityDisposition {
        self.disposition
    }
}

impl std::fmt::Display for ProgramStorageEntryWholeRootAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryWholeRootAuthorityError {}

#[derive(Debug)]
pub struct ProgramStorageEntryRootAuthorityDispositionError {
    pub(super) roots: InstalledProgramStorageRoots,
    pub(super) diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageEntryRootAuthorityDispositionError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_roots(self) -> InstalledProgramStorageRoots {
        self.roots
    }
}

impl std::fmt::Display for ProgramStorageEntryRootAuthorityDispositionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryRootAuthorityDispositionError {}
