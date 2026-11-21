//! Darwin's poll waits on file descriptors supplied by the caller. A zero
//! count makes the wait independent of descriptors. The calling convention
//! does not constrain the count.

use calling_conventions::{BoundaryEntryPlan, CallSignature, CallingPolicy, ValueShape};
use semantic_vocabulary::{
    BoundaryMachineId, IntegerSign, IntegerValue, InvocationSite, ScalarType,
    TerminalAuthorityDisposition, ValueId,
};
use terminal_authority_receipts::{
    CheckedSyscallArgumentContractIdentity, NormalizedForeignTerminalMechanismIdentity,
    TerminalMechanismIdentity,
};
use terminal_psi::{BoundaryMachineDeclaration, BoundaryParameterKind};
use terminal_semantics::MachineScalarIndex;

use super::{TerminalAuthorityPolicy, TerminalAuthorityPolicyRow};
use crate::{
    AdmittedBoundarySettlement, AdmittedTerminalMechanism, digest_encoder::DomainDigestEncoder,
};
use target_operations::{BoundarySettlementRealization, NormalizedForeignCallMechanism};

pub(super) fn is_poll(import: NormalizedForeignTerminalMechanismIdentity) -> bool {
    if import.target() != target::TargetProfile::MacosArm64 {
        return false;
    }
    static LOCATOR: std::sync::OnceLock<target::ForeignLocatorIdentityDigest> =
        std::sync::OnceLock::new();
    import.locator_identity()
        == *LOCATOR.get_or_init(|| {
            super::host_cohorts::darwin_clock_locator(b"_poll")
                .expect("reviewed Darwin libSystem poll locator")
        })
}

fn has_poll_signature(declaration: &BoundaryMachineDeclaration, entry: &BoundaryEntryPlan) -> bool {
    let [
        ScalarType::Integer(pointer),
        ScalarType::Integer(count),
        ScalarType::Integer(timeout),
    ] = declaration.scalar_parameters.as_slice()
    else {
        return false;
    };
    let word64 = ValueShape::integer(8, 8);
    let word32 = ValueShape::integer(4, 4);
    // Darwin reads an unsigned 32-bit count. The library also passes it
    // in a 64-bit argument. A checked zero is zero in both placements.
    let count_word = match count.bits() {
        Some(32) => word32,
        Some(64) => word64,
        _ => return false,
    };
    declaration.parameter_order == [BoundaryParameterKind::Scalar; 3]
        && declaration.structural_parameters.is_empty()
        && declaration.completion == terminal_psi::BoundaryCompletion::NormalReturn
        && pointer.bits() == Some(64)
        && pointer.sign() == IntegerSign::Unsigned
        && !pointer.is_address()
        && count.sign() == IntegerSign::Unsigned
        && !count.is_address()
        && timeout.bits() == Some(32)
        && timeout.sign() == IntegerSign::Signed
        && !timeout.is_address()
        && (declaration.result.is_unit()
            || declaration.result.scalar() == Some(ScalarType::Integer(*timeout)))
        && calling_conventions::evaluate_call_plan(
            CallingPolicy::DarwinAapcs64,
            &CallSignature {
                parameters: vec![word64, count_word, word32],
                result: declaration.result.scalar().map(|_| word32),
            },
        )
        .is_ok_and(|expected| expected == entry.call)
}

fn selected_call(
    operation: &abstract_operations::AbstractOperation,
    required: BoundaryMachineId,
) -> Option<(InvocationSite, &[ValueId])> {
    use abstract_operations::AbstractOperation;
    let (boundary, site, arguments) = match operation {
        AbstractOperation::BoundaryCall {
            boundary,
            psi_operation,
            arguments,
            ..
        } => (
            *boundary,
            InvocationSite::Operation(*psi_operation),
            arguments,
        ),
        AbstractOperation::ExternalCompletion {
            boundary,
            psi_edge,
            arguments,
            ..
        } => (
            *boundary,
            InvocationSite::ExternalCompletion(*psi_edge),
            arguments,
        ),
        _ => return None,
    };
    (boundary == required).then_some((site, arguments.as_slice()))
}

fn has_zero_count(
    selected: &[ValueId],
    original: &[ValueId],
    count_type: ScalarType,
    constant: impl FnOnce(ValueId) -> Option<(ScalarType, IntegerValue)>,
) -> bool {
    let [_, count, _] = original else {
        return false;
    };
    selected == original && constant(*count) == Some((count_type, IntegerValue::Unsigned(0)))
}

fn verify_calls(
    module: &terminal_psi::TerminalModule,
    plan: &abstract_operations::AbstractOperationPlan,
    declaration: &BoundaryMachineDeclaration,
    proof: &mut DomainDigestEncoder,
) -> Option<()> {
    let mut found = false;
    let mut coordinate = Vec::with_capacity(9);
    for function in &plan.functions {
        let mut scalars = None;
        for block in &function.blocks {
            let mut selected = block
                .operations
                .iter()
                .filter_map(|op| {
                    let (site, args) = selected_call(op, declaration.id)?;
                    Some((site, args, op.invocation_arguments()))
                })
                .peekable();
            if selected.peek().is_none() {
                continue;
            }
            if scalars.is_none() {
                scalars = Some(MachineScalarIndex::new(module.machine(function.machine)?));
            }
            let scalars = scalars.as_ref()?;
            let mut original = scalars
                .machine()
                .block(&block.block)?
                .boundary_invocations()
                .filter(|call| call.boundary == declaration.id);
            for (site, args, structures) in selected {
                let call = original.next()?;
                if site != call.site
                    || !structures.is_empty()
                    || !call.structural_arguments.is_empty()
                    || !has_zero_count(
                        args,
                        call.arguments,
                        declaration.scalar_parameters[1],
                        |id| scalars.constant_value(id),
                    )
                {
                    return None;
                }
                found = true;
                proof.u64(function.machine.get());
                proof.u64(block.block.get());
                coordinate.clear();
                site.encode_identity(&mut coordinate);
                proof.bytes(&coordinate);
            }
            if original.next().is_some() {
                return None;
            }
        }
    }
    found.then_some(())
}

fn checked_wait(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    plan: &abstract_operations::AbstractOperationPlan,
    boundary: BoundaryMachineId,
    raw: NormalizedForeignTerminalMechanismIdentity,
    settlement: &AdmittedBoundarySettlement<'_>,
) -> Option<TerminalMechanismIdentity> {
    let module = artifact.semantic_module();
    let declaration = module.boundary_machine(boundary)?;
    let BoundarySettlementRealization::NormalizedForeignCall(binding) = &settlement.realization
    else {
        return None;
    };
    let NormalizedForeignCallMechanism::Import { locator, .. } = &binding.mechanism else {
        return None;
    };
    if plan.psi != artifact.closed_module().identity()
        || plan.boundary_machine(boundary) != Some(declaration)
        || locator.identity_digest() != raw.locator_identity()
        || binding.boundary_entry_plan.contract_commitment_digest()
            != raw.implementation_contract().as_bytes()
        || !has_poll_signature(declaration, &binding.boundary_entry_plan)
    {
        return None;
    }
    // The receipt identifies the original program, native import and every
    // selected call whose count the checked program proves is zero.
    let mut proof = DomainDigestEncoder::new(b"omega.macos.poll.zero-descriptor-count.v1\0");
    proof.raw(artifact.manifest().identity().as_bytes());
    proof.u64(plan.entry.get());
    proof.u64(boundary.get());
    proof.bytes(&terminal_authority_receipts::terminal_mechanism_identity_bytes(raw.into()));
    verify_calls(module, plan, declaration, &mut proof)?;
    Some(
        raw.with_checked_argument_contract(CheckedSyscallArgumentContractIdentity::from_digest(
            proof.finish(),
        ))
        .into(),
    )
}

pub(crate) fn classify_checked_waits(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    plan: &abstract_operations::AbstractOperationPlan,
    settlements: &[AdmittedBoundarySettlement<'_>],
    mechanisms: &mut [AdmittedTerminalMechanism],
    supplied: &TerminalAuthorityPolicy,
    rows: &mut Vec<TerminalAuthorityPolicyRow>,
) {
    for leaf in mechanisms {
        let TerminalMechanismIdentity::NormalizedForeign(raw) = leaf.mechanism else {
            continue;
        };
        if !is_poll(raw)
            || raw.argument_contract()
                != terminal_authority_receipts::NormalizedForeignArgumentContract::AdmittedPlan
            || supplied.classify(leaf.mechanism).is_ok()
        {
            continue;
        }
        if let Some(wait) = settlements
            .iter()
            .find(|s| s.boundary == leaf.boundary)
            .and_then(|settlement| checked_wait(artifact, plan, leaf.boundary, raw, settlement))
        {
            leaf.mechanism = wait;
            rows.push(TerminalAuthorityPolicyRow::new(
                wait,
                TerminalAuthorityDisposition::from_classes([]),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::IntegerType;

    #[test]
    fn a_wait_needs_the_zero_in_the_original_descriptor_count_argument() {
        let word = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let args = [
            ValueId::FIRST,
            ValueId::new(2).unwrap(),
            ValueId::new(3).unwrap(),
        ];
        let known_zero = |value| (value == args[1]).then_some((word, IntegerValue::Unsigned(0)));
        assert!(has_zero_count(&args, &args, word, known_zero));
        assert!(!has_zero_count(&args, &args, word, |_| None));
        assert!(!has_zero_count(&args, &args, word, |value| (value
            == args[0])
            .then_some((word, IntegerValue::Unsigned(0)))));
        assert!(!has_zero_count(
            &args,
            &[args[1], args[0], args[2]],
            word,
            known_zero
        ));
        assert!(!has_zero_count(&args, &args, word, |_| Some((
            word,
            IntegerValue::Unsigned(1)
        ))));
    }
}
