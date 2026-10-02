//! Reserve each computed range's descriptor, then establish it at its formal
//! position. Reservation does not evaluate bounds or allocate element backing.
//! The existing subslice effect proves bounds against the original live view.
use super::{CheckedTrees, LoweringError, unsupported};
use crate::unit::attached_unit::view_ranges;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedUnitStructuralArgumentPlan,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Clone)]
pub(crate) struct Slot {
    pub(super) expression: ExpressionHandle,
    pub(super) place: PlaceId,
    pub(super) structural_type: StructuralTypeId,
    bytes: bool,
    storage: view_ranges::RangeStorage,
    whole_view: Option<PlaceId>,
}

pub(super) use crate::unit::attached_unit::view_ranges::range;

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machines: &[symbols::SymbolHandle],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut slots: Vec<Slot> = Vec::new();
    for (_, root) in plans
        .roots
        .iter()
        .filter(|(_, root)| machines.contains(&root.machine))
    {
        for handle in super::reachable_nodes(checked, &[root.root])? {
            let arguments: Vec<_> = match &plans.nodes.get(handle).kind {
                CheckedScalarComputationKind::Call {
                    structural_arguments,
                    ..
                } => plans
                    .structural_arguments
                    .span(*structural_arguments)
                    .ok_or(LoweringError::Unsupported(
                        "computed range has stale structural operands",
                    ))?
                    .iter()
                    .filter_map(CheckedScalarComputationStructuralArgument::as_place)
                    .collect(),
                CheckedScalarComputationKind::ByteSequenceLiteralComparison { subject, .. }
                | CheckedScalarComputationKind::ByteSequencePredicate { subject, .. } => {
                    vec![subject]
                }
                _ => Vec::new(),
            };
            for argument in arguments {
                let Some(range) = range(argument) else {
                    continue;
                };
                let expression = range.expression;
                let bytes = range.family
                    == crate::emission::operation_emission::view_subslice::ViewFamily::Bytes;
                let declaration = types
                    .iter()
                    .find(|declaration| declaration.identity == argument.type_identity)
                    .ok_or(LoweringError::Unsupported(
                        "computed range has no structural type namespace",
                    ))?;
                if !matches!(
                    (&declaration.shape, bytes),
                    (
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. }),
                        true
                    ) | (StructuralTypeShape::ElementView { .. }, false)
                ) {
                    return unsupported("computed range changed its view family");
                }
                if let Some(existing) = slots.iter().find(|slot| slot.expression == expression) {
                    if existing.structural_type != declaration.id || existing.bytes != bytes {
                        return unsupported("computed range occurrence has conflicting types");
                    }
                    continue;
                }
                let storage = view_ranges::storage(
                    checked,
                    root.state,
                    root.statement_ordinal,
                    argument,
                    types,
                )?;
                let whole_view = storage
                    .requires_establishment()
                    .then(|| super::super::allocate_dense(next_place).map(super::super::place_id))
                    .transpose()?;
                slots.push(Slot {
                    expression,
                    place: super::super::place_id(super::super::allocate_dense(next_place)?),
                    structural_type: declaration.id,
                    bytes,
                    storage,
                    whole_view,
                });
            }
        }
    }
    Ok(slots)
}

pub(super) fn effects(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    site: checked_trees::CheckedSubsliceSite,
    argument: &CheckedUnitStructuralArgumentPlan,
    slot: &Slot,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
) -> Result<
    Vec<crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarEffect>,
    LoweringError,
> {
    let range = range(argument).ok_or(LoweringError::Unsupported(
        "computed range lost its retained source",
    ))?;
    let (expression, start, end) = (range.expression, range.start, range.end);
    let bytes =
        range.family == crate::emission::operation_emission::view_subslice::ViewFamily::Bytes;
    if slot.expression != expression || slot.bytes != bytes {
        return unsupported("computed range disagrees with its descriptor reservation");
    }
    let (source, establishment) = slot
        .storage
        .bind(checked, state, bindings, slot.whole_view)?;
    let (start, end) = crate::unit::attached_unit::view_ranges::endpoints(
        checked,
        &crate::unit::attached_unit::view_ranges::ViewRangeSite {
            state,
            statement,
            site,
            expression,
            retained: Some((start, end)),
        },
        source.symbol,
        &argument.path,
        bindings,
    )?;
    let start = start.unwrap_or(
        crate::emission::operation_emission::expressions::LoweredDirectExpression::IntegerLiteral {
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
            scalar_type: super::super::terminal_scalar_type(
                checked_trees::types::PrimitiveType::U64,
            )?,
        },
    );
    use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarEffect;
    let mut effects = Vec::with_capacity(2);
    if let Some(establishment) = establishment {
        effects.push(LoweredScalarEffect::EstablishElementView(establishment));
    }
    effects.push(if bytes {
        LoweredScalarEffect::ByteSequenceSubslice {
            source: source.place,
            access: source.access,
            start,
            end,
            place: slot.place,
            structural_type: slot.structural_type,
        }
    } else {
        LoweredScalarEffect::ElementViewSubslice {
            source: source.place,
            access: source.access,
            start,
            end,
            place: slot.place,
            structural_type: slot.structural_type,
        }
    });
    Ok(effects)
}

pub(crate) fn declarations(
    operations: &[terminal_psi::Operation],
) -> impl Iterator<Item = terminal_psi::StructuralPlaceDeclaration> + '_ {
    operations.iter().filter_map(|operation| {
        if !matches!(
            operation.kind,
            terminal_psi::OperationKind::ByteSequenceSubslice { .. }
                | terminal_psi::OperationKind::ElementViewSubslice { .. }
                | terminal_psi::OperationKind::EstablishElementView { .. }
                | terminal_psi::OperationKind::EstablishMutableElementView { .. }
        ) {
            return None;
        }
        let terminal_psi::OperationResult::Structural(result) = &operation.result else {
            return None;
        };
        Some(terminal_psi::StructuralPlaceDeclaration {
            id: result.place,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation.id,
                structural_type: result.structural_type,
            },
        })
    })
}
