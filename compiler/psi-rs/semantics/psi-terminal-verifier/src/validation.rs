use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentConservation, ContentPlaceSegment,
    ContentProjectionIdentity, ContentStructuralPlace, ContentTerm, ContractId, EdgeId, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionContext, PropositionError,
    ScalarTerm, ScalarType, StructuralPlaceKind, ValueId,
};
use psi_terminal::{
    ContentPartitionComposition, OperationKind, SemanticVersion, TerminalMachine, TerminalModule,
    Terminator,
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
                .map(|place| (place.id, place.kind)),
        )
        .map_err(ModuleError::MalformedProposition)
    }
}

pub fn validate_module(
    module: &TerminalModule,
) -> Result<ValidatedTerminalModule<'_>, ModuleError> {
    if !matches!(
        module.semantic_version,
        SemanticVersion::V1
            | SemanticVersion::V2
            | SemanticVersion::V3
            | SemanticVersion::V4
            | SemanticVersion::V5
            | SemanticVersion::V6
            | SemanticVersion::V7
            | SemanticVersion::V8
            | SemanticVersion::V9
            | SemanticVersion::V10
            | SemanticVersion::V11
            | SemanticVersion::V12
    ) {
        return Err(ModuleError::UnsupportedSemanticVersion(
            module.semantic_version,
        ));
    }
    if module.machines.is_empty() {
        return Err(ModuleError::EmptyModule);
    }

    let mut registry = IdRegistry::default();
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
        validate_machine(module.semantic_version, machine, &mut registry)?;
    }
    if !registry.machines.contains(&module.entry) {
        return Err(ModuleError::UnknownEntryMachine(module.entry));
    }

    Ok(ValidatedTerminalModule { module })
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
    content_projection_algebras: BTreeMap<ContentProjectionIdentity, ContentAlgebra>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StructuralRootKey {
    Parameter(u32),
    Result,
}

fn validate_machine(
    semantic_version: SemanticVersion,
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    if machine.blocks.is_empty() {
        return Err(ModuleError::MachineHasNoBlocks(machine.id));
    }

    let mut blocks = BTreeMap::new();
    let mut value_types = BTreeMap::new();
    let mut structural_roots = BTreeSet::new();
    if semantic_version < SemanticVersion::V9 && !machine.structural_places.is_empty() {
        return Err(ModuleError::StructuralPlacesRequireSemanticVersion {
            machine: machine.id,
            required: SemanticVersion::V9,
            actual: semantic_version,
        });
    }
    if semantic_version < SemanticVersion::V10 && !machine.content_identity_reshuffles.is_empty() {
        return Err(
            ModuleError::ContentIdentityReshufflesRequireSemanticVersion {
                machine: machine.id,
                required: SemanticVersion::V10,
                actual: semantic_version,
            },
        );
    }
    if semantic_version < SemanticVersion::V12 && !machine.content_partition_compositions.is_empty()
    {
        return Err(
            ModuleError::ContentPartitionCompositionsRequireSemanticVersion {
                machine: machine.id,
                required: SemanticVersion::V12,
                actual: semantic_version,
            },
        );
    }
    let mut structural_place_kinds = BTreeMap::new();
    for place in &machine.structural_places {
        insert_unique(&mut registry.places, place.id, ModuleError::DuplicatePlace)?;
        let root = match place.kind {
            psi_core::StructuralPlaceKind::Parameter { position, .. } => {
                StructuralRootKey::Parameter(position)
            }
            psi_core::StructuralPlaceKind::Result => StructuralRootKey::Result,
        };
        if !structural_roots.insert(root) {
            return Err(ModuleError::DuplicateStructuralPlaceRoot {
                machine: machine.id,
                kind: place.kind,
            });
        }
        structural_place_kinds.insert(place.id, place.kind);
    }
    for declaration in machine
        .parameters
        .iter()
        .chain(std::iter::once(&machine.result))
    {
        insert_value(
            &mut value_types,
            &mut registry.values,
            declaration.id,
            declaration.scalar_type,
        )?;
    }
    for block in &machine.blocks {
        insert_unique(&mut registry.blocks, block.id, ModuleError::DuplicateBlock)?;
        if blocks.insert(block.id, block).is_some() {
            return Err(ModuleError::DuplicateBlock(block.id));
        }
        for parameter in &block.parameters {
            insert_value(
                &mut value_types,
                &mut registry.values,
                parameter.id,
                parameter.scalar_type,
            )?;
        }
        for operation in &block.operations {
            insert_unique(
                &mut registry.operations,
                operation.id,
                ModuleError::DuplicateOperation,
            )?;
            insert_value(
                &mut value_types,
                &mut registry.values,
                operation.result.id,
                operation.result.scalar_type,
            )?;
            match operation.kind {
                OperationKind::IntegerConstant { value } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        return Err(ModuleError::IntegerConstantRequiresIntegerResult(
                            operation.id,
                        ));
                    };
                    if !integer_type.admits(value) {
                        return Err(ModuleError::IntegerConstantOutsideResultType(operation.id));
                    }
                }
                OperationKind::BooleanConstant { .. } => {
                    if semantic_version < SemanticVersion::V2 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V2,
                            actual: semantic_version,
                        });
                    }
                    if operation.result.scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanConstantRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerAdd { .. } => {
                    if semantic_version < SemanticVersion::V3 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V3,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerAdd { .. } => {
                    if semantic_version < SemanticVersion::V4 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V4,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::SaturatingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerSubtract { .. } => {
                    if semantic_version < SemanticVersion::V5 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V5,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerSubtract { .. } => {
                    if semantic_version < SemanticVersion::V6 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V6,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::SaturatingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerMultiply { .. } => {
                    if semantic_version < SemanticVersion::V7 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V7,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::WrappingIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerMultiply { .. } => {
                    if semantic_version < SemanticVersion::V8 {
                        return Err(ModuleError::OperationRequiresSemanticVersion {
                            operation: operation.id,
                            required: SemanticVersion::V8,
                            actual: semantic_version,
                        });
                    }
                    if !matches!(operation.result.scalar_type, ScalarType::Integer(_)) {
                        return Err(ModuleError::SaturatingIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
            }
        }
        insert_unique(
            &mut registry.edges,
            block.terminator.edge(),
            ModuleError::DuplicateEdge,
        )?;
    }

    let Some(entry) = blocks.get(&machine.entry) else {
        return Err(ModuleError::UnknownEntryBlock {
            machine: machine.id,
            block: machine.entry,
        });
    };
    if !entry.parameters.is_empty() {
        return Err(ModuleError::EntryBlockCannotHaveParameters(machine.entry));
    }

    let context = PropositionContext::from_value_types_and_places(
        value_types.iter().map(|(id, ty)| (*id, *ty)),
        machine
            .structural_places
            .iter()
            .map(|place| (place.id, place.kind)),
    )
    .map_err(ModuleError::MalformedProposition)?;
    validate_content_identity_reshuffles(
        machine,
        semantic_version,
        registry,
        &structural_place_kinds,
        &context,
    )?;
    validate_content_partition_compositions(
        machine,
        semantic_version,
        registry,
        &structural_place_kinds,
        &context,
    )?;
    let requires_values = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let mut ensures_values = requires_values.clone();
    ensures_values.insert(machine.result.id);
    for proposition in &machine.contract.requires {
        validate_proposition_semantic_version(
            proposition,
            semantic_version,
            machine.contract.id,
            ContractClauseKind::Requires,
        )?;
        context
            .validate(proposition)
            .map_err(ModuleError::MalformedProposition)?;
        validate_contract_scope(
            proposition,
            &requires_values,
            machine.contract.id,
            ContractClauseKind::Requires,
        )?;
    }
    for clause in &machine.contract.ensures {
        insert_unique(
            &mut registry.obligations,
            clause.obligation,
            ModuleError::DuplicateObligation,
        )?;
        validate_proposition_semantic_version(
            &clause.proposition,
            semantic_version,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
        context
            .validate(&clause.proposition)
            .map_err(ModuleError::MalformedProposition)?;
        validate_contract_scope(
            &clause.proposition,
            &ensures_values,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
    }

    validate_straight_line_flow(machine, &blocks, &value_types)
}

fn validate_content_identity_reshuffles(
    machine: &TerminalMachine,
    semantic_version: SemanticVersion,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let mut claims = BTreeSet::<ClaimId>::new();
    let mut inputs = BTreeSet::<ContentStructuralPlace>::new();
    let mut outputs = BTreeSet::<ContentStructuralPlace>::new();
    for reshuffle in &machine.content_identity_reshuffles {
        insert_unique(&mut claims, reshuffle.claim, ModuleError::DuplicateClaim)?;
        if reshuffle.projections.is_empty() {
            return Err(ModuleError::ContentIdentityReshuffleHasNoProjections(
                reshuffle.claim,
            ));
        }
        if semantic_version < SemanticVersion::V11
            && reshuffle
                .input
                .segments
                .iter()
                .chain(&reshuffle.output.segments)
                .any(|segment| matches!(segment, ContentPlaceSegment::Case(_)))
        {
            return Err(
                ModuleError::ContentIdentityCasePathRequiresSemanticVersion {
                    claim: reshuffle.claim,
                    required: SemanticVersion::V11,
                    actual: semantic_version,
                },
            );
        }
        if reshuffle
            .projections
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentIdentityProjectionOrder(
                reshuffle.claim,
            ));
        }
        if reshuffle.input.version != psi_core::ContentPlaceVersion::Entry
            || !matches!(
                structural_place_kinds.get(&reshuffle.input.root),
                Some(StructuralPlaceKind::Parameter { .. })
            )
        {
            return Err(ModuleError::ContentIdentityReshuffleRequiresEntryParameter(
                reshuffle.claim,
            ));
        }
        if reshuffle.output.version != psi_core::ContentPlaceVersion::Current
            || !matches!(
                structural_place_kinds.get(&reshuffle.output.root),
                Some(StructuralPlaceKind::Result)
            )
        {
            return Err(ModuleError::ContentIdentityReshuffleRequiresCurrentResult(
                reshuffle.claim,
            ));
        }
        if inputs.contains(&reshuffle.input) {
            return Err(ModuleError::DuplicateContentIdentityInput(
                reshuffle.input.clone(),
            ));
        }
        if let Some(previous) = inputs
            .iter()
            .find(|previous| content_places_overlap(previous, &reshuffle.input))
        {
            return Err(ModuleError::OverlappingContentIdentityInput {
                first: previous.clone(),
                second: reshuffle.input.clone(),
            });
        }
        inputs.insert(reshuffle.input.clone());
        if outputs.contains(&reshuffle.output) {
            return Err(ModuleError::DuplicateContentIdentityOutput(
                reshuffle.output.clone(),
            ));
        }
        if let Some(previous) = outputs
            .iter()
            .find(|previous| content_places_overlap(previous, &reshuffle.output))
        {
            return Err(ModuleError::OverlappingContentIdentityOutput {
                first: previous.clone(),
                second: reshuffle.output.clone(),
            });
        }
        outputs.insert(reshuffle.output.clone());
        for (content, proposition) in reshuffle
            .projections
            .iter()
            .zip(reshuffle.inferred_propositions())
        {
            if let Some(previous) = registry
                .content_projection_algebras
                .insert(content.projection, content.algebra.clone())
                && previous != content.algebra
            {
                return Err(ModuleError::ContentProjectionAlgebraMismatch(
                    content.projection,
                ));
            }
            context
                .validate(&proposition)
                .map_err(ModuleError::MalformedProposition)?;
        }
    }
    Ok(())
}

fn validate_content_partition_compositions(
    machine: &TerminalMachine,
    semantic_version: SemanticVersion,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let mut rows = BTreeSet::<&ContentPartitionComposition>::new();
    for composition in &machine.content_partition_compositions {
        if !rows.insert(composition) {
            return Err(ModuleError::DuplicateContentPartitionComposition);
        }
        if composition.input_claims.is_empty() {
            return Err(ModuleError::ContentPartitionCompositionHasNoInputClaims);
        }
        if composition
            .input_claims
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentPartitionInputClaims);
        }
        if composition
            .substitutions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentPartitionSubstitutions);
        }
        if composition.source.algebra() != composition.derived.algebra() {
            return Err(ModuleError::ContentPartitionAlgebraMismatch);
        }
        if !content_term_contains_partition(composition.source.left())
            && !content_term_contains_partition(composition.source.right())
        {
            return Err(ModuleError::ContentPartitionSourceHasNoSeparation);
        }

        let source_kinds = validate_partition_source_places(composition)?;
        let source_context = PropositionContext::from_value_types_and_places(
            [],
            composition
                .source_structural_places
                .iter()
                .map(|place| (place.id, place.kind)),
        )
        .map_err(ModuleError::MalformedProposition)?;
        source_context
            .validate(&Proposition::ContentConservation(
                composition.source.clone(),
            ))
            .map_err(ModuleError::MalformedProposition)?;
        context
            .validate(&composition.inferred_proposition())
            .map_err(ModuleError::MalformedProposition)?;
        validate_partition_case_version(composition, semantic_version)?;
        register_partition_projections(registry, &composition.source)?;
        register_partition_projections(registry, &composition.derived)?;

        let substitutions = composition
            .substitutions
            .iter()
            .map(|substitution| (substitution.source.clone(), substitution.target.clone()))
            .collect::<BTreeMap<_, _>>();
        if substitutions.len() != composition.substitutions.len() {
            return Err(ModuleError::NonCanonicalContentPartitionSubstitutions);
        }
        let target_count = composition
            .substitutions
            .iter()
            .map(|substitution| &substitution.target)
            .collect::<BTreeSet<_>>()
            .len();
        if target_count != composition.substitutions.len() {
            return Err(ModuleError::DuplicateContentPartitionSubstitutionTarget);
        }
        let source_subjects = content_conservation_subjects(&composition.source);
        if source_subjects
            != substitutions
                .keys()
                .cloned()
                .collect::<BTreeSet<ContentStructuralPlace>>()
        {
            return Err(ModuleError::ContentPartitionSubstitutionCoverageMismatch);
        }
        for substitution in &composition.substitutions {
            validate_partition_substitution_shape(
                substitution,
                &source_kinds,
                structural_place_kinds,
            )?;
        }
        let replayed = replay_partition_conservation(&composition.source, &substitutions)?;
        if replayed != composition.derived {
            return Err(ModuleError::ContentPartitionReplayMismatch);
        }

        let listed_claims = composition
            .input_claims
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut used_claims = BTreeSet::new();
        for (projection, subject) in content_conservation_projections(&composition.derived) {
            if subject.version != psi_core::ContentPlaceVersion::Entry {
                continue;
            }
            let matching = machine
                .content_identity_reshuffles
                .iter()
                .filter(|reshuffle| {
                    reshuffle.input == subject
                        && reshuffle.projections.iter().any(|content| {
                            content.projection == projection
                                && content.algebra == *composition.derived.algebra()
                        })
                })
                .collect::<Vec<_>>();
            let [reshuffle] = matching.as_slice() else {
                return Err(ModuleError::ContentPartitionInputProjectionNotClaimBound(
                    subject,
                ));
            };
            if !listed_claims.contains(&reshuffle.claim) {
                return Err(ModuleError::ContentPartitionInputClaimNotListed(
                    reshuffle.claim,
                ));
            }
            used_claims.insert(reshuffle.claim);
        }
        if used_claims != listed_claims {
            return Err(ModuleError::ContentPartitionInputClaimUnused);
        }
    }
    Ok(())
}

fn validate_partition_source_places(
    composition: &ContentPartitionComposition,
) -> Result<BTreeMap<PlaceId, StructuralPlaceKind>, ModuleError> {
    let mut ids = BTreeMap::new();
    let mut roots = BTreeSet::new();
    for place in &composition.source_structural_places {
        if ids.insert(place.id, place.kind).is_some() {
            return Err(ModuleError::DuplicateContentPartitionSourcePlace(place.id));
        }
        let root = match place.kind {
            StructuralPlaceKind::Parameter { position, .. } => {
                StructuralRootKey::Parameter(position)
            }
            StructuralPlaceKind::Result => StructuralRootKey::Result,
        };
        if !roots.insert(root) {
            return Err(ModuleError::DuplicateContentPartitionSourceRoot(place.kind));
        }
    }
    Ok(ids)
}

fn validate_partition_substitution_shape(
    substitution: &psi_terminal::ContentPlaceSubstitution,
    source_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    target_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
) -> Result<(), ModuleError> {
    match (
        substitution.source.version,
        source_kinds.get(&substitution.source.root),
        substitution.target.version,
        target_kinds.get(&substitution.target.root),
    ) {
        (
            psi_core::ContentPlaceVersion::Entry,
            Some(StructuralPlaceKind::Parameter { .. }),
            psi_core::ContentPlaceVersion::Entry,
            Some(StructuralPlaceKind::Parameter { .. }),
        )
        | (
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Result),
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Result),
        ) => Ok(()),
        _ => Err(ModuleError::InvalidContentPartitionSubstitutionShape),
    }
}

fn replay_partition_conservation(
    source: &ContentConservation,
    substitutions: &BTreeMap<ContentStructuralPlace, ContentStructuralPlace>,
) -> Result<ContentConservation, ModuleError> {
    Ok(ContentConservation::new(
        source.algebra().clone(),
        replay_partition_term(source.left(), substitutions)?,
        replay_partition_term(source.right(), substitutions)?,
    ))
}

fn replay_partition_term(
    term: &ContentTerm,
    substitutions: &BTreeMap<ContentStructuralPlace, ContentStructuralPlace>,
) -> Result<ContentTerm, ModuleError> {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => Ok(ContentTerm::Projection {
            projection: *projection,
            subject: substitutions
                .get(subject)
                .cloned()
                .ok_or(ModuleError::ContentPartitionSubstitutionCoverageMismatch)?,
        }),
        ContentTerm::Separate(terms) => ContentTerm::separate(
            terms
                .iter()
                .map(|term| replay_partition_term(term, substitutions))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(ModuleError::MalformedProposition),
    }
}

fn content_term_contains_partition(term: &ContentTerm) -> bool {
    match term {
        ContentTerm::Projection { .. } => false,
        ContentTerm::Separate(_) => true,
    }
}

fn content_conservation_subjects(
    conservation: &ContentConservation,
) -> BTreeSet<ContentStructuralPlace> {
    content_conservation_projections(conservation)
        .into_iter()
        .map(|(_, subject)| subject)
        .collect()
}

fn content_conservation_projections(
    conservation: &ContentConservation,
) -> Vec<(ContentProjectionIdentity, ContentStructuralPlace)> {
    fn collect(
        term: &ContentTerm,
        projections: &mut Vec<(ContentProjectionIdentity, ContentStructuralPlace)>,
    ) {
        match term {
            ContentTerm::Projection {
                projection,
                subject,
            } => projections.push((*projection, subject.clone())),
            ContentTerm::Separate(terms) => {
                for term in terms {
                    collect(term, projections);
                }
            }
        }
    }
    let mut projections = Vec::new();
    collect(conservation.left(), &mut projections);
    collect(conservation.right(), &mut projections);
    projections
}

fn register_partition_projections(
    registry: &mut IdRegistry,
    conservation: &ContentConservation,
) -> Result<(), ModuleError> {
    for (projection, _) in content_conservation_projections(conservation) {
        if let Some(previous) = registry
            .content_projection_algebras
            .insert(projection, conservation.algebra().clone())
            && previous != *conservation.algebra()
        {
            return Err(ModuleError::ContentProjectionAlgebraMismatch(projection));
        }
    }
    Ok(())
}

fn validate_partition_case_version(
    composition: &ContentPartitionComposition,
    semantic_version: SemanticVersion,
) -> Result<(), ModuleError> {
    if semantic_version < SemanticVersion::V11
        && [
            composition.source.left(),
            composition.source.right(),
            composition.derived.left(),
            composition.derived.right(),
        ]
        .into_iter()
        .any(content_term_uses_case)
    {
        return Err(
            ModuleError::ContentPartitionCasePathRequiresSemanticVersion {
                required: SemanticVersion::V11,
                actual: semantic_version,
            },
        );
    }
    Ok(())
}

fn content_places_overlap(left: &ContentStructuralPlace, right: &ContentStructuralPlace) -> bool {
    if left.version != right.version || left.root != right.root {
        return false;
    }
    let shared = left.segments.len().min(right.segments.len());
    left.segments[..shared] == right.segments[..shared]
}

fn validate_proposition_semantic_version(
    proposition: &Proposition,
    semantic_version: SemanticVersion,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        Proposition::Conjunction(conjuncts) => {
            for conjunct in conjuncts {
                validate_proposition_semantic_version(
                    conjunct,
                    semantic_version,
                    contract,
                    clause,
                )?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_proposition_semantic_version(premise, semantic_version, contract, clause)?;
            validate_proposition_semantic_version(conclusion, semantic_version, contract, clause)
        }
        Proposition::ContentConservation(conservation) => {
            if semantic_version < SemanticVersion::V9 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V9,
                    actual: semantic_version,
                });
            }
            if clause != ContractClauseKind::Ensures {
                return Err(ModuleError::ContentConservationRequiresEnsures { contract });
            }
            if semantic_version < SemanticVersion::V11
                && (content_term_uses_case(conservation.left())
                    || content_term_uses_case(conservation.right()))
            {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V11,
                    actual: semantic_version,
                });
            }
            Ok(())
        }
    }
}

fn content_term_uses_case(term: &ContentTerm) -> bool {
    match term {
        ContentTerm::Projection { subject, .. } => subject
            .segments
            .iter()
            .any(|segment| matches!(segment, ContentPlaceSegment::Case(_))),
        ContentTerm::Separate(terms) => terms.iter().any(content_term_uses_case),
    }
}

fn validate_term_semantic_version(
    term: &ScalarTerm,
    semantic_version: SemanticVersion,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match term {
        ScalarTerm::WrappingIntegerAdd { left, right, .. } => {
            if semantic_version < SemanticVersion::V3 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V3,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
            if semantic_version < SemanticVersion::V4 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V4,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::WrappingIntegerSubtract { left, right, .. } => {
            if semantic_version < SemanticVersion::V5 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V5,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => {
            if semantic_version < SemanticVersion::V6 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V6,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::WrappingIntegerMultiply { left, right, .. } => {
            if semantic_version < SemanticVersion::V7 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V7,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            if semantic_version < SemanticVersion::V8 {
                return Err(ModuleError::PropositionRequiresSemanticVersion {
                    contract,
                    clause,
                    required: SemanticVersion::V8,
                    actual: semantic_version,
                });
            }
            validate_term_semantic_version(left, semantic_version, contract, clause)?;
            validate_term_semantic_version(right, semantic_version, contract, clause)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => Ok(()),
    }
}

fn validate_contract_scope(
    proposition: &Proposition,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term_scope(left, allowed, contract, clause)?;
            validate_term_scope(right, allowed, contract, clause)
        }
        Proposition::Conjunction(conjuncts) => {
            for conjunct in conjuncts {
                validate_contract_scope(conjunct, allowed, contract, clause)?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_contract_scope(premise, allowed, contract, clause)?;
            validate_contract_scope(conclusion, allowed, contract, clause)
        }
        Proposition::ContentConservation(_) => Ok(()),
    }
}

fn validate_term_scope(
    term: &ScalarTerm,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match term {
        ScalarTerm::Value { id, .. } => {
            if !allowed.contains(id) {
                return Err(ModuleError::ContractValueOutsideScope {
                    contract,
                    clause,
                    value: *id,
                });
            }
        }
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            validate_term_scope(left, allowed, contract, clause)?;
            validate_term_scope(right, allowed, contract, clause)?;
        }
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
    }
    Ok(())
}

fn validate_straight_line_flow(
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<(), ModuleError> {
    let mut defined = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let mut visited = BTreeSet::new();
    let mut current = machine.entry;

    loop {
        if !visited.insert(current) {
            return Err(ModuleError::ControlCycle(current));
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(ModuleError::UnknownTargetBlock(current))?;
        for parameter in &block.parameters {
            defined.insert(parameter.id);
        }
        for operation in &block.operations {
            if let Some((left, right, arithmetic)) = match operation.kind {
                OperationKind::WrappingIntegerAdd { left, right } => {
                    Some((left, right, ArithmeticOperandKind::WrappingAdd))
                }
                OperationKind::SaturatingIntegerAdd { left, right } => {
                    Some((left, right, ArithmeticOperandKind::SaturatingAdd))
                }
                OperationKind::WrappingIntegerSubtract { left, right } => {
                    Some((left, right, ArithmeticOperandKind::WrappingSubtract))
                }
                OperationKind::SaturatingIntegerSubtract { left, right } => {
                    Some((left, right, ArithmeticOperandKind::SaturatingSubtract))
                }
                OperationKind::WrappingIntegerMultiply { left, right } => {
                    Some((left, right, ArithmeticOperandKind::WrappingMultiply))
                }
                OperationKind::SaturatingIntegerMultiply { left, right } => {
                    Some((left, right, ArithmeticOperandKind::SaturatingMultiply))
                }
                OperationKind::IntegerConstant { .. } | OperationKind::BooleanConstant { .. } => {
                    None
                }
            } {
                let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                    unreachable!("operation shape validation requires an integer result")
                };
                for operand in [left, right] {
                    if !defined.contains(&operand) {
                        return Err(ModuleError::ValueUsedBeforeDefinition(operand));
                    }
                    let actual = value_types
                        .get(&operand)
                        .copied()
                        .ok_or(ModuleError::UnknownValue(operand))?;
                    let expected = ScalarType::Integer(integer_type);
                    if actual != expected {
                        return Err(match arithmetic {
                            ArithmeticOperandKind::SaturatingAdd => {
                                ModuleError::SaturatingIntegerAddOperandTypeMismatch {
                                    operation: operation.id,
                                    operand,
                                    expected,
                                    actual,
                                }
                            }
                            ArithmeticOperandKind::WrappingAdd => {
                                ModuleError::WrappingIntegerAddOperandTypeMismatch {
                                    operation: operation.id,
                                    operand,
                                    expected,
                                    actual,
                                }
                            }
                            ArithmeticOperandKind::WrappingSubtract => {
                                ModuleError::WrappingIntegerSubtractOperandTypeMismatch {
                                    operation: operation.id,
                                    operand,
                                    expected,
                                    actual,
                                }
                            }
                            ArithmeticOperandKind::SaturatingSubtract => {
                                ModuleError::SaturatingIntegerSubtractOperandTypeMismatch {
                                    operation: operation.id,
                                    operand,
                                    expected,
                                    actual,
                                }
                            }
                            ArithmeticOperandKind::WrappingMultiply => {
                                ModuleError::WrappingIntegerMultiplyOperandTypeMismatch {
                                    operation: operation.id,
                                    operand,
                                    expected,
                                    actual,
                                }
                            }
                            ArithmeticOperandKind::SaturatingMultiply => {
                                ModuleError::SaturatingIntegerMultiplyOperandTypeMismatch {
                                    operation: operation.id,
                                    operand,
                                    expected,
                                    actual,
                                }
                            }
                        });
                    }
                }
            }
            defined.insert(operation.result.id);
        }
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => {
                let target_block = blocks
                    .get(target)
                    .copied()
                    .ok_or(ModuleError::UnknownTargetBlock(*target))?;
                if target_block.parameters.len() != arguments.len() {
                    return Err(ModuleError::JumpArityMismatch {
                        edge: block.terminator.edge(),
                        expected: target_block.parameters.len(),
                        actual: arguments.len(),
                    });
                }
                for (argument, parameter) in arguments.iter().zip(&target_block.parameters) {
                    if !defined.contains(argument) {
                        return Err(ModuleError::ValueUsedBeforeDefinition(*argument));
                    }
                    let argument_type = value_types
                        .get(argument)
                        .copied()
                        .ok_or(ModuleError::UnknownValue(*argument))?;
                    if argument_type != parameter.scalar_type {
                        return Err(ModuleError::JumpTypeMismatch {
                            edge: block.terminator.edge(),
                            argument: argument_type,
                            parameter: parameter.scalar_type,
                        });
                    }
                }
                current = *target;
            }
            Terminator::Return { value, .. } => {
                if !defined.contains(value) {
                    return Err(ModuleError::ValueUsedBeforeDefinition(*value));
                }
                let value_type = value_types
                    .get(value)
                    .copied()
                    .ok_or(ModuleError::UnknownValue(*value))?;
                if value_type != machine.result.scalar_type {
                    return Err(ModuleError::ReturnTypeMismatch {
                        machine: machine.id,
                        value: value_type,
                        result: machine.result.scalar_type,
                    });
                }
                break;
            }
        }
    }

    if visited.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !visited.contains(block))
            .copied()
            .expect("different set lengths guarantee an unvisited block");
        return Err(ModuleError::UnreachableBlock(block));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum ArithmeticOperandKind {
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}

pub(crate) fn machine_value_types(
    machine: &TerminalMachine,
) -> impl Iterator<Item = (ValueId, ScalarType)> + '_ {
    machine
        .parameters
        .iter()
        .chain(std::iter::once(&machine.result))
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.operations.iter().map(|operation| &operation.result)),
        )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractClauseKind {
    Requires,
    Ensures,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleError {
    UnsupportedSemanticVersion(SemanticVersion),
    EmptyModule,
    DuplicateMachine(MachineId),
    DuplicateBlock(BlockId),
    DuplicateContract(ContractId),
    DuplicateOperation(OperationId),
    DuplicateEdge(EdgeId),
    DuplicateObligation(ObligationId),
    DuplicateValue(ValueId),
    DuplicatePlace(PlaceId),
    DuplicateClaim(ClaimId),
    DuplicateStructuralPlaceRoot {
        machine: MachineId,
        kind: psi_core::StructuralPlaceKind,
    },
    UnknownEntryMachine(MachineId),
    MachineHasNoBlocks(MachineId),
    UnknownEntryBlock {
        machine: MachineId,
        block: BlockId,
    },
    EntryBlockCannotHaveParameters(BlockId),
    ContractValueOutsideScope {
        contract: ContractId,
        clause: ContractClauseKind,
        value: ValueId,
    },
    StructuralPlacesRequireSemanticVersion {
        machine: MachineId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    ContentIdentityReshufflesRequireSemanticVersion {
        machine: MachineId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    ContentPartitionCompositionsRequireSemanticVersion {
        machine: MachineId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    ContentIdentityReshuffleHasNoProjections(ClaimId),
    NonCanonicalContentIdentityProjectionOrder(ClaimId),
    ContentIdentityReshuffleRequiresEntryParameter(ClaimId),
    ContentIdentityReshuffleRequiresCurrentResult(ClaimId),
    ContentIdentityCasePathRequiresSemanticVersion {
        claim: ClaimId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    DuplicateContentIdentityInput(ContentStructuralPlace),
    DuplicateContentIdentityOutput(ContentStructuralPlace),
    OverlappingContentIdentityInput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    OverlappingContentIdentityOutput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    ContentProjectionAlgebraMismatch(ContentProjectionIdentity),
    DuplicateContentPartitionComposition,
    ContentPartitionCompositionHasNoInputClaims,
    NonCanonicalContentPartitionInputClaims,
    NonCanonicalContentPartitionSubstitutions,
    DuplicateContentPartitionSubstitutionTarget,
    ContentPartitionAlgebraMismatch,
    ContentPartitionSourceHasNoSeparation,
    DuplicateContentPartitionSourcePlace(PlaceId),
    DuplicateContentPartitionSourceRoot(StructuralPlaceKind),
    InvalidContentPartitionSubstitutionShape,
    ContentPartitionSubstitutionCoverageMismatch,
    ContentPartitionReplayMismatch,
    ContentPartitionInputProjectionNotClaimBound(ContentStructuralPlace),
    ContentPartitionInputClaimNotListed(ClaimId),
    ContentPartitionInputClaimUnused,
    ContentPartitionCasePathRequiresSemanticVersion {
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    ContentConservationRequiresEnsures {
        contract: ContractId,
    },
    UnknownTargetBlock(BlockId),
    UnknownValue(ValueId),
    ValueUsedBeforeDefinition(ValueId),
    IntegerConstantRequiresIntegerResult(OperationId),
    IntegerConstantOutsideResultType(OperationId),
    BooleanConstantRequiresBooleanResult(OperationId),
    WrappingIntegerAddRequiresIntegerResult(OperationId),
    SaturatingIntegerAddRequiresIntegerResult(OperationId),
    WrappingIntegerSubtractRequiresIntegerResult(OperationId),
    SaturatingIntegerSubtractRequiresIntegerResult(OperationId),
    WrappingIntegerMultiplyRequiresIntegerResult(OperationId),
    SaturatingIntegerMultiplyRequiresIntegerResult(OperationId),
    WrappingIntegerAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    WrappingIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    WrappingIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    OperationRequiresSemanticVersion {
        operation: OperationId,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    PropositionRequiresSemanticVersion {
        contract: ContractId,
        clause: ContractClauseKind,
        required: SemanticVersion,
        actual: SemanticVersion,
    },
    JumpArityMismatch {
        edge: EdgeId,
        expected: usize,
        actual: usize,
    },
    JumpTypeMismatch {
        edge: EdgeId,
        argument: ScalarType,
        parameter: ScalarType,
    },
    ReturnTypeMismatch {
        machine: MachineId,
        value: ScalarType,
        result: ScalarType,
    },
    ControlCycle(BlockId),
    UnreachableBlock(BlockId),
    MalformedProposition(PropositionError),
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ModuleError {}
