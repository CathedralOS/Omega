//! Reserve each computed range's descriptor, then establish it at its formal
//! position. Reservation does not evaluate bounds or allocate element backing.
//! The existing subslice effect proves bounds against the original live view.
use super::{CheckedTrees, LoweringError, unsupported};
use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedScalarExpression, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Clone)]
pub(crate) struct Slot {
    pub(super) expression: ExpressionHandle,
    pub(super) place: PlaceId,
    pub(super) structural_type: StructuralTypeId,
    bytes: bool,
}

pub(super) fn range(
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Option<(
    checked_trees::CheckedStorageRoot,
    ExpressionHandle,
    &Option<CheckedScalarExpression>,
    &Option<CheckedScalarExpression>,
    bool,
)> {
    match &argument.source {
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root,
            expression,
            start,
            end,
        } => Some((*root, *expression, start, end, true)),
        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root,
            expression,
            start,
            end,
        } => Some((*root, *expression, start, end, false)),
        _ => None,
    }
}

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut slots: Vec<Slot> = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        let CheckedScalarComputationKind::Call {
            structural_arguments,
            ..
        } = plans.nodes.get(handle).kind
        else {
            continue;
        };
        for argument in plans
            .structural_arguments
            .span(structural_arguments)
            .ok_or(LoweringError::Unsupported(
                "computed range has stale structural operands",
            ))?
        {
            let CheckedScalarComputationStructuralArgument::Place(argument) = argument else {
                continue;
            };
            let Some((_, expression, _, _, bytes)) = range(argument) else {
                continue;
            };
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
            slots.push(Slot {
                expression,
                place: super::super::place_id(super::super::allocate_dense(next_place)?),
                structural_type: declaration.id,
                bytes,
            });
        }
    }
    Ok(slots)
}

pub(super) fn effect(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    site: checked_trees::CheckedSubsliceSite,
    argument: &CheckedUnitStructuralArgumentPlan,
    slot: &Slot,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
) -> Result<
    crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarEffect,
    LoweringError,
> {
    let (root, expression, start, end, bytes) = range(argument).ok_or(
        LoweringError::Unsupported("computed range lost its retained source"),
    )?;
    if slot.expression != expression || slot.bytes != bytes || !argument.path.is_empty() {
        return unsupported("computed range disagrees with its descriptor reservation");
    }
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let (symbol, source) = bindings.shared_view_source(
        root,
        checked.state_parameters(authored),
        checked,
        slot.structural_type,
        bytes,
    )?;
    let (start, end) = crate::unit::attached_unit::view_ranges::endpoints(
        checked,
        &crate::unit::attached_unit::view_ranges::ViewRangeSite {
            state,
            statement,
            site,
            expression,
            retained: Some((start, end)),
        },
        symbol,
        &[],
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
    Ok(if bytes {
        LoweredScalarEffect::ByteSequenceSubslice {
            source,
            start,
            end,
            place: slot.place,
            structural_type: slot.structural_type,
        }
    } else {
        LoweredScalarEffect::ElementViewSubslice {
            source,
            start,
            end,
            place: slot.place,
            structural_type: slot.structural_type,
        }
    })
}

pub(crate) fn declarations(
    operations: &[terminal_psi::Operation],
) -> impl Iterator<Item = terminal_psi::StructuralPlaceDeclaration> + '_ {
    operations.iter().filter_map(|operation| {
        if !matches!(
            operation.kind,
            terminal_psi::OperationKind::ByteSequenceSubslice { .. }
                | terminal_psi::OperationKind::ElementViewSubslice { .. }
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
