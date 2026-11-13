//! Crash projections requiring old judgments refuse; explicit route projection remains.

use super::rejected;
use crate::capture::contracts::{
    expressions::projection::project_contract_expression, facts::ContractProjectionContext,
};
use crate::capture::{PackageReviewInput, SourceDeclarationInput};
use crate::record::{PackagePolicyCrash, PackagePolicyCrashGuard, PackagePolicyCrashRoute};
use checked_trees::RealizedMachineContractEnvelope;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
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
        "no product route yet: package-review callable crash evidence",
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
