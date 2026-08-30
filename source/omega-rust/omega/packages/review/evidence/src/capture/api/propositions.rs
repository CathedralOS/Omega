use crate::capture::contracts::expressions::projection::project_contract_expression;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::contracts::propositions::application::project_contract_proposition;
use crate::capture::contracts::propositions::endpoint::project_proposition_signature;
use crate::capture::contracts::propositions::evidence::project_evidence_interface;
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::capture::source::{ProjectedNestedSourceLocation, ProjectedReviewRow};
use crate::record::{
    PackageReviewContractFact, PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
    PackageReviewSourceLocationRole,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(crate) fn project_public_propositions(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewPropositionShape>>, Vec<Diagnostic>> {
    use psi_typed_trees::proposition::{PropositionBody, PropositionFormula};

    let mut rows = Vec::new();
    for declaration in compilation
        .propositions()
        .iter()
        .filter(|declaration| declaration.is_public)
    {
        let identity = nominal_identity(compilation, declaration.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let (binders, parameter_types) = project_proposition_signature(compilation, declaration)?;
        let nested_source_locations = match (
            &declaration.body,
            declaration.transparent_formula_source_span,
        ) {
            (PropositionBody::Transparent { .. }, Some(source_span)) => {
                vec![ProjectedNestedSourceLocation {
                    source_span,
                    role: PackageReviewSourceLocationRole::PropositionFormula,
                }]
            }
            (PropositionBody::Primitive | PropositionBody::Witness { .. }, None) => Vec::new(),
            (PropositionBody::Transparent { .. }, None) => {
                return Err(vec![Diagnostic::error(format!(
                    "public transparent proposition `{}` has no exact formula source custody",
                    identity.path
                ))]);
            }
            (PropositionBody::Primitive | PropositionBody::Witness { .. }, Some(_)) => {
                return Err(vec![Diagnostic::error(format!(
                    "public non-transparent proposition `{}` retains contradictory formula source custody",
                    identity.path
                ))]);
            }
        };
        let body = match &declaration.body {
            PropositionBody::Primitive | PropositionBody::Witness { .. } => {
                let matching = compilation
                    .facts
                    .proof
                    .proposition_vocabulary
                    .declarations
                    .iter()
                    .filter(|checked| checked.symbol == declaration.symbol)
                    .collect::<Vec<_>>();
                let [checked] = matching.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "public proposition `{}` has {} checked declaration rows; expected one",
                        identity.path,
                        matching.len()
                    ))]);
                };
                if !checked.is_public {
                    return Err(vec![Diagnostic::error(format!(
                        "public proposition `{}` lost visibility during checked lowering",
                        identity.path
                    ))]);
                }
                match declaration.body {
                    PropositionBody::Primitive => PackageReviewPublicPropositionBody::Primitive,
                    PropositionBody::Witness { evidence } => {
                        let declaration_binders = compilation.proposition_binders(declaration);
                        let binder_symbols = declaration_binders
                            .iter()
                            .enumerate()
                            .map(|(position, binder)| {
                                (binder.symbol, format!("proposition-binder:{position}"))
                            })
                            .collect::<Vec<_>>();
                        PackageReviewPublicPropositionBody::Witness(project_evidence_interface(
                            compilation,
                            evidence,
                            &binder_symbols,
                        )?)
                    }
                    PropositionBody::Transparent { .. } => unreachable!(),
                }
            }
            PropositionBody::Transparent { proposition } => {
                let parameters = compilation.proposition_parameters(declaration);
                let declaration_binders = compilation.proposition_binders(declaration);
                let binder_symbols = declaration_binders
                    .iter()
                    .enumerate()
                    .map(|(position, binder)| {
                        (binder.symbol, format!("proposition-binder:{position}"))
                    })
                    .collect::<Vec<_>>();
                let context = ContractProjectionContext {
                    subject_kind: "public proposition",
                    subject_name: &identity.path,
                    owner: psi_checked_trees::ContractProofFactOwner::Unknown,
                    point: psi_facts::ProgramPoint::Definition {
                        symbol: declaration.symbol,
                    },
                    parameters,
                    domain_symbol: None,
                    data_symbol: None,
                    lifetime_binders: &[],
                };
                let mut visiting = vec![declaration.symbol];
                let expansion = match proposition {
                    PropositionFormula::Application(application) => project_contract_proposition(
                        compilation,
                        &context,
                        &binder_symbols,
                        application,
                        None,
                        &[],
                        &[],
                        &mut visiting,
                        0,
                    )?,
                    PropositionFormula::BooleanExpression(expression) => {
                        PackageReviewContractFact::Expression(project_contract_expression(
                            compilation,
                            &context,
                            &binder_symbols,
                            *expression,
                            None,
                            0,
                        )?)
                    }
                };
                PackageReviewPublicPropositionBody::Transparent(expansion)
            }
        };
        rows.push(ProjectedReviewRow {
            row: PackageReviewPropositionShape {
                identity,
                binders,
                parameter_types,
                body,
            },
            declaration: declaration.symbol,
            nested_source_locations,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}
