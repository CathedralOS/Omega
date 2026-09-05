use std::collections::{BTreeMap, BTreeSet};

use effects::ComponentEraEntryLedger;
use extents::{
    AddressSpaceId, Extent, ExtentLineageId, ExtentProgramLocalOrigin, ExtentProvenanceId,
    ExtentRights, ExtentRootGrant, MappingEraId, ValidatedExtentGeometry,
};
use semantic_vocabulary::ContentAlgebraKind;

use super::{
    EstablishedProgramLocalRoot, EstablishedProgramLocalRootCapacity, ExternalRootDiagnostic,
    ProgramLocalRootInstallationLedger, RetiredProgramLocalRootOccurrence,
};

/// Installation-checked runtime facts required to realize one interval account
/// as one concrete Extent. These facts describe the runtime address-space
/// occurrence; they are not authority and cannot replace the established root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalExtentMaterializationPlan {
    carrier_identity: String,
    qualification_identity: String,
    algebra_parameter: String,
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    mapping_era: MappingEraId,
}

impl ProgramLocalExtentMaterializationPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        carrier_identity: impl Into<String>,
        qualification_identity: impl Into<String>,
        algebra_parameter: impl Into<String>,
        base: u64,
        length: u64,
        address_space: AddressSpaceId,
        rights: ExtentRights,
        provenance: ExtentProvenanceId,
        mapping_era: MappingEraId,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let carrier_identity = carrier_identity.into();
        let qualification_identity = qualification_identity.into();
        let algebra_parameter = algebra_parameter.into();
        if carrier_identity.is_empty()
            || qualification_identity.is_empty()
            || algebra_parameter.is_empty()
        {
            return Err(ExternalRootDiagnostic(
                "program-local Extent materialization identities cannot be empty".into(),
            ));
        }
        ValidatedExtentGeometry::check(base, length).map_err(|diagnostic| {
            ExternalRootDiagnostic(format!(
                "program-local Extent materialization geometry is invalid: {diagnostic}"
            ))
        })?;
        Ok(Self {
            carrier_identity,
            qualification_identity,
            algebra_parameter,
            base,
            length,
            address_space,
            rights,
            provenance,
            mapping_era,
        })
    }

    pub fn carrier_identity(&self) -> &str {
        &self.carrier_identity
    }

    pub fn qualification_identity(&self) -> &str {
        &self.qualification_identity
    }

    pub fn algebra_parameter(&self) -> &str {
        &self.algebra_parameter
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn address_space(&self) -> AddressSpaceId {
        self.address_space
    }

    pub const fn rights(&self) -> &ExtentRights {
        &self.rights
    }

    pub const fn provenance(&self) -> ExtentProvenanceId {
        self.provenance
    }

    pub const fn mapping_era(&self) -> MappingEraId {
        self.mapping_era
    }
}

#[derive(Debug)]
struct HeldProgramLocalExtent<'root, 'code> {
    root: EstablishedProgramLocalRoot<'root, 'code>,
    lineage: ExtentLineageId,
    plan: ProgramLocalExtentMaterializationPlan,
}

/// Epoch/installation owner for exact program-local Extent accounts.
///
/// Extents carry only passive origin identity. This registry retains the
/// actual installed occurrence and lifecycle lease while any split descendant
/// may remain live. Dropping a registry does not retire its accounts; it drops
/// the Rust carrier while the underlying lifecycle ledger remains held, which
/// fails closed by preventing quiescence.
#[derive(Debug)]
pub struct ProgramLocalExtentRegistry<'root, 'code> {
    held: BTreeMap<ExtentProgramLocalOrigin, HeldProgramLocalExtent<'root, 'code>>,
    next_lineage: u64,
}

impl<'root, 'code> Default for ProgramLocalExtentRegistry<'root, 'code> {
    fn default() -> Self {
        Self {
            held: BTreeMap::new(),
            next_lineage: 1,
        }
    }
}

impl<'root, 'code> ProgramLocalExtentRegistry<'root, 'code> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn held_accounts(&self) -> usize {
        self.held.len()
    }

    /// Atomically materialize a batch. Every account and plan is validated
    /// before any passive Extent grant is minted or any account is retained.
    pub fn materialize_batch(
        &mut self,
        inputs: Vec<(
            EstablishedProgramLocalRoot<'root, 'code>,
            ProgramLocalExtentMaterializationPlan,
        )>,
    ) -> Result<Vec<Extent>, Box<ProgramLocalExtentMaterializationError<'root, 'code>>> {
        let mut origins = BTreeSet::new();
        for (root, plan) in &inputs {
            let origin = match exact_origin(root) {
                Ok(origin) => origin,
                Err(diagnostic) => {
                    return Err(Box::new(ProgramLocalExtentMaterializationError {
                        inputs,
                        diagnostic,
                    }));
                }
            };
            if self.held.contains_key(&origin) || !origins.insert(origin) {
                return Err(Box::new(ProgramLocalExtentMaterializationError {
                    inputs,
                    diagnostic: ExternalRootDiagnostic(
                        "program-local Extent batch repeats an exact established occurrence".into(),
                    ),
                }));
            }
            if let Err(diagnostic) = validate_materialization(root, plan) {
                return Err(Box::new(ProgramLocalExtentMaterializationError {
                    inputs,
                    diagnostic,
                }));
            }
        }

        let count = match u64::try_from(inputs.len()) {
            Ok(count) => count,
            Err(_) => {
                return Err(Box::new(ProgramLocalExtentMaterializationError {
                    inputs,
                    diagnostic: ExternalRootDiagnostic(
                        "program-local Extent batch cardinality does not fit its lineage space"
                            .into(),
                    ),
                }));
            }
        };
        let Some(next_lineage) = self.next_lineage.checked_add(count) else {
            return Err(Box::new(ProgramLocalExtentMaterializationError {
                inputs,
                diagnostic: ExternalRootDiagnostic(
                    "program-local Extent lineage space is exhausted".into(),
                ),
            }));
        };

        let first_lineage = self.next_lineage;
        self.next_lineage = next_lineage;
        let mut extents = Vec::with_capacity(inputs.len());
        for (offset, (root, plan)) in inputs.into_iter().enumerate() {
            let origin = exact_origin(&root)
                .expect("validated established program-local origin remains exact");
            let lineage = ExtentLineageId::from_normalized_identity(
                first_lineage + u64::try_from(offset).expect("batch offset fits u64"),
            )
            .expect("reserved program-local lineage identities are nonzero");
            let geometry = ValidatedExtentGeometry::check(plan.base, plan.length)
                .expect("validated program-local Extent geometry remains valid");
            let extent = ExtentRootGrant::from_established_program_local(
                origin,
                lineage,
                plan.address_space,
                plan.rights.clone(),
                plan.provenance,
                plan.mapping_era,
            )
            .mint_validated(geometry);
            let previous = self.held.insert(
                origin,
                HeldProgramLocalExtent {
                    root,
                    lineage,
                    plan,
                },
            );
            debug_assert!(previous.is_none());
            extents.push(extent);
        }
        Ok(extents)
    }

    pub fn materialize(
        &mut self,
        root: EstablishedProgramLocalRoot<'root, 'code>,
        plan: ProgramLocalExtentMaterializationPlan,
    ) -> Result<Extent, Box<ProgramLocalExtentMaterializationError<'root, 'code>>> {
        let [extent]: [Extent; 1] = self
            .materialize_batch(vec![(root, plan)])?
            .try_into()
            .expect("one program-local input materializes one Extent");
        Ok(extent)
    }

    /// Consume the exact recombined root Extent and release its retained
    /// installed occurrence. Split descendants and substituted runtime facts
    /// reject without removing the held account.
    pub fn retire(
        &mut self,
        extent: Extent,
        installation: &mut ProgramLocalRootInstallationLedger,
        lifecycle: &mut ComponentEraEntryLedger,
    ) -> Result<RetiredProgramLocalRootOccurrence, Box<ProgramLocalExtentRetirementError>> {
        let Some(origin) = extent.program_local_origin() else {
            return Err(Box::new(ProgramLocalExtentRetirementError::new(
                extent,
                "program-local Extent retirement received a provider-issued root",
            )));
        };
        let Some(held) = self.held.get(&origin) else {
            return Err(Box::new(ProgramLocalExtentRetirementError::new(
                extent,
                "program-local Extent retirement names no held exact occurrence",
            )));
        };
        if !extent.is_lineage_root()
            || extent.lineage_root() != held.lineage
            || extent.base() != held.plan.base
            || extent.length() != held.plan.length
            || extent.address_space() != held.plan.address_space
            || extent.provenance() != held.plan.provenance
            || extent.era() != held.plan.mapping_era
            || !held.plan.rights.contains(extent.rights())
        {
            return Err(Box::new(ProgramLocalExtentRetirementError::new(
                extent,
                "program-local Extent retirement requires the exact recombined root and compatible runtime facts",
            )));
        }

        let held = self
            .held
            .remove(&origin)
            .expect("validated held program-local account remains present");
        match installation.retire_established(held.root, lifecycle) {
            Ok(retired) => Ok(retired),
            Err(error) => {
                let root = (*error).into_root();
                let replaced = self.held.insert(
                    origin,
                    HeldProgramLocalExtent {
                        root,
                        lineage: held.lineage,
                        plan: held.plan,
                    },
                );
                debug_assert!(replaced.is_none());
                Err(Box::new(ProgramLocalExtentRetirementError::new(
                    extent,
                    "program-local Extent retirement could not release its exact lifecycle lease",
                )))
            }
        }
    }
}

#[derive(Debug)]
pub struct ProgramLocalExtentMaterializationError<'root, 'code> {
    inputs: Vec<(
        EstablishedProgramLocalRoot<'root, 'code>,
        ProgramLocalExtentMaterializationPlan,
    )>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'root, 'code> ProgramLocalExtentMaterializationError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_inputs(
        self,
    ) -> Vec<(
        EstablishedProgramLocalRoot<'root, 'code>,
        ProgramLocalExtentMaterializationPlan,
    )> {
        self.inputs
    }
}

#[derive(Debug)]
pub struct ProgramLocalExtentRetirementError {
    extent: Extent,
    diagnostic: ExternalRootDiagnostic,
}

impl ProgramLocalExtentRetirementError {
    fn new(extent: Extent, diagnostic: impl Into<String>) -> Self {
        Self {
            extent,
            diagnostic: ExternalRootDiagnostic(diagnostic.into()),
        }
    }

    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_extent(self) -> Extent {
        self.extent
    }
}

fn exact_origin(
    root: &EstablishedProgramLocalRoot<'_, '_>,
) -> Result<ExtentProgramLocalOrigin, ExternalRootDiagnostic> {
    let occurrence = root.occurrence_identity();
    let prebinding = occurrence.prebinding();
    ExtentProgramLocalOrigin::from_normalized_identities([
        prebinding.installed_code().normalized_identity(),
        prebinding.root().normalized_identity(),
        prebinding.slot().normalized_identity(),
        root.prebinding().schema_compatibility_report_identity(),
        occurrence.lifecycle_ledger().normalized_identity(),
        occurrence.lifecycle_epoch(),
        root.invocation().normalized_identity(),
        root.subject_place().normalized_identity(),
    ])
    .map_err(|diagnostic| {
        ExternalRootDiagnostic(format!(
            "established program-local root has no exact Extent origin: {diagnostic}"
        ))
    })
}

fn validate_materialization(
    root: &EstablishedProgramLocalRoot<'_, '_>,
    plan: &ProgramLocalExtentMaterializationPlan,
) -> Result<(), ExternalRootDiagnostic> {
    let prebinding = root.prebinding();
    if prebinding.carrier_identity() != plan.carrier_identity
        || prebinding.qualification_identity() != plan.qualification_identity
    {
        return Err(ExternalRootDiagnostic(
            "program-local Extent plan substituted the established carrier or qualification".into(),
        ));
    }
    let EstablishedProgramLocalRootCapacity::IntervalSet(capacity) = root.capacity() else {
        return Err(ExternalRootDiagnostic(
            "counted program-local capacity cannot materialize one Extent".into(),
        ));
    };
    if prebinding.algebra().kind != ContentAlgebraKind::IntervalSet
        || prebinding.algebra().parameter != plan.algebra_parameter
    {
        return Err(ExternalRootDiagnostic(
            "program-local Extent requires the exact installed interval-set algebra".into(),
        ));
    }
    let [member] = capacity.members() else {
        return Err(ExternalRootDiagnostic(
            "program-local Extent requires exactly one nonempty interval".into(),
        ));
    };
    let Some(start) = member.start().to_u64() else {
        return Err(ExternalRootDiagnostic(
            "program-local Extent interval start does not fit the target address model".into(),
        ));
    };
    let Some(end) = member.end().to_u64() else {
        return Err(ExternalRootDiagnostic(
            "program-local Extent interval end does not fit the target address model".into(),
        ));
    };
    let expected_end = plan.base.checked_add(plan.length).ok_or_else(|| {
        ExternalRootDiagnostic("program-local Extent plan range overflows".into())
    })?;
    if start != plan.base || end != expected_end || start == end {
        return Err(ExternalRootDiagnostic(
            "program-local Extent geometry does not equal its established interval capacity".into(),
        ));
    }
    Ok(())
}
