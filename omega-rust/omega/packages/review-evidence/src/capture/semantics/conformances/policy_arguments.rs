//! Reify retained resolved arguments without parsing their display paths.

use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::StaticMachineArgument;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "conformance policy cannot retain {reason}"
    ))]
}

pub(crate) fn argument_type_reference(
    program: &mut typed_trees::TypedTrees,
    argument: &StaticMachineArgument,
    kind: &TypeParameterKind,
    depth: usize,
) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
    if depth >= 64 || argument.evidence_projection.is_some() {
        return Err(rejected("an unsupported or over-deep static argument"));
    }
    if let Some(literal) = &argument.const_literal {
        if !kind.is_const_or_value() || argument.application.is_some() {
            return Err(rejected("a const literal outside its exact telescope slot"));
        }
        let carrier = match kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => *type_reference,
            _ => unreachable!("const slot checked above"),
        };
        let integer = literal
            .decimal_value_i128()
            .ok_or_else(|| rejected("a non-integer const literal"))?;
        let primitive = program
            .type_reference_table
            .primitive_type(carrier)
            .filter(|primitive| primitive.accepts_integer_literal())
            .ok_or_else(|| rejected("an integer literal without its exact integer carrier"))?;
        let identity = CanonicalConstIdentity::integer(primitive.name(), integer);
        validation::validate_exact_const_value_encoding(program, carrier, &identity.encoding)
            .map_err(|reason| {
                rejected(&format!(
                    "a literal with invalid carrier agreement: {reason}"
                ))
            })?;
        let value = CanonicalConstValue::new(
            program.normalized_type_identity_text(carrier),
            identity.encoding,
            "",
        );
        return Ok(program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(value.atom()),
            }));
    }
    if argument.type_reference.is_valid() {
        if kind.is_const_or_value()
            && matches!(
                program
                    .type_reference_table
                    .type_reference(argument.type_reference),
                TypeReferenceNode::ConstExpression(_)
            )
            && !argument.symbol.is_valid()
            && argument.path.is_empty()
            && argument.application.is_none()
        {
            // The checked closed application owns category/carrier admission.
            // Publication preserves the same typed expression for exact-owner
            // identity and selected-operation validation below.
            return Ok(argument.type_reference);
        }
        if !matches!(kind, TypeParameterKind::Type) {
            return Err(rejected("a type payload outside its exact type slot"));
        }
        if let TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } = program
            .type_reference_table
            .type_reference(argument.type_reference)
            .clone()
        {
            let definition = program
                .data_definition_by_symbol(base_symbol)
                .cloned()
                .ok_or_else(|| rejected("a type payload without its exact data declaration"))?;
            let parameters = program.data_type_parameters(&definition).to_vec();
            let children = program
                .type_reference_table
                .type_reference_handles(arguments)
                .to_vec();
            if parameters.len() != children.len()
                || definition.lifetime_parameters.len() != lifetime_arguments.len()
            {
                return Err(rejected("a type payload with stale telescope arity"));
            }
            let children = children
                .iter()
                .zip(&parameters)
                .map(|(reference, parameter)| {
                    let child = program
                        .static_operand_from_type_reference(*reference)
                        .map_err(rejected)?;
                    let kind = applied_kind(program, &parameter.kind, &parameters, &children);
                    argument_type_reference(program, &child, &kind, depth + 1)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = program
                .type_reference_table
                .insert_type_reference_handles(children);
            let owner = program
                .type_reference_table
                .operand_lifetime_owner(argument.type_reference);
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    lifetime_arguments,
                    arguments,
                });
            program
                .type_reference_table
                .retain_operand_lifetime_owner(reference, owner)
                .map_err(rejected)?;
            return Ok(reference);
        }
        let node = program
            .type_reference_table
            .type_reference(argument.type_reference)
            .clone();
        let child = |program: &mut typed_trees::TypedTrees, reference| {
            argument_type_reference(
                program,
                &StaticMachineArgument {
                    type_reference: reference,
                    ..StaticMachineArgument::default()
                },
                &TypeParameterKind::Type,
                depth + 1,
            )
        };
        let node = match node {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => TypeReferenceNode::Reference {
                referee: child(program, referee)?,
                access,
                lifetime,
            },
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => TypeReferenceNode::FixedArray {
                element_type: child(program, element_type)?,
                length,
            },
            TypeReferenceNode::Slice { element_type } => TypeReferenceNode::Slice {
                element_type: child(program, element_type)?,
            },
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => TypeReferenceNode::Constrained {
                base_type: child(program, base_type)?,
                constraints,
            },
            _ => return Ok(argument.type_reference),
        };
        let owner = program
            .type_reference_table
            .operand_lifetime_owner(argument.type_reference);
        let reference = program.type_reference_table.insert(node);
        program
            .type_reference_table
            .retain_operand_lifetime_owner(reference, owner)
            .map_err(rejected)?;
        return Ok(reference);
    }
    if !argument.symbol.is_valid() {
        return Err(rejected("an argument without an exact resolved symbol"));
    }
    if let Some(application) = &argument.application {
        if !matches!(kind, TypeParameterKind::Type) {
            return Err(rejected("a nested application outside a type slot"));
        }
        let definition = program
            .data_definition_by_symbol(argument.symbol)
            .cloned()
            .ok_or_else(|| rejected("a generic application without an exact data declaration"))?;
        let parameters = program.data_type_parameters(&definition).to_vec();
        if parameters.len() != application.arguments.len()
            || definition.lifetime_parameters.len() != application.lifetime_arguments.len()
        {
            return Err(rejected("a generic application with stale telescope arity"));
        }
        let mut arguments = Vec::with_capacity(parameters.len());
        for (parameter, child) in parameters.iter().zip(application.arguments.iter()) {
            // Earlier type arguments supply declaration-owned carriers such as
            // Box<Element, const Count: Element>; never guess a literal width.
            let kind = applied_kind(program, &parameter.kind, &parameters, &arguments);
            arguments.push(argument_type_reference(program, child, &kind, depth + 1)?);
        }
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles(arguments);
        return Ok(program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: definition.symbol,
                base_name: definition.name,
                lifetime_arguments: application.lifetime_arguments.to_vec(),
                arguments,
            }));
    }
    let symbol_kind = program.symbols.get(argument.symbol).kind;
    match kind {
        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
            if symbol_kind == SymbolKind::Const =>
        {
            let declaration = program
                .const_declaration_by_symbol(argument.symbol)
                .ok_or_else(|| rejected("a named const without its exact declaration"))?;
            let encoding = declaration
                .evaluated_value
                .as_ref()
                .and_then(|value| value.encoding())
                .ok_or_else(|| rejected("a named const without a checked value"))?;
            validation::validate_exact_const_value_encoding(
                program,
                declaration.declared_type,
                encoding,
            )
            .map_err(|reason| {
                rejected(&format!(
                    "a named const with invalid carrier agreement: {reason}"
                ))
            })?;
            let value = CanonicalConstValue::new(
                program.normalized_type_identity_text(declaration.declared_type),
                encoding,
                "",
            );
            return Ok(program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::generated(value.atom()),
                }));
        }
        TypeParameterKind::Type
            if matches!(
                symbol_kind,
                SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
            ) => {}
        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
            if symbol_kind == SymbolKind::TypeParameter => {}
        TypeParameterKind::Machine { .. }
            if matches!(
                symbol_kind,
                SymbolKind::State | SymbolKind::MachineParameter
            ) => {}
        _ => return Err(rejected("an argument of the wrong static category")),
    };
    let name = Identifier::generated(program.symbols.name(argument.symbol));
    Ok(program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: argument.symbol,
            name,
        }))
}

pub(super) fn argument_context(
    compilation: &PackageReviewInput<'_>,
    arguments: &[StaticMachineArgument],
    lifetimes: &[Identifier],
    binders: &mut Vec<(SymbolHandle, String)>,
    depth: usize,
) -> Result<(), Vec<Diagnostic>> {
    if depth >= 64 {
        return Err(rejected("an over-deep argument context"));
    }
    // Reuse the typed ownership traversal, including const-expression leaves;
    // a nested static operand cannot lose its supplying declaration merely
    // because its outer storage lane is a type-reference handle.
    struct ArgumentSymbols(Vec<SymbolHandle>);
    impl Extend<SymbolHandle> for ArgumentSymbols {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
            self.0.extend(symbols);
        }
    }
    impl typed_trees::references::TypedReferences for ArgumentSymbols {}
    for argument in arguments {
        let mut symbols = ArgumentSymbols(Vec::new());
        typed_trees::references::visit_static_argument(
            compilation.checking_input.typed_program(),
            &mut symbols,
            argument,
        );
        for symbol in symbols.0 {
            if matches!(
                compilation.symbols.get(symbol).kind,
                SymbolKind::TypeParameter | SymbolKind::MachineParameter
            ) && !binders.iter().any(|(candidate, _)| *candidate == symbol)
            {
                let nominal = super::policy_callables::caller_binder_identity(compilation, symbol)?;
                let owner = match nominal.owner {
                    crate::record::PackageReviewNominalOwner::Package(package) => {
                        super::super::encoding::canonical_digest_label("package", package.digest())
                    }
                    crate::record::PackageReviewNominalOwner::ToolchainSource(source) => {
                        super::super::encoding::canonical_digest_label(
                            "toolchain-source",
                            source.digest(),
                        )
                    }
                    crate::record::PackageReviewNominalOwner::Unresolved => {
                        return Err(rejected("an unowned caller binder"));
                    }
                };
                binders.push((
                    symbol,
                    super::super::encoding::framed_identity(
                        "conformance-caller-binder",
                        &[owner, nominal.path],
                    ),
                ));
            }
        }
        if let Some(application) = &argument.application {
            for lifetime in &application.lifetime_arguments {
                if !lifetimes.contains(lifetime) {
                    return Err(rejected(
                        "a nested lifetime outside the containing telescope",
                    ));
                }
            }
            argument_context(
                compilation,
                &application.arguments,
                lifetimes,
                binders,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

// Resolve only an exact type binder's carrier in the containing data telescope.
// This mirrors ordinary applied-carrier checking without rewriting its declaration.
fn applied_kind(
    program: &typed_trees::TypedTrees,
    kind: &TypeParameterKind,
    parameters: &[TypeParameter],
    arguments: &[TypeReferenceHandle],
) -> TypeParameterKind {
    let carrier = match kind {
        TypeParameterKind::Const { type_reference }
        | TypeParameterKind::Value { type_reference } => *type_reference,
        _ => return kind.clone(),
    };
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(carrier)
    else {
        return kind.clone();
    };
    let actual = parameters
        .iter()
        .zip(arguments)
        .find_map(|(parameter, argument)| {
            (symbol.is_valid()
                && parameter.symbol == *symbol
                && matches!(parameter.kind, TypeParameterKind::Type))
            .then_some(*argument)
        })
        .unwrap_or(carrier);
    match kind {
        TypeParameterKind::Const { .. } => TypeParameterKind::Const {
            type_reference: actual,
        },
        TypeParameterKind::Value { .. } => TypeParameterKind::Value {
            type_reference: actual,
        },
        _ => unreachable!("value carrier checked above"),
    }
}
