//! Exact erased transport slots. Expected rows belong to receiving authority,
//! never to the decoded module whose evidence is being admitted.
use super::MathematicalProofActualError as Error;
use semantic_vocabulary::ProofTerm;
use std::collections::BTreeSet;
use terminal_psi::{
    Block, CheckedMachineDeclaration, ErasedProofFormal, OperationKind, TerminalMachine,
    TerminalModule, Terminator,
};

pub(super) struct Slot<'a> {
    pub key: (u64, u64, u8, u64, u64),
    pub target: (u64, u64),
    pub owner: &'a TerminalMachine,
    pub block: &'a Block,
    pub callee: Option<&'a CheckedMachineDeclaration>,
    pub formals: &'a [ErasedProofFormal],
    pub actuals: &'a [ProofTerm],
}

pub(super) fn contains_mathematical(
    module: &TerminalModule,
    term: &ProofTerm,
    caller: &[ErasedProofFormal],
    invocation: &[ErasedProofFormal],
) -> bool {
    let mut pending = vec![term];
    while let Some(term) = pending.pop() {
        match term {
            ProofTerm::Mathematical { .. } | ProofTerm::MathematicalApplication { .. } => {
                return true;
            }
            ProofTerm::Formal { position }
                if caller.get(*position as usize).is_some_and(|formal| {
                    has_mathematical(module, std::slice::from_ref(formal))
                }) =>
            {
                return true;
            }
            ProofTerm::InvocationFormal { position }
                if invocation.get(*position as usize).is_some_and(|formal| {
                    has_mathematical(module, std::slice::from_ref(formal))
                }) =>
            {
                return true;
            }
            ProofTerm::Construction { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.term))
            }
            ProofTerm::Array { elements, .. } => pending.extend(elements),
            _ => {}
        }
    }
    false
}
fn has_mathematical(module: &TerminalModule, formals: &[ErasedProofFormal]) -> bool {
    formals.iter().any(|formal| {
        module
            .mathematical_carriers
            .iter()
            .any(|carrier| carrier.type_identity == formal.type_identity)
    })
}
pub(super) fn same_formals(
    left: &TerminalModule,
    a: &[ErasedProofFormal],
    right: &TerminalModule,
    b: &[ErasedProofFormal],
) -> Result<(), Error> {
    if a.len() != b.len()
        || a.iter().zip(b).any(|(a, b)| {
            let left = left
                .identities
                .text(a.type_identity)
                .filter(|s| !s.is_empty());
            let right = right
                .identities
                .text(b.type_identity)
                .filter(|s| !s.is_empty());
            a.source_position != b.source_position
                || left.is_none()
                || right.is_none()
                || left != right
        })
    {
        return Err(Error::SourceCarrierMismatch);
    }
    Ok(())
}
fn same_declaration(
    left: &TerminalModule,
    a: Option<&CheckedMachineDeclaration>,
    right: &TerminalModule,
    b: Option<&CheckedMachineDeclaration>,
) -> Result<(), Error> {
    match (a, b) {
        (Some(a), Some(b)) if a.same_identity_as(&left.identities, b, &right.identities) => Ok(()),
        _ => Err(Error::SourceCarrierMismatch),
    }
}
pub(super) fn bind_slot(
    actual: &TerminalModule,
    a: &Slot<'_>,
    expected: &TerminalModule,
    b: &Slot<'_>,
) -> Result<(), Error> {
    if a.target != b.target {
        return Err(Error::SourceCarrierMismatch);
    }
    same_declaration(
        actual,
        a.owner.declaration.as_ref(),
        expected,
        b.owner.declaration.as_ref(),
    )?;
    if a.callee.is_some() || b.callee.is_some() {
        same_declaration(actual, a.callee, expected, b.callee)?;
    }
    same_formals(
        actual,
        &a.owner.contract.erased_proof_formals,
        expected,
        &b.owner.contract.erased_proof_formals,
    )?;
    same_formals(actual, a.formals, expected, b.formals)?;
    same_formals(
        actual,
        &a.block.erased_proof_formals,
        expected,
        &b.block.erased_proof_formals,
    )
}

pub(super) fn collect(module: &TerminalModule) -> Result<Vec<Slot<'_>>, Error> {
    let mut identities = BTreeSet::new();
    let mut texts = BTreeSet::new();
    for row in &module.mathematical_carriers {
        if !identities.insert(row.type_identity) {
            return Err(Error::KernelCarrierMismatch);
        }
        let text = module
            .identities
            .text(row.type_identity)
            .filter(|text| !text.is_empty())
            .ok_or(Error::SourceCarrierMismatch)?;
        if !texts.insert(text) {
            return Err(Error::SourceCarrierMismatch);
        }
    }
    let mut machine_ids = BTreeSet::new();
    let mut slots = Vec::new();
    for owner in &module.machines {
        if !machine_ids.insert(owner.id) {
            return Err(Error::SourceCarrierMismatch);
        }
        let mut block_ids = BTreeSet::new();
        for block in &owner.blocks {
            if !block_ids.insert(block.id) {
                return Err(Error::SourceCarrierMismatch);
            }
            for (ordinal, operation) in block.operations.iter().enumerate() {
                let direct = match &operation.kind {
                    OperationKind::Call {
                        callee,
                        erased_proof_arguments,
                        ..
                    } => Some((1, callee, erased_proof_arguments)),
                    OperationKind::CallUnit {
                        callee,
                        erased_proof_arguments,
                        ..
                    } => Some((2, callee, erased_proof_arguments)),
                    OperationKind::CallStructuralScalar {
                        callee,
                        erased_proof_arguments,
                        ..
                    } => Some((3, callee, erased_proof_arguments)),
                    OperationKind::CallStructuralWithScalarArguments {
                        callee,
                        erased_proof_arguments,
                        ..
                    } => Some((4, callee, erased_proof_arguments)),
                    OperationKind::CallDynamic {
                        erased_proof_arguments,
                        ..
                    } if erased_proof_arguments.iter().any(|term| {
                        contains_mathematical(
                            module,
                            term,
                            &block.erased_proof_formals,
                            &owner.contract.erased_proof_formals,
                        )
                    }) =>
                    {
                        return Err(Error::KernelCarrierMismatch);
                    }
                    _ => None,
                };
                if let Some((kind, callee, actuals)) = direct {
                    let mut candidates = module
                        .machines
                        .iter()
                        .filter(|machine| machine.id == *callee);
                    let target = candidates.next().ok_or(Error::SourceCarrierMismatch)?;
                    if candidates.next().is_some() {
                        return Err(Error::SourceCarrierMismatch);
                    }
                    let formals = &target.contract.erased_proof_formals;
                    if has_mathematical(module, formals)
                        || actuals.iter().any(|term| {
                            contains_mathematical(
                                module,
                                term,
                                &block.erased_proof_formals,
                                &owner.contract.erased_proof_formals,
                            )
                        })
                    {
                        slots.push(Slot {
                            key: (
                                owner.id.get(),
                                block.id.get(),
                                kind,
                                ordinal as u64,
                                operation.id.get(),
                            ),
                            target: (callee.get(), 0),
                            owner,
                            block,
                            callee: target.declaration.as_ref(),
                            formals,
                            actuals,
                        });
                    }
                }
            }
            if let Some(invocation) = block.terminator.module_invocation() {
                let mut candidates = module
                    .machines
                    .iter()
                    .filter(|machine| machine.id == invocation.callee);
                let target = candidates.next().ok_or(Error::SourceCarrierMismatch)?;
                if candidates.next().is_some() {
                    return Err(Error::SourceCarrierMismatch);
                }
                let formals = &target.contract.erased_proof_formals;
                let actuals = invocation.erased_proof_arguments;
                if has_mathematical(module, formals)
                    || actuals.iter().any(|term| {
                        contains_mathematical(
                            module,
                            term,
                            &block.erased_proof_formals,
                            &owner.contract.erased_proof_formals,
                        )
                    })
                {
                    // Tag 8 is an invocation edge, not an operation ordinal.
                    // Keep the source edge identity even when its numeric id
                    // equals an ordinary operation in the same block.
                    slots.push(Slot {
                        key: (
                            owner.id.get(),
                            block.id.get(),
                            8,
                            block.terminator.edge().get(),
                            0,
                        ),
                        target: (invocation.callee.get(), 0),
                        owner,
                        block,
                        callee: target.declaration.as_ref(),
                        formals,
                        actuals,
                    });
                }
            }
            let edges = match &block.terminator {
                Terminator::Jump {
                    edge,
                    target,
                    erased_proof_arguments,
                    ..
                } => vec![(5, *edge, *target, erased_proof_arguments)],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    (
                        6,
                        when_true.edge,
                        when_true.target,
                        &when_true.erased_proof_arguments,
                    ),
                    (
                        7,
                        when_false.edge,
                        when_false.target,
                        &when_false.erased_proof_arguments,
                    ),
                ],
                _ => Vec::new(),
            };
            for (kind, edge, target, actuals) in edges {
                let mut candidates = owner.blocks.iter().filter(|block| block.id == target);
                let destination = candidates.next().ok_or(Error::SourceCarrierMismatch)?;
                if candidates.next().is_some() {
                    return Err(Error::SourceCarrierMismatch);
                }
                let formals = &destination.erased_proof_formals;
                if has_mathematical(module, formals)
                    || actuals.iter().any(|term| {
                        contains_mathematical(
                            module,
                            term,
                            &block.erased_proof_formals,
                            &owner.contract.erased_proof_formals,
                        )
                    })
                {
                    slots.push(Slot {
                        key: (owner.id.get(), block.id.get(), kind, 0, edge.get()),
                        target: (0, target.get()),
                        owner,
                        block,
                        callee: None,
                        formals,
                        actuals,
                    });
                }
            }
        }
    }
    let mut keys = BTreeSet::new();
    if slots.iter().any(|slot| !keys.insert(slot.key)) {
        return Err(Error::SourceCarrierMismatch);
    }
    Ok(slots)
}

/// Retain source/question authority for every mathematical formal roster,
/// including an unused formal whose block currently has no proof actual.
pub(super) fn bind_rosters(
    actual: &TerminalModule,
    expected: &TerminalModule,
) -> Result<(), Error> {
    for (left, right) in [(actual, expected), (expected, actual)] {
        for owner in &left.machines {
            let math_contract = has_mathematical(left, &owner.contract.erased_proof_formals);
            let math_blocks = owner
                .blocks
                .iter()
                .filter(|block| has_mathematical(left, &block.erased_proof_formals))
                .collect::<Vec<_>>();
            if !math_contract && math_blocks.is_empty() {
                continue;
            }
            let mut candidates = right
                .machines
                .iter()
                .filter(|machine| machine.id == owner.id);
            let other = candidates.next().ok_or(Error::SourceCarrierMismatch)?;
            if candidates.next().is_some() {
                return Err(Error::SourceCarrierMismatch);
            }
            same_declaration(
                left,
                owner.declaration.as_ref(),
                right,
                other.declaration.as_ref(),
            )?;
            if math_contract {
                same_formals(
                    left,
                    &owner.contract.erased_proof_formals,
                    right,
                    &other.contract.erased_proof_formals,
                )?;
            }
            for block in math_blocks {
                let mut candidates = other
                    .blocks
                    .iter()
                    .filter(|candidate| candidate.id == block.id);
                let other = candidates.next().ok_or(Error::SourceCarrierMismatch)?;
                if candidates.next().is_some() {
                    return Err(Error::SourceCarrierMismatch);
                }
                same_formals(
                    left,
                    &block.erased_proof_formals,
                    right,
                    &other.erased_proof_formals,
                )?;
            }
        }
    }
    Ok(())
}
