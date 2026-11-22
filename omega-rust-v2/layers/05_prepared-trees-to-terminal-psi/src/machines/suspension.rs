//! Determine which existing values a closed local call must hold until normal completion.
use super::calls::Callees;
use crate::{
    ActivationValue, BlockTerminator, BranchBlock, CallArgument, CallSite, CallTarget, CleanupAction, CompletionContinuation, LoweringError, LoweringTypes,
    MachineBody, OperationId, OperationKind, OperationResult, PlaceId, ReturnEdge, Site, SuccessorEdge, SuspensionSite, TypeShape, ValueId, ValueOperand,
};

/// Dense local keys keep scalar values distinct from structural roots.
struct Keys(usize);
impl Keys {
    fn place(
        &self,
        p: PlaceId,
    ) -> usize {
        self.0 + p.index()
    }
    fn result(
        &self,
        v: OperationResult,
    ) -> Option<usize> {
        match v {
            OperationResult::Scalar(v) => Some(v.index()),
            OperationResult::Structural(p) => Some(self.place(p)),
            OperationResult::Unit => None,
        }
    }
    fn projected(
        &self,
        place: &crate::ProjectedPlace,
        mut read: impl FnMut(usize),
    ) {
        read(self.place(place.root));
        for step in &place.path {
            if let crate::PlaceProjection::ViewElement { index, length } = step {
                read(index.selector.index());
                read(length.index());
            }
        }
    }
    fn operand(
        &self,
        operand: &ValueOperand,
        mut read: impl FnMut(usize),
    ) {
        match operand {
            ValueOperand::Scalar(x) => read(x.index()),
            ValueOperand::CopyWhole(p) | ValueOperand::MoveWhole(p) => read(self.place(*p)),
            ValueOperand::Borrow { place, .. } => self.projected(place, read),
        }
    }
    fn cleanup(
        &self,
        actions: &[CleanupAction],
        mut read: impl FnMut(usize),
    ) {
        for action in actions {
            match action {
                CleanupAction::DiscardRoot(p) | CleanupAction::RetireHome(p) => read(self.place(*p)),
                CleanupAction::DiscardResidual { place, .. } | CleanupAction::Invoke { place, .. } => self.projected(place, &mut read),
            }
        }
    }
    fn call(
        &self,
        target: CallTarget,
        args: &[CallArgument],
        reads: &mut Vec<usize>,
    ) {
        if let CallTarget::Runtime { descriptor, .. } = target {
            reads.push(descriptor.index());
        }
        for arg in args {
            self.operand(arg, |key| reads.push(key));
        }
    }
    fn operation_reads(
        &self,
        op: &OperationKind,
    ) -> Result<Vec<usize>, &'static str> {
        use OperationKind::*;
        let mut reads = Vec::new();
        match op {
            Constant(_) | ConstructCallableDescriptor { .. } | DescribeMachine(_) | EstablishZeroedStructural => {}
            // An erased citation names its subjects without demanding them at runtime.
            ApplyTheorem(_) => {}
            Boolean { operands, .. } | Integer { operands, .. } | Float { operands, .. } | EstablishScalarArray { elements: operands } => {
                reads.extend(operands.iter().map(|v| v.index()))
            }
            TrappingInteger { left, right, .. } => reads.extend([left.index(), right.index()]),
            ExactNumericCast { operand } | RefineCallableDescriptor { source: operand, .. } | EstablishPrimitiveLocal { value: operand, .. } => {
                reads.push(operand.index())
            }
            Call { callee, arguments, .. } => self.call(*callee, arguments, &mut reads),
            ByteSequenceLength { source } | ElementViewLength { source } | ScalarBitRecast { source, .. } => reads.push(self.place(*source)),
            MutableScalarBitRecast { lender, .. } => reads.push(self.place(*lender)),
            ByteSequenceRead { source, index, length, .. } | ElementViewRead { source, index, length, .. } => {
                reads.extend([self.place(*source), index.index(), length.index()])
            }
            ElementViewWrite {
                destination,
                index,
                value,
                length,
                ..
            } => reads.extend([self.place(*destination), index.index(), value.index(), length.index()]),
            BorrowPlace { source, .. }
            | BorrowedMove { source }
            | CopyStructural { source }
            | MoveStructural { source }
            | StructuralCaseMembership { source, .. }
            | DecomposeRecord { source, .. } => self.projected(source, |key| reads.push(key)),
            StructuralStore {
                destination,
                value,
                disposition,
            } => {
                self.projected(destination, |key| reads.push(key));
                reads.push(self.place(*value));
                if let crate::StructuralStoreDisposition::Dispose(actions) = disposition {
                    self.cleanup(actions, |key| reads.push(key));
                }
            }
            EstablishRecord { fields } | EstablishCase { fields, .. } => {
                for field in fields {
                    self.operand(&field.operand, |key| reads.push(key));
                }
            }
            StructuralScalarFieldStore { destination, value, .. } | ProjectedScalarStore { destination, value } => {
                self.projected(destination, |key| reads.push(key));
                reads.push(value.index());
            }
            ProjectedScalarRead { source } => self.projected(source, |key| reads.push(key)),
            StructuralLeafCopy { source, path } | PrimitiveScalarRead { source, path } => {
                reads.push(self.place(*source));
                reads.extend(path.iter().map(|p| p.selector.index()));
            }
            EstablishStructuralArray { elements } => reads.extend(elements.iter().map(|p| self.place(*p))),
            BeginAssembly(_) | AssemblyInstruction(_) => return Err("suspension needs the assembly register and control frontier"),
        }
        Ok(reads)
    }
}

mod field_reborrows;
mod returned_views;
mod windows;
use returned_views::{ReturnInputs, ViewSource, input_origins};
use std::cell::LazyCell;

struct Liveness<'a> {
    body: &'a MachineBody,
    data: &'a [crate::LoweredData],
    keys: Keys,
    reads: Vec<Vec<usize>>,
    definitions: Vec<Vec<usize>>,
    at_entry: Vec<Vec<bool>>,
    input_origins: std::cell::OnceCell<Vec<Option<Vec<ViewSource<usize>>>>>,
}
impl<'a> Liveness<'a> {
    fn new(
        body: &'a MachineBody,
        data: &'a [crate::LoweredData],
        site: &Site,
    ) -> Result<Self, LoweringError> {
        let keys = Keys(body.values.len());
        let mut reads = Vec::with_capacity(body.operations.len());
        let mut definitions = Vec::with_capacity(body.operations.len());
        for op in &body.operations {
            reads.push(
                keys.operation_reads(&op.kind)
                    .map_err(|e| LoweringError::new(site, e))?,
            );
            let mut outputs = Vec::new();
            outputs.extend(keys.result(op.result));
            if let OperationKind::DecomposeRecord { fields, .. } = &op.kind {
                outputs.extend(fields.iter().filter_map(|f| keys.result(f.result)));
            }
            definitions.push(outputs);
        }
        let mut graph = Self {
            keys,
            body,
            data,
            reads,
            definitions,
            input_origins: std::cell::OnceCell::new(),
            at_entry: vec![vec![false; body.values.len() + body.places.len()]; body.blocks.len()],
        };
        loop {
            let mut changed = false;
            for block in body.blocks.iter().rev() {
                let mut live = graph.before_terminator(block);
                for pos in block.operations.clone().rev() {
                    graph.kill(pos, &mut live);
                    graph.add_reads(pos, &mut live);
                }
                let incoming = &mut graph.at_entry[block.id.get() - 1];
                if live != *incoming {
                    *incoming = live;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Ok(graph)
    }
    fn kill(
        &self,
        pos: usize,
        live: &mut [bool],
    ) {
        for &key in &self.definitions[pos] {
            live[key] = false;
        }
    }
    fn add_reads(
        &self,
        pos: usize,
        live: &mut [bool],
    ) {
        for &key in &self.reads[pos] {
            live[key] = true;
        }
    }
    /// The successor's live parameters come from this edge's actual operands.
    fn edge(
        &self,
        edge: &SuccessorEdge,
        live: &mut [bool],
    ) {
        let target = &self.body.blocks[edge.target.get() - 1];
        let incoming = &self.at_entry[edge.target.get() - 1];
        let mut translated = incoming.clone();
        for &p in &target.scalar_parameters {
            translated[p.index()] = false;
        }
        for &p in &target.structural_parameters {
            translated[self.keys.place(p)] = false;
        }
        for (&p, &v) in target.scalar_parameters.iter().zip(&edge.arguments) {
            if incoming[p.index()] {
                translated[v.index()] = true;
            }
        }
        for (&p, v) in target
            .structural_parameters
            .iter()
            .zip(&edge.structural_arguments)
        {
            if incoming[self.keys.place(p)] || matches!(v, ValueOperand::MoveWhole(_) | ValueOperand::Borrow { .. }) {
                self.keys.operand(v, |key| translated[key] = true);
            }
        }
        for (out, reachable) in live.iter_mut().zip(translated) {
            *out |= reachable;
        }
        if let Some(selected) = &edge.selected_case {
            self.keys.projected(&selected.source, |key| live[key] = true);
        }
        self.keys.cleanup(&edge.cleanup, |key| live[key] = true);
    }
    fn before_terminator(
        &self,
        block: &BranchBlock,
    ) -> Vec<bool> {
        let mut live = vec![false; self.body.values.len() + self.body.places.len()];
        let mut reads = Vec::new();
        match &block.terminator {
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                live[condition.index()] = true;
                self.edge(when_true, &mut live);
                self.edge(when_false, &mut live);
            }
            BlockTerminator::Jump(edge) => self.edge(edge, &mut live),
            BlockTerminator::Return { result, cleanup, .. } => {
                match result {
                    ReturnEdge::Scalar(v) => live[v.index()] = true,
                    ReturnEdge::Structural { source, .. } => live[self.keys.place(*source)] = true,
                    ReturnEdge::Unit => {}
                }
                self.keys.cleanup(cleanup, |key| reads.push(key));
            }
            BlockTerminator::TerminalCall {
                callee, arguments, cleanup, ..
            } => {
                self.keys.call(*callee, arguments, &mut reads);
                self.keys.cleanup(cleanup, |key| reads.push(key));
            }
            BlockTerminator::ExternalExit(exit) => {
                for arg in &exit.arguments {
                    self.keys.operand(arg, |key| reads.push(key));
                }
            }
        }
        for k in reads {
            live[k] = true;
        }
        live
    }
    /// A primitive local's home, the activation storage that `EstablishPrimitiveLocal` reserved.
    fn is_home(
        &self,
        place: PlaceId,
    ) -> bool {
        matches!(self.body.places[place.index()].origin, crate::PlaceOrigin::Operation(id)
            if matches!(self.body.operations[id.index()].kind, OperationKind::EstablishPrimitiveLocal { .. }))
    }
    /// Copyable aggregate storage needs a home when a call or continuation
    /// uses it. Descriptors stored inside that home keep their own origins.
    fn copyable_home(
        &self,
        place: PlaceId,
        types: &LoweringTypes,
    ) -> bool {
        let mut ty = self.body.places[place.index()].ty;
        if !matches!(types.get(ty), Some(TypeShape::Nominal(_) | TypeShape::QualifiedNominal { .. } | TypeShape::FixedArray { .. })) {
            return false;
        }
        while let Some(TypeShape::FixedArray { element, .. }) = types.get(ty) {
            ty = *element;
        }
        match types.get(ty) {
            Some(TypeShape::Nominal(_) | TypeShape::QualifiedNominal { .. }) => !super::ownership::owned(ty, types, self.data),
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::QualifiedScalar { .. }
                | TypeShape::SharedByteView | TypeShape::SharedElementView { .. }
                | TypeShape::SharedQualifiedScalarReference { .. } | TypeShape::IndexedSharedReference(_)
                | TypeShape::ScalarReference { access: crate::ReferenceAccess::Shared, .. }
                | TypeShape::NominalReference { access: crate::ReferenceAccess::Shared, .. }) => true,
            _ => false,
        }
    }
    /// The activation home needed by a borrow. Below a stored reference,
    /// the home holds the parent descriptor and its traced origin supplies
    /// the referent's backing.
    fn backing_home(
        &self,
        place: &crate::ProjectedPlace,
        types: &LoweringTypes,
        site: &Site,
    ) -> Option<PlaceId> {
        use crate::PlaceProjection;

        let captured = super::loans::captured_place(self.body, place);
        if captured.path.is_empty() && self.is_home(captured.root) {
            return Some(captured.root);
        }
        if !self.copyable_home(captured.root, types) {
            return None;
        }
        if let Some(crossing) = captured
            .path
            .iter()
            .position(|step| matches!(step, PlaceProjection::Referent | PlaceProjection::ViewElement { .. }))
        {
            let (fields, rest) = captured.path.split_at(crossing);
            let [PlaceProjection::Referent, referent_path @ ..] = rest else {
                return None;
            };
            let mut parent = self.body.places[captured.root.index()].ty;
            let mut steps = fields.iter();
            while let Some(step) = steps.next() {
                let (TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) = types.get(parent)? else {
                    return None;
                };
                let schema = &self.data[id.get() - 1];
                let (fields, field) = match step {
                    PlaceProjection::Case(case) => {
                        let case = schema.cases.iter().find(|candidate| candidate.declaration == *case)?;
                        (&case.fields, steps.next()?)
                    }
                    _ => (&schema.fields, step),
                };
                let PlaceProjection::Field(field) = field else {
                    return None;
                };
                parent = fields.iter().find(|candidate| candidate.declaration == *field)?.carrier;
            }
            let TypeShape::NominalReference { referent, access: crate::ReferenceAccess::Shared } = types.get(parent)? else {
                return None;
            };
            field_reborrows::check_backing(*referent, referent_path, types, self.data, site).ok()?;
        }
        Some(captured.root)
    }
    /// A direct operand names its storage. A named reborrow reaches that same
    /// storage through its defining BorrowPlace. Loan permissions remain on
    /// the original operands; this lookup only identifies their live backing.
    fn borrow_backing<'members>(
        &self,
        place: &crate::ProjectedPlace,
        owners: &[crate::RetainedOwner],
        types: &LoweringTypes,
        returns: &LazyCell<ReturnInputs<'members>, impl FnOnce() -> ReturnInputs<'members>>,
        site: &Site,
    ) -> Result<bool, LoweringError> {
        let captured;
        let place = if place.path.first() == Some(&crate::PlaceProjection::Referent)
            && matches!(self.body.places[place.root.index()].origin, crate::PlaceOrigin::Operation(_))
        {
            captured = super::loans::captured_place(self.body, place);
            &captured
        } else {
            place
        };
        let root = &self.body.places[place.root.index()];
        // A join parameter is an incoming loan when every arriving edge
        // supplies one; the edge arguments identify which loan arrived.
        let incoming = match root.origin {
            crate::PlaceOrigin::Parameter => true,
            // Only formals arrive as incoming loans; a provider result is not one.
            crate::PlaceOrigin::BlockParameter(_) => self
                .input_origins
                .get_or_init(|| input_origins(self.body, returns))[place.root.index()]
            .as_ref()
            .is_some_and(|sources| sources.iter().all(|source| matches!(source, ViewSource::Input(_)))),
            _ => false,
        };
        if incoming
            && let Some(TypeShape::NominalReference { referent, .. }) = types.get(root.ty)
            && let [crate::PlaceProjection::Referent, fields @ ..] = place.path.as_slice()
            && matches!(fields.first(), Some(crate::PlaceProjection::Field(_)))
        {
            // The incoming loan is the provenance anchor. A later `Referent`
            // crosses a descriptor stored in that record: the stored loan,
            // not this incoming one, is the parent of what follows it.
            field_reborrows::check_backing(*referent, fields, types, self.data, site)?;
            return Ok(true);
        }
        // The activation retains a borrowed home; `activation` publishes its row.
        if self.backing_home(place, types, site).is_some() {
            return Ok(true);
        }
        if !matches!(types.get(root.ty), Some(TypeShape::Nominal(_) | TypeShape::QualifiedNominal { .. } | TypeShape::FixedArray { .. })) {
            return Ok(false);
        }
        // The owner row publishes who holds the root during the call and which
        // paths have already moved out. Owned storage transferred whole has
        // no row; its move operation names the destination. A loan that
        // overlaps a hole or a transfer is a conflict the checker judges, as
        // it does for the same call without a suspension. A projection through a
        // stored descriptor needs its own referent, outside that home.
        if owners
            .binary_search_by_key(&place.root.index(), |owner| owner.place.index())
            .is_ok()
            || super::ownership::owned(root.ty, types, self.data)
        {
            return Ok(true);
        }
        Err(LoweringError::new(site, "parked borrow has no live owned backing for its selected place"))
    }

    fn activation<'members>(
        &self,
        site: &Site,
        target: CallTarget,
        args: &[CallArgument],
        after: &[bool],
        owners: &[crate::RetainedOwner],
        types: &LoweringTypes,
        returns: &LazyCell<ReturnInputs<'members>, impl FnOnce() -> ReturnInputs<'members>>,
    ) -> Result<(Vec<ActivationValue>, Vec<crate::RetainedCallable>, Vec<crate::RetainedLoan>, Vec<crate::RetainedHome>), LoweringError> {
        let mut held = vec![false; after.len()];
        if let CallTarget::Runtime { descriptor, .. } = target { held[descriptor.index()] = true; }
        let mut borrowed_root = vec![false; self.body.places.len()];
        for arg in args {
            if let CallArgument::Borrow { place, .. } = arg {
                self.borrow_backing(place, owners, types, returns, site)?;
                borrowed_root[place.root.index()] = true;
            }
            self.keys.operand(arg, |key| held[key] = true);
        }
        let mut retained_loans = Vec::new();
        // Whether the callee and the continuation need each home's storage.
        let mut home_needs = vec![(false, false); self.body.places.len()];
        for (index, place) in self.body.places.iter().enumerate() {
            let key = self.keys.0 + index;
            if !held[key] && !after[key] {
                continue;
            }
            let id = PlaceId(super::body::next_id(index));
            if self.is_home(id) || self.copyable_home(id, types) {
                let need = &mut home_needs[index];
                *need = (need.0 | held[key], need.1 | after[key]);
                continue;
            }
            if owners
                .binary_search_by_key(&index, |owner| owner.place.index())
                .is_ok()
            {
                continue;
            }
            // The call borrows from owned storage that moved out whole before
            // it. That storage contributes no claim; the borrow operand and
            // the move stay in Psi for the checker.
            if borrowed_root[index] && !after[key] && super::ownership::owned(place.ty, types, self.data) {
                continue;
            }
            // At a join, retain the selected descriptor. The arriving edge
            // supplies its backing. Other arms need not remain live after it.
            // A boundary result remains
            // anchored at its producing call, with an input lifetime dependency
            // or, for a caller-chosen lifetime, with only the provider's backing.
            let traceable = match place.origin {
                crate::PlaceOrigin::Parameter => true,
                crate::PlaceOrigin::Operation(id) if let OperationKind::BorrowPlace { source, .. } = &self.body.operations[id.index()].kind => {
                    self.borrow_backing(source, owners, types, returns, site)?
                }
                crate::PlaceOrigin::BlockParameter(_) | crate::PlaceOrigin::Operation(_) => self
                    .input_origins
                    .get_or_init(|| input_origins(self.body, returns))[index]
                    .is_some(),
                _ => false,
            };
            let access = match types.get(place.ty) {
                Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. }) if traceable => crate::ReferenceAccess::Shared,
                Some(TypeShape::NominalReference { access, .. } | TypeShape::ScalarReference { access, .. }) if traceable => *access,
                _ => {
                    return Err(LoweringError::new(
                        site,
                        "suspension needs the exact backing of the live structural place",
                    ));
                }
            };
            // A named loan of a home keeps that home in the activation.
            if let crate::PlaceOrigin::Operation(id) = place.origin
                && let OperationKind::BorrowPlace { source, .. } = &self.body.operations[id.index()].kind
                && let Some(home) = self.backing_home(source, types, site)
            {
                let need = &mut home_needs[home.index()];
                *need = (need.0 | held[key], need.1 | after[key]);
            }
            retained_loans.push(crate::RetainedLoan {
                descriptor: PlaceId(super::body::next_id(index)),
                ty: place.ty,
                access,
                needed_by_callee: held[key],
                needed_after_completion: after[key],
            });
        }
        let mut activation = Vec::new();
        let mut retained_callables = Vec::new();
        for (index, (&callee, &continuation)) in held[..self.keys.0].iter().zip(after).enumerate() {
            if !callee && !continuation {
                continue;
            }
            let ty = self.body.values[index].ty;
            if let Some(TypeShape::Callable(contract)) = types.get(ty) {
                retained_callables.push(crate::RetainedCallable { descriptor: ValueId(super::body::next_id(index)), contract: *contract,
                    needed_by_callee: callee, needed_after_completion: continuation });
                continue;
            }
            if !matches!(types.get(ty), Some(TypeShape::PrimitiveScalar(_) | TypeShape::QualifiedScalar { .. })) {
                return Err(LoweringError::new(
                    site,
                    "live scalar operand has a non-scalar type",
                ));
            }
            activation.push(ActivationValue {
                value: ValueId(super::body::next_id(index)),
                ty,
                needed_by_callee: callee,
                needed_after_completion: continuation,
            });
        }
        let retained_homes = home_needs
            .iter()
            .enumerate()
            .filter(|(_, need)| need.0 || need.1)
            .map(|(index, &(needed_by_callee, needed_after_completion))| {
                let place = PlaceId(super::body::next_id(index));
                crate::RetainedHome {
                    place,
                    ty: self.body.places[index].ty,
                    embedded_loans: Vec::new(),
                    needed_by_callee,
                    needed_after_completion,
                }
            })
            .collect();
        Ok((activation, retained_callables, retained_loans, retained_homes))
    }
}

/// Derive the crossing roster from the selected machine envelopes independently of
/// the source acknowledgements. A parked call still holds its evaluated arguments.
pub(crate) fn plan_package(
    machines: &mut [crate::LoweredMachine],
    members: &[crate::MemberCallSelection],
    data: &[crate::LoweredData],
    types: &LoweringTypes,
    callees: &Callees<'_>,
    crossing_owners: &mut [super::ownership::OwnedCrossings],
) -> Result<(), LoweringError> {
    if !machines.iter().any(|m| m.contract.realized_pause.suspend) {
        return Ok(());
    }
    // Every local callee's return edges now exist. Scalar-only crossings do
    // not need to compute which inputs can back a returned view.
    let returns = LazyCell::new(|| ReturnInputs::from_machines(machines, types, members));
    let sites = machines
        .iter()
        .zip(crossing_owners)
        .map(|(machine, owners)| {
            // The same authored selector premises that chose each store's disposition.
            let premises = super::view_loans::selector_premises(&machine.contract.requires, &machine.body.parameters);
            plan(&machine.body, &premises, data, types, callees, &returns, owners)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (machine, crossings) in machines.iter_mut().zip(sites) {
        machine.body.suspension_sites = crossings;
    }
    Ok(())
}

fn plan<'members>(
    body: &MachineBody,
    premises: &[super::view_loans::SelectorPremise],
    data: &[crate::LoweredData],
    types: &LoweringTypes,
    callees: &Callees<'_>,
    returns: &LazyCell<ReturnInputs<'members>, impl FnOnce() -> ReturnInputs<'members>>,
    owners: &mut super::ownership::OwnedCrossings,
) -> Result<Vec<SuspensionSite>, LoweringError> {
    let first = body
        .operations
        .iter()
        .find_map(|op| match op.kind {
            OperationKind::Call { callee, .. } if callees.may_park(callee) => Some(&body.diagnostics.operations[op.id.index()]),
            _ => None,
        })
        .or_else(|| {
            body.blocks.iter().find_map(|b| match b.terminator {
                BlockTerminator::TerminalCall { id, callee, .. } if callees.may_park(callee) => Some(&body.diagnostics.edges[id.get() - 1]),
                _ => None,
            })
        });
    let Some(first) = first else { return Ok(Vec::new()) };
    let graph = Liveness::new(body, data, first)?;
    let windows = windows::Windows::new(body, premises);
    let open_windows = |block, end, site: &Site| {
        windows
            .open_before(block, end)
            .ok_or_else(|| LoweringError::new(site, "suspension needs the exact open loan restoration windows"))
    };
    let mut crossings = Vec::new();
    for block in &body.blocks {
        let start = crossings.len();
        if let BlockTerminator::TerminalCall {
            id,
            callee,
            arguments,
            cleanup,
            ..
        } = &block.terminator
            && callees.may_park(*callee)
        {
            let mut after = vec![false; body.values.len() + body.places.len()];
            graph.keys.cleanup(cleanup, |key| after[key] = true);
            let site = &body.diagnostics.edges[id.get() - 1];
            let retained_owners = owners.take(CallSite::Edge(*id));
            let (activation, retained_callables, retained_loans, retained_homes) = graph.activation(site, *callee, arguments, &after, &retained_owners, types, returns)?;
            crossings.push(SuspensionSite {
                call: CallSite::Edge(*id),
                callee: *callee,
                continuation: CompletionContinuation::Return,
                activation,
                retained_callables,
                retained_loans,
                retained_owners,
                retained_homes,
                open_windows: open_windows(block, block.operations.end, site)?,
            });
        }
        let mut live = graph.before_terminator(block);
        for pos in block.operations.clone().rev() {
            let op = &body.operations[pos];
            graph.kill(pos, &mut live);
            if let OperationKind::Call { callee, arguments, .. } = &op.kind
                && callees.may_park(*callee)
            {
                let site = &body.diagnostics.operations[pos];
                if matches!(op.result, OperationResult::Structural(_)) {
                    return Err(LoweringError::new(site, "suspending structural results need their completion claim binding"));
                }
                let continuation = if pos + 1 < block.operations.end {
                    CompletionContinuation::Operation(OperationId(super::body::next_id(pos + 1)))
                } else {
                    CompletionContinuation::Terminator(block.id)
                };
                let retained_owners = owners.take(CallSite::Operation(op.id));
                let (activation, retained_callables, retained_loans, retained_homes) = graph.activation(site, *callee, arguments, &live, &retained_owners, types, returns)?;
                crossings.push(SuspensionSite {
                    call: CallSite::Operation(op.id),
                    callee: *callee,
                    continuation,
                    activation,
                    retained_callables,
                    retained_loans,
                    retained_owners,
                    retained_homes,
                    open_windows: open_windows(block, pos, site)?,
                });
            }
            graph.add_reads(pos, &mut live);
        }
        crossings[start..].reverse();
    }
    owners.trace_home_loans(body, &mut crossings, types, data)?;
    Ok(crossings)
}
