//! Name what backs the one view a caller parks, reading only published Psi.
//!
//! A parked view can reach the crossing from three places: an incoming formal,
//! a boundary call whose result lifetime follows one of its operands, and a
//! boundary call whose result lifetime each caller chooses. Only a boundary's
//! provider establishes the bytes of the last two
//! (`wiki/spec/build/foreign_storage.md#provider-owned-views-and-completion`).
//! This reader keeps the providers apart from the inputs, so a caller-chosen
//! result shows up as an invocation with no input instead of borrowing one.
use omega_psi::{
    BlockTerminator, BoundaryResultLifetime, CallArgument, CallSite, CallTarget, LoweredMachine, LoweredPackage, MachineId, MemberRealization, OperationKind,
    OperationResult, Parameter, PlaceId, PlaceOrigin, ReferenceAccess, ReturnEdge, TypeShape,
};

use super::machine;

/// A boundary invocation whose admitted provider establishes a parked view's bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderInvocation {
    /// The machine whose body contains the invocation.
    pub machine: MachineId,
    pub call: CallSite,
    /// Index into `LoweredPackage::member_calls`.
    pub member: usize,
    pub lifetime: BoundaryResultLifetime,
}

/// What a parked view depends on.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewSources {
    /// Formals of the reading machine whose bytes or lifetime the view depends on, ascending.
    pub inputs: Vec<usize>,
    /// Boundary invocations whose provider may establish the bytes, in the order reached.
    pub providers: Vec<ProviderInvocation>,
}

impl ViewSources {
    fn merge(
        &mut self,
        other: ViewSources,
    ) {
        self.inputs.extend(other.inputs);
        self.inputs.sort_unstable();
        self.inputs.dedup();
        for provider in other.providers {
            if !self.providers.contains(&provider) {
                self.providers.push(provider);
            }
        }
    }
}

/// Name the sources of the one view `caller` retains at its only parked call.
/// A view-returning machine's summary starts at every formal of the result's
/// view type, as its elided result relation allows, and keeps the formals its
/// returns reach. Providers join a summary when a return reaches one. The
/// reader follows formals, completed calls, terminal calls and boundary member
/// results, and refuses any other form, or a boundary view without a
/// published lifetime relation, instead of guessing.
pub fn parked_view_sources(
    package: &LoweredPackage,
    caller: &str,
) -> Result<ViewSources, &'static str> {
    let caller = machine(package, caller).ok_or("missing caller")?;
    let body = &caller.body;
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one parked call");
    };
    let [loan] = site.retained_loans.as_slice() else {
        return Err("expected one retained loan");
    };
    if loan.access != ReferenceAccess::Shared || !loan.needed_by_callee || loan.needed_after_completion || !is_view(package, loan.ty) {
        return Err("wrong retained loan access, role or type");
    }
    let parked = match site.call {
        CallSite::Cleanup(_) => return Err("expected a parked call that takes the retained view"),
        CallSite::Operation(id) => match body.operations.get(id.index()).map(|op| &op.kind) {
            Some(OperationKind::Call { callee, arguments, .. }) => (*callee, arguments.as_slice()),
            _ => return Err("missing parked call"),
        },
        CallSite::Edge(id) => match body
            .blocks
            .iter()
            .map(|b| &b.terminator)
            .find(|t| matches!(t, BlockTerminator::TerminalCall { id: e, .. } if *e == id))
        {
            Some(BlockTerminator::TerminalCall { callee, arguments, .. }) => (*callee, arguments.as_slice()),
            _ => return Err("missing parked terminal call"),
        },
    };
    if parked.0 != site.callee || !parked.1.contains(&CallArgument::CopyWhole(loan.descriptor)) {
        return Err("the parked call must use the retained descriptor");
    }
    if body.places.get(loan.descriptor.index()).map(|p| p.ty) != Some(loan.ty) {
        return Err("wrong retained descriptor type");
    }

    let mut summaries: Vec<Option<ViewSources>> = package
        .machines
        .iter()
        .map(|m| {
            let result = m.result_type.filter(|&ty| is_view(package, ty))?;
            let inputs = (m.body.parameters.iter().enumerate())
                .filter(|(_, p)| matches!(p, Parameter::Structural(f) if m.body.places[f.index()].ty == result))
                .map(|(slot, _)| slot)
                .collect();
            Some(ViewSources { inputs, providers: Vec::new() })
        })
        .collect();
    // The formal sets only shrink and the provider lists only grow, so a
    // well-formed package settles. Refuse rather than loop on a malformed one.
    let mut rounds = 0;
    loop {
        let mut changed = false;
        for (index, m) in package.machines.iter().enumerate() {
            if summaries[index].is_none() {
                continue;
            }
            let reader = Reader {
                package,
                machine: m,
                summaries: &summaries,
            };
            let next = reader.returns()?;
            if summaries[index].as_ref() != Some(&next) {
                summaries[index] = Some(next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
        rounds += 1;
        if rounds > 4 * package.machines.len() + 4 {
            return Err("the view summaries did not settle");
        }
    }
    Reader {
        package,
        machine: caller,
        summaries: &summaries,
    }
    .place(loan.descriptor)
}

fn is_view(
    package: &LoweredPackage,
    ty: omega_psi::TypeId,
) -> bool {
    matches!(package.types.get(ty), Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. }))
}

struct Reader<'a> {
    package: &'a LoweredPackage,
    machine: &'a LoweredMachine,
    summaries: &'a [Option<ViewSources>],
}

impl Reader<'_> {
    /// The sources every normal and terminal return of this machine reaches.
    fn returns(&self) -> Result<ViewSources, &'static str> {
        let mut sources = ViewSources::default();
        for block in &self.machine.body.blocks {
            match &block.terminator {
                BlockTerminator::Return {
                    result: ReturnEdge::Structural { source, .. },
                    ..
                } => sources.merge(self.place(*source)?),
                BlockTerminator::TerminalCall { id, callee, arguments, .. } => sources.merge(self.call(CallSite::Edge(*id), *callee, arguments)?),
                BlockTerminator::Conditional { .. } | BlockTerminator::Jump(_) => {}
                _ => return Err("unsupported view-returning terminator"),
            }
        }
        Ok(sources)
    }

    fn place(
        &self,
        place: PlaceId,
    ) -> Result<ViewSources, &'static str> {
        let body = &self.machine.body;
        match body.places.get(place.index()).ok_or("missing place")?.origin {
            PlaceOrigin::Parameter => {
                let formal = (body.parameters.iter())
                    .position(|p| *p == Parameter::Structural(place))
                    .ok_or("missing formal")?;
                Ok(ViewSources {
                    inputs: vec![formal],
                    providers: Vec::new(),
                })
            }
            PlaceOrigin::Operation(id) => {
                let op = body.operations.get(id.index()).ok_or("missing operation")?;
                match &op.kind {
                    OperationKind::Call { callee, arguments, .. } if op.result == OperationResult::Structural(place) => {
                        self.call(CallSite::Operation(id), *callee, arguments)
                    }
                    _ => Err("unsupported returned view operation"),
                }
            }
            _ => Err("unsupported returned view origin"),
        }
    }

    fn actual(
        &self,
        arguments: &[CallArgument],
        slot: usize,
    ) -> Result<ViewSources, &'static str> {
        match arguments.get(slot) {
            Some(CallArgument::CopyWhole(actual)) => self.place(*actual),
            _ => Err("a view source needs a shared descriptor actual"),
        }
    }

    fn call(
        &self,
        site: CallSite,
        callee: CallTarget,
        arguments: &[CallArgument],
    ) -> Result<ViewSources, &'static str> {
        let local = match callee {
            CallTarget::Machine(id) => id,
            CallTarget::Member(slot) => {
                let selection = self
                    .package
                    .member_calls
                    .get(slot)
                    .ok_or("missing member selection")?;
                match selection.target {
                    MemberRealization::Conformance { machine, .. } => machine,
                    MemberRealization::Boundary { .. } => {
                        if !selection.result.is_some_and(|ty| is_view(self.package, ty)) {
                            return Err("a boundary source must return a shared view");
                        }
                        let lifetime = selection
                            .boundary_result_lifetime
                            .ok_or("boundary view result has no published lifetime relation")?;
                        let mut sources = ViewSources {
                            inputs: Vec::new(),
                            providers: vec![ProviderInvocation {
                                machine: self.machine.id,
                                call: site,
                                member: slot,
                                lifetime,
                            }],
                        };
                        if let BoundaryResultLifetime::Input(position) = lifetime {
                            // The anchor carries the validity interval, not the bytes.
                            let anchor = self.actual(arguments, position)?;
                            if !anchor.providers.is_empty() {
                                return Err("a lifetime anchor must trace to incoming formals");
                            }
                            sources.inputs = anchor.inputs;
                        }
                        return Ok(sources);
                    }
                }
            }
            CallTarget::Runtime { .. } => return Err("a runtime callee has no published view summary"),
        };
        let summary = self.summaries[(local.get() - 1) as usize]
            .as_ref()
            .ok_or("the callee returns no followable view")?;
        let mut sources = ViewSources {
            inputs: Vec::new(),
            providers: summary.providers.clone(),
        };
        for &slot in &summary.inputs {
            sources.merge(self.actual(arguments, slot)?);
        }
        Ok(sources)
    }
}
