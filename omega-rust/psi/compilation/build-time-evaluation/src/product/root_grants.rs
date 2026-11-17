//! Build acceptance records intent for one exact subject. Selected-plan and
//! package trust settlement decide whether that subject can be admitted.

use super::authored_requests::{AuthoredRequests, refusal};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use language_semantics::declaration_selection::{
    AuthoredRootGrant, BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
    ProductOperandRole, RootGrantSubject,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::ProductDeclarationLookup;

#[derive(Clone)]
struct FormedSubject {
    selector: String,
    subject: RootGrantSubject,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
}

fn subject(
    typed: &TypedTrees,
    declarations: &ProductDeclarationLookup<'_>,
    request: &ProductDeclarationRequest,
    author: SymbolHandle,
    environment: Option<&ProductStaticEnvironment>,
) -> Result<Option<FormedSubject>, Vec<Diagnostic>> {
    request
        .validate_shape()
        .map_err(|reason| refusal(request, reason))?;
    let [operand] = request.operands.as_ref() else {
        return Err(refusal(request, "root grant needs one exact subject"));
    };
    let role = ProductOperandRole::RootGrantSubject;
    let declaration = declarations.resolve(&operand.path, operand.source_span, role)
        .ok_or_else(|| refusal(request, format!("root grant subject `{}` must name one visible provider or admitted machine declaration", operand.path)))?;
    let (selected, application) = if operand.application.is_some() {
        let Some(handle) = typed.prepared_product_operand(operand, environment, author, role)
        else {
            return if environment.is_none() {
                Ok(None)
            } else {
                Err(refusal(
                    request,
                    "executed root grant lost its complete subject application",
                ))
            };
        };
        let prepared = typed.product_operand_applications.get(handle);
        if prepared.argument.symbol != declaration {
            return Err(refusal(
                request,
                "root grant application belongs to another declaration",
            ));
        }
        let identity = typed
            .product_operand_application_identity(handle)
            .map_err(|reason| refusal(request, reason))?;
        (prepared.declaration, identity)
    } else {
        (declaration, None)
    };
    let identity = typed
        .normalized_hermetic_symbol_identity(selected)
        .map_err(|reason| refusal(request, reason))?;
    let subject = if let Some(provider) = typed.conformance_by_symbol(declaration) {
        if application.is_none()
            && (!provider.type_parameters.is_empty() || !provider.lifetime_parameters.is_empty())
        {
            return Err(refusal(
                request,
                "root grant provider requires its complete static application",
            ));
        }
        RootGrantSubject::Provider {
            declaration: identity,
            application: application.clone(),
        }
    } else if let Some((owner, signature)) = typed.trait_machine_signature(selected) {
        RootGrantSubject::BoundaryRequirement {
            declaration: identity,
            callable: typed.trait_requirement_callable_identity_text(owner, signature),
        }
    } else if let Some(machine) = typed.machine_by_symbol(selected) {
        if machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim {
            RootGrantSubject::AcceptedMachine {
                declaration: identity,
                machine: selected,
            }
        } else {
            RootGrantSubject::BoundaryRequirement {
                declaration: identity,
                callable: typed
                    .machine_callable_identity_text(machine)
                    .ok_or_else(|| {
                        refusal(
                            request,
                            "root grant requirement has no complete callable identity",
                        )
                    })?,
            }
        }
    } else {
        return Err(refusal(
            request,
            "root grant lost its exact subject declaration",
        ));
    };
    Ok(Some(FormedSubject {
        selector: application.unwrap_or_else(|| typed.symbols.display_path(declaration, "::")),
        subject,
        package: typed.symbols.symbol_package_identity(declaration),
    }))
}

pub(super) fn collect(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
) -> Result<Vec<AuthoredRootGrant>, Vec<Diagnostic>> {
    if !super::build_requests_any(declared, executed, &[BuildOperation::BoundaryAcceptance]) {
        return Ok(Vec::new());
    }
    let authored = AuthoredRequests::new(typed, declared, BuildOperation::BoundaryAcceptance)?;
    let declarations =
        ProductDeclarationLookup::new(typed, &[ProductOperandRole::RootGrantSubject]);
    let formed = authored
        .rows
        .iter()
        .map(|row| subject(typed, &declarations, row.request, row.author, None))
        .collect::<Result<Vec<_>, _>>()?;
    let mut grants = Vec::new();
    for event in executed
        .iter()
        .filter(|event| event.request.operation == BuildOperation::BoundaryAcceptance)
    {
        let (index, author, _) = authored.execution(activation, event)?;
        let request = &event.request;
        let selector = if request
            .operands
            .iter()
            .any(|operand| operand.application.is_some())
        {
            subject(
                typed,
                &declarations,
                request,
                author,
                event.static_environment.as_deref(),
            )?
        } else {
            formed[index].clone()
        }
        .ok_or_else(|| {
            refusal(
                request,
                "executed root grant has no complete subject application",
            )
        })?;
        grants.push(AuthoredRootGrant {
            selector: selector.selector,
            subject: selector.subject,
            subject_package: selector.package,
            selecting_machine: author,
            source_span: typed.symbols.rejoin_product_occurrence(request.source_span),
        });
    }
    Ok(grants)
}
