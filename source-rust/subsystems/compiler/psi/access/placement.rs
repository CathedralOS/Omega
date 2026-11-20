use crate::StructuralAccess;
use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalAccessViewInput {
    pub machine: MachineId,
    pub position: u32,
    pub runtime_position: u32,
    pub carrier_identity: TerminalIdentityIndex,
    pub source_machine_identity: TerminalIdentityIndex,
    pub source_state_identity: TerminalIdentityIndex,
    pub source_parameter_identity: TerminalIdentityIndex,
    pub access: StructuralAccess,
    pub binding_is_const: bool,
    pub binding_is_mutable: bool,
    pub view_identity: TerminalIdentityIndex,
    pub policy_identity: TerminalIdentityIndex,
    pub policy_plan_machine_identity: TerminalIdentityIndex,
    pub schema_identity: TerminalIdentityIndex,
    pub placement_report_fingerprint: u64,
    pub placement_commitment: [u8; 32],
}

pub fn canonical_access_view_identity(policy: &str, schema: &str) -> String {
    format!(
        "access-view:{}:{policy}:{}:{schema}",
        policy.len(),
        schema.len()
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAccessViewPlan {
    pub carrier: semantic_vocabulary::StructuralTypeId,
    pub schema: semantic_vocabulary::StructuralTypeId,
    pub view_identity: TerminalIdentityIndex,
    pub policy_identity: TerminalIdentityIndex,
    pub policy_plan_machine_identity: TerminalIdentityIndex,
    pub schema_identity: TerminalIdentityIndex,
    pub placement: access_admission::ValidatedPlacement,
    pub fields: Vec<TerminalAccessFieldPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAccessLifecyclePlan {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub target: Option<crate::TargetLayoutInput>,
    pub operation: TerminalAccessLifecycleOperation,
    pub execution: TerminalAccessLifecycleExecution,
    pub storage: TerminalAccessStorage,
    pub schema: semantic_vocabulary::StructuralTypeId,
    pub image_fields: Vec<semantic_vocabulary::StructuralFieldId>,
    pub policy_identity: TerminalIdentityIndex,
    pub policy_plan_machine_identity: TerminalIdentityIndex,
    pub schema_identity: TerminalIdentityIndex,
    pub placement: access_admission::ValidatedPlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAccessLifecycleExecution {
    SelectedProvider,
    CompilerPrimitive {
        profile_type: Option<semantic_vocabulary::StructuralTypeId>,
        profile_requirements: Vec<TerminalAccessProfileRequirement>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAccessProfileRequirement {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub owner: crate::NominalDeclarationIdentity,
    pub callable: TerminalIdentityIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAccessStorage {
    pub carrier: semantic_vocabulary::StructuralTypeId,
    pub owner: TerminalAccessOwner,
    pub base: semantic_vocabulary::StructuralFieldId,
    pub length: semantic_vocabulary::StructuralFieldId,
    pub granted: semantic_vocabulary::StructuralDomainId,
    pub vacant: Option<semantic_vocabulary::StructuralDomainId>,
    pub resident: Option<semantic_vocabulary::StructuralDomainId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAccessOwner {
    Extent,
    Run {
        carrier: semantic_vocabulary::StructuralTypeId,
        storage: semantic_vocabulary::StructuralFieldId,
        count: semantic_vocabulary::StructuralFieldId,
        singleton: Option<semantic_vocabulary::StructuralDomainId>,
    },
}

impl TerminalAccessStorage {
    pub fn same_custody(&self, other: &Self) -> bool {
        let owner_roles = |owner| match owner {
            TerminalAccessOwner::Run {
                carrier,
                storage,
                count,
                ..
            } => TerminalAccessOwner::Run {
                carrier,
                storage,
                count,
                singleton: None,
            },
            TerminalAccessOwner::Extent => TerminalAccessOwner::Extent,
        };
        self.carrier == other.carrier
            && owner_roles(self.owner) == owner_roles(other.owner)
            && self.base == other.base
            && self.length == other.length
            && self.granted == other.granted
            && self.vacant == other.vacant
            && self.resident == other.resident
    }

    pub fn owner_storage_path(
        &self,
        types: &[crate::StructuralTypeDeclaration],
    ) -> Option<Vec<crate::StructuralPathSegment>> {
        match self.owner {
            TerminalAccessOwner::Extent => Some(Vec::new()),
            TerminalAccessOwner::Run {
                carrier, storage, ..
            } => {
                let crate::StructuralTypeShape::Record { fields, .. } =
                    &crate::find_structural_type(types, carrier)?.shape
                else {
                    return None;
                };
                let backing = fields.iter().find(|field| field.id == storage)?;
                Some(vec![crate::StructuralPathSegment::Field(backing.identity)])
            }
        }
    }

    pub fn owner_carrier(&self) -> semantic_vocabulary::StructuralTypeId {
        match self.owner {
            TerminalAccessOwner::Extent => self.carrier,
            TerminalAccessOwner::Run { carrier, .. } => carrier,
        }
    }

    pub fn owned_content(
        &self,
    ) -> Result<
        (
            semantic_vocabulary::StructuralDomainId,
            semantic_vocabulary::StructuralDomainId,
        ),
        &'static str,
    > {
        match (self.vacant, self.resident) {
            (Some(vacant), Some(resident)) => Ok((vacant, resident)),
            _ => Err("owned access lifecycle lost its exact content qualification roles"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResidentReferenceSource {
    pub base: semantic_vocabulary::StructuralFieldId,
    pub length: semantic_vocabulary::StructuralFieldId,
    pub granted: semantic_vocabulary::StructuralDomainId,
    pub resident: semantic_vocabulary::StructuralDomainId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalAccessLifecycleOperation {
    Initialize,
    MoveOut,
    View,
    Relocate,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalAccessProfileOperand {
    pub storage: usize,
    pub profile: usize,
    pub occupied: bool,
}

impl TerminalAccessLifecycleOperation {
    pub const fn profile_operands(self) -> &'static [TerminalAccessProfileOperand] {
        match self {
            Self::Initialize => &[TerminalAccessProfileOperand {
                storage: 0,
                profile: 2,
                occupied: false,
            }],
            Self::MoveOut => &[TerminalAccessProfileOperand {
                storage: 0,
                profile: 1,
                occupied: true,
            }],
            Self::View => &[TerminalAccessProfileOperand {
                storage: 0,
                profile: 1,
                occupied: false,
            }],
            Self::Relocate => &[
                TerminalAccessProfileOperand {
                    storage: 0,
                    profile: 2,
                    occupied: true,
                },
                TerminalAccessProfileOperand {
                    storage: 1,
                    profile: 3,
                    occupied: false,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAccessFieldPlan {
    pub access_slot: u32,
    pub view_field: TerminalIdentityIndex,
    pub schema_field: TerminalIdentityIndex,
    pub accessor: semantic_vocabulary::StructuralTypeId,
    pub operations: Vec<TerminalAccessor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalAccessorKind {
    Read,
    Take,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAccessor {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub kind: TerminalAccessorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessRamFieldCorrespondence {
    pub access_slot: u32,
    pub source_field: semantic_vocabulary::StructuralFieldId,
    pub byte_offset: u64,
    pub transfer_width_bits: u16,
    pub logical_scalar: semantic_vocabulary::ScalarType,
    pub stored_scalar: semantic_vocabulary::ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessRamCorrespondence {
    pub schema: semantic_vocabulary::StructuralTypeId,
    pub carrier: semantic_vocabulary::StructuralTypeId,
    pub source_byte_length: u64,
    pub required_base: access_admission::BaseCongruence,
    pub fields: Vec<AccessRamFieldCorrespondence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessorScalarBoundaryDemand {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub carrier: semantic_vocabulary::StructuralTypeId,
    pub schema: semantic_vocabulary::StructuralTypeId,
    pub accessor: semantic_vocabulary::StructuralTypeId,
    pub view_field: crate::TerminalIdentityIndex,
    pub access_slot: u32,
    pub placement_commitment: [u8; 32],
    pub operation: TerminalAccessorKind,
    pub access: crate::StructuralAccess,
    pub scalar: semantic_vocabulary::ScalarType,
    pub byte_offset: u64,
    pub transfer_width_bits: u16,
    pub placement_byte_extent: u64,
    pub observation: access_vocabulary::ObservationModel,
}
