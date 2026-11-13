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

pub(crate) fn crash(
    _compilation: &PackageReviewInput<'_>,
    _machine: &Machine,
    _entry: &State,
    _binders: &[(SymbolHandle, String)],
    _lifetime_binders: &[typed_trees::name::Identifier],
    _envelope: &RealizedMachineContractEnvelope,
    _inferred_causes: &[(SymbolHandle, Vec<checked_trees::CrashCause>)],
) -> Result<PackagePolicyCrash, Vec<Diagnostic>> {
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: package review requires lowered-form evidence",
    )])
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
