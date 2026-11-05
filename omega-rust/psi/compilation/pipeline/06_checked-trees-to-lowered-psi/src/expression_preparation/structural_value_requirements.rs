//! Domain catalog dependencies of completed structural value occurrences.
//! Every producer retains the checker's exact judgments before its consumer
//! runs. This walk registers their theories, never membership or custody.
//! Byte literal formation remains a separate validation owner.

use crate::shared::checked_facts::value_facts;
use crate::shared::lowering_error::{LoweringError, unsupported};
use crate::shared::terminal_identities::stored_structural_identity;
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{CheckedTrees, CheckedUnitStructuralArgumentSourcePlan};
use language_semantics::SemanticDomainId;

/// Pointer formations consume the completed source's standing contract.
/// Prepare carrier roles once for domain publication. Literal-view discovery
/// uses the same value walk without computing contracts it will not publish.
pub(crate) struct ReferenceContractCatalog {
    carrier_roles: Vec<ReferenceCarrierRole>,
}

#[derive(Clone, Copy, Default)]
enum ReferenceCarrierRole {
    #[default]
    Absent,
    Pointer,
    Other,
    Ambiguous,
}

impl ReferenceContractCatalog {
    pub(crate) fn new(checked: &CheckedTrees) -> Self {
        let mut carrier_roles = Vec::new();
        for carrier in &checked.facts.flow.terminal_unit_effects.structural_types {
            let Some(index) = carrier.identity.storage_index() else {
                continue;
            };
            if carrier_roles.len() <= index {
                carrier_roles.resize(index + 1, ReferenceCarrierRole::Absent);
            }
            let role = &mut carrier_roles[index];
            *role = if !matches!(role, ReferenceCarrierRole::Absent) {
                ReferenceCarrierRole::Ambiguous
            } else if matches!(
                carrier.shape,
                checked_trees::CheckedUnitStructuralTypeShape::Reference { .. }
            ) {
                ReferenceCarrierRole::Pointer
            } else {
                ReferenceCarrierRole::Other
            };
        }
        Self { carrier_roles }
    }

    fn is_pointer(
        &self,
        identity: typed_trees::type_identity::TypeIdentityId,
    ) -> Result<bool, LoweringError> {
        match identity
            .storage_index()
            .and_then(|index| self.carrier_roles.get(index))
        {
            Some(ReferenceCarrierRole::Pointer) => Ok(true),
            Some(ReferenceCarrierRole::Other) => Ok(false),
            Some(ReferenceCarrierRole::Ambiguous) => {
                unsupported("reference domain catalog has ambiguous result carriers")
            }
            Some(ReferenceCarrierRole::Absent) | None => {
                unsupported("reference domain catalog lost its result carrier")
            }
        }
    }
}

/// Catalog qualifications and report whether construction needs a literal view.
/// Record children carry their declaration's type; selection arms keep the same
/// destination. This registers theories, while emission and execution
/// preserve the checked occurrence's actual memberships; byte validation stays separate.
pub(crate) fn collect_operation_requirements(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
    domains: &mut Vec<SemanticDomainId>,
    reference_contracts: Option<&ReferenceContractCatalog>,
) -> Result<bool, LoweringError> {
    if let Some(reference_contracts) = reference_contracts
        && let checked_trees::CheckedUnitEffectOperationPlan::EstablishReference {
            backing_type,
            storage,
            result,
            ..
        } = operation
    {
        collect_reference_contract(
            checked,
            result.type_identity,
            *backing_type,
            *storage,
            domains,
            reference_contracts,
        )?;
    }
    let Some((value, _)) = operation.structural_value_evaluation() else {
        return Ok(false);
    };
    let statement = operation
        .source_statement_index()
        .ok_or(LoweringError::Unsupported(
            "structural value evaluation has no source statement",
        ))?;
    let values = &checked.facts.values.structural_values;
    let mut roots = values.roots.iter().map(|(_, root)| root).filter(|root| {
        root.machine == machine && root.statement_ordinal == statement && root.root == value
    });
    let reference = roots
        .next()
        .map(|root| root.type_reference)
        .unwrap_or_default();
    if roots.next().is_some() {
        return unsupported("structural domain catalog has ambiguous value roots");
    }
    collect_domains(checked, value, reference, domains, reference_contracts)
}

/// Catalog the memberships a structural value tree establishes when a
/// computed operand constructs it rather than an operation of the body
/// (`observe(Envelope { line: main.line, .. })` as a scalar call argument),
/// and retain whether its construction needs an immutable literal carrier.
pub(crate) fn collect_value_requirements(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    value: checked_trees::CheckedStructuralValueHandle,
    domains: &mut Vec<SemanticDomainId>,
    reference_contracts: Option<&ReferenceContractCatalog>,
) -> Result<bool, LoweringError> {
    let reference = value_facts(checked)
        .structural_values
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| root.machine == machine && root.root == value)
        .map(|root| root.type_reference)
        .unwrap_or_default();
    collect_domains(checked, value, reference, domains, reference_contracts)
}

fn collect_domains(
    checked: &CheckedTrees,
    value: checked_trees::CheckedStructuralValueHandle,
    expected: TypeReferenceHandle,
    domains: &mut Vec<SemanticDomainId>,
    reference_contracts: Option<&ReferenceContractCatalog>,
) -> Result<bool, LoweringError> {
    use checked_trees::{CheckedStructuralRecordFieldValue, CheckedStructuralValueKind};
    let plans = &checked.facts.values.structural_values;
    let mut pending = vec![(value, expected)];
    let mut visited = Vec::new();
    let mut requires_literal_view = false;
    while let Some((value, reference)) = pending.pop() {
        if !plans.nodes.is_valid(value) {
            return unsupported("structural domain catalog has stale construction");
        }
        if visited.contains(&(value, reference)) {
            continue;
        }
        visited.push((value, reference));
        let node = plans.nodes.get(value);
        // A call result depends on its retained operand producers as well as
        // its result shape. Their formations can require theories absent from
        // every published parameter (for example a qualified literal field).
        // Follow checked schedule edges, never unrelated arena occurrences.
        for call in plans
            .call_schedule(value)
            .map(|(_, calls)| calls)
            .unwrap_or(&[])
        {
            for operation in call.operands().iter() {
                if let Some((value, _)) = operation.structural_value_evaluation() {
                    let child = plans.nodes.get(value);
                    pending.push((value, child.type_reference));
                }
            }
        }
        // Every completed occurrence uses its checked membership judgments.
        // Their catalog dependencies are independent of whether the producer
        // is a literal, copy or fresh aggregate. Do not infer a theory from the
        // receiving annotation or require a signature to register it first.
        for decision in checked
            .facts
            .qualifications
            .value_formations
            .span(node.qualification_decisions)
            .ok_or(LoweringError::Unsupported(
                "structural value membership decisions are stale",
            ))?
        {
            if !domains.contains(&decision.semantic_domain) {
                domains.push(decision.semantic_domain);
            }
        }
        match &node.kind {
            CheckedStructuralValueKind::Reference {
                backing_type,
                source_value,
                storage,
                ..
            } => {
                if let Some(reference_contracts) = reference_contracts {
                    collect_reference_contract(
                        checked,
                        stored_structural_identity(checked, node.type_reference),
                        *backing_type,
                        *storage,
                        domains,
                        reference_contracts,
                    )?;
                }
                if source_value.is_valid() {
                    if !plans.nodes.is_valid(*source_value) {
                        return unsupported("reference domain catalog has stale source value");
                    }
                    let child = plans.nodes.get(*source_value);
                    pending.push((*source_value, child.type_reference));
                }
            }
            CheckedStructuralValueKind::ZeroedStructural {
                projected_qualifications,
            } => {
                for row in projected_qualifications {
                    if !domains.contains(&row.domain) {
                        domains.push(row.domain);
                    }
                }
            }
            CheckedStructuralValueKind::ByteLiteralView { .. } => {
                requires_literal_view = true;
            }
            CheckedStructuralValueKind::Place(argument)
                if matches!(
                    argument.source,
                    CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
                ) =>
            {
                requires_literal_view = true;
                for domain in super::byte_literals::validate(checked, reference, argument)? {
                    if !domains.contains(&domain) {
                        domains.push(domain);
                    }
                }
            }
            checked_trees::record_or_structural_case_pattern!(fields, ..) => {
                for field in plans
                    .record_fields
                    .span(*fields)
                    .ok_or(LoweringError::Unsupported(
                        "structural domain catalog has stale fields",
                    ))?
                {
                    if let CheckedStructuralRecordFieldValue::Structural(child) = field.value {
                        pending.push((child, field.type_reference));
                    }
                }
            }
            CheckedStructuralValueKind::Dispatch { arms, .. } => {
                for arm in plans
                    .dispatch_arms
                    .span(*arms)
                    .ok_or(LoweringError::Unsupported(
                        "structural domain catalog has stale arms",
                    ))?
                {
                    pending.push((arm.value, reference));
                }
            }
            CheckedStructuralValueKind::FixedArray { elements } => {
                let element = if checked
                    .type_reference_table
                    .contains_type_reference(reference)
                    && let TypeReferenceNode::FixedArray { element_type, .. } =
                        checked.type_reference_table.type_reference(reference)
                {
                    *element_type
                } else {
                    TypeReferenceHandle::default()
                };
                pending.extend(
                    elements
                        .iter()
                        .map(|element_value| (*element_value, element)),
                );
            }
            CheckedStructuralValueKind::ViewSubslice { source, .. } => {
                let child = plans.nodes.get(*source);
                pending.push((*source, child.type_reference));
            }
            CheckedStructuralValueKind::Projection { source, .. } => {
                if !plans.nodes.is_valid(*source) {
                    return unsupported("structural domain catalog has stale projection source");
                }
                // Owned projections materialize their source. Copied-place
                // sources are provenance for a leaf copy, not a new producer.
                if !matches!(
                    plans.nodes.get(*source).kind,
                    CheckedStructuralValueKind::CopiedStructuralPlace { .. }
                ) {
                    let source_reference = plans
                        .roots
                        .iter()
                        .map(|(_, root)| root)
                        .find(|root| root.root == *source)
                        .map(|root| root.type_reference)
                        .unwrap_or_default();
                    pending.push((*source, source_reference));
                }
            }
            _ => {}
        }
    }
    Ok(requires_literal_view)
}

/// Reference formation consumes the completed source's standing contract,
/// even when no signature names its domains. Register those same theories;
/// this dependency walk establishes no membership on the source or its home.
fn collect_reference_contract(
    checked: &CheckedTrees,
    result_type: typed_trees::type_identity::TypeIdentityId,
    backing_type: TypeReferenceHandle,
    storage: checked_trees::CheckedReferenceStorage,
    domains: &mut Vec<SemanticDomainId>,
    reference_contracts: &ReferenceContractCatalog,
) -> Result<(), LoweringError> {
    // Access views retain their separate profile/owner interpretation. Only
    // the ordinary and explicit representation-view formations below consume
    // this declared payload inventory.
    if !matches!(
        storage,
        checked_trees::CheckedReferenceStorage::Declared
            | checked_trees::CheckedReferenceStorage::View { .. }
    ) {
        return Ok(());
    }
    // The emitted pointer consumes this contract. Collection views use their
    // own element/byte membership producers, even for Declared storage. Read
    // that distinction from the retained result shape, not the source syntax.
    if !reference_contracts.is_pointer(result_type)? {
        return Ok(());
    }
    if !backing_type.is_valid() {
        return unsupported("reference domain catalog lost its supplying type");
    }
    let contract =
        validation::structural_result_qualification_contract(&checked.typed, backing_type)
            .map_err(LoweringError::Unsupported)?;
    for (domain, _) in contract.domains {
        if !domains.contains(&domain) {
            domains.push(domain);
        }
    }
    Ok(())
}
