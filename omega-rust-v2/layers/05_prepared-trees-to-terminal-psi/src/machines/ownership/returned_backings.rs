//! Resolve the caller storage needed by a loan carried out of a local call.
//! The published loan still points to the completed call. The storage roster
//! follows the selected body's returns, substituting the actual at each formal
//! slot and the exact case and field path within it.
use std::collections::{BTreeMap, BTreeSet};

use super::{Arrivals, CleanupContext, Point, field_type};
use crate::{
    BlockTerminator, CallTarget, EmbeddedLoanOrigin, EmbeddedLoanSource, LoweredData, LoweredMachine, LoweringTypes, MemberCallSelection, MemberRealization,
    OperationKind, OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection, ProjectedPlace, ReferenceAccess, ReturnEdge, TypeId, TypeShape,
    ValueOperand,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ResultPath {
    machine: usize,
    path: Vec<PlaceProjection>,
    ty: TypeId,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FormalLoan {
    slot: usize,
    path: Vec<PlaceProjection>,
    ty: TypeId,
}

#[derive(Default)]
struct Backings {
    local: Vec<PlaceId>,
    formals: BTreeSet<FormalLoan>,
}

/// Compute summaries on demand. A recursive result starts with no origins
/// and gains formal paths as finite chains to normal returns are discovered.
/// Each pass substitutes the current summaries, so recursion that permutes
/// or replaces the actuals can add paths on later passes. Unknown origins
/// poison dependent summaries. Inactive case paths contribute no origin.
pub(crate) struct ReturnBackings<'a> {
    machines: &'a [LoweredMachine],
    members: &'a [MemberCallSelection],
    types: &'a LoweringTypes,
    data: &'a [LoweredData],
    formals: BTreeMap<ResultPath, Option<BTreeSet<FormalLoan>>>,
}

impl<'a> ReturnBackings<'a> {
    pub(crate) fn new(
        machines: &'a [LoweredMachine],
        members: &'a [MemberCallSelection],
        types: &'a LoweringTypes,
        data: &'a [LoweredData],
    ) -> Self {
        Self {
            machines,
            members,
            types,
            data,
            formals: BTreeMap::new(),
        }
    }

    pub(super) fn local_backings<'b>(
        &mut self,
        arrivals: &mut Arrivals<'_>,
        origins: impl Iterator<Item = &'b EmbeddedLoanOrigin> + Clone,
    ) -> Option<Vec<PlaceId>> {
        loop {
            let demand_count = self.formals.len();
            let mut backings = Backings::default();
            for origin in origins.clone() {
                self.origin(arrivals, origin, &mut backings, false)?;
            }
            if demand_count == self.formals.len() {
                return Some(backings.local);
            }
            self.settle();
        }
    }

    fn settle(&mut self) {
        loop {
            let demands: Vec<_> = self.formals.keys().cloned().collect();
            let mut pass_changed = false;
            for demand in &demands {
                if self.formals[demand].is_none() {
                    continue;
                }
                let next = self.normal_returns(demand);
                if self.formals[demand] != next {
                    self.formals.insert(demand.clone(), next);
                    pass_changed = true;
                }
            }
            if !pass_changed && demands.len() == self.formals.len() {
                return;
            }
        }
    }

    fn normal_returns(
        &mut self,
        demand: &ResultPath,
    ) -> Option<BTreeSet<FormalLoan>> {
        let machine = self.machines.get(demand.machine)?;
        let body = &machine.body;
        let context = CleanupContext {
            places: &body.places,
            types: self.types,
            data: self.data,
            site: &body.diagnostics.body,
        };
        if field_type(machine.result_type?, &demand.path, &context)? != demand.ty {
            return None;
        }
        let mut arrivals = Arrivals::new(body, &context);
        let mut backings = Backings::default();
        let terminal_completions: Vec<_> = body
            .blocks
            .iter()
            .filter_map(|block| match block.terminator {
                BlockTerminator::TerminalCall { completion, .. } => completion,
                _ => None,
            })
            .collect();
        for (index, block) in body.blocks.iter().enumerate() {
            let at = arrivals.end_of(index);
            match &block.terminator {
                BlockTerminator::Return {
                    result: ReturnEdge::Structural { source, .. },
                    ..
                } => {
                    // A synthesized cleanup continuation returns the
                    // terminal call's completion parameter unchanged. The
                    // terminal edge below accounts for that same return.
                    if matches!(body.places[source.index()].origin, PlaceOrigin::BlockParameter(block)
                        if terminal_completions.contains(&block))
                    {
                        continue;
                    }
                    for origin in arrivals.value(*source, demand.path.clone(), demand.ty, at)? {
                        self.origin(&mut arrivals, &origin, &mut backings, true)?;
                    }
                }
                BlockTerminator::TerminalCall { callee, arguments, .. } => {
                    self.actuals(&mut arrivals, (*callee, arguments), demand.path.clone(), demand.ty, at, &mut backings)?;
                }
                BlockTerminator::Return { .. } => return None,
                _ => {}
            }
        }
        // Borrowing a by-value formal's own storage or a callee local does
        // not give the caller a surviving allocation. A referent reached
        // through a carried loan will already have become a formal path.
        backings.local.is_empty().then_some(backings.formals)
    }

    fn origin(
        &mut self,
        arrivals: &mut Arrivals<'_>,
        origin: &EmbeddedLoanOrigin,
        backings: &mut Backings,
        returning: bool,
    ) -> Option<()> {
        let body = arrivals.body;
        match &origin.source {
            EmbeddedLoanSource::Formal { formal, path } => {
                let slot = body
                    .parameters
                    .iter()
                    .position(|parameter| *parameter == Parameter::Structural(*formal))?;
                let ty = field_type(body.places.get(formal.index())?.ty, path, arrivals.context)?;
                backings
                    .formals
                    .insert(FormalLoan { slot, path: path.clone(), ty });
            }
            EmbeddedLoanSource::Borrow { operation } => {
                let OperationKind::BorrowPlace { source, .. } = &body.operations.get(operation.index())?.kind else {
                    return None;
                };
                backings.local.push(source.root);
            }
            EmbeddedLoanSource::CallResult { operation, result, path } => {
                let op = body.operations.get(operation.index())?;
                let OperationKind::Call { callee, arguments, .. } = &op.kind else {
                    return None;
                };
                if op.result != OperationResult::Structural(*result) {
                    return None;
                }
                let ty = field_type(body.places.get(result.index())?.ty, path, arrivals.context)?;
                let at = arrivals.point(operation.index())?;
                self.actuals(arrivals, (*callee, arguments), path.clone(), ty, at, backings)?;
            }
            // Existing crossing records leave writable origins to L6. A
            // returned field with this origin has no exact actual to substitute.
            EmbeddedLoanSource::Written { .. } if returning => return None,
            EmbeddedLoanSource::Written { .. } => {}
        }
        Some(())
    }

    fn actuals(
        &mut self,
        arrivals: &mut Arrivals<'_>,
        (target, arguments): (CallTarget, &[ValueOperand]),
        path: Vec<PlaceProjection>,
        ty: TypeId,
        at: Point,
        backings: &mut Backings,
    ) -> Option<()> {
        let machine = match target {
            CallTarget::Machine(id) => id,
            CallTarget::Member(slot) => match self.members.get(slot)?.target {
                MemberRealization::Conformance { machine, .. } => machine,
                MemberRealization::Boundary { .. } => return None,
            },
            CallTarget::Runtime { .. } => return None,
        };
        let demand = ResultPath {
            machine: machine.get() as usize - 1,
            path,
            ty,
        };
        let formals = self
            .formals
            .entry(demand)
            .or_insert_with(|| Some(BTreeSet::new()))
            .clone()?;
        for formal in formals {
            let reference_access = match self.types.get(formal.ty)? {
                TypeShape::ScalarReference { access, .. } | TypeShape::NominalReference { access, .. } => Some(*access),
                TypeShape::SharedQualifiedScalarReference { .. } | TypeShape::IndexedSharedReference(_) => Some(ReferenceAccess::Shared),
                _ => None,
            };
            let origins = match arguments.get(formal.slot)? {
                ValueOperand::CopyWhole(_) if reference_access.is_some_and(|access| access != ReferenceAccess::Shared) => {
                    return None;
                }
                ValueOperand::CopyWhole(place) | ValueOperand::MoveWhole(place) => arrivals.value(*place, formal.path, formal.ty, at)?,
                ValueOperand::Borrow { place, access } if formal.path.is_empty() && reference_access == Some(*access) => {
                    let Some((root, path, ty)) = borrowed_descriptor(arrivals, place)? else {
                        backings.local.push(place.root);
                        continue;
                    };
                    arrivals.value(root, path, ty, at)?
                }
                _ => return None,
            };
            for origin in &origins {
                self.origin(arrivals, origin, backings, true)?;
            }
        }
        Some(())
    }
}

/// A direct borrow can capture caller storage or reborrow it through a
/// descriptor. In the latter case follow the descriptor before the crossing
/// into its referent, rather than retaining its private carrier as backing.
fn borrowed_descriptor(
    arrivals: &Arrivals<'_>,
    place: &ProjectedPlace,
) -> Option<Option<(PlaceId, Vec<PlaceProjection>, TypeId)>> {
    for (index, step) in place.path.iter().enumerate() {
        match step {
            PlaceProjection::Referent | PlaceProjection::ViewElement { .. } => {
                // More than one crossed descriptor needs the reached
                // descriptor's own lineage and cannot be replaced by its parent.
                if place.path[index + 1..]
                    .iter()
                    .any(|step| matches!(step, PlaceProjection::Referent | PlaceProjection::ViewElement { .. }))
                {
                    return None;
                }
                let path = &place.path[..index];
                let ty = field_type(arrivals.body.places.get(place.root.index())?.ty, path, arrivals.context)?;
                return Some(Some((place.root, path.to_vec(), ty)));
            }
            _ => {}
        }
    }
    Some(None)
}
