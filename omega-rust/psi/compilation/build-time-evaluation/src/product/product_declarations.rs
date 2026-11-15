//! Root service audits come from the Build events that actually ran. Each
//! event names an original Build occurrence and an exact visible boundary
//! declaration. The portable declaration anchor lets review rejoin it in
//! the accepted Product source.

use super::assumed_bindings::AssumedBindingContract;
use super::test_groups::TestGroupRegistration;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest, ProductOperandRole,
};
use language_semantics::wire::WireCompatibilityDemand;
use std::sync::Arc;
use symbols::{RetainedDeclaration, SymbolHandle};
use typed_trees::TypedTrees;
use typed_trees::product_operand::{ProductDeclarationLookup, ProductRequestAuthors};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAuditRegistration {
    pub declaration: RetainedDeclaration,
}

pub struct AdmittedProductDeclarations {
    pub service_audits: Vec<ServiceAuditRegistration>,
    pub assumed_bindings: Vec<AssumedBindingContract>,
    pub wire_demands: Vec<WireCompatibilityDemand>,
    pub test_groups: Vec<TestGroupRegistration>,
}

pub fn admit(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    audit_formations: &[(Arc<ProductDeclarationRequest>, SymbolHandle)],
) -> Result<AdmittedProductDeclarations, Vec<Diagnostic>> {
    if super::build_requests_any(
        declared,
        executed,
        &[
            BuildOperation::AssumedBindingContract,
            BuildOperation::WireCompatibilityRequest,
            BuildOperation::TestGroupRegistration,
            BuildOperation::TestGroupService,
        ],
    ) {
        return Err(vec![Diagnostic::error(
            "no product route yet: Build product declaration admission",
        )]);
    }
    let authored_audits = declared
        .iter()
        .filter(|request| request.operation == BuildOperation::ServiceAudit)
        .count();
    if authored_audits != audit_formations.len() {
        return Err(vec![Diagnostic::error(
            "service audit lost its original Build formations",
        )]);
    }
    let service_audits = admit_service_audits(typed, activation, executed, audit_formations)?;
    Ok(AdmittedProductDeclarations {
        service_audits,
        assumed_bindings: Vec::new(),
        wire_demands: Vec::new(),
        test_groups: Vec::new(),
    })
}

fn admit_service_audits(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    executed: &[ExecutedProductSelection],
    authored: &[(Arc<ProductDeclarationRequest>, SymbolHandle)],
) -> Result<Vec<ServiceAuditRegistration>, Vec<Diagnostic>> {
    if authored.is_empty()
        && !executed
            .iter()
            .any(|event| event.request.operation == BuildOperation::ServiceAudit)
    {
        return Ok(Vec::new());
    }
    let declarations = ProductDeclarationLookup::new(typed, &[ProductOperandRole::BoundarySubject]);
    let authors = ProductRequestAuthors::new(typed);
    let mut formations = Vec::with_capacity(authored.len());
    for (request, author) in authored {
        let service = audit_service(typed, &declarations, request)?;
        if !typed
            .symbols
            .symbol_is_in_dependency_scope(*author, source::DependencyScope::Build)
        {
            return Err(refuse(request, "service audit author is not Build source"));
        }
        formations.push((occurrence(typed, request), *author, request, service));
    }
    formations.sort_unstable_by_key(|&(key, _, _, _)| key);

    let mut services = Vec::new();
    for event in executed
        .iter()
        .filter(|event| event.request.operation == BuildOperation::ServiceAudit)
    {
        let request = &event.request;
        if activation.is_none_or(|machine| {
            typed.machine_by_symbol(machine).is_none()
                || !typed
                    .symbols
                    .symbol_is_in_dependency_scope(machine, source::DependencyScope::Build)
        }) || event.target.is_some()
            || event.composition.is_some()
        {
            return Err(refuse(
                request,
                "service audit has no exact root Build activation",
            ));
        }
        let author =
            authors
                .resolve(event.selecting_machine.ok_or_else(|| {
                    refuse(request, "service audit lost its original Build author")
                })?)
                .map_err(|reason| refuse(request, reason))?;
        let key = occurrence(typed, request);
        let start = formations.partition_point(|formation| formation.0 < key);
        let (_, _, _, service) = formations[start..]
            .iter()
            .take_while(|formation| formation.0 == key)
            .find(|(_, owner, declared, _)| {
                *owner == author && typed.same_product_request_source(declared, request)
            })
            .ok_or_else(|| {
                refuse(
                    request,
                    "executed service audit has no matching source occurrence",
                )
            })?;
        services.push(*service);
    }
    services.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    services.dedup();
    services
        .into_iter()
        .map(|symbol| {
            Ok(ServiceAuditRegistration {
                declaration: RetainedDeclaration::capture(&typed.symbols, symbol)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
            })
        })
        .collect()
}

fn audit_service(
    typed: &TypedTrees,
    declarations: &ProductDeclarationLookup<'_>,
    request: &ProductDeclarationRequest,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    request
        .validate_shape()
        .map_err(|reason| refuse(request, reason))?;
    let [operand] = request.operands.as_ref() else {
        return Err(refuse(
            request,
            "service audit has no single declaration operand",
        ));
    };
    let service = if operand.application.is_none() {
        declarations
            .resolve(
                &operand.path,
                operand.source_span,
                ProductOperandRole::BoundarySubject,
            )
            .filter(|symbol| {
                typed
                    .trait_definition_by_symbol(*symbol)
                    .is_some_and(|declaration| {
                        declaration.is_boundary
                            && declaration.lifetime_parameters.is_empty()
                            && typed.trait_type_parameters(declaration).is_empty()
                    })
            })
    } else {
        None
    };
    service.ok_or_else(|| {
        refuse(
            request,
            format!(
                "service audit operand `{}` must name one visible boundary trait declaration",
                operand.path
            ),
        )
    })
}

fn occurrence(typed: &TypedTrees, request: &ProductDeclarationRequest) -> (usize, usize, usize) {
    let span = typed.symbols.rejoin_product_occurrence(request.source_span);
    (span.source_id.0, span.span.start, span.span.end)
}

fn refuse(request: &ProductDeclarationRequest, reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason).with_source_span(request.source_span)]
}
