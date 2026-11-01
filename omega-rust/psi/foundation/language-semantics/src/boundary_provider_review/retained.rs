//! Source-independent declaration evidence retained at Psi preparation.

use super::{
    BoundaryApplicationEvidenceError, BoundaryOperatorApplication, BoundaryOperatorDeclaration,
    BoundaryOperatorUse, BoundaryProviderReview, BoundaryRequirementReview, BoundaryRequirementUse,
};
use crate::{BoundaryBodySelection, CheckedBodyCandidate};
use semantic_vocabulary::PackageKeyIdentity;

/// Complete ordered policy inputs. Retaining these records neither selects a
/// provider nor admits an execution; missing evidence stays deferred data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedBoundaryProviderReview {
    declarations: Vec<OperatorDeclaration>,
    operator_uses: Vec<OperatorUse>,
    requirement_uses: Vec<RequirementUse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Requirement {
    schema_identity: String,
    package_identity: Option<PackageKeyIdentity>,
    numeric_requirement: Result<String, &'static str>,
}

impl Requirement {
    fn retain(value: BoundaryRequirementReview<'_>) -> Self {
        Self {
            schema_identity: value.schema_identity.to_owned(),
            package_identity: value.package_identity,
            numeric_requirement: value.numeric_requirement.map(str::to_owned),
        }
    }

    fn borrow(&self) -> BoundaryRequirementReview<'_> {
        BoundaryRequirementReview {
            schema_identity: &self.schema_identity,
            package_identity: self.package_identity,
            numeric_requirement: self
                .numeric_requirement
                .as_deref()
                .map_err(|reason| *reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Body {
    declaration: BoundaryBodySelection,
    target: Option<String>,
    is_product: bool,
}

impl Body {
    fn retain(value: CheckedBodyCandidate<'_, ()>) -> Self {
        Self {
            declaration: value.declaration.clone(),
            target: value.target.map(str::to_owned),
            is_product: value.is_product,
        }
    }

    fn borrow(&self) -> CheckedBodyCandidate<'_, ()> {
        CheckedBodyCandidate {
            body: (),
            declaration: &self.declaration,
            target: self.target.as_deref(),
            is_product: self.is_product,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OperatorDeclaration {
    requirement: Requirement,
    bodies: Vec<Body>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OperatorUse {
    requirement: Requirement,
    application: OperatorApplication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OperatorApplication {
    GenericTemplate,
    Closed(Result<Vec<Body>, BoundaryApplicationEvidenceError>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequirementUse {
    requirement: Requirement,
    diagnostic_name: String,
}

impl BoundaryProviderReview<'_> {
    /// Retain every original declaration and occurrence, including duplicate
    /// candidates, generic templates, and unavailable application evidence.
    pub fn retain(self) -> RetainedBoundaryProviderReview {
        RetainedBoundaryProviderReview {
            declarations: self
                .declarations
                .into_iter()
                .map(|value| OperatorDeclaration {
                    requirement: Requirement::retain(value.requirement),
                    bodies: value.bodies.into_iter().map(Body::retain).collect(),
                })
                .collect(),
            operator_uses: self
                .operator_uses
                .into_iter()
                .map(|value| OperatorUse {
                    requirement: Requirement::retain(value.requirement),
                    application: match value.application {
                        BoundaryOperatorApplication::GenericTemplate => {
                            OperatorApplication::GenericTemplate
                        }
                        BoundaryOperatorApplication::Closed(bodies) => OperatorApplication::Closed(
                            bodies.map(|bodies| bodies.into_iter().map(Body::retain).collect()),
                        ),
                    },
                })
                .collect(),
            requirement_uses: self
                .requirement_uses
                .into_iter()
                .map(|value| RequirementUse {
                    requirement: Requirement::retain(value.requirement),
                    diagnostic_name: value.diagnostic_name.to_owned(),
                })
                .collect(),
        }
    }
}

impl RetainedBoundaryProviderReview {
    /// Lend the ordinary policy input without consulting the checked producer.
    /// Ordered occurrences and exact refusal payloads remain unchanged.
    pub fn borrow(&self) -> BoundaryProviderReview<'_> {
        BoundaryProviderReview {
            declarations: self
                .declarations
                .iter()
                .map(|value| BoundaryOperatorDeclaration {
                    requirement: value.requirement.borrow(),
                    bodies: value.bodies.iter().map(Body::borrow).collect(),
                })
                .collect(),
            operator_uses: self
                .operator_uses
                .iter()
                .map(|value| BoundaryOperatorUse {
                    requirement: value.requirement.borrow(),
                    application: match &value.application {
                        OperatorApplication::GenericTemplate => {
                            BoundaryOperatorApplication::GenericTemplate
                        }
                        OperatorApplication::Closed(bodies) => BoundaryOperatorApplication::Closed(
                            bodies
                                .as_ref()
                                .map(|bodies| bodies.iter().map(Body::borrow).collect())
                                .map_err(|reason| *reason),
                        ),
                    },
                })
                .collect(),
            requirement_uses: self
                .requirement_uses
                .iter()
                .map(|value| BoundaryRequirementUse {
                    requirement: value.requirement.borrow(),
                    diagnostic_name: &value.diagnostic_name,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detached_review_preserves_ambiguous_and_unavailable_evidence() {
        let retained = {
            let schema = String::from("package::schema");
            let numeric = String::from("package::Number::add");
            let diagnostic_name = String::from("ordinary requirement");
            let declaration = BoundaryBodySelection {
                requirement: "package::requirement".into(),
                callable: "package::implementation".into(),
                provider: "package::Provider".into(),
                provider_application: Some("Provider<Argument>".into()),
                requirement_lifetime_partition: vec![1, 0, 1],
                target: Some("linux_x86_64".into()),
                is_product: true,
                ..Default::default()
            };
            // Eligibility and the declaration are independent evidence. Do not
            // reconstruct either from the other while crossing ownership.
            let body = CheckedBodyCandidate {
                body: (),
                declaration: &declaration,
                target: Some("windows_x86_64"),
                is_product: false,
            };
            let requirement = BoundaryRequirementReview {
                schema_identity: &schema,
                package_identity: None,
                numeric_requirement: Ok(&numeric),
            };
            BoundaryProviderReview {
                declarations: vec![BoundaryOperatorDeclaration {
                    requirement,
                    bodies: vec![body, body],
                }],
                operator_uses: vec![
                    BoundaryOperatorUse {
                        requirement,
                        application: BoundaryOperatorApplication::Closed(Ok(vec![body, body])),
                    },
                    BoundaryOperatorUse {
                        requirement,
                        application: BoundaryOperatorApplication::Closed(Err(
                            BoundaryApplicationEvidenceError::ApplicationCount(2),
                        )),
                    },
                    BoundaryOperatorUse {
                        requirement: BoundaryRequirementReview {
                            numeric_requirement: Err("numeric declaration unavailable"),
                            ..requirement
                        },
                        application: BoundaryOperatorApplication::GenericTemplate,
                    },
                ],
                requirement_uses: vec![BoundaryRequirementUse {
                    requirement,
                    diagnostic_name: &diagnostic_name,
                }],
            }
            .retain()
        };
        // All source-owned strings and declarations have been dropped.
        let review = retained.borrow();
        assert_eq!(review.declarations.len(), 1);
        assert_eq!(review.declarations[0].bodies.len(), 2);
        let body = &review.declarations[0].bodies[0];
        assert_eq!(
            body.declaration.provider_application.as_deref(),
            Some("Provider<Argument>")
        );
        assert_eq!(body.declaration.requirement_lifetime_partition, [1, 0, 1]);
        assert_eq!(body.declaration.target.as_deref(), Some("linux_x86_64"));
        assert!(body.declaration.is_product);
        assert_eq!(body.target, Some("windows_x86_64"));
        assert!(!body.is_product);
        assert_eq!(review.operator_uses.len(), 3);
        assert!(matches!(&review.operator_uses[0].application,
            BoundaryOperatorApplication::Closed(Ok(bodies)) if bodies.len() == 2));
        assert!(matches!(
            review.operator_uses[1].application,
            BoundaryOperatorApplication::Closed(Err(
                BoundaryApplicationEvidenceError::ApplicationCount(2)
            ))
        ));
        assert!(matches!(
            review.operator_uses[2].application,
            BoundaryOperatorApplication::GenericTemplate
        ));
        assert_eq!(
            review.operator_uses[2].requirement.numeric_requirement,
            Err("numeric declaration unavailable")
        );
        assert_eq!(
            review.requirement_uses[0].diagnostic_name,
            "ordinary requirement"
        );
        assert_eq!(
            review.requirement_uses[0].requirement.schema_identity,
            "package::schema"
        );
        assert_eq!(
            review.requirement_uses[0].requirement.numeric_requirement,
            Ok("package::Number::add")
        );
    }
}
