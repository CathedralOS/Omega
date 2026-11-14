/*
Canonical encoding, decoding, and structural validation for `MathematicalProofActual` carrier keys,
applications, and slots, along with `IndexedTheoremRootTable` theorem-root lookup tables.
*/

pub(crate) mod applications {

    use proof_admission::{Budget, Context, Signature, Term, infer_sort, weak_head_normalize};
    use semantic_vocabulary::{MathematicalArgument, ScalarTerm};
    use terminal_psi::{ErasedProofFormal, TerminalModule};

    use super::{MathematicalProofActualError as Error, mathematical_carrier_key};

    pub(crate) fn prepare(
        module: &TerminalModule,
        caller: &[ErasedProofFormal],
        expected: &TerminalModule,
        trusted_caller: &[ErasedProofFormal],
        certificate: &[u8],
        arguments: &[MathematicalArgument],
        expected_result_key: &[u8],
        slot: &super::slots::Slot<'_>,
        trusted_slot: &super::slots::Slot<'_>,
        budget: &mut Budget,
    ) -> Result<crate::DecodedMathematicalCertificate, Error> {
        if arguments.is_empty() {
            return Err(Error::SourceCarrierMismatch);
        }
        let mut decoded =
            crate::decode_mathematical_certificate(certificate).map_err(Error::Wire)?;
        if decoded.certificate.level_arity != 0 || !decoded.certificate.context.is_empty() {
            return Err(Error::OpenContext);
        }
        let signature = Signature::from_declarations(decoded.certificate.signature.clone());
        let mut result = decoded.certificate.expected;
        for argument in arguments {
            let carrier = match argument {
                MathematicalArgument::Formal { position } => {
                    let source = caller
                        .get(*position as usize)
                        .ok_or(Error::SourceCarrierMismatch)?;
                    let trusted = trusted_caller
                        .get(*position as usize)
                        .ok_or(Error::SourceCarrierMismatch)?;
                    let identity = expected
                        .identities
                        .text(trusted.type_identity)
                        .filter(|identity| !identity.is_empty())
                        .ok_or(Error::SourceCarrierMismatch)?;
                    if module.identities.text(source.type_identity) != Some(identity) {
                        return Err(Error::SourceCarrierMismatch);
                    }
                    let carrier = expected
                        .mathematical_carriers
                        .iter()
                        .find(|row| row.type_identity == trusted.type_identity)
                        .ok_or(Error::KernelCarrierMismatch)?;
                    let actual_carrier = module
                        .mathematical_carriers
                        .iter()
                        .find(|row| row.type_identity == source.type_identity)
                        .ok_or(Error::KernelCarrierMismatch)?;
                    if actual_carrier.carrier_key != carrier.carrier_key {
                        return Err(Error::KernelCarrierMismatch);
                    }
                    carrier
                }
                MathematicalArgument::Scalar {
                    type_identity,
                    term,
                } => {
                    let ScalarTerm::Value { id, scalar_type } = term else {
                        return Err(Error::SourceCarrierMismatch);
                    };
                    for scope in [slot, trusted_slot] {
                        let mut found = false;
                        for value in scope.owner.parameters.iter().chain(&scope.block.parameters) {
                            if value.id == *id {
                                if value.scalar_type != *scalar_type {
                                    return Err(Error::SourceCarrierMismatch);
                                }
                                found = true;
                            }
                        }
                        if !found {
                            return Err(Error::SourceCarrierMismatch);
                        }
                    }
                    let carrier = expected
                        .mathematical_carriers
                        .iter()
                        .find(|row| {
                            expected.identities.text(row.type_identity)
                                == Some(type_identity.as_str())
                        })
                        .ok_or(Error::KernelCarrierMismatch)?;
                    let actual_carrier = module
                        .mathematical_carriers
                        .iter()
                        .find(|row| {
                            module.identities.text(row.type_identity)
                                == Some(type_identity.as_str())
                        })
                        .ok_or(Error::KernelCarrierMismatch)?;
                    if actual_carrier.carrier_key != carrier.carrier_key {
                        return Err(Error::KernelCarrierMismatch);
                    }
                    carrier
                }
            };
            let function = weak_head_normalize(&mut decoded.arena, &signature, result, budget)
                .map_err(Error::Kernel)?;
            let Term::Pi { domain, codomain } = decoded.arena.get(function) else {
                return Err(Error::KernelCarrierMismatch);
            };
            infer_sort(
                &mut decoded.arena,
                &Context::empty().with_signature(signature.clone()),
                domain,
                budget,
            )
            .map_err(Error::Kernel)?;
            let key = mathematical_carrier_key(
                &decoded.arena,
                &proof_admission::MathematicalCertificate {
                    term: domain,
                    expected: domain,
                    ..decoded.certificate.clone()
                },
            )
            .map_err(Error::Wire)?;
            if key != carrier.carrier_key {
                return Err(Error::KernelCarrierMismatch);
            }
            result = codomain;
        }
        infer_sort(
            &mut decoded.arena,
            &Context::empty().with_signature(signature.clone()),
            result,
            budget,
        )
        .map_err(Error::Kernel)?;
        let key = mathematical_carrier_key(
            &decoded.arena,
            &proof_admission::MathematicalCertificate {
                term: result,
                expected: result,
                ..decoded.certificate.clone()
            },
        )
        .map_err(Error::Wire)?;
        if key != expected_result_key {
            return Err(Error::KernelCarrierMismatch);
        }
        Ok(decoded)
    }
}
pub(crate) mod slots {
    use super::MathematicalProofActualError as Error;
    use semantic_vocabulary::ProofTerm;
    use std::collections::BTreeSet;
    use terminal_psi::{
        Block, CheckedMachineDeclaration, ErasedProofFormal, OperationKind, TerminalMachine,
        TerminalModule, Terminator,
    };

    pub(crate) struct Slot<'a> {
        pub key: (u64, u64, u8, u64, u64),
        pub target: (u64, u64),
        pub owner: &'a TerminalMachine,
        pub block: &'a Block,
        pub callee: Option<&'a CheckedMachineDeclaration>,
        pub formals: &'a [ErasedProofFormal],
        pub actuals: &'a [ProofTerm],
    }

    pub(crate) fn contains_mathematical(
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
    pub(crate) fn same_formals(
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
            (Some(a), Some(b)) if a.same_identity_as(&left.identities, b, &right.identities) => {
                Ok(())
            }
            _ => Err(Error::SourceCarrierMismatch),
        }
    }
    pub(crate) fn bind_slot(
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

    pub(crate) fn collect(module: &TerminalModule) -> Result<Vec<Slot<'_>>, Error> {
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
                        candidates.next().ok_or(Error::SourceCarrierMismatch)?;
                        if candidates.next().is_some() {
                            return Err(Error::SourceCarrierMismatch);
                        }
                        let contract = module
                            .invocation_machine_contract(*callee, operation.required_contract)
                            .ok_or(Error::SourceCarrierMismatch)?;
                        let formals = &contract.erased_proof_formals;
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
                                callee: contract.declaration.as_ref(),
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
                    candidates.next().ok_or(Error::SourceCarrierMismatch)?;
                    if candidates.next().is_some() {
                        return Err(Error::SourceCarrierMismatch);
                    }
                    let contract = module
                        .invocation_machine_contract(
                            invocation.callee,
                            invocation.required_contract,
                        )
                        .ok_or(Error::SourceCarrierMismatch)?;
                    let formals = &contract.erased_proof_formals;
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
                            callee: contract.declaration.as_ref(),
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

    pub(crate) fn bind_rosters(
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
}

use std::collections::BTreeSet;

use proof_admission::{
    Budget, CoreError, MathematicalCertificate, MathematicalCertificateBatchError, TermArena,
    certificate_assumption_closure, verify_mathematical_certificate,
    verify_mathematical_certificates,
};
use semantic_vocabulary::ProofTerm;

use crate::{CodecError, decode_mathematical_certificate, encode_mathematical_certificate};

#[derive(Debug)]
pub enum MathematicalProofActualError {
    NotMathematical,
    SourceCarrierMismatch,
    KernelCarrierMismatch,
    OpenContext,
    UngrantedAssumptions(BTreeSet<u32>),
    Wire(CodecError),
    Kernel(CoreError),
    KernelBatch(MathematicalCertificateBatchError),
}

pub fn mathematical_carrier_key(
    arena: &TermArena,
    certificate: &MathematicalCertificate,
) -> Result<Vec<u8>, CodecError> {
    encode_mathematical_certificate(
        arena,
        &MathematicalCertificate {
            term: certificate.expected,
            ..certificate.clone()
        },
    )
}

pub fn verify_mathematical_proof_actual(
    actual: &ProofTerm,
    expected_source_identity: &str,
    expected_carrier_key: &[u8],
    granted_assumptions: &BTreeSet<u32>,
    budget: &mut Budget,
) -> Result<(), MathematicalProofActualError> {
    let mut decoded =
        prepare_mathematical_proof_actual(actual, expected_source_identity, expected_carrier_key)?;
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, budget)
        .map_err(MathematicalProofActualError::Kernel)?;
    check_grants(&decoded, granted_assumptions)
}

fn prepare_mathematical_proof_actual(
    actual: &ProofTerm,
    expected_source_identity: &str,
    expected_carrier_key: &[u8],
) -> Result<crate::DecodedMathematicalCertificate, MathematicalProofActualError> {
    let ProofTerm::Mathematical {
        type_identity,
        certificate,
    } = actual
    else {
        return Err(MathematicalProofActualError::NotMathematical);
    };
    if type_identity != expected_source_identity {
        return Err(MathematicalProofActualError::SourceCarrierMismatch);
    }
    let decoded =
        decode_mathematical_certificate(certificate).map_err(MathematicalProofActualError::Wire)?;
    if !decoded.certificate.context.is_empty() || decoded.certificate.level_arity != 0 {
        return Err(MathematicalProofActualError::OpenContext);
    }
    if mathematical_carrier_key(&decoded.arena, &decoded.certificate)
        .map_err(MathematicalProofActualError::Wire)?
        != expected_carrier_key
    {
        return Err(MathematicalProofActualError::KernelCarrierMismatch);
    }
    Ok(decoded)
}

fn check_grants(
    decoded: &crate::DecodedMathematicalCertificate,
    granted_assumptions: &BTreeSet<u32>,
) -> Result<(), MathematicalProofActualError> {
    let closure = certificate_assumption_closure(&decoded.arena, &decoded.certificate);
    let ungranted = closure
        .difference(granted_assumptions)
        .copied()
        .collect::<BTreeSet<_>>();
    if !ungranted.is_empty() {
        return Err(MathematicalProofActualError::UngrantedAssumptions(
            ungranted,
        ));
    }
    Ok(())
}

pub fn verify_module_mathematical_actuals(
    module: &terminal_psi::TerminalModule,
    expected: &terminal_psi::TerminalModule,
    mut grants: impl FnMut(
        &terminal_psi::TerminalMathematicalCarrier,
    ) -> Result<BTreeSet<u32>, MathematicalProofActualError>,
    budget: &mut Budget,
) -> Result<(), MathematicalProofActualError> {
    use MathematicalProofActualError as Error;
    let actual_slots = slots::collect(module)?;
    let expected_slots = slots::collect(expected)?;
    slots::bind_rosters(module, expected)?;
    if actual_slots.len() != expected_slots.len() {
        return Err(Error::SourceCarrierMismatch);
    }
    let mut pending = Vec::new();
    let mut matched = BTreeSet::new();
    for actual in &actual_slots {
        let mut candidates = expected_slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.key == actual.key);
        let (ordinal, trusted) = candidates.next().ok_or(Error::SourceCarrierMismatch)?;
        if candidates.next().is_some() || !matched.insert(ordinal) {
            return Err(Error::SourceCarrierMismatch);
        }
        slots::bind_slot(module, actual, expected, trusted)?;
        if actual.actuals.len() != trusted.formals.len()
            || trusted.actuals.len() != trusted.formals.len()
        {
            return Err(Error::SourceCarrierMismatch);
        }
        for ((term, formal), source_term) in actual
            .actuals
            .iter()
            .zip(trusted.formals)
            .zip(trusted.actuals)
        {
            let carrier = expected
                .mathematical_carriers
                .iter()
                .find(|row| row.type_identity == formal.type_identity);
            match (term, carrier) {
                (ProofTerm::Mathematical { .. }, Some(carrier)) => {
                    let identity = expected
                        .identities
                        .text(formal.type_identity)
                        .ok_or(Error::SourceCarrierMismatch)?;
                    let actual_carrier = module
                        .mathematical_carriers
                        .iter()
                        .find(|row| module.identities.text(row.type_identity) == Some(identity))
                        .ok_or(Error::KernelCarrierMismatch)?;
                    if actual_carrier.carrier_key != carrier.carrier_key {
                        return Err(Error::KernelCarrierMismatch);
                    }
                    let decoded =
                        prepare_mathematical_proof_actual(term, identity, &carrier.carrier_key)?;
                    pending.push((decoded, grants(carrier)?));
                }
                (
                    ProofTerm::MathematicalApplication {
                        type_identity,
                        certificate,
                        arguments,
                        formal_scope,
                    },
                    Some(carrier),
                ) => {
                    let identity = expected
                        .identities
                        .text(formal.type_identity)
                        .ok_or(Error::SourceCarrierMismatch)?;
                    if type_identity != identity {
                        return Err(Error::SourceCarrierMismatch);
                    }
                    let actual_carrier = module
                        .mathematical_carriers
                        .iter()
                        .find(|row| module.identities.text(row.type_identity) == Some(identity))
                        .ok_or(Error::KernelCarrierMismatch)?;
                    if actual_carrier.carrier_key != carrier.carrier_key {
                        return Err(Error::KernelCarrierMismatch);
                    }
                    let (caller, trusted_caller) = match formal_scope {
                        semantic_vocabulary::ProofFormalScope::Block => (
                            &actual.block.erased_proof_formals,
                            &trusted.block.erased_proof_formals,
                        ),
                        semantic_vocabulary::ProofFormalScope::Invocation => (
                            &actual.owner.contract.erased_proof_formals,
                            &trusted.owner.contract.erased_proof_formals,
                        ),
                    };
                    let ProofTerm::MathematicalApplication {
                        arguments: source_arguments,
                        formal_scope: source_scope,
                        ..
                    } = source_term
                    else {
                        return Err(Error::SourceCarrierMismatch);
                    };
                    if arguments != source_arguments || formal_scope != source_scope {
                        return Err(Error::SourceCarrierMismatch);
                    }
                    let decoded = applications::prepare(
                        module,
                        caller,
                        expected,
                        trusted_caller,
                        certificate,
                        arguments,
                        &carrier.carrier_key,
                        actual,
                        trusted,
                        budget,
                    )?;
                    pending.push((decoded, grants(carrier)?));
                }
                (
                    ProofTerm::Formal { position } | ProofTerm::InvocationFormal { position },
                    Some(carrier),
                ) => {
                    let _authorized = grants(carrier)?;
                    let (caller, trusted_caller) =
                        if matches!(term, ProofTerm::InvocationFormal { .. }) {
                            (
                                &actual.owner.contract.erased_proof_formals,
                                &trusted.owner.contract.erased_proof_formals,
                            )
                        } else {
                            (
                                &actual.block.erased_proof_formals,
                                &trusted.block.erased_proof_formals,
                            )
                        };
                    let source = caller
                        .get(*position as usize)
                        .ok_or(Error::SourceCarrierMismatch)?;
                    let trusted_source = trusted_caller
                        .get(*position as usize)
                        .ok_or(Error::SourceCarrierMismatch)?;
                    if module.identities.text(source.type_identity)
                        != expected.identities.text(formal.type_identity)
                        || expected.identities.text(trusted_source.type_identity)
                            != expected.identities.text(formal.type_identity)
                    {
                        return Err(Error::SourceCarrierMismatch);
                    }
                }
                (_, Some(_)) => return Err(Error::NotMathematical),
                (_, None)
                    if slots::contains_mathematical(
                        module,
                        term,
                        &actual.block.erased_proof_formals,
                        &actual.owner.contract.erased_proof_formals,
                    ) =>
                {
                    return Err(Error::KernelCarrierMismatch);
                }
                _ => {}
            }
        }
    }
    let judgments = pending
        .iter()
        .map(|(decoded, _)| (&decoded.arena, &decoded.certificate))
        .collect::<Vec<_>>();
    verify_mathematical_certificates(&judgments, budget).map_err(Error::KernelBatch)?;
    for (decoded, granted) in &pending {
        check_grants(decoded, granted)?;
    }
    Ok(())
}

pub(crate) mod indexed_theorem_roots {

    use std::collections::BTreeSet;

    use proof_admission::{
        Budget, certificate_assumption_closure, verify_mathematical_certificates,
    };
    use terminal_psi::{TerminalIndexedTheorem, TerminalModule};

    use crate::{
        MathematicalProofActualError as Error, decode_mathematical_certificate,
        mathematical_carrier_key,
    };

    pub fn verify_indexed_theorem_roots(
        module: &TerminalModule,
        expected: &TerminalModule,
        mut grants: impl FnMut(&TerminalIndexedTheorem) -> Result<BTreeSet<u32>, Error>,
        budget: &mut Budget,
    ) -> Result<(), Error> {
        if module.indexed_theorems.len() != expected.indexed_theorems.len() {
            return Err(Error::KernelCarrierMismatch);
        }
        let mut matched = vec![false; expected.indexed_theorems.len()];
        let mut pending = Vec::new();
        for actual in &module.indexed_theorems {
            if actual.evidence_source_position as usize
                >= actual.source_contract.parameter_types.len()
                || module
                    .identities
                    .text(actual.type_identity)
                    .is_none_or(str::is_empty)
                || module
                    .identities
                    .text(actual.source_contract.semantic_declaration)
                    .is_none_or(str::is_empty)
                || module
                    .identities
                    .text(actual.source_contract.callable)
                    .is_none_or(str::is_empty)
            {
                return Err(Error::SourceCarrierMismatch);
            }
            let candidates = expected
                .indexed_theorems
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    actual.source_contract.same_identity_as(
                        &module.identities,
                        &row.source_contract,
                        &expected.identities,
                    ) && actual.ensure_ordinal == row.ensure_ordinal
                        && actual.evidence_source_position == row.evidence_source_position
                        && module
                            .identities
                            .text(actual.type_identity)
                            .filter(|text| !text.is_empty())
                            == expected
                                .identities
                                .text(row.type_identity)
                                .filter(|text| !text.is_empty())
                })
                .collect::<Vec<_>>();
            let [(position, trusted)] = candidates.as_slice() else {
                return Err(Error::SourceCarrierMismatch);
            };
            if matched[*position] {
                return Err(Error::SourceCarrierMismatch);
            }
            matched[*position] = true;
            let decoded =
                decode_mathematical_certificate(&actual.certificate).map_err(Error::Wire)?;
            let expected_judgment =
                decode_mathematical_certificate(&trusted.certificate).map_err(Error::Wire)?;
            if !decoded.certificate.context.is_empty()
                || !expected_judgment.certificate.context.is_empty()
                || decoded.certificate.level_arity != expected_judgment.certificate.level_arity
            {
                return Err(Error::OpenContext);
            }
            if mathematical_carrier_key(&decoded.arena, &decoded.certificate)
                .map_err(Error::Wire)?
                != mathematical_carrier_key(
                    &expected_judgment.arena,
                    &expected_judgment.certificate,
                )
                .map_err(Error::Wire)?
            {
                return Err(Error::KernelCarrierMismatch);
            }
            pending.push((decoded, actual, grants(trusted)?));
        }
        let judgments = pending
            .iter()
            .map(|(decoded, _, _)| (&decoded.arena, &decoded.certificate))
            .collect::<Vec<_>>();
        verify_mathematical_certificates(&judgments, budget).map_err(Error::KernelBatch)?;
        for (decoded, actual, granted) in &pending {
            let closure = certificate_assumption_closure(&decoded.arena, &decoded.certificate);
            if closure.iter().copied().collect::<Vec<_>>() != actual.assumption_closure {
                return Err(Error::KernelCarrierMismatch);
            }
            let ungranted = closure
                .difference(granted)
                .copied()
                .collect::<BTreeSet<_>>();
            if !ungranted.is_empty() {
                return Err(Error::UngrantedAssumptions(ungranted));
            }
        }
        Ok(())
    }
}
