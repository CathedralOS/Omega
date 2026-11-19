//! Build acceptance records intent for one exact subject. Selected-plan and
//! package trust settlement decide whether that subject can be admitted.

use super::authored_requests::{AuthoredRequests, refusal};
use crate::declaration_selection::product_operands::ProductStaticEnvironment;
use crate::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest, ProductOperandRole,
};
use crate::source_state::TypedTrees;
use crate::source_state::product_operand::ProductDeclarationLookup;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;
use symbols::root_grants::{AuthoredRootGrant, RootGrantSubject};

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
        if machine.supply_mode == source_declarations::MachineSupplyMode::AdmissionClaim {
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

pub fn collect_product_root_grants(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
) -> Result<Vec<AuthoredRootGrant>, Vec<Diagnostic>> {
    if !declared
        .iter()
        .any(|r| r.operation == BuildOperation::BoundaryAcceptance)
        && !executed
            .iter()
            .any(|e| e.request.operation == BuildOperation::BoundaryAcceptance)
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_state::product_operand::collect_product_selection_occurrences;
    use crate::source_symbols::SourceScopedTopLevelBinding;
    use source_admission::{SourceOrigin, SourceResolutionStratum};
    use source_provenance::DependencyScope;

    #[test]
    fn a_grant_requires_execution_of_its_original_build_occurrence() {
        let prelude = r#"
public data Build { marker: u64; }
public boundary trait BuildBindingOperations {
    machine prepare(builder: @Build) -> u64 reaches none crashes none;
    machine select(activation: u64) reaches none crashes none;
}
public machine build_selection_receiver<'a>(builder: @'a mut Build) -> @'a mut Build { builder }
"#;
        let build =
            "machine build(builder: @mut Build) { Build::accept_boundary<First>(builder); }";
        let product = "public boundary trait Service { machine tick() reaches none crashes none; }
                       public First satisfies Service { machine tick() reaches none crashes none {} }";
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let mut syntax = None;
        let mut ids = Vec::new();
        for (path, text, origin, scope) in [
            (
                "root/build.omg",
                build,
                SourceOrigin::User,
                DependencyScope::Build,
            ),
            (
                "<build-prelude>",
                prelude,
                SourceOrigin::Toolchain,
                DependencyScope::Build,
            ),
            (
                "<build-prelude:Build>",
                "module Build;",
                SourceOrigin::Toolchain,
                DependencyScope::Build,
            ),
            (
                "root/main.omg",
                product,
                SourceOrigin::User,
                DependencyScope::Product,
            ),
        ] {
            let id = sources
                .add_checked_instance(
                    path.into(),
                    text,
                    "root".into(),
                    Some(package),
                    origin,
                    SourceResolutionStratum::Base,
                    scope,
                )
                .source_id;
            ids.push(id);
            let tokens = crate::frontend_fixture::Lexer::new(text)
                .tokenize()
                .unwrap();
            match &mut syntax {
                Some(syntax) => {
                    crate::frontend_fixture::parse(syntax, id, &tokens).unwrap();
                }
                None => {
                    syntax = Some(
                        crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap(),
                    );
                }
            }
        }
        let resolved = crate::resolution::resolve(crate::resolution::ResolutionRequest {
            syntax: syntax.unwrap(),
            sources: Some(Arc::new(sources)),
            retained_base: None,
            top_level_bindings: vec![
                SourceScopedTopLevelBinding::new(ids[0], ids[1], "Build"),
                SourceScopedTopLevelBinding::module_import(ids[0], ids[2], "Build", 0),
            ],
        })
        .unwrap();
        let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let occurrence = collect_product_selection_occurrences(&typed)
            .into_iter()
            .find(|row| row.request.operation == BuildOperation::BoundaryAcceptance)
            .unwrap();
        let author = typed.symbols.authored_origin(occurrence.machine);
        let declared = [occurrence.request];
        let event = ExecutedProductSelection {
            request: Arc::clone(&declared[0]),
            selecting_machine: typed.symbols.symbol_source_span(author),
            target: None,
            composition: None,
            static_environment: None,
        };
        assert!(
            collect_product_root_grants(&typed, None, &declared, &[])
                .unwrap()
                .is_empty()
        );
        let events = [event];
        let grants = collect_product_root_grants(&typed, Some(author), &declared, &events).unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].selecting_machine, author);
        assert_eq!(grants[0].subject_package, Some(package));
        assert!(matches!(
            &grants[0].subject,
            RootGrantSubject::Provider {
                application: None,
                ..
            }
        ));
        for (activation, requests, required) in [
            (None, declared.as_slice(), "no original Build activation"),
            (
                Some(author),
                &[][..],
                "does not match its declared source occurrence and author",
            ),
        ] {
            let refused =
                collect_product_root_grants(&typed, activation, requests, &events).unwrap_err();
            assert!(refused[0].message.contains(required), "{refused:?}");
        }
    }
}
