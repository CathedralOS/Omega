//! Project source associations once into a borrowed, source-free policy input.
//! Preparation selects no provider and rejects nothing. Missing body or numeric
//! evidence is observed only if the later policy needs that exact evidence.

use checked_trees::{
    CheckFacts, CheckedBoundaryOperatorApplicationUseSite, CheckedProviderRequirementKind,
};
use language_semantics::CheckedBodyCandidate;
use language_semantics::boundary_provider_review::{
    BoundaryApplicationEvidenceError, BoundaryOperatorApplication, BoundaryOperatorDeclaration,
    BoundaryOperatorUse, BoundaryProviderReview, BoundaryRequirementReview, BoundaryRequirementUse,
};

pub(super) fn prepare<'facts>(
    facts: &'facts CheckFacts,
    external: &'facts super::NumericRealizationDeclarations,
) -> BoundaryProviderReview<'facts> {
    // Keep source correspondence local. Occurrences copy the small borrowed
    // coordinate, not declaration text, a second body catalog, or source keys.
    let requirements = facts
        .operators
        .provider_requirements
        .iter()
        .map(|(_, requirement)| {
            (
                requirement,
                BoundaryRequirementReview {
                    schema_identity: &requirement.schema_identity,
                    package_identity: requirement.package_identity,
                    numeric_requirement: numeric_requirement(external, requirement.symbol),
                },
            )
        })
        .collect::<Vec<_>>();
    let declarations = requirements
        .iter()
        .filter_map(|(source, requirement)| {
            if !matches!(
                source.kind,
                CheckedProviderRequirementKind::Operator {
                    is_product_boundary: true
                }
            ) {
                return None;
            }
            let bodies = facts
                .operators
                .boundary_declarations
                .iter()
                .filter(|(_, declaration)| declaration.requirement == source.symbol)
                .map(|(_, declaration)| CheckedBodyCandidate {
                    body: (),
                    declaration: &declaration.selection,
                    target: declaration.selection.target.as_deref(),
                    is_product: declaration.selection.is_product,
                })
                .collect();
            Some(BoundaryOperatorDeclaration {
                requirement: *requirement,
                bodies,
            })
        })
        .collect();

    let spelled = facts.operators.uses.iter().map(|(_, usage)| {
        (
            usage.selected_operator_symbol,
            usage.application_site(),
            usage.is_generic_template,
        )
    });
    let named = facts.operators.named_uses.iter().map(|(_, usage)| {
        (
            usage.selected_operator_symbol,
            CheckedBoundaryOperatorApplicationUseSite::Expression {
                expression: usage.expression,
                origin: usage.origin,
            },
            usage.is_generic_template,
        )
    });
    let operator_uses = spelled
        .chain(named)
        .filter_map(|(symbol, site, template)| {
            let (_, requirement) = requirements.iter().find(|(source, _)| {
                source.symbol == symbol
                    && matches!(source.kind, CheckedProviderRequirementKind::Operator { .. })
            })?;
            let application = if template {
                BoundaryOperatorApplication::GenericTemplate
            } else {
                BoundaryOperatorApplication::Closed(application_bodies(facts, symbol, site))
            };
            Some(BoundaryOperatorUse {
                requirement: *requirement,
                application,
            })
        })
        .collect();

    let requirement_uses = facts
        .operators
        .named_requirement_uses
        .iter()
        .filter_map(|(_, usage)| {
            let (source, requirement) = requirements.iter().find(|(source, _)| {
                source.symbol == usage.requirement_symbol
                    && matches!(
                        source.kind,
                        CheckedProviderRequirementKind::DirectCall { .. }
                    )
            })?;
            let CheckedProviderRequirementKind::DirectCall { diagnostic_name } = &source.kind
            else {
                return None;
            };
            Some(BoundaryRequirementUse {
                requirement: *requirement,
                diagnostic_name,
            })
        })
        .collect();
    BoundaryProviderReview {
        declarations,
        operator_uses,
        requirement_uses,
    }
}

fn numeric_requirement(
    external: &super::NumericRealizationDeclarations,
    symbol: symbols::SymbolHandle,
) -> Result<&str, &'static str> {
    let mut declarations = external
        .correspondence()
        .iter()
        .filter(|(source, _)| *source == symbol);
    let (_, declaration) = declarations
        .next()
        .ok_or("numeric requirement has no checked nominal declaration")?;
    if declarations.next().is_some() {
        return Err("numeric requirement has competing nominal declarations");
    }
    external
        .identities
        .text(declaration.identity())
        .ok_or("numeric requirement identity is absent from its checked catalog")
}

fn application_bodies(
    facts: &CheckFacts,
    symbol: symbols::SymbolHandle,
    site: CheckedBoundaryOperatorApplicationUseSite,
) -> Result<Vec<CheckedBodyCandidate<'_, ()>>, BoundaryApplicationEvidenceError> {
    let operators = &facts.operators;
    let mut applications = operators
        .boundary_applications
        .iter()
        .filter(|application| application.requirement_symbol == symbol && application.site == site);
    let first = applications.next();
    let count = usize::from(first.is_some()) + applications.count();
    if count != 1 {
        return Err(BoundaryApplicationEvidenceError::ApplicationCount(count));
    }
    let Some(application) = first else {
        return Err(BoundaryApplicationEvidenceError::ApplicationCount(0));
    };
    operators.project_body_candidates(application, |_| ())
}
