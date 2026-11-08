/*
We place validated access plans onto concrete address-space extents, governing
access admission (`access_admission`), borrowed and owned resident custody views
(`borrowed_view`, `resident_views`, `atomic_resident_views`,
`owned_access_lifecycle`, `owned_resident_custody`,
`owned_atomic_resident_custody`), external correspondence
(`owned_external_correspondence`), and schema-to-device correspondence
(`schema_correspondence`).
*/

macro_rules! resident_view_accessors {
    () => {
        pub const fn admission(&self) -> AccessAdmissionId {
            self.admission
        }

        pub const fn access_plan(&self) -> &ValidatedAccessPlan {
            &self.plan
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.profile_receipt
        }

        pub(super) const fn loan(&self) -> &ExtentLoan<'resident> {
            &self.loan
        }

        pub(super) const fn content(&self) -> &'resident ProviderExistingContentGrant {
            self.content
        }

        pub const fn base(&self) -> u64 {
            self.loan.base()
        }

        pub const fn length(&self) -> u64 {
            self.loan.length()
        }

        pub const fn loan_polarity(&self) -> LoanPolarity {
            self.loan.polarity()
        }

        pub const fn resident_claim(&self) -> ResidentClaimId {
            self.content.resident_claim()
        }

        pub const fn occurrence(&self) -> AccessOccurrenceId {
            self.occurrence
        }

        pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
            self.content.validity_receipt()
        }

        pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
            self.content.custody_receipt()
        }

        pub fn project<'view>(
            &'view self,
            key: AccessFieldKey,
        ) -> Result<AccessFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
            self.project_with(key, BorrowPolarity::Shared)
        }

        pub fn project_mut<'view>(
            &'view mut self,
            key: AccessFieldKey,
        ) -> Result<AccessFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
            self.project_with(key, BorrowPolarity::Exclusive)
        }

        #[cfg(test)]
        pub(crate) fn replace_content_for_test(
            &mut self,
            content: &'resident ProviderExistingContentGrant,
        ) -> &'resident ProviderExistingContentGrant {
            std::mem::replace(&mut self.content, content)
        }
    };
}

macro_rules! owned_view_accessors {
    (resident, occurrence) => {
        owned_view_accessors!(resident);

        pub const fn occurrence(&self) -> AccessOccurrenceId {
            self.occurrence
        }
    };
    (resident) => {
        owned_view_accessors!();

        pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
            self.content.validity_receipt()
        }

        pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
            self.content.custody_receipt()
        }

        pub const fn resident_claim(&self) -> ResidentClaimId {
            self.content.resident_claim()
        }
    };
    (correspondence, occurrence) => {
        owned_view_accessors!(correspondence);

        pub const fn occurrence(&self) -> AccessOccurrenceId {
            self.occurrence
        }
    };
    (correspondence) => {
        owned_view_accessors!();

        pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
            &self.correspondence
        }
    };
    (occurrence) => {
        owned_view_accessors!();

        pub const fn occurrence(&self) -> AccessOccurrenceId {
            self.occurrence
        }
    };
    () => {
        pub const fn admission(&self) -> AccessAdmissionId {
            self.admission.identity
        }

        pub const fn access_plan(&self) -> &ValidatedAccessPlan {
            &self.admission.access_plan
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.admission.profile_receipt
        }

        pub const fn resources(&self) -> &AccessResourceCompatibility {
            &self.admission.resources
        }

        pub const fn extent(&self) -> &Extent {
            &self.admission.extent
        }
    };
}

macro_rules! field_projection_accessors {
    ($extent:lifetime) => {
        pub fn project<'view>(
            &'view self,
            key: AccessFieldKey,
        ) -> Result<AccessFieldProjection<'view, $extent>, AccessPlanDiagnostic> {
            self.project_with(key, BorrowPolarity::Shared)
        }

        pub fn project_mut<'view>(
            &'view mut self,
            key: AccessFieldKey,
        ) -> Result<AccessFieldProjection<'view, $extent>, AccessPlanDiagnostic> {
            self.project_with(key, BorrowPolarity::Exclusive)
        }
    };
}
pub(crate) use field_projection_accessors;

pub(crate) mod access_admission {
    use extents::{Extent, ExtentLoan};

    use crate::ResourceProfileReceiptId;
    use crate::normalized_identity;
    use crate::{
        AccessPlanDiagnostic, AccessResourceCompatibility, AccessView, AdmittedResourceProfile,
        OwnedAccessAdmission, OwnedAccessRejection, ValidatedAccessPlan, validate_access_resources,
    };

    normalized_identity!(AccessAdmissionId, "placement-admission identity cannot be zero", pub(crate));

    normalized_identity!(
        AccessOccurrenceId,
        "placed-occurrence identity cannot be zero"
    );

    #[derive(Debug)]
    pub struct AccessAdmission<'extent> {
        pub(crate) identity: AccessAdmissionId,
        pub(crate) access_plan: ValidatedAccessPlan,
        pub(crate) profile_receipt: ResourceProfileReceiptId,
        pub(crate) profile: AdmittedResourceProfile,
        pub(crate) resources: AccessResourceCompatibility,
        pub(crate) loan: ExtentLoan<'extent>,
    }

    #[derive(Debug)]
    pub struct AccessRejection<'extent> {
        loan: ExtentLoan<'extent>,
        diagnostic: AccessPlanDiagnostic,
    }

    #[derive(Debug)]
    pub struct PlaceEstablishmentError<'extent> {
        admission: AccessAdmission<'extent>,
        diagnostic: AccessPlanDiagnostic,
    }

    impl<'extent> AccessAdmission<'extent> {
        pub const fn identity(&self) -> AccessAdmissionId {
            self.identity
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.profile_receipt
        }

        pub const fn resources(&self) -> &AccessResourceCompatibility {
            &self.resources
        }

        pub fn withdraw(self) -> ExtentLoan<'extent> {
            self.loan
        }
    }

    impl<'extent> PlaceEstablishmentError<'extent> {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (AccessAdmission<'extent>, AccessPlanDiagnostic) {
            (self.admission, self.diagnostic)
        }
    }

    impl<'extent> AccessRejection<'extent> {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (ExtentLoan<'extent>, AccessPlanDiagnostic) {
            (self.loan, self.diagnostic)
        }
    }

    pub fn admit_access<'extent>(
        identity: AccessAdmissionId,
        loan: ExtentLoan<'extent>,
        plan: &ValidatedAccessPlan,
        profile: &AdmittedResourceProfile,
    ) -> Result<AccessAdmission<'extent>, AccessRejection<'extent>> {
        let validation = validate_access_admission(&loan, plan, profile);
        match validation {
            Ok(resources) => Ok(AccessAdmission {
                identity,
                access_plan: plan.clone(),
                profile_receipt: profile.receipt(),
                profile: profile.clone(),
                resources,
                loan,
            }),
            Err(diagnostic) => Err(AccessRejection { loan, diagnostic }),
        }
    }

    pub fn admit_owned_access(
        identity: AccessAdmissionId,
        extent: Extent,
        plan: &ValidatedAccessPlan,
        profile: &AdmittedResourceProfile,
    ) -> Result<OwnedAccessAdmission, OwnedAccessRejection> {
        let validation = match extent.loan(0, extent.length()) {
            Ok(loan) => validate_access_admission(&loan, plan, profile),
            Err(diagnostic) => Err(AccessPlanDiagnostic(format!(
                "owned extent could not produce its internal whole-range loan: {diagnostic}"
            ))),
        };
        match validation {
            Ok(resources) => Ok(OwnedAccessAdmission {
                identity,
                access_plan: plan.clone(),
                profile_receipt: profile.receipt(),
                profile: profile.clone(),
                resources,
                extent,
            }),
            Err(diagnostic) => Err(OwnedAccessRejection { extent, diagnostic }),
        }
    }

    pub(super) fn validate_access_admission(
        loan: &ExtentLoan<'_>,
        plan: &ValidatedAccessPlan,
        profile: &AdmittedResourceProfile,
    ) -> Result<AccessResourceCompatibility, AccessPlanDiagnostic> {
        let restricted = profile.restrict_to_loan(loan)?;
        let compatibility = validate_access_resources(plan, &restricted)?;
        if !compatibility.base.admits(loan.base()) {
            return Err(AccessPlanDiagnostic(format!(
                "extent loan base {} does not satisfy placement base congruence: base mod {} must equal {}",
                loan.base(),
                compatibility.base.modulus,
                compatibility.base.residue
            )));
        }
        Ok(compatibility)
    }

    pub fn place<'extent>(
        admission: AccessAdmission<'extent>,
    ) -> Result<AccessView<'extent>, PlaceEstablishmentError<'extent>> {
        let diagnostic = if admission.profile.receipt() != admission.profile_receipt {
            Some(AccessPlanDiagnostic(
                "borrowed placement profile receipt differs from its retained admitted profile"
                    .into(),
            ))
        } else {
            match validate_access_admission(
                &admission.loan,
                &admission.access_plan,
                &admission.profile,
            ) {
                Ok(resources) if resources == admission.resources => None,
                Ok(_) => Some(AccessPlanDiagnostic(
                    "borrowed placement replayed resource compatibility differs from the retained admission"
                        .into(),
                )),
                Err(diagnostic) => Some(AccessPlanDiagnostic(format!(
                    "borrowed access-view establishment could not replay the admitted resource profile: {diagnostic}"
                ))),
            }
        };
        if let Some(diagnostic) = diagnostic {
            return Err(PlaceEstablishmentError {
                admission,
                diagnostic,
            });
        }
        Ok(AccessView {
            loan: admission.loan,
            plan: admission.access_plan,
            profile_receipt: admission.profile_receipt,
            profile: admission.profile,
            resources: admission.resources,
            admission: admission.identity,
        })
    }
}

pub(crate) mod access_authority {
    use extents::{LoanPolarity, ResidentClaimId};

    use crate::placements::access_admission::validate_access_admission;
    use crate::placements::owned_atomic_resident_custody::validate_owned_atomic_resident_authority;
    use crate::placements::owned_resident_custody::replay_owned_admission_resources;
    use crate::placements::owned_resident_custody::validate_owned_content_binding;
    use crate::placements::owned_resident_custody::validate_provider_content_binding;
    use crate::placements::owned_resident_custody::validate_resident_observation;
    use crate::{
        AccessAdmissionId, AccessOccurrenceId, AccessPlanDiagnostic, AccessResourceCompatibility,
        AccessView, AdmittedResourceProfile, AdmittedSchemaDeviceCorrespondence, BorrowPolarity,
        EstablishedBorrowedAtomicResidentAccess, EstablishedBorrowedResidentAccess,
        EstablishedOwnedAccess, EstablishedOwnedAtomicAccess, EstablishedOwnedExternalAccess,
        ResourceProfileReceiptId, ValidatedAccessPlan,
    };

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum AccessAuthorityRef<'view, 'extent> {
        Borrowed(&'view AccessView<'extent>),
        CorrespondedBorrowed(
            &'view crate::placements::schema_correspondence::SchemaCorrespondedAccessView<'extent>,
        ),
        BorrowedResident(&'view EstablishedBorrowedResidentAccess<'extent>),
        BorrowedAtomicResident(&'view EstablishedBorrowedAtomicResidentAccess<'extent>),
        EstablishedOwned(&'view EstablishedOwnedAccess),
        EstablishedOwnedAtomic(&'view EstablishedOwnedAtomicAccess),
        OwnedCorrespondedExternal(&'view EstablishedOwnedExternalAccess),
    }

    impl<'view, 'extent> AccessAuthorityRef<'view, 'extent> {
        pub(crate) const fn base(self) -> u64 {
            match self {
                Self::Borrowed(view) => view.loan.base(),
                Self::CorrespondedBorrowed(view) => view.view().loan.base(),
                Self::BorrowedResident(established) => established.base(),
                Self::BorrowedAtomicResident(established) => established.base(),
                Self::EstablishedOwned(established) => established.extent().base(),
                Self::EstablishedOwnedAtomic(established) => established.extent().base(),
                Self::OwnedCorrespondedExternal(established) => established.extent().base(),
            }
        }

        pub(crate) const fn access_plan(self) -> &'view ValidatedAccessPlan {
            match self {
                Self::Borrowed(view) => &view.plan,
                Self::CorrespondedBorrowed(view) => &view.view().plan,
                Self::BorrowedResident(established) => established.access_plan(),
                Self::BorrowedAtomicResident(established) => established.access_plan(),
                Self::EstablishedOwned(established) => established.access_plan(),
                Self::EstablishedOwnedAtomic(established) => established.access_plan(),
                Self::OwnedCorrespondedExternal(established) => established.access_plan(),
            }
        }

        pub(crate) const fn profile_receipt(self) -> ResourceProfileReceiptId {
            match self {
                Self::Borrowed(view) => view.profile_receipt,
                Self::CorrespondedBorrowed(view) => view.view().profile_receipt,
                Self::BorrowedResident(established) => established.profile_receipt(),
                Self::BorrowedAtomicResident(established) => established.profile_receipt(),
                Self::EstablishedOwned(established) => established.profile_receipt(),
                Self::EstablishedOwnedAtomic(established) => established.profile_receipt(),
                Self::OwnedCorrespondedExternal(established) => established.profile_receipt(),
            }
        }

        pub(crate) const fn profile(self) -> &'view AdmittedResourceProfile {
            match self {
                Self::Borrowed(view) => &view.profile,
                Self::CorrespondedBorrowed(view) => &view.view().profile,
                Self::BorrowedResident(established) => established.profile(),
                Self::BorrowedAtomicResident(established) => established.profile(),
                Self::EstablishedOwned(established) => &established.admission.profile,
                Self::EstablishedOwnedAtomic(established) => established.profile(),
                Self::OwnedCorrespondedExternal(established) => &established.admission.profile,
            }
        }

        pub(crate) fn replay_resources(
            self,
        ) -> Result<AccessResourceCompatibility, AccessPlanDiagnostic> {
            match self {
                Self::Borrowed(view) => {
                    validate_access_admission(&view.loan, &view.plan, &view.profile)
                }
                Self::CorrespondedBorrowed(view) => validate_access_admission(
                    &view.view().loan,
                    &view.view().plan,
                    &view.view().profile,
                ),
                Self::BorrowedResident(established) => validate_access_admission(
                    established.loan(),
                    established.access_plan(),
                    established.profile(),
                ),
                Self::BorrowedAtomicResident(established) => {
                    established
                        .validate_lender_binding("borrowed Atomic resident resource replay")?;
                    validate_access_admission(
                        established.loan(),
                        established.access_plan(),
                        established.profile(),
                    )
                }
                Self::EstablishedOwned(established) => {
                    replay_owned_admission_resources(&established.admission)
                }
                Self::EstablishedOwnedAtomic(established) => {
                    replay_owned_admission_resources(&established.admission)
                }
                Self::OwnedCorrespondedExternal(established) => {
                    replay_owned_admission_resources(&established.admission)
                }
            }
        }

        pub(crate) fn replay_resident_content(
            self,
            transition: &str,
        ) -> Result<(), AccessPlanDiagnostic> {
            let replay = match self {
                Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => return Ok(()),
                Self::BorrowedResident(established) => validate_provider_content_binding(
                    established.access_plan(),
                    established.loan(),
                    established.content(),
                )
                .and_then(|()| {
                    validate_resident_observation(
                        established.access_plan(),
                        crate::access_fields::ObservationModel::Stable,
                        transition,
                    )
                }),
                Self::BorrowedAtomicResident(established) => validate_provider_content_binding(
                    established.access_plan(),
                    established.loan(),
                    established.content(),
                )
                .and_then(|()| {
                    validate_resident_observation(
                        established.access_plan(),
                        crate::access_fields::ObservationModel::Atomic,
                        transition,
                    )
                }),
                Self::EstablishedOwned(established) => {
                    validate_owned_content_binding(&established.admission, &established.content)
                        .and_then(|()| {
                            validate_resident_observation(
                                established.access_plan(),
                                crate::access_fields::ObservationModel::Stable,
                                transition,
                            )
                        })
                }
                Self::EstablishedOwnedAtomic(established) => {
                    validate_owned_atomic_resident_authority(
                        &established.admission,
                        &established.content,
                        transition,
                    )
                }
                Self::OwnedCorrespondedExternal(established) => {
                    established.validate_external_observation(transition)
                }
            };
            replay.map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "{transition} could not replay the retained resident content grant: {diagnostic}"
                ))
            })
        }

        pub(crate) fn replay_correspondence(
            self,
            transition: &str,
        ) -> Result<(), AccessPlanDiagnostic> {
            match self {
                Self::CorrespondedBorrowed(view) => {
                    view.validate_correspondence().map_err(|diagnostic| {
                        AccessPlanDiagnostic(format!(
                            "{transition} could not replay the retained schema/device correspondence: {diagnostic}"
                        ))
                    })
                }
                Self::OwnedCorrespondedExternal(established) => {
                    established.validate_correspondence(transition)
                }
                _ => Ok(()),
            }
        }

        pub(crate) const fn correspondence(
            self,
        ) -> Option<&'view AdmittedSchemaDeviceCorrespondence> {
            match self {
                Self::CorrespondedBorrowed(view) => Some(view.correspondence()),
                Self::OwnedCorrespondedExternal(established) => Some(established.correspondence()),
                _ => None,
            }
        }

        pub(crate) const fn resources(self) -> &'view AccessResourceCompatibility {
            match self {
                Self::Borrowed(view) => &view.resources,
                Self::CorrespondedBorrowed(view) => &view.view().resources,
                Self::BorrowedResident(established) => established.resources(),
                Self::BorrowedAtomicResident(established) => established.resources(),
                Self::EstablishedOwned(established) => established.resources(),
                Self::EstablishedOwnedAtomic(established) => established.resources(),
                Self::OwnedCorrespondedExternal(established) => established.resources(),
            }
        }

        pub(crate) const fn admission(self) -> AccessAdmissionId {
            match self {
                Self::Borrowed(view) => view.admission,
                Self::CorrespondedBorrowed(view) => view.view().admission,
                Self::BorrowedResident(established) => established.admission(),
                Self::BorrowedAtomicResident(established) => established.admission(),
                Self::EstablishedOwned(established) => established.admission(),
                Self::EstablishedOwnedAtomic(established) => established.admission(),
                Self::OwnedCorrespondedExternal(established) => established.admission(),
            }
        }

        pub(crate) const fn source_loan(self) -> BorrowPolarity {
            let polarity = match self {
                Self::Borrowed(view) => view.loan.polarity(),
                Self::CorrespondedBorrowed(view) => view.view().loan.polarity(),
                Self::BorrowedResident(established) => established.loan_polarity(),
                Self::BorrowedAtomicResident(established) => established.loan_polarity(),
                Self::EstablishedOwned(_)
                | Self::EstablishedOwnedAtomic(_)
                | Self::OwnedCorrespondedExternal(_) => LoanPolarity::Exclusive,
            };
            match polarity {
                LoanPolarity::Shared => BorrowPolarity::Shared,
                LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
            }
        }

        pub(crate) const fn resident_claim(self) -> Option<ResidentClaimId> {
            match self {
                Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => None,
                Self::BorrowedResident(established) => Some(established.resident_claim()),
                Self::BorrowedAtomicResident(established) => Some(established.resident_claim()),
                Self::EstablishedOwned(established) => Some(established.resident_claim()),
                Self::EstablishedOwnedAtomic(established) => Some(established.resident_claim()),
                Self::OwnedCorrespondedExternal(_) => None,
            }
        }

        pub(crate) const fn access_occurrence(self) -> Option<AccessOccurrenceId> {
            match self {
                Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => None,
                Self::BorrowedResident(established) => Some(established.occurrence()),
                Self::BorrowedAtomicResident(established) => Some(established.occurrence()),
                Self::EstablishedOwned(established) => Some(established.occurrence),
                Self::EstablishedOwnedAtomic(established) => Some(established.occurrence()),
                Self::OwnedCorrespondedExternal(established) => Some(established.occurrence()),
            }
        }
    }
}

pub(crate) mod access_plan {
    use crate::plan_policy::normalized_identities::{
        authoritative_access_interpretation, non_authoritative_access_compatibility_fingerprint,
    };
    use crate::validate_access_fields;
    use crate::{AccessFields, AccessPlanDiagnostic, BoundaryReach, ValidatedAccessFields};
    use layout_plans::LayoutPlanReport;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AccessPlan {
        pub layout: LayoutPlanReport,
        pub access: AccessFields,
        pub reach: BoundaryReach,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct AccessPlanId(pub(crate) u64);

    impl AccessPlanId {
        pub const fn compatibility_fingerprint(self) -> u64 {
            self.0
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ValidatedAccessPlan {
        pub(crate) identity: AccessPlanId,
        pub(crate) content_interpretation: extents::ExtentContentInterpretation,
        pub(crate) layout: LayoutPlanReport,
        pub(crate) access: ValidatedAccessFields,
        pub(crate) reach: BoundaryReach,
    }

    impl ValidatedAccessPlan {
        pub const fn identity(&self) -> AccessPlanId {
            self.identity
        }

        pub const fn content_interpretation(&self) -> extents::ExtentContentInterpretation {
            self.content_interpretation
        }

        pub const fn layout(&self) -> &LayoutPlanReport {
            &self.layout
        }

        pub const fn access(&self) -> &ValidatedAccessFields {
            &self.access
        }

        pub const fn reach(&self) -> &BoundaryReach {
            &self.reach
        }
    }

    pub fn validate_access_plan(
        plan: AccessPlan,
    ) -> Result<ValidatedAccessPlan, AccessPlanDiagnostic> {
        let AccessPlan {
            layout,
            access,
            reach,
        } = plan;
        let access = validate_access_fields(access, &layout)?;
        let identity =
            non_authoritative_access_compatibility_fingerprint(access.identity(), &reach);
        let mut validated = ValidatedAccessPlan {
            identity,
            content_interpretation: extents::ExtentContentInterpretation::from_sha256_commitment(
                extents::ExtentContentInterpretationId::from_normalized_identity(
                    identity.compatibility_fingerprint(),
                )
                .map_err(|diagnostic| AccessPlanDiagnostic(diagnostic.to_string()))?,
                [0; 32],
            ),
            layout,
            access,
            reach,
        };
        validated.content_interpretation = authoritative_access_interpretation(&validated);
        Ok(validated)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AccessPlanSnapshot {
        pub layout: LayoutPlanReport,
        pub access: Vec<crate::FieldAccess>,
        pub reach: BoundaryReach,
    }

    impl ValidatedAccessPlan {
        pub fn snapshot(&self) -> AccessPlanSnapshot {
            AccessPlanSnapshot {
                layout: self.layout.clone(),
                access: self
                    .access
                    .plan()
                    .entries()
                    .iter()
                    .map(|entry| entry.access().clone())
                    .collect(),
                reach: self.reach.clone(),
            }
        }
    }

    impl AccessPlanSnapshot {
        pub fn validate(self) -> Result<ValidatedAccessPlan, AccessPlanDiagnostic> {
            let mut access = AccessFields::inaccessible(&self.layout)?;
            if access.entries().len() != self.access.len() {
                return Err(AccessPlanDiagnostic(
                    "placement snapshot access cardinality differs from its exact layout".into(),
                ));
            }
            for (slot, field_access) in self.access.into_iter().enumerate() {
                let key = access.key_at(slot).ok_or_else(|| {
                    AccessPlanDiagnostic("placement snapshot lost its canonical access slot".into())
                })?;
                access.set(key, field_access)?;
            }
            validate_access_plan(AccessPlan {
                layout: self.layout,
                access,
                reach: self.reach,
            })
        }
    }
}

pub(crate) mod atomic_resident_views {
    use extents::{
        ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLoan, LoanPolarity,
        ProviderExistingContentGrant, ResidentClaimId,
    };

    use crate::placements::access_admission::validate_access_admission;
    use crate::placements::access_authority::AccessAuthorityRef;
    use crate::placements::owned_atomic_resident_custody::validate_owned_atomic_resident_authority;
    use crate::placements::owned_resident_custody::{
        validate_provider_content_binding, validate_resident_observation,
    };
    use crate::primitive_access::field_projection::project_access_field;
    use crate::{
        AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
        AccessPlanDiagnostic, AccessResourceCompatibility, AdmittedResourceProfile, BorrowPolarity,
        DormantOwnedAtomicResident, ObservationModel, ResourceProfileReceiptId,
        ValidatedAccessPlan,
    };

    impl DormantOwnedAtomicResident {
        pub fn borrow_view(
            &self,
            occurrence: AccessOccurrenceId,
        ) -> Result<EstablishedBorrowedAtomicResidentAccess<'_>, AccessPlanDiagnostic> {
            validate_owned_atomic_resident_authority(
                &self.admission,
                &self.content,
                "borrowed Atomic resident shared-view establishment",
            )?;
            let length = self.admission.extent.length();
            let loan = self
                .admission
                .extent
                .loan(0, length)
                .map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "borrowed Atomic resident whole-range loan rejected: {diagnostic}"
                    ))
                })?;
            Ok(EstablishedBorrowedAtomicResidentAccess {
                plan: self.admission.access_plan.clone(),
                lender_plan: &self.admission.access_plan,
                profile_receipt: self.admission.profile_receipt,
                profile: &self.admission.profile,
                resources: &self.admission.resources,
                admission: self.admission.identity,
                lender_admission: &self.admission.identity,
                loan,
                content: &self.content,
                occurrence,
            })
        }

        pub fn borrow_view_mut(
            &mut self,
            occurrence: AccessOccurrenceId,
        ) -> Result<EstablishedBorrowedAtomicResidentAccess<'_>, AccessPlanDiagnostic> {
            validate_owned_atomic_resident_authority(
                &self.admission,
                &self.content,
                "borrowed Atomic resident exclusive-view establishment",
            )?;
            let plan = self.admission.access_plan.clone();
            let lender_plan = &self.admission.access_plan;
            let profile_receipt = self.admission.profile_receipt;
            let profile = &self.admission.profile;
            let resources = &self.admission.resources;
            let admission = self.admission.identity;
            let lender_admission = &self.admission.identity;
            let content = &self.content;
            let length = self.admission.extent.length();
            let loan = self
                .admission
                .extent
                .loan_mut(0, length)
                .map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "borrowed Atomic resident whole-range loan rejected: {diagnostic}"
                    ))
                })?;
            Ok(EstablishedBorrowedAtomicResidentAccess {
                plan,
                lender_plan,
                profile_receipt,
                profile,
                resources,
                admission,
                lender_admission,
                loan,
                content,
                occurrence,
            })
        }
    }

    #[derive(Debug)]
    #[must_use = "active borrowed Atomic content retains an exact resident loan"]
    pub struct EstablishedBorrowedAtomicResidentAccess<'resident> {
        plan: ValidatedAccessPlan,
        lender_plan: &'resident ValidatedAccessPlan,
        profile_receipt: ResourceProfileReceiptId,
        profile: &'resident AdmittedResourceProfile,
        resources: &'resident AccessResourceCompatibility,
        admission: AccessAdmissionId,
        lender_admission: &'resident AccessAdmissionId,
        loan: ExtentLoan<'resident>,
        content: &'resident ProviderExistingContentGrant,
        occurrence: AccessOccurrenceId,
    }

    #[derive(Debug)]
    pub struct BorrowedAtomicResidentRetirementError<'resident> {
        established: EstablishedBorrowedAtomicResidentAccess<'resident>,
        diagnostic: AccessPlanDiagnostic,
    }

    impl<'resident> BorrowedAtomicResidentRetirementError<'resident> {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            EstablishedBorrowedAtomicResidentAccess<'resident>,
            AccessPlanDiagnostic,
        ) {
            (self.established, self.diagnostic)
        }
    }

    impl<'resident> EstablishedBorrowedAtomicResidentAccess<'resident> {
        pub(super) const fn profile(&self) -> &AdmittedResourceProfile {
            self.profile
        }

        pub(super) const fn resources(&self) -> &AccessResourceCompatibility {
            self.resources
        }

        resident_view_accessors!();

        fn project_with<'view>(
            &'view self,
            key: AccessFieldKey,
            current_borrow: BorrowPolarity,
        ) -> Result<AccessFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
            self.validate_lender_binding("borrowed Atomic resident projection")?;
            let source_loan = BorrowPolarity::from(self.loan.polarity());
            project_access_field(
                &self.plan,
                self.profile_receipt,
                self.resources,
                self.admission,
                self.loan.base(),
                key,
                current_borrow,
                source_loan,
                Some(ObservationModel::Atomic),
                AccessAuthorityRef::BorrowedAtomicResident(self),
            )
        }

        fn validate_retirement_authority(&self) -> Result<(), AccessPlanDiagnostic> {
            self.validate_lender_binding("borrowed Atomic resident retirement")?;
            if self.profile.receipt() != self.profile_receipt {
                return Err(AccessPlanDiagnostic(
                    "borrowed Atomic resident retirement profile receipt differs from its retained admitted profile"
                        .into(),
                ));
            }
            let replayed = validate_access_admission(&self.loan, &self.plan, self.profile)
                .map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "borrowed Atomic resident retirement could not replay the retained access authority: {diagnostic}"
                    ))
                })?;
            if &replayed != self.resources {
                return Err(AccessPlanDiagnostic(
                    "borrowed Atomic resident retirement replayed resource compatibility differs from the retained active carrier"
                        .into(),
                ));
            }
            validate_provider_content_binding(&self.plan, &self.loan, self.content).map_err(
                |diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "borrowed Atomic resident retirement could not replay the retained provider content grant: {diagnostic}"
                    ))
                },
            )?;
            validate_resident_observation(
                &self.plan,
                ObservationModel::Atomic,
                "borrowed Atomic resident retirement",
            )
        }

        pub(super) fn validate_lender_binding(
            &self,
            transition: &str,
        ) -> Result<(), AccessPlanDiagnostic> {
            if &self.plan != self.lender_plan
                || self.profile_receipt != self.profile.receipt()
                || &self.admission != self.lender_admission
            {
                return Err(AccessPlanDiagnostic(format!(
                    "{transition} retained placement/profile authority differs from the exact lender"
                )));
            }
            Ok(())
        }

        pub fn retire(self) -> Result<(), BorrowedAtomicResidentRetirementError<'resident>> {
            if let Err(diagnostic) = self.validate_retirement_authority() {
                return Err(BorrowedAtomicResidentRetirementError {
                    established: self,
                    diagnostic,
                });
            }
            Ok(())
        }

        #[cfg(test)]
        pub(crate) fn replace_plan_for_test(
            &mut self,
            plan: ValidatedAccessPlan,
        ) -> ValidatedAccessPlan {
            std::mem::replace(&mut self.plan, plan)
        }

        #[cfg(test)]
        pub(crate) fn replace_admission_for_test(
            &mut self,
            admission: AccessAdmissionId,
        ) -> AccessAdmissionId {
            std::mem::replace(&mut self.admission, admission)
        }
    }
}

pub(crate) mod borrowed_view {
    use extents::ExtentLoan;

    use crate::AdmittedResourceProfile;
    use crate::ResourceProfileReceiptId;
    use crate::placements::access_admission::validate_access_admission;
    use crate::placements::access_authority::AccessAuthorityRef;
    use crate::primitive_access::field_projection::project_access_field;
    use crate::{
        AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessPlanDiagnostic,
        BorrowPolarity,
    };
    use crate::{AccessResourceCompatibility, ValidatedAccessPlan};

    #[derive(Debug)]
    pub struct AccessView<'extent> {
        pub(crate) loan: ExtentLoan<'extent>,
        pub(crate) plan: ValidatedAccessPlan,
        pub(crate) profile_receipt: ResourceProfileReceiptId,
        pub(crate) profile: AdmittedResourceProfile,
        pub(crate) resources: AccessResourceCompatibility,
        pub(crate) admission: AccessAdmissionId,
    }

    #[derive(Debug)]
    pub struct AccessViewRetirementError<'extent> {
        view: AccessView<'extent>,
        diagnostic: AccessPlanDiagnostic,
    }

    impl<'extent> AccessView<'extent> {
        pub const fn admission(&self) -> AccessAdmissionId {
            self.admission
        }

        pub const fn base(&self) -> u64 {
            self.loan.base()
        }

        pub const fn length(&self) -> u64 {
            self.loan.length()
        }

        pub fn retire(self) -> Result<ExtentLoan<'extent>, AccessViewRetirementError<'extent>> {
            if let Err(diagnostic) = self.validate_authority("borrowed access-view retirement") {
                return Err(AccessViewRetirementError {
                    view: self,
                    diagnostic,
                });
            }
            Ok(self.loan)
        }

        crate::placements::field_projection_accessors!('extent);

        fn project_with<'view>(
            &'view self,
            key: AccessFieldKey,
            current_borrow: BorrowPolarity,
        ) -> Result<AccessFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
            let source_loan = BorrowPolarity::from(self.loan.polarity());
            project_access_field(
                &self.plan,
                self.profile_receipt,
                &self.resources,
                self.admission,
                self.loan.base(),
                key,
                current_borrow,
                source_loan,
                None,
                AccessAuthorityRef::Borrowed(self),
            )
        }

        pub(super) fn validate_authority(
            &self,
            transition: &str,
        ) -> Result<(), AccessPlanDiagnostic> {
            if self.profile.receipt() != self.profile_receipt {
                return Err(AccessPlanDiagnostic(format!(
                    "{transition} could not replay the exact admitted resource-profile receipt"
                )));
            }
            let replayed = validate_access_admission(&self.loan, &self.plan, &self.profile)
                .map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "{transition} could not replay the retained access authority: {diagnostic}"
                    ))
                })?;
            if replayed != self.resources {
                return Err(AccessPlanDiagnostic(format!(
                    "{transition} replayed resource compatibility differs from the retained view"
                )));
            }
            Ok(())
        }
    }

    impl<'extent> AccessViewRetirementError<'extent> {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (AccessView<'extent>, AccessPlanDiagnostic) {
            (self.view, self.diagnostic)
        }
    }
}

pub(crate) mod owned_access_lifecycle {
    use extents::{
        Extent, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId,
        ProviderExistingContentGrant, ResidentClaimId,
    };

    use crate::AdmittedResourceProfile;
    use crate::placements::access_authority::AccessAuthorityRef;
    use crate::placements::owned_resident_custody::validate_owned_resident_authority;
    use crate::primitive_access::field_projection::project_access_field;
    use crate::{
        AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
        AccessPlanDiagnostic, AccessResourceCompatibility, BorrowPolarity, ObservationModel,
        ResourceProfileReceiptId, ValidatedAccessPlan,
    };

    #[derive(Debug)]
    #[must_use = "an owned access admission retains linear Extent authority"]
    pub struct OwnedAccessAdmission {
        pub(crate) identity: AccessAdmissionId,
        pub(crate) access_plan: ValidatedAccessPlan,
        pub(crate) profile_receipt: ResourceProfileReceiptId,
        pub(crate) profile: AdmittedResourceProfile,
        pub(crate) resources: AccessResourceCompatibility,
        pub(crate) extent: Extent,
    }

    #[derive(Debug)]
    pub struct OwnedAccessRejection {
        pub(crate) extent: Extent,
        pub(crate) diagnostic: AccessPlanDiagnostic,
    }

    #[derive(Debug)]
    #[must_use = "dormant resident content retains linear Extent and content custody"]
    pub struct DormantOwnedResident {
        pub(crate) admission: OwnedAccessAdmission,
        pub(crate) content: ProviderExistingContentGrant,
    }

    #[derive(Debug)]
    pub struct OwnedResidentViewEstablishmentError {
        resident: DormantOwnedResident,
        occurrence: AccessOccurrenceId,
        diagnostic: AccessPlanDiagnostic,
    }

    #[derive(Debug)]
    #[must_use = "active owned placed content retains linear resident custody"]
    pub struct EstablishedOwnedAccess {
        pub(crate) admission: OwnedAccessAdmission,
        pub(crate) content: ProviderExistingContentGrant,
        pub(crate) occurrence: AccessOccurrenceId,
    }

    #[derive(Debug)]
    pub struct OwnedResidentRetirementError {
        established: EstablishedOwnedAccess,
        diagnostic: AccessPlanDiagnostic,
    }

    #[derive(Debug)]
    pub struct OwnedStableAdoptionError {
        pub(crate) admission: OwnedAccessAdmission,
        pub(crate) content: ProviderExistingContentGrant,
        pub(crate) diagnostic: AccessPlanDiagnostic,
    }

    impl OwnedAccessAdmission {
        pub const fn identity(&self) -> AccessAdmissionId {
            self.identity
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.profile_receipt
        }

        pub const fn resources(&self) -> &AccessResourceCompatibility {
            &self.resources
        }

        pub const fn extent(&self) -> &Extent {
            &self.extent
        }

        pub const fn access_plan(&self) -> &ValidatedAccessPlan {
            &self.access_plan
        }

        pub(crate) fn project_field<'view>(
            &'view self,
            key: AccessFieldKey,
            current_borrow: BorrowPolarity,
            source_loan: BorrowPolarity,
            required_observation: Option<ObservationModel>,
            authority: AccessAuthorityRef<'view, 'view>,
        ) -> Result<AccessFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
            project_access_field(
                &self.access_plan,
                self.profile_receipt,
                &self.resources,
                self.identity,
                self.extent.base(),
                key,
                current_borrow,
                source_loan,
                required_observation,
                authority,
            )
        }

        pub fn withdraw(self) -> Extent {
            self.extent
        }
    }

    impl OwnedAccessRejection {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (Extent, AccessPlanDiagnostic) {
            (self.extent, self.diagnostic)
        }
    }

    impl OwnedResidentViewEstablishmentError {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            DormantOwnedResident,
            AccessOccurrenceId,
            AccessPlanDiagnostic,
        ) {
            (self.resident, self.occurrence, self.diagnostic)
        }
    }

    impl DormantOwnedResident {
        owned_view_accessors!(resident);

        pub fn view(
            self,
            occurrence: AccessOccurrenceId,
        ) -> Result<EstablishedOwnedAccess, OwnedResidentViewEstablishmentError> {
            if let Err(diagnostic) = validate_owned_resident_authority(
                &self.admission,
                &self.content,
                "owned resident view",
            ) {
                return Err(OwnedResidentViewEstablishmentError {
                    resident: self,
                    occurrence,
                    diagnostic,
                });
            }
            Ok(EstablishedOwnedAccess {
                admission: self.admission,
                content: self.content,
                occurrence,
            })
        }
    }

    impl OwnedResidentRetirementError {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (EstablishedOwnedAccess, AccessPlanDiagnostic) {
            (self.established, self.diagnostic)
        }
    }

    impl EstablishedOwnedAccess {
        owned_view_accessors!(resident, occurrence);

        pub fn retire_resident(self) -> Result<DormantOwnedResident, OwnedResidentRetirementError> {
            if let Err(diagnostic) = validate_owned_resident_authority(
                &self.admission,
                &self.content,
                "resident-preserving retirement",
            ) {
                return Err(OwnedResidentRetirementError {
                    established: self,
                    diagnostic,
                });
            }
            Ok(DormantOwnedResident {
                admission: self.admission,
                content: self.content,
            })
        }

        crate::placements::field_projection_accessors!('view);

        fn project_with<'view>(
            &'view self,
            key: AccessFieldKey,
            current_borrow: BorrowPolarity,
        ) -> Result<AccessFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
            self.admission.project_field(
                key,
                current_borrow,
                BorrowPolarity::Exclusive,
                Some(ObservationModel::Stable),
                AccessAuthorityRef::EstablishedOwned(self),
            )
        }
    }

    impl OwnedStableAdoptionError {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            OwnedAccessAdmission,
            ProviderExistingContentGrant,
            AccessPlanDiagnostic,
        ) {
            (self.admission, self.content, self.diagnostic)
        }
    }
}

pub(crate) mod owned_atomic_resident_custody {
    use extents::{
        Extent, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId,
        ProviderExistingContentGrant, ResidentClaimId,
    };

    use crate::placements::access_authority::AccessAuthorityRef;
    use crate::placements::owned_resident_custody::validate_owned_admission_authority;
    use crate::placements::owned_resident_custody::validate_owned_content_binding;
    use crate::{
        AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
        AccessPlanDiagnostic, AccessResourceCompatibility, BorrowPolarity, ObservationModel,
        OwnedAccessAdmission, ResourceProfileReceiptId, ValidatedAccessPlan,
    };

    #[derive(Debug)]
    #[must_use = "dormant Atomic resident content retains linear Extent and content custody"]
    pub struct DormantOwnedAtomicResident {
        pub(crate) admission: OwnedAccessAdmission,
        pub(crate) content: ProviderExistingContentGrant,
    }

    #[derive(Debug)]
    #[must_use = "active owned Atomic placement retains linear resident custody"]
    pub struct EstablishedOwnedAtomicAccess {
        pub(super) admission: OwnedAccessAdmission,
        pub(crate) content: ProviderExistingContentGrant,
        pub(super) occurrence: AccessOccurrenceId,
    }

    #[derive(Debug)]
    pub struct OwnedAtomicAdoptionError {
        admission: OwnedAccessAdmission,
        content: ProviderExistingContentGrant,
        diagnostic: AccessPlanDiagnostic,
    }

    impl OwnedAtomicAdoptionError {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            OwnedAccessAdmission,
            ProviderExistingContentGrant,
            AccessPlanDiagnostic,
        ) {
            (self.admission, self.content, self.diagnostic)
        }
    }

    #[derive(Debug)]
    pub struct OwnedAtomicResidentViewEstablishmentError {
        resident: DormantOwnedAtomicResident,
        occurrence: AccessOccurrenceId,
        diagnostic: AccessPlanDiagnostic,
    }

    impl OwnedAtomicResidentViewEstablishmentError {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            DormantOwnedAtomicResident,
            AccessOccurrenceId,
            AccessPlanDiagnostic,
        ) {
            (self.resident, self.occurrence, self.diagnostic)
        }
    }

    #[derive(Debug)]
    pub struct OwnedAtomicResidentRetirementError {
        established: EstablishedOwnedAtomicAccess,
        diagnostic: AccessPlanDiagnostic,
    }

    impl OwnedAtomicResidentRetirementError {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (EstablishedOwnedAtomicAccess, AccessPlanDiagnostic) {
            (self.established, self.diagnostic)
        }
    }

    pub fn adopt_owned_atomic(
        admission: OwnedAccessAdmission,
        content: ProviderExistingContentGrant,
    ) -> Result<DormantOwnedAtomicResident, OwnedAtomicAdoptionError> {
        if let Err(diagnostic) =
            validate_owned_atomic_resident_authority(&admission, &content, "Atomic adoption")
        {
            return Err(OwnedAtomicAdoptionError {
                admission,
                content,
                diagnostic,
            });
        }
        Ok(DormantOwnedAtomicResident { admission, content })
    }

    impl DormantOwnedAtomicResident {
        owned_view_accessors!(resident);

        pub fn view(
            self,
            occurrence: AccessOccurrenceId,
        ) -> Result<EstablishedOwnedAtomicAccess, OwnedAtomicResidentViewEstablishmentError>
        {
            if let Err(diagnostic) = validate_owned_atomic_resident_authority(
                &self.admission,
                &self.content,
                "Atomic resident view",
            ) {
                return Err(OwnedAtomicResidentViewEstablishmentError {
                    resident: self,
                    occurrence,
                    diagnostic,
                });
            }
            Ok(EstablishedOwnedAtomicAccess {
                admission: self.admission,
                content: self.content,
                occurrence,
            })
        }
    }

    impl EstablishedOwnedAtomicAccess {
        owned_view_accessors!(resident, occurrence);

        pub(super) const fn profile(
            &self,
        ) -> &crate::resources::resource_profile_admission::AdmittedResourceProfile {
            &self.admission.profile
        }

        pub fn project<'view>(
            &'view self,
            key: AccessFieldKey,
        ) -> Result<AccessFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
            self.admission.project_field(
                key,
                BorrowPolarity::Shared,
                BorrowPolarity::Exclusive,
                Some(ObservationModel::Atomic),
                AccessAuthorityRef::EstablishedOwnedAtomic(self),
            )
        }

        pub fn retire_resident(
            self,
        ) -> Result<DormantOwnedAtomicResident, OwnedAtomicResidentRetirementError> {
            if let Err(diagnostic) = validate_owned_atomic_resident_authority(
                &self.admission,
                &self.content,
                "Atomic resident-preserving retirement",
            ) {
                return Err(OwnedAtomicResidentRetirementError {
                    established: self,
                    diagnostic,
                });
            }
            Ok(DormantOwnedAtomicResident {
                admission: self.admission,
                content: self.content,
            })
        }
    }

    pub(super) fn validate_owned_atomic_resident_authority(
        admission: &OwnedAccessAdmission,
        content: &ProviderExistingContentGrant,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        validate_owned_admission_authority(
            admission,
            ObservationModel::Atomic,
            |transition| {
                validate_owned_content_binding(admission, content).map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "{transition} could not replay the retained provider content grant: {diagnostic}"
                    ))
                })
            },
            transition,
        )
    }
}

pub(crate) mod owned_external_correspondence {
    use extents::Extent;

    use crate::placements::access_authority::AccessAuthorityRef;
    use crate::placements::owned_resident_custody::{
        validate_owned_admission_authority, validate_resident_observation,
    };
    use crate::{
        AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
        AccessPlanDiagnostic, AccessResourceCompatibility, AdmittedSchemaDeviceCorrespondence,
        BorrowPolarity, ObservationModel, OwnedAccessAdmission, ResourceProfileReceiptId,
        ValidatedAccessPlan,
    };

    #[derive(Debug)]
    #[must_use = "owned corresponded admission retains linear Extent and physical provenance"]
    pub struct OwnedCorrespondedExternalAdmission {
        pub(crate) admission: OwnedAccessAdmission,
        pub(crate) correspondence: AdmittedSchemaDeviceCorrespondence,
    }

    #[derive(Debug)]
    #[must_use = "active owned External placement retains linear custody and physical provenance"]
    pub struct EstablishedOwnedExternalAccess {
        pub(crate) admission: OwnedAccessAdmission,
        pub(crate) correspondence: AdmittedSchemaDeviceCorrespondence,
        pub(crate) occurrence: AccessOccurrenceId,
    }

    #[derive(Debug)]
    pub struct OwnedExternalAdoptionError {
        admission: OwnedAccessAdmission,
        correspondence: AdmittedSchemaDeviceCorrespondence,
        diagnostic: AccessPlanDiagnostic,
    }

    impl OwnedExternalAdoptionError {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            OwnedAccessAdmission,
            AdmittedSchemaDeviceCorrespondence,
            AccessPlanDiagnostic,
        ) {
            (self.admission, self.correspondence, self.diagnostic)
        }
    }

    #[derive(Debug)]
    pub struct OwnedExternalViewEstablishmentError {
        carrier: OwnedCorrespondedExternalAdmission,
        occurrence: AccessOccurrenceId,
        diagnostic: AccessPlanDiagnostic,
    }

    impl OwnedExternalViewEstablishmentError {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            OwnedCorrespondedExternalAdmission,
            AccessOccurrenceId,
            AccessPlanDiagnostic,
        ) {
            (self.carrier, self.occurrence, self.diagnostic)
        }
    }

    #[derive(Debug)]
    pub struct OwnedExternalRetirementError {
        established: EstablishedOwnedExternalAccess,
        diagnostic: AccessPlanDiagnostic,
    }

    impl OwnedExternalRetirementError {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (EstablishedOwnedExternalAccess, AccessPlanDiagnostic) {
            (self.established, self.diagnostic)
        }
    }

    pub fn adopt_owned_external(
        admission: OwnedAccessAdmission,
        correspondence: AdmittedSchemaDeviceCorrespondence,
    ) -> Result<OwnedCorrespondedExternalAdmission, OwnedExternalAdoptionError> {
        if let Err(diagnostic) =
            validate_owned_external_authority(&admission, &correspondence, "External adoption")
        {
            return Err(OwnedExternalAdoptionError {
                admission,
                correspondence,
                diagnostic,
            });
        }
        Ok(OwnedCorrespondedExternalAdmission {
            admission,
            correspondence,
        })
    }

    fn validate_owned_external_authority(
        admission: &OwnedAccessAdmission,
        correspondence: &AdmittedSchemaDeviceCorrespondence,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        validate_owned_admission_authority(
            admission,
            ObservationModel::External,
            |transition| {
                validate_external_correspondence_binding(admission, correspondence, transition)
            },
            transition,
        )
    }

    fn validate_external_correspondence_binding(
        admission: &OwnedAccessAdmission,
        correspondence: &AdmittedSchemaDeviceCorrespondence,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        correspondence.validate_structure().map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "{transition} could not replay the retained schema/device correspondence: {diagnostic}"
            ))
        })?;
        if correspondence.access_plan() != &admission.access_plan
            || correspondence.profile_receipt() != admission.profile_receipt
        {
            return Err(AccessPlanDiagnostic(format!(
                "{transition} schema correspondence does not bind the owned access admission's exact plan and resource-profile receipt"
            )));
        }
        Ok(())
    }

    impl OwnedCorrespondedExternalAdmission {
        owned_view_accessors!(correspondence);

        pub fn into_parts(self) -> (OwnedAccessAdmission, AdmittedSchemaDeviceCorrespondence) {
            (self.admission, self.correspondence)
        }

        pub fn withdraw(self) -> (Extent, AdmittedSchemaDeviceCorrespondence) {
            (self.admission.withdraw(), self.correspondence)
        }

        pub fn view(
            self,
            occurrence: AccessOccurrenceId,
        ) -> Result<EstablishedOwnedExternalAccess, OwnedExternalViewEstablishmentError> {
            if let Err(diagnostic) = validate_owned_external_authority(
                &self.admission,
                &self.correspondence,
                "owned External view",
            ) {
                return Err(OwnedExternalViewEstablishmentError {
                    carrier: self,
                    occurrence,
                    diagnostic,
                });
            }
            Ok(EstablishedOwnedExternalAccess {
                admission: self.admission,
                correspondence: self.correspondence,
                occurrence,
            })
        }
    }

    impl EstablishedOwnedExternalAccess {
        owned_view_accessors!(correspondence, occurrence);

        pub(crate) fn validate_correspondence(
            &self,
            transition: &str,
        ) -> Result<(), AccessPlanDiagnostic> {
            validate_external_correspondence_binding(
                &self.admission,
                &self.correspondence,
                transition,
            )
        }

        pub(crate) fn validate_external_observation(
            &self,
            transition: &str,
        ) -> Result<(), AccessPlanDiagnostic> {
            validate_resident_observation(
                &self.admission.access_plan,
                ObservationModel::External,
                transition,
            )
        }

        pub fn retire(
            self,
        ) -> Result<OwnedCorrespondedExternalAdmission, OwnedExternalRetirementError> {
            if let Err(diagnostic) = validate_owned_external_authority(
                &self.admission,
                &self.correspondence,
                "owned External retirement",
            ) {
                return Err(OwnedExternalRetirementError {
                    established: self,
                    diagnostic,
                });
            }
            Ok(OwnedCorrespondedExternalAdmission {
                admission: self.admission,
                correspondence: self.correspondence,
            })
        }

        crate::placements::field_projection_accessors!('view);

        fn project_with<'view>(
            &'view self,
            key: AccessFieldKey,
            current_borrow: BorrowPolarity,
        ) -> Result<AccessFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
            self.admission.project_field(
                key,
                current_borrow,
                BorrowPolarity::Exclusive,
                Some(ObservationModel::External),
                AccessAuthorityRef::OwnedCorrespondedExternal(self),
            )
        }
    }
}

pub(crate) mod owned_resident_custody {
    use extents::{ExtentLoan, ProviderExistingContentGrant};

    use crate::access_fields::diagnostic::reject_when;
    use crate::placements::access_admission::validate_access_admission;
    use crate::{
        AccessPlanDiagnostic, AccessResourceCompatibility, DormantOwnedResident, ObservationModel,
        OwnedAccessAdmission, OwnedStableAdoptionError, ValidatedAccessPlan,
    };

    pub fn adopt_owned_stable(
        admission: OwnedAccessAdmission,
        content: ProviderExistingContentGrant,
    ) -> Result<DormantOwnedResident, OwnedStableAdoptionError> {
        if let Err(diagnostic) = validate_owned_stable_adoption(&admission, &content) {
            return Err(OwnedStableAdoptionError {
                admission,
                content,
                diagnostic,
            });
        }
        Ok(DormantOwnedResident { admission, content })
    }

    fn validate_owned_stable_adoption(
        admission: &OwnedAccessAdmission,
        content: &ProviderExistingContentGrant,
    ) -> Result<(), AccessPlanDiagnostic> {
        let replayed_resources =
            replay_owned_admission_resources(admission).map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "Stable adoption could not replay the admitted resource profile: {diagnostic}"
                ))
            })?;
        if replayed_resources != admission.resources {
            return Err(AccessPlanDiagnostic(
                "Stable adoption replayed resource compatibility differs from the owned admission"
                    .into(),
            ));
        }

        validate_owned_content_binding(admission, content)?;
        validate_resident_observation(
            &admission.access_plan,
            ObservationModel::Stable,
            "Stable adoption",
        )
    }

    pub(super) fn validate_owned_content_binding(
        admission: &OwnedAccessAdmission,
        content: &ProviderExistingContentGrant,
    ) -> Result<(), AccessPlanDiagnostic> {
        let extent = &admission.extent;
        let loan = extent.loan(0, extent.length()).map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "owned content binding could not replay its whole-range loan: {diagnostic}"
            ))
        })?;
        validate_provider_content_binding(&admission.access_plan, &loan, content)
    }

    pub(super) fn validate_provider_content_binding(
        plan: &ValidatedAccessPlan,
        loan: &ExtentLoan<'_>,
        content: &ProviderExistingContentGrant,
    ) -> Result<(), AccessPlanDiagnostic> {
        reject_when! {
            content.interpretation() != plan.content_interpretation()
                => "provider existing-content interpretation commitment does not match the admitted placement",
            content.origin() != loan.origin()
                => "provider existing-content origin does not match the admitted Extent",
            content.lineage_root() != loan.lineage_root()
                => "provider existing-content lineage does not match the admitted Extent",
            content.base() != loan.base() || content.length() != loan.length()
                => "provider existing-content geometry does not match the admitted Extent",
            content.address_space() != loan.address_space()
                => "provider existing-content address space does not match the admitted Extent",
            content.provenance() != loan.provenance()
                => "provider existing-content provenance does not match the admitted Extent",
            content.era() != loan.era()
                => "provider existing-content mapping era does not match the admitted Extent",
        }
        Ok(())
    }

    pub(super) fn validate_resident_observation(
        plan: &ValidatedAccessPlan,
        required: ObservationModel,
        route: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        if let Some(descriptor) = plan
            .access()
            .field_descriptors()
            .iter()
            .find(|descriptor| descriptor.observation() != required)
        {
            return Err(AccessPlanDiagnostic(format!(
                "field `{}` uses {:?} observation and cannot enter the {route}",
                descriptor.field(),
                descriptor.observation()
            )));
        }
        Ok(())
    }

    pub(super) fn replay_owned_admission_resources(
        admission: &OwnedAccessAdmission,
    ) -> Result<AccessResourceCompatibility, AccessPlanDiagnostic> {
        if admission.profile.receipt() != admission.profile_receipt {
            return Err(AccessPlanDiagnostic(
                "owned placement profile receipt differs from its retained admitted profile".into(),
            ));
        }
        let extent = &admission.extent;
        let loan = extent.loan(0, extent.length()).map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "owned placement could not replay its whole-range loan: {diagnostic}"
            ))
        })?;
        validate_access_admission(&loan, &admission.access_plan, &admission.profile)
    }

    pub(super) fn validate_owned_admission_authority(
        admission: &OwnedAccessAdmission,
        model: ObservationModel,
        binding: impl FnOnce(&str) -> Result<(), AccessPlanDiagnostic>,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        let resources = replay_owned_admission_resources(admission).map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "{transition} could not replay the retained access authority: {diagnostic}"
            ))
        })?;
        if resources != admission.resources {
            return Err(AccessPlanDiagnostic(format!(
                "{transition} replayed resource compatibility differs from the retained admission"
            )));
        }
        binding(transition)?;
        validate_resident_observation(&admission.access_plan, model, transition)
    }

    pub(super) fn validate_owned_resident_authority(
        admission: &OwnedAccessAdmission,
        content: &ProviderExistingContentGrant,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        validate_owned_admission_authority(
            admission,
            ObservationModel::Stable,
            |transition| {
                validate_owned_content_binding(admission, content).map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "{transition} could not replay the retained provider content grant: {diagnostic}"
                    ))
                })
            },
            transition,
        )
    }
}

pub(crate) mod resident_views {
    use crate::placements::access_admission::validate_access_admission;
    use crate::placements::access_authority::AccessAuthorityRef;
    use crate::placements::owned_resident_custody::validate_owned_resident_authority;
    use crate::placements::owned_resident_custody::validate_provider_content_binding;
    use crate::placements::owned_resident_custody::validate_resident_observation;
    use crate::primitive_access::field_projection::project_access_field;
    use crate::{
        AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
        AccessPlanDiagnostic, AccessResourceCompatibility, AdmittedResourceProfile, BorrowPolarity,
        DormantOwnedResident, ObservationModel, ResourceProfileReceiptId, ValidatedAccessPlan,
    };
    use extents::{
        ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLoan, LoanPolarity,
        ProviderExistingContentGrant, ResidentClaimId,
    };

    impl DormantOwnedResident {
        pub fn borrow_view(
            &self,
            occurrence: AccessOccurrenceId,
        ) -> Result<EstablishedBorrowedResidentAccess<'_>, AccessPlanDiagnostic> {
            validate_owned_resident_authority(
                &self.admission,
                &self.content,
                "borrowed resident shared-view establishment",
            )?;
            let length = self.admission.extent.length();
            let loan = self
                .admission
                .extent
                .loan(0, length)
                .map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "borrowed resident whole-range loan rejected: {diagnostic}"
                    ))
                })?;
            Ok(EstablishedBorrowedResidentAccess {
                plan: self.admission.access_plan.clone(),
                profile_receipt: self.admission.profile_receipt,
                profile: self.admission.profile.clone(),
                resources: self.admission.resources.clone(),
                admission: self.admission.identity,
                loan,
                content: &self.content,
                occurrence,
            })
        }

        pub fn borrow_view_mut(
            &mut self,
            occurrence: AccessOccurrenceId,
        ) -> Result<EstablishedBorrowedResidentAccess<'_>, AccessPlanDiagnostic> {
            validate_owned_resident_authority(
                &self.admission,
                &self.content,
                "borrowed resident exclusive-view establishment",
            )?;
            let plan = self.admission.access_plan.clone();
            let profile_receipt = self.admission.profile_receipt;
            let profile = self.admission.profile.clone();
            let resources = self.admission.resources.clone();
            let admission = self.admission.identity;
            let content = &self.content;
            let length = self.admission.extent.length();
            let loan = self
                .admission
                .extent
                .loan_mut(0, length)
                .map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "borrowed resident whole-range loan rejected: {diagnostic}"
                    ))
                })?;
            Ok(EstablishedBorrowedResidentAccess {
                plan,
                profile_receipt,
                profile,
                resources,
                admission,
                loan,
                content,
                occurrence,
            })
        }
    }

    #[derive(Debug)]
    #[must_use = "active borrowed placed content retains an exact resident loan"]
    pub struct EstablishedBorrowedResidentAccess<'resident> {
        plan: ValidatedAccessPlan,
        profile_receipt: ResourceProfileReceiptId,
        profile: AdmittedResourceProfile,
        resources: AccessResourceCompatibility,
        admission: AccessAdmissionId,
        loan: ExtentLoan<'resident>,
        content: &'resident ProviderExistingContentGrant,
        occurrence: AccessOccurrenceId,
    }

    #[derive(Debug)]
    pub struct BorrowedResidentRetirementError<'resident> {
        established: EstablishedBorrowedResidentAccess<'resident>,
        diagnostic: AccessPlanDiagnostic,
    }

    impl<'resident> BorrowedResidentRetirementError<'resident> {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            EstablishedBorrowedResidentAccess<'resident>,
            AccessPlanDiagnostic,
        ) {
            (self.established, self.diagnostic)
        }
    }

    impl<'resident> EstablishedBorrowedResidentAccess<'resident> {
        pub(super) const fn profile(&self) -> &AdmittedResourceProfile {
            &self.profile
        }

        pub(super) const fn resources(&self) -> &AccessResourceCompatibility {
            &self.resources
        }

        resident_view_accessors!();

        fn project_with<'view>(
            &'view self,
            key: AccessFieldKey,
            current_borrow: BorrowPolarity,
        ) -> Result<AccessFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
            let source_loan = BorrowPolarity::from(self.loan.polarity());
            project_access_field(
                &self.plan,
                self.profile_receipt,
                &self.resources,
                self.admission,
                self.loan.base(),
                key,
                current_borrow,
                source_loan,
                Some(ObservationModel::Stable),
                AccessAuthorityRef::BorrowedResident(self),
            )
        }

        fn validate_retirement_authority(&self) -> Result<(), AccessPlanDiagnostic> {
            if self.profile.receipt() != self.profile_receipt {
                return Err(AccessPlanDiagnostic(
                    "borrowed resident retirement profile receipt differs from its retained admitted profile"
                        .into(),
                ));
            }
            let replayed = validate_access_admission(&self.loan, &self.plan, &self.profile)
                .map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "borrowed resident retirement could not replay the retained access authority: {diagnostic}"
                    ))
                })?;
            if replayed != self.resources {
                return Err(AccessPlanDiagnostic(
                    "borrowed resident retirement replayed resource compatibility differs from the retained active carrier"
                        .into(),
                ));
            }
            validate_provider_content_binding(&self.plan, &self.loan, self.content).map_err(
                |diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "borrowed resident retirement could not replay the retained provider content grant: {diagnostic}"
                    ))
                },
            )?;
            validate_resident_observation(
                &self.plan,
                ObservationModel::Stable,
                "borrowed resident retirement",
            )
        }

        pub fn retire(self) -> Result<(), BorrowedResidentRetirementError<'resident>> {
            if let Err(diagnostic) = self.validate_retirement_authority() {
                return Err(BorrowedResidentRetirementError {
                    established: self,
                    diagnostic,
                });
            }
            Ok(())
        }

        #[cfg(test)]
        pub(crate) fn replace_profile_for_test(
            &mut self,
            profile: AdmittedResourceProfile,
        ) -> AdmittedResourceProfile {
            std::mem::replace(&mut self.profile, profile)
        }
    }
}

pub(crate) mod schema_correspondence {
    use crate::placements::access_admission::validate_access_admission;
    use crate::placements::access_authority::AccessAuthorityRef;
    use crate::primitive_access::field_projection::project_access_field;
    use crate::{
        AccessAdmission, AccessAdmissionId, AccessFieldKey, AccessFieldProjection,
        AccessPlanDiagnostic, AccessPlanId, AccessView, AdmittedResourceProfile, BorrowPolarity,
        ResourceProfileReceiptId, ValidatedAccessPlan, place,
    };
    use extents::ExtentLoan;

    use crate::normalized_identity;

    normalized_identity!(
        SchemaCorrespondenceProviderId,
        "schema-correspondence provider identity"
    );
    normalized_identity!(StableDeviceInstanceId, "stable device-instance identity");
    normalized_identity!(
        SchemaCorrespondenceSourceId,
        "schema-correspondence provenance-source identity"
    );
    normalized_identity!(
        RuntimeDeviceRevisionObservationId,
        "runtime device-revision observation identity"
    );
    normalized_identity!(
        DeviceRevisionPredicateId,
        "device-revision predicate identity"
    );

    #[derive(Debug)]
    #[must_use = "runtime revision evidence retains its provider/device/grant binding"]
    pub struct RuntimeDeviceRevisionEvidence {
        observation: RuntimeDeviceRevisionObservationId,
        predicate: DeviceRevisionPredicateId,
        provider: SchemaCorrespondenceProviderId,
        device: StableDeviceInstanceId,
        profile_receipt: ResourceProfileReceiptId,
        observed_revision: u64,
    }

    impl RuntimeDeviceRevisionEvidence {
        pub fn from_admitted_provider(
            observation: RuntimeDeviceRevisionObservationId,
            predicate: DeviceRevisionPredicateId,
            provider: SchemaCorrespondenceProviderId,
            device: StableDeviceInstanceId,
            profile_receipt: ResourceProfileReceiptId,
            observed_revision: u64,
        ) -> Self {
            Self {
                observation,
                predicate,
                provider,
                device,
                profile_receipt,
                observed_revision,
            }
        }

        pub const fn observation(&self) -> RuntimeDeviceRevisionObservationId {
            self.observation
        }

        pub const fn predicate(&self) -> DeviceRevisionPredicateId {
            self.predicate
        }

        pub const fn provider(&self) -> SchemaCorrespondenceProviderId {
            self.provider
        }

        pub const fn device(&self) -> StableDeviceInstanceId {
            self.device
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.profile_receipt
        }

        pub const fn observed_revision(&self) -> u64 {
            self.observed_revision
        }
    }

    #[derive(Debug)]
    #[must_use = "schema correspondence retains admitted provider/device provenance"]
    pub struct SchemaDeviceCorrespondenceGrant {
        provider: SchemaCorrespondenceProviderId,
        device: StableDeviceInstanceId,
        source: SchemaCorrespondenceSourceId,
        placement: ValidatedAccessPlan,
        profile_receipt: ResourceProfileReceiptId,
        revision: Option<RuntimeDeviceRevisionEvidence>,
    }

    impl SchemaDeviceCorrespondenceGrant {
        pub fn from_admitted_provider(
            provider: SchemaCorrespondenceProviderId,
            device: StableDeviceInstanceId,
            source: SchemaCorrespondenceSourceId,
            placement: &ValidatedAccessPlan,
            profile_receipt: ResourceProfileReceiptId,
            revision: Option<RuntimeDeviceRevisionEvidence>,
        ) -> Result<Self, SchemaDeviceCorrespondenceGrantError> {
            if revision.as_ref().is_some_and(|revision| {
                revision.provider != provider
                    || revision.device != device
                    || revision.profile_receipt != profile_receipt
            }) {
                return Err(SchemaDeviceCorrespondenceGrantError {
                    revision,
                    diagnostic: AccessPlanDiagnostic(
                        "runtime revision evidence does not bind the correspondence provider, stable device instance, and resource-profile grant"
                            .into(),
                    ),
                });
            }
            Ok(Self {
                provider,
                device,
                source,
                placement: placement.clone(),
                profile_receipt,
                revision,
            })
        }

        pub fn admit(
            self,
            placement: &ValidatedAccessPlan,
            profile: &AdmittedResourceProfile,
        ) -> Result<AdmittedSchemaDeviceCorrespondence, SchemaDeviceCorrespondenceAdmissionError>
        {
            admit_schema_device_correspondence(self, placement, profile.receipt())
        }
    }

    #[derive(Debug)]
    pub struct SchemaDeviceCorrespondenceGrantError {
        revision: Option<RuntimeDeviceRevisionEvidence>,
        diagnostic: AccessPlanDiagnostic,
    }

    impl SchemaDeviceCorrespondenceGrantError {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (Option<RuntimeDeviceRevisionEvidence>, AccessPlanDiagnostic) {
            (self.revision, self.diagnostic)
        }
    }

    #[derive(Debug)]
    #[must_use = "admitted schema correspondence retains physical provenance"]
    pub struct AdmittedSchemaDeviceCorrespondence {
        provider: SchemaCorrespondenceProviderId,
        device: StableDeviceInstanceId,
        source: SchemaCorrespondenceSourceId,
        placement: ValidatedAccessPlan,
        profile_receipt: ResourceProfileReceiptId,
        revision: Option<RuntimeDeviceRevisionEvidence>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SchemaDeviceCorrespondenceReceiptContext {
        provider: SchemaCorrespondenceProviderId,
        device: StableDeviceInstanceId,
        source: SchemaCorrespondenceSourceId,
        placement: ValidatedAccessPlan,
        profile_receipt: ResourceProfileReceiptId,
        revision: Option<RuntimeDeviceRevisionReceiptContext>,
    }

    impl SchemaDeviceCorrespondenceReceiptContext {
        pub const fn provider(&self) -> SchemaCorrespondenceProviderId {
            self.provider
        }

        pub const fn device(&self) -> StableDeviceInstanceId {
            self.device
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RuntimeDeviceRevisionReceiptContext {
        observation: RuntimeDeviceRevisionObservationId,
        predicate: DeviceRevisionPredicateId,
        provider: SchemaCorrespondenceProviderId,
        device: StableDeviceInstanceId,
        profile_receipt: ResourceProfileReceiptId,
        observed_revision: u64,
    }

    impl From<&RuntimeDeviceRevisionEvidence> for RuntimeDeviceRevisionReceiptContext {
        fn from(revision: &RuntimeDeviceRevisionEvidence) -> Self {
            Self {
                observation: revision.observation,
                predicate: revision.predicate,
                provider: revision.provider,
                device: revision.device,
                profile_receipt: revision.profile_receipt,
                observed_revision: revision.observed_revision,
            }
        }
    }

    impl AdmittedSchemaDeviceCorrespondence {
        pub const fn provider(&self) -> SchemaCorrespondenceProviderId {
            self.provider
        }

        pub const fn device(&self) -> StableDeviceInstanceId {
            self.device
        }

        pub const fn source(&self) -> SchemaCorrespondenceSourceId {
            self.source
        }

        pub const fn placement(&self) -> AccessPlanId {
            self.placement.identity()
        }

        pub const fn access_plan(&self) -> &ValidatedAccessPlan {
            &self.placement
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.profile_receipt
        }

        pub const fn revision(&self) -> Option<&RuntimeDeviceRevisionEvidence> {
            self.revision.as_ref()
        }

        pub fn receipt_context(&self) -> SchemaDeviceCorrespondenceReceiptContext {
            SchemaDeviceCorrespondenceReceiptContext {
                provider: self.provider,
                device: self.device,
                source: self.source,
                placement: self.placement.clone(),
                profile_receipt: self.profile_receipt,
                revision: self
                    .revision
                    .as_ref()
                    .map(RuntimeDeviceRevisionReceiptContext::from),
            }
        }

        pub(crate) fn validate_structure(&self) -> Result<(), AccessPlanDiagnostic> {
            if self.revision.as_ref().is_some_and(|revision| {
                revision.provider != self.provider
                    || revision.device != self.device
                    || revision.profile_receipt != self.profile_receipt
            }) {
                return Err(AccessPlanDiagnostic(
                    "admitted schema correspondence could not replay its runtime revision provider, stable device instance, and resource-profile grant"
                        .into(),
                ));
            }
            Ok(())
        }

        #[cfg(test)]
        pub(crate) fn replace_access_plan_for_test(
            &mut self,
            placement: AccessPlanId,
        ) -> AccessPlanId {
            std::mem::replace(&mut self.placement.identity, placement)
        }

        #[cfg(test)]
        pub(super) fn replace_profile_receipt_for_test(
            &mut self,
            receipt: ResourceProfileReceiptId,
        ) -> ResourceProfileReceiptId {
            std::mem::replace(&mut self.profile_receipt, receipt)
        }
    }

    #[derive(Debug)]
    pub struct SchemaDeviceCorrespondenceAdmissionError {
        grant: SchemaDeviceCorrespondenceGrant,
        diagnostic: AccessPlanDiagnostic,
    }

    impl SchemaDeviceCorrespondenceAdmissionError {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (SchemaDeviceCorrespondenceGrant, AccessPlanDiagnostic) {
            (self.grant, self.diagnostic)
        }
    }

    fn admit_schema_device_correspondence(
        grant: SchemaDeviceCorrespondenceGrant,
        placement: &ValidatedAccessPlan,
        profile_receipt: ResourceProfileReceiptId,
    ) -> Result<AdmittedSchemaDeviceCorrespondence, SchemaDeviceCorrespondenceAdmissionError> {
        if grant.revision.as_ref().is_some_and(|revision| {
            revision.provider != grant.provider
                || revision.device != grant.device
                || revision.profile_receipt != grant.profile_receipt
        }) {
            return Err(SchemaDeviceCorrespondenceAdmissionError {
                grant,
                diagnostic: AccessPlanDiagnostic(
                    "schema correspondence could not replay its runtime revision provider, stable device instance, and resource-profile grant"
                        .into(),
                ),
            });
        }
        if grant.placement != *placement {
            return Err(SchemaDeviceCorrespondenceAdmissionError {
                grant,
                diagnostic: AccessPlanDiagnostic(
                    "schema correspondence does not name the exact validated placement".into(),
                ),
            });
        }
        if grant.profile_receipt != profile_receipt {
            return Err(SchemaDeviceCorrespondenceAdmissionError {
                grant,
                diagnostic: AccessPlanDiagnostic(
                    "schema correspondence does not name the exact admitted resource-profile grant"
                        .into(),
                ),
            });
        }
        let SchemaDeviceCorrespondenceGrant {
            provider,
            device,
            source,
            placement,
            profile_receipt,
            revision,
        } = grant;
        Ok(AdmittedSchemaDeviceCorrespondence {
            provider,
            device,
            source,
            placement,
            profile_receipt,
            revision,
        })
    }

    #[derive(Debug)]
    #[must_use = "corresponded access admission retains loan and physical provenance"]
    pub struct SchemaCorrespondedAccessAdmission<'extent> {
        admission: AccessAdmission<'extent>,
        correspondence: AdmittedSchemaDeviceCorrespondence,
    }

    impl<'extent> SchemaCorrespondedAccessAdmission<'extent> {
        pub const fn admission(&self) -> AccessAdmissionId {
            self.admission.identity
        }

        pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
            &self.correspondence
        }

        pub fn into_parts(self) -> (AccessAdmission<'extent>, AdmittedSchemaDeviceCorrespondence) {
            (self.admission, self.correspondence)
        }

        pub fn withdraw(self) -> (ExtentLoan<'extent>, AdmittedSchemaDeviceCorrespondence) {
            (self.admission.withdraw(), self.correspondence)
        }

        pub fn establish_view(
            self,
        ) -> Result<
            SchemaCorrespondedAccessView<'extent>,
            SchemaCorrespondedPlaceEstablishmentError<'extent>,
        > {
            if let Err(diagnostic) =
                validate_schema_correspondence_access_binding(&self.admission, &self.correspondence)
            {
                return Err(SchemaCorrespondedPlaceEstablishmentError {
                    bound: self,
                    diagnostic,
                });
            }
            let Self {
                admission,
                correspondence,
            } = self;
            match place(admission) {
                Ok(view) => Ok(SchemaCorrespondedAccessView {
                    view,
                    correspondence,
                }),
                Err(rejection) => {
                    let (admission, diagnostic) = rejection.into_parts();
                    Err(SchemaCorrespondedPlaceEstablishmentError {
                        bound: Self {
                            admission,
                            correspondence,
                        },
                        diagnostic,
                    })
                }
            }
        }

        #[cfg(test)]
        pub(crate) fn replace_correspondence_access_plan_for_test(
            &mut self,
            placement: AccessPlanId,
        ) -> AccessPlanId {
            self.correspondence.replace_access_plan_for_test(placement)
        }
    }

    #[derive(Debug)]
    #[must_use = "corresponded access view retains loan and physical provenance"]
    pub struct SchemaCorrespondedAccessView<'extent> {
        view: AccessView<'extent>,
        correspondence: AdmittedSchemaDeviceCorrespondence,
    }

    impl<'extent> SchemaCorrespondedAccessView<'extent> {
        pub const fn admission(&self) -> AccessAdmissionId {
            self.view.admission()
        }

        pub const fn base(&self) -> u64 {
            self.view.base()
        }

        pub const fn length(&self) -> u64 {
            self.view.length()
        }

        pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
            &self.correspondence
        }

        pub fn retire(
            self,
        ) -> Result<
            (ExtentLoan<'extent>, AdmittedSchemaDeviceCorrespondence),
            SchemaCorrespondedPlaceRetirementError<'extent>,
        > {
            if let Err(diagnostic) = self.validate_retirement() {
                return Err(SchemaCorrespondedPlaceRetirementError {
                    view: self,
                    diagnostic,
                });
            }
            let Self {
                view,
                correspondence,
            } = self;
            Ok((view.loan, correspondence))
        }

        crate::placements::field_projection_accessors!('extent);

        fn project_with<'view>(
            &'view self,
            key: AccessFieldKey,
            current_borrow: BorrowPolarity,
        ) -> Result<AccessFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
            let source_loan = BorrowPolarity::from(self.view.loan.polarity());
            project_access_field(
                &self.view.plan,
                self.view.profile_receipt,
                &self.view.resources,
                self.view.admission,
                self.view.loan.base(),
                key,
                current_borrow,
                source_loan,
                None,
                AccessAuthorityRef::CorrespondedBorrowed(self),
            )
        }

        pub(super) const fn view(&self) -> &AccessView<'extent> {
            &self.view
        }

        pub(super) fn validate_correspondence(&self) -> Result<(), AccessPlanDiagnostic> {
            self.correspondence.validate_structure()?;
            if self.correspondence.placement != self.view.plan
                || self.correspondence.profile_receipt != self.view.profile_receipt
            {
                return Err(AccessPlanDiagnostic(
                    "corresponded access view could not replay its exact placement and resource-profile correspondence"
                        .into(),
                ));
            }
            Ok(())
        }

        fn validate_retirement(&self) -> Result<(), AccessPlanDiagnostic> {
            self.validate_correspondence()?;
            self.view
                .validate_authority("corresponded access-view retirement")
        }

        #[cfg(test)]
        pub(crate) fn replace_correspondence_access_plan_for_test(
            &mut self,
            placement: AccessPlanId,
        ) -> AccessPlanId {
            self.correspondence.replace_access_plan_for_test(placement)
        }

        #[cfg(test)]
        pub(crate) fn replace_view_profile_receipt_for_test(
            &mut self,
            receipt: ResourceProfileReceiptId,
        ) -> ResourceProfileReceiptId {
            std::mem::replace(&mut self.view.profile_receipt, receipt)
        }

        #[cfg(test)]
        pub(crate) fn replace_correspondence_profile_receipt_for_test(
            &mut self,
            receipt: ResourceProfileReceiptId,
        ) -> ResourceProfileReceiptId {
            self.correspondence
                .replace_profile_receipt_for_test(receipt)
        }
    }

    #[derive(Debug)]
    pub struct SchemaCorrespondedPlaceRetirementError<'extent> {
        view: SchemaCorrespondedAccessView<'extent>,
        diagnostic: AccessPlanDiagnostic,
    }

    impl<'extent> SchemaCorrespondedPlaceRetirementError<'extent> {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (SchemaCorrespondedAccessView<'extent>, AccessPlanDiagnostic) {
            (self.view, self.diagnostic)
        }
    }

    #[derive(Debug)]
    pub struct SchemaCorrespondedPlaceEstablishmentError<'extent> {
        bound: SchemaCorrespondedAccessAdmission<'extent>,
        diagnostic: AccessPlanDiagnostic,
    }

    impl<'extent> SchemaCorrespondedPlaceEstablishmentError<'extent> {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            SchemaCorrespondedAccessAdmission<'extent>,
            AccessPlanDiagnostic,
        ) {
            (self.bound, self.diagnostic)
        }
    }

    #[derive(Debug)]
    pub struct SchemaCorrespondenceAccessBindingError<'extent> {
        admission: AccessAdmission<'extent>,
        correspondence: AdmittedSchemaDeviceCorrespondence,
        diagnostic: AccessPlanDiagnostic,
    }

    impl<'extent> SchemaCorrespondenceAccessBindingError<'extent> {
        diagnostic_accessor!();

        pub fn into_parts(
            self,
        ) -> (
            AccessAdmission<'extent>,
            AdmittedSchemaDeviceCorrespondence,
            AccessPlanDiagnostic,
        ) {
            (self.admission, self.correspondence, self.diagnostic)
        }
    }

    pub fn bind_schema_correspondence_to_access_plan<'extent>(
        admission: AccessAdmission<'extent>,
        correspondence: AdmittedSchemaDeviceCorrespondence,
    ) -> Result<
        SchemaCorrespondedAccessAdmission<'extent>,
        SchemaCorrespondenceAccessBindingError<'extent>,
    > {
        let diagnostic = validate_schema_correspondence_access_binding(&admission, &correspondence);
        if let Err(diagnostic) = diagnostic {
            return Err(SchemaCorrespondenceAccessBindingError {
                admission,
                correspondence,
                diagnostic,
            });
        }
        Ok(SchemaCorrespondedAccessAdmission {
            admission,
            correspondence,
        })
    }

    fn validate_schema_correspondence_access_binding(
        admission: &AccessAdmission<'_>,
        correspondence: &AdmittedSchemaDeviceCorrespondence,
    ) -> Result<(), AccessPlanDiagnostic> {
        correspondence.validate_structure()?;
        if correspondence.placement != admission.access_plan
            || correspondence.profile_receipt != admission.profile_receipt
            || admission.profile.receipt() != admission.profile_receipt
        {
            return Err(AccessPlanDiagnostic(
                "schema correspondence does not bind the access admission's exact plan and resource-profile receipt"
                    .into(),
            ));
        }
        let replayed = validate_access_admission(
            &admission.loan,
            &admission.access_plan,
            &admission.profile,
        )
        .map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "schema correspondence could not replay the retained access admission: {diagnostic}"
            ))
        })?;
        if replayed != admission.resources {
            return Err(AccessPlanDiagnostic(
                "schema correspondence replayed resource compatibility differs from the retained access admission"
                    .into(),
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{
            DeviceRevisionPredicateId, ResourceProfileReceiptId, RuntimeDeviceRevisionEvidence,
            RuntimeDeviceRevisionObservationId, SchemaCorrespondenceProviderId,
            SchemaCorrespondenceSourceId, SchemaDeviceCorrespondenceGrant,
            SchemaDeviceCorrespondenceReceiptContext, StableDeviceInstanceId, ValidatedAccessPlan,
            admit_schema_device_correspondence,
        };
        use crate::{AccessFields, AccessPlan, BoundaryReach, validate_access_plan};
        use layout_plans::LayoutPlanReport;

        fn provider_id(identity: u64) -> SchemaCorrespondenceProviderId {
            SchemaCorrespondenceProviderId::from_normalized_identity(identity).expect("provider")
        }

        fn device_id(identity: u64) -> StableDeviceInstanceId {
            StableDeviceInstanceId::from_normalized_identity(identity).expect("device")
        }

        fn source(identity: u64) -> SchemaCorrespondenceSourceId {
            SchemaCorrespondenceSourceId::from_normalized_identity(identity).expect("source")
        }

        fn receipt_id(identity: u64) -> ResourceProfileReceiptId {
            ResourceProfileReceiptId::from_normalized_identity(identity).expect("profile receipt")
        }

        fn test_placement(schema_report_fingerprint: u64) -> ValidatedAccessPlan {
            let layout = LayoutPlanReport {
                schema_report_fingerprint,
                entries: Vec::new(),
                offsets: Some(Vec::new()),
                size: Some(0),
                align: 1,
            };
            let access =
                AccessFields::inaccessible(&layout).expect("empty inaccessible access plan");
            validate_access_plan(AccessPlan {
                layout,
                access,
                reach: BoundaryReach::default(),
            })
            .expect("empty validated placement")
        }

        fn revision(
            provider: SchemaCorrespondenceProviderId,
            device: StableDeviceInstanceId,
            receipt: ResourceProfileReceiptId,
        ) -> RuntimeDeviceRevisionEvidence {
            RuntimeDeviceRevisionEvidence::from_admitted_provider(
                RuntimeDeviceRevisionObservationId::from_normalized_identity(41)
                    .expect("observation"),
                DeviceRevisionPredicateId::from_normalized_identity(42).expect("predicate"),
                provider,
                device,
                receipt,
                0x17,
            )
        }

        #[test]
        fn correspondence_admission_retains_separate_provider_device_and_revision_evidence() {
            let provider = provider_id(7);
            let device = device_id(8);
            let receipt = receipt_id(9);
            let placement = test_placement(11);
            let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
                provider,
                device,
                source(10),
                &placement,
                receipt,
                Some(revision(provider, device, receipt)),
            )
            .expect("provider correspondence grant");

            let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
                .expect("exact placement/profile join");
            assert_eq!(admitted.provider(), provider);
            assert_eq!(admitted.device(), device);
            assert_eq!(admitted.source().normalized_identity(), 10);
            assert_eq!(admitted.access_plan(), &placement);
            assert_eq!(admitted.profile_receipt(), receipt);
            let revision = admitted.revision().expect("revision evidence");
            assert_eq!(revision.observed_revision(), 0x17);
            assert_eq!(revision.provider(), provider);
            assert_eq!(revision.device(), device);
            assert_eq!(revision.profile_receipt(), receipt);
        }

        #[test]
        fn revision_and_admission_drift_return_exact_grants_for_retry() {
            let provider = provider_id(17);
            let device = device_id(18);
            let receipt = receipt_id(19);
            let placement = test_placement(21);
            let revision = revision(provider_id(99), device, receipt);
            let revision_observation = revision.observation();
            let rejection = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
                provider,
                device,
                source(20),
                &placement,
                receipt,
                Some(revision),
            )
            .expect_err("foreign-provider revision evidence must reject");
            assert!(rejection.diagnostic().0.contains("stable device instance"));
            let (revision, _) = rejection.into_parts();
            assert_eq!(
                revision.expect("returned revision evidence").observation(),
                revision_observation
            );

            let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
                provider,
                device,
                source(20),
                &placement,
                receipt,
                None,
            )
            .expect("unconditional provider correspondence grant");
            let drifted_placement = test_placement(22);
            let rejection = admit_schema_device_correspondence(grant, &drifted_placement, receipt)
                .expect_err("placement drift must reject");
            assert!(rejection.diagnostic().0.contains("validated placement"));
            let (grant, _) = rejection.into_parts();

            let rejection = admit_schema_device_correspondence(grant, &placement, receipt_id(23))
                .expect_err("profile-grant drift must reject");
            assert!(rejection.diagnostic().0.contains("resource-profile grant"));
            let (grant, _) = rejection.into_parts();

            let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
                .expect("returned provider grant supports corrected retry");
            assert_eq!(admitted.provider(), provider);
            assert_eq!(admitted.device(), device);
            assert!(admitted.revision().is_none());
        }

        #[test]
        fn admission_replays_revision_binding_and_returns_grant_for_retry() {
            let provider = provider_id(27);
            let device = device_id(28);
            let receipt = receipt_id(29);
            let placement = test_placement(31);
            let mut grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
                provider,
                device,
                source(30),
                &placement,
                receipt,
                Some(revision(provider, device, receipt)),
            )
            .expect("provider correspondence grant");
            grant.revision.as_mut().expect("revision evidence").device = device_id(99);

            let rejection = admit_schema_device_correspondence(grant, &placement, receipt)
                .expect_err("admission must independently replay revision binding");
            assert!(rejection.diagnostic().0.contains("could not replay"));
            let (mut grant, _) = rejection.into_parts();
            grant
                .revision
                .as_mut()
                .expect("returned revision evidence")
                .device = device;

            let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
                .expect("repaired grant remains valid for retry");
            assert_eq!(admitted.device(), device);
            assert_eq!(
                admitted.revision().expect("revision evidence").device(),
                device
            );
        }

        #[test]
        fn receipt_context_compares_complete_correspondence_structure() {
            let provider = provider_id(37);
            let device = device_id(38);
            let receipt = receipt_id(39);
            let placement = test_placement(40);
            let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
                provider,
                device,
                source(41),
                &placement,
                receipt,
                Some(revision(provider, device, receipt)),
            )
            .expect("provider correspondence grant");
            let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
                .expect("exact placement/profile join");
            let context = admitted.receipt_context();

            let assert_drift = |drifted: &SchemaDeviceCorrespondenceReceiptContext| {
                assert_eq!(context.device(), drifted.device());
                assert_eq!(context.placement.identity(), drifted.placement.identity());
                assert_ne!(&context, drifted);
            };

            assert_eq!(context, context.clone());
            assert_eq!(context.provider(), provider);
            assert_eq!(context.device(), device);

            let mut provider_drift = context.clone();
            provider_drift.provider = provider_id(42);
            assert_drift(&provider_drift);

            let mut source_drift = context.clone();
            source_drift.source = source(43);
            assert_drift(&source_drift);

            let mut profile_drift = context.clone();
            profile_drift.profile_receipt = receipt_id(44);
            assert_drift(&profile_drift);

            let mut revision_absent = context.clone();
            revision_absent.revision = None;
            assert_drift(&revision_absent);

            let mut revision_observation_drift = context.clone();
            revision_observation_drift
                .revision
                .as_mut()
                .expect("revision context")
                .observation = RuntimeDeviceRevisionObservationId::from_normalized_identity(46)
                .expect("observation");
            assert_drift(&revision_observation_drift);

            let mut revision_predicate_drift = context.clone();
            revision_predicate_drift
                .revision
                .as_mut()
                .expect("revision context")
                .predicate =
                DeviceRevisionPredicateId::from_normalized_identity(47).expect("predicate");
            assert_drift(&revision_predicate_drift);

            let mut revision_provider_drift = context.clone();
            revision_provider_drift
                .revision
                .as_mut()
                .expect("revision context")
                .provider = provider_id(48);
            assert_drift(&revision_provider_drift);

            let mut revision_device_drift = context.clone();
            revision_device_drift
                .revision
                .as_mut()
                .expect("revision context")
                .device = device_id(49);
            assert_drift(&revision_device_drift);

            let mut revision_profile_drift = context.clone();
            revision_profile_drift
                .revision
                .as_mut()
                .expect("revision context")
                .profile_receipt = receipt_id(50);
            assert_drift(&revision_profile_drift);

            let mut revision_drift = context.clone();
            revision_drift
                .revision
                .as_mut()
                .expect("revision context")
                .observed_revision = 0x18;
            assert_drift(&revision_drift);

            let mut placement_drift = context.clone();
            placement_drift.placement.layout.schema_report_fingerprint = 45;
            assert_drift(&placement_drift);
        }
    }
}
