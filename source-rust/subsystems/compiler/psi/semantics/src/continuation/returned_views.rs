//! A view returned by a provider remains usable at a checked handoff only if
//! both programs identify the same completed invocation and still hold every
//! input that its lifetime depends on.

use super::{BoundaryPair, CheckedPrefix};
use crate::borrowed_storage::Projection;
use crate::descriptor_lifetimes::DescriptorLocation;
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorInvocationOwner, DescriptorProviderOrigin,
    DescriptorProviderOutput, DescriptorSourceOrigin, operation_position,
    terminal_borrowed_storage_graph_from,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, StructuralAccess, StructuralTypeShape, TargetMachineCallable,
    TerminalMachine, TerminalModule, TerminalSuspensionLiveValue, TerminalSuspensionPlace,
    TerminalSuspensionStorage, TerminalSuspensionValueLoans, TerminalSuspensionValueType,
};

type Provider = DescriptorProviderOrigin<DescriptorSourceOrigin>;
type AtCall<'a> = (
    &'a TerminalMachine,
    &'a Operation,
    &'a TerminalSuspensionLiveValue,
);

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct View {
    machine: MachineId,
    call: OperationId,
    place: PlaceId,
}

impl View {
    fn shared(module: &TerminalModule, (machine, call, value): AtCall<'_>) -> Option<Self> {
        let TerminalSuspensionPlace::Structural(arg) = &value.place else {
            return None;
        };
        let TerminalSuspensionValueType::Structural(ty) = value.value_type else {
            return None;
        };
        (arg.path.is_empty()
            && matches!(
                arg.access,
                StructuralAccess::Owned | StructuralAccess::SharedBorrow
            )
            && machine.structural_place_type(arg.place) == Some(ty)
            && matches!(
                module.structural_type(ty)?.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                })
            ))
        .then_some(Self {
            machine: machine.id,
            call: call.id,
            place: arg.place,
        })
    }
}

#[derive(Default)]
pub(super) struct ReachedInvocations {
    calls: Vec<(BoundaryPair, bool)>,
    views: Vec<(View, View)>,
}

impl ReachedInvocations {
    pub(super) fn record(&mut self, pair: BoundaryPair, boundary: bool) {
        self.calls.push((pair, boundary));
    }

    pub(super) fn require_returned_view(
        &mut self,
        prefix: &CheckedPrefix<'_>,
        old: AtCall<'_>,
        new: AtCall<'_>,
    ) -> bool {
        let Some(views) = View::shared(prefix.old, old).zip(View::shared(prefix.new, new)) else {
            return false;
        };
        self.views.push(views);
        true
    }

    pub(super) fn finish(
        mut self,
        prefix: &CheckedPrefix<'_>,
    ) -> Result<Vec<BoundaryPair>, &'static str> {
        self.calls.sort_unstable();
        self.calls.dedup();
        if !self.views.is_empty() {
            self.views.sort_unstable();
            self.views.dedup();
            let old = providers(prefix.old, self.views.iter().map(|pair| pair.0).collect())?;
            let new = providers(prefix.new, self.views.iter().map(|pair| pair.1).collect())?;
            for ((old, new), (a, b)) in old.iter().zip(&new).zip(&self.views) {
                if !self.same_origin(prefix, *a, old, *b, new) {
                    return Err(
                        "successor changed a returned view's producing call or its retained lender",
                    );
                }
            }
        }
        Ok(self
            .calls
            .into_iter()
            .filter_map(|(pair, boundary)| boundary.then_some(pair))
            .collect())
    }

    fn same_origin(
        &self,
        prefix: &CheckedPrefix<'_>,
        a: View,
        old: &Provider,
        b: View,
        new: &Provider,
    ) -> bool {
        let empty = Projection::new(&[]);
        if !matches!((&old.output, &new.output), (DescriptorProviderOutput::Result(x), DescriptorProviderOutput::Result(y)) if x.is_empty() && y.is_empty())
            || old.projection != empty
            || new.projection != empty
            || old.invocations.is_empty()
            || old.invocations.len() != new.invocations.len()
            || old.lifetime_dependencies != new.lifetime_dependencies
        {
            return false;
        }
        let mut owners = (
            TargetMachineCallable::Machine(a.machine),
            TargetMachineCallable::Machine(b.machine),
        );
        for (l, r) in old.invocations.iter().zip(&new.invocations) {
            let (DescriptorInvocationOwner::Operation(x), DescriptorInvocationOwner::Operation(y)) =
                (l.owner, r.owner)
            else {
                return false;
            };
            if owners
                != (
                    TargetMachineCallable::Machine(l.machine),
                    TargetMachineCallable::Machine(r.machine),
                )
                || self
                    .calls
                    .binary_search_by_key(&((l.machine, x), (r.machine, y)), |(pair, _)| *pair)
                    .is_err()
            {
                return false;
            }
            let callee = |module: &TerminalModule, machine: MachineId, op| {
                Some(
                    module
                        .machine(machine)?
                        .operation(op)?
                        .declared_invocation()?
                        .callee,
                )
            };
            let Some(next) = callee(prefix.old, l.machine, x).zip(callee(prefix.new, r.machine, y))
            else {
                return false;
            };
            owners = next;
        }
        owners
            == (
                TargetMachineCallable::Boundary(old.boundary),
                TargetMachineCallable::Boundary(new.boundary),
            )
    }
}

fn providers(module: &TerminalModule, views: Vec<View>) -> Result<Vec<Provider>, &'static str> {
    let mut roots = views.iter().map(|v| v.machine).collect::<Vec<_>>();
    roots.sort_unstable();
    roots.dedup();
    let graph = terminal_borrowed_storage_graph_from(module, roots, &[])
        .map_err(|_| "could not reconstruct the producing calls of a retained byte view")?;
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.types.structural_types, &graph);
    let mut origins = Vec::with_capacity(views.len());
    for group in views.chunk_by(|a, b| (a.machine, a.call) == (b.machine, b.call)) {
        let view = group[0];
        let machine = module
            .machine(view.machine)
            .ok_or("missing owner of a retained view")?;
        let call = machine
            .operation(view.call)
            .ok_or("missing call holding a retained view")?;
        let at = operation_position(machine, call).ok_or("missing position of a retained view")?;
        let plan =
            super::frontier::plan(module, call).ok_or("missing plan holding a retained view")?;
        // A provider may bind its result lifetime to an input without storing
        // the bytes inside that input. Each such lender must survive this call.
        let mut lenders = plan
            .live_values
            .iter()
            .filter_map(|row| {
                let TerminalSuspensionPlace::Structural(arg) = &row.place else {
                    return None;
                };
                (row.storage != TerminalSuspensionStorage::CallArgument
                    && row.loans == TerminalSuspensionValueLoans::ViewFree
                    && arg.path.is_empty()
                    && matches!(
                        arg.access,
                        StructuralAccess::Owned | StructuralAccess::SharedBorrow
                    ))
                .then_some(arg.place)
            })
            .collect::<Vec<_>>();
        lenders.sort_unstable();
        let whole = Projection::new(&[]);
        for view in group {
            let mut at_cut = analysis
                .symbolic_source_origins(view.machine, at, &DescriptorLocation::whole(view.place))
                .ok_or("the retained byte view has no checked provider origin")?;
            let Some(DescriptorSourceOrigin::Provider(provider)) = at_cut.pop() else {
                return Err("the retained byte view has no unique producing provider call");
            };
            if !at_cut.is_empty() {
                return Err("the retained byte view has more than one producing origin");
            }
            if !provider.lifetime_dependencies.iter().all(|debt| {
                matches!(&debt.origin, DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {source, projection})
                    if projection == &whole && lenders.binary_search(source).is_ok())
                    && debt.access == StructuralAccess::SharedBorrow
            }) {
                return Err("the provider's returned view lost its original checked lender");
            }
            origins.push(provider);
        }
    }
    Ok(origins)
}
