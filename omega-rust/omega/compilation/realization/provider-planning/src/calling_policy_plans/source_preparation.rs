// We capture original review correspondence (`CallingSourcePreparation`) once
// beside shared Terminal code. Canonical requests drive target work while this
// companion supplies their original diagnostics and review subjects after
// Source has expired, keeping fallible requests, reviews, and target helpers
// inert until consumed.

use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use checked_trees::typed_trees::service_declarations::signatures::PreparedBoundarySignatures;
use checked_trees::typed_trees::service_declarations::signatures::calling_applications::PreparedCallingInstance;
use diagnostics::Diagnostic;
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use semantic_vocabulary::{CallingAbiIdentity, CallingSignatureId};
use std::sync::{Arc, Weak};

#[derive(Debug, Clone, Default)]
pub struct CallingSourcePreparation {
    pub(super) owner: Weak<checked_trees::CheckedTrees>,
    pub(super) requests: Vec<CallingSourceRequest>,
    pub(super) helpers: Vec<(
        target::TargetProfile,
        Result<provider_selection::PreparedTargetChoices, Vec<Diagnostic>>,
    )>,
}

impl PartialEq for CallingSourcePreparation {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.owner, &other.owner)
            && self.requests == other.requests
            && self.helpers == other.helpers
    }
}
impl Eq for CallingSourcePreparation {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CallingSourceRequest {
    pub boundary: symbols::SymbolHandle,
    pub identity: Result<String, String>,
    pub span: source::SourceSpan,
    pub requester: DeclarationCustody,
    pub instances: Result<Vec<CallingSourceInstance>, Vec<Diagnostic>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CallingSourceInstance {
    pub arguments: Vec<checked_trees::typed_trees::types::TypeReferenceHandle>,
    pub argument_owner: symbols::SymbolHandle,
    pub policy_name: Result<String, Vec<Diagnostic>>,
    pub signatures: Result<Vec<CallingSourceSignature>, Vec<Diagnostic>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CallingSourceSignature {
    pub signature: CallingSignatureId,
    pub abi: Result<CallingAbiIdentity, String>,
    pub requirement: symbols::SymbolHandle,
    pub review: Result<super::signature_review::PreparedCallingReview, String>,
}

impl CallingSourcePreparation {
    pub fn capture(
        checked: &Arc<checked_trees::CheckedTrees>,
        services: ServiceCatalog<'_>,
        opaque_reviews: Arc<[super::PreparedOpaqueReviewSelection]>,
        providers: super::CallingPolicyProviderInputs<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if !std::ptr::eq(&*checked.typed, services.context().typed)
            || services.context().scope != source::DependencyScope::Product
        {
            return Err(vec![Diagnostic::error(
                "calling plans require the exact Product source owner's prepared signature catalog",
            )]);
        }
        let typed = &checked.typed;
        let signatures = typed.boundary_signatures();
        if !std::ptr::eq(signatures.as_ref(), services.signatures()) {
            return Err(vec![Diagnostic::error(
                "calling admission has a different frozen declaration census",
            )]);
        }
        let calling = signatures.calling();
        let mut requests = Vec::new();
        let mut has_pending = false;
        for request in calling.requests() {
            let instances = match calling.instances(request) {
                Err(refusals) => Err(refusals),
                Ok(instances) => {
                    let mut captured = Vec::with_capacity(instances.len());
                    for instance in instances {
                        let policy_name = match &instance.policy {
                            Err(reason) => Err(vec![
                                Diagnostic::error(format!(
                                    "boundary trait `{}` cannot evaluate Calling<Policy>: {reason}",
                                    request.name,
                                ))
                                .with_source_span(request.span),
                            ]),
                            Ok(policy) => typed
                                .machine(*policy)
                                .map(|machine| machine.name.as_str().to_owned())
                                .ok_or_else(|| {
                                    vec![
                                        Diagnostic::error(
                                            "prepared calling policy lost its exact machine",
                                        )
                                        .with_source_span(request.span),
                                    ]
                                }),
                        };
                        let rows =
                            capture_signatures(signatures, instance, request.span, &opaque_reviews);
                        has_pending |=
                            policy_name.is_ok() && rows.as_ref().is_ok_and(|rows| !rows.is_empty());
                        captured.push(CallingSourceInstance {
                            arguments: calling.arguments(instance).to_vec(),
                            argument_owner: instance.argument_owner,
                            policy_name,
                            signatures: rows,
                        });
                    }
                    Ok(captured)
                }
            };
            requests.push(CallingSourceRequest {
                boundary: request.boundary,
                identity: typed.normalized_hermetic_symbol_identity(request.boundary),
                span: request.span,
                requester: match typed.symbols.source_file(request.span) {
                    Some(file) if file.origin == source::SourceOrigin::Toolchain => {
                        DeclarationCustody::Toolchain
                    }
                    Some(file) if file.origin == source::SourceOrigin::ToolchainContract => {
                        DeclarationCustody::ToolchainContract
                    }
                    Some(file) => file
                        .package_identity
                        .map(DeclarationCustody::Package)
                        .unwrap_or(DeclarationCustody::UnownedUser),
                    None => DeclarationCustody::Missing,
                },
                instances,
            });
        }
        let helpers = if has_pending {
            super::selected_providers::prepare(checked, services, &providers)
        } else {
            Vec::new()
        };
        Ok(Self {
            owner: Arc::downgrade(checked),
            requests,
            helpers,
        })
    }
}

fn capture_signatures(
    signatures: &PreparedBoundarySignatures,
    instance: &PreparedCallingInstance,
    span: source::SourceSpan,
    opaque_reviews: &Arc<[super::PreparedOpaqueReviewSelection]>,
) -> Result<Vec<CallingSourceSignature>, Vec<Diagnostic>> {
    let mut captured = Vec::new();
    for application in signatures.calling().signatures(instance) {
        if let Some(reason) = &application.abi_refusal {
            return Err(vec![Diagnostic::error(reason).with_source_span(span)]);
        }
        let declaration = signatures.applied_declaration(application).ok_or_else(|| {
            vec![Diagnostic::error(
                "calling admission lost its exact prepared signature declaration",
            )]
        })?;
        for &signature in &application.abi_signatures {
            captured.push(CallingSourceSignature {
                signature,
                abi: source_abi(signatures, signature),
                requirement: declaration.symbol,
                review: super::signature_review::PreparedCallingReview::prepare(
                    declaration,
                    Arc::clone(opaque_reviews),
                ),
            });
        }
    }
    Ok(captured)
}

fn source_abi(
    signatures: &PreparedBoundarySignatures,
    signature: CallingSignatureId,
) -> Result<CallingAbiIdentity, String> {
    let mut matching = signatures
        .abi_signatures()
        .iter()
        .filter(|row| row.id == signature);
    let row = matching
        .next()
        .ok_or("calling review lost its Source ABI application")?;
    if matching.next().is_some() {
        return Err("calling review has competing Source ABI applications".into());
    }
    let abi = CallingAbiIdentity {
        requirement_identity: row.requirement_identity.clone(),
        declaring_application: row.declaring_application.clone(),
        operands: row.operands.clone(),
    };
    abi.validate()?;
    if abi.id() != signature {
        return Err("calling review ABI coordinate differs from its complete Source key".into());
    }
    Ok(abi)
}
