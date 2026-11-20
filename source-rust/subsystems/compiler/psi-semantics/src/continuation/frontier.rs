//! Match the values needed by the successor to the storage the predecessor
//! actually parked. Loan links and live-value ordinals index different plans.

use super::CheckedPrefix;
use terminal_psi::{
    Operation, TargetMachineCallable, TerminalMachine, TerminalModule,
    TerminalSuspensionCallPlan as Plan, TerminalSuspensionCallTarget as Target,
    TerminalSuspensionLiveValue as Live, TerminalSuspensionLoan,
    TerminalSuspensionLoanBacking as Backing, TerminalSuspensionLoanCapture as Capture,
    TerminalSuspensionPlace as Place, TerminalSuspensionStorage as Storage,
    TerminalSuspensionValueLoans as Loans, TerminalSuspensionValueType as Type,
};

pub(super) fn plan<'a>(module: &'a TerminalModule, op: &Operation) -> Option<&'a Plan> {
    let crossing = op.suspension_crossing?;
    let target = match op.declared_invocation()?.callee {
        TargetMachineCallable::Machine(m) => Target::Machine(m),
        TargetMachineCallable::Boundary(b) => Target::Boundary(b),
    };
    let plans = &module.execution.suspension_call_plans;
    let mut found = plans.iter().filter(|p| p.crossing == crossing);
    let p = found.next()?;
    (found.next().is_none()
        && p.operation == op.id
        && p.target == target
        && p.live_values.len() == p.live_value_count as usize)
        .then_some(p)
}

fn image_literal<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    cut: &Operation,
    live: &Live,
) -> Option<&'a terminal_psi::OperationKind> {
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::{ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralTypeShape};

    let Place::Structural(argument) = &live.place else {
        return None;
    };
    let StructuralPlaceKind::ByteSequenceLiteral {
        structural_type, ..
    } = machine.structural_place(argument.place)?.kind
    else {
        return None;
    };
    if !argument.path.is_empty()
        || !matches!(
            argument.access,
            StructuralAccess::Owned | StructuralAccess::SharedBorrow
        )
        || live.value_type != Type::Structural(structural_type)
        || !matches!(
            module.structural_type(structural_type)?.shape,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            })
        )
    {
        return None;
    }
    let mut established = None;
    let mut used = None;
    for block in &machine.blocks {
        for (index, op) in block.operations.iter().enumerate() {
            if op.id == cut.id {
                used = Some((block.id, index));
            }
            if matches!(&op.kind, OperationKind::EstablishByteSequenceLiteral { destination, .. }
                if *destination == argument.place)
                && established.replace((block.id, index, &op.kind)).is_some()
            {
                return None;
            }
        }
    }
    let (block, index, instruction) = established?;
    let (cut_block, cut_index) = used?;
    let dominates = if block == cut_block {
        index < cut_index
    } else {
        crate::control_graph::dominators(machine).dominates(block, cut_block)
    };
    dominates.then_some(instruction)
}

impl CheckedPrefix<'_> {
    fn same_live_place(&self, left: &Place, right: &Place) -> bool {
        match (left, right) {
            (Place::Structural(a), Place::Structural(b)) => self.same_argument(a, b),
            (Place::Scalar(a), Place::Scalar(b)) => a == b,
            _ => false,
        }
    }

    pub(super) fn suspension(
        &self,
        left: &TerminalMachine,
        right: &TerminalMachine,
        old: &Operation,
        new: &Operation,
        invocations: &mut super::returned_views::ReachedInvocations,
    ) -> Result<(), &'static str> {
        let first = plan(self.old, old).ok_or("previous call lost its checked suspension plan")?;
        let second =
            plan(self.new, new).ok_or("successor call lost its checked suspension plan")?;
        if first.effective != second.effective {
            return Err("successor changed the carry policy of a reached call");
        }
        for target in &second.live_values {
            let same_parameter = match &target.place {
                Place::Structural(arg) if arg.path.is_empty() => left
                    .signature_structural_parameter(arg.place)
                    .zip(right.signature_structural_parameter(arg.place))
                    .is_some_and(|(a, b)| {
                        a.access == b.access
                            && self.structural_type(a.structural_type) == Some(b.structural_type)
                    }),
                _ => false,
            };
            let source = first
                .live_values
                .iter()
                .find(|v| {
                    self.same_live_place(&v.place, &target.place) && v.storage == target.storage
                })
                .or_else(|| {
                    // The full suffix can keep using a receiver after the call
                    // returns. The truncated entry already lends that exact
                    // incoming parameter to the same call, with the same access.
                    (same_parameter && target.storage == Storage::Parameter)
                        .then(|| {
                            first.live_values.iter().find(|v| {
                                self.same_live_place(&v.place, &target.place)
                                    && v.storage == Storage::CallArgument
                            })
                        })
                        .flatten()
                })
                .ok_or("successor needs a value the previous stage did not park")?;
            let ty = match (source.value_type, target.value_type) {
                (Type::Structural(a), Type::Structural(b)) => self.structural_type(a) == Some(b),
                (a, b) => a == b,
            };
            if !ty
                || source.claims != target.claims
                || source.claim_count != target.claim_count
                || source.effective != target.effective
            {
                return Err("successor changed the type, claims or carry policy of a live value");
            }
            if !same_parameter
                && matches!(
                    (source.loans, target.loans),
                    (Loans::Unresolved, Loans::Unresolved)
                )
                && let Some(original) = image_literal(self.old, left, old, source)
                && image_literal(self.new, right, new, target) == Some(original)
            {
                // The image owns these octets for the whole execution. The
                // matched establishment in the checked prefix binds its bytes,
                // qualifications and destination independently of the loan plan.
                continue;
            }
            if !same_parameter
                && matches!(
                    (source.loans, target.loans),
                    (Loans::Unresolved, Loans::Unresolved)
                )
                && invocations.require_returned_view(
                    self,
                    (left, old, source),
                    (right, new, target),
                )
            {
                continue;
            }
            self.value_loans(first, second, source, target, same_parameter)?;
        }
        Ok(())
    }

    fn value_loans(
        &self,
        old: &Plan,
        new: &Plan,
        a: &Live,
        b: &Live,
        parameter: bool,
    ) -> Result<(), &'static str> {
        match (a.loans, b.loans) {
            (Loans::ViewFree, Loans::ViewFree) => Ok(()),
            // Its external backing is the original, still bound input to this
            // machine. Unresolved local references have no such witness.
            (Loans::Unresolved, Loans::Unresolved) if parameter => Ok(()),
            (Loans::Retained { links: p }, Loans::Retained { links: q }) => {
                let original = old
                    .loans
                    .value_links
                    .span(p)
                    .ok_or("invalid old suspension loan links")?;
                let successor = new
                    .loans
                    .value_links
                    .span(q)
                    .ok_or("invalid successor suspension loan links")?;
                if original.len() != successor.len() || original.is_empty() {
                    return Err("the parked loan set changed");
                }
                for (&left, &right) in original.iter().zip(successor) {
                    let l = old
                        .loans
                        .occurrences
                        .try_get(left)
                        .ok_or("missing original loan occurrence")?;
                    let r = new
                        .loans
                        .occurrences
                        .try_get(right)
                        .ok_or("missing successor loan occurrence")?;
                    self.loan(old, new, l, r)?;
                }
                Ok(())
            }
            _ => Err("successor cannot recover a live value's checked loan lineage"),
        }
    }

    fn loan(
        &self,
        old: &Plan,
        new: &Plan,
        a: &TerminalSuspensionLoan,
        b: &TerminalSuspensionLoan,
    ) -> Result<(), &'static str> {
        let same_live = |x: u32, y: u32| {
            old.live_values
                .get(x as usize)
                .zip(new.live_values.get(y as usize))
                .is_some_and(|(left, right)| {
                    self.same_live_place(&left.place, &right.place) && left.storage == right.storage
                })
        };
        if a.access != b.access
            || a.active_at_call != b.active_at_call
            || a.resource != b.resource
            || [a, b]
                .iter()
                .any(|l| l.parent.is_valid() || !l.suppliers.is_empty() || !l.inputs.is_empty())
        {
            return Err("the suspension needs a different or unsupported loan parent");
        }
        let paths = |x, y| -> bool {
            old.loans
                .paths
                .span(x)
                .zip(new.loans.paths.span(y))
                .is_some_and(|(l, r)| self.paths(l, r))
        };
        let captured = match (&a.capture, &b.capture) {
            (
                Capture::Reference {
                    formation: x,
                    carrier: c,
                    source: s,
                    source_access: u,
                    source_path: p,
                },
                Capture::Reference {
                    formation: y,
                    carrier: d,
                    source: t,
                    source_access: v,
                    source_path: q,
                },
            ) => x == y && c == d && s == t && u == v && paths(*p, *q),
            (
                Capture::RetainedValue {
                    live_value: x,
                    path: p,
                },
                Capture::RetainedValue {
                    live_value: y,
                    path: q,
                },
            ) => same_live(*x, *y) && paths(*p, *q),
            _ => false,
        };
        let backing = match (&a.backing, &b.backing) {
            (
                Backing::RetainedOwner { live_value: x },
                Backing::RetainedOwner { live_value: y },
            ) => same_live(*x, *y),
            (
                Backing::IncomingReferent { argument: x },
                Backing::IncomingReferent { argument: y },
            ) => self.same_argument(x, y),
            _ => false,
        };
        if !captured || !backing {
            return Err("successor changed the captured reference or its backing storage");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuation::PrefixCall;
    use semantic_vocabulary::{
        BlockId, MachineId, OperationId, PlaceId, StructuralTypeId, SuspensionCrossingId,
    };
    use terminal_psi::{StructuralAccess, StructuralArgument, TerminalSuspensionLoanResource};

    #[test]
    fn a_retained_reference_cannot_move_to_another_backing_home() {
        let root = PlaceId::new(1).unwrap();
        let different = PlaceId::new(2).unwrap();
        let mut module = TerminalModule::declarations();
        let mut names = terminal_psi::TerminalIdentityTableBuilder::default();
        let amount = names.intern("amount");
        let neighbor = names.intern("neighbor");
        module.identities = names.build();
        let cut = PrefixCall {
            machine: MachineId::new(1).unwrap(),
            block: BlockId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
            index: 0,
        };
        let proof = CheckedPrefix {
            old: &module,
            new: &module,
            stack: vec![(cut, cut)],
            machines: Vec::new(),
            boundaries: Vec::new(),
            types: Default::default(),
        };
        let mut plan = Plan {
            operation: cut.operation,
            crossing: SuspensionCrossingId::new(1).unwrap(),
            target: Target::Machine(cut.machine),
            effective: language_core::CarryPolicy::PERMISSIVE,
            live_value_count: 2,
            live_values: Vec::new(),
            loans: Default::default(),
        };
        for place in [root, different] {
            plan.live_values.push(Live {
                place: Place::Structural(StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                }),
                value_type: Type::Structural(StructuralTypeId::new(1).unwrap()),
                storage: Storage::Local,
                claim_count: 0,
                claims: Vec::new(),
                effective: language_core::CarryPolicy::PERMISSIVE,
                loans: Loans::ViewFree,
            });
        }
        let selected = plan
            .loans
            .paths
            .insert_many([terminal_psi::StructuralPathSegment::Field(amount)]);
        let sibling = plan
            .loans
            .paths
            .insert_many([terminal_psi::StructuralPathSegment::Field(neighbor)]);
        let loan = TerminalSuspensionLoan {
            capture: Capture::Reference {
                formation: cut.operation,
                carrier: PlaceId::new(3).unwrap(),
                source: root,
                source_access: StructuralAccess::Owned,
                source_path: selected,
            },
            backing: Backing::RetainedOwner { live_value: 0 },
            resource: TerminalSuspensionLoanResource::DirectRoot,
            access: StructuralAccess::MutableBorrow,
            active_at_call: true,
            ..Default::default()
        };
        assert!(proof.loan(&plan, &plan, &loan, &loan).is_ok());
        let mut redirected = loan.clone();
        let Capture::Reference { source_path, .. } = &mut redirected.capture else {
            unreachable!()
        };
        *source_path = sibling;
        assert!(proof.loan(&plan, &plan, &loan, &redirected).is_err());
        let wrong = TerminalSuspensionLoan {
            backing: Backing::RetainedOwner { live_value: 1 },
            ..loan.clone()
        };
        assert!(proof.loan(&plan, &plan, &loan, &wrong).is_err());
    }
}
