use super::rejected;
use crate::capture::contracts::{
    expressions::projection::project_contract_expression, facts::ContractProjectionContext,
};
use crate::capture::semantics::facts::exactly_one;
use crate::record::{
    PackagePolicyCrash, PackagePolicyCrashGuard, PackagePolicyCrashRoute,
    PackagePolicyInferredCrash, PackageReviewCrashInterface,
};
use omega_compiler::CheckedCompilation;
use psi_checked_trees::RealizedMachineContractEnvelope;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::{
    domain::ProofFact,
    expression::ExpressionNode,
    machine::Machine,
    signature::{CrashCause, SignatureContractKind},
    state::State,
};

pub(crate) fn crash(
    compilation: &CheckedCompilation,
    machine: &Machine,
    entry: &State,
    binders: &[(SymbolHandle, String)],
    envelope: &RealizedMachineContractEnvelope,
    inferred_causes: &[(SymbolHandle, Vec<psi_checked_trees::CrashCause>)],
) -> Result<PackagePolicyCrash, Vec<Diagnostic>> {
    let plan = exactly_one(
        compilation
            .facts
            .contract_plans
            .machines
            .iter()
            .filter(|plan| plan.machine == machine.symbol),
        machine.name.as_str(),
        "machine contract plan",
    )?;
    // Concrete machines own a MachineContractPlan. Separate capsules belong
    // only to trait/static signatures, as in the checked call-target owner.
    let derived = psi_typed_trees_to_checked_trees::derive_authored_machine_crash_buckets(
        &compilation.typed,
        machine,
    );
    if envelope.machine != machine.symbol
        || compilation
            .machine_states(machine)
            .first()
            .map(|state| state.symbol)
            != Some(entry.symbol)
        || plan.commitment != envelope.contract_commitment
        || derived != plan.crash.published()
        || envelope.checked_crash != plan.crash
    {
        return Err(rejected(
            "authored crash routes do not equal their exact checked entry plan and envelope",
        ));
    }
    let context = ContractProjectionContext {
        subject_kind: "callable",
        subject_name: machine.name.as_str(),
        owner: psi_checked_trees::ContractProofFactOwner::Machine {
            machine_symbol: machine.symbol,
        },
        point: psi_facts::ProgramPoint::Machine {
            machine_symbol: machine.symbol,
        },
        parameters: compilation.state_parameters(entry),
        domain_symbol: None,
        data_symbol: None,
        lifetime_binders: &machine.lifetime_parameters,
        lifetime_substitutions: &[],
        selection_exposure: if machine.is_public || machine.supply_mode.is_boundary_declaration() {
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
        },
    };
    let published = crash_routes(
        compilation,
        compilation.machine_contracts(machine),
        &context,
        binders,
        &derived,
    )?;
    let mut matching = inferred_causes
        .iter()
        .filter(|(candidate, _)| *candidate == machine.symbol);
    let inferred = matching.next();
    if matching.next().is_some() {
        return Err(rejected(
            "duplicate inferred crash summary for one exact machine",
        ));
    }
    Ok(PackagePolicyCrash {
        interface: match plan.crash.interface() {
            psi_checked_trees::CrashInterface::InternalInferred => {
                PackageReviewCrashInterface::InternalInferred
            }
            psi_checked_trees::CrashInterface::PublishedCeiling => {
                PackageReviewCrashInterface::PublishedCeiling
            }
        },
        published,
        structural_runtime_requirements: plan.crash.structural_runtime_requirements().map(
            |requirements| {
                requirements
                    .iter()
                    .map(crate::capture::behavior::crash::project_boolean_expression)
                    .collect()
            },
        ),
        inferred: inferred.map_or(PackagePolicyInferredCrash::Unknown, |(_, causes)| {
            PackagePolicyInferredCrash::Complete {
                causes: causes
                    .iter()
                    .map(|cause| match cause {
                        psi_checked_trees::CrashCause::Trap => {
                            crate::record::PackageReviewCrashCause::Trap
                        }
                        psi_checked_trees::CrashCause::Abort => {
                            crate::record::PackageReviewCrashCause::Abort
                        }
                    })
                    .collect(),
            }
        }),
    })
}

pub(crate) fn crash_routes(
    compilation: &CheckedCompilation,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    buckets: &[psi_checked_trees::CrashRouteBucket],
) -> Result<Vec<PackagePolicyCrashRoute>, Vec<Diagnostic>> {
    let mut published = Vec::new();
    for bucket in buckets {
        let cause = match bucket.cause() {
            psi_checked_trees::CrashCause::Trap => CrashCause::Trap,
            psi_checked_trees::CrashCause::Abort => CrashCause::Abort,
        };
        let mut guards = Vec::new();
        if bucket.alternative_guards() == [psi_checked_trees::CrashRouteGuard::Truth] {
            guards.push(PackagePolicyCrashGuard::Truth);
        } else {
            for contract in contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Crashes { cause })
            {
                for offset in 0..contract.facts.count() {
                    let fact_handle = psi_arena::Handle::from_parts(
                        contract.facts.start().arena_index() + offset,
                        contract.facts.start().generation(),
                    );
                    let ProofFact::Expression(expression) =
                        compilation.proof_facts.get(fact_handle)
                    else {
                        return Err(rejected(
                            "authored crash guard is not a checked expression fact",
                        ));
                    };
                    if matches!(
                        compilation.expression_table.expression(*expression),
                        ExpressionNode::Boolean(true)
                    ) {
                        return Err(rejected(
                            "guarded crash capsule disagrees with its unconditional source fact",
                        ));
                    }
                    guards.push(PackagePolicyCrashGuard::Expression(
                        project_contract_expression(
                            compilation,
                            context,
                            binders,
                            *expression,
                            Some(fact_handle),
                            0,
                        )?,
                    ));
                }
            }
            guards.sort();
            guards.dedup();
            if guards.is_empty() {
                return Err(rejected("guarded crash bucket has no exact authored facts"));
            }
        }
        published.push(PackagePolicyCrashRoute {
            cause: crate::capture::behavior::project_crash_cause(bucket.cause()),
            alternative_guards: guards,
        });
    }
    published.sort();
    Ok(published)
}
