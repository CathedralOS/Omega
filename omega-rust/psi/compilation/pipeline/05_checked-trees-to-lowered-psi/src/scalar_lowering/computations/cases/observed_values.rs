//! A computed unrestricted sum owns one retained result before tag observation.
//! Reservation names its whole value; preparation evaluates the ordinary call
//! exactly once at the observation, preserving effects and normal completion.
//! The observation reads only the tag. Payloads of every case stay in the
//! retained value and are discarded with it, which `[copy]` multiplicity
//! permits without cleanup; an affine or linear result is refused upstream.
use super::{
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument, CheckedTrees,
    Expansion, LoweredBooleanReturnExpression, LoweredDirectExpression, LoweredScalarBinding,
    LoweringError, QualifiedScalarType, Site, Slot, StructuralMultiplicity,
    StructuralTypeDeclaration, StructuralTypeShape, structural_case_identity, unsupported,
};

pub(super) fn reserve(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    node: &CheckedScalarComputationKind,
    types: &[StructuralTypeDeclaration],
    slots: &[Slot],
    next_place: &mut u64,
) -> Result<Option<Slot>, LoweringError> {
    let CheckedScalarComputationKind::CaseMembership {
        receiver_value,
        subject,
        case,
        ..
    } = node
    else {
        return Ok(None);
    };
    if !receiver_value.is_valid() {
        return Ok(None);
    }

    let CheckedScalarComputationStructuralArgument::Place(argument) = subject else {
        return unsupported("computed enum membership must observe its retained owner");
    };
    let values = &checked.facts.values.structural_values;
    let value = values.nodes.get(*receiver_value);
    if slots.iter().any(|slot| {
        slot.receiver
            .as_ref()
            .is_some_and(|(root, _)| root == receiver_value)
    }) {
        return Ok(None);
    }
    let root = unique_root(checked, *receiver_value)?;
    if !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
        || argument.type_identity != checked.normalized_type_identity(root.type_reference)
        || argument.source
            != (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression: value.expression,
            })
        || checked.type_multiplicity(root.type_reference)
            != language_semantics::Multiplicity::Unrestricted
    {
        return unsupported("computed enum membership disagrees with its retained owner");
    }
    let identity = checked.normalized_type_identity(root.type_reference);
    let declaration = crate::terminal_identities::structural_declaration_by_identity(
        types,
        identities,
        checked,
        identity,
        "computed enum membership has no carrier namespace",
    )?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return unsupported("computed enum membership requires a sum");
    };
    if cases.is_empty() {
        return unsupported("computed enum membership requires a case");
    }
    let identity = structural_case_identity(checked, *case)?;
    let (path, selected) = crate::expression_preparation::bindings::structural_cases::resolve_at(
        identities,
        types,
        declaration.id,
        &[],
        &identity,
    )?;
    if !path.is_empty() {
        return unsupported("computed enum membership selected a foreign case");
    }
    let storage = crate::scalar_lowering::branch_states::structural_values::Storage::reserve(
        identities,
        checked,
        *receiver_value,
        types,
        next_place,
    )?;
    Ok(Some(Slot {
        expression: value.expression,
        place: storage.place(*receiver_value)?,
        structural_type: declaration.id,
        multiplicity: StructuralMultiplicity::Unrestricted,
        case: selected,
        cases: Vec::new(),
        fields: Vec::new(),
        receiver: Some((*receiver_value, storage)),
    }))
}

pub(super) fn lower(
    expansion: &mut Expansion<'_>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    receiver_value: checked_trees::CheckedStructuralValueHandle,
    observed_case: symbols::SymbolHandle,
    input_types: &[QualifiedScalarType],
    target: usize,
    site: &Site<'_>,
) -> Result<usize, LoweringError> {
    let slot = expansion
        .cases
        .iter()
        .find(|slot| {
            slot.receiver
                .as_ref()
                .is_some_and(|(root, _)| *root == receiver_value)
        })
        .ok_or(LoweringError::Unsupported(
            "computed enum observation has no reserved owner",
        ))?
        .clone();
    let values = &expansion.checked.facts.values.structural_values;
    let value = values.nodes.get(receiver_value);
    let root = unique_root(expansion.checked, receiver_value)?;
    if root.state != site.state || root.statement_ordinal != site.statement {
        return unsupported("computed enum observation lost its source occurrence");
    }
    let prepared = crate::scalar_lowering::branch_states::structural_values::prepare_value(
        identities,
        expansion.checked,
        expansion.qualifications,
        site.state,
        facts::PlaceRoot::Expression(value.expression),
        site.statement,
        receiver_value,
        values.calls_for_root(receiver_value).unwrap_or(&[]),
        root.type_reference,
        site.bindings,
        input_types,
        expansion.structural_types,
        &slot
            .receiver
            .as_ref()
            .ok_or(LoweringError::Unsupported(
                "computed enum observation lost its storage",
            ))?
            .1,
    )?;
    let identity = structural_case_identity(expansion.checked, observed_case)?;
    let (path, case) = crate::expression_preparation::bindings::structural_cases::resolve_at(
        identities,
        expansion.structural_types,
        slot.structural_type,
        &[],
        &identity,
    )?;
    if !path.is_empty() {
        return unsupported("computed enum observation selected a foreign case");
    }
    let observation = expansion.binding(
        input_types,
        input_types.len(),
        target,
        LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean {
            expression: Box::new(LoweredBooleanReturnExpression::StructuralCaseMembership {
                source: prepared.place,
                path: Vec::new(),
                case,
            }),
        }),
    );
    prepared.finish(
        identities,
        site.state,
        site.bindings,
        input_types,
        observation,
        expansion,
    )
}

/// A retained receiver names one complete authored occurrence. Reservation,
/// namespace discovery and observation use the same unique source custody.
pub(super) fn unique_root(
    checked: &CheckedTrees,
    receiver: checked_trees::CheckedStructuralValueHandle,
) -> Result<&checked_trees::CheckedStructuralValueRoot, LoweringError> {
    let values = &checked.facts.values.structural_values;
    if !values.nodes.is_valid(receiver) {
        return unsupported("computed enum receiver has a stale value handle");
    }
    let value = values.nodes.get(receiver);
    let mut roots = values
        .roots
        .iter()
        .filter_map(|(_, root)| (root.root == receiver).then_some(root));
    let root = roots.next().ok_or(LoweringError::Unsupported(
        "computed enum receiver lost its value root",
    ))?;
    if roots.next().is_some()
        || root.machine != value.machine
        || root.state != value.state
        || root.statement_ordinal != value.statement_index
        || root.expression != value.expression
        || checked.normalized_type_identity(root.type_reference)
            != checked.normalized_type_identity(value.type_reference)
    {
        return unsupported("computed enum receiver has conflicting source custody");
    }
    Ok(root)
}
