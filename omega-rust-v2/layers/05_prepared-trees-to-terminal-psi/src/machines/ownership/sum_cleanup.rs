//! A dying intact owned sum has an actual runtime discriminator. When one
//! alternative needs a cleanup invocation, place each payload's ordered
//! actions on its selected successor. Other roots keep their positions in
//! the normal return's reverse establishment order.
use super::*;
use crate::machines::body::next_id;
use crate::machines::expressions::{EmitOperation, OperationOutput};
use crate::{BlockId, BranchBlock, CaseSelection, DeclarationId, Place, PlaceOrigin, ReferenceAccess};

pub(super) enum Phase {
    Actions(Vec<CleanupAction>),
    Cases {
        source: ProjectedPlace,
        alternatives: Vec<(DeclarationId, Vec<CleanupAction>)>,
    },
}

/// A child that selects a cleanup machine requires an executable disposition
/// for the case that actually contains it.
fn contains_cleanup_call(
    schema: &LoweredData,
    context: &CleanupContext<'_>,
) -> bool {
    let mut pending: Vec<_> = schema
        .cases
        .iter()
        .flat_map(|case| &case.fields)
        .map(|field| field.carrier)
        .collect();
    let mut visited = std::collections::HashSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        if context.types.cleanup_target(ty, context.data).is_some() {
            return true;
        }
        match context.types.get(ty) {
            Some(TypeShape::FixedArray { element, length }) if *length != 0 => pending.push(*element),
            Some(TypeShape::Nominal(owner) | TypeShape::QualifiedNominal { data: owner, .. }) => {
                let data = &context.data[owner.get() - 1];
                pending.extend(
                    data.fields
                        .iter()
                        .chain(data.cases.iter().flat_map(|case| &case.fields))
                        .map(|field| field.carrier),
                );
            }
            _ => {}
        }
    }
    false
}

pub(super) fn return_plan(
    state: &Frontier,
    context: &CleanupContext<'_>,
) -> Result<Option<Vec<Phase>>, LoweringError> {
    let selected_sum = |root: &LiveRoot| {
        let ty = context.places[root.place.index()].ty;
        let (TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) = context.types.get(ty)? else {
            return None;
        };
        let schema = &context.data[id.get() - 1];
        (root.moved.is_empty()
            && schema.multiplicity == Multiplicity::Affine
            && context.types.cleanup_target(ty, context.data).is_none()
            && !schema.cases.is_empty()
            && contains_cleanup_call(schema, context))
        .then_some(schema)
    };
    if !state.0.iter().any(|root| selected_sum(root).is_some()) {
        return Ok(None);
    }
    let mut phases = Vec::new();
    let mut pending = Vec::new();
    for root in state.0.iter().rev() {
        if let Some(schema) = selected_sum(root) {
            if !pending.is_empty() {
                phases.push(Phase::Actions(std::mem::take(&mut pending)));
            }
            let mut alternatives = Vec::new();
            for case in &schema.cases {
                let mut actions = Vec::new();
                for field in case.fields.iter().rev() {
                    // Unrestricted scalar, array and record payloads carry no
                    // disposal. A descriptor still needs its own loan closure.
                    if (plain_copy_storage(field.carrier, context.types, context.data)
                        || matches!(context.types.get(field.carrier), Some(TypeShape::QualifiedScalar { .. })))
                        && ensure_disposable(field.carrier, context.types, context.data, context.site, LinearDebt::Refuse).is_ok()
                    {
                        continue;
                    }
                    let path = vec![
                        PlaceProjection::Case(case.declaration.clone()),
                        PlaceProjection::Field(field.declaration.clone()),
                    ];
                    context.at(root, path, field.carrier, &mut actions)?;
                }
                actions.push(CleanupAction::RetireHome(root.place));
                alternatives.push((case.declaration.clone(), actions));
            }
            phases.push(Phase::Cases {
                source: ProjectedPlace {
                    root: root.place,
                    path: vec![],
                },
                alternatives,
            });
        } else {
            context.at(root, vec![], context.places[root.place.index()].ty, &mut pending)?;
        }
    }
    if !pending.is_empty() {
        phases.push(Phase::Actions(pending));
    }
    Ok(Some(phases))
}

fn block_id(index: usize) -> BlockId {
    BlockId(next_id(index))
}

fn finish_block(
    body: &mut MachineBody,
    start: usize,
    terminator: BlockTerminator,
) {
    body.blocks.push(BranchBlock {
        id: block_id(body.blocks.len()),
        scalar_parameters: vec![],
        structural_parameters: vec![],
        scalar_domain_invariants: vec![],
        operations: start..body.operations.len(),
        terminator,
    });
}

fn selected_edge(
    body: &mut MachineBody,
    target: BlockId,
    source: &ProjectedPlace,
    alternative: (DeclarationId, Vec<CleanupAction>),
    site: &Site,
) -> crate::SuccessorEdge {
    let mut edge = branches::successor(body, target, vec![], site.clone());
    edge.selected_case = Some(CaseSelection {
        source: source.clone(),
        case: alternative.0,
    });
    edge.cleanup = alternative.1;
    edge
}

fn cases(
    body: &mut MachineBody,
    source: &ProjectedPlace,
    alternatives: Vec<(DeclarationId, Vec<CleanupAction>)>,
    bool_ty: TypeId,
    site: &Site,
) {
    let target = block_id(body.blocks.len() + alternatives.len().saturating_sub(1).max(1));
    let mut alternatives = alternatives.into_iter().peekable();
    while let Some(alternative) = alternatives.next() {
        let start = body.operations.len();
        if alternatives.peek().is_none() {
            let edge = selected_edge(body, target, source, alternative, site);
            finish_block(body, start, BlockTerminator::Jump(edge));
            break;
        }
        let OperationResult::Scalar(condition) = body.push_operation(
            OperationOutput::Scalar(bool_ty),
            OperationKind::StructuralCaseMembership {
                source: source.clone(),
                case: alternative.0.clone(),
            },
            site.clone(),
        ) else {
            unreachable!("case membership is Boolean")
        };
        let when_true = selected_edge(body, target, source, alternative, site);
        // On the last test the false arm is the only remaining declared case.
        let when_false = if alternatives.len() == 1 {
            selected_edge(body, target, source, alternatives.next().unwrap(), site)
        } else {
            branches::successor(body, block_id(body.blocks.len() + 1), vec![], site.clone())
        };
        finish_block(
            body,
            start,
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            },
        );
    }
}

pub(super) fn emit(
    body: &mut MachineBody,
    original: usize,
    phases: Vec<Phase>,
    types: &LoweringTypes,
    data: &[LoweredData],
) -> Result<(), LoweringError> {
    let BlockTerminator::Return { id, mut result, .. } = body.blocks[original].terminator else {
        unreachable!("a planned normal return")
    };
    let site = body.diagnostics.edges[id.get() - 1].clone();
    let bool_ty = types
        .id_for(&omega_typed::ValueType::Bool)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    let first = body.blocks.len();
    let mut enter = branches::successor(body, block_id(first), vec![], site.clone());
    // Capture a structural result before any cleanup. The immutable scalar
    // result already names its evaluated SSA value throughout these blocks.
    let mut captured = None;
    if let ReturnEdge::Structural { source, result: output } = result {
        let ty = body.places[source.index()].ty;
        let exclusive = matches!(
            types.get(ty),
            Some(
                TypeShape::MutableElementView { .. }
                    | TypeShape::ScalarReference {
                        access: ReferenceAccess::Mutable | ReferenceAccess::WriteOnly,
                        ..
                    }
                    | TypeShape::NominalReference {
                        access: ReferenceAccess::Mutable | ReferenceAccess::WriteOnly,
                        ..
                    }
            )
        );
        enter
            .structural_arguments
            .push(if exclusive || owned(ty, types, data) {
                CallArgument::MoveWhole(source)
            } else {
                CallArgument::CopyWhole(source)
            });
        let place = PlaceId(next_id(body.places.len()));
        body.places.push(Place {
            ty,
            origin: PlaceOrigin::BlockParameter(block_id(first)),
        });
        captured = Some(place);
        result = ReturnEdge::Structural { source: place, result: output };
    }
    body.blocks[original].terminator = BlockTerminator::Jump(enter);
    for phase in phases {
        match phase {
            Phase::Actions(cleanup) => {
                let mut edge = branches::successor(body, block_id(body.blocks.len() + 1), vec![], site.clone());
                edge.cleanup = cleanup;
                finish_block(body, body.operations.len(), BlockTerminator::Jump(edge));
            }
            Phase::Cases { source, alternatives } => cases(body, &source, alternatives, bool_ty, &site),
        }
    }
    if let Some(place) = captured {
        body.blocks[first].structural_parameters.push(place);
    }
    finish_block(body, body.operations.len(), BlockTerminator::Return { id, result, cleanup: vec![] });
    Ok(())
}
