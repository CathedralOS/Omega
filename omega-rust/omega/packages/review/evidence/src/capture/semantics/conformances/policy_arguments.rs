//! Reify retained resolved arguments without parsing their display paths.

use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "conformance policy cannot retain {reason}"
    ))]
}

pub(super) fn argument_type_reference(
    compilation: &mut CheckedCompilation,
    argument: &StaticMachineArgument,
    kind: &TypeParameterKind,
    depth: usize,
) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
    if depth >= 64 || argument.evidence_projection.is_some() {
        return Err(rejected("an unsupported or over-deep static argument"));
    }
    if let Some(literal) = &argument.const_literal {
        if !matches!(kind, TypeParameterKind::Const { .. }) || argument.application.is_some() {
            return Err(rejected("a const literal outside its exact telescope slot"));
        }
        return Ok(compilation
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(literal.text()),
            }));
    }
    if !argument.symbol.is_valid() {
        return Err(rejected("an argument without an exact resolved symbol"));
    }
    if let Some(application) = &argument.application {
        if !matches!(kind, TypeParameterKind::Type) {
            return Err(rejected("a nested application outside a type slot"));
        }
        let definition = compilation
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == argument.symbol)
            .cloned()
            .ok_or_else(|| rejected("a generic application without an exact data declaration"))?;
        let parameters = compilation.data_type_parameters(&definition).to_vec();
        if parameters.len() != application.arguments.len()
            || definition.lifetime_parameters.len() != application.lifetime_arguments.len()
        {
            return Err(rejected("a generic application with stale telescope arity"));
        }
        let mut arguments = Vec::with_capacity(parameters.len());
        for (parameter, child) in parameters.iter().zip(application.arguments.iter()) {
            arguments.push(argument_type_reference(
                compilation,
                child,
                &parameter.kind,
                depth + 1,
            )?);
        }
        let arguments = compilation
            .typed
            .type_reference_table
            .insert_type_reference_handles(arguments);
        return Ok(compilation
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: definition.symbol,
                base_name: definition.name,
                lifetime_arguments: application.lifetime_arguments.to_vec(),
                arguments,
            }));
    }
    let symbol_kind = compilation.symbols.get(argument.symbol).kind;
    match kind {
        TypeParameterKind::Const { .. } if symbol_kind == SymbolKind::Const => {
            let declaration = compilation
                .const_declarations()
                .iter()
                .find(|declaration| declaration.symbol == argument.symbol)
                .ok_or_else(|| rejected("a named const without its exact declaration"))?;
            let encoding = declaration
                .canonical_value_encoding
                .as_ref()
                .ok_or_else(|| rejected("a named const without a checked value"))?;
            validation::validate_exact_const_value_encoding(
                &compilation.typed,
                declaration.declared_type,
                encoding,
            )
            .map_err(|reason| {
                rejected(&format!(
                    "a named const with invalid carrier agreement: {reason}"
                ))
            })?;
            let value = CanonicalConstValue::new(
                compilation
                    .normalized_type_identity(declaration.declared_type)
                    .into_string(),
                encoding.clone(),
                "",
            );
            return Ok(compilation
                .typed
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
        TypeParameterKind::Const { .. } if symbol_kind == SymbolKind::TypeParameter => {}
        TypeParameterKind::Machine { .. }
            if matches!(
                symbol_kind,
                SymbolKind::State | SymbolKind::MachineParameter
            ) => {}
        _ => return Err(rejected("an argument of the wrong static category")),
    };
    let name = Identifier::generated(compilation.symbols.name(argument.symbol));
    Ok(compilation
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: argument.symbol,
            name,
        }))
}

pub(super) fn argument_context(
    compilation: &CheckedCompilation,
    arguments: &[StaticMachineArgument],
    lifetimes: &[Identifier],
    binders: &mut Vec<(SymbolHandle, String)>,
    depth: usize,
) -> Result<(), Vec<Diagnostic>> {
    if depth >= 64 {
        return Err(rejected("an over-deep argument context"));
    }
    for argument in arguments {
        if matches!(
            compilation.symbols.get(argument.symbol).kind,
            SymbolKind::TypeParameter | SymbolKind::MachineParameter
        ) && !binders.iter().any(|(symbol, _)| *symbol == argument.symbol)
        {
            let nominal =
                super::policy_callables::caller_binder_identity(compilation, argument.symbol)?;
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
                argument.symbol,
                super::super::encoding::framed_identity(
                    "conformance-caller-binder",
                    &[owner, nominal.path],
                ),
            ));
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
