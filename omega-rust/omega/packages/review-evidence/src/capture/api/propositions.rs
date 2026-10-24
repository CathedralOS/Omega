use crate::capture::PackageReviewInput;
use crate::capture::contracts::expressions::projection::project_contract_expression;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::contracts::propositions::application::project_contract_proposition;
use crate::capture::contracts::propositions::endpoint::project_proposition_signature;
use crate::capture::contracts::propositions::evidence::project_evidence_interface;
use crate::capture::semantics::declarations::{
    is_product_scope_instance, nominal_identity, reviewed_package_owns,
};
use crate::capture::source::ProjectedReviewRow;
use crate::record::{
    PackageReviewContractFact, PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use typed_trees::proposition::PropositionDefinition;

pub(crate) fn project_public_propositions<'a>(
    compilation: &'a PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<
    Vec<ProjectedReviewRow<'a, PackageReviewPropositionShape, PropositionDefinition>>,
    Vec<Diagnostic>,
> {
    use typed_trees::proposition::{PropositionBody, PropositionFormula};

    let mut rows = Vec::new();
    for declaration in compilation.propositions().iter().filter(|declaration| {
        declaration.is_public
            && is_product_scope_instance(&compilation.typed.symbols, declaration.symbol)
    }) {
        let identity = nominal_identity(compilation, declaration.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let (binders, parameter_types) = project_proposition_signature(compilation, declaration)?;
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
                    owner: checked_trees::ContractProofFactOwner::Unknown,
                    point: facts::ProgramPoint::Definition {
                        symbol: declaration.symbol,
                    },
                    parameters,
                    domain_symbol: None,
                    data_symbol: None,
                    lifetime_binders: &[],
                    lifetime_substitutions: &[],
                    selection_exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
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
            declaration,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}
