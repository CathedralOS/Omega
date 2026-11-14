//! Checks admitted backing profiles against access and relocation demands.

pub(crate) mod resource_compatibility {
    use crate::{AccessFieldKey, AccessPlanId, ResourceProfileId};
    use crate::{
        AccessPlanDiagnostic, AtomicCapability, ExternalCapability, ExternalRead,
        ExternalReadBehavior, FieldAccess, FieldRegion, ResourceRegion, ValidatedAccessPlan,
        ValidatedResourceProfile,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct BaseCongruence {
        pub(crate) modulus: u64,
        pub(crate) residue: u64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum EffectiveSupplyKind {
        Stable,
        External,
        Atomic,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct EffectiveFieldSupply {
        pub(crate) key: AccessFieldKey,
        pub(crate) field: String,
        pub(crate) offset: u64,
        pub(crate) width_bits: u16,
        pub(crate) alignment_bytes: u64,
        pub(crate) kind: EffectiveSupplyKind,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AccessResourceCompatibility {
        pub(crate) placement: AccessPlanId,
        profile: ResourceProfileId,
        pub(crate) base: BaseCongruence,
        pub(crate) fields: Vec<EffectiveFieldSupply>,
        pub(crate) field_regions: Vec<FieldRegion>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InitializationResourceCompatibility {
        placement: AccessResourceCompatibility,
        byte_length: u64,
    }

    impl InitializationResourceCompatibility {
        pub const fn placement(&self) -> &AccessResourceCompatibility {
            &self.placement
        }

        pub const fn byte_length(&self) -> u64 {
            self.byte_length
        }
    }

    pub fn validate_initialization_resources(
        plan: &ValidatedAccessPlan,
        profile: &ValidatedResourceProfile,
    ) -> Result<InitializationResourceCompatibility, AccessPlanDiagnostic> {
        let placement = validate_access_resources(plan, profile)?;
        let byte_length = plan.access.layout_size_bytes;
        require_stable_region_supply(
            profile,
            0,
            byte_length,
            false,
            true,
            &plan.reach,
            "initialization image",
        )?;
        Ok(InitializationResourceCompatibility {
            placement,
            byte_length,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn require_stable_region_supply(
        profile: &ValidatedResourceProfile,
        byte_offset: u64,
        byte_length: u64,
        read: bool,
        write: bool,
        reach: &crate::BoundaryReach,
        description: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        let end = byte_offset.checked_add(byte_length).ok_or_else(|| {
            AccessPlanDiagnostic(format!("{description} resource interval overflows"))
        })?;
        let mut covered = byte_offset;
        for region in &profile.regions {
            if covered == end {
                break;
            }
            let region_end = region.offset + region.length;
            if region_end <= covered {
                continue;
            }
            if region.offset > covered {
                return Err(AccessPlanDiagnostic(format!(
                    "{description} has no resource supply at byte {covered}"
                )));
            }
            if !region.stable.permits(read, write) {
                let operations = match (read, write) {
                    (true, true) => "read/write",
                    (true, false) => "read",
                    (false, true) => "write",
                    (false, false) => "none",
                };
                return Err(AccessPlanDiagnostic(format!(
                    "{description} requires Stable {operations} supply at byte {covered}"
                )));
            }
            if !region.reach.contains_all(reach) {
                return Err(AccessPlanDiagnostic(format!(
                    "{description} lacks the placement's complete boundary reach at byte {covered}"
                )));
            }
            covered = region_end.min(end);
        }
        if covered != end {
            return Err(AccessPlanDiagnostic(format!(
                "{description} has no resource supply at byte {covered}"
            )));
        }
        Ok(())
    }

    impl BaseCongruence {
        pub const fn modulus(self) -> u64 {
            self.modulus
        }

        pub const fn residue(self) -> u64 {
            self.residue
        }

        pub const fn admits(self, base: u64) -> bool {
            base % self.modulus == self.residue
        }
    }

    impl EffectiveFieldSupply {
        pub const fn key(&self) -> AccessFieldKey {
            self.key
        }

        pub fn field(&self) -> &str {
            &self.field
        }

        pub const fn offset(&self) -> u64 {
            self.offset
        }

        pub const fn width_bits(&self) -> u16 {
            self.width_bits
        }

        pub const fn alignment_bytes(&self) -> u64 {
            self.alignment_bytes
        }

        pub const fn kind(&self) -> EffectiveSupplyKind {
            self.kind
        }
    }

    impl AccessResourceCompatibility {
        pub const fn placement(&self) -> AccessPlanId {
            self.placement
        }

        pub const fn profile(&self) -> ResourceProfileId {
            self.profile
        }

        pub const fn base_congruence(&self) -> BaseCongruence {
            self.base
        }

        pub fn fields(&self) -> &[EffectiveFieldSupply] {
            &self.fields
        }

        pub fn field_regions(&self) -> &[FieldRegion] {
            &self.field_regions
        }
    }

    pub fn validate_access_resources(
        plan: &ValidatedAccessPlan,
        profile: &ValidatedResourceProfile,
    ) -> Result<AccessResourceCompatibility, AccessPlanDiagnostic> {
        if plan.access.layout_size_bytes > profile.length {
            return Err(AccessPlanDiagnostic(format!(
                "{}-byte placed layout exceeds {}-byte resource profile",
                plan.access.layout_size_bytes, profile.length
            )));
        }
        let mut congruence = CongruenceAccumulator {
            value: BaseCongruence {
                modulus: 1,
                residue: 0,
            },
            source: "unconstrained base".into(),
        };
        require_base_congruence(&mut congruence, "layout base", 0, plan.layout.align)?;

        let mut fields = Vec::with_capacity(plan.access.fields.len());
        for descriptor in &plan.access.fields {
            let width_bytes = u64::from(descriptor.transfer_width_bits / 8);
            let end = descriptor
                .container_byte_offset
                .checked_add(width_bytes)
                .ok_or_else(|| {
                    AccessPlanDiagnostic(format!(
                        "field `{}` resource interval overflows",
                        descriptor.field
                    ))
                })?;
            let region = profile
                .regions
                .iter()
                .find(|region| {
                    descriptor.container_byte_offset >= region.offset
                        && end <= region.offset + region.length
                })
                .ok_or_else(|| {
                    AccessPlanDiagnostic(format!(
                        "field `{}` transfer at {}..{end} is not covered by one resource region",
                        descriptor.field, descriptor.container_byte_offset
                    ))
                })?;
            if !region.reach.contains_all(&plan.reach) {
                return Err(AccessPlanDiagnostic(format!(
                    "resource region covering field `{}` does not supply the placement's complete boundary reach",
                    descriptor.field
                )));
            }
            let access = plan
                .access
                .field(descriptor.key)
                .expect("validated descriptor must retain its source access decision")
                .access();
            let (kind, alignment_bytes) =
                select_effective_supply(&descriptor.field, access, region)?;
            require_base_congruence(
                &mut congruence,
                descriptor.field.as_str(),
                descriptor.container_byte_offset,
                alignment_bytes,
            )?;
            fields.push(EffectiveFieldSupply {
                key: descriptor.key,
                field: descriptor.field.clone(),
                offset: descriptor.container_byte_offset,
                width_bits: descriptor.transfer_width_bits,
                alignment_bytes,
                kind,
            });
        }
        for region in &plan.access.field_regions {
            if region.observation == crate::ObservationModel::Stable {
                require_stable_region_supply(
                    profile,
                    region.byte_offset,
                    region.byte_length,
                    region.permissions.read,
                    region.permissions.write,
                    &plan.reach,
                    &format!("stable structural field `{}`", region.field),
                )?;
            } else {
                let width = u64::from(region.transfer_width_bits / 8);
                let access = region.scalar_access();
                let mut offset = region.byte_offset;
                let end = offset + region.byte_length;
                while offset < end {
                    let supply = profile.regions.iter().find(|supply| {
                        supply.offset <= offset && offset + width <= supply.offset + supply.length
                    }).ok_or_else(|| AccessPlanDiagnostic(format!(
                        "structural field `{}` transfer at {offset} lacks one complete resource region", region.field
                    )))?;
                    if !supply.reach.contains_all(&plan.reach) {
                        return Err(AccessPlanDiagnostic(format!(
                            "structural field `{}` lacks boundary reach",
                            region.field
                        )));
                    }
                    let (_, alignment) = select_effective_supply(&region.field, &access, supply)?;
                    require_base_congruence(&mut congruence, &region.field, offset, alignment)?;
                    let covered_end = end.min(supply.offset + supply.length);
                    if (covered_end - offset) % width != 0 {
                        return Err(AccessPlanDiagnostic(format!(
                            "structural field `{}` transfer straddles a resource boundary",
                            region.field
                        )));
                    }
                    if covered_end - offset > width && width % alignment != 0 {
                        return Err(AccessPlanDiagnostic(format!(
                            "structural field `{}` transfer stride violates supplied alignment",
                            region.field
                        )));
                    }
                    offset = covered_end;
                }
            }
            require_base_congruence(
                &mut congruence,
                &region.field,
                region.byte_offset,
                region.byte_alignment,
            )?;
        }
        Ok(AccessResourceCompatibility {
            placement: plan.identity,
            profile: profile.identity,
            base: congruence.value,
            fields,
            field_regions: plan.access.field_regions.clone(),
        })
    }

    fn select_effective_supply(
        field: &str,
        access: &FieldAccess,
        region: &ResourceRegion,
    ) -> Result<(EffectiveSupplyKind, u64), AccessPlanDiagnostic> {
        match access {
            FieldAccess::Region { .. } => Err(AccessPlanDiagnostic(
                "structural region supply cannot be selected as one primitive transfer".into(),
            )),
            FieldAccess::Inaccessible => {
                unreachable!("inaccessible fields do not have validated descriptors")
            }
            FieldAccess::Stable {
                transfer_width_bits,
                read,
                write,
                ..
            } => {
                if !region.stable.permits(*read, *write) {
                    return Err(AccessPlanDiagnostic(format!(
                        "field `{field}` requests Stable read={read} write={write}, but its resource region does not supply them"
                    )));
                }
                Ok((
                    EffectiveSupplyKind::Stable,
                    stable_transfer_alignment(*transfer_width_bits),
                ))
            }
            FieldAccess::External {
                transfer_width_bits,
                read,
                write,
                ..
            } => {
                if let ExternalCapability::Access {
                    read: supplied_read,
                    write: supplied_write,
                    transfers,
                } = &region.external
                    && external_read_compatible(*read, *supplied_read)
                    && (!*write || *supplied_write)
                    && let Some(rule) = transfers
                        .iter()
                        .find(|rule| rule.width_bits == *transfer_width_bits)
                {
                    return Ok((EffectiveSupplyKind::External, rule.alignment_bytes));
                }
                let stable_read = *read == ExternalRead::Read;
                if *read != ExternalRead::Take && region.stable.permits(stable_read, *write) {
                    return Ok((
                        EffectiveSupplyKind::Stable,
                        stable_transfer_alignment(*transfer_width_bits),
                    ));
                }
                Err(AccessPlanDiagnostic(format!(
                    "field `{field}` requests incompatible External {transfer_width_bits}-bit read={read:?} write={write}"
                )))
            }
            FieldAccess::Atomic {
                transfer_width_bits,
                operations,
                ..
            } => {
                let AtomicCapability::Access { transfers } = &region.atomic else {
                    return Err(AccessPlanDiagnostic(format!(
                        "field `{field}` requests Atomic access, but its resource region supplies none"
                    )));
                };
                let rule = transfers
                    .iter()
                    .find(|rule| {
                        rule.transfer.width_bits == *transfer_width_bits
                            && rule.operations.contains(*operations)
                    })
                    .ok_or_else(|| {
                        AccessPlanDiagnostic(format!(
                            "field `{field}` requests unsupported Atomic {transfer_width_bits}-bit operation families"
                        ))
                    })?;
                Ok((EffectiveSupplyKind::Atomic, rule.transfer.alignment_bytes))
            }
        }
    }

    const fn external_read_compatible(demand: ExternalRead, supply: ExternalReadBehavior) -> bool {
        match demand {
            ExternalRead::None => true,
            ExternalRead::Read => matches!(supply, ExternalReadBehavior::Repeatable),
            ExternalRead::Take => matches!(supply, ExternalReadBehavior::Destructive),
        }
    }

    const fn stable_transfer_alignment(width_bits: u16) -> u64 {
        let width_bytes = width_bits / 8;
        if width_bytes.is_power_of_two() {
            width_bytes as u64
        } else {
            1
        }
    }

    struct CongruenceAccumulator {
        value: BaseCongruence,
        source: String,
    }

    fn require_base_congruence(
        accumulated: &mut CongruenceAccumulator,
        source: &str,
        offset: u64,
        alignment: u64,
    ) -> Result<(), AccessPlanDiagnostic> {
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(AccessPlanDiagnostic(format!(
                "field `{source}` requires invalid transfer alignment {alignment}"
            )));
        }
        let required = BaseCongruence {
            modulus: alignment,
            residue: (alignment - offset % alignment) % alignment,
        };
        let shared_modulus = accumulated.value.modulus.min(required.modulus);
        if accumulated.value.residue % shared_modulus != required.residue % shared_modulus {
            return Err(AccessPlanDiagnostic(format!(
                "field `{source}` at offset {offset} with {alignment}-byte transfer alignment conflicts with {} (base mod {} = {}, required base mod {alignment} = {})",
                accumulated.source,
                accumulated.value.modulus,
                accumulated.value.residue,
                required.residue
            )));
        }
        if required.modulus > accumulated.value.modulus {
            accumulated.value = required;
            accumulated.source = format!(
                "field `{source}` at offset {offset} with {alignment}-byte transfer alignment"
            );
        }
        Ok(())
    }
}

pub(crate) mod resource_profile {
    use crate::validate_resource_profile;
    use crate::{AccessPlanDiagnostic, AtomicPermissions, BoundaryReach};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StableCapability {
        None,
        Read,
        Write,
        ReadWrite,
    }

    impl StableCapability {
        pub(crate) const fn permits(self, read: bool, write: bool) -> bool {
            (!read || matches!(self, Self::Read | Self::ReadWrite))
                && (!write || matches!(self, Self::Write | Self::ReadWrite))
        }

        pub(crate) const fn any(self) -> bool {
            !matches!(self, Self::None)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ExternalReadBehavior {
        None,
        Repeatable,
        Destructive,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum PeerWritability {
        Exclusive,
        HostileShared,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TransferRule {
        pub width_bits: u16,
        pub alignment_bytes: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum ExternalCapability {
        None,
        Access {
            read: ExternalReadBehavior,
            write: bool,
            transfers: Vec<TransferRule>,
        },
    }

    impl ExternalCapability {
        pub(crate) fn any(&self) -> bool {
            !matches!(self, Self::None)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct AtomicTransferRule {
        pub transfer: TransferRule,
        pub operations: AtomicPermissions,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum AtomicCapability {
        None,
        Access { transfers: Vec<AtomicTransferRule> },
    }

    impl AtomicCapability {
        pub(crate) fn any(&self) -> bool {
            !matches!(self, Self::None)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct ResourceRegion {
        pub offset: u64,
        pub length: u64,
        pub peer: PeerWritability,
        pub stable: StableCapability,
        pub external: ExternalCapability,
        pub atomic: AtomicCapability,
        pub reach: BoundaryReach,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
    pub struct ResourceProfile {
        pub regions: Vec<ResourceRegion>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ResourceProfileId(pub(crate) u64);

    impl ResourceProfileId {
        pub const fn compatibility_fingerprint(self) -> u64 {
            self.0
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedResourceProfile {
        pub(crate) identity: ResourceProfileId,
        pub(crate) length: u64,
        pub(crate) regions: Vec<ResourceRegion>,
    }

    impl ValidatedResourceProfile {
        pub const fn identity(&self) -> ResourceProfileId {
            self.identity
        }

        pub const fn length(&self) -> u64 {
            self.length
        }

        pub fn regions(&self) -> &[ResourceRegion] {
            &self.regions
        }

        pub fn restrict(
            &self,
            offset: u64,
            length: u64,
            permitted_reach: &BoundaryReach,
        ) -> Result<Self, AccessPlanDiagnostic> {
            let end = offset.checked_add(length).ok_or_else(|| {
                AccessPlanDiagnostic("resource-profile restriction range overflows".into())
            })?;
            if end > self.length {
                return Err(AccessPlanDiagnostic(format!(
                    "resource-profile restriction {offset}..{end} exceeds {}-byte parent",
                    self.length
                )));
            }
            let mut regions = Vec::new();
            for region in &self.regions {
                let region_end = region.offset + region.length;
                let start = region.offset.max(offset);
                let retained_end = region_end.min(end);
                if start >= retained_end {
                    continue;
                }
                regions.push(ResourceRegion {
                    offset: start - offset,
                    length: retained_end - start,
                    peer: region.peer,
                    stable: region.stable,
                    external: region.external.clone(),
                    atomic: region.atomic.clone(),
                    reach: region.reach.intersection(permitted_reach),
                });
            }
            validate_resource_profile(ResourceProfile { regions }, length)
        }
    }
}

pub(crate) mod resource_profile_admission {
    use extents::{
        AddressSpaceId, Extent, ExtentLineageId, ExtentLoan, ExtentProvenanceId, ExtentRights,
        ExtentRootOrigin, MappingEraId,
    };

    use crate::access_fields::diagnostic::reject_when;
    use crate::normalized_identity;
    use crate::{
        AccessPlanDiagnostic, BoundaryReach, ResourceProfile, ValidatedResourceProfile,
        validate_resource_profile,
    };

    normalized_identity!(
        ResourceProfileReceiptId,
        "resource-profile receipt identity cannot be zero"
    );

    #[derive(Debug)]
    pub struct ResourceProfileGrant {
        receipt: ResourceProfileReceiptId,
        base: u64,
        length: u64,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        origin: ExtentRootOrigin,
        lineage_root: ExtentLineageId,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    }

    impl ResourceProfileGrant {
        pub fn from_admitted_provider(
            receipt: ResourceProfileReceiptId,
            extent: &Extent,
            required_rights: ExtentRights,
            permitted_reach: BoundaryReach,
        ) -> Result<Self, AccessPlanDiagnostic> {
            Self::from_bound_extent(
                receipt,
                extent.base(),
                extent.length(),
                extent.address_space(),
                extent.provenance(),
                extent.era(),
                extent.origin(),
                extent.lineage_root(),
                required_rights,
                permitted_reach,
            )
        }

        pub fn from_admitted_provider_loan(
            receipt: ResourceProfileReceiptId,
            loan: &ExtentLoan<'_>,
            required_rights: ExtentRights,
            permitted_reach: BoundaryReach,
        ) -> Result<Self, AccessPlanDiagnostic> {
            Self::from_bound_extent(
                receipt,
                loan.base(),
                loan.length(),
                loan.address_space(),
                loan.provenance(),
                loan.era(),
                loan.origin(),
                loan.lineage_root(),
                required_rights,
                permitted_reach,
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn from_bound_extent(
            receipt: ResourceProfileReceiptId,
            base: u64,
            length: u64,
            address_space: AddressSpaceId,
            provenance: ExtentProvenanceId,
            era: MappingEraId,
            origin: ExtentRootOrigin,
            lineage_root: ExtentLineageId,
            required_rights: ExtentRights,
            permitted_reach: BoundaryReach,
        ) -> Result<Self, AccessPlanDiagnostic> {
            base.checked_add(length).ok_or_else(|| {
                AccessPlanDiagnostic("resource-profile grant range overflows".into())
            })?;
            Ok(Self {
                receipt,
                base,
                length,
                address_space,
                provenance,
                era,
                origin,
                lineage_root,
                required_rights,
                permitted_reach,
            })
        }

        pub fn admit(
            self,
            profile: ResourceProfile,
        ) -> Result<AdmittedResourceProfile, ResourceProfileAdmissionError> {
            let validated = match validate_resource_profile(profile.clone(), self.length) {
                Ok(validated) => validated,
                Err(diagnostic) => {
                    return Err(ResourceProfileAdmissionError {
                        grant: Box::new(self),
                        profile,
                        diagnostic,
                    });
                }
            };
            if let Some(region) = validated
                .regions
                .iter()
                .find(|region| !self.permitted_reach.contains_all(&region.reach))
            {
                return Err(ResourceProfileAdmissionError {
                    grant: Box::new(self),
                    profile,
                    diagnostic: AccessPlanDiagnostic(format!(
                        "resource region {}..{} claims reach outside the provider grant",
                        region.offset,
                        region.offset + region.length
                    )),
                });
            }
            Ok(AdmittedResourceProfile {
                receipt: self.receipt,
                base: self.base,
                length: self.length,
                address_space: self.address_space,
                provenance: self.provenance,
                era: self.era,
                origin: self.origin,
                lineage_root: self.lineage_root,
                required_rights: self.required_rights,
                permitted_reach: self.permitted_reach,
                profile: validated,
            })
        }
    }

    #[derive(Debug)]
    pub struct ResourceProfileAdmissionError {
        grant: Box<ResourceProfileGrant>,
        profile: ResourceProfile,
        diagnostic: AccessPlanDiagnostic,
    }

    impl ResourceProfileAdmissionError {
        diagnostic_accessor!();

        pub fn into_parts(self) -> (ResourceProfileGrant, ResourceProfile, AccessPlanDiagnostic) {
            (*self.grant, self.profile, self.diagnostic)
        }
    }

    #[derive(Debug, Clone)]
    pub struct AdmittedResourceProfile {
        receipt: ResourceProfileReceiptId,
        base: u64,
        length: u64,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        origin: ExtentRootOrigin,
        lineage_root: ExtentLineageId,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
        profile: ValidatedResourceProfile,
    }

    impl AdmittedResourceProfile {
        pub const fn receipt(&self) -> ResourceProfileReceiptId {
            self.receipt
        }

        pub const fn profile(&self) -> &ValidatedResourceProfile {
            &self.profile
        }

        pub fn initialization_compatibility(
            &self,
            loan: &ExtentLoan<'_>,
            plan: &crate::ValidatedAccessPlan,
        ) -> Result<crate::InitializationResourceCompatibility, AccessPlanDiagnostic> {
            if loan.polarity() != extents::LoanPolarity::Exclusive {
                return Err(AccessPlanDiagnostic(
                    "initialization requires an exclusive extent loan".into(),
                ));
            }
            let restricted = self.restrict_to_loan(loan)?;
            let compatibility = crate::validate_initialization_resources(plan, &restricted)?;
            let base = compatibility.placement().base_congruence();
            if !base.admits(loan.base()) {
                return Err(AccessPlanDiagnostic(format!(
                    "initialization base {} must satisfy base mod {} = {}",
                    loan.base(),
                    base.modulus(),
                    base.residue()
                )));
            }
            Ok(compatibility)
        }

        pub fn restrict_to_loan(
            &self,
            loan: &ExtentLoan<'_>,
        ) -> Result<ValidatedResourceProfile, AccessPlanDiagnostic> {
            reject_when! {
                loan.address_space() != self.address_space
                    => "extent address space does not match admitted resource profile",
                loan.provenance() != self.provenance
                    => "extent provenance does not match admitted resource profile",
                loan.era() != self.era
                    => "extent mapping era does not match admitted resource profile",
                loan.origin() != self.origin
                    => "extent sealed root origin does not match admitted resource profile",
                loan.lineage_root() != self.lineage_root
                    => "extent root lineage does not match admitted resource profile",
                !loan.rights().contains(&self.required_rights)
                    => "extent lacks rights bound into the admitted resource profile",
            }
            let offset = loan.base().checked_sub(self.base).ok_or_else(|| {
                AccessPlanDiagnostic(
                    "extent loan begins before the admitted resource-profile range".into(),
                )
            })?;
            let end = offset.checked_add(loan.length()).ok_or_else(|| {
                AccessPlanDiagnostic("extent loan range overflows resource profile".into())
            })?;
            if end > self.length {
                return Err(AccessPlanDiagnostic(format!(
                    "extent loan relative range {offset}..{end} exceeds {}-byte admitted resource profile",
                    self.length
                )));
            }
            self.profile
                .restrict(offset, loan.length(), &self.permitted_reach)
        }
    }
}

pub(crate) mod resource_profile_validation {
    use crate::access_fields::diagnostic::reject_when;
    use crate::plan_policy::normalized_identities::non_authoritative_resource_profile_compatibility_fingerprint;
    use crate::{
        AccessPlanDiagnostic, AtomicCapability, ExternalCapability, ExternalReadBehavior,
        PeerWritability, ResourceProfile, ResourceRegion, TransferRule, ValidatedResourceProfile,
    };

    pub fn validate_resource_profile(
        mut profile: ResourceProfile,
        length: u64,
    ) -> Result<ValidatedResourceProfile, AccessPlanDiagnostic> {
        profile
            .regions
            .sort_by_key(|region| (region.offset, region.length));
        let mut normalized: Vec<ResourceRegion> = Vec::with_capacity(profile.regions.len());
        for mut region in profile.regions {
            reject_when! { region.length == 0 => "resource-profile region cannot be empty" }
            let end = region.offset.checked_add(region.length).ok_or_else(|| {
                AccessPlanDiagnostic("resource-profile region range overflows".into())
            })?;
            if end > length {
                return Err(AccessPlanDiagnostic(format!(
                    "resource-profile region {}..{end} exceeds {length}-byte profile",
                    region.offset
                )));
            }
            normalize_external_capability(&mut region.external)?;
            normalize_atomic_capability(&mut region.atomic)?;
            if region.peer == PeerWritability::HostileShared && region.stable.any() {
                return Err(AccessPlanDiagnostic(format!(
                    "resource-profile region {}..{end} supplies stable access over hostile-shared memory",
                    region.offset
                )));
            }
            if !region.stable.any() && !region.external.any() && !region.atomic.any() {
                return Err(AccessPlanDiagnostic(format!(
                    "resource-profile region {}..{end} supplies no operation",
                    region.offset
                )));
            }
            if let Some(previous) = normalized.last_mut() {
                let previous_end = previous.offset + previous.length;
                if region.offset < previous_end {
                    return Err(AccessPlanDiagnostic(format!(
                        "resource-profile regions {}..{} and {}..{end} overlap",
                        previous.offset, previous_end, region.offset
                    )));
                }
                if region.offset == previous_end
                    && previous.peer == region.peer
                    && previous.stable == region.stable
                    && previous.external == region.external
                    && previous.atomic == region.atomic
                    && previous.reach == region.reach
                {
                    previous.length =
                        previous.length.checked_add(region.length).ok_or_else(|| {
                            AccessPlanDiagnostic(
                                "merged resource-profile region length overflows".into(),
                            )
                        })?;
                    continue;
                }
            }
            normalized.push(region);
        }
        let identity =
            non_authoritative_resource_profile_compatibility_fingerprint(length, &normalized);
        Ok(ValidatedResourceProfile {
            identity,
            length,
            regions: normalized,
        })
    }

    fn normalize_external_capability(
        capability: &mut ExternalCapability,
    ) -> Result<(), AccessPlanDiagnostic> {
        let ExternalCapability::Access {
            read,
            write,
            transfers,
        } = capability
        else {
            return Ok(());
        };
        if *read == ExternalReadBehavior::None && !*write {
            return Err(AccessPlanDiagnostic(
                "external capability supplies no operation; use None".into(),
            ));
        }
        normalize_transfer_rules(transfers)?;
        reject_when! {
            transfers.is_empty() => "external capability must list at least one transfer rule",
        }
        Ok(())
    }

    fn normalize_atomic_capability(
        capability: &mut AtomicCapability,
    ) -> Result<(), AccessPlanDiagnostic> {
        let AtomicCapability::Access { transfers } = capability else {
            return Ok(());
        };
        transfers.sort_by_key(|rule| rule.transfer.width_bits);
        let mut prior_width = None;
        for rule in transfers.iter() {
            validate_transfer_rule(rule.transfer)?;
            if !rule.operations.any() {
                return Err(AccessPlanDiagnostic(format!(
                    "atomic {}-bit transfer supplies no operation",
                    rule.transfer.width_bits
                )));
            }
            if prior_width.replace(rule.transfer.width_bits) == Some(rule.transfer.width_bits) {
                return Err(AccessPlanDiagnostic(format!(
                    "atomic capability repeats {}-bit transfer width",
                    rule.transfer.width_bits
                )));
            }
        }
        reject_when! {
            transfers.is_empty() => "atomic capability must list at least one transfer rule",
        }
        Ok(())
    }

    fn normalize_transfer_rules(
        transfers: &mut [TransferRule],
    ) -> Result<(), AccessPlanDiagnostic> {
        transfers.sort_by_key(|rule| rule.width_bits);
        let mut prior_width = None;
        for rule in transfers.iter().copied() {
            validate_transfer_rule(rule)?;
            if prior_width.replace(rule.width_bits) == Some(rule.width_bits) {
                return Err(AccessPlanDiagnostic(format!(
                    "external capability repeats {}-bit transfer width",
                    rule.width_bits
                )));
            }
        }
        Ok(())
    }

    fn validate_transfer_rule(rule: TransferRule) -> Result<(), AccessPlanDiagnostic> {
        if rule.width_bits == 0 || rule.width_bits > 128 || !rule.width_bits.is_multiple_of(8) {
            return Err(AccessPlanDiagnostic(format!(
                "resource transfer width {} is not a supported whole-byte width in 8..=128",
                rule.width_bits
            )));
        }
        if rule.alignment_bytes == 0 || !rule.alignment_bytes.is_power_of_two() {
            return Err(AccessPlanDiagnostic(format!(
                "resource transfer alignment {} is not a positive power of two",
                rule.alignment_bytes
            )));
        }
        Ok(())
    }
}

pub(crate) mod run_relocation {
    use crate::{
        AccessPlanDiagnostic, InitializationResourceCompatibility, ValidatedAccessPlan,
        ValidatedResourceProfile, require_stable_region_supply, validate_initialization_resources,
    };

    pub fn validate_run_relocation_resources(
        plan: &ValidatedAccessPlan,
        count: u64,
        destination_base: u64,
        source: &ValidatedResourceProfile,
        destination: &ValidatedResourceProfile,
    ) -> Result<Option<InitializationResourceCompatibility>, AccessPlanDiagnostic> {
        let footprint = plan.layout().size.ok_or_else(|| {
            AccessPlanDiagnostic("run relocation requires a fixed cell footprint".into())
        })?;
        let occupied = count.checked_mul(footprint).ok_or_else(|| {
            AccessPlanDiagnostic("run relocation occupied byte length overflows".into())
        })?;
        if source.length() != occupied || destination.length() != occupied {
            return Err(AccessPlanDiagnostic(
                "run relocation requires two exact occupied-length windows".into(),
            ));
        }
        destination_base.checked_add(occupied).ok_or_else(|| {
            AccessPlanDiagnostic("run relocation destination address range overflows".into())
        })?;
        require_stable_region_supply(
            source,
            0,
            occupied,
            true,
            false,
            plan.reach(),
            "run relocation source",
        )?;
        require_stable_region_supply(
            destination,
            0,
            occupied,
            false,
            true,
            plan.reach(),
            "run relocation destination",
        )?;
        if count == 0 {
            return Ok(None);
        }

        let mut cells = vec![0];
        if count > 1 {
            cells.push(1);
            cells.push(count - 1);
        }
        if let Some(footprint) = std::num::NonZeroU64::new(footprint) {
            for region in destination.regions() {
                for boundary in [region.offset, region.offset + region.length] {
                    let cell = boundary / footprint;
                    if cell > 0 && cell - 1 < count {
                        cells.push(cell - 1);
                    }
                    if cell < count {
                        cells.push(cell);
                    }
                    for distance in [1, 2] {
                        if let Some(next) = cell.checked_add(distance).filter(|next| *next < count)
                        {
                            cells.push(next);
                        }
                    }
                }
            }
        } else {
            cells.truncate(1);
        }
        cells.sort_unstable();
        cells.dedup();
        let mut first = None;
        for cell in cells {
            let offset = cell * footprint;
            let profile = destination.restrict(offset, footprint, plan.reach())?;
            let compatibility = validate_initialization_resources(plan, &profile)?;
            if !compatibility
                .placement()
                .base_congruence()
                .admits(destination_base + offset)
            {
                return Err(AccessPlanDiagnostic(format!(
                    "run relocation destination cell {cell} violates placement alignment",
                )));
            }
            if cell == 0 {
                first = Some(compatibility);
            }
        }
        Ok(first)
    }
}
