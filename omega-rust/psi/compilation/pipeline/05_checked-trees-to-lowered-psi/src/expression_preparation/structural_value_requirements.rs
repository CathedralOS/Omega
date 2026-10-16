//! Domain catalog dependencies of completed structural value occurrences.
//! Every producer retains the checker's exact judgments before its consumer
//! runs. This walk registers their theories, never membership or custody.
//! Byte literal formation remains a separate validation owner.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{CheckedTrees, CheckedUnitStructuralArgumentSourcePlan};
use language_semantics::SemanticDomainId;

/// Catalog qualifications and report whether construction needs a literal view.
/// Record children carry their declaration's type; selection arms keep the same
/// destination. This registers theories, while emission and execution
/// preserve the checked occurrence's actual memberships; byte validation stays separate.
pub(crate) fn collect_operation_requirements(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
    domains: &mut Vec<SemanticDomainId>,
) -> Result<bool, LoweringError> {
    let checked_trees::CheckedUnitEffectOperationPlan::EstablishLocal(
        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, value, .. },
    ) = operation
    else {
        return Ok(false);
    };
    let values = &checked.facts.values.structural_values;
    let mut roots = values.roots.iter().map(|(_, root)| root).filter(|root| {
        root.machine == machine
            && root.statement_ordinal == result.statement_index
            && root.root == *value
    });
    let reference = roots
        .next()
        .map(|root| root.type_reference)
        .unwrap_or_default();
    if roots.next().is_some() {
        return unsupported("structural domain catalog has ambiguous value roots");
    }
    collect_domains(checked, *value, reference, domains)
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
) -> Result<bool, LoweringError> {
    let reference = checked
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| root.machine == machine && root.root == value)
        .map(|root| root.type_reference)
        .unwrap_or_default();
    collect_domains(checked, value, reference, domains)
}

fn collect_domains(
    checked: &CheckedTrees,
    value: checked_trees::CheckedStructuralValueHandle,
    expected: TypeReferenceHandle,
    domains: &mut Vec<SemanticDomainId>,
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
        for call in plans.calls_for_root(value).unwrap_or(&[]) {
            for operation in call.operands().iter() {
                if let checked_trees::CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
                ) = operation
                {
                    let child = plans.nodes.get(*value);
                    pending.push((*value, child.type_reference));
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
