use super::PrepareCandidateLockError as Error;
use crate::declarations::BuildDeclarationKind;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::resolution::package_compilation_inputs_for;
use crate::review::CompilerIssuedPackageReviewSet;
use omega_package_evidence::ledger::OrdinaryPackageObligationSchemaIdentity;
use omega_package_source::local::operations::verify_package_source_snapshot;

pub(super) fn obligations(
    reviews: &CompilerIssuedPackageReviewSet,
    sources: &ExactTargetPackageSourceClosure<'_>,
) -> Result<(), Error> {
    for review in reviews.reviews() {
        let package = review.key();
        let identity = package.identity();
        let ledger = review.obligations();
        let results = review.obligation_results();
        let expected_closure = package_compilation_inputs_for(sources.source_closure(), package)
            .map_err(|errors| Error::CompilerInput {
                package: package.clone(),
                errors,
            })?
            .dependency_closure();
        let role = if package == sources.source_closure().graph().root() {
            sources.source_closure().root_role()
        } else {
            BuildDeclarationKind::Package
        };
        if ledger.package() != identity
            || results.package() != identity
            || ledger.target() != sources.target_profile()
            || results.target() != ledger.target()
            || ledger.schema() != OrdinaryPackageObligationSchemaIdentity::current()
            || results.schema() != ledger.schema()
            || ledger.dependency_closure().root() != identity
            || ledger.dependency_closure().root_role() != role
            || results.dependency_closure() != ledger.dependency_closure()
            || ledger.dependency_closure() != &expected_closure
        {
            return Err(Error::ObligationAssociation {
                package: package.clone(),
            });
        }
        // The private compiler-review producer reconstructed and rechecked
        // discharge certificates on its exact final CheckedCompilation. Do not
        // run that compiler projection again or interpret an acceptance choice
        // as a proof. Disclosed OpenRootAdmission rows are not OpenLaterDischarge.
        if !results.open_contract_entailment_obligations().is_empty() {
            return Err(Error::OpenContractEntailment {
                package: package.clone(),
            });
        }
        let generated = review.generated_source_bundle();
        if generated.package() != identity
            || generated.target() != ledger.target()
            || generated.dependency_closure() != ledger.dependency_closure()
            || generated.source_consumption_commitment() != review.source_consumption_commitment()
        {
            return Err(Error::GeneratedSourceAssociation {
                package: package.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn source_custody(sources: &ExactTargetPackageSourceClosure<'_>) -> Result<(), Error> {
    for custody in sources.source_closure().custodies() {
        verify_package_source_snapshot(
            custody.snapshot_root(),
            custody.materialization().content(),
            custody.source_limits(),
        )
        .map_err(|error| Error::SourceSnapshot {
            package: custody.key().clone(),
            error,
        })?;
        custody
            .selection_evidence()
            .revalidate()
            .map_err(|error| Error::SourceSelection {
                package: custody.key().clone(),
                error,
            })?;
    }
    Ok(())
}
