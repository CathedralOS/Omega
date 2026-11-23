//! A returned shared descriptor can keep an anonymous caller allocation alive.
//! Resolve its backing in the selected callee, then place the owner's disposition
//! after the last consumer whose operands we can follow on this continuation.
use super::{OwnedCrossings, ReturnBackings};
use crate::machines::{
    body::{Completion, next_id},
    branches,
    calls::Callees,
    operand_reads::Keys,
    references::{LoanRoute, has_loan_route},
};
use crate::{
    BlockId, BlockTerminator, BranchBlock, CallTarget, CleanupAction, LoweredData, LoweredMachine, LoweringError, LoweringTypes, MachineBody,
    MemberCallSelection, MemberRealization, OperationId, OperationKind, OperationResult, PlaceId, PlaceProjection, ProjectedPlace, ReferenceAccess, ReturnEdge,
    TypeShape, ValueOperand,
};

pub(crate) struct ReturnedTemporary {
    pub operation: OperationId,
    pub roots: Vec<PlaceId>,
}

#[derive(Clone, Copy)]
struct Disposition {
    block: usize,
    after: usize,
    root: PlaceId,
}

const UNTRACED: &str = "a loan carried beyond this call needs its temporary backing on the later continuation";

pub(crate) fn finish_returned_loans(
    machines: &mut [LoweredMachine],
    members: &[MemberCallSelection],
    types: &LoweringTypes,
    data: &[LoweredData],
    callees: &Callees<'_>,
    completion: Vec<Completion>,
) -> Result<Vec<OwnedCrossings>, LoweringError> {
    // Compute every last use against the original operations and blocks before
    // inserting any cleanup continuation. Machine declaration order is immaterial.
    let graph = Package {
        machines,
        members,
        types,
        data,
    };
    let mut backings = ReturnBackings::new(machines, members, types, data);
    let mut plans = Vec::with_capacity(completion.len());
    for (index, item) in completion.iter().enumerate() {
        let mut plan = Vec::new();
        if let Completion::ReturnedLoans { borrows, .. } = item {
            let body = &machines[index].body;
            for borrow in borrows {
                let op = &body.operations[borrow.operation.index()];
                let refuse = || LoweringError::new(&body.diagnostics.operations[op.id.index()], UNTRACED);
                let OperationResult::Structural(result) = op.result else {
                    return Err(refuse());
                };
                let result_ty = body.places[result.index()].ty;
                if !matches!(
                    types.get(result_ty),
                    Some(
                        TypeShape::ScalarReference {
                            access: ReferenceAccess::Shared,
                            ..
                        } | TypeShape::NominalReference {
                            access: ReferenceAccess::Shared,
                            ..
                        }
                    )
                ) {
                    return Err(refuse());
                }
                let (local, incoming) = backings
                    .completed_descriptor(index, result)
                    .ok_or_else(refuse)?;
                let block = body
                    .blocks
                    .iter()
                    .position(|b| b.operations.contains(&op.id.index()))
                    .expect("call has a block");
                for &root in &borrow.roots {
                    let after = if local.contains(&root) {
                        if incoming || local.len() != 1 {
                            return Err(refuse());
                        }
                        graph
                            .last_use(body, block, borrow.operation, result)
                            .ok_or_else(refuse)?
                    } else {
                        op.id.index()
                    };
                    if super::owned(body.places[root.index()].ty, types, data) {
                        plan.push(Disposition { block, after, root });
                    }
                }
            }
        }
        plans.push(plan);
    }
    completion
        .into_iter()
        .zip(machines)
        .zip(plans)
        .map(|((item, machine), mut plan)| match item {
            Completion::Scheduled(crossings) => Ok(crossings),
            Completion::ReturnedLoans { result, .. } => {
                // Splitting from the end preserves earlier coordinates. At one split,
                // a newer owner is inserted ahead of the older owner's disposition.
                plan.sort_unstable_by_key(|d| (std::cmp::Reverse(d.after), d.root.index()));
                for d in plan {
                    dispose_after(&mut machine.body, d);
                }
                super::schedule(&mut machine.body, types, data, callees, result)
            }
        })
        .collect()
}

struct Package<'a> {
    machines: &'a [LoweredMachine],
    members: &'a [MemberCallSelection],
    types: &'a LoweringTypes,
    data: &'a [LoweredData],
}

/// Each path ends at a stored descriptor for the temporary under examination.
/// An empty path is the descriptor itself. A read of another field is independent.
type Paths = std::collections::BTreeMap<usize, Vec<Vec<PlaceProjection>>>;

fn operand_root(value: &ValueOperand) -> Option<PlaceId> {
    match value {
        ValueOperand::Scalar(_) => None,
        ValueOperand::CopyWhole(place) | ValueOperand::MoveWhole(place) => Some(*place),
        ValueOperand::Borrow { place, .. } => Some(place.root),
    }
}

impl Package<'_> {
    fn last_use(
        &self,
        body: &MachineBody,
        block: usize,
        producer: OperationId,
        descriptor: PlaceId,
    ) -> Option<usize> {
        let keys = Keys(body.values.len());
        let mut paths = Paths::from([(descriptor.index(), vec![Vec::new()])]);
        let mut last = producer.index();
        for op in body.operations.iter().skip(producer.index() + 1) {
            let reads = keys.operation_reads(&op.kind).ok()?;
            if !reads.iter().any(|key| {
                key.checked_sub(body.values.len())
                    .is_some_and(|index| paths.contains_key(&index))
            }) {
                continue;
            }
            if !body.blocks[block].operations.contains(&op.id.index()) {
                return None;
            }
            let mut carried = Vec::new();
            match &op.kind {
                OperationKind::ProjectedScalarRead { source } => {
                    if !overlaps_loan(source, &paths) {
                        continue;
                    }
                }
                OperationKind::Call { callee, arguments, .. } => {
                    if !arguments.iter().any(|arg| operand_uses_loan(arg, &paths)) {
                        continue;
                    }
                    if !self.consumes_loans(*callee) {
                        return None;
                    }
                }
                OperationKind::EstablishRecord { fields } | OperationKind::EstablishCase { fields, .. } => {
                    for field in fields {
                        let Some(parent) = operand_root(&field.operand).and_then(|place| paths.get(&place.index())) else {
                            continue;
                        };
                        if matches!(field.operand, ValueOperand::Borrow { .. }) {
                            return None;
                        }
                        for path in parent {
                            let mut field_path = Vec::new();
                            if let OperationKind::EstablishCase { case, .. } = &op.kind {
                                field_path.push(PlaceProjection::Case(case.clone()));
                            }
                            field_path.push(PlaceProjection::Field(field.field.clone()));
                            field_path.extend_from_slice(path);
                            carried.push(field_path);
                        }
                    }
                }
                OperationKind::CopyStructural { source } | OperationKind::MoveStructural { source } => {
                    for path in paths.get(&source.root.index())? {
                        if let Some(rest) = path.strip_prefix(source.path.as_slice()) {
                            carried.push(rest.to_vec());
                        } else if source.path.starts_with(path) {
                            return None;
                        }
                    }
                    if carried.is_empty() {
                        continue;
                    }
                }
                // A write to existing storage, reborrow, or forwarded child needs
                // its own traced lineage before this backing can be retired.
                _ => return None,
            }
            if !carried.is_empty() {
                let OperationResult::Structural(result) = op.result else { return None };
                // User cleanup can read a stored descriptor after its last explicit use.
                if self
                    .types
                    .cleanup_target(body.places[result.index()].ty, self.data)
                    .is_some()
                {
                    return None;
                }
                paths.insert(result.index(), carried);
            }
            last = op.id.index();
        }
        let uses = |arg: &ValueOperand| operand_uses_loan(arg, &paths);
        let edge_uses = |edge: &crate::SuccessorEdge| {
            edge.structural_arguments.iter().any(uses)
                || edge
                    .selected_case
                    .as_ref()
                    .is_some_and(|case| overlaps_loan(&case.source, &paths))
        };
        for block in &body.blocks {
            if match &block.terminator {
                BlockTerminator::Jump(edge) => edge_uses(edge),
                BlockTerminator::Conditional { when_true, when_false, .. } => edge_uses(when_true) || edge_uses(when_false),
                BlockTerminator::Return { result, .. } => matches!(result, ReturnEdge::Structural { source, .. } if paths.contains_key(&source.index())),
                BlockTerminator::TerminalCall { arguments, .. } => arguments.iter().any(uses),
                BlockTerminator::ExternalExit(exit) => exit.arguments.iter().any(uses),
            } {
                return None;
            }
        }
        Some(last)
    }

    fn consumes_loans(
        &self,
        callee: CallTarget,
    ) -> bool {
        let id = match callee {
            CallTarget::Machine(id) => id,
            CallTarget::Member(slot) => match self.members[slot].target {
                MemberRealization::Conformance { machine, .. } => machine,
                _ => return false,
            },
            _ => return false,
        };
        let callee = &self.machines[(id.get() - 1) as usize];
        !callee
            .parameter_types
            .iter()
            .any(|ty| has_loan_route(self.types, self.data, *ty, LoanRoute::WritableDestination))
            && !callee
                .result_type
                .is_some_and(|ty| has_loan_route(self.types, self.data, ty, LoanRoute::Descriptor))
    }
}

fn operand_uses_loan(
    arg: &ValueOperand,
    paths: &Paths,
) -> bool {
    match arg {
        ValueOperand::Borrow { place, .. } => overlaps_loan(place, paths),
        _ => operand_root(arg).is_some_and(|place| paths.contains_key(&place.index())),
    }
}

fn overlaps_loan(
    place: &ProjectedPlace,
    paths: &Paths,
) -> bool {
    paths.get(&place.root.index()).is_some_and(|loans| {
        loans
            .iter()
            .any(|loan| place.path.starts_with(loan) || loan.starts_with(&place.path))
    })
}

fn dispose_after(
    body: &mut MachineBody,
    d: Disposition,
) {
    let tail = BlockId(next_id(body.blocks.len()));
    let mut edge = branches::successor(body, tail, Vec::new(), body.diagnostics.operations[d.after].clone());
    edge.cleanup.push(CleanupAction::DiscardRoot(d.root));
    let current = &mut body.blocks[d.block];
    let terminator = std::mem::replace(&mut current.terminator, BlockTerminator::Jump(edge));
    let start = d.after + 1;
    let end = std::mem::replace(&mut current.operations.end, start);
    body.blocks.push(BranchBlock {
        id: tail,
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        scalar_domain_invariants: Vec::new(),
        operations: start..end,
        terminator,
    });
}
