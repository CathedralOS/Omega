//! Root service audits come from the Build events that actually ran. Each
//! event names an original Build occurrence and an exact visible boundary
//! declaration. The portable declaration anchor lets review rejoin it in
//! the accepted Product source.

use super::assumed_bindings::AssumedBindingContract;
use super::test_groups::{TestGroupRegistration, TestGroupService};
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
    if super::build_requests_any(
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
        BuildOperation::ServiceAudit
            | BuildOperation::TestGroupRegistration
            | BuildOperation::TestGroupService
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
                .symbol_is_in_dependency_scope(*author, source::DependencyScope::Build)
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
                || !typed
                    .symbols
                    .symbol_is_in_dependency_scope(machine, source::DependencyScope::Build)
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
) -> Result<Vec<ServiceAuditRegistration>, Vec<Diagnostic>> {
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

/*
Test groups (wiki/spec/build/testing.md#registration-and-discovery). Each
executed `builder.tests.group<Requirement>()` registers one exact non-boundary
trait requirement once; registering it again rejects. Each executed
`builder.tests.service<Requirement, Slot>()` configures a group the same Build
already registered, naming one exact boundary trait at most once per group.
Groups keep their execution order, which is the roster order. Which slots the
runner can back is decided by the runner, not here.
*/
fn admit_test_groups(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    executed: &[ExecutedProductSelection],
    formations: &AuthoredFormations<'_>,
) -> Result<Vec<TestGroupRegistration>, Vec<Diagnostic>> {
    let is_group = |operation| {
        matches!(
            operation,
            BuildOperation::TestGroupRegistration | BuildOperation::TestGroupService
        )
    };
    let mut events = executed
        .iter()
        .filter(|event| is_group(event.request.operation))
        .peekable();
    let mut authored = formations
        .rows
        .iter()
        .filter(|row| is_group(row.request.operation))
        .peekable();
    if authored.peek().is_none() && events.peek().is_none() {
        return Ok(Vec::new());
    }
    let declarations = ProductDeclarationLookup::new(
        typed,
        &[
            ProductOperandRole::TestRequirement,
            ProductOperandRole::BoundarySubject,
        ],
    );
    // Every authored registration names a real requirement and service slot,
    // whether or not the Build reached it.
    for formation in authored {
        test_group_operands(typed, &declarations, formation.request)?;
    }
    let authors = ProductRequestAuthors::new(typed);
    let mut groups = Vec::<TestGroupRegistration>::new();
    for event in events {
        let formation = formations.executed(typed, &authors, activation, event, "test group")?;
        let request = formation.request;
        let (requirement, slot) = test_group_operands(typed, &declarations, request)?;
        let source_span = typed.symbols.rejoin_product_occurrence(request.source_span);
        match slot {
            None => {
                if groups.iter().any(|group| group.requirement == requirement) {
                    return Err(refuse(
                        request,
                        "test group registration repeats the same requirement",
                    ));
                }
                groups.push(TestGroupRegistration {
                    requirement,
                    selecting_machine: formation.author,
                    source_span,
                    services: Vec::new(),
                });
            }
            Some(slot) => {
                let group = groups
                    .iter_mut()
                    .find(|group| group.requirement == requirement)
                    .ok_or_else(|| {
                        refuse(
                            request,
                            "test group service names a group this Build has not registered; register it with `builder.tests.group<…>()` first",
                        )
                    })?;
                if group.services.iter().any(|service| service.slot == slot) {
                    return Err(refuse(
                        request,
                        "test group service repeats the same slot for one group",
                    ));
                }
                group.services.push(TestGroupService { slot, source_span });
            }
        }
    }
    Ok(groups)
}

/// The requirement a test-group request names and, for a service request, the
/// boundary trait slot it asks the runner to back.
fn test_group_operands(
    typed: &TypedTrees,
    declarations: &ProductDeclarationLookup<'_>,
    request: &ProductDeclarationRequest,
) -> Result<(SymbolHandle, Option<SymbolHandle>), Vec<Diagnostic>> {
    let resolve = |position: usize, role: ProductOperandRole| {
        request
            .operands
            .get(position)
            .filter(|operand| operand.application.is_none())
            .and_then(|operand| declarations.resolve(&operand.path, operand.source_span, role))
    };
    let path = |position: usize| {
        request
            .operands
            .get(position)
            .map_or("", |operand| operand.path.as_str())
    };
    let requirement = resolve(0, ProductOperandRole::TestRequirement).ok_or_else(|| {
        refuse(
            request,
            format!(
                "test group operand `{}` must name one visible requirement of a non-boundary trait",
                path(0)
            ),
        )
    })?;
    if request.operation != BuildOperation::TestGroupService {
        return Ok((requirement, None));
    }
    let slot = resolve(1, ProductOperandRole::BoundarySubject)
        .filter(|symbol| {
            typed
                .trait_definition_by_symbol(*symbol)
                .is_some_and(|declaration| declaration.is_boundary)
        })
        .ok_or_else(|| {
            refuse(
                request,
                format!(
                    "test group service operand `{}` must name one visible boundary trait declaration",
                    path(1)
                ),
            )
        })?;
    Ok((requirement, Some(slot)))
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
