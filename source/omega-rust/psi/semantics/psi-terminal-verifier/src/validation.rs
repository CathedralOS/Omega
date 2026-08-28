use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentConservation, ContentDomainId, ContentProjectionExpression, ContentProjectionIdentity,
    ContentProjectionScalar, ContentStructuralPlace, ContentTerm, ContractId, EdgeId,
    EvidenceTermId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, Proposition, PropositionContext, PropositionId, ScalarTerm, ScalarType, ServiceId,
    StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
    content_conservation_fingerprint,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, ContentPartitionComposition,
    CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, EntryClaim, EvidenceContractLaneKind,
    OperationKind, OperationResult, PropositionBinderArgumentKind, PropositionBinderKind,
    PropositionEvidence, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, program_local_root_introduction_identity,
};

use crate::verification::{
    substitute_proposition_structural_places, substitute_proposition_values,
};

mod affine_cleanup;
mod call_graph;
mod conformance_applications;
mod content;
mod contracts;
mod control_flow;
mod crash;
mod error;
mod evidence;
mod float_meaning;
mod foundation;
mod frontier;
mod machine;
mod operations;
mod propositions;
mod quotient_correspondence;
mod root_service_reach;
mod structural_operations;

use call_graph::validate_call_graph;
use conformance_applications::validate_closed_conformance_applications;
pub use error::{ContractClauseKind, ModuleError};
use evidence::{validate_evidence_contract_lanes, validate_proposition_vocabulary};
pub use foundation::{ServiceCeilingOwner, StructuralSignatureOwner};
use foundation::{
    is_bounded_partial_affine_path, is_nonempty_field_path, partial_affine_residuals,
    resolve_structural_path, validate_structural_foundation,
};
pub use frontier::{
    VerifiedLiveClaim, VerifiedMachineStructuralFrontiers, VerifiedOwnedStructuralPlace,
    VerifiedPartialStructuralCustody, VerifiedStructuralOwnershipFrontier,
    VerifiedTerminalStructuralFrontiers,
};
pub(crate) use structural_operations::{
    exact_payloadless_case_return_exits, structural_argument_canonical_prefix,
};

#[derive(Debug, Clone, Copy)]
pub struct ValidatedTerminalModule<'module> {
    module: &'module TerminalModule,
}

impl<'module> ValidatedTerminalModule<'module> {
    pub const fn module(self) -> &'module TerminalModule {
        self.module
    }

    pub fn machine(self, id: MachineId) -> Option<&'module TerminalMachine> {
        self.module.machines.iter().find(|machine| machine.id == id)
    }

    pub fn value_context(
        self,
        machine: &TerminalMachine,
    ) -> Result<PropositionContext, ModuleError> {
        PropositionContext::from_value_types_and_places(
            machine_value_types(machine),
            machine
                .structural_places
                .iter()
                .map(|place| (place.id, place.kind))
                .chain(
                    affine_cleanup::nominal_cleanup_contract_receiver(self.module, machine.id).map(
                        |receiver| {
                            (
                                receiver,
                                StructuralPlaceKind::Parameter {
                                    position: 0,
                                    is_self: true,
                                },
                            )
                        },
                    ),
                ),
        )
        .map_err(ModuleError::MalformedProposition)
    }
}

pub fn validate_module(
    module: &TerminalModule,
) -> Result<ValidatedTerminalModule<'_>, ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Execution)?;
    Ok(ValidatedTerminalModule { module })
}

/// Validate a Terminal-Psi module and expose the exact deterministic ownership
/// frontier snapshots computed by the verifier's own custody walk.
pub fn reconstruct_structural_ownership_frontiers(
    module: &TerminalModule,
) -> Result<VerifiedTerminalStructuralFrontiers, ModuleError> {
    validate_module(module)?;
    reconstruct_validated_structural_ownership_frontiers(module)
}

pub(crate) fn reconstruct_validated_structural_ownership_frontiers(
    module: &TerminalModule,
) -> Result<VerifiedTerminalStructuralFrontiers, ModuleError> {
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let snapshots = module
        .machines
        .iter()
        .map(|machine| {
            let blocks = machine
                .blocks
                .iter()
                .map(|block| (block.id, block))
                .collect::<BTreeMap<_, _>>();
            frontier::validate_structural_frontier(module, machine, &machines, &blocks)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(VerifiedTerminalStructuralFrontiers {
        machines: snapshots,
    })
}

/// Validate the representation-wide invariants needed by canonical codecs.
///
/// This remains a distinct entry point so canonical representation checks do
/// not themselves confer an execution-grade `ValidatedTerminalModule`.
pub fn validate_module_representation(module: &TerminalModule) -> Result<(), ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Representation)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationPolicy {
    Execution,
    Representation,
}

fn validate_module_with_policy(
    module: &TerminalModule,
    policy: ValidationPolicy,
) -> Result<(), ModuleError> {
    if module.machines.is_empty() {
        return Err(ModuleError::EmptyModule);
    }
    validate_proposition_vocabulary(module)?;
    validate_structural_foundation(module)?;
    validate_closed_conformance_applications(module)?;
    float_meaning::validate_float_meaning_projections(module)?;
    quotient_correspondence::validate_quotient_correspondences(module, policy)?;

    let mut registry = IdRegistry::default();
    for projection in module
        .structural_domains
        .iter()
        .filter_map(|domain| domain.content_projection.as_ref())
    {
        if registry
            .owner_content_projections
            .insert(
                projection.identity.domain,
                (projection.identity, projection.algebra.clone()),
            )
            .is_some()
        {
            return Err(ModuleError::ContentProjectionOwnerMismatch(
                projection.identity,
            ));
        }
    }
    content::validate_boundary_content_guarantees(module, &mut registry)?;
    for machine in &module.machines {
        insert_unique(
            &mut registry.machines,
            machine.id,
            ModuleError::DuplicateMachine,
        )?;
        insert_unique(
            &mut registry.contracts,
            machine.contract.id,
            ModuleError::DuplicateContract,
        )?;
    }
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    validate_evidence_contract_lanes(module, &machines)?;
    for machine in &module.machines {
        machine::validate_machine(module, machine, &machines, &mut registry, policy)?;
    }
    validate_call_graph(module)?;
    if !registry.machines.contains(&module.entry) {
        return Err(ModuleError::UnknownEntryMachine(module.entry));
    }
    root_service_reach::validate_root_service_reach_exact(module)?;

    Ok(())
}

#[derive(Default)]
struct IdRegistry {
    machines: BTreeSet<MachineId>,
    blocks: BTreeSet<BlockId>,
    contracts: BTreeSet<ContractId>,
    operations: BTreeSet<OperationId>,
    edges: BTreeSet<EdgeId>,
    obligations: BTreeSet<ObligationId>,
    values: BTreeSet<ValueId>,
    places: BTreeSet<PlaceId>,
    owner_content_projections:
        BTreeMap<ContentDomainId, (ContentProjectionIdentity, ContentAlgebra)>,
    content_projection_algebras: BTreeMap<ContentProjectionIdentity, ContentAlgebra>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StructuralRootKey {
    Parameter(u32),
    Result,
    OperationResult(OperationId),
    ByteSequenceLiteral(u32),
    ProviderAttachment(StructuralTypeId, StructuralFieldId, BoundaryMachineId),
    TrivialAffineLocal(u32),
}

fn validate_boolean_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    source: PlaceId,
    field: StructuralFieldId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidBooleanStructuralField {
        operation,
        source,
        field,
    };
    if machine.id != module.entry {
        return Err(invalid());
    }
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .filter(|parameter| {
            parameter.multiplicity == StructuralMultiplicity::Affine
                && parameter.qualifications.is_empty()
        })
        .ok_or_else(invalid)?;
    if parameter.access == StructuralAccess::WriteOnlyBorrow {
        return Err(ModuleError::StructuralObservationRequiresReadableAccess { operation, source });
    }
    if machine
        .entry_claims
        .iter()
        .any(|claim| claim.input == source)
        || !machine.content_entry_claims.is_empty()
        || !machine
            .parameters
            .iter()
            .any(|parameter| parameter.scalar_type == ScalarType::Boolean)
        || !affine_cleanup::every_scalar_return_nominally_cleans(machine, source)
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|candidate| {
                matches!(candidate.kind,
                OperationKind::BooleanStructuralField {
                    source: other_source,
                    field: other_field,
                } if (other_source, other_field) != (source, field))
            })
    {
        return Err(invalid());
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(invalid());
    };
    if !fields.iter().any(|candidate| {
        candidate.id == field
            && !candidate.relevance.is_erased()
            && candidate.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
    }) {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn machine_value_types(
    machine: &TerminalMachine,
) -> impl Iterator<Item = (ValueId, ScalarType)> + '_ {
    machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| operation.result.scalar_ref())
        }))
        .map(|declaration| (declaration.id, declaration.scalar_type))
}

fn insert_value(
    values: &mut BTreeMap<ValueId, ScalarType>,
    module_values: &mut BTreeSet<ValueId>,
    id: ValueId,
    scalar_type: ScalarType,
) -> Result<(), ModuleError> {
    if values.insert(id, scalar_type).is_some() || !module_values.insert(id) {
        return Err(ModuleError::DuplicateValue(id));
    }
    Ok(())
}

fn insert_unique<T: Ord + Copy>(
    set: &mut BTreeSet<T>,
    value: T,
    error: impl FnOnce(T) -> ModuleError,
) -> Result<(), ModuleError> {
    if !set.insert(value) {
        return Err(error(value));
    }
    Ok(())
}
