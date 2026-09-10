//! Constant substitution and exact const-index declaration custody; value contract: wiki/spec/language/constants.md.
//!
//! Const VALUE semantics exist only until symbol resolution:
//! A constant declares a named pure value; each selected expression path gets
//! a fresh copy of its initializer before resolution publishes its result.
//! Symbol-resolved trees, typed trees,
//! validation, proofs, backends, and the interpreter never grow a const-value
//! concept -- each use IS the literal, which is exactly the copied-at-each-use
//! semantics the contract specifies (and why interior mutability can never hide in
//! one). The symbol table retains only declaration provenance so authored-
//! selection and package-authority checks cannot be erased by substitution.
//!
//! Scalar and closed aggregate literals substitute
//! only after the shared resolver has selected their namespace and lexical
//! binding. Detached initializer roots resolve constructors and fields in their
//! declaring source. Each use deep-copies aggregate children and carries both
//! that declaration-side selection custody and its own constant-use occurrence.
//! Root and module declarations use this same path; no pre-resolution spelling
//! substitution or whole-forest local-shadowing restriction is needed. Module-owned scoped declarations
//! additionally select an exact nongeneric carrier in their declaring module.
//! The authored scope token survives until complete symbol assignment, then joins
//! the ordinary visibility/selection ledger; the structural value encoder alone
//! cannot establish attachment ownership. Seeded declarations keep their existing
//! selections rather than inventing a new scope occurrence from a display name.
//! Numeric and Boolean initializers owe their declared landing even when private
//! and unused. Check declarations before substitution, reusing the same validator
//! during module normalization so public identity construction cannot bypass it.
//!
//! Nominal index values retain a private link to the actual parent-owned
//! argument span while lowering. Finalization rejoins the resolved parameter
//! and selected declaration before publishing occurrence custody. The ordinary
//! generic application and indexed-domain owners provide that relationship;
//! Body materialization retains those same nominal declaration identities without
//! turning canonical index encodings into constructor authority.
//!
//! Remaining boundaries, enforced loudly:
//! - LITERAL-ONLY initializers (scalars, negated scalars -- already folded by
//!   the parser -- payloadless cases, and struct/array literals of those).
//!   Richer const expressions are the build-time-evaluation arc.
//! - A const may not collide with a case of its scope type: `Type::NAME` must
//!   stay unambiguous against case-constructor paths, which substitution
//!   would otherwise shadow.
//! - Selected numeric values retain their declared landing recursively through
//!   arrays. Destination checking rejoins complete array declaration types from
//!   retained selections, so empty arrays cannot lose their element identity.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionRecordError,
};
use source::{SourceSpan, Span};
use symbol_resolved_trees::{SymbolResolvedTrees, expression::ExpressionHandle};
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConstDefinition, DataMember, Item};

mod carrier;

/// Public declaration identity includes floating scalars with determined bits, independently
/// of the narrower structural values eligible for generic and domain indices.
pub(crate) fn public_declaration_value_encoding(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&crate::generic_data::constant_selection::ConstantSelection>,
) -> Result<String, String> {
    use numerics::literals::FloatLiteral;
    use syntax_trees::{expression::ExpressionNode, types::TypeReferenceNode};

    if let TypeReferenceNode::Named(carrier) = syntax
        .type_references
        .type_reference(definition.type_reference)
        && matches!(carrier.as_str(), "f32" | "f64")
    {
        validate_scalar_initializer(syntax, definition)?;
        let literal = match syntax.expressions.expression(definition.value) {
            ExpressionNode::Float(text) => FloatLiteral::parse(text.as_str()),
            ExpressionNode::Integer(integer) => integer
                .value_bignum()
                .and_then(|value| FloatLiteral::parse(&value.to_string())),
            _ => None,
        }
        .ok_or("floating declaration identity requires a scalar literal")?;
        // Read the declared format directly from exact source meaning. Going
        // through f64 for f32 would introduce a second rounding at midpoints.
        // Signed infinity has one exact encoding per format. Payloadless NaN
        // meaning cannot choose representation bits for a public declaration.
        return if carrier.as_str() == "f32" {
            let value = literal.value_f32();
            if value.is_nan() {
                return Err(
                    "floating declaration identity requires explicit NaN representation bits"
                        .to_owned(),
                );
            }
            Ok(format!("float:f32:{:08x}", value.to_bits()))
        } else {
            let value = literal.value_f64();
            if value.is_nan() {
                return Err(
                    "floating declaration identity requires explicit NaN representation bits"
                        .to_owned(),
                );
            }
            Ok(format!("float:f64:{:016x}", value.to_bits()))
        };
    }
    crate::generic_data::canonicalize_selected_declared_const_definition(
        syntax, definition, selection,
    )
    .map(|value| value.encoding)
}

pub(crate) fn validate_scalar_initializer(
    syntax: &SyntaxTrees,
    constant: &syntax_trees::item::ConstDefinition,
) -> Result<(), String> {
    use numerics::literals::{FloatFormat, FloatLiteral};
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::types::TypeReferenceNode;

    // Unused private declarations never reach substitution or public identity
    // checks. Their numeric and Boolean landing obligations still apply.
    // Floating values need no const-index encoding to check their carrier.
    let TypeReferenceNode::Named(carrier) = syntax
        .type_references
        .type_reference(constant.type_reference)
    else {
        return Ok(());
    };
    if !matches!(
        carrier.as_str(),
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "addr"
            | "f32"
            | "f64"
    ) {
        return Ok(());
    }
    let initializer = syntax.expressions.expression(constant.value);
    match carrier.as_str() {
        "f32" | "f64" => {
            let format = if carrier.as_str() == "f32" {
                FloatFormat::F32
            } else {
                FloatFormat::F64
            };
            let compatible = match initializer {
                ExpressionNode::Float(text) => {
                    FloatLiteral::parse(text.as_str()).is_some_and(|literal| {
                        literal.landing().is_none_or(|landing| landing == format)
                    })
                }
                ExpressionNode::Integer(literal) => {
                    literal.landing().is_none() && literal.value_bignum().is_some()
                }
                _ => false,
            };
            if compatible {
                Ok(())
            } else {
                Err(format!(
                    "initializer conflicts with declared floating carrier `{carrier}`"
                ))
            }
        }
        _ => crate::generic_data::canonicalize_declared_const_definition(syntax, constant)
            .map(|_| ()),
    }
}

/// Declaration validity precedes all use-site substitution, including unused
/// private declarations. Namespace and case collisions are checked later against
/// exact symbols; lexical locals do not make a constant declaration ambiguous.
pub(crate) fn validate_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<(), Diagnostic> {
    validate_scalar_initializer(syntax, definition).map_err(|reason| {
        Diagnostic::error(format!(
            "scalar constant `{}` is invalid: {reason}",
            definition.name.as_str()
        ))
        .with_source_span(definition.name.source_span())
    })?;
    validate_literal_initializer(syntax, definition, definition.value)
}

pub(crate) fn retain_const_initializer(
    lowerer: &mut crate::lowerer::Lowerer,
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<(), Diagnostic> {
    if !has_scalar_initializer(syntax, definition) {
        if crate::module_normalization::module_literal_constant(syntax, definition) {
            // Unused private arrays still owe declaration shape and landing.
            crate::generic_data::canonicalize_declared_const_definition(syntax, definition)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "array constant `{}` is invalid: {reason}",
                        semantic_const_name(definition)
                    ))
                    .with_source_span(definition.name.source_span())
                })?;
        } else {
            validate_literal_initializer(syntax, definition, definition.value)?;
        }
    }
    let initializer =
        crate::expression::lower_expression_into_table(lowerer, syntax, definition.value)?;
    lowerer
        .pending_const_values
        .push((lowerer.pending_const_declarations.len(), initializer));
    Ok(())
}

fn has_scalar_initializer(syntax: &SyntaxTrees, definition: &ConstDefinition) -> bool {
    use syntax_trees::expression::ExpressionNode;
    matches!(
        syntax.expressions.expression(definition.value),
        ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_)
    )
}

pub(crate) fn substitute_resolved_constants(
    program: &mut SymbolResolvedTrees,
    initializers: &[(usize, ExpressionHandle)],
    authored: &[crate::lowerer::PendingAuthoredExpression],
    selections: &mut Vec<crate::lowerer::PendingConstSelection>,
    retained_const_count: usize,
    retain_selection_only: bool,
) -> Result<(), Diagnostic> {
    use symbol_resolved_trees::expression::ExpressionNode;
    let declarations = program
        .roots
        .const_declarations
        .iter()
        .map(|declaration| declaration.symbol)
        .collect::<Vec<_>>();
    for (ordinal, symbol) in declarations.iter().enumerate() {
        if declarations[..ordinal].iter().any(|other| {
            program.symbols.name(*other) == program.symbols.name(*symbol)
                && program.symbols.same_symbol_source_package(*other, *symbol)
                && !program.symbols.source_scopes_separate(*other, *symbol)
        }) {
            return Err(Diagnostic::error(format!(
                "duplicate const `{}`",
                program.symbols.display_path(*symbol, "::")
            ))
            .with_source_span(
                program
                    .symbols
                    .symbol_source_span(*symbol)
                    .unwrap_or_default(),
            ));
        }
        if let Some((carrier, case)) = program.symbols.name(*symbol).rsplit_once("::") {
            let reference = program
                .symbols
                .symbol_source_span(*symbol)
                .unwrap_or_default();
            if let Some(data) = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    carrier,
                    &[SymbolKind::Data],
                    reference,
                )
                && program
                    .symbols
                    .find_child_by_name_and_kind(data, case, SymbolKind::Variant)
                    .is_some()
            {
                return Err(Diagnostic::error(format!(
                    "const `{}` collides with the case `{case}`",
                    program.symbols.display_path(*symbol, "::")
                ))
                .with_source_span(reference));
            }
        } else if program
            .symbols
            .child_handles(program.symbols.root())
            .into_iter()
            .flatten()
            .any(|other| {
                matches!(
                    program.symbols.get(other).kind,
                    SymbolKind::Data | SymbolKind::Machine
                ) && program.symbols.name(other) == program.symbols.name(*symbol)
                    && program.symbols.same_symbol_source_package(other, *symbol)
                    && !program.symbols.source_scopes_separate(other, *symbol)
            })
        {
            return Err(Diagnostic::error(format!(
                "free-floating const `{}` collides with a declaration in the same namespace",
                program.symbols.display_path(*symbol, "::")
            ))
            .with_source_span(
                program
                    .symbols
                    .symbol_source_span(*symbol)
                    .unwrap_or_default(),
            ));
        }
    }
    // Fixed scalar projections preserve their full typed indexing expression.
    // Dynamic selectors, slicing and borrowed projections still need value/view
    // lowering. Fence their original root before substituting names, including
    // nonliteral outer selectors in a nested projection.
    let unsupported_array_projection_sources = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, node)| {
            let mut collection = match node {
                ExpressionNode::Indexed(indexed)
                    if !matches!(
                        program.tables.bodies.expressions.expression(indexed.index),
                        ExpressionNode::Integer(_)
                    ) || matches!(
                        program
                            .tables
                            .bodies
                            .expressions
                            .expression(indexed.collection),
                        ExpressionNode::Borrow(_)
                    ) =>
                {
                    indexed.collection
                }
                ExpressionNode::Borrow(borrow)
                    if matches!(
                        program.tables.bodies.expressions.expression(borrow.target),
                        ExpressionNode::Indexed(_)
                    ) =>
                {
                    borrow.target
                }
                _ => return None,
            };
            loop {
                match program.tables.bodies.expressions.expression(collection) {
                    ExpressionNode::Borrow(borrow) => collection = borrow.target,
                    ExpressionNode::Indexed(inner) => collection = inner.collection,
                    _ => break,
                }
            }
            Some(collection)
        })
        .collect::<Vec<_>>();
    for occurrence in authored {
        let ExpressionNode::Name(path) = program
            .tables
            .bodies
            .expressions
            .expression(occurrence.expression)
        else {
            continue;
        };
        if path.is_self_value
            || matches!(
                program.symbols.get(path.head_symbol).kind,
                SymbolKind::Local
                    | SymbolKind::Parameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::TypeParameter
                    | SymbolKind::ConformanceParameter
            )
        {
            continue;
        }
        let members = program
            .tables
            .bodies
            .expressions
            .name_path_members(path.members);
        let Some(first) = members.first() else {
            continue;
        };
        let Some(last) = members.last() else {
            continue;
        };
        let reference = SourceSpan::new(
            first.source_span().source_id,
            Span::new(first.source_span().span.start, last.source_span().span.end),
        );
        let name = members
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let Some(selected) = program
            .symbols
            .find_top_level_by_name_and_kinds_from_source(&name, &[SymbolKind::Const], reference)
        else {
            continue;
        };
        let Some(declaration_ordinal) = declarations.iter().position(|symbol| *symbol == selected)
        else {
            return Err(Diagnostic::error(
                "resolved constant selection has no retained declaration",
            ));
        };
        if retain_selection_only {
            // This private prepass publishes declaration custody, not a value
            // for typing or execution. Leave the resolved expression intact.
            selections.push(crate::lowerer::PendingConstSelection {
                expression: occurrence.expression,
                source_span: reference,
                declaration_ordinal,
                exposure: occurrence.exposure,
            });
            continue;
        }
        let Some((_, initializer)) = initializers
            .iter()
            .find(|(ordinal, _)| *ordinal == declaration_ordinal)
        else {
            let message = if declaration_ordinal < retained_const_count {
                "seeded constant references require retained initializer substitution"
            } else {
                "constant substitution lost its retained declaration initializer"
            };
            return Err(Diagnostic::error(message).with_source_span(reference));
        };
        // Every aggregate child belongs to this use. Constructor and field
        // symbols were selected in the declaring source before this deep copy;
        // consumer spelling and later numeric landing cannot reinterpret them.
        if matches!(
            program.tables.bodies.expressions.expression(*initializer),
            ExpressionNode::ArrayLiteral(_)
        ) && unsupported_array_projection_sources.contains(&occurrence.expression)
        {
            return Err(Diagnostic::error(
                "array constant projection currently requires unborrowed literal integer selectors; dynamic indexing, borrowing and slicing require value-based array projection"
            ).with_source_span(reference));
        }
        let initializer = if matches!(
            program.tables.bodies.expressions.expression(*initializer),
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::Name(_)
        ) {
            program
                .tables
                .bodies
                .expressions
                .copy_from_self(*initializer)
        } else {
            *initializer
        };
        let initializer_selections = program
            .tables
            .bodies
            .expressions
            .authored_selection_occurrences(initializer)
            .collect::<Vec<_>>();
        program
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(occurrence.expression, initializer_selections);
        let value = program
            .tables
            .bodies
            .expressions
            .expression(initializer)
            .clone();
        *program
            .tables
            .bodies
            .expressions
            .expression_mut(occurrence.expression) = value;
        selections.push(crate::lowerer::PendingConstSelection {
            expression: occurrence.expression,
            source_span: reference,
            declaration_ordinal,
            exposure: occurrence.exposure,
        });
    }
    Ok(())
}

pub(crate) fn semantic_const_name(definition: &ConstDefinition) -> String {
    if definition.scope.as_str().is_empty() {
        definition.name.as_str().to_owned()
    } else {
        format!(
            "{}::{}",
            definition.scope.as_str(),
            definition.name.as_str()
        )
    }
}

/// Attach the authored const selection to the substituted expression. The
/// value remains fully erased; only declaration custody survives.
pub(crate) fn finalize_const_selections(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::lowerer::PendingConstSelection],
) -> Result<(), Diagnostic> {
    let const_symbols = program
        .symbols
        .child_handles(program.symbols.root())
        .into_iter()
        .flatten()
        .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Const)
        .collect::<Vec<_>>();

    for selection in pending {
        let Some(symbol) = const_symbols.get(selection.declaration_ordinal).copied() else {
            return Err(Diagnostic::error(
                "failed to retain const declaration selection provenance",
            ));
        };
        carrier::retain_declared_carrier(
            program,
            selection.expression,
            symbol,
            selection.source_span,
        )?;
        let occurrence = program
            .record_resolved_authored_declaration_selection(
                selection.source_span,
                selection.exposure,
                AuthoredDeclarationSelectionKind::StaticPathSegment,
                symbol,
            )
            .map_err(const_selection_record_diagnostic)?;
        program
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(selection.expression, [occurrence]);
    }

    Ok(())
}

/// Bind retained const declarations to the symbols minted from the parallel
/// pending declaration list. Value substitution is independent of this root:
/// only source identity and visibility survive here.
pub(crate) fn finalize_const_declarations(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::lowerer::PendingConstDeclaration],
) -> Result<(), Diagnostic> {
    let const_symbols = program
        .symbols
        .child_handles(program.symbols.root())
        .into_iter()
        .flatten()
        .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Const)
        .collect::<Vec<_>>();
    if const_symbols.len() != pending.len()
        || program.roots.const_declarations.len() != pending.len()
    {
        return Err(Diagnostic::error(
            "failed to retain const declaration visibility provenance",
        ));
    }
    // Scoped module constants select their carrier independently of scalar
    // substitution or structural value encoding. Retain that authored selection
    // before erasing the scope token; a constant's visibility cannot authorize
    // naming its private carrier. Foreign and generic attachments still need
    // their complete normalization owners.
    for (declaration, constant_symbol) in pending.iter().zip(&const_symbols) {
        let module = program.symbols.symbol_module(*constant_symbol);
        if declaration.scope.as_str().is_empty() || !module.is_valid() {
            continue;
        }
        let carrier = program
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                declaration.scope.as_str(),
                &[SymbolKind::Data],
                declaration.scope.source_span(),
            );
        let Some(carrier) = carrier.filter(|carrier| {
            program.symbols.symbol_module(*carrier) == module
                && program
                    .symbols
                    .same_symbol_source_package(*carrier, *constant_symbol)
                && program.data_definitions.iter().any(|definition| {
                    definition.symbol == *carrier && definition.type_parameters.is_empty()
                })
        }) else {
            return Err(Diagnostic::error(
                "module type-scoped constants require an exact nongeneric data carrier in their declaring module",
            ).with_source_span(declaration.scope.source_span()));
        };
        let exposure = if declaration.is_public {
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
        };
        program
            .record_resolved_authored_declaration_selection(
                declaration.scope.source_span(),
                exposure,
                AuthoredDeclarationSelectionKind::TypeReference,
                carrier,
            )
            .map_err(const_selection_record_diagnostic)?;
    }
    let mut ordinal = 0usize;
    let mut visibility_drifted = false;
    program
        .roots
        .const_declarations
        .for_each_mut(|declaration| {
            declaration.symbol = const_symbols[ordinal];
            visibility_drifted |= declaration.is_public != pending[ordinal].is_public;
            ordinal += 1;
        });
    if visibility_drifted {
        return Err(Diagnostic::error(
            "const declaration visibility drifted before symbol assignment",
        ));
    }
    Ok(())
}

fn const_selection_record_diagnostic(error: AuthoredDeclarationSelectionRecordError) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to retain const declaration selection: {error:?}"
    ))
}

/// Named origins already retain their selected declaration's value. Direct
/// structured atoms instead require the original constructor expression, even
/// when a malformed input clears its handle.
pub(crate) fn normalization_requires_expression(
    normalization: &syntax_trees::types::ConstArgumentNormalization,
) -> bool {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    normalization.authored_expression.is_valid()
        || (normalization.selections.is_empty()
            && matches!(
                CanonicalConstValue::new("", &normalization.canonical_result_encoding, "")
                    .decode_encoding(),
                Some(
                    DecodedCanonicalConstValue::Array { .. }
                        | DecodedCanonicalConstValue::Record { .. }
                        | DecodedCanonicalConstValue::Variant { .. }
                )
            ))
}

pub(crate) fn validate_normalized_expression(
    syntax: &syntax_trees::SyntaxTrees,
    normalization: &syntax_trees::types::ConstArgumentNormalization,
) -> Result<(), Diagnostic> {
    if normalization_requires_expression(normalization)
        && (!syntax
            .expressions
            .contains_expression(normalization.authored_expression)
            || syntax
                .expressions
                .source_span(normalization.authored_expression)
                != normalization.reference)
    {
        return Err(Diagnostic::error(
            "direct constant argument lost its exact authored expression",
        )
        .with_source_span(normalization.reference));
    }
    Ok(())
}

/// Check the rewritten payload against its captured canonical value before
/// binding the selected declaration to a final symbol.
pub(crate) fn validate_normalized_const_argument(
    argument: &syntax_trees::types::TypeReferenceNode,
    normalization: &syntax_trees::types::ConstArgumentNormalization,
) -> Result<(), Diagnostic> {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    let encoded = CanonicalConstValue::new("", &normalization.canonical_result_encoding, "");
    let matches = match (argument, encoded.decode_encoding()) {
        (
            syntax_trees::types::TypeReferenceNode::Named(name),
            Some(DecodedCanonicalConstValue::Integer { value, .. }),
        ) => name.as_str() == value.to_string(),
        (syntax_trees::types::TypeReferenceNode::Named(name), Some(decoded)) => {
            CanonicalConstValue::from_atom(name.as_str()).is_some_and(|value| {
                let carrier = match &decoded {
                    DecodedCanonicalConstValue::Boolean(_) => "bool",
                    DecodedCanonicalConstValue::Array { type_name, .. }
                    | DecodedCanonicalConstValue::Record { type_name, .. }
                    | DecodedCanonicalConstValue::Variant { type_name, .. }
                    | DecodedCanonicalConstValue::Integer { type_name, .. } => type_name.as_str(),
                };
                value.encoding == normalization.canonical_result_encoding
                    && value.type_name == carrier
                    && value.decode_encoding() == Some(decoded)
            })
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(Diagnostic::error(
            "normalized constant argument value drifted from its retained canonical result",
        )
        .with_source_span(normalization.reference))
    }
}

/// Bind exact selected declaration custody after ordinary symbol allocation.
pub(crate) fn finalize_const_argument_selections(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::lowerer::PendingConstArgumentSelection],
    slots: &[crate::lowerer::PendingConstArgumentSlot],
) -> Result<(), Diagnostic> {
    for (selection_ordinal, selection) in pending.iter().enumerate() {
        let origin = &selection.origin;
        let mut declarations = program.const_declarations.iter().filter(|declaration| {
            program.symbols.symbol_source_span(declaration.symbol) == Some(origin.declaration)
        });
        let declaration = declarations.next().ok_or_else(|| {
            Diagnostic::error("normalized constant argument lost its exact selected declaration")
                .with_source_span(origin.reference)
        })?;
        if declarations.next().is_some()
            || declaration.initializer_source_span != origin.initializer
            || declaration.canonical_value_encoding.as_ref()
                != Some(&origin.canonical_value_encoding)
        {
            return Err(Diagnostic::error(
                "normalized constant argument declaration or value custody drifted before resolution",
            ).with_source_span(origin.reference));
        }
        let requires_nominal_slot =
            carrier::has_nominal_carrier(program, &declaration.declared_type)
                || carrier::encoding_has_nominal_carrier(&origin.canonical_value_encoding);
        let mut matched = false;
        for slot in slots
            .iter()
            .filter(|slot| slot.selection == selection_ordinal)
        {
            let parameters =
                carrier::receiving_parameters(program, slot.arguments).ok_or_else(|| {
                    Diagnostic::error("constant lost its actual receiving application")
                        .with_source_span(origin.reference)
                })?;
            let arguments = program.child_type_references(slot.arguments);
            if parameters.len() != arguments.len() {
                return Err(Diagnostic::error(
                    "constant receiving application has mismatched argument arity",
                )
                .with_source_span(origin.reference));
            }
            let argument = arguments.get(slot.ordinal).ok_or_else(|| {
                Diagnostic::error("constant lost its actual receiving argument")
                    .with_source_span(origin.reference)
            })?;
            let parameter = parameters.get(slot.ordinal).ok_or_else(|| {
                Diagnostic::error("constant lost its receiving generic slot")
                    .with_source_span(origin.reference)
            })?;
            let symbol_resolved_trees::data::TypeParameterKind::Const { type_reference } =
                &parameter.kind
            else {
                return Err(
                    Diagnostic::error("constant did not enter a const parameter")
                        .with_source_span(origin.reference),
                );
            };
            if requires_nominal_slot || carrier::has_nominal_carrier(program, type_reference) {
                if !carrier::same_resolved_carrier(
                    program,
                    &declaration.declared_type,
                    type_reference,
                ) {
                    return Err(Diagnostic::error("selected constant nominal carrier differs from its receiving generic parameter").with_source_span(origin.reference));
                }
                let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = argument
                else {
                    return Err(Diagnostic::error(
                        "nominal constant receiving argument lost its canonical value",
                    )
                    .with_source_span(origin.reference));
                };
                if symbol.is_valid()
                    || !language_semantics::const_value::CanonicalConstValue::from_atom(
                        name.as_str(),
                    )
                    .is_some_and(|value| value.encoding == origin.canonical_value_encoding)
                {
                    return Err(Diagnostic::error(
                        "nominal constant receiving argument differs from its selected value",
                    )
                    .with_source_span(origin.reference));
                }
            }
            matched = true;
        }
        if requires_nominal_slot && !matched {
            return Err(Diagnostic::error(
                "nominal constant argument has no retained receiving generic slot",
            )
            .with_source_span(origin.reference));
        }
        let selected = declaration.symbol;
        program
            .record_resolved_authored_declaration_selection(
                origin.reference,
                selection.exposure,
                AuthoredDeclarationSelectionKind::StaticPathSegment,
                selected,
            )
            .map_err(const_selection_record_diagnostic)?;
    }
    Ok(())
}

/// v0 initializers are literals all the way down. Payloadless case names are
/// nullary structural literals. The parser already folds `-5` into a single
/// literal, so no operator node is legitimate here.
fn validate_literal_initializer(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
    value: syntax_trees::expression::ExpressionHandle,
) -> Result<(), Diagnostic> {
    use syntax_trees::expression::ExpressionNode;
    match syntax_trees.expressions.expression(value) {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_) => Ok(()),
        ExpressionNode::ArrayLiteral(values) => {
            for element in syntax_trees.expressions.expression_handles(*values) {
                validate_literal_initializer(syntax_trees, definition, *element)?;
            }
            Ok(())
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in syntax_trees.expressions.struct_fields(literal.fields) {
                validate_literal_initializer(syntax_trees, definition, field.value)?;
            }
            Ok(())
        }
        ExpressionNode::Name(path) => {
            let members = syntax_trees.expressions.identifier_path_members(*path);
            let [type_name, case_name] = members else {
                return invalid_literal_initializer(syntax_trees, definition, value);
            };
            let is_payloadless_case = syntax_trees.root_items().any(|item| {
                let Item::Data(data) = item else {
                    return false;
                };
                data.name.as_str() == type_name.as_str()
                    && syntax_trees
                        .items
                        .data_members(data.members)
                        .iter()
                        .any(|member| {
                            matches!(
                                member,
                                DataMember::Variant(variant)
                                    if variant.name.as_str() == case_name.as_str()
                                        && variant.payload.is_empty()
                            )
                        })
            });
            if is_payloadless_case {
                Ok(())
            } else {
                invalid_literal_initializer(syntax_trees, definition, value)
            }
        }
        _ => invalid_literal_initializer(syntax_trees, definition, value),
    }
}

fn invalid_literal_initializer(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
    value: syntax_trees::expression::ExpressionHandle,
) -> Result<(), Diagnostic> {
    Err(Diagnostic::error(format!(
        "const `{}::{}` initializer must be a literal (a scalar, a payloadless \
         case, or a struct/array literal of literals) in const-v0; `{}` is not \
         -- richer const expressions arrive with build-time evaluation",
        definition.scope.as_str(),
        definition.name.as_str(),
        syntax_trees
            .expressions
            .expression(value)
            .display_name(&syntax_trees.expressions),
    )))
}

#[cfg(test)]
mod module_tests {
    use super::*;
    use source::SourceId;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::statement::StatementNode;
    use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

    fn resolve(sources: &[&str]) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
        let mut syntax = SyntaxTrees::default();
        for (ordinal, text) in sources.iter().enumerate() {
            let tokens = Lexer::new(text)
                .tokenize()
                .expect("tokenize module constants");
            parse_syntax_trees_into_with_id(&mut syntax, SourceId(ordinal), &tokens)
                .expect("parse module constants");
        }
        crate::lower_syntax_trees(&syntax)
    }

    #[test]
    fn public_float_identity_rejects_payloadless_nan() {
        for carrier in ["f32", "f64"] {
            let source = format!("pub const VALUE: {carrier} = 1.0;");
            let tokens = Lexer::new(&source)
                .tokenize()
                .expect("tokenize float constant");
            let mut syntax = SyntaxTrees::default();
            parse_syntax_trees_into_with_id(&mut syntax, SourceId(0), &tokens)
                .expect("parse float constant");
            let Item::Const(definition) =
                syntax.root_items().next().expect("one declaration").clone()
            else {
                panic!("expected one constant declaration");
            };
            // Evaluator-produced NaN meaning carries no selected payload bits.
            syntax.expressions.replace_expression(
                definition.value,
                syntax_trees::expression::ExpressionNode::Float(source::SourceText::new(
                    "NaN",
                    definition.name.source_span(),
                )),
            );
            let error = public_declaration_value_encoding(&syntax, &definition, None)
                .expect_err("payloadless NaN cannot acquire public representation identity");
            assert!(
                error.contains("explicit NaN representation bits"),
                "{error}"
            );
        }
    }

    #[test]
    fn unused_scalar_declarations_check_landing_before_substitution() {
        for namespace in ["", "module settings;"] {
            for (carrier, value, accepted) in [
                ("u8", "256", false),
                ("u8", "1u64", false),
                ("bool", "1", false),
                ("u64", "true", false),
                ("f32", "1.0f64", false),
                ("f32", "1u32", false),
                ("u8", "255", true),
                ("i8", "-128", true),
                ("bool", "true", true),
                ("f32", "1.0f32", true),
                ("f64", "1", true),
            ] {
                let source = format!("{namespace} const VALUE: {carrier} = {value};");
                let result = resolve(&[&source]);
                assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
            }
        }
    }

    #[test]
    fn unused_array_declarations_check_shape_and_landing() {
        for namespace in ["", "module settings;"] {
            for visibility in ["", "pub "] {
                for (carrier, value, accepted) in [
                    ("[u8; 2]", "[1]", false),
                    ("[u8; 1]", "[256]", false),
                    ("[u8; 1]", "[1u64]", false),
                    ("[bool; 1]", "[1]", false),
                    ("[[u8; 2]; 1]", "[[1]]", false),
                    ("[u8; 2]", "[0, 255]", true),
                    ("[[bool; 2]; 1]", "[[true, false]]", true),
                    ("[u8; 0]", "[]", true),
                ] {
                    let source =
                        format!("{namespace} {visibility}const VALUE: {carrier} = {value};");
                    let result = resolve(&[&source]);
                    assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
                }
            }
        }
    }

    fn local_value(
        program: &SymbolResolvedTrees,
        machine_name: &str,
        local_name: &str,
    ) -> ExpressionHandle {
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("constant test machine");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == local_name => {
                    Some(local.initial_value)
                }
                _ => None,
            })
            .expect("constant test local")
    }

    #[test]
    fn scoped_array_attachment_uses_the_exact_module_across_sources() {
        let sources = [
            "data Sizes { case SIZE; }",
            "module settings; data Sizes {}",
            "module settings; const Sizes::SIZE: [u8; 1] = [1];",
        ];
        for ordered in [sources, [sources[2], sources[1], sources[0]]] {
            let program = resolve(&ordered).expect("one exact carrier across module sources");
            let selection = program.authored_declaration_selections().iter().find(|selection| {
                matches!(selection.target(), language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if program.symbols.display_path(target.selected_symbol(), "::") == "settings::Sizes")
            }).expect("attachment selects the module carrier");
            let text = ordered[selection.source_span().source_id.0];
            let span = selection.source_span().span;
            assert_eq!(&text[span.start..span.end], "Sizes");
        }
        resolve(&[
            "module settings; data Sizes {} const Sizes::SIZE: [u8; 1] = [1];",
            "module settings; const Sizes::SIZE: [u8; 1] = [1];",
        ])
        .expect_err("duplicate scoped declarations across sources reject");
    }

    #[test]
    fn scoped_array_attachment_cannot_borrow_another_module_or_generic_owner() {
        for (sources, expected) in [
            (
                vec![
                    "module other; data Sizes {}",
                    "module settings; use other::Sizes; const Sizes::SIZE: [u8; 1] = [1];",
                ],
                "exact nongeneric data carrier",
            ),
            (
                vec!["module settings; data Sizes<T> {} const Sizes::SIZE: [u8; 1] = [1];"],
                "namespace-aware template normalization",
            ),
        ] {
            let diagnostics =
                resolve(&sources).expect_err("attachment normalization is incomplete");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{diagnostics:?}"
            );
        }
    }

    #[test]
    fn module_constants_select_exact_local_qualified_and_imported_values() {
        let program = resolve(&[
            "module combat; pub const DAMAGE: u64 = 7; machine combat_value() -> u64 { let observed: u64 = DAMAGE; observed }",
            "module rooms; pub const DAMAGE: u64 = 9; machine room_value() -> u64 { let observed: u64 = DAMAGE; observed }",
            "use combat::DAMAGE; use rooms; machine imported() -> u64 { let observed: u64 = DAMAGE; observed } machine qualified() -> u64 { let observed: u64 = rooms::DAMAGE; observed }",
        ]).expect("resolve distinct module constants");
        for (machine, expected) in [
            ("combat_value", 7),
            ("room_value", 9),
            ("imported", 7),
            ("qualified", 9),
        ] {
            let expression = local_value(&program, machine, "observed");
            let ExpressionNode::Integer(value) =
                program.tables.bodies.expressions.expression(expression)
            else {
                panic!("substituted scalar constant");
            };
            assert_eq!(value.value_u64(), Some(expected));
            assert_eq!(
                program
                    .tables
                    .bodies
                    .expressions
                    .authored_selection_occurrences(expression)
                    .count(),
                1
            );
        }
        let declarations = program
            .roots
            .const_declarations
            .iter()
            .map(|declaration| program.symbols.display_path(declaration.symbol, "::"))
            .collect::<Vec<_>>();
        assert_eq!(declarations, ["combat::DAMAGE", "rooms::DAMAGE"]);
    }

    #[test]
    fn module_constant_yields_to_state_parameter_and_preceding_local() {
        let program = resolve(&["module combat; const DAMAGE: u64 = 7;
            machine parameter(DAMAGE: u64) -> u64 { let observed: u64 = DAMAGE; observed }
            machine local() -> u64 { let before: u64 = DAMAGE; let DAMAGE: u64 = 12; let after: u64 = DAMAGE; after }
            machine initialize() -> u64 { let DAMAGE: u64 = DAMAGE; let observed: u64 = DAMAGE; observed }"]).expect("lexical bindings shadow constants");
        for (machine, local) in [
            ("parameter", "observed"),
            ("local", "after"),
            ("initialize", "observed"),
        ] {
            let expression = local_value(&program, machine, local);
            let ExpressionNode::Name(path) =
                program.tables.bodies.expressions.expression(expression)
            else {
                panic!("lexical value remains a Name");
            };
            assert!(matches!(
                program.symbols.get(path.symbol).kind,
                SymbolKind::Local | SymbolKind::Parameter
            ));
        }
        let ExpressionNode::Integer(value) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "local", "before"))
        else {
            panic!("earlier constant use");
        };
        assert_eq!(value.value_u64(), Some(7));
        let ExpressionNode::Integer(value) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "initialize", "DAMAGE"))
        else {
            panic!("initializer selects preceding constant");
        };
        assert_eq!(value.value_u64(), Some(7));
    }

    #[test]
    fn module_constant_yields_to_parser_generated_inferred_local() {
        let tokens = Lexer::new("module combat; const DAMAGE: u64 = 7; machine inferred() -> u64 { let DAMAGE: u64 = 12; let observed: u64 = DAMAGE; observed }")
            .tokenize().expect("tokenize inferred-local control");
        let mut syntax = SyntaxTrees::default();
        parse_syntax_trees_into_with_id(&mut syntax, SourceId(0), &tokens)
            .expect("parse inferred-local control");
        let machine = syntax
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .expect("inferred machine");
        let state = syntax
            .items
            .state(syntax.items.state_handles(machine.states)[0]);
        let statement = syntax.items.statements(state.statements)[0];
        let syntax_trees::statement::StatementNode::LocalData(mut local) =
            syntax.statements.statement(statement).clone()
        else {
            panic!("first local");
        };
        // Parser-generated destructuring locals use this same absent-type
        // representation; no new inferred-let source syntax is introduced.
        local.type_reference = syntax_trees::types::TypeReferenceHandle::invalid();
        syntax.statements.replace_statement(
            statement,
            syntax_trees::statement::StatementNode::LocalData(local),
        );
        let program = crate::lower_syntax_trees(&syntax).expect("resolve inferred local shadow");
        let ExpressionNode::Name(path) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "inferred", "observed"))
        else {
            panic!("inferred local reference remains lexical");
        };
        assert_eq!(program.symbols.get(path.symbol).kind, SymbolKind::Local);
    }

    #[test]
    fn same_module_duplicate_constants_reject_across_files() {
        let errors = resolve(&[
            "module combat; const DAMAGE: u64 = 7;",
            "module combat; const DAMAGE: u64 = 9;",
        ])
        .expect_err("duplicate exact module constant");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("duplicate const"))
        );
    }

    #[test]
    fn qualified_nominal_constants_keep_declaring_constructors_and_fresh_children() {
        let mut program = resolve(&[
            "module settings; pub data Leaf [copy] { count: u64; } pub data Value [copy] { leaf: Leaf; enabled: bool; }
             pub const VALUE: Value = Value { enabled: true, leaf: Leaf { count: 1 } };",
            "use settings; data Leaf [copy] { count: u64; } data Value [copy] { leaf: Leaf; enabled: bool; }
             machine keep() -> settings::Value {
                let first: settings::Value = settings::VALUE;
                let second: settings::Value = settings::VALUE;
                second
             }",
        ]).expect("qualified nominal body copies");
        let first = local_value(&program, "keep", "first");
        let second = local_value(&program, "keep", "second");
        let ExpressionNode::StructLiteral(first_literal) =
            program.tables.bodies.expressions.expression(first).clone()
        else {
            panic!("first nominal value");
        };
        let ExpressionNode::StructLiteral(second_literal) =
            program.tables.bodies.expressions.expression(second).clone()
        else {
            panic!("second nominal value");
        };
        assert_eq!(
            program
                .symbols
                .display_path(first_literal.type_symbol, "::"),
            "settings::Value"
        );
        assert_eq!(first_literal.type_symbol, second_literal.type_symbol);
        assert_ne!(first_literal.fields, second_literal.fields);
        let first_fields = program
            .tables
            .bodies
            .expressions
            .struct_fields(first_literal.fields)
            .to_vec();
        let second_fields = program
            .tables
            .bodies
            .expressions
            .struct_fields(second_literal.fields)
            .to_vec();
        for (first_field, second_field) in first_fields.iter().zip(&second_fields) {
            assert_eq!(first_field.field_symbol, second_field.field_symbol);
            assert!(first_field.field_symbol.is_valid());
            assert_ne!(first_field.value, second_field.value);
        }
        let ExpressionNode::StructLiteral(leaf) = program
            .tables
            .bodies
            .expressions
            .expression(first_fields[1].value)
        else {
            panic!("nested leaf");
        };
        assert_eq!(
            program.symbols.display_path(leaf.type_symbol, "::"),
            "settings::Leaf"
        );
        let unchanged = program
            .tables
            .bodies
            .expressions
            .expression(second_fields[0].value)
            .clone();
        *program
            .tables
            .bodies
            .expressions
            .expression_mut(first_fields[0].value) = ExpressionNode::Boolean(false);
        assert_eq!(
            program
                .tables
                .bodies
                .expressions
                .expression(second_fields[0].value),
            &unchanged
        );
        for root in [first, second] {
            let kinds = program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(root)
                .map(|occurrence| {
                    program
                        .authored_declaration_selections()
                        .iter()
                        .find(|selection| selection.occurrence_id() == occurrence)
                        .expect("retained selection")
                        .kind()
                })
                .collect::<Vec<_>>();
            assert!(kinds.contains(&AuthoredDeclarationSelectionKind::StructLiteralType));
            assert!(kinds.contains(&AuthoredDeclarationSelectionKind::StructLiteralField));
            let occurrences = program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(root)
                .map(|occurrence| {
                    program
                        .authored_declaration_selections()
                        .get(occurrence)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let constant_uses = occurrences.iter().filter(|selection| {
                let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else { return false; };
                program.symbols.display_path(target.selected_symbol(), "::") == "settings::VALUE"
            }).collect::<Vec<_>>();
            assert_eq!(constant_uses.len(), 1);
            assert_eq!(
                constant_uses[0].kind(),
                AuthoredDeclarationSelectionKind::StaticPathSegment
            );
            assert_eq!(constant_uses[0].source_span().source_id, SourceId(1));
            for selection in occurrences.iter().filter(|selection| {
                matches!(
                    selection.kind(),
                    AuthoredDeclarationSelectionKind::StructLiteralType
                        | AuthoredDeclarationSelectionKind::StructLiteralField
                )
            }) {
                assert_eq!(selection.source_span().source_id, SourceId(0));
                let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else { panic!("declaring constructor selection resolved"); };
                assert!(
                    program
                        .symbols
                        .display_path(target.selected_symbol(), "::")
                        .starts_with("settings::Value")
                );
            }
        }
    }

    #[test]
    fn root_nominal_constants_share_lexical_selection_and_fresh_materialization() {
        let program = resolve(&["data Value [copy] { value:u64; }
            const VALUE:Value = Value { value:7 };
            machine keep()->Value {
                let before:Value = VALUE;
                let VALUE:Value = VALUE;
                let after:Value = VALUE;
                after
            }"])
        .expect("root nominal values use ordinary lexical selection");
        let before = local_value(&program, "keep", "before");
        let initializer = local_value(&program, "keep", "VALUE");
        let after = local_value(&program, "keep", "after");
        let ExpressionNode::StructLiteral(first) =
            program.tables.bodies.expressions.expression(before)
        else {
            panic!("earlier constant use");
        };
        let ExpressionNode::StructLiteral(second) =
            program.tables.bodies.expressions.expression(initializer)
        else {
            panic!("self initializer selects prior constant");
        };
        assert_eq!(first.type_symbol, second.type_symbol);
        assert_ne!(first.fields, second.fields);
        let ExpressionNode::Name(local) = program.tables.bodies.expressions.expression(after)
        else {
            panic!("later use selects local");
        };
        assert_eq!(program.symbols.get(local.symbol).kind, SymbolKind::Local);
    }

    #[test]
    fn ordinary_module_presence_keeps_root_aggregate_substitution() {
        let program = resolve(&[
            "module other; data Marker {}",
            "data Pair { value: u64; } const PAIR: Pair = Pair { value: 7 }; machine read() -> Pair { let observed: Pair = PAIR; observed }",
        ]).expect("unrelated module declarations do not change aggregate substitution");
        assert!(matches!(
            program
                .tables
                .bodies
                .expressions
                .expression(local_value(&program, "read", "observed")),
            ExpressionNode::StructLiteral(_)
        ));
    }

    #[test]
    fn root_scalar_lexical_selection_coexists_with_aggregate_materialization() {
        let program = resolve(&["data Pair { value: u64; }
            const PAIR: Pair = Pair { value: 11 };
            machine read() -> u64 {
                let aggregate: Pair = PAIR;
                let before: u64 = SIZE;
                let SIZE: u64 = SIZE;
                let after: u64 = SIZE;
                after
            }
            machine parameter(SIZE: u64) -> u64 { let observed: u64 = SIZE; observed }
            const SIZE: u64 = 7;"])
        .expect("forward scalar selection preserves lexical scope beside an aggregate");
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "read",
                "aggregate"
            )),
            ExpressionNode::StructLiteral(_)
        ));
        for local in ["before", "SIZE"] {
            let expression = local_value(&program, "read", local);
            let ExpressionNode::Integer(value) =
                program.tables.bodies.expressions.expression(expression)
            else {
                panic!("earlier use and self-initializer select the forward constant");
            };
            assert_eq!(value.value_u64(), Some(7));
            assert_eq!(
                program
                    .tables
                    .bodies
                    .expressions
                    .authored_selection_occurrences(expression)
                    .count(),
                1
            );
        }
        for (machine, local, kind) in [
            ("read", "after", SymbolKind::Local),
            ("parameter", "observed", SymbolKind::Parameter),
        ] {
            let expression = local_value(&program, machine, local);
            let ExpressionNode::Name(path) =
                program.tables.bodies.expressions.expression(expression)
            else {
                panic!("actual lexical binding remains a name");
            };
            assert_eq!(program.symbols.get(path.symbol).kind, kind);
        }
    }

    #[test]
    fn root_scalar_and_explicit_receiver_field_remain_distinct() {
        let program = resolve(&["const SIZE: u64 = 7; data Main { SIZE: u64; }
            machine Main::read(&self) -> u64 { let field: u64 = self.SIZE; let constant: u64 = SIZE; constant }"])
            .expect("bare fields do not alias explicit receiver projections");
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "Main::read",
                "field"
            )),
            ExpressionNode::Member(_)
        ));
        let ExpressionNode::Integer(value) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "Main::read", "constant"))
        else {
            panic!("bare spelling selects the constant");
        };
        assert_eq!(value.value_u64(), Some(7));
    }

    #[test]
    fn explicit_receiver_field_is_not_a_module_constant_reference() {
        let program = resolve(&[
            "module combat; const DAMAGE: u64 = 7; data Main { DAMAGE: u64; }
             machine Main::field(&self) -> u64 { let observed: u64 = self.DAMAGE; observed }
             machine Main::value(&self) -> u64 { let observed: u64 = DAMAGE; observed }",
        ])
        .expect("explicit receiver projection and bare constant have distinct meanings");
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "Main::field",
                "observed"
            )),
            ExpressionNode::Member(_)
        ));
        let ExpressionNode::Integer(value) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "Main::value", "observed"))
        else {
            panic!("bare spelling selects module constant, not implicit field");
        };
        assert_eq!(value.value_u64(), Some(7));
    }

    #[test]
    fn conformance_parameter_cannot_be_replaced_by_module_constant() {
        let program = resolve(&[
            "trait Ranked {}",
            "module combat; const Order: u64 = 7; machine sort<Element, Order: Element satisfies Ranked>(values: &mut [Element]) -> u64 { let observed: u64 = Order; observed }",
        ]).expect("resolution preserves the proof-static machine binder");
        let ExpressionNode::Name(path) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "sort", "observed"))
        else {
            panic!("conformance binder is not a constant value");
        };
        assert_eq!(
            program.symbols.get(path.symbol).kind,
            SymbolKind::ConformanceParameter
        );
    }

    #[test]
    fn module_scalar_substitution_preserves_boolean_float_and_text_kinds() {
        let program = resolve(&[
            "module boolean; const VALUE: bool = true; machine boolean_value() -> bool { let observed: bool = VALUE; observed }",
            "module floating; const VALUE: f64 = 1.5; machine float_value() -> f64 { let observed: f64 = VALUE; observed }",
            "module text; const VALUE: string = \"value\"; machine text_value() -> string { let observed: string = VALUE; observed }",
        ]).expect("primitive literal kinds retain their module selections");
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "boolean_value",
                "observed"
            )),
            ExpressionNode::Boolean(true)
        ));
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "float_value",
                "observed"
            )),
            ExpressionNode::Float(_)
        ));
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "text_value",
                "observed"
            )),
            ExpressionNode::String(_)
        ));
    }

    #[test]
    fn root_case_collisions_still_reject_in_module_constant_closures() {
        let errors = resolve(&[
            "module combat; const DAMAGE: u64 = 7;",
            "data Choice { case None; } const Choice::None: u64 = 1;",
        ])
        .expect_err("const cannot shadow an exact case constructor");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("collides with the case"))
        );
    }

    #[test]
    fn colliding_imported_constant_leaves_do_not_choose_traversal_order() {
        let program = resolve(&[
            "module combat; pub const DAMAGE: u64 = 7;",
            "module rooms; pub const DAMAGE: u64 = 9;",
            "use combat::DAMAGE; use rooms::DAMAGE; machine ambiguous() -> u64 { let observed: u64 = DAMAGE; observed }",
        ]).expect("resolution preserves an unresolved ambiguous value for checking");
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "ambiguous",
                "observed"
            )),
            ExpressionNode::Name(_)
        ));
    }

    fn resolve_seeded(
        extension: &str,
    ) -> Result<crate::lowerer::SeededSymbolResolvedTrees, Vec<Diagnostic>> {
        use std::{path::PathBuf, sync::Arc};
        let mut sources = source::SourceMap::default();
        let base_text = "module combat; pub const DAMAGE: u64 = 7;";
        let base_source = sources
            .add(PathBuf::from("combat.omg"), base_text.to_owned())
            .source_id;
        let tokens = Lexer::new(base_text)
            .tokenize()
            .expect("tokenize retained constant");
        let mut syntax = SyntaxTrees::default();
        parse_syntax_trees_into_with_id(&mut syntax, base_source, &tokens)
            .expect("parse retained constant");
        let base = crate::lower_syntax_trees_with_sources(&syntax, Arc::new(sources.clone()))
            .expect("resolve retained constant");
        let extension_source = sources
            .add(PathBuf::from("extension.omg"), extension.to_owned())
            .source_id;
        let tokens = Lexer::new(extension)
            .tokenize()
            .expect("tokenize constant extension");
        let mut syntax = SyntaxTrees::default();
        parse_syntax_trees_into_with_id(&mut syntax, extension_source, &tokens)
            .expect("parse constant extension");
        crate::lowerer::lower_syntax_extension_with_authored_selection_frontier(
            base,
            &syntax,
            Arc::new(sources),
            Vec::new(),
        )
    }

    #[test]
    fn seeded_module_constant_references_identify_missing_retained_value() {
        let errors = resolve_seeded(
            "machine read() -> u64 { let observed: u64 = combat::DAMAGE; observed }",
        )
        .expect_err("retained base has no value initializer");
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("seeded constant references require retained initializer substitution")
        }));
    }

    #[test]
    fn seeded_raw_constant_spelling_cannot_erase_module_ambiguity() {
        let extension = resolve_seeded("const combat::DAMAGE: u64 = 9; machine read() -> u64 { let observed: u64 = combat::DAMAGE; observed }").expect("ambiguous path remains unresolved");
        let program = extension.trees();
        assert!(matches!(
            program
                .tables
                .bodies
                .expressions
                .expression(local_value(program, "read", "observed")),
            ExpressionNode::Name(_)
        ));
    }
}
