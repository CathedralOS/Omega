//! Runtime callable signatures use the same checked predicate vocabulary as
//! ordinary machine contracts. The catalog supplies the exact formal namespace.

use checked_trees::CheckedTrees;
use semantic_vocabulary::ContractId;
use terminal_psi::{ContractClause, ErasedProofFormal, MachineContract, ValueDeclaration};

use crate::scalar_lowering::contracts::{self, ContractViewNamespace};
use crate::shared::lowering_error::{LoweringError, unsupported};

pub(crate) struct RuntimeContractNamespace<'namespace> {
    pub scalar: &'namespace [ValueDeclaration],
    pub erased_scalar: &'namespace [ValueDeclaration],
    pub erased_proof: &'namespace [ErasedProofFormal],
    pub scalar_result: Option<ValueDeclaration>,
    pub views: ContractViewNamespace<'namespace>,
}

pub(crate) fn lower_runtime_callable_contract(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source: typed_trees::callable_values::RuntimeCallableContractHandle,
    id: ContractId,
    namespace: RuntimeContractNamespace<'_>,
    next_obligation: &mut u64,
) -> Result<MachineContract, LoweringError> {
    if !checked.typed.runtime_callable_contracts.is_valid(source) {
        return unsupported("runtime callable proof lost its exact signature row");
    }
    let row = checked.typed.runtime_callable_contract(source);
    let plan = checked
        .facts
        .contract_plans
        .runtime_callable_scalar_contract(source)
        .ok_or(LoweringError::Unsupported(
            "runtime callable signature has no checked predicate plan",
        ))?;
    let capsule = checked
        .facts
        .contract_plans
        .runtime_callable_crash_capsule(source)
        .ok_or(LoweringError::Unsupported(
            "runtime callable signature has no unique published crash capsule",
        ))?;
    if capsule.runtime_callable_source() != Some((source, row.lexical_owner)) {
        return unsupported("runtime callable proof capsule belongs to another signature scope");
    }
    if plan.has_outcome_specific_clauses() {
        return unsupported(
            "runtime callable guarded guarantees require exact row-owned outcome evidence",
        );
    }
    let mut requires = contracts::clauses(
        identities,
        plan.requires(),
        namespace.scalar,
        namespace.erased_scalar,
        &namespace.views,
    )?
    .into_iter()
    .collect::<Vec<_>>();
    contracts::canonicalize_requires(&mut requires)?;
    let mut result_namespace = namespace.scalar.to_vec();
    result_namespace.extend(namespace.scalar_result);
    let guarantees = contracts::clauses(
        identities,
        plan.ensures(),
        &result_namespace,
        namespace.erased_scalar,
        &namespace.views,
    )?;
    let ensures = guarantees
        .into_iter()
        .map(|proposition| {
            Ok(ContractClause {
                obligation: crate::shared::terminal_identities::obligation_id(
                    crate::shared::terminal_identities::allocate_dense(next_obligation)?,
                ),
                proposition,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let crash_routes = crate::proofs::crash_routes::lower_structural_crash_route_buckets(
        identities,
        capsule.published_buckets(),
        namespace.scalar,
        namespace.views.parameters,
        namespace.views.structural_types,
        &requires,
    )?;
    Ok(MachineContract {
        id,
        crash_routes,
        erased_scalar_formals: namespace.erased_scalar.to_vec(),
        erased_proof_formals: namespace.erased_proof.to_vec(),
        source_checked_requires: requires.clone(),
        requires,
        ensures,
        outcome_specific_ensures: Vec::new(),
    })
}

/// Every returned certificate proves the corresponding reconstructed goal in
/// the existing lowered-form proof owner. The descriptor must already retain
/// its exact canonical obligation IDs.
pub(crate) fn derive_descriptor_refinement_evidence(
    module: &terminal_psi::TerminalModule,
    descriptor: semantic_vocabulary::MachineDescriptorId,
) -> Result<Vec<terminal_psi::CertificateEnvelope>, LoweringError> {
    let retained = module
        .callables
        .descriptor(descriptor)
        .ok_or(LoweringError::Unsupported(
            "refinement has no retained machine descriptor",
        ))?;
    let contract =
        module
            .callables
            .contract(retained.contract)
            .ok_or(LoweringError::Unsupported(
                "refinement has no required callable contract",
            ))?;
    let context = terminal_semantics::calls::callable_refinement::callable_refinement_context(
        module,
        retained.contract,
    )
    .map_err(LoweringError::Unsupported)?;
    let mut declarations = contract
        .scalar_parameters
        .iter()
        .chain(&contract.proof.erased_scalar_formals)
        .chain(contract.proof_result.scalar_ref())
        .map(|value| (value.id, value.scalar_type))
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(id, _)| *id);
    if declarations.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return unsupported("runtime callable proof namespace repeats a scalar formal identity");
    }
    terminal_semantics::calls::callable_refinement::descriptor_refinement_goals(module, descriptor)
        .map_err(LoweringError::Unsupported)?
        .into_iter()
        .map(|(id, goal)| {
            lowered_discharge::derive_contract_obligation(
                &context,
                id,
                &goal.proposition,
                &declarations,
            )
            .map_err(LoweringError::Unsupported)
        })
        .collect()
}
