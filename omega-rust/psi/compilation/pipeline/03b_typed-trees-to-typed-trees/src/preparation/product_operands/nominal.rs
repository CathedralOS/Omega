//! Form designated boundary and implementation applications through the same
//! owned graph and ordinary Type/Const application owners as callable operands.
use super::*;
use typed_trees::data::TypeParameterKind;
use typed_trees::product_operand::{ProductApplicationDemand, ProductOperandApplication};
use typed_trees::trait_definition::TraitApplicationArguments;

#[derive(Clone, Copy)]
enum NominalDeclaration {
    Authored,
    Retained(SymbolHandle),
}

fn require_application_inputs(
    program: &mut TypedTrees,
    arguments: &[expr::StaticMachineArgument],
    lexical_owner: SymbolHandle,
    demand: ProductApplicationDemand,
) -> Result<(), Vec<Diagnostic>> {
    let references = arguments
        .iter()
        .map(|argument| program.static_operand_type_reference_in_scope(argument, lexical_owner))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let result = match demand {
        ProductApplicationDemand::FormationOnly => {
            program.require_declared_application_operands(&references, lexical_owner)
        }
        ProductApplicationDemand::Selected => {
            program.require_owned_application_operands(&references, lexical_owner)
        }
    };
    result.map_err(|reason| vec![Diagnostic::error(reason)])
}

pub(super) fn require_trait_inputs(
    program: &TypedTrees,
    scope: &AppliedTraitScope<'_>,
    demand: ProductApplicationDemand,
) -> Result<(), &'static str> {
    match demand {
        ProductApplicationDemand::FormationOnly => program.require_declared_trait_operands(scope),
        ProductApplicationDemand::Selected => program.require_receiving_trait_operands(scope),
    }
}

pub fn prepare_retained_product_application(
    program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    role: ProductOperandRole,
    declaration: SymbolHandle,
) -> Result<Handle<ProductOperandApplication>, Vec<Diagnostic>> {
    let handle = prepare(
        program,
        operand,
        role,
        declaration,
        None,
        ProductApplicationDemand::Selected,
        NominalDeclaration::Retained(declaration),
    )?
    .ok_or_else(|| {
        vec![Diagnostic::error(
            "retained Product choice lost its closed application",
        )]
    })?;
    program.retain_dependency_product_application(handle);
    Ok(handle)
}

pub fn prepare_product_operand_application(
    program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    role: ProductOperandRole,
    author: SymbolHandle,
    static_environment: Option<&ProductStaticEnvironment>,
    demand: ProductApplicationDemand,
) -> Result<Option<Handle<ProductOperandApplication>>, Vec<Diagnostic>> {
    prepare(
        program,
        operand,
        role,
        author,
        static_environment,
        demand,
        NominalDeclaration::Authored,
    )
}

fn prepare(
    program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    role: ProductOperandRole,
    author: SymbolHandle,
    static_environment: Option<&ProductStaticEnvironment>,
    demand: ProductApplicationDemand,
    binding: NominalDeclaration,
) -> Result<Option<Handle<ProductOperandApplication>>, Vec<Diagnostic>> {
    let Some(graph) = operand.application.as_deref() else {
        return Ok(None);
    };
    let refuse =
        |reason: &str| vec![Diagnostic::error(reason).with_source_span(operand.source_span)];
    if !matches!(
        role,
        ProductOperandRole::BoundarySubject | ProductOperandRole::Provider
    ) {
        return Err(refuse(
            "nominal Product application has a non-nominal declaration role",
        ));
    }
    if let Some(existing) =
        program.prepared_product_operand(operand, static_environment, author, role)
    {
        if demand == ProductApplicationDemand::Selected {
            let retained = program.product_operand_applications.get(existing);
            let supplying = retained.supplying_application;
            let lexical_owner = program
                .expression_table
                .static_machine_application(supplying)
                .lexical_owner;
            let [whole] = program
                .expression_table
                .static_machine_application_arguments(supplying)
            else {
                return Err(refuse("Product application lost its whole closed operand"));
            };
            let arguments = whole
                .application
                .as_ref()
                .ok_or_else(|| refuse("Product application lost its static telescope"))?
                .arguments
                .clone();
            let retained_scope = retained.scope;
            require_application_inputs(program, &arguments, lexical_owner, demand)?;
            let scope = retained_scope
                .scope(program)
                .ok_or_else(|| refuse("Product application lost its receiving scope"))?;
            require_trait_inputs(program, &scope, demand).map_err(refuse)?;
            program
                .product_operand_applications
                .get_mut(existing)
                .demand = demand;
        }
        return Ok(Some(existing));
    }
    let lexical_owner = environment::supplier_owner(program, static_environment, author)?;
    let mut lower = Lower::new(program, graph, lexical_owner, static_environment);
    if matches!(binding, NominalDeclaration::Retained(_)) {
        lower.naming = OperandNaming::RetainedDeclarations;
    }
    if !graph.operands.is_valid(graph.root) {
        return Err(refuse("Product application lost its owned root"));
    }
    let root = graph.operands.get(graph.root).clone();
    if !graph.applications.is_valid(root.application) {
        return Err(refuse("Product application lost its explicit telescope"));
    }
    if graph.applications.get(root.application).member.is_valid() {
        return Err(refuse(
            "a boundary or implementation application cannot select a callable member",
        ));
    }
    let path = lower.path(root.path)?;
    if path != operand.path {
        return Err(refuse(
            "Product application changed its source-issued declaration path",
        ));
    }
    let declaration = match binding {
        NominalDeclaration::Retained(declaration) => declaration,
        NominalDeclaration::Authored => typed_trees::product_operand::resolve_product_path(
        lower.program,
        &path,
        operand.source_span,
        role,
    )
    .ok_or_else(|| {
        refuse(
            "Product application does not name one exact visible declaration of its required role",
        )
    })?,
    };
    let source_argument = expr::StaticMachineArgument {
        symbol: declaration,
        path: Box::from([Identifier::generated(path)]),
        application: Some(Box::new(lower.application_arguments(root.application)?)),
        ..Default::default()
    };
    // Retain unfurled recipes before canonical closure replaces expressions.
    // Checking, rather than the folded tuple, owns arithmetic formation.
    let parameters = match role {
        ProductOperandRole::BoundarySubject => {
            let definition = lower
                .program
                .trait_definition_by_symbol(declaration)
                .ok_or_else(|| refuse("an applied boundary lost its trait declaration"))?;
            lower.program.trait_type_parameters(definition).to_vec()
        }
        ProductOperandRole::Provider => {
            let definition = lower
                .program
                .conformance_by_symbol(declaration)
                .ok_or_else(|| {
                    refuse("an applied provider must name an ordinary named implementation")
                })?;
            lower
                .program
                .conformance_type_parameters(definition)
                .to_vec()
        }
        _ => unreachable!("nominal roles checked above"),
    };
    let arguments = &source_argument
        .application
        .as_ref()
        .expect("created above")
        .arguments;
    if parameters.len() != arguments.len() {
        return Err(refuse(
            "Product application must supply its complete declaration telescope",
        ));
    }
    for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
        // Only Type actuals are runtime type formation roots. Const recipes
        // remain unfurled in the retained application and are independently
        // judged by the ordinary static-argument and arithmetic checkers.
        if matches!(binding, NominalDeclaration::Authored)
            && matches!(parameter.kind, TypeParameterKind::Type)
        {
            let reference = lower
                .program
                .static_operand_type_reference_in_scope(argument, lexical_owner)
                .map_err(refuse)?;
            lower.program.retain_designated_static_type_argument(
                lexical_owner,
                reference,
                operand.source_span,
            );
        }
    }
    let mut closed = source_argument.clone();
    crate::monomorphization::normalize_const_arguments(
        lower.program,
        lexical_owner,
        std::slice::from_mut(&mut closed),
    )
    .map_err(|reason| refuse(&reason))?;
    let arguments = closed
        .application
        .as_ref()
        .expect("normalization retains a nominal application");
    require_application_inputs(lower.program, &arguments.arguments, lexical_owner, demand)?;
    let (scope, implementation) = match role {
        ProductOperandRole::BoundarySubject => {
            let reference = lower
                .program
                .static_operand_type_reference_in_scope(&closed, lexical_owner)
                .map_err(refuse)?;
            let scope =
                AppliedTraitScope::from_type_reference(lower.program, reference, lexical_owner)
                    .ok_or_else(|| {
                        refuse("boundary application has an invalid or incomplete telescope")
                    })?;
            require_trait_inputs(lower.program, &scope, demand).map_err(refuse)?;
            (
                scope.environment().retain(lower.program).map_err(refuse)?,
                None,
            )
        }
        ProductOperandRole::Provider => {
            let application = validation::conformance_applications::close_conformance_application(
                lower.program,
                &closed,
            )
            .map_err(|diagnostic| vec![diagnostic.with_source_span(operand.source_span)])?;
            let references = arguments
                .arguments
                .iter()
                .map(|argument| {
                    lower
                        .program
                        .static_operand_type_reference_in_scope(argument, lexical_owner)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(refuse)?;
            let definition = lower
                .program
                .conformance_by_symbol(declaration)
                .expect("checked above");
            let scope = AppliedTraitScope::from_conformance_application(
                lower.program,
                definition,
                TraitApplicationArguments {
                    lexical_owner,
                    enclosing_data_applications: &[],
                    arguments: &references,
                    lifetimes: &arguments.lifetime_arguments,
                },
            )
            .ok_or_else(|| {
                refuse("implementation application lost its exact satisfied boundary scope")
            })?;
            require_trait_inputs(lower.program, &scope, demand).map_err(refuse)?;
            (
                scope.environment().retain(lower.program).map_err(refuse)?,
                Some(application),
            )
        }
        _ => unreachable!("nominal roles checked above"),
    };
    let supplying = lower
        .program
        .expression_table
        .retain_static_machine_application(
            declaration,
            Identifier::generated(&operand.path),
            std::slice::from_ref(&closed),
        );
    let application = lower
        .program
        .expression_table
        .static_machine_application_mut(supplying);
    application.binding_declaration = declaration;
    application.lexical_owner = lexical_owner;
    if let Some(implementation) = &implementation {
        application.conformance_selection = Some(Box::new(
            typed_trees::static_applications::StaticConformanceSelection {
                operand_ordinal: 0,
                application: Some(Box::new(implementation.clone())),
            },
        ));
    }
    let input_selections = lower
        .program
        .product_input_selections
        .insert_many(std::mem::take(&mut lower.input_selections));
    let retained = ProductOperandApplication {
        input_selections,
        operand: operand.clone(),
        environment: static_environment.cloned().map(std::sync::Arc::new),
        owner: author,
        role: Some(role),
        declaration,
        demand,
        argument: source_argument,
        scope,
        supplying_application: supplying,
        implementation,
    };
    Ok(Some(
        lower.program.retain_product_operand_application(retained),
    ))
}
