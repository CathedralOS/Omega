//! Prove that switching a parked entry to another admitted program preserves the
//! instructions and storage it has already established. The selected call belongs
//! to each program independently. The suffix and additional machines may differ.

use semantic_vocabulary::{BlockId, MachineId, OperationId, PlaceId};
use terminal_psi::{
    Block, Operation, OperationKind, TargetMachineCallable, TerminalMachine, TerminalModule,
    Terminator,
};

mod coordinates;
mod declarations;
mod frontier;
mod predicates;
mod returned_views;
mod returns;

/// The operation is complete at the cut. Execution continues at its next instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefixCall {
    pub machine: MachineId,
    pub block: BlockId,
    pub operation: OperationId,
    pub index: usize,
}

pub type BoundaryPair = ((MachineId, OperationId), (MachineId, OperationId));

/// Only the exact two retained modules can use this comparison.
pub struct CheckedPrefix<'a> {
    old: &'a TerminalModule,
    new: &'a TerminalModule,
    stack: Vec<(PrefixCall, PrefixCall)>,
    machines: Vec<(MachineId, MachineId)>,
    boundaries: Vec<BoundaryPair>,
    types: declarations::ReachedTypes,
}

impl<'a> CheckedPrefix<'a> {
    pub fn check(
        old: &'a TerminalModule,
        old_call: (MachineId, OperationId),
        new: &'a TerminalModule,
        new_call: (MachineId, OperationId),
    ) -> Result<Self, &'static str> {
        let (_, previous) = cut(old, old_call)?;
        Self::check_stack(old, &[previous], new, new_call)
    }

    /// Match the actual caller return sites in order. The old interpreter can
    /// be parked below the entry; each paired call selects the next machine.
    /// The final call is also bound to the successor's authored preparation.
    pub fn check_stack(
        old: &'a TerminalModule,
        stack: &[PrefixCall],
        new: &'a TerminalModule,
        new_call: (MachineId, OperationId),
    ) -> Result<Self, &'static str> {
        let entry = |m: &TerminalModule| match m.entry {
            Some(TargetMachineCallable::Machine(id)) => Ok(id),
            _ => Err("a continuation requires a checked machine entry"),
        };
        let mut pair = (entry(old)?, entry(new)?);
        let (successor_machine, successor) = cut(new, new_call)?;
        if successor_machine.block(&successor.block).is_some_and(|block| {
            matches!(block.terminator, Terminator::PendingNormalCompletion { call, .. } if call == successor.operation)
        }) {
            return Err("the checked successor still owes the selected completion");
        }
        let mut proof = Self {
            old,
            new,
            stack: Vec::with_capacity(stack.len()),
            machines: Vec::new(),
            boundaries: Vec::new(),
            types: Default::default(),
        };
        let mut invocations = returned_views::ReachedInvocations::default();
        let mut whole = Vec::new();
        for (depth, &site) in stack.iter().enumerate() {
            proof.remember_machine(pair)?;
            let (left, actual) = cut(old, (site.machine, site.operation))?;
            if site.machine != pair.0 || site != actual {
                return Err("a saved caller does not name its original checked frame");
            }
            let right = new
                .machine(pair.1)
                .ok_or("missing successor stack machine")?;
            let entered = depth + 1 < stack.len();
            let mapped = proof
                .machine(
                    left,
                    right,
                    Some((site, entered, depth == 0)),
                    &mut whole,
                    &mut invocations,
                )?
                .ok_or("the saved caller is unreachable from its machine entry")?;
            proof.stack.push((site, mapped));
            if entered {
                let direct = |m: &TerminalMachine, op| {
                    m.operation(op)
                        .and_then(|op| op.kind.direct_callee())
                        .ok_or("a suspended caller did not invoke the next checked machine")
                };
                pair = (
                    direct(left, site.operation)?,
                    direct(right, mapped.operation)?,
                );
            }
        }
        if proof.stack.last().map(|&(_, q)| q) != Some(successor) {
            return Err("the stack does not reach the selected successor preparation");
        }
        // A machine called earlier may also be parked now. Its completed calls
        // require a complete comparison independent of its saved return site.
        let mut completed = Vec::new();
        while let Some(pair) = whole.pop() {
            if completed.contains(&pair) {
                continue;
            }
            proof.remember_machine(pair)?;
            completed.push(pair);
            proof.machine(
                old.machine(pair.0).ok_or("missing previous callee")?,
                new.machine(pair.1).ok_or("missing successor callee")?,
                None,
                &mut whole,
                &mut invocations,
            )?;
        }
        proof.boundaries = invocations.finish(&proof)?;
        Ok(proof)
    }

    fn remember_machine(&mut self, pair: (MachineId, MachineId)) -> Result<(), &'static str> {
        if !self.machines.contains(&pair) {
            if self
                .machines
                .iter()
                .any(|&(a, b)| a == pair.0 || b == pair.1)
            {
                return Err("reached machine declarations have no unique successor");
            }
            self.machines.push(pair);
        }
        Ok(())
    }

    /// Outer caller through innermost preparation, with each frame's actual return instruction.
    pub fn stack(&self) -> &[(PrefixCall, PrefixCall)] {
        &self.stack
    }

    pub fn boundary_pairs(&self) -> &[BoundaryPair] {
        &self.boundaries
    }
    pub fn machine_pairs(&self) -> &[(MachineId, MachineId)] {
        &self.machines
    }

    pub fn accepts(&self, old: &TerminalModule, new: &TerminalModule) -> bool {
        std::ptr::eq(old, self.old) && std::ptr::eq(new, self.new)
    }
    pub fn old_call(&self) -> PrefixCall {
        self.stack.last().expect("checked a nonempty stack").0
    }
    pub fn new_call(&self) -> PrefixCall {
        self.stack.last().expect("checked a nonempty stack").1
    }

    fn machine(
        &self,
        left: &TerminalMachine,
        right: &TerminalMachine,
        cut: Option<(PrefixCall, bool, bool)>,
        calls: &mut Vec<(MachineId, MachineId)>,
        invocations: &mut returned_views::ReachedInvocations,
    ) -> Result<Option<PrefixCall>, &'static str> {
        self.signature(left, right, cut.is_some_and(|(_, _, root)| root))?;
        let mut pending = vec![(left.entry, right.entry)];
        // A parked frame proves that its old control flow reached this block.
        // Paths from which the call could never be reached have contributed no
        // state. Previously completed invocations still compare every path.
        let before = cut
            .map(|(site, _, _)| blocks_reaching(left, site.block))
            .transpose()?;
        let mut visited = Vec::new();
        let mut found = None;
        while let Some((a, b)) = pending.pop() {
            if before
                .as_ref()
                .is_some_and(|blocks| blocks.binary_search(&a).is_err())
            {
                continue;
            }
            if visited.contains(&(a, b)) {
                continue;
            }
            if visited.iter().any(|&(x, y)| x == a || y == b) {
                return Err("successor changed the control flow of the checked prefix");
            }
            visited.push((a, b));
            let l = left.block(&a).ok_or("missing previous block")?;
            let r = right.block(&b).ok_or("missing successor block")?;
            if !self.scalars(&l.parameters, &r.parameters)
                || l.erased_scalar_formals != r.erased_scalar_formals
                || l.erased_proof_formals != r.erased_proof_formals
                || !self.parameters(&l.structural_parameters, &r.structural_parameters)
            {
                return Err("successor changed the parameters of a reached block");
            }
            for parameter in &l.structural_parameters {
                let previous = left
                    .block_entry_claims
                    .iter()
                    .filter(|c| c.input == parameter.place);
                let successor = right
                    .block_entry_claims
                    .iter()
                    .filter(|c| c.input == parameter.place);
                if !same_rows(previous, successor, |a, b| self.entry_claim(a, b)) {
                    return Err("successor changed the claim on a reached block parameter");
                }
            }
            let selected = cut.filter(|(p, _, _)| p.block == a);
            let operations = if let Some((p, _, _)) = selected {
                let op = r
                    .operations
                    .get(p.index)
                    .ok_or("the successor lost a saved return instruction")?;
                // If an outgoing edge leads back here, the caller could have
                // executed its suffix on an earlier visit to the parked block.
                if before.as_ref().is_some_and(|blocks| {
                    l.terminator
                        .successor_targets()
                        .iter()
                        .any(|id| blocks.binary_search(id).is_ok())
                }) {
                    calls.push((left.id, right.id));
                }
                found = Some(PrefixCall {
                    machine: right.id,
                    block: b,
                    operation: op.id,
                    index: p.index,
                });
                Some(p.index + 1)
            } else {
                None
            };
            let lhs = operations.map_or(&l.operations[..], |len| &l.operations[..len]);
            let rhs = operations.map_or(&r.operations[..], |len| &r.operations[..len]);
            if lhs.len() != rhs.len() {
                return Err("successor changed the operations before the preparation");
            }
            for (index, (p, q)) in lhs.iter().zip(rhs).enumerate() {
                // This call's body has not returned. Its next stack frame proves
                // the portion that actually ran and owns its saved return site.
                let is_parent = selected.is_some_and(|(s, entered, _)| entered && s.index == index);
                self.operation(
                    left,
                    right,
                    p,
                    q,
                    (!is_parent).then_some(&mut *calls),
                    invocations,
                )?;
            }
            if operations.is_none() {
                for projection in l.terminator.storage_uses() {
                    self.same_place(left, right, projection.root)?;
                }
                self.edges(left, l, r, &mut pending)?;
            }
        }
        Ok(found)
    }

    fn edges(
        &self,
        machine: &TerminalMachine,
        left: &Block,
        right: &Block,
        pending: &mut Vec<(BlockId, BlockId)>,
    ) -> Result<(), &'static str> {
        let cleanup = |old: &[terminal_psi::TerminalAffineCleanupAction],
                       new: &mut [terminal_psi::TerminalAffineCleanupAction]|
         -> Result<(), &'static str> {
            use terminal_psi::TerminalAffineCleanupAction::*;
            if old.len() != new.len() {
                return Err("successor changed the number of cleanup actions");
            }
            for (a, b) in old.iter().zip(new) {
                match (a, b) {
                    (RetireHome(x), RetireHome(y)) | (DiscardRoot(x), DiscardRoot(y)) if x == y => {
                    }
                    (DiscardResidual(x), DiscardResidual(y))
                        if x.place == y.place
                            && self.structural_type(x.structural_type)
                                == Some(y.structural_type)
                            && self.paths(&x.path, &y.path) =>
                    {
                        y.path.clone_from_slice(&x.path);
                        y.structural_type = x.structural_type;
                    }
                    _ => return Err("successor changed or cannot verify a reached cleanup action"),
                }
            }
            Ok(())
        };
        // These arguments name the original homes, but their field selectors
        // belong to the receiving program's identity table.
        let arguments = |old: &[terminal_psi::StructuralArgument],
                         new: &mut [terminal_psi::StructuralArgument]| {
            if old.len() != new.len() {
                return Err("successor changed the number of structural edge arguments");
            }
            for (a, b) in old.iter().zip(new) {
                if !self.same_argument(machine, a, b) {
                    return Err("successor changed the storage of a structural edge argument");
                }
                b.path.clone_from_slice(&a.path);
            }
            Ok(())
        };
        let mut comparable = right.terminator.clone();
        match (&left.terminator, &mut comparable) {
            (
                Terminator::Jump {
                    target: a,
                    structural_arguments: ax,
                    cleanup_actions: ca,
                    ..
                },
                Terminator::Jump {
                    target: b,
                    structural_arguments: bx,
                    cleanup_actions: cb,
                    ..
                },
            ) => {
                arguments(ax, bx)?;
                cleanup(ca, cb)?;
                pending.push((*a, *b));
                *b = *a;
            }
            (
                Terminator::Conditional {
                    when_true: a,
                    when_false: b,
                    ..
                },
                Terminator::Conditional {
                    when_true: c,
                    when_false: d,
                    ..
                },
            ) => {
                arguments(&a.structural_arguments, &mut c.structural_arguments)?;
                arguments(&b.structural_arguments, &mut d.structural_arguments)?;
                cleanup(&a.cleanup_actions, &mut c.cleanup_actions)?;
                cleanup(&b.cleanup_actions, &mut d.cleanup_actions)?;
                pending.extend([(a.target, c.target), (b.target, d.target)]);
                c.target = a.target;
                d.target = b.target;
            }
            (
                Terminator::Return {
                    cleanup_actions: a, ..
                },
                Terminator::Return {
                    cleanup_actions: b, ..
                },
            )
            | (
                Terminator::ReturnUnit {
                    cleanup_actions: a, ..
                },
                Terminator::ReturnUnit {
                    cleanup_actions: b, ..
                },
            ) => cleanup(a, b)?,
            (
                Terminator::ReturnStructural {
                    returned_claims: a,
                    cleanup_actions: old,
                    ..
                },
                Terminator::ReturnStructural {
                    returned_claims: b,
                    cleanup_actions: new,
                    ..
                },
            ) if self.result_claims(a, b) => {
                for (a, b) in a.iter().zip(b.iter_mut()) {
                    b.path.clone_from_slice(&a.path);
                }
                cleanup(old, new)?;
            }
            (
                Terminator::StructuralCase {
                    cases: a,
                    path: ap,
                    source,
                },
                Terminator::StructuralCase {
                    cases: b, path: bp, ..
                },
            ) if a.len() == b.len() && self.paths(ap, bp) => {
                let owner = self.projected_type(machine, *source, ap);
                bp.clone_from_slice(ap);
                for (x, y) in a.iter().zip(b) {
                    self.check_case(owner, x.case, &mut y.case)?;
                    self.payload_fields(owner, &x.payload_fields, &mut y.payload_fields)?;
                    arguments(&x.structural_arguments, &mut y.structural_arguments)?;
                    cleanup(&x.cleanup_actions, &mut y.cleanup_actions)?;
                    pending.push((x.target, y.target));
                    y.target = x.target;
                }
            }
            _ => return Err("no checked correspondence for the reached control transfer"),
        }
        if left.terminator != comparable {
            return Err("successor changed a reached control transfer");
        }
        Ok(())
    }

    fn operation(
        &self,
        left: &TerminalMachine,
        right: &TerminalMachine,
        p: &Operation,
        q: &Operation,
        calls: Option<&mut Vec<(MachineId, MachineId)>>,
        invocations: &mut returned_views::ReachedInvocations,
    ) -> Result<(), &'static str> {
        if p.static_reach_binding.is_some()
            || q.static_reach_binding.is_some()
            || p.required_contract.is_some()
            || q.required_contract.is_some()
        {
            return Err("the reached call needs a checked selection correspondence");
        }
        // Returning a borrowed argument can restore a different home or claim
        // even when the printed call instruction itself is identical.
        let lc = left
            .loan_completions
            .iter()
            .filter(|row| row.operation == p.id);
        let rc = right
            .loan_completions
            .iter()
            .filter(|row| row.operation == q.id);
        if !same_rows(lc, rc, |a, b| {
            a.place == b.place
                && a.multiplicity == b.multiplicity
                && self.paths(&a.path, &b.path)
                && self.result_claims(&a.claims, &b.claims)
        }) {
            return Err("successor changed an earlier loan completion");
        }
        if p.suspension_crossing.is_some() || q.suspension_crossing.is_some() {
            self.suspension(left, right, p, q, invocations)?;
        }
        let mut kind = q.kind.clone();
        use OperationKind::*;
        match (&p.kind, &mut kind) {
            (Call { callee: a, .. }, Call { callee: b, .. })
            | (CallUnit { callee: a, .. }, CallUnit { callee: b, .. })
            | (CallStructuralScalar { callee: a, .. }, CallStructuralScalar { callee: b, .. })
            | (CallStructural { callee: a, .. }, CallStructural { callee: b, .. })
            | (
                CallStructuralWithScalarArguments { callee: a, .. },
                CallStructuralWithScalarArguments { callee: b, .. },
            ) => {
                // The checker discharged the callee's ordered preconditions
                // separately in each program. These witness numbers have no
                // runtime value. Match the actual conditions and their caller
                // operands before normalizing the two local numberings.
                let first = self.old.machine(*a).ok_or("missing previous callee")?;
                let second = self.new.machine(*b).ok_or("missing successor callee")?;
                if !self.same_entry_requirements(first, second)
                    || !published_call_receipts(self.old, left.id, p, first)
                    || !published_call_receipts(self.new, right.id, q, second)
                {
                    return Err("successor changed which preconditions the call must establish");
                }
                if let Some(calls) = calls {
                    calls.push((*a, *b));
                }
                invocations.record(((left.id, p.id), (right.id, q.id)), false);
                *b = *a;
            }
            (BoundaryCall { boundary: a, .. }, BoundaryCall { boundary: b, .. }) => {
                self.boundary(*a, *b)?;
                invocations.record(((left.id, p.id), (right.id, q.id)), true);
                *b = *a;
            }
            (
                terminal_psi::integer_arithmetic_operations!()
                | IntegerExactCast { .. }
                | ExactIntegerShiftLeft { .. }
                | ExactIntegerShiftRight { .. }
                | EstablishElementView { .. }
                | EstablishMutableElementView { .. }
                | ByteSequenceLength { .. }
                | ByteSequenceRead { .. }
                | ElementViewLength { .. }
                | ElementViewRead { .. }
                | ElementViewSubslice { .. }
                | ElementViewWrite { .. }
                | PackedIntegerUnary { .. } | PackedIntegerBinary { .. }
                | EstablishScalarArray { .. }
                | EstablishStructuralArray { .. }
                | EstablishReference { .. }
                | ReleaseReference { .. }
                | EstablishRecord { .. }
                | EstablishStructuralCase { .. }
                | EstablishScalarCase { .. }
                | EstablishZeroedStructural {}
                | EstablishStructuralLocal {}
                | EstablishPrimitiveLocal { .. }
                | PrimitiveScalarRead { .. }
                | StoreStructuralField { .. }
                | MoveStructuralField { .. }
                | MoveStructuralValue { .. }
                | EstablishByteSequenceLiteral { .. }
                | StructuralByteViewFieldStore { .. }
                | StructuralScalarFieldStore { .. }
                | StructuralLeafCopy { .. }
                | ScalarStructuralField { .. }
                | BooleanStructuralField { .. }
                | WriteOnlyPrimitiveStore { .. },
                _,
            ) => {}
            (kind, _)
                if crate::operation_semantic_row(kind)
                    .is_ok_and(|row| row.goal_free_scalar_leaf().is_some()) => {}
            _ => return Err("no checked correspondence for an operation before the preparation"),
        }
        self.operation_coordinates(left, p, &mut kind)?;
        // A path's field selectors index the owning module's identity table.
        // Compare their declarations before comparing the instruction operands.
        macro_rules! normalize_path {
            ($($kind:ident),+) => {
                match (&p.kind, &mut kind) {
                    $(( $kind { path: a, .. }, $kind { path: b, .. } ))|+ => {
                        if !self.paths(a, b) {
                            return Err("successor changed a stored projection");
                        }
                        b.clone_from_slice(a);
                    }
                    (EstablishReference { source: a, .. }, EstablishReference { source: b, .. })
                    | (MoveStructuralValue { source: a }, MoveStructuralValue { source: b }) => {
                        if !self.same_argument(left, a, b) {
                            return Err("successor changed the storage of a reached reference or move");
                        }
                        b.path.clone_from_slice(&a.path);
                    }
                    _ => {}
                }
            }
        }
        normalize_path!(
            ElementViewRead,
            ElementViewWrite,
            ReleaseReference,
            PrimitiveScalarRead,
            WriteOnlyPrimitiveStore,
            StructuralScalarFieldStore,
            MoveStructuralField,
            StoreStructuralField,
            StructuralLeafCopy,
            StructuralByteViewFieldStore
        );
        match &mut kind {
            CallUnit {
                claim_transfers, ..
            }
            | CallStructural {
                claim_transfers, ..
            }
            | CallStructuralScalar {
                claim_transfers, ..
            }
            | CallStructuralWithScalarArguments {
                claim_transfers, ..
            } => {
                for (old, new) in p.kind.claim_transfers().iter().zip(claim_transfers) {
                    self.normalize_path(&old.path, &mut new.path)?;
                }
            }
            BoundaryCall {
                completion_receipts,
                ..
            } => {
                if let BoundaryCall {
                    completion_receipts: old,
                    ..
                } = &p.kind
                {
                    for (old, new) in old.iter().zip(completion_receipts) {
                        self.normalize_path(&old.path, &mut new.path)?;
                    }
                }
            }
            _ => {}
        }
        if let Some(arguments) = kind.structural_arguments_mut() {
            for (a, b) in p.kind.structural_arguments().iter().zip(arguments) {
                self.check_argument(left, a, b)?;
            }
        }
        if let Call {
            requirement_obligations,
            ..
        }
        | CallUnit {
            requirement_obligations,
            ..
        }
        | CallStructuralScalar {
            requirement_obligations,
            ..
        }
        | CallStructural {
            requirement_obligations,
            ..
        }
        | CallStructuralWithScalarArguments {
            requirement_obligations,
            ..
        } = &mut kind
        {
            let old = p
                .declared_invocation()
                .ok_or("missing old call")?
                .requirement_obligations;
            requirement_obligations.clear();
            requirement_obligations.extend_from_slice(old);
        }
        if let EstablishByteSequenceLiteral { qualifications, .. } = &p.kind
            && !qualifications
                .iter()
                .all(|id| self.storage_domain_unchanged(*id))
        {
            return Err("successor changed a byte literal membership");
        }
        if p.kind != kind
            || !self.result(&p.result, &q.result)
            || p.suspension_crossing.is_some() != q.suspension_crossing.is_some()
        {
            return Err("successor changed an operation or its result in the executed prefix");
        }
        for ((storage, _), (new_storage, _)) in
            p.kind.storage_uses().into_iter().zip(q.kind.storage_uses())
        {
            use terminal_psi::StructuralStorageUse::*;
            let root = match (storage, new_storage) {
                (Place(r), Place(_))
                | (Canonical { root: r, .. }, Canonical { .. })
                | (ObservedField { root: r, .. }, ObservedField { .. }) => r,
                (Projection(a), Projection(b))
                    if self.paths_from(Some((left, a.root)), a.path, b.path) =>
                {
                    a.root
                }
                (
                    Element { descriptor, path },
                    Element {
                        descriptor: other,
                        path: other_path,
                    },
                ) if self.paths_from(
                    Some((left, descriptor.root)),
                    descriptor.path,
                    other.path,
                ) && self.paths(path, other_path) =>
                {
                    descriptor.root
                }
                (Type(a), Type(b)) if self.structural_type(a) == Some(b) => continue,
                _ => return Err("successor changed a structural operand"),
            };
            self.same_place(left, right, root)?;
        }
        Ok(())
    }

    fn paths(
        &self,
        old: &[terminal_psi::StructuralPathSegment],
        new: &[terminal_psi::StructuralPathSegment],
    ) -> bool {
        self.paths_from(None, old, new)
    }

    fn paths_from(
        &self,
        root: Option<(&TerminalMachine, PlaceId)>,
        old: &[terminal_psi::StructuralPathSegment],
        new: &[terminal_psi::StructuralPathSegment],
    ) -> bool {
        use terminal_psi::StructuralPathSegment::*;
        old.len() == new.len()
            && old
                .iter()
                .zip(new)
                .enumerate()
                .all(|(index, (left, right))| match (left, right) {
                    (Field(a), Field(b)) => self.field_identity(*a) == Some(*b),
                    (
                        RepresentationView {
                            structural_type: a,
                            stored_scalar: x,
                        },
                        RepresentationView {
                            structural_type: b,
                            stored_scalar: y,
                        },
                    ) => {
                        x == y
                            && !matches!(x, Some(semantic_vocabulary::ScalarType::Callable(_)))
                            && self.structural_type(*a) == Some(*b)
                    }
                    (Case(a), Case(b)) => {
                        let owner = root.and_then(|(machine, place)| {
                            self.projected_type(machine, place, &old[..index])
                        });
                        let case = match owner {
                            Some(ty) => self.case_id(ty, *a),
                            None => self.unambiguous_case_id(*a),
                        };
                        case == Some(*b)
                    }
                    (AllElements | FixedIndex(_) | FixedRange { .. } | Referent, _) => {
                        left == right
                    }
                    _ => false,
                })
    }

    fn normalize_path(
        &self,
        old: &[terminal_psi::StructuralPathSegment],
        new: &mut [terminal_psi::StructuralPathSegment],
    ) -> Result<(), &'static str> {
        if !self.paths(old, new) {
            return Err("successor changed a checked storage path");
        }
        new.clone_from_slice(old);
        Ok(())
    }

    fn entry_claim(&self, a: &terminal_psi::EntryClaim, b: &terminal_psi::EntryClaim) -> bool {
        a.claim == b.claim && a.input == b.input && self.paths(&a.path, &b.path)
    }

    fn result_claims(
        &self,
        a: &[terminal_psi::StructuralResultClaimBinding],
        b: &[terminal_psi::StructuralResultClaimBinding],
    ) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(a, b)| a.claim == b.claim && self.paths(&a.path, &b.path))
    }

    fn same_argument(
        &self,
        machine: &TerminalMachine,
        a: &terminal_psi::StructuralArgument,
        b: &terminal_psi::StructuralArgument,
    ) -> bool {
        a.place == b.place
            && a.access == b.access
            && self.paths_from(Some((machine, a.place)), &a.path, &b.path)
    }

    fn same_place(
        &self,
        a: &TerminalMachine,
        b: &TerminalMachine,
        root: semantic_vocabulary::PlaceId,
    ) -> Result<(), &'static str> {
        let ty = a
            .structural_place_type(root)
            .ok_or("the old storage has no declared type")?;
        if self
            .structural_type(ty)
            .is_none_or(|expected| Some(expected) != b.structural_place_type(root))
        {
            return Err("the successor changed an established storage type");
        }
        Ok(())
    }
}

/// Compare filtered rows in publication order, including the end of both rosters.
fn same_rows<T>(
    mut old: impl Iterator<Item = T>,
    mut new: impl Iterator<Item = T>,
    same: impl Fn(T, T) -> bool,
) -> bool {
    loop {
        match (old.next(), new.next()) {
            (Some(a), Some(b)) => {
                if !same(a, b) {
                    return false;
                }
            }
            (None, None) => return true,
            _ => return false,
        }
    }
}

/// Each numbered receipt is local to its own admitted invocation. An inferred
/// private premise can differ between programs only if this exact call proved it.
fn published_call_receipts(
    module: &TerminalModule,
    caller: MachineId,
    operation: &Operation,
    callee: &TerminalMachine,
) -> bool {
    let Some(call) = operation.declared_invocation() else {
        return false;
    };
    call.requirement_obligations.len() == callee.requires.len()
        && call
            .requirement_obligations
            .iter()
            .enumerate()
            .all(|(clause, &receipt)| {
                !module.identities.inferred_input_clause(callee.id, clause)
                    || (module.identities.matches_inferred_input_clause(
                        callee.id,
                        clause,
                        &callee.requires[clause],
                    ) && module.identities.proved_inferred_input(
                        caller,
                        semantic_vocabulary::InvocationSite::Operation(operation.id),
                        callee.id,
                        clause,
                        receipt,
                    ))
            })
}

fn blocks_reaching(machine: &TerminalMachine, cut: BlockId) -> Result<Vec<BlockId>, &'static str> {
    let mut ids = machine
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    let slot = |id| {
        ids.binary_search(&id)
            .map_err(|_| "a saved caller lost a control-flow target")
    };
    let mut incoming = vec![Vec::new(); ids.len()];
    for block in &machine.blocks {
        let from = slot(block.id)?;
        for target in block.terminator.successor_targets() {
            incoming[slot(target)?].push(from);
        }
    }
    let mut seen = vec![false; ids.len()];
    let mut work = vec![slot(cut)?];
    while let Some(position) = work.pop() {
        if !std::mem::replace(&mut seen[position], true) {
            work.extend_from_slice(&incoming[position]);
        }
    }
    Ok(ids
        .into_iter()
        .zip(seen)
        .filter_map(|(id, yes)| yes.then_some(id))
        .collect())
}

fn cut(
    module: &TerminalModule,
    (machine, operation): (MachineId, OperationId),
) -> Result<(&TerminalMachine, PrefixCall), &'static str> {
    let entry = module.machine(machine).ok_or("module lost its entry")?;
    for block in &entry.blocks {
        if let Some(index) = block.operations.iter().position(|op| op.id == operation) {
            let op = &block.operations[index];
            if op.declared_invocation().is_none() || op.suspension_crossing.is_none() {
                return Err("the checked cut is not a suspending call");
            }
            return Ok((
                entry,
                PrefixCall {
                    machine,
                    block: block.id,
                    operation,
                    index,
                },
            ));
        }
    }
    Err("the checked entry does not contain its preparation")
}
