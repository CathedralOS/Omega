//! Bind each checked POSIX import contract to all of its selected call sites.
#[path = "posix_imports/contracts.rs"]
mod contracts;
use super::{TerminalAuthorityPolicy, TerminalAuthorityPolicyRow};
use crate::{
    AdmittedBoundarySettlement, AdmittedTerminalMechanism, digest_encoder::DomainDigestEncoder,
};
pub(super) use contracts::Contract;
use semantic_vocabulary::{
    BoundaryMachineId, InvocationSite, TerminalAuthorityDisposition, ValueId,
};
use target_operations::BoundarySettlementRealization;
use terminal_authority_receipts::{
    CheckedSyscallArgumentContractIdentity, NormalizedForeignTerminalMechanismIdentity,
    TerminalMechanismIdentity,
};
use terminal_psi::BoundaryMachineDeclaration;
use terminal_semantics::MachineScalarIndex;

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

fn verify_calls(
    contract: Contract,
    module: &terminal_psi::TerminalModule,
    plan: &abstract_operations::AbstractOperationPlan,
    declaration: &BoundaryMachineDeclaration,
    proof: &mut DomainDigestEncoder,
) -> Option<()> {
    let mut found = false;
    let mut coordinate = Vec::with_capacity(9);
    for function in &plan.functions {
        let machine = module.machine(function.machine)?;
        let mut scalars = None;
        for block in &function.blocks {
            let mut original = machine
                .block(&block.block)
                .into_iter()
                .flat_map(|block| block.boundary_invocations())
                .filter(|call| call.boundary == declaration.id);
            let selected = block.operations.iter().filter_map(|op| {
                let (site, args) = selected_call(op, declaration.id)?;
                Some((site, args, op.invocation_arguments()))
            });
            for (site, args, structures) in selected {
                let call = original.next()?;
                let scalars = scalars.get_or_insert_with(|| MachineScalarIndex::new(machine));
                if site != call.site
                    || !structures.is_empty()
                    || !call.structural_arguments.is_empty()
                    || !contract.admits_arguments(
                        args,
                        call.arguments,
                        &declaration.scalar_parameters,
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

fn checked_mechanism(
    contract: Contract,
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
    let locator = binding.foreign_import_locator()?;
    if plan.psi != artifact.closed_module().identity()
        || plan.boundary_machine(boundary) != Some(declaration)
        || locator.identity_digest() != raw.locator_identity()
        || binding.boundary_entry_plan.contract_commitment_digest()
            != raw.implementation_contract().as_bytes()
        || !contract.admits_signature(declaration, &binding.boundary_entry_plan)
    {
        return None;
    }
    // The receipt identifies the original program, native import and every
    // selected occurrence satisfying this contract.
    let mut proof = DomainDigestEncoder::new(contract.domain());
    proof.raw(artifact.manifest().identity().as_bytes());
    proof.u64(plan.entry.get());
    proof.u64(boundary.get());
    proof.bytes(&terminal_authority_receipts::terminal_mechanism_identity_bytes(raw.into()));
    verify_calls(contract, module, plan, declaration, &mut proof)?;
    Some(
        raw.with_checked_argument_contract(CheckedSyscallArgumentContractIdentity::from_digest(
            proof.finish(),
        ))
        .into(),
    )
}

pub(crate) fn classify_checked_calls(
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
        let Some(contract) = Contract::for_import(raw) else {
            continue;
        };
        if raw.argument_contract()
            != terminal_authority_receipts::NormalizedForeignArgumentContract::AdmittedPlan
            || supplied.classify(leaf.mechanism).is_ok()
        {
            continue;
        }
        if let Some(checked) = settlements
            .iter()
            .find(|s| s.boundary == leaf.boundary)
            .and_then(|settlement| {
                checked_mechanism(contract, artifact, plan, leaf.boundary, raw, settlement)
            })
        {
            leaf.mechanism = checked;
            rows.push(TerminalAuthorityPolicyRow::new(
                checked,
                TerminalAuthorityDisposition::from_classes(contract.classes().iter().copied()),
            ));
        }
    }
}
