//! Root service audits come from the Build events that actually ran. Each
//! event names an original Build occurrence and an exact visible boundary
//! declaration. Admission checks that each executed audit can be retained
//! as a declaration in the accepted Product source.

use crate::declaration_selection::BuildOperation;
use crate::declaration_selection::ExecutedProductSelection;
use crate::declaration_selection::ProductDeclarationRequest;
use crate::declaration_selection::ProductOperandRole;
use crate::source_state::TypedTrees;
use crate::source_state::product_operand::{ProductDeclarationLookup, ProductRequestAuthors};
use diagnostics::Diagnostic;
use source_declarations::source_selections::{AssumedBindingContract, TestGroupRegistration};
use std::sync::Arc;
use symbols::SymbolHandle;
use crate::WireCompatibilityDemand;

pub struct AdmittedProductDeclarations {
    pub service_audits: Vec<SymbolHandle>,
    pub assumed_bindings: Vec<AssumedBindingContract>,
    pub wire_demands: Vec<WireCompatibilityDemand>,
    pub test_groups: Vec<TestGroupRegistration>,
}

/// `formations` pairs every authored service-audit and test-group request with
/// the Build machine that wrote it, so each executed event can be rejoined to
/// its exact source occurrence and author.
pub fn admit(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    formations: &[(Arc<ProductDeclarationRequest>, SymbolHandle)],
) -> Result<AdmittedProductDeclarations, Vec<Diagnostic>> {
    if build_requests_any(
        declared,
        executed,
        &[
            BuildOperation::AssumedBindingContract,
            BuildOperation::WireCompatibilityRequest,
        ],
    ) {
        return Err(vec![Diagnostic::error(
            "no product route yet: Build product declaration admission",
        )]);
    }
    let authored = declared
        .iter()
        .filter(|request| admitted_here(request.operation))
        .count();
    if authored != formations.len()
        || formations
            .iter()
            .any(|(request, _)| !admitted_here(request.operation))
    {
        return Err(vec![Diagnostic::error(
            "Build declaration lost its original Build formations",
        )]);
    }
    let formations = AuthoredFormations::new(typed, formations)?;
    let service_audits = admit_service_audits(typed, activation, executed, &formations)?;
    let test_groups = admit_test_groups(typed, activation, executed, &formations)?;
    Ok(AdmittedProductDeclarations {
        service_audits,
        assumed_bindings: Vec::new(),
        wire_demands: Vec::new(),
        test_groups,
    })
}

fn admitted_here(operation: BuildOperation) -> bool {
    matches!(
        operation,
        BuildOperation::ServiceAudit | BuildOperation::TestGroupRegistration
    )
}

/// The authored requests sorted by source occurrence, each checked to be
/// written by Build source.
struct AuthoredFormations<'request> {
    rows: Vec<AuthoredFormation<'request>>,
}

struct AuthoredFormation<'request> {
    occurrence: (usize, usize, usize),
    author: SymbolHandle,
    request: &'request ProductDeclarationRequest,
}

impl<'request> AuthoredFormations<'request> {
    fn new(
        typed: &TypedTrees,
        formations: &'request [(Arc<ProductDeclarationRequest>, SymbolHandle)],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut rows = Vec::with_capacity(formations.len());
        for (request, author) in formations {
            request
                .validate_shape()
                .map_err(|reason| refuse(request, reason))?;
            if !typed
                .symbols
                .symbol_is_in_dependency_scope(*author, source_provenance::DependencyScope::Build)
            {
                return Err(refuse(
                    request,
                    "Build declaration author is not Build source",
                ));
            }
            rows.push(AuthoredFormation {
                occurrence: occurrence(typed, request),
                author: *author,
                request,
            });
        }
        rows.sort_unstable_by_key(|row| row.occurrence);
        Ok(Self { rows })
    }

    fn of(&self, operation: BuildOperation) -> impl Iterator<Item = &AuthoredFormation<'request>> {
        self.rows
            .iter()
            .filter(move |row| row.request.operation == operation)
    }

    /// The authored formation an executed event ran, refused unless the event
    /// came from the exact root Build activation and rejoins one source
    /// occurrence written by the same Build machine.
    fn executed(
        &self,
        typed: &TypedTrees,
        authors: &ProductRequestAuthors<'_>,
        activation: Option<SymbolHandle>,
        event: &ExecutedProductSelection,
        what: &str,
    ) -> Result<&AuthoredFormation<'request>, Vec<Diagnostic>> {
        let request = &event.request;
        if activation.is_none_or(|machine| {
            typed.machine_by_symbol(machine).is_none()
                || !typed.symbols.symbol_is_in_dependency_scope(
                    machine,
                    source_provenance::DependencyScope::Build,
                )
        }) || event.target.is_some()
            || event.composition.is_some()
        {
            return Err(refuse(
                request,
                format!("{what} has no exact root Build activation"),
            ));
        }
        let author =
            authors
                .resolve(event.selecting_machine.ok_or_else(|| {
                    refuse(request, format!("{what} lost its original Build author"))
                })?)
                .map_err(|reason| refuse(request, reason))?;
        let key = occurrence(typed, request);
        let start = self.rows.partition_point(|row| row.occurrence < key);
        self.rows[start..]
            .iter()
            .take_while(|row| row.occurrence == key)
            .find(|row| {
                row.author == author && typed.same_product_request_source(row.request, request)
            })
            .ok_or_else(|| {
                refuse(
                    request,
                    format!("executed {what} has no matching source occurrence"),
                )
            })
    }
}

fn admit_service_audits(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    executed: &[ExecutedProductSelection],
    formations: &AuthoredFormations<'_>,
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let mut audits = executed
        .iter()
        .filter(|event| event.request.operation == BuildOperation::ServiceAudit)
        .peekable();
    if formations.of(BuildOperation::ServiceAudit).next().is_none() && audits.peek().is_none() {
        return Ok(Vec::new());
    }
    let declarations = ProductDeclarationLookup::new(typed, &[ProductOperandRole::BoundarySubject]);
    // Every authored audit names a real boundary service, whether or not the
    // Build reached it.
    for formation in formations.of(BuildOperation::ServiceAudit) {
        audit_service(typed, &declarations, formation.request)?;
    }
    let authors = ProductRequestAuthors::new(typed);
    let mut services = Vec::new();
    for event in audits {
        let formation = formations.executed(typed, &authors, activation, event, "service audit")?;
        services.push(audit_service(typed, &declarations, formation.request)?);
    }
    services.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    services.dedup();
    for &symbol in &services {
        crate::source_symbols::retain_declaration(&typed.symbols, symbol)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
    }
    Ok(services)
}

/// Resolve every authored group, then retain the exact registrations reached by Build.
fn admit_test_groups(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    executed: &[ExecutedProductSelection],
    formations: &AuthoredFormations<'_>,
) -> Result<Vec<TestGroupRegistration>, Vec<Diagnostic>> {
    let operation = BuildOperation::TestGroupRegistration;
    let mut events = executed
        .iter()
        .filter(|event| event.request.operation == operation)
        .peekable();
    if formations.of(operation).next().is_none() && events.peek().is_none() {
        return Ok(Vec::new());
    }
    let declarations = ProductDeclarationLookup::new(typed, &[ProductOperandRole::TestRequirement]);
    for formation in formations.of(operation) {
        test_group_requirement(typed, &declarations, formation.request)?;
    }
    let authors = ProductRequestAuthors::new(typed);
    let mut groups = Vec::<TestGroupRegistration>::new();
    for (ordinal, event) in events.enumerate() {
        let formation = formations.executed(typed, &authors, activation, event, "test group")?;
        let request = formation.request;
        let requirement = test_group_requirement(typed, &declarations, request)?;
        let operand = &event.request.operands[0];
        let closed_requirement = if operand.application.is_some() {
            let application = typed
                .prepared_product_operand(
                    operand,
                    event.static_environment.as_deref(),
                    formation.author,
                    ProductOperandRole::TestRequirement,
                )
                .ok_or_else(|| refuse(request, "test group lost its prepared trait application"))?;
            let closed = typed
                .product_requirement_application(application)
                .map_err(|why| refuse(request, why))?;
            if closed.template != requirement {
                return Err(refuse(
                    request,
                    "test group application belongs to another requirement",
                ));
            }
            Some(closed.signature.symbol)
        } else {
            None
        };
        if groups.iter().any(|group| {
            group.requirement == requirement && group.closed_requirement == closed_requirement
        }) {
            return Err(refuse(
                request,
                "test group registration repeats the same requirement",
            ));
        }
        groups.push(TestGroupRegistration {
            configuration: event.test_group.clone().ok_or_else(|| {
                refuse(request, "test group registration lost its evaluated configuration")
            })?,
            requirement,
            closed_requirement,
            selecting_machine: formation.author,
            source_span: typed.symbols.rejoin_product_occurrence(request.source_span),
            origin: super::declaration_lookup::capture_build_selection_origin(
                &typed.symbols,
                activation
                    .ok_or_else(|| refuse(request, "test group lost its Build activation"))?,
                formation.author,
                typed.symbols.rejoin_product_occurrence(request.source_span),
                Some(
                    u64::try_from(ordinal)
                        .map_err(|_| refuse(request, "test group event ordinal exceeds u64"))?,
                ),
            )
            .map_err(|why| refuse(request, why))?,
        });
    }
    Ok(groups)
}

fn test_group_requirement(
    typed: &TypedTrees,
    declarations: &ProductDeclarationLookup<'_>,
    request: &ProductDeclarationRequest,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    let [operand] = request.operands.as_ref() else {
        return Err(refuse(
            request,
            "test group registration requires one requirement operand",
        ));
    };
    let Some(requirement) = declarations.resolve(
        &operand.path,
        operand.source_span,
        ProductOperandRole::TestRequirement,
    ) else {
        return Err(refuse(
            request,
            format!(
                "test group operand `{}` must name one visible requirement of a non-boundary trait",
                operand.path
            ),
        ));
    };
    if operand.application.is_none()
        && typed
            .trait_machine_signature(requirement)
            .is_some_and(|(owner, signature)| {
                !owner.type_parameters.is_empty()
                    || !owner.lifetime_parameters.is_empty()
                    || !signature.type_parameters.is_empty()
                    || !signature.lifetime_parameters.is_empty()
            })
    {
        return Err(refuse(
            request,
            "a test group must name its complete concrete trait requirement application",
        ));
    }
    Ok(requirement)
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

fn build_requests_any(
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    operations: &[BuildOperation],
) -> bool {
    declared
        .iter()
        .any(|request| operations.contains(&request.operation))
        || executed
            .iter()
            .any(|event| operations.contains(&event.request.operation))
}
