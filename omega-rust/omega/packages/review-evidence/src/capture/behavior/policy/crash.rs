/*
Callable crash policy projection (`crash`, `validate_crash_publication`, and
`crash_routes`).

Concrete machines own a `MachineContractPlan`, which we cross-check against
`derive_authored_machine_crash_buckets` and the realized contract envelope
(including virtual family heads and their members against immutable compiler
custody). When projecting guarded crash routes in `crash_routes`, we pass `None`
for the proof-fact handle because crash predicates are ceiling guards rather
than established requires/ensures facts with proof-place dependencies.
*/

use super::rejected;
use crate::capture::contracts::{
    expressions::projection::project_contract_expression, facts::ContractProjectionContext,
};
use crate::capture::semantics::facts::exactly_one;
use crate::capture::{PackageReviewInput, SourceDeclarationInput};
use crate::record::{
    PackagePolicyCrash, PackagePolicyCrashGuard, PackagePolicyCrashRoute,
    PackagePolicyInferredCrash, PackageReviewCrashCause, PackageReviewCrashInterface,
};
use checked_trees::CrashInterface;
use checked_trees::RealizedMachineContractEnvelope;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    expression::ExpressionNode,
    machine::Machine,
    signature::{CrashCause, SignatureContractKind},
    state::State,
};
use typed_trees_to_checked_trees::derive_authored_machine_crash_buckets;

pub(crate) fn crash(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
    entry: &State,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[typed_trees::name::Identifier],
    envelope: &RealizedMachineContractEnvelope,
    inferred_causes: &[(SymbolHandle, Vec<checked_trees::CrashCause>)],
) -> Result<PackagePolicyCrash, Vec<Diagnostic>> {
    let plan = machine_contract_plan(compilation.program, machine)?;
    let derived = derive_authored_machine_crash_buckets(compilation.checking_input, machine)
        .map_err(|diagnostic| vec![diagnostic])?;
    validate_crash_publication(compilation, machine, plan, &derived)?;
    if envelope.machine != machine.symbol
        || compilation
            .machine_states(machine)
            .first()
            .map(|state| state.symbol)
            != Some(entry.symbol)
        || plan.commitment != envelope.contract_commitment
        || envelope.checked_crash != plan.crash
    {
        return Err(rejected(
            "authored crash routes do not equal their exact checked entry plan and envelope",
        ));
    }
    let context = ContractProjectionContext {
        subject_kind: "callable",
        subject_name: machine.name.as_str(),
        owner: checked_trees::ContractProofFactOwner::Machine {
            machine_symbol: machine.symbol,
        },
        point: facts::ProgramPoint::Machine {
            machine_symbol: machine.symbol,
        },
        parameters: compilation.state_parameters(entry),
        domain_symbol: None,
        data_symbol: None,
        lifetime_binders,
        lifetime_substitutions: &[],
        selection_exposure: crate::capture::contracts::facts::callable_contract_exposure(machine),
    };
    let published = crash_routes(
        compilation,
        compilation.machine_contracts(machine),
        &context,
        binders,
        plan.crash.published(),
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
            CrashInterface::InternalInferred => PackageReviewCrashInterface::InternalInferred,
            CrashInterface::PublishedCeiling => PackageReviewCrashInterface::PublishedCeiling,
        },
        published,
        structural_runtime_requirements: plan
            .crash
            .structural_runtime_requirements()
            .map(|requirements| {
                requirements
                    .iter()
                    .map(crate::capture::behavior::crash::project_boolean_expression)
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        rejected("crash requirements contain an unsupported scalar expression")
                    })
            })
            .transpose()?,
        inferred: inferred.map_or(PackagePolicyInferredCrash::Unknown, |(_, causes)| {
            PackagePolicyInferredCrash::Complete {
                causes: causes
                    .iter()
                    .map(|cause| match cause {
                        checked_trees::CrashCause::Trap => PackageReviewCrashCause::Trap,
                        checked_trees::CrashCause::Abort => PackageReviewCrashCause::Abort,
                    })
                    .collect(),
            }
        }),
    })
}

fn validate_crash_publication(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
    plan: &checked_trees::MachineContractPlan,
    authored: &[checked_trees::CrashRouteBucket],
) -> Result<(), Vec<Diagnostic>> {
    let Some(members) = crate::capture::source::family_interfaces::members(compilation, machine)?
    else {
        if authored != plan.crash.published() {
            return Err(rejected(
                "authored crash routes do not equal their exact checked publication",
            ));
        }
        return Ok(());
    };
    let original_machine = exactly_one(
        compilation
            .original_program
            .machines()
            .iter()
            .filter(|candidate| candidate.symbol == machine.symbol),
        machine.name.as_str(),
        "family head declaration custody",
    )?;
    let original_head = machine_contract_plan(compilation.original_program, original_machine)?;
    let original_authored = derive_authored_machine_crash_buckets(
        compilation.original_checking_input,
        original_machine,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    if plan.crash != original_head.crash
        || plan.commitment != original_head.commitment
        || authored != original_authored
    {
        return Err(rejected(
            "derived family crash routes differ from their exact checked custody",
        ));
    }
    for member in members {
        let checked = machine_contract_plan(compilation.program, member)?;
        let original = machine_contract_plan(compilation.original_program, member)?;
        let member_authored =
            derive_authored_machine_crash_buckets(compilation.checking_input, member)
                .map_err(|diagnostic| vec![diagnostic])?;
        if member_authored != checked.crash.published()
            || checked.crash != original.crash
            || checked.commitment != original.commitment
        {
            return Err(rejected(
                "family member authored crash routes differ from their exact checked custody",
            ));
        }
    }
    Ok(())
}

fn machine_contract_plan<'a>(
    program: &'a checked_trees::CheckedTrees,
    machine: &Machine,
) -> Result<&'a checked_trees::MachineContractPlan, Vec<Diagnostic>> {
    exactly_one(
        program
            .facts
            .contract_plans
            .machines()
            .iter()
            .filter(|plan| plan.machine == machine.symbol),
        machine.name.as_str(),
        "machine contract plan",
    )
}

pub(crate) fn crash_routes(
    compilation: &SourceDeclarationInput<'_>,
    contracts: &[typed_trees::signature::SignatureContract],
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    buckets: &[checked_trees::CrashRouteBucket],
) -> Result<Vec<PackagePolicyCrashRoute>, Vec<Diagnostic>> {
    let mut published = Vec::new();
    for bucket in buckets {
        let cause = match bucket.cause() {
            checked_trees::CrashCause::Trap => CrashCause::Trap,
            checked_trees::CrashCause::Abort => CrashCause::Abort,
        };
        let mut guards = Vec::new();
        if bucket.alternative_guards() == [checked_trees::CrashRouteGuard::Truth] {
            guards.push(PackagePolicyCrashGuard::Truth);
        } else {
            for contract in contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Crashes { cause })
            {
                for fact_handle in contract.facts.handles() {
                    let Some(expression) = compilation.proof_facts.get(fact_handle).as_expression()
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
                            None,
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
