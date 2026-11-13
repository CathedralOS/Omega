/*
    Constant substitution and exact const-index declaration custody; value contract: wiki/spec/language/constants.md.

    Const VALUE semantics exist only until symbol resolution:
    A constant declares a named pure value; each selected expression path gets
    a fresh copy of its initializer before resolution publishes its result.
    Typed trees, validation, proofs, backends, and the interpreter never grow a
    const-value concept -- each use IS the literal, which is exactly the copied-at-each-use
    semantics the contract specifies (and why interior mutability can never hide in
    one). Resolved declarations keep a detached initializer handle so later
    source extensions can use the same substitution without reparsing or
    decoding review encodings. Only newly authored roots resolve names: retained
    roots keep their declaration-side selections. The symbol table preserves
    declaration provenance for authored-selection and package-authority checks.

    Scalar and closed aggregate literals substitute
    only after the shared resolver has selected their namespace and lexical
    binding. Detached initializer roots resolve constructors and fields in their
    declaring source. Each use deep-copies aggregate children and carries both
    that declaration-side selection custody and its own constant-use occurrence.
    Root and module declarations use this same path; no pre-resolution spelling
    substitution or whole-forest local-shadowing restriction is needed. Module-owned scoped declarations
    additionally select an exact nongeneric carrier under their declaring
    source's ordinary name law: a module-local carrier outranks imported and
    unmoduled candidates, and an exact import exposes a foreign-module carrier.
    The authored scope token survives until complete symbol assignment, then joins
    the ordinary visibility/selection ledger; the structural value encoder alone
    cannot establish attachment ownership. Seeded declarations keep their existing
    selections rather than inventing a new scope occurrence from a display name.
    Numeric and Boolean initializers owe their declared landing even when private
    and unused. Check declarations before substitution, reusing the same validator
    during module normalization so public identity construction cannot bypass it.

    Nominal index values retain a private link to the actual parent-owned
    argument span while lowering. Finalization rejoins the resolved parameter
    and selected declaration before publishing occurrence custody. The ordinary
    generic application and indexed-domain owners provide that relationship;
    Body materialization retains those same nominal declaration identities without
    turning canonical index encodings into constructor authority.

    Remaining boundaries, enforced loudly:
    - Complete resolution consumes literal materializations (scalars, payloadless
      cases, and struct/array literals). Call-free integer/Boolean computations
      reach that form through build-time evaluation, retaining declaration-owned
      original syntax and selection receipts. Initializer preparation is a
      separate non-executing mode: pending expressions acquire no value identity.
      Computed integer/Boolean leaves in arrays and selected nominal literals
      evaluate independently. Calls and floating computations still need evaluation.
    - A const may not collide with a case of its scope type: `Type::NAME` must
      stay unambiguous against case-constructor paths, which substitution
      would otherwise shadow.
    - Selected numeric values retain their declared landing recursively through
      arrays. Destination checking rejoins complete array declaration types from
      retained selections, so empty arrays cannot lose their element identity.

    This module decides which initializers need evaluation and finalizes
    constants. `initializer_leaves` collects pending initializer leaves
    and their selected destinations, `declaration_values` validates and
    retains declaration values, `substitution` substitutes resolved
    constants, `selections` finalizes const selections and arguments,
    `normalized_expressions` validates normalized expressions,
    `scalar_carriers` recognizes builtin scalar carrier spellings and
    `carrier`, `initializer_dependencies` and
    `initializer_normalization` carry the const carrier and initializer
    dependencies.
*/

mod carrier {
    /* Numeric substitution preserves the selected declaration's landing boundary. */

    use crate::tables::body_expressions;
    use diagnostics::Diagnostic;
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::literals::{FloatFormat, FloatLiteral, IntegerLanding, LandedIntegerType};
    use source::SourceSpan;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbol_resolved_trees::types::TypeReference;
    use symbols::{BuiltinTypeAtom, SymbolHandle};

    pub(super) fn retain_declared_carrier(
        program: &mut SymbolResolvedTrees,
        expression: ExpressionHandle,
        declaration_symbol: SymbolHandle,
        reference: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let declaration = program
            .const_declaration(declaration_symbol)
            .ok_or_else(|| Diagnostic::error("constant substitution lost its declared carrier"))?;
        let declared_type = declaration.declared_type.clone();
        retain_value_carrier(program, expression, &declared_type, reference)
    }

    fn retain_value_carrier(
        program: &mut SymbolResolvedTrees,
        expression: ExpressionHandle,
        declared_type: &TypeReference,
        reference: SourceSpan,
    ) -> Result<(), Diagnostic> {
        if let TypeReference::FixedArray(array) = declared_type {
            let ExpressionNode::ArrayLiteral(elements) =
                program.tables.bodies.expressions.expression(expression)
            else {
                return Ok(());
            };
            let elements = *elements;
            let element_type = program.child_type_reference(array.element_type).clone();
            for element_ordinal in 0..elements.count() {
                let element = *body_expressions(program)
                    .expression_handle_at_offset(elements, element_ordinal);
                retain_value_carrier(program, element, &element_type, reference)?;
            }
            return Ok(());
        }
        let TypeReference::Named { symbol, .. } = declared_type else {
            return Ok(());
        };
        let Some(atom) = program.symbols.builtin_type_atom(*symbol) else {
            return Ok(());
        };
        let integer_type = match atom {
            BuiltinTypeAtom::I8 => Some(LandedIntegerType::I8),
            BuiltinTypeAtom::I16 => Some(LandedIntegerType::I16),
            BuiltinTypeAtom::I32 => Some(LandedIntegerType::I32),
            BuiltinTypeAtom::I64 => Some(LandedIntegerType::I64),
            BuiltinTypeAtom::U8 => Some(LandedIntegerType::U8),
            BuiltinTypeAtom::U16 => Some(LandedIntegerType::U16),
            BuiltinTypeAtom::U32 => Some(LandedIntegerType::U32),
            BuiltinTypeAtom::U64 => Some(LandedIntegerType::U64),
            BuiltinTypeAtom::Address => Some(LandedIntegerType::Addr),
            _ => None,
        };
        let float_format = match atom {
            BuiltinTypeAtom::F32 => Some(FloatFormat::F32),
            BuiltinTypeAtom::F64 => Some(FloatFormat::F64),
            _ => None,
        };
        let mismatch = || {
            Diagnostic::error(
                "constant initializer landing conflicts with its declared numeric carrier",
            )
            .with_source_span(reference)
        };
        let node = program.tables.bodies.expressions.expression_mut(expression);
        match node {
            ExpressionNode::Integer(literal) => {
                if let Some(landed_type) = integer_type {
                    let landing = IntegerLanding {
                        landed_type,
                        domain: ArithmeticDomain::Exact,
                    };
                    if literal.landing().is_some_and(|actual| actual != landing) {
                        return Err(mismatch());
                    }
                    *literal = literal.with_landing(landing);
                } else if let Some(format) = float_format {
                    if literal.landing().is_some() {
                        return Err(mismatch());
                    }
                    let value = literal.value_bignum().ok_or_else(mismatch)?;
                    let float = FloatLiteral::parse(&value.to_string()).ok_or_else(mismatch)?;
                    *node = ExpressionNode::Float(float.with_landing(format));
                }
            }
            ExpressionNode::Float(literal) => {
                if integer_type.is_some() {
                    return Err(mismatch());
                }
                if let Some(format) = float_format {
                    if literal.landing().is_some_and(|actual| actual != format) {
                        return Err(mismatch());
                    }
                    *literal = literal.with_landing(format);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /* Nominal leaves require identity custody beyond their encoded value labels. */
    pub(super) fn has_nominal_carrier(
        program: &SymbolResolvedTrees,
        carrier: &TypeReference,
    ) -> bool {
        match carrier {
            TypeReference::Named { symbol, .. } => {
                program.symbols.get(*symbol).kind == symbols::SymbolKind::Data
            }
            TypeReference::FixedArray(array) => {
                has_nominal_carrier(program, program.child_type_reference(array.element_type))
            }
            TypeReference::Constrained(carrier) => {
                has_nominal_carrier(program, program.child_type_reference(carrier.base_type))
            }
            _ => false,
        }
    }

    /*
        Reconstruct equality from resolved declarations and array geometry, never
        from rendered type names or equal structural initializer encodings.
    */
    pub(super) fn same_resolved_carrier(
        program: &SymbolResolvedTrees,
        left: &TypeReference,
        right: &TypeReference,
    ) -> bool {
        match (left, right) {
            (
                TypeReference::Named { symbol: left, .. },
                TypeReference::Named { symbol: right, .. },
            ) => left.is_valid() && left == right,
            (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
                left.length == right.length
                    && same_resolved_carrier(
                        program,
                        program.child_type_reference(left.element_type),
                        program.child_type_reference(right.element_type),
                    )
            }
            _ => false,
        }
    }

    pub(super) fn encoding_has_nominal_carrier(encoding: &str) -> bool {
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        fn contains(value: &DecodedCanonicalConstValue) -> bool {
            match value {
                DecodedCanonicalConstValue::Record { .. }
                | DecodedCanonicalConstValue::Variant { .. } => true,
                DecodedCanonicalConstValue::Array { values, .. } => values.iter().any(contains),
                _ => false,
            }
        }
        CanonicalConstValue::new("", encoding, "")
            .decode_encoding()
            .is_some_and(|value| contains(&value))
    }

    /*
        Rejoin the parent-owned child span in actual resolved type owners. Copies
        of one application may share its span, but must agree on the resolved base.
        No encoded name or detached provisional application supplies this identity.
    */
    pub(super) fn receiving_parameters<'program>(
        program: &'program SymbolResolvedTrees,
        arguments: arena::HandleSpan<TypeReference>,
        lexical_inputs: &[crate::lowering::state::StateLexicalInputs],
    ) -> Option<&'program [symbol_resolved_trees::data::TypeParameter]> {
        let mut selected = SymbolHandle::invalid();
        let mut conflicting = false;
        let mut inspect = |reference: &TypeReference| {
            if let TypeReference::Generic(application) = reference
                && application.arguments == arguments
            {
                if !application.base_symbol.is_valid()
                    || (selected.is_valid() && selected != application.base_symbol)
                {
                    conflicting = true;
                }
                selected = application.base_symbol;
            }
        };
        let declarations = &program.tables.declarations;
        for (_, reference) in declarations.child_type_references.iter() {
            inspect(reference);
        }
        for declaration in program.const_declarations.iter() {
            inspect(&declaration.declared_type);
        }
        for (_, parameter) in declarations.state_parameters.iter() {
            inspect(&parameter.type_reference);
        }
        for (_, state) in declarations.machine_states.iter() {
            if let Some(reference) = &state.return_type {
                inspect(reference);
            }
        }
        for (_, member) in declarations.data_members.iter() {
            if let symbol_resolved_trees::data::DataMember::Field(field) = member {
                inspect(&field.type_reference);
            }
        }
        for (_, field) in declarations.data_payload_fields.iter() {
            inspect(&field.type_reference);
        }
        for (_, statement) in declarations.state_statements.iter() {
            if let symbol_resolved_trees::statement::Statement::LocalData(local) = statement {
                inspect(&local.type_reference);
            }
        }
        for input in lexical_inputs {
            for statement in &input.locals {
                if let symbol_resolved_trees::statement::Statement::LocalData(local) = statement {
                    inspect(&local.type_reference);
                }
            }
            for (_, reference) in &input.types {
                inspect(reference);
            }
        }
        for definition in program.data_definitions.iter() {
            if let Some(reference) = &definition.generic_instance {
                inspect(reference);
            }
        }
        for (_, constraint) in program.tables.types.constraints.iter() {
            if let symbol_resolved_trees::types::TypeConstraint::Domain(domain) = constraint
                && domain.arguments == arguments
            {
                let domain_symbol = program
                    .symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        domain.name.as_str(),
                        &[symbols::SymbolKind::Domain],
                        domain.name.source_span(),
                    )?;
                if selected.is_valid() && selected != domain_symbol {
                    conflicting = true;
                }
                selected = domain_symbol;
            }
        }
        if conflicting || !selected.is_valid() {
            return None;
        }
        if let Some(template) = program.data_definition_by_symbol(selected) {
            return Some(program.data_type_parameters(template.type_parameters));
        }
        let domain = program.domain_definition_by_symbol(selected)?;
        let parameters = program.data_type_parameters(domain.type_parameters);
        let carrier_binder = parameters.first().is_some_and(|parameter| matches!(parameter.kind, symbol_resolved_trees::data::TypeParameterKind::Type)
        && matches!(&domain.target_type, TypeReference::Named { symbol, .. } if *symbol == parameter.symbol));
        Some(if carrier_binder {
            &parameters[1..]
        } else {
            parameters
        })
    }
}
mod declaration_values {
    /* Public declaration value encodings and validated initializers. */

    use crate::constant::initializer_normalization;
    use crate::constant::substitution::semantic_const_name;
    use crate::lowering::expression::lower_expression_into_table;
    use crate::preparation::generic_data::canonicalize_declared_const_definition;
    use crate::preparation::generic_data::canonicalize_selected_declared_const_definition;
    use crate::preparation::generic_data::prove_const_literal_predicates;
    use crate::tables::syntax_expressions;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use diagnostics::Diagnostic;
    use language_core::constant_value::EvaluatedConstant;
    use numerics::literals::FloatFormat;
    use source::{SourceSpan, Span};
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::expression::ExpressionHandle;
    use symbols::JoinedPath;
    use symbols::SymbolKind;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::item::{ConstDefinition, DataMember};
    use syntax_trees::types::TypeConstraintNode;
    use syntax_trees::types::TypeReferenceNode;

    /*
        Even unused private constants owe their exact nominal destination. Check
        resolved constructor and field identities after specialization, without
        requiring a public/index encoding or duplicating scalar type checking.
    */
    pub(super) fn validate_nominal_destinations(
        program: &SymbolResolvedTrees,
    ) -> Result<(), Diagnostic> {
        use symbol_resolved_trees::data::DataMember;
        use symbol_resolved_trees::expression::ExpressionNode;
        use symbol_resolved_trees::types::TypeReference;

        let mut pending = program
            .const_declarations
            .iter()
            .map(|declaration| (&declaration.declared_type, declaration.initializer))
            .collect::<Vec<_>>();
        while let Some((destination, expression)) = pending.pop() {
            let table = &program.tables.bodies.expressions;
            if let TypeReference::Constrained(constraint) = destination {
                pending.push((
                    program.child_type_reference(constraint.base_type),
                    expression,
                ));
                continue;
            }
            if let (TypeReference::FixedArray(array), ExpressionNode::ArrayLiteral(elements)) =
                (destination, table.expression(expression))
            {
                pending.extend(
                    table.expression_handles(*elements).iter().map(|element| {
                        (program.child_type_reference(array.element_type), *element)
                    }),
                );
                continue;
            }
            let expected = match destination {
                TypeReference::Named { symbol, .. } => *symbol,
                TypeReference::Generic(application) if application.arguments.is_empty() => {
                    application.base_symbol
                }
                _ => continue,
            };
            let (actual, literal) = match table.expression(expression) {
                ExpressionNode::StructLiteral(literal) => (literal.type_symbol, Some(literal)),
                ExpressionNode::Name(path)
                    if program.symbols.get(path.symbol).kind == SymbolKind::Variant =>
                {
                    (program.symbols.get(path.symbol).parent, None)
                }
                _ => continue,
            };
            if !expected.is_valid() || !actual.is_valid() || expected != actual {
                return Err(Diagnostic::error(format!(
                "constant constructor selects a different nominal carrier than its declared destination `{}`",
                program.symbols.display_path(expected, "::"),
            )).with_source_span(table.source_span(expression)));
            }
            let Some(literal) = literal else {
                continue;
            };
            let Some(definition) = program.data_definition_by_symbol(expected) else {
                continue;
            };
            for field in table.struct_fields(literal.fields) {
                let declared = program
                    .data_members(definition.members)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(declared) if declared.symbol == field.field_symbol => {
                            Some(declared)
                        }
                        DataMember::Variant(variant)
                            if literal.case_symbol == Some(variant.symbol) =>
                        {
                            program
                                .data_payload_fields(variant.payload)
                                .iter()
                                .find(|declared| declared.symbol == field.field_symbol)
                        }
                        _ => None,
                    });
                if let Some(declared) = declared {
                    pending.push((&declared.type_reference, field.value));
                }
            }
        }
        Ok(())
    }

    /*
        Keep evaluation separate from byte demand. Only the explicit internal semantic
        node may carry a payloadless NaN; source float spelling still follows the ordinary
        encoder. The retained authored root is independently replayed after typing.
    */
    pub(crate) fn evaluated_declaration_value(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    ) -> Result<EvaluatedConstant, String> {
        use language_core::constant_value::EvaluatedConstant;
        if let Some(normalization) = &definition.normalization {
            return Ok(normalization.result_value.clone());
        }
        if let ExpressionNode::UndeterminedNaN(format) =
            syntax.expressions.expression(definition.value)
        {
            let mut carrier = definition.type_reference;
            while let TypeReferenceNode::Constrained { base_type, .. } =
                syntax.type_references.type_reference(carrier)
            {
                carrier = *base_type;
            }
            let Some(name) = syntax.type_references.type_reference(carrier).as_named() else {
                return Err(
                    "semantic NaN constant requires its exact declared floating carrier".into(),
                );
            };
            let actual = match name.as_str(syntax) {
                "f32" => FloatFormat::F32,
                "f64" => FloatFormat::F64,
                _ => {
                    return Err(
                        "semantic NaN constant requires its exact declared floating carrier".into(),
                    );
                }
            };
            if actual != *format {
                return Err(
                    "semantic NaN constant differs from its declared floating format".into(),
                );
            }
            return Ok(EvaluatedConstant::UndeterminedNaN(*format));
        }
        public_declaration_value_encoding(syntax, definition, selection)
            .map(EvaluatedConstant::Encoded)
    }

    /*
        Public declaration identity includes floating components with determined bits, independently
        of the narrower structural values eligible for generic and domain indices.
    */
    pub(crate) fn public_declaration_value_encoding(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    ) -> Result<String, String> {
        canonicalize_selected_declared_const_definition(syntax, definition, selection)
            .map(|value| value.encoding)
    }

    pub(crate) fn validate_scalar_initializer(
        syntax: &SyntaxTrees,
        constant: &syntax_trees::item::ConstDefinition,
    ) -> Result<(), String> {
        use numerics::literals::FloatFormat;
        use syntax_trees::expression::ExpressionNode;

        let Some(carrier) = syntax_type_references(syntax)
            .type_reference(constant.type_reference)
            .as_named()
        else {
            return Ok(());
        };
        if !crate::constant::is_builtin_scalar_carrier(carrier.as_str(syntax)) {
            return Ok(());
        }
        let initializer = syntax.expressions.expression(constant.value);
        match carrier.as_str(syntax) {
            "f32" | "f64" => {
                let format = if carrier.as_str(syntax) == "f32" {
                    FloatFormat::F32
                } else {
                    FloatFormat::F64
                };
                let compatible = match initializer {
                    ExpressionNode::UndeterminedNaN(actual) => *actual == format,
                    ExpressionNode::Float(text) => text.literal(syntax).is_some_and(|literal| {
                        literal.landing().is_none_or(|landing| landing == format)
                    }),
                    ExpressionNode::Integer(literal) => {
                        literal.landing().is_none() && literal.value_bignum().is_some()
                    }
                    _ => false,
                };
                if compatible {
                    Ok(())
                } else {
                    Err(format!(
                        "initializer conflicts with declared floating carrier `{carrier}`",
                        carrier = carrier.as_str(syntax)
                    ))
                }
            }
            _ => canonicalize_declared_const_definition(syntax, constant).map(|_| ()),
        }
    }

    /*
        Declaration validity precedes all use-site substitution, including unused
        private declarations. Namespace and case collisions are checked later against
        exact symbols; lexical locals do not make a constant declaration ambiguous.
    */
    pub(crate) fn validate_const_definition(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    ) -> Result<(), Diagnostic> {
        if definition.normalization.is_some() {
            return Ok(());
        }
        if definition.pending_target_observation {
            let supported = syntax_type_references(syntax)
                .type_reference(definition.type_reference)
                .as_named()
                .and_then(|name| selection.and_then(|selected| selected.builtin_type(syntax, name)))
                .is_some_and(|atom| {
                    matches!(
                        atom,
                        symbols::BuiltinTypeAtom::I8
                            | symbols::BuiltinTypeAtom::I16
                            | symbols::BuiltinTypeAtom::I32
                            | symbols::BuiltinTypeAtom::I64
                            | symbols::BuiltinTypeAtom::U8
                            | symbols::BuiltinTypeAtom::U16
                            | symbols::BuiltinTypeAtom::U32
                            | symbols::BuiltinTypeAtom::U64
                            | symbols::BuiltinTypeAtom::Bool
                            | symbols::BuiltinTypeAtom::F32
                            | symbols::BuiltinTypeAtom::F64
                    )
                });
            return if supported {
                Ok(())
            } else {
                Err(Diagnostic::error("target-dependent constant evaluation requires an unqualified builtin scalar destination")
                .with_source_span(definition.name.source_span()))
            };
        }
        if let TypeReferenceNode::Constrained { constraints, .. } =
            syntax_type_references(syntax).type_reference(definition.type_reference)
            && syntax_type_references(syntax)
                .constraints(*constraints)
                .iter()
                .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)))
        {
            canonicalize_selected_declared_const_definition(syntax, definition, selection)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "constrained const `{}` is invalid: {reason}",
                        semantic_const_name(syntax, definition)
                    ))
                    .with_source_span(definition.name.source_span())
                })?;
        } else {
            prove_const_literal_predicates(syntax, definition, selection).map_err(|reason| {
                Diagnostic::error(format!(
                    "constant `{}` is invalid: {reason}",
                    semantic_const_name(syntax, definition)
                ))
                .with_source_span(definition.name.source_span())
            })?;
        }
        validate_scalar_initializer(syntax, definition).map_err(|reason| {
            Diagnostic::error(format!(
                "scalar constant `{}` is invalid: {reason}",
                definition.name.as_str(syntax)
            ))
            .with_source_span(definition.name.source_span())
        })?;
        validate_literal_initializer(syntax, definition, definition.value)
    }

    pub(crate) fn retain_const_initializer(
        lowerer: &mut crate::resolution::lowerer::Lowerer,
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
    ) -> Result<ExpressionHandle, Diagnostic> {
        let pending = lowerer.const_resolution_mode
            != crate::resolution::lowerer::ConstResolutionMode::Complete;
        if !pending
            && definition.normalization.is_none()
            && !has_scalar_initializer(syntax, definition)
        {
            if crate::preparation::module_normalization::module_literal_constant(syntax, definition)
            {
                canonicalize_declared_const_definition(syntax, definition).map_err(|reason| {
                    Diagnostic::error(format!(
                        "array constant `{}` is invalid: {reason}",
                        semantic_const_name(syntax, definition)
                    ))
                    .with_source_span(definition.name.source_span())
                })?;
            } else {
                if !definition.pending_target_observation {
                    validate_literal_initializer(syntax, definition, definition.value)?;
                }
            }
        }
        let initializer = lower_expression_into_table(lowerer, syntax, definition.value)?;
        lowerer.pending_const_values.push(initializer);
        initializer_normalization::retain(lowerer, syntax, definition, initializer)?;
        Ok(initializer)
    }

    fn has_scalar_initializer(syntax: &SyntaxTrees, definition: &ConstDefinition) -> bool {
        use syntax_trees::expression::ExpressionNode;
        matches!(
            syntax.expressions.expression(definition.value),
            ExpressionNode::Boolean(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::String(_)
        )
    }

    /*
        Select a constant through the ordinary source-scoped path before its value
        is substituted. Initializer dependency discovery uses the same selection:
        module body names need not yet carry their finalized constant ledger rows.
    */
    pub(crate) fn selected_expression_constant(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
    ) -> Option<(SourceSpan, symbols::SymbolHandle)> {
        let table = &program.tables.bodies.expressions;
        let path = table.name_path_node(expression)?;
        if matches!(
            program.symbols.get(path.head_symbol).kind,
            SymbolKind::Local
                | SymbolKind::Parameter
                | SymbolKind::MachineParameter
                | SymbolKind::TypeParameter
                | SymbolKind::ConformanceParameter
                | SymbolKind::Field
        ) {
            return None;
        }
        let members = table.name_path_members(path.members);
        let first = members.first()?;
        let last = members.last()?;
        let reference = SourceSpan::new(
            first.source_span().source_id,
            Span::new(first.source_span().span.start, last.source_span().span.end),
        );
        let name = members.joined_path();
        let selected = program
            .symbols
            .find_top_level_by_name_and_kinds_from_source(&name, &[SymbolKind::Const], reference)?;
        Some((reference, selected))
    }

    /*
        v0 initializers are literals all the way down. Payloadless case names are
        nullary structural literals. The parser already folds `-5` into a single
        literal, so no operator node is legitimate here.
    */
    fn validate_literal_initializer(
        syntax_trees: &SyntaxTrees,
        definition: &ConstDefinition,
        value: syntax_trees::expression::ExpressionHandle,
    ) -> Result<(), Diagnostic> {
        use syntax_trees::expression::ExpressionNode;
        if crate::constant::direct_selector_literal(syntax_trees, value) {
            return Ok(());
        }
        match syntax_trees.expressions.expression(value) {
            ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
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
                let Some(data) = item.as_data() else {
                    return false;
                };
                data.name.as_str(syntax_trees) == type_name.as_str(syntax_trees)
                    && syntax_items(syntax_trees).data_members(data.members)
                        .iter()
                        .any(|member| {
                            matches!(
                                member,
                                DataMember::Variant(variant)
                                    if variant.name.as_str(syntax_trees) == case_name.as_str(syntax_trees)
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
            definition.scope.as_str(syntax_trees),
            definition.name.as_str(syntax_trees),
            syntax_expressions(syntax_trees)
                .expression(value)
                .display_name(syntax_trees),
        )))
    }
}
pub(crate) mod initializer_dependencies {
    /*
        Declaration-owned dependency discovery before provisional values exist.

        A static callable has two independent dependencies: its authored binder is
        the call's logical selection, and its explicit operand supplies the body
        that specialization may execute. Retain both without treating an abstract
        binder as an ordinary body or granting execution from source readiness.
    */

    use crate::selection::call_identity::{call_declaration_source, selected_declaration_source};
    use crate::tables::body_expressions;
    use crate::tables::resolved_declarations;
    use language_core::data_layout_queries::is_data_layout_query_spelling;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionTarget, ResolvedAuthoredCallBinding,
    };
    use source::SourceSpan;
    use std::iter::once;
    use symbol_resolved_trees::domain::ProofFact;
    use symbol_resolved_trees::expression::StaticMachineArgument;
    use symbol_resolved_trees::signature::SignatureContract;
    use symbol_resolved_trees::types::TypeReference;
    use symbol_resolved_trees::{
        SymbolResolvedTrees,
        expression::{ExpressionHandle, ExpressionNode},
        machine::MachineBodyStatus,
    };
    use symbols::{SymbolHandle, SymbolKind};

    mod types {
        /*
            Type work belongs to the demanded computation too. A selected record's
            extent may depend on a constant even when its initializer is never read as
            an expression. Discover that dependency before probe placeholders exist.
        */

        use super::{Collector, DependencyWork};
        use arena::{Handle, HandleSpan};
        use source::SourceSpan;
        use symbol_resolved_trees::{
            data::{DataMember, MachineParameterContract, TypeParameter, TypeParameterKind},
            expression::StaticMachineArgument,
            signature::StateParameter,
            types::{FixedArrayLength, TypeConstraint, TypeReference},
        };
        use symbols::{SymbolHandle, SymbolKind};

        impl<'program> Collector<'program, '_> {
            pub(super) fn constant(
                &mut self,
                symbol: SymbolHandle,
                reference: SourceSpan,
            ) -> Result<(), String> {
                let mut declarations = self
                    .program
                    .const_declarations
                    .iter()
                    .filter(|declaration| declaration.symbol == symbol);
                let declaration = declarations
                    .next()
                    .ok_or("constant dependency lost its retained declaration")?;
                if declarations.next().is_some() {
                    return Err("constant dependency has ambiguous declaration custody".to_owned());
                }
                let source = self
                    .program
                    .symbols
                    .symbol_source_span(symbol)
                    .ok_or("constant dependency lost its declaration source")?;
                let dependency = (reference, source);
                if !self.dependencies.constants.contains(&dependency) {
                    self.dependencies.constants.push(dependency);
                }
                if self.follow_dependencies && !self.expanded_constants.contains(&symbol) {
                    self.expanded_constants.push(symbol);
                    self.types.push(DependencyWork {
                        value: &declaration.declared_type,
                        lexical_owner: symbol,
                    });
                    let initializer = if declaration.authored_initializer.is_valid() {
                        declaration.authored_initializer
                    } else {
                        declaration.initializer
                    };
                    self.expressions.push(DependencyWork {
                        value: initializer,
                        lexical_owner: symbol,
                    });
                    let work = DependencyWork {
                        value: initializer,
                        lexical_owner: symbol,
                    };
                    if !self.initializer_expressions.contains(&work) {
                        self.initializer_expressions.push(work);
                    }
                }
                Ok(())
            }

            pub(super) fn type_child(
                &mut self,
                handle: Handle<TypeReference>,
            ) -> Result<(), String> {
                if !self.follow_dependencies
                    || self
                        .visited_type_children
                        .contains(&(handle, self.lexical_owner))
                {
                    return Ok(());
                }
                let children = &self.program.tables.declarations.child_type_references;
                if !children.is_valid(handle) {
                    return Err("constant dependency has a stale child type reference".to_owned());
                }
                self.visited_type_children
                    .push((handle, self.lexical_owner));
                self.queue_type(children.get(handle));
                Ok(())
            }

            pub(super) fn type_arguments(
                &mut self,
                arguments: HandleSpan<TypeReference>,
            ) -> Result<(), String> {
                if !self.follow_dependencies {
                    return Ok(());
                }
                let children = self.program.child_type_references(arguments);
                if children.len() != arguments.len() {
                    return Err("constant dependency has stale type arguments".to_owned());
                }
                for argument in arguments.handles() {
                    self.type_child(argument)?;
                }
                Ok(())
            }

            pub(super) fn type_reference(
                &mut self,
                reference: &'program TypeReference,
            ) -> Result<(), String> {
                match reference {
                    TypeReference::AliasUse {
                        expansion,
                        application,
                        ..
                    } => {
                        self.queue_type(expansion);
                        self.queue_type(application);
                    }
                    TypeReference::Reference(reference) => self.type_child(reference.referee)?,
                    TypeReference::Slice(reference) => self.type_child(reference.element_type)?,
                    TypeReference::FixedArray(array) => {
                        self.type_child(array.element_type)?;
                        match &array.length {
                            FixedArrayLength::ConstParameter { symbol, name }
                                if self.program.symbols.get(*symbol).kind == SymbolKind::Const =>
                            {
                                self.constant(*symbol, name.source_span())?;
                            }
                            FixedArrayLength::Expression(value) => self.type_child(*value)?,
                            FixedArrayLength::ConstCall { machine, name } => {
                                self.call(*machine, name.source_span())?;
                            }
                            FixedArrayLength::ConstParameter { .. }
                            | FixedArrayLength::Literal(_) => {}
                        }
                    }
                    TypeReference::Constrained(reference) => {
                        self.type_child(reference.base_type)?;
                        let constraints = self
                            .program
                            .tables
                            .types
                            .constraints
                            .span_or_empty(reference.constraints);
                        if constraints.len() != reference.constraints.len() {
                            return Err("constant dependency has stale type constraints".to_owned());
                        }
                        for constraint in constraints {
                            match constraint {
                                TypeConstraint::Domain(domain) => {
                                    self.type_arguments(domain.arguments)?
                                }
                                TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
                            }
                        }
                    }
                    TypeReference::Generic(application) => {
                        self.type_declaration(application.base_symbol)?;
                        self.type_arguments(application.arguments)?;
                    }
                    TypeReference::Named { symbol, name, .. }
                        if self.program.symbols.get(*symbol).kind == SymbolKind::Const =>
                    {
                        self.constant(*symbol, name.source_span())?;
                    }
                    TypeReference::Named { symbol, .. } => {
                        self.type_declaration(*symbol)?;
                    }
                    TypeReference::ConstExpression(expression) => {
                        self.queue_expression(expression.expression)
                    }
                    TypeReference::DynamicTrait { .. } | TypeReference::Unit => {}
                }
                Ok(())
            }

            pub(super) fn type_declaration(&mut self, symbol: SymbolHandle) -> Result<(), String> {
                let previous = self.lexical_owner;
                self.lexical_owner = symbol;
                let result = self.type_declaration_in_scope(symbol);
                self.lexical_owner = previous;
                result
            }

            fn type_declaration_in_scope(&mut self, symbol: SymbolHandle) -> Result<(), String> {
                if self.program.symbols.get(symbol).kind == SymbolKind::Conformance {
                    return self.conformance_declaration(symbol);
                }
                if self.program.symbols.get(symbol).kind == SymbolKind::TypeAlias {
                    if !self.follow_dependencies || self.visited_type_declarations.contains(&symbol)
                    {
                        return Ok(());
                    }
                    let mut aliases = self
                        .program
                        .type_aliases
                        .iter()
                        .filter(|alias| alias.symbol == symbol);
                    let alias = aliases
                        .next()
                        .ok_or("constant dependency lost its selected alias declaration")?;
                    if aliases.next().is_some() {
                        return Err(
                            "constant dependency has ambiguous alias declaration custody".into(),
                        );
                    }
                    self.visited_type_declarations.push(symbol);
                    self.type_parameters(alias.type_parameters)?;
                    for bound in &alias.conformance_bounds {
                        self.type_arguments(bound.arguments)?;
                    }
                    self.queue_type(&alias.expansion);
                    return Ok(());
                }
                if !self.follow_dependencies
                    || self.program.symbols.get(symbol).kind != SymbolKind::Data
                    || self.visited_type_declarations.contains(&symbol)
                {
                    return Ok(());
                }
                self.visited_type_declarations.push(symbol);
                let mut definitions = self
                    .program
                    .data_definitions
                    .iter()
                    .filter(|definition| definition.symbol == symbol);
                let definition = definitions
                    .next()
                    .ok_or("constant dependency lost its selected data declaration")?;
                if definitions.next().is_some() {
                    return Err(
                        "constant dependency has ambiguous data declaration custody".to_owned()
                    );
                }
                self.type_parameters(definition.type_parameters)?;
                if self.purpose == super::DependencyPurpose::DeclarationReadiness {
                    if let Some(application) = &definition.generic_instance {
                        self.queue_type(application);
                    }
                    self.predicate_facts(definition.requires_facts)?;
                }
                if let Some(quotient) = &definition.quotient {
                    self.queue_type(&quotient.carrier);
                }
                let members = self.program.data_members(definition.members);
                if members.len() != definition.members.len() {
                    return Err("constant dependency has stale data members".to_owned());
                }
                for member in members {
                    match member {
                        DataMember::Field(field) => self.queue_type(&field.type_reference),
                        DataMember::Variant(variant) => {
                            if self.purpose == super::DependencyPurpose::DeclarationReadiness {
                                self.predicate_facts(variant.requires_facts)?;
                            }
                            let fields = self.program.data_payload_fields(variant.payload);
                            if fields.len() != variant.payload.len() {
                                return Err(
                                    "constant dependency has stale variant fields".to_owned()
                                );
                            }
                            self.queue_types(fields.iter().map(|field| &field.type_reference));
                        }
                    }
                }
                Ok(())
            }

            fn conformance_declaration(&mut self, symbol: SymbolHandle) -> Result<(), String> {
                if !self.follow_dependencies
                    || self.purpose != super::DependencyPurpose::DeclarationReadiness
                    || self.visited_type_declarations.contains(&symbol)
                {
                    return Ok(());
                }
                let definitions = self
                    .program
                    .conformances
                    .iter()
                    .filter(|definition| definition.symbol == symbol)
                    .collect::<Vec<_>>();
                let [definition] = definitions.as_slice() else {
                    return Err(
                        "constant dependency lost its exact selected implementation declaration"
                            .into(),
                    );
                };
                self.visited_type_declarations.push(symbol);
                self.type_parameters(definition.type_parameters)?;
                self.type_arguments(definition.arguments)?;
                for row in &definition.rows {
                    let machine = crate::symbols::owned_conformance_realization(self.program, row)?;
                    if crate::symbols::owned_conformance_namespace(self.program, machine)? != symbol
                    {
                        return Err(
                            "constant dependency implementation member lost its exact owner".into(),
                        );
                    }
                    self.machines.push(machine);
                }
                Ok(())
            }

            pub(super) fn type_parameters(
                &mut self,
                parameters: HandleSpan<TypeParameter>,
            ) -> Result<(), String> {
                let parameters_slice = self
                    .program
                    .tables
                    .declarations
                    .data_type_parameters
                    .span_or_empty(parameters);
                if parameters_slice.len() != parameters.len() {
                    return Err("constant dependency has stale type parameters".to_owned());
                }
                for parameter in parameters_slice {
                    match &parameter.kind {
                        TypeParameterKind::Const { type_reference }
                        | TypeParameterKind::Value { type_reference } => {
                            self.queue_type(type_reference)
                        }
                        TypeParameterKind::Machine {
                            contract: MachineParameterContract::Structural(signature),
                        } => {
                            self.contracts(signature.contracts)?;
                            self.type_parameters(signature.type_parameters)?;
                            self.state_parameters(signature.parameters)?;
                            if let Some(reference) = &signature.return_type {
                                self.queue_type(reference);
                            }
                        }
                        TypeParameterKind::Proposition { contract } => {
                            self.state_parameters(contract.parameters)?;
                        }
                        TypeParameterKind::Type
                        | TypeParameterKind::Implementation
                        | TypeParameterKind::Machine { .. } => {}
                    }
                }
                Ok(())
            }

            pub(super) fn state_parameters(
                &mut self,
                parameters: HandleSpan<StateParameter>,
            ) -> Result<(), String> {
                let parameters_slice = self
                    .program
                    .tables
                    .declarations
                    .state_parameters
                    .span_or_empty(parameters);
                if parameters_slice.len() != parameters.len() {
                    return Err("constant dependency has stale state parameters".to_owned());
                }
                self.queue_types(
                    parameters_slice
                        .iter()
                        .map(|parameter| &parameter.type_reference),
                );
                Ok(())
            }

            pub(super) fn static_arguments(
                &mut self,
                arguments: &'program [StaticMachineArgument],
            ) -> Result<(), String> {
                if !self.follow_dependencies {
                    return Ok(());
                }
                for argument in arguments {
                    let selected = self.program.symbols.get(argument.symbol);
                    match selected.kind {
                        SymbolKind::Conformance => self.conformance_declaration(argument.symbol)?,
                        SymbolKind::Machine => self.machines.push(argument.symbol),
                        SymbolKind::State
                            if self.program.symbols.get(selected.parent).kind
                                == SymbolKind::Machine =>
                        {
                            self.machines.push(selected.parent);
                        }
                        _ => {}
                    }
                    if argument.type_reference.is_valid() {
                        self.type_child(argument.type_reference)?;
                    }
                    if let Some(application) = &argument.application {
                        self.static_arguments(&application.arguments)?;
                    }
                }
                Ok(())
            }
        }
    }

    /*
        Declaration preparation may visit every owned implementation member. An
        authored invocation closure stops at its logical bound until realization.
    */
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum DependencyPurpose {
        Authored,
        DeclarationReadiness,
    }

    /*
        Exact source occurrence/declaration pairs. Body dependencies are retained
        even when execution later selects another branch.
    */
    #[derive(Debug, Default)]
    pub struct ConstInitializerDependencies {
        /* A selected layout observation remains symbolic until target closure. */
        pub pending_target_observation: bool,
        /*
            Scoped overload choices still owed by ordinary typed call selection.
            Candidate availability contributes no selected-body constant dependency.
        */
        pub pending_calls: Vec<(SourceSpan, Vec<SourceSpan>)>,
        pub constants: Vec<(SourceSpan, SourceSpan)>,
        pub calls: Vec<(SourceSpan, SourceSpan)>,
        pub initializer_operators: Vec<SourceSpan>,
        /* Exact authored owners whose type applications belong to this dependency closure. */
        pub normalization_owners: Vec<SourceSpan>,
        pub(crate) call_targets: Vec<AuthoredCallDependency>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct AuthoredCallDependency {
        pub reference: SourceSpan,
        pub declaration: SourceSpan,
        pub target: SymbolHandle,
    }

    /*
        Exact machine roots and their resolved call dependencies. A deferred body
        terminates traversal: complete `deferred_machines` and collect again before
        treating `machines` as the complete execution roster. This is body demand,
        not permission to execute or evidence of checked call selection.
    */
    #[derive(Debug, Default)]
    pub struct ResolvedMachineBodyDemand {
        pub machines: Vec<SymbolHandle>,
        pub deferred_machines: Vec<SymbolHandle>,
    }

    pub(crate) fn collect_machine_body_demand(
        program: &SymbolResolvedTrees,
        machine_roots: &[SymbolHandle],
        expression_roots: &[ExpressionHandle],
    ) -> Result<ResolvedMachineBodyDemand, String> {
        let mut collector = Collector {
            machines: machine_roots.iter().rev().copied().collect(),
            expressions: expression_roots
                .iter()
                .rev()
                .copied()
                .map(DependencyWork::unscoped)
                .collect(),
            initializer_expressions: expression_roots
                .iter()
                .copied()
                .map(DependencyWork::unscoped)
                .collect(),
            types: program
                .const_declarations
                .iter()
                .filter(|declaration| {
                    let initializer = if declaration.authored_initializer.is_valid() {
                        declaration.authored_initializer
                    } else {
                        declaration.initializer
                    };
                    expression_roots.contains(&initializer)
                })
                .map(|declaration| DependencyWork::unscoped(&declaration.declared_type))
                .collect(),
            deferred_machines: Some(Vec::new()),
            purpose: DependencyPurpose::DeclarationReadiness,
            ..Collector::new(program)
        };
        collector.collect()?;
        let mut pending = collector.available_machines.clone();
        let mut visited = collector.visited_machines.clone();
        while let Some(machine) = pending.pop() {
            if visited.contains(&machine) {
                continue;
            }
            let mut availability = Collector {
                machines: vec![machine],
                deferred_machines: Some(Vec::new()),
                purpose: DependencyPurpose::DeclarationReadiness,
                ..Collector::new(program)
            };
            availability.collect()?;
            visited.extend(availability.visited_machines);
            pending.extend(availability.available_machines);
            let deferred = collector
                .deferred_machines
                .as_mut()
                .expect("body-demand owner");
            for machine in availability.deferred_machines.unwrap_or_default() {
                if !deferred.contains(&machine) {
                    deferred.push(machine);
                }
            }
        }
        Ok(ResolvedMachineBodyDemand {
            machines: collector.visited_machines,
            deferred_machines: collector.deferred_machines.unwrap_or_default(),
        })
    }

    pub(crate) fn collect(
        program: &SymbolResolvedTrees,
        root: ExpressionHandle,
        selected_calls: Option<&[(SourceSpan, SourceSpan)]>,
    ) -> Result<ConstInitializerDependencies, String> {
        let mut collector = Collector {
            expressions: vec![DependencyWork::unscoped(root)],
            initializer_expressions: vec![DependencyWork::unscoped(root)],
            selected_calls,
            ..Collector::new(program)
        };
        collector.collect()?;
        Ok(collector.dependencies)
    }

    /*
        A declaration's destination participates in readiness independently of the
        expression leaves. In particular an empty/literal array still demands its
        exact extent before its value can be encoded or selected by another job.
    */
    pub(crate) fn collect_declaration(
        program: &SymbolResolvedTrees,
        root: ExpressionHandle,
        destination: &TypeReference,
        purpose: DependencyPurpose,
        retained_base: Option<&SymbolResolvedTrees>,
        selected_calls: Option<&[(SourceSpan, SourceSpan)]>,
    ) -> Result<ConstInitializerDependencies, String> {
        let mut collector = Collector {
            expressions: vec![DependencyWork::unscoped(root)],
            initializer_expressions: vec![DependencyWork::unscoped(root)],
            selected_calls,
            types: vec![DependencyWork::unscoped(destination)],
            purpose,
            ..Collector::new(program)
        };
        collector.collect()?;
        for symbol in collector
            .visited_machines
            .iter()
            .chain(&collector.available_machines)
            .chain(&collector.visited_type_declarations)
            .chain(&collector.expanded_constants)
        {
            let declaration = collector.program.symbols.get(*symbol);
            let namespace = if declaration.kind == SymbolKind::Machine {
                crate::symbols::owned_conformance_namespace(collector.program, *symbol)?
            } else {
                SymbolHandle::invalid()
            };
            let owner = if namespace.is_valid() {
                namespace
            } else {
                *symbol
            };
            if retained_base.is_some_and(|base| {
                base.symbols.symbols().nodes().is_valid(owner)
                    && base.symbols.symbol_source_is_selected(owner)
            }) {
                continue;
            }
            let source = collector
                .program
                .symbols
                .symbol_source_span(owner)
                .ok_or("initializer normalization owner lost its declaration source")?;
            if !collector
                .dependencies
                .normalization_owners
                .contains(&source)
            {
                collector.dependencies.normalization_owners.push(source);
            }
        }

        Ok(collector.dependencies)
    }

    pub(crate) fn expression_at_source(
        program: &SymbolResolvedTrees,
        root: ExpressionHandle,
        source: SourceSpan,
    ) -> Result<ExpressionHandle, String> {
        let mut collector = Collector {
            expressions: vec![DependencyWork::unscoped(root)],
            follow_dependencies: false,
            ..Collector::new(program)
        };
        let mut selected = None;
        while let Some(work) = collector.expressions.pop() {
            collector.lexical_owner = work.lexical_owner;
            let expression = work.value;
            if collector.visited_expressions.contains(&work) {
                continue;
            }
            collector.visited_expressions.push(work);
            if program.tables.bodies.expressions.source_span(expression) == source {
                if selected.is_some() {
                    return Err("initializer leaf source correspondence is ambiguous".to_owned());
                }
                selected = Some(expression);
            }
            collector.expression(expression)?;
        }
        selected
            .ok_or_else(|| "initializer leaf is absent from its retained original root".to_owned())
    }

    /*
        Bind constant-initializer calls through a named implementation
        to its public requirement before authored-selection recording. Every copy
        of those source tokens is bound too: selection finalization groups copies
        by token, and one unresolved copy keeps a late call selection. The private
        realization supplies body demand separately, just as for machine-body calls.
    */
    pub(crate) fn bind_named_implementation_const_calls(
        program: &mut SymbolResolvedTrees,
        roots: &[ExpressionHandle],
    ) -> Result<(), String> {
        let mut spans = Vec::new();
        collect_call_target_spans(program, roots, &mut spans);
        if spans.is_empty() {
            return Ok(());
        }
        let expressions = body_expressions(program)
            .iter_expressions()
            .map(|(expression, _)| expression)
            .collect::<Vec<_>>();
        let mut routing =
            crate::selection::conformance_blocks::ProvisionalBodyCallRouting::new(program);
        let mut targets = Vec::new();
        for expression in expressions {
            let node = expression_node(program, expression);
            let ExpressionNode::Call(call) = &node else {
                continue;
            };
            if !spans.contains(&call.target.source_span()) {
                continue;
            }
            let receiver_symbol = body_expressions(program)
                .name_path_node(call.receiver)
                .map_or_else(SymbolHandle::invalid, |path| path.symbol);
            let Some(binding) = routing.named_binding(
                &crate::selection::body_calls::CallSite {
                    receiver_symbol,
                    target_symbol: call.target_symbol,
                    target: &call.target,
                    member: &call.target,
                    requirement_application: call.requirement_application,
                    local_receiver: !call.receiver.is_valid(),
                    argument_count: body_expressions(program)
                        .expression_handles(call.arguments)
                        .len(),
                },
                &call.machine_arguments,
            )?
            else {
                continue;
            };
            targets.push((expression, binding.original));
        }
        drop(routing);
        for (expression, requirement) in targets {
            let ExpressionNode::Call(call) =
                program.tables.bodies.expressions.expression_mut(expression)
            else {
                continue;
            };
            call.target_symbol = requirement;
        }
        Ok(())
    }

    fn collect_call_target_spans(
        program: &SymbolResolvedTrees,
        roots: &[ExpressionHandle],
        spans: &mut Vec<SourceSpan>,
    ) {
        let mut pending = roots.to_vec();
        let mut visited = Vec::new();
        while let Some(expression) = pending.pop() {
            if !expression.is_valid() || visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            if !body_expressions(program).expression_is_valid(expression) {
                continue;
            }
            let node = expression_node(program, expression);
            if let ExpressionNode::Call(call) = &node
                && !spans.contains(&call.target.source_span())
            {
                spans.push(call.target.source_span());
            }
            pending.extend(expression_child_handles(program, &node));
        }
    }

    /* The node a handle names, copied out before a walk mutates the table. */
    fn expression_node(program: &SymbolResolvedTrees, handle: ExpressionHandle) -> ExpressionNode {
        program.tables.bodies.expressions.expression(handle).clone()
    }

    fn expression_child_handles(
        program: &SymbolResolvedTrees,
        node: &ExpressionNode,
    ) -> Vec<ExpressionHandle> {
        let table = &program.tables.bodies.expressions;
        match node {
            ExpressionNode::Call(call) => {
                let mut children = Vec::new();
                if call.receiver.is_valid() {
                    children.push(call.receiver);
                }
                children.extend(table.expression_handles(call.arguments).iter().copied());
                children
            }
            ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
            ExpressionNode::Unary(unary) => vec![unary.operand],
            ExpressionNode::Cast(cast) => vec![cast.value],
            ExpressionNode::Member(member) => vec![member.receiver],
            ExpressionNode::Borrow(borrow) => vec![borrow.target],
            ExpressionNode::Indexed(indexed) => vec![indexed.collection, indexed.index],
            ExpressionNode::Membership(membership) => vec![membership.value],
            ExpressionNode::Atomic(atomic) => vec![atomic.value, atomic.result],
            ExpressionNode::Range(range) => [range.start, range.end]
                .into_iter()
                .filter(|handle| handle.is_valid())
                .collect(),
            ExpressionNode::ArrayLiteral(elements) => table.expression_handles(*elements).to_vec(),
            ExpressionNode::StructLiteral(literal) => table
                .struct_fields(literal.fields)
                .iter()
                .map(|field| field.value)
                .collect(),
            ExpressionNode::Match(dispatch) => {
                let mut children = vec![dispatch.subject];
                for arm in table.match_arms(dispatch.arms) {
                    children.extend(table.match_pattern_dependencies(arm.pattern));
                    children.push(arm.value);
                }
                children
            }
            ExpressionNode::SavedSubject(saved) => vec![
                table.match_subject(saved.capture).subject,
                saved.pattern_head,
            ],
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => Vec::new(),
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct DependencyWork<Value> {
        value: Value,
        /*
            Default bodies can share source nodes across distinct realizations.
            Their queued work and visited keys must keep each row's exact owner.
        */
        lexical_owner: SymbolHandle,
    }

    impl<Value> DependencyWork<Value> {
        fn unscoped(value: Value) -> Self {
            Self {
                value,
                lexical_owner: SymbolHandle::invalid(),
            }
        }
    }

    struct Collector<'program, 'selection> {
        program: &'program SymbolResolvedTrees,
        body_call_routing:
            crate::selection::conformance_blocks::ProvisionalBodyCallRouting<'program>,
        dependencies: ConstInitializerDependencies,
        expressions: Vec<DependencyWork<ExpressionHandle>>,
        lexical_owner: SymbolHandle,
        /*
            Exact expression-tree membership, independent of diagnostic spans.
            Machine bodies and declaration type work enter their separate queues;
            following a call does not make its body an initializer expression.
        */
        initializer_expressions: Vec<DependencyWork<ExpressionHandle>>,
        machines: Vec<SymbolHandle>,
        available_machines: Vec<SymbolHandle>,
        types: Vec<DependencyWork<&'program TypeReference>>,
        visited_type_children: Vec<(arena::Handle<TypeReference>, SymbolHandle)>,
        visited_type_declarations: Vec<SymbolHandle>,
        expanded_constants: Vec<SymbolHandle>,
        visited_expressions: Vec<DependencyWork<ExpressionHandle>>,
        visited_initializer_expressions: Vec<DependencyWork<ExpressionHandle>>,
        visited_machines: Vec<SymbolHandle>,
        /*
            Only a retained-body owner can request an incomplete dependency frontier.
            Ordinary initializer collection requires every reached body to exist.
        */
        deferred_machines: Option<Vec<SymbolHandle>>,
        follow_dependencies: bool,
        purpose: DependencyPurpose,
        /*
            Typed choices disambiguate unresolved overload families. Readiness can
            also visit independently selected member bodies absent from that hint.
            Final normalization compares the complete reconstructed source roster.
        */
        selected_calls: Option<&'selection [(SourceSpan, SourceSpan)]>,
    }

    impl<'program, 'selection> Collector<'program, 'selection> {
        fn new(program: &'program SymbolResolvedTrees) -> Self {
            Self {
                program,
                body_call_routing:
                    crate::selection::conformance_blocks::ProvisionalBodyCallRouting::new(program),
                dependencies: ConstInitializerDependencies::default(),
                expressions: Vec::new(),
                lexical_owner: SymbolHandle::invalid(),
                initializer_expressions: Vec::new(),
                machines: Vec::new(),
                available_machines: Vec::new(),
                types: Vec::new(),
                visited_type_children: Vec::new(),
                visited_type_declarations: Vec::new(),
                expanded_constants: Vec::new(),
                visited_expressions: Vec::new(),
                visited_initializer_expressions: Vec::new(),
                visited_machines: Vec::new(),
                deferred_machines: None,
                follow_dependencies: true,
                purpose: DependencyPurpose::Authored,
                selected_calls: None,
            }
        }

        fn expression_work(&self, value: ExpressionHandle) -> DependencyWork<ExpressionHandle> {
            DependencyWork {
                value,
                lexical_owner: self.lexical_owner,
            }
        }

        fn queue_expression(&mut self, value: ExpressionHandle) {
            self.expressions.push(DependencyWork {
                value,
                lexical_owner: self.lexical_owner,
            });
        }

        fn queue_expressions(&mut self, values: impl IntoIterator<Item = ExpressionHandle>) {
            let lexical_owner = self.lexical_owner;
            self.expressions
                .extend(values.into_iter().map(|value| DependencyWork {
                    value,
                    lexical_owner,
                }));
        }

        fn queue_type(&mut self, value: &'program TypeReference) {
            self.types.push(DependencyWork {
                value,
                lexical_owner: self.lexical_owner,
            });
        }

        fn queue_types(&mut self, values: impl IntoIterator<Item = &'program TypeReference>) {
            let lexical_owner = self.lexical_owner;
            self.types
                .extend(values.into_iter().map(|value| DependencyWork {
                    value,
                    lexical_owner,
                }));
        }

        fn collect(&mut self) -> Result<(), String> {
            while !self.expressions.is_empty()
                || !self.machines.is_empty()
                || !self.types.is_empty()
            {
                if let Some(work) = self.expressions.pop() {
                    self.lexical_owner = work.lexical_owner;
                    let expression = work.value;
                    let initializer = self.initializer_expressions.contains(&work);
                    if self.visited_expressions.contains(&work)
                        && (!initializer || self.visited_initializer_expressions.contains(&work))
                    {
                        continue;
                    }
                    if !self.visited_expressions.contains(&work) {
                        self.visited_expressions.push(work);
                    }
                    if initializer {
                        self.visited_initializer_expressions.push(work);
                    }
                    self.expression(expression)?;
                } else if let Some(machine) = self.machines.pop() {
                    if self.visited_machines.contains(&machine) {
                        continue;
                    }
                    self.visited_machines.push(machine);
                    self.machine(machine)?;
                } else if let Some(work) = self.types.pop() {
                    self.lexical_owner = work.lexical_owner;
                    self.type_reference(work.value)?;
                }
            }
            Ok(())
        }

        fn call(&mut self, target: SymbolHandle, reference: SourceSpan) -> Result<(), String> {
            let symbol = self.program.symbols.get(target);
            let trait_requirement = symbol.kind == SymbolKind::State
                && self.program.traits.iter().any(|definition| {
                    definition.symbol == symbol.parent
                        && self
                            .program
                            .trait_machine_signatures(definition.machines)
                            .iter()
                            .any(|requirement| requirement.symbol == target)
                });
            if trait_requirement {
                self.dependencies.pending_target_observation = true;
                return self.record_logical_call(target, reference);
            }
            let bound_requirement = symbol.kind == SymbolKind::State
                && symbol.parent.is_valid()
                && self.program.symbols.get(symbol.parent).kind == SymbolKind::ConformanceParameter;
            if matches!(
                symbol.kind,
                SymbolKind::Operator | SymbolKind::MachineParameter
            ) || bound_requirement
            {
                return self.record_logical_call(target, reference);
            }
            let machine = match symbol.kind {
                SymbolKind::Machine => target,
                SymbolKind::State
                    if self.program.symbols.get(symbol.parent).kind == SymbolKind::Machine =>
                {
                    symbol.parent
                }
                _ if self.deferred_machines.is_some() => return Ok(()),
                _ => {
                    return Err(format!(
                        "constant initializer call has no exact ordinary machine declaration: selected {:?} at {reference:?}",
                        symbol.kind,
                    ));
                }
            };
            let declaration = call_declaration_source(self.program, target)?;
            let selection = (reference, declaration);
            if !self.dependencies.calls.contains(&selection) {
                self.dependencies.calls.push(selection);
            }
            let target = if symbol.kind == SymbolKind::Machine && self.deferred_machines.is_none() {
                let mut owners = self
                    .program
                    .machines
                    .iter()
                    .filter(|owner| owner.symbol == machine);
                let owner = owners
                    .next()
                    .ok_or("constant callable selection lost its machine")?;
                if owners.next().is_some() {
                    return Err("constant callable selection has ambiguous machine identity".into());
                }
                let states = self.program.machine_state_handles(owner.states);
                if states.len() != owner.states.len() {
                    return Err("constant callable selection has stale machine states".into());
                }
                let entry = states
                    .first()
                    .ok_or("constant callable selection has no entry state")?;
                self.program.machine_state(*entry).symbol
            } else {
                target
            };
            let dependency = AuthoredCallDependency {
                reference,
                declaration,
                target,
            };
            if !self.dependencies.call_targets.contains(&dependency) {
                self.dependencies.call_targets.push(dependency);
            }
            if self.follow_dependencies {
                self.machines.push(machine);
            }
            Ok(())
        }

        fn record_logical_call(
            &mut self,
            target: SymbolHandle,
            reference: SourceSpan,
        ) -> Result<(), String> {
            let declaration = selected_declaration_source(self.program, target)?;
            let selection = (reference, declaration);
            if !self.dependencies.calls.contains(&selection) {
                self.dependencies.calls.push(selection);
            }
            let dependency = AuthoredCallDependency {
                reference,
                declaration,
                target,
            };
            if !self.dependencies.call_targets.contains(&dependency) {
                self.dependencies.call_targets.push(dependency);
            }
            Ok(())
        }

        fn call_dependency(
            &mut self,
            site: crate::selection::body_calls::CallSite<'_>,
            target_span: SourceSpan,
            static_arguments: &[StaticMachineArgument],
        ) -> Result<(), String> {
            let receiver_symbol = site.receiver_symbol;
            let target_symbol = site.target_symbol;
            let target_name = site.member.as_str();
            if let Some(binding) = self
                .body_call_routing
                .named_binding(&site, static_arguments)?
            {
                return self.realized_call_dependency(binding, target_span);
            }
            if let Some(binding) = self.body_call_routing.binding(self.lexical_owner, &site)? {
                return self.realized_call_dependency(binding, target_span);
            }
            if !target_symbol.is_valid() {
                let symbols = &self.program.symbols;
                let candidates = if symbols.get(receiver_symbol).kind == SymbolKind::Module {
                    symbols
                        .child_handles(receiver_symbol)
                        .into_iter()
                        .flatten()
                        .filter(|candidate| {
                            symbols.get(*candidate).kind == SymbolKind::Machine
                                && symbols.name(*candidate) == target_name
                        })
                        .collect::<Vec<_>>()
                } else if !receiver_symbol.is_valid() {
                    symbols.top_level_candidates_from_source(
                        target_name,
                        &[SymbolKind::Machine],
                        target_span,
                    )
                } else {
                    Vec::new()
                };
                if !candidates.is_empty() {
                    if self.deferred_machines.is_some() {
                        if self.follow_dependencies {
                            self.available_machines.extend(candidates);
                        }
                        return Ok(());
                    }
                    let declarations = candidates
                        .iter()
                        .map(|candidate| call_declaration_source(self.program, *candidate))
                        .collect::<Result<Vec<_>, _>>()?;
                    if let Some(selected_calls) = self.selected_calls
                        && selected_calls
                            .iter()
                            .any(|(reference, _)| *reference == target_span)
                    {
                        let choices = selected_calls
                            .iter()
                            .filter(|(reference, _)| *reference == target_span)
                            .collect::<Vec<_>>();
                        let [(_, declaration)] = choices.as_slice() else {
                            return Err(
                                "constant overload has no unique typed source selection".into()
                            );
                        };
                        let selected = candidates
                            .iter()
                            .zip(&declarations)
                            .filter(|(_, source)| *source == declaration)
                            .map(|(candidate, _)| *candidate)
                            .collect::<Vec<_>>();
                        let [selected] = selected.as_slice() else {
                            return Err(
                                "constant overload selection is outside its exact scoped family"
                                    .into(),
                            );
                        };
                        return self.call(*selected, target_span);
                    }
                    for candidate in candidates {
                        if !self.available_machines.contains(&candidate) {
                            self.available_machines.push(candidate);
                        }
                    }
                    let pending = (target_span, declarations);
                    if !self.dependencies.pending_calls.contains(&pending) {
                        self.dependencies.pending_calls.push(pending);
                    }
                    return Ok(());
                }
            }
            self.call(target_symbol, target_span)
        }

        fn realized_call_dependency(
            &mut self,
            binding: ResolvedAuthoredCallBinding,
            reference: SourceSpan,
        ) -> Result<(), String> {
            if binding.original == binding.selected {
                return self.call(binding.original, reference);
            }
            self.record_logical_call(binding.original, reference)?;
            if self.follow_dependencies {
                self.machines
                    .push(self.program.symbols.get(binding.selected).parent);
            }
            Ok(())
        }

        fn expression(&mut self, expression: ExpressionHandle) -> Result<(), String> {
            let initializer = self
                .initializer_expressions
                .contains(&self.expression_work(expression));
            let child_frontier = self.expressions.len();
            self.expression_dependencies(expression)?;
            if initializer {
                let children = expression_child_handles(
                    self.program,
                    self.program
                        .tables
                        .bodies
                        .expressions
                        .expression(expression),
                );
                for child in children {
                    if self.expressions[child_frontier..]
                        .iter()
                        .any(|work| *work == self.expression_work(child))
                        && !self
                            .initializer_expressions
                            .contains(&self.expression_work(child))
                    {
                        self.initializer_expressions
                            .push(self.expression_work(child));
                    }
                }
            }
            Ok(())
        }

        fn expression_dependencies(&mut self, expression: ExpressionHandle) -> Result<(), String> {
            let program = self.program;
            let table = &program.tables.bodies.expressions;
            if !table.expression_is_valid(expression) {
                return Err("constant initializer dependency has a stale expression".to_owned());
            }
            let authored = table.materialization_source(expression);
            if authored.is_valid() {
                self.queue_expression(authored);
                if self
                    .initializer_expressions
                    .contains(&self.expression_work(expression))
                    && !self
                        .initializer_expressions
                        .contains(&self.expression_work(authored))
                {
                    self.initializer_expressions
                        .push(self.expression_work(authored));
                }
            }
            if matches!(
                table.expression(expression),
                ExpressionNode::Binary(_) | ExpressionNode::Unary(_)
            ) {
                let reference = table.source_span(expression);
                if self
                    .initializer_expressions
                    .contains(&self.expression_work(expression))
                    && !self.dependencies.initializer_operators.contains(&reference)
                {
                    self.dependencies.initializer_operators.push(reference);
                }
            }
            let direct_constant = super::selected_expression_constant(program, expression);
            let terminal_segment = match table.expression(expression) {
                ExpressionNode::Name(path) => table
                    .name_path_members(path.members)
                    .last()
                    .map(|member| member.source_span()),
                _ => None,
            };
            for occurrence in table.authored_selection_occurrences(expression) {
                let selection = self
                    .program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .ok_or("constant initializer lost its declaration selection")?;
                let AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target()
                else {
                    continue;
                };
                if self.program.symbols.get(selected.selected_symbol()).kind != SymbolKind::Const {
                    continue;
                }
                let reference = direct_constant
                    .filter(|(_, selected_symbol)| {
                        *selected_symbol == selected.selected_symbol()
                            && terminal_segment == Some(selection.source_span())
                    })
                    .map_or(selection.source_span(), |(reference, _)| reference);
                self.constant(selected.selected_symbol(), reference)?;
            }
            match table.expression(expression) {
                ExpressionNode::Call(call) => {
                    self.static_arguments(&call.machine_arguments)?;
                    let native_query = call.target.as_str() == "code_layout"
                        && !call.target_symbol.is_valid()
                        && !call.receiver.is_valid();
                    if native_query && self.deferred_machines.is_none() {
                        return Err("late native code-layout metadata is unavailable to same-product semantic constant, type, layout, or optimization evaluation".to_owned());
                    }
                    let layout_query = is_data_layout_query_spelling(
                        call.target.as_str(),
                        call.target_symbol.is_valid(),
                        call.receiver.is_valid(),
                    );
                    self.dependencies.pending_target_observation |= layout_query;
                    if !native_query && !layout_query {
                        let receiver_symbol = table
                            .name_path_node(call.receiver)
                            .map_or_else(SymbolHandle::invalid, |path| path.symbol);
                        self.call_dependency(
                            crate::selection::body_calls::CallSite {
                                receiver_symbol,
                                target_symbol: call.target_symbol,
                                target: &call.target,
                                member: &call.target,
                                requirement_application: call.requirement_application,
                                local_receiver: !call.receiver.is_valid(),
                                argument_count: table.expression_handles(call.arguments).len(),
                            },
                            call.target.source_span(),
                            &call.machine_arguments,
                        )?;
                    }
                    if call.receiver.is_valid() {
                        self.queue_expression(call.receiver);
                    }
                    let arguments = table.expression_handles(call.arguments);
                    if arguments.len() != call.arguments.len() {
                        return Err("constant call arguments have a stale span".to_owned());
                    }
                    self.queue_expressions(arguments.iter().rev().copied());
                }
                ExpressionNode::Binary(binary) => {
                    self.queue_expressions([binary.right, binary.left])
                }
                ExpressionNode::Unary(unary) => self.queue_expression(unary.operand),
                ExpressionNode::Cast(cast) => {
                    self.type_child(cast.target_type)?;
                    self.type_arguments(cast.semantic_domain_arguments)?;
                    self.queue_expression(cast.value);
                }
                ExpressionNode::Member(member) => self.queue_expression(member.receiver),
                ExpressionNode::Borrow(borrow) => self.queue_expression(borrow.target),
                ExpressionNode::Indexed(indexed) => {
                    self.queue_expressions([indexed.index, indexed.collection])
                }
                ExpressionNode::Membership(membership) => self.queue_expression(membership.value),
                ExpressionNode::Atomic(atomic) => {
                    self.queue_expressions([atomic.result, atomic.value])
                }
                ExpressionNode::Range(range) => self.queue_expressions(
                    [range.end, range.start]
                        .into_iter()
                        .filter(|handle| handle.is_valid()),
                ),
                ExpressionNode::ArrayLiteral(elements) => {
                    let values = table.expression_handles(*elements);
                    if values.len() != elements.len() {
                        return Err("constant array dependency has a stale span".to_owned());
                    }
                    self.queue_expressions(values.iter().rev().copied());
                }
                ExpressionNode::StructLiteral(literal) => {
                    self.type_declaration(literal.type_symbol)?;
                    let fields = table.struct_fields(literal.fields);
                    if fields.len() != literal.fields.len() {
                        return Err("constant constructor dependency has a stale span".to_owned());
                    }
                    self.queue_expressions(fields.iter().rev().map(|field| field.value));
                }
                ExpressionNode::Match(dispatch) => {
                    let arms = table.match_arms(dispatch.arms);
                    if arms.len() != dispatch.arms.len() {
                        return Err("constant dispatch dependency has a stale span".to_owned());
                    }
                    for arm in arms.iter().rev() {
                        self.queue_expression(arm.value);
                        self.queue_expressions(table.match_pattern_dependencies(arm.pattern));
                    }
                    self.queue_expression(dispatch.subject);
                }
                ExpressionNode::SavedSubject(saved) => {
                    self.queue_expressions([
                        table.match_subject(saved.capture).subject,
                        saved.pattern_head,
                    ]);
                }
                ExpressionNode::Name(_) => {
                    if let Some((reference, selected)) =
                        super::selected_expression_constant(program, expression)
                    {
                        self.constant(selected, reference)?;
                    } else if let ExpressionNode::Name(path) = table.expression(expression)
                        && program.symbols.get(path.symbol).kind == SymbolKind::Variant
                    {
                        self.type_declaration(program.symbols.get(path.symbol).parent)?;
                    }
                }
                ExpressionNode::TypeExpression(reference)
                | ExpressionNode::ZeroValue(reference) => self.type_child(*reference)?,
                ExpressionNode::Integer(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::String(_) => {}
            }
            Ok(())
        }

        /*
            Invocation formation demands the complete published clauses before a
            body can execute. A helper whose value body is independent can still
            need selected providers to form its requires/ensures operands.
        */
        fn contracts(
            &mut self,
            contracts: arena::HandleSpan<SignatureContract>,
        ) -> Result<(), String> {
            let program = self.program;
            let clauses = program.signature_contracts(contracts);
            if clauses.len() != contracts.len() {
                return Err("constant dependency has stale signature clauses".to_owned());
            }
            for clause in clauses {
                self.predicate_facts(clause.facts)?;
            }
            Ok(())
        }

        /*
            Type/default-domain and invocation predicates share the same exact
            dependency roster. Readiness selects their bodies; it proves no clause.
        */
        fn predicate_facts(&mut self, span: arena::HandleSpan<ProofFact>) -> Result<(), String> {
            let facts = self.program.proof_facts(span);
            if facts.len() != span.len() {
                return Err("constant dependency has stale predicate facts".to_owned());
            }
            for fact in facts {
                match fact {
                    ProofFact::Expression(expression) => {
                        self.queue_expression(*expression);
                    }
                    ProofFact::Membership(membership) => {
                        self.queue_expression(membership.value);
                        self.queue_types(
                            self.program
                                .child_type_references(membership.domain_arguments),
                        );
                    }
                }
            }
            Ok(())
        }

        fn machine(&mut self, symbol: SymbolHandle) -> Result<(), String> {
            use symbol_resolved_trees::statement::{Statement, TransitionGuard, TransitionTarget};
            self.lexical_owner = symbol;
            let program = self.program;
            let mut machines = program
                .machines
                .iter()
                .filter(|machine| machine.symbol == symbol);
            let machine = machines
                .next()
                .ok_or("constant dependency call lost its machine body")?;
            if machines.next().is_some() {
                return Err("constant dependency machine is ambiguous".to_owned());
            }
            self.contracts(machine.contracts)?;
            if machine.target_family_interface {
                self.dependencies.pending_target_observation = true;
                self.machines.extend(
                    program
                        .machines
                        .iter()
                        .filter(|member| {
                            member.target_family == symbol && !member.target_family_interface
                        })
                        .map(|member| member.symbol),
                );
            }
            self.type_parameters(machine.type_parameters)?;
            for equation in resolved_declarations(program)
                .declared_type_equations
                .span_or_empty(machine.declared_type_equations)
            {
                self.queue_types([&equation.parameter, &equation.definition]);
            }
            if let Some(reference) = &machine.attached_data_application {
                self.queue_type(reference);
            }
            if machine.body_status == MachineBodyStatus::Deferred {
                let Some(deferred) = &mut self.deferred_machines else {
                    return Err(format!(
                        "constant dependency requires a deferred machine body: `{}` at {:?}",
                        program.symbols.display_path(symbol, "::"),
                        program.symbols.symbol_source_span(symbol),
                    ));
                };
                deferred.push(symbol);
                return Ok(());
            }
            let states = program.machine_state_handles(machine.states);
            if states.len() != machine.states.len() {
                return Err("constant dependency machine has stale states".to_owned());
            }
            let owned_values = program.machine_owned_data(machine.owned_data);
            if owned_values.len() != machine.owned_data.len() {
                return Err("constant dependency machine has stale owned initializers".to_owned());
            }
            for owned in owned_values {
                self.queue_type(&owned.type_reference);
                if owned.initial_value.is_valid() {
                    self.queue_expression(owned.initial_value);
                }
            }
            let expressions = &program.tables.bodies.expressions;
            for handle in states {
                let state = program.machine_state(*handle);
                self.contracts(state.contracts)?;
                self.state_parameters(state.parameters)?;
                if let Some(reference) = &state.return_type {
                    self.queue_type(reference);
                }
                if !state.symbol.is_valid()
                    || program.symbols.get(state.symbol).parent != machine.symbol
                {
                    return Err("constant dependency has a stale or foreign state".to_owned());
                }
                let body = program.state_statements(state.statements);
                if body.len() != state.statements.len() {
                    return Err("constant dependency state has stale statements".to_owned());
                }
                let mut statements: Vec<_> = body.iter().rev().collect();
                while let Some(statement) = statements.pop() {
                    match statement {
                        Statement::Assembly(handle) => {
                            let declarations = &program.tables.declarations;
                            let graph = declarations.assembly_graphs.get(*handle);
                            let blocks = declarations.assembly_blocks.span_or_empty(graph.blocks);
                            if blocks.len() != graph.blocks.len() {
                                return Err(
                                    "constant dependency assembly has stale blocks".to_owned()
                                );
                            }
                            for block in blocks.iter().rev() {
                                for span in [block.requires, block.ensures] {
                                    let expressions =
                                        declarations.assembly_expressions.span_or_empty(span);
                                    if expressions.len() != span.len() {
                                        return Err(
                                            "constant dependency assembly has stale contracts"
                                                .to_owned(),
                                        );
                                    }
                                    self.queue_expressions(expressions.iter().copied());
                                }
                                let body = declarations
                                    .state_statements
                                    .span_or_empty(block.instructions);
                                if body.len() != block.instructions.len() {
                                    return Err(
                                        "constant dependency assembly has stale instructions"
                                            .to_owned(),
                                    );
                                }
                                statements.extend(body.iter().rev());
                            }
                        }
                        Statement::Expression(expression) => self.queue_expression(*expression),
                        Statement::LocalData(local) => {
                            self.queue_type(&local.type_reference);
                            if local.initial_value.is_valid() {
                                self.queue_expression(local.initial_value);
                            }
                        }
                        Statement::Assignment(assignment) => {
                            self.queue_expressions([assignment.target, assignment.value])
                        }
                        Statement::RootBinding(binding) => {
                            self.queue_expression(binding.receiver);
                            if binding.implementation_operand.is_valid() {
                                self.queue_expression(binding.implementation_operand);
                            }
                        }
                        Statement::AssemblyFact(fact) => self.queue_expression(fact.expression),
                        Statement::ProofOutputBindingStatement(binding) => {
                            self.queue_expression(binding.call)
                        }
                        Statement::Call(call) => {
                            self.static_arguments(&call.machine_arguments)?;
                            let native_query = call.target.as_str() == "code_layout"
                                && !call.target_symbol.is_valid()
                                && call.receiver.is_empty();
                            if native_query && self.deferred_machines.is_none() {
                                return Err("late native code-layout metadata is unavailable to same-product semantic constant, type, layout, or optimization evaluation".to_owned());
                            }
                            if !native_query {
                                self.call_dependency(
                                    crate::selection::body_calls::CallSite {
                                        receiver_symbol: call.receiver_symbol,
                                        target_symbol: call.target_symbol,
                                        target: &call.target,
                                        member: &call.target,
                                        requirement_application: arena::Handle::invalid(),
                                        local_receiver: call.receiver.is_empty(),
                                        argument_count: expressions
                                            .expression_handles(call.arguments)
                                            .len(),
                                    },
                                    call.target.source_span(),
                                    &call.machine_arguments,
                                )?;
                            }
                            let arguments = expressions.expression_handles(call.arguments);
                            if arguments.len() != call.arguments.len() {
                                return Err(
                                    "constant statement call has stale arguments".to_owned()
                                );
                            }
                            self.queue_expressions(arguments.iter().rev().copied());
                        }
                        Statement::Transition(transition) => {
                            if let TransitionGuard::When(guard) = &transition.guard {
                                self.queue_expression(*guard);
                            }
                            for target in
                                once(&transition.target).chain(transition.continuation.iter())
                            {
                                match target {
                                    TransitionTarget::Value(value) => self.queue_expression(*value),
                                    TransitionTarget::Named(target) => {
                                        let path = resolved_declarations(program)
                                            .statement_path_members
                                            .span_or_empty(target.path);
                                        let name = path
                                            .last()
                                            .ok_or("constant transition lost its callable name")?;
                                        self.call_dependency(
                                            crate::selection::body_calls::CallSite {
                                                receiver_symbol: target.receiver_symbol,
                                                target_symbol: target.symbol,
                                                target: name,
                                                member: name,
                                                requirement_application: arena::Handle::invalid(),
                                                local_receiver: path.len() == 1,
                                                argument_count: expressions
                                                    .expression_handles(target.arguments)
                                                    .len(),
                                            },
                                            target.source_span,
                                            &[],
                                        )?;
                                        let values =
                                            expressions.expression_handles(target.arguments);
                                        if values.len() != target.arguments.len() {
                                            return Err("constant transition has stale arguments"
                                                .to_owned());
                                        }
                                        self.queue_expressions(values.iter().rev().copied());
                                    }
                                    TransitionTarget::Terminal => {}
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        }
    }
}
mod initializer_leaves {
    /*
        Discover authored initializer leaves without demanding unfinished geometry.

        Discovery precedes dependency scheduling. Array cardinality is checked by
        completed value encoding, after the exact declared extent has evaluated;
        neither the initializer roster nor a probe stand-in supplies that extent.
    */

    use crate::constant::requires_const_initializer_evaluation;
    use crate::preparation::declaration_selection::{
        GenericApplicationSubstitution, has_deferred_const_argument, substituted_generic_arguments,
        type_mentions_parameters,
    };
    use crate::tables::syntax_type_references;
    use source::{SourceSpan, Span};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::identifier::JoinedSyntaxPath;
    use syntax_trees::item::{ConstDefinition, DataDefinition, DataMember};
    use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    /*
        One pending initializer leaf: the authored expression and the exact
        declared carrier it must produce. `structured` leaves are aggregate-producing
        expressions (ordinary calls, matches, or evaluated constant references at a
        closed nominal or fixed-array destination) whose value arrives through the
        checked interpreter rather than the scalar probe evaluator.
    */
    #[derive(Clone, Copy)]
    pub struct PendingConstInitializerLeaf {
        pub expression: syntax_trees::expression::ExpressionHandle,
        pub destination: TypeReferenceHandle,
        pub structured: bool,
    }

    /*
        Whether `type_reference` is a carrier a leaf probe machine can spell: every
        part already closed, no enclosing parameter left in any argument or member
        position, and no deferred const-expression argument whose layout stand-in
        would drift the materialized constructor's instance name.
    */
    fn closed_leaf_carrier(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        substitution: &GenericApplicationSubstitution,
    ) -> bool {
        !type_mentions_parameters(syntax, type_reference, substitution)
            && !has_deferred_const_argument(syntax, type_reference)
    }

    /*
        The member carrier a closed application's bindings assign: a bare `T`
        member type reselects the argument's existing handle. Composite member
        types keep their authored handle — `Generic` and `FixedArray` arms resolve
        their own arguments and const lengths against the same bindings — while
        any other shape still spelling a parameter declines.
    */
    fn effective_member_type(
        syntax: &SyntaxTrees,
        member_type: TypeReferenceHandle,
        substitution: &GenericApplicationSubstitution,
    ) -> Result<TypeReferenceHandle, String> {
        match syntax.type_references.type_reference(member_type) {
            TypeReferenceNode::Named(name) if substitution.contains_key(name.as_str(syntax)) => {
                Ok(substitution[name.as_str(syntax)])
            }
            TypeReferenceNode::Generic { .. } | TypeReferenceNode::FixedArray { .. } => {
                Ok(member_type)
            }
            _ if type_mentions_parameters(syntax, member_type, substitution) => {
                Err("computed constant member type is not yet a closed structural type".to_owned())
            }
            _ => Ok(member_type),
        }
    }

    pub(crate) fn pending_const_initializer_leaves(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        selection: &crate::preparation::declaration_selection::ConstantSelection,
    ) -> Result<Vec<PendingConstInitializerLeaf>, String> {
        use syntax_trees::expression::ExpressionNode;

        fn collect(
            syntax: &SyntaxTrees,
            selection: &crate::preparation::declaration_selection::ConstantSelection,
            expression: syntax_trees::expression::ExpressionHandle,
            type_reference: TypeReferenceHandle,
            substitution: &GenericApplicationSubstitution,
            leaves: &mut Vec<PendingConstInitializerLeaf>,
        ) -> Result<(), String> {
            if let Some(name) = syntax_type_references(syntax)
                .type_reference(type_reference)
                .as_named()
                && let Some(argument) = substitution.get(name.as_str(syntax))
            {
                return collect(
                    syntax,
                    selection,
                    expression,
                    *argument,
                    substitution,
                    leaves,
                );
            }
            let origin = syntax_type_references(syntax).generic_application_origin(type_reference);
            if origin.is_valid() && origin != type_reference {
                return collect(syntax, selection, expression, origin, substitution, leaves);
            }
            match syntax.type_references.type_reference(type_reference) {
            TypeReferenceNode::DynamicTrait { .. }
                if crate::constant::direct_selector_literal(syntax, expression) => Ok(()),
            TypeReferenceNode::FixedArray { element_type, .. } => match syntax.expressions.expression(expression) {
                ExpressionNode::ArrayLiteral(elements) => {
                    let element_type =
                        effective_member_type(syntax, *element_type, substitution)?;
                    let values = syntax.expressions.expression_handles(*elements);
                    if values.len() != elements.len() {
                        return Err("computed array constant has a stale element roster".to_owned());
                    }
                    for element in values {
                        collect(
                            syntax,
                            selection,
                            *element,
                            element_type,
                            substitution,
                            leaves,
                        )?;
                    }
                    Ok(())
                }
                _
                    if closed_leaf_carrier(syntax, type_reference, substitution)
                        && structured_leaf(syntax, selection, expression) =>
                {
                    push_structured_leaf(leaves, expression, type_reference);
                    Ok(())
                }
                _ => Err("computed array constant requires an array literal".to_owned()),
            },
            TypeReferenceNode::Named(name)
                if matches!(
                    selection.builtin_type(syntax, name),
                    Some(symbols::BuiltinTypeAtom::I8 | symbols::BuiltinTypeAtom::I16
                        | symbols::BuiltinTypeAtom::I32 | symbols::BuiltinTypeAtom::I64
                        | symbols::BuiltinTypeAtom::U8 | symbols::BuiltinTypeAtom::U16
                        | symbols::BuiltinTypeAtom::U32 | symbols::BuiltinTypeAtom::U64
                        | symbols::BuiltinTypeAtom::Bool
                        | symbols::BuiltinTypeAtom::F32 | symbols::BuiltinTypeAtom::F64)
                ) =>
            {
                let materialized = match syntax.expressions.expression(expression) {
                    ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
                    ExpressionNode::Float(_) | ExpressionNode::UndeterminedNaN(_) => matches!(selection.builtin_type(syntax, name),
                        Some(symbols::BuiltinTypeAtom::F32 | symbols::BuiltinTypeAtom::F64)),
                    _ => false,
                };
                if !materialized {
                    leaves.push(PendingConstInitializerLeaf {
                        expression,
                        destination: type_reference,
                        structured: false,
                    });
                }
                Ok(())
            }
            TypeReferenceNode::Named(name) => {
                let ExpressionNode::StructLiteral(literal) = syntax.expressions.expression(expression) else {
                    if selection.builtin_type(syntax, name).is_none()
                        && structured_leaf(syntax, selection, expression)
                    {
                        selection.require_closed_data_leaf(syntax, name)?;
                        push_structured_leaf(leaves, expression, type_reference);
                        return Ok(());
                    }
                    return if matches!(syntax.expressions.expression(expression),
                        ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_) | ExpressionNode::String(_)) {
                        Ok(())
                    } else {
                        Err("computed constant leaf requires an exact builtin integer or Boolean carrier".to_owned())
                    };
                };
                let declared = require_closed_data(selection, syntax, name)?;
                collect_literal_fields(
                    syntax,
                    selection,
                    literal,
                    declared,
                    &GenericApplicationSubstitution::new(),
                    leaves,
                )
            }
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                if closed_leaf_carrier(syntax, type_reference, substitution)
                    && structured_leaf(syntax, selection, expression) {
                    push_structured_leaf(leaves, expression, type_reference);
                    return Ok(());
                }
                let declared = selection.data(syntax, base_name)?;
                let argument_handles =
                    syntax.type_references.type_reference_handles(*arguments);
                let parameters = syntax.items.type_parameters(declared.type_parameters);
                if !lifetime_arguments.is_empty()
                    || !declared.lifetime_parameters.is_empty()
                    || argument_handles.len() != arguments.len()
                    || parameters.len() != declared.type_parameters.len()
                    || parameters.len() != argument_handles.len()
                {
                    return Err(
                        "computed constant requires a closed generic application".to_owned(),
                    );
                }
                let next_substitution = substituted_generic_arguments(syntax, parameters, argument_handles, substitution)?;
                match syntax.expressions.expression(expression) {
                    ExpressionNode::StructLiteral(literal) => collect_literal_fields(
                        syntax,
                        selection,
                        literal,
                        declared,
                        &next_substitution,
                        leaves,
                    ),
                    _
                        if closed_leaf_carrier(syntax, type_reference, substitution)
                            && structured_leaf(syntax, selection, expression) =>
                    {
                        push_structured_leaf(leaves, expression, type_reference);
                        Ok(())
                    }
                    ExpressionNode::Call(_) | ExpressionNode::Match(_) => Err(
                        "computed constant call carrier is not a closed generic application"
                            .to_owned(),
                    ),
                    ExpressionNode::Name(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::Boolean(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::String(_) => Ok(()),
                    _ => Err(
                        "computed constant leaf requires an exact builtin integer or Boolean carrier"
                            .to_owned(),
                    ),
                }
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                let start = leaves.len();
                collect(syntax, selection, expression, *base_type, substitution, leaves)?;
                for leaf in &mut leaves[start..] {
                    if leaf.structured && leaf.expression == expression {
                        leaf.destination = type_reference;
                    }
                }
                Ok(())
            }
            _ => Err(
                "computed constant requires a closed array, nominal literal, or builtin scalar carrier"
                    .to_owned(),
            ),
        }
        }

        /*
            A call, match, cast, or evaluated constant reference is one structured leaf at
            a closed destination: the checked interpreter supplies its result and
            admission checks it against the exact destination carrier. A bare case
            or an unresolved spelling is not this leaf and keeps its literal route.
        */
        fn structured_leaf(
            syntax: &SyntaxTrees,
            selection: &crate::preparation::declaration_selection::ConstantSelection,
            expression: syntax_trees::expression::ExpressionHandle,
        ) -> bool {
            match syntax.expressions.expression(expression) {
                ExpressionNode::Call(_) | ExpressionNode::Match(_) | ExpressionNode::Cast(_) => {
                    true
                }
                ExpressionNode::Name(path) => name_selects_const(syntax, selection, *path),
                _ => false,
            }
        }

        fn push_structured_leaf(
            leaves: &mut Vec<PendingConstInitializerLeaf>,
            expression: syntax_trees::expression::ExpressionHandle,
            destination: TypeReferenceHandle,
        ) {
            leaves.push(PendingConstInitializerLeaf {
                expression,
                destination,
                structured: true,
            });
        }

        fn same_constructor_application(
            syntax: &SyntaxTrees,
            selection: &crate::preparation::declaration_selection::ConstantSelection,
            declared: &DataDefinition,
            constructed: &DataDefinition,
            substitution: &GenericApplicationSubstitution,
        ) -> bool {
            use crate::preparation::generic_data::{
                ClosedArgumentIdentity, closed_argument_identity,
                closed_parameter_argument_identity,
            };
            if std::ptr::eq(declared, constructed) {
                return true;
            }
            let Some(origin) = constructed.generic_instance else {
                return false;
            };
            let Some(ClosedArgumentIdentity::Instance(template, actual)) =
                closed_argument_identity(syntax, Some(selection), origin, false)
            else {
                return false;
            };
            if !syntax
                .root_item(template)
                .as_data()
                .is_some_and(|template| std::ptr::eq(template, declared))
            {
                return false;
            }
            let expected = syntax
                .items
                .type_parameters(declared.type_parameters)
                .iter()
                .map(|parameter| {
                    let argument = *substitution.get(parameter.name.as_str(syntax))?;
                    closed_parameter_argument_identity(
                        syntax,
                        Some(selection),
                        argument,
                        &parameter.kind,
                    )
                })
                .collect::<Option<Vec<_>>>();
            expected.as_ref() == Some(&actual)
        }

        fn collect_literal_fields(
            syntax: &SyntaxTrees,
            selection: &crate::preparation::declaration_selection::ConstantSelection,
            literal: &syntax_trees::expression::TableStructLiteral,
            declared: &DataDefinition,
            substitution: &GenericApplicationSubstitution,
            leaves: &mut Vec<PendingConstInitializerLeaf>,
        ) -> Result<(), String> {
            let (constructed, case) = selection.constructor(syntax, &literal.constructor_name)?;
            if !same_constructor_application(syntax, selection, declared, constructed, substitution)
            {
                return Err(
                    "computed constant constructor differs from its declared nominal carrier"
                        .to_owned(),
                );
            }
            let members = syntax.items.data_members(declared.members);
            if members.len() != declared.members.len() {
                return Err("computed constant data member span is stale".to_owned());
            }
            let fields = if let Some(case) = case {
                let variant = members
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant)
                            if variant.name.as_str(syntax) == case.as_str(syntax) =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or("computed constant case is absent from its selected owner")?;
                let fields = syntax.items.data_payload_fields(variant.payload);
                if fields.len() != variant.payload.len() {
                    return Err("computed constant payload span is stale".to_owned());
                }
                fields.iter().collect::<Vec<_>>()
            } else {
                if members
                    .iter()
                    .any(|member| matches!(member, DataMember::Variant(_)))
                {
                    return Err("computed constant sum must select its exact case".to_owned());
                }
                members
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field),
                        _ => None,
                    })
                    .collect()
            };
            let authored = syntax.expressions.struct_fields(literal.fields);
            if authored.len() != literal.fields.len() || authored.len() != fields.len() {
                return Err(
                    "computed constant field roster differs from its declaration".to_owned(),
                );
            }
            for field in authored {
                if authored
                    .iter()
                    .filter(|candidate| candidate.name.as_str(syntax) == field.name.as_str(syntax))
                    .count()
                    != 1
                {
                    return Err(format!(
                        "duplicate computed constant field `{}`",
                        field.name.as_str(syntax)
                    ));
                }
                let declaration = fields
                    .iter()
                    .find(|candidate| candidate.name.as_str(syntax) == field.name.as_str(syntax))
                    .ok_or_else(|| {
                        format!(
                            "unknown computed constant field `{}`",
                            field.name.as_str(syntax)
                        )
                    })?;
                let member_type =
                    effective_member_type(syntax, declaration.type_reference, substitution)?;
                collect(
                    syntax,
                    selection,
                    field.value,
                    member_type,
                    substitution,
                    leaves,
                )?;
            }
            Ok(())
        }

        let mut leaves = Vec::new();
        if !requires_const_initializer_evaluation(syntax, definition) {
            return Ok(leaves);
        }
        collect(
            syntax,
            selection,
            definition.value,
            definition.type_reference,
            &GenericApplicationSubstitution::new(),
            &mut leaves,
        )?;
        Ok(leaves)
    }

    /*
        The selected declaration for a computed nominal constant must be closed:
        generic or lifetime-carrying owners cannot produce one materialized value.
    */
    fn require_closed_data<'syntax>(
        selection: &crate::preparation::declaration_selection::ConstantSelection,
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
    ) -> Result<&'syntax syntax_trees::item::DataDefinition, String> {
        let declared = selection.data(syntax, name)?;
        if !declared.type_parameters.is_empty() || !declared.lifetime_parameters.is_empty() {
            return Err(
                "computed nominal constant requires a closed selected declaration".to_owned(),
            );
        }
        Ok(declared)
    }

    /*
        Whether one authored name path selects an exact constant declaration from
        its own source scope, distinguishing an evaluated value reference from a
        literal case or constructor spelling.
    */
    fn name_selects_const(
        syntax: &SyntaxTrees,
        selection: &crate::preparation::declaration_selection::ConstantSelection,
        path: arena::HandleSpan<Identifier>,
    ) -> bool {
        let members = syntax.expressions.identifier_path_members(path);
        let (Some(first), Some(last)) = (members.first(), members.last()) else {
            return false;
        };
        let reference = SourceSpan::new(
            first.source_span().source_id,
            Span::new(first.source_span().span.start, last.source_span().span.end),
        );
        let name = members.joined_path_in(syntax);
        selection.selects_const(syntax, &Identifier::new(name, reference))
    }
}
pub(crate) mod initializer_normalization {
    /*
        Rejoin declaration-owned folds before any use copies their materialized value.

        The semantic evaluator establishes the result and builtin meanings. Resolution
        independently checks the retained scalar payload, source roots, and complete
        selected dependency/operator roster. Original roots still undergo ordinary
        declaration-source resolution; initializer implementation exposure never
        becomes public index exposure just because its constant is public.
    */

    use super::initializer_dependencies::DependencyPurpose;
    use crate::lowering::expression::lower_expression_into_table;
    use crate::selection::call_identity::selected_declaration_source;
    use crate::tables::body_expressions;
    use crate::tables::syntax_expressions;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure,
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionLateBinding as LateBinding,
        AuthoredDeclarationSelectionTarget as Target,
    };
    use source::SourceSpan;
    use symbol_resolved_trees::{SymbolResolvedTrees, expression::ExpressionHandle};
    use symbols::SymbolKind;
    use syntax_trees::identifier::JoinedSyntaxPath;
    use syntax_trees::item::ConstInitializerCustody;
    use syntax_trees::{SyntaxTrees, item::ConstDefinition, types::ConstArgumentOrigin};

    mod resolved_payload {
        /*
            Independently encode a materialized initializer after declaration binding.

            Current and retained declarations share resolved identities here. No source
            header is reconstructed, and the receipt supplies neither fields nor types.
            The original declared carrier remains intact; only authenticated explicit
            storage representations project their semantic value for payload encoding.
        */

        use language_core::constant_value::EvaluatedConstant;
        use language_semantics::DataSupplyMode;
        use language_semantics::const_value::{
            CanonicalConstIdentity, DecodedCanonicalConstValue as Value,
        };
        use numerics::literals::FloatFormat;
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbol_resolved_trees::data::{DataDefinition, DataField, DataMember};
        use symbol_resolved_trees::expression::{
            ExpressionHandle, ExpressionNode, TableStructLiteralField,
        };
        use symbol_resolved_trees::types::{FixedArrayLength, TypeReference};
        use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};

        #[derive(Clone, Copy)]
        struct Operand<'program> {
            reference: &'program TypeReference,
            frame: usize,
        }

        struct Binding<'program> {
            parameter: SymbolHandle,
            actual: Operand<'program>,
        }

        struct Encoder<'program> {
            program: &'program SymbolResolvedTrees,
            bindings: Vec<Binding<'program>>,
        }

        pub(super) fn validate(
            program: &SymbolResolvedTrees,
            destination: &TypeReference,
            materialized: ExpressionHandle,
            expected: &EvaluatedConstant,
        ) -> Result<(), String> {
            let mut encoder = Encoder {
                program,
                bindings: Vec::new(),
            };
            let operand = Operand {
                reference: destination,
                frame: 0,
            };
            let actual = if let ExpressionNode::UndeterminedNaN(format) =
                program.tables.bodies.expressions.expression(materialized)
            {
                let operand = encoder.unconstrained(operand)?;
                let TypeReference::Named { symbol, .. } = operand.reference else {
                    return Err("undetermined NaN lost its declared floating carrier".into());
                };
                if float_format(program.symbols.builtin_type_atom(*symbol)) != Some(*format) {
                    return Err(
                        "undetermined NaN conflicts with its declared floating carrier".into(),
                    );
                }
                EvaluatedConstant::UndeterminedNaN(*format)
            } else {
                let value = encoder.value(operand, materialized, 0)?;
                let identity = CanonicalConstIdentity::from_declaration_value(&value).ok_or(
                    "materialized initializer exceeds canonical declaration encoding limits",
                )?;
                EvaluatedConstant::Encoded(identity.encoding)
            };
            if &actual != expected {
                return Err("materialized value drifted from its canonical result".into());
            }
            Ok(())
        }

        impl<'program> Encoder<'program> {
            fn resolve(&self, mut operand: Operand<'program>) -> Result<Operand<'program>, String> {
                for _ in 0..=self.bindings.len() {
                    let TypeReference::Named { symbol, .. } = operand.reference else {
                        return Ok(operand);
                    };
                    let Some(binding) = self.bindings[..operand.frame]
                        .iter()
                        .rev()
                        .find(|binding| binding.parameter == *symbol)
                    else {
                        return Ok(operand);
                    };
                    operand = binding.actual;
                }
                Err("materialized initializer has cyclic declaration arguments".into())
            }

            fn unconstrained(
                &self,
                mut operand: Operand<'program>,
            ) -> Result<Operand<'program>, String> {
                loop {
                    operand = self.resolve(operand)?;
                    operand.reference = match operand.reference {
                        TypeReference::Constrained(constraint) => {
                            self.program.child_type_reference(constraint.base_type)
                        }
                        TypeReference::AliasUse { expansion, .. } => expansion,
                        _ => return Ok(operand),
                    };
                }
            }

            fn value(
                &mut self,
                operand: Operand<'program>,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Result<Value, String> {
                if depth >= 64 {
                    return Err(
                        "materialized initializer exceeds canonical declaration depth".into(),
                    );
                }
                let operand = self.unconstrained(operand)?;
                let program = self.program;
                if let TypeReference::Generic(application) = operand.reference
                    && let Some(definition) =
                        program.data_definition_by_symbol(application.base_symbol)
                    && is_layout_storage(program, definition)
                {
                    let arguments = program.child_type_references(application.arguments);
                    if arguments.len() != 2 || !application.lifetime_arguments.is_empty() {
                        return Err(
                            "explicit storage initializer lost its complete application".into()
                        );
                    }
                    return self.value(
                        Operand {
                            reference: &arguments[0],
                            frame: operand.frame,
                        },
                        expression,
                        depth + 1,
                    );
                }
                let table = &program.tables.bodies.expressions;
                match operand.reference {
                    TypeReference::FixedArray(array) => {
                        let ExpressionNode::ArrayLiteral(elements) = table.expression(expression)
                        else {
                            return Err("materialized initializer is not its declared array".into());
                        };
                        let length = self.array_length(&array.length, operand.frame)?;
                        let elements = table.expression_handles(*elements);
                        if elements.len() != length {
                            return Err("materialized initializer array length drifted".into());
                        }
                        let element = Operand {
                            reference: program.child_type_reference(array.element_type),
                            frame: operand.frame,
                        };
                        let type_name = format!("[{}; {length}]", self.label(element, depth + 1)?);
                        let values = elements
                            .iter()
                            .map(|element_expression| {
                                self.value(element, *element_expression, depth + 1)
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(Value::Array { type_name, values })
                    }
                    TypeReference::Named { symbol, .. }
                        if program.symbols.builtin_type_atom(*symbol).is_some() =>
                    {
                        self.scalar(
                            program
                                .symbols
                                .builtin_type_atom(*symbol)
                                .ok_or("scalar initializer lost builtin identity")?,
                            expression,
                        )
                    }
                    TypeReference::Named { symbol, .. } => {
                        let definition = program
                            .data_definition_by_symbol(*symbol)
                            .ok_or("materialized initializer lost its selected data declaration")?;
                        self.nominal(
                            definition,
                            None,
                            operand.frame,
                            definition.name.as_str().to_owned(),
                            expression,
                            depth,
                        )
                    }
                    TypeReference::Generic(application) => {
                        let definition = program
                    .data_definition_by_symbol(application.base_symbol)
                    .ok_or("materialized initializer lost its selected generic data declaration")?;
                        let label = self.label(operand, depth + 1)?;
                        self.nominal(
                            definition,
                            Some(application),
                            operand.frame,
                            label,
                            expression,
                            depth,
                        )
                    }
                    _ => {
                        Err("materialized initializer has no canonical declaration carrier".into())
                    }
                }
            }

            fn nominal(
                &mut self,
                definition: &'program DataDefinition,
                application: Option<&'program symbol_resolved_trees::types::GenericTypeReference>,
                frame: usize,
                label: String,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Result<Value, String> {
                let program = self.program;
                let actual_type = match program.tables.bodies.expressions.expression(expression) {
                    ExpressionNode::StructLiteral(literal) => literal.type_symbol,
                    ExpressionNode::Name(path)
                        if program.symbols.get(path.symbol).kind == SymbolKind::Variant =>
                    {
                        program.symbols.get(path.symbol).parent
                    }
                    _ => return Err("materialized initializer lost its nominal constructor".into()),
                };
                let (definition, application) = if actual_type == definition.symbol {
                    (definition, application)
                } else if let Some(TypeReference::Generic(origin)) = &definition.generic_instance {
                    if actual_type != origin.base_symbol {
                        return Err(
                            "materialized initializer selects a different nominal instance".into(),
                        );
                    }
                    (
                        program
                            .data_definition_by_symbol(origin.base_symbol)
                            .ok_or("materialized instance lost its template declaration")?,
                        Some(origin),
                    )
                } else {
                    return Err(
                        "materialized initializer selects a different nominal carrier".into(),
                    );
                };
                if definition.supply_mode != DataSupplyMode::CheckedShape
                    || definition.quotient.is_some()
                {
                    return Err(
                        "materialized initializer has no ordinary source-owned constructor".into(),
                    );
                }
                let start = self.bindings.len();
                let parameters = program.data_type_parameters(definition.type_parameters);
                if let Some(application) = application {
                    let arguments = program.child_type_references(application.arguments);
                    if arguments.len() != parameters.len()
                        || application.lifetime_arguments.len()
                            != definition.lifetime_parameters.len()
                    {
                        return Err(
                            "materialized initializer lost its complete generic application".into(),
                        );
                    }
                    for (parameter, argument) in parameters.iter().zip(arguments) {
                        self.bindings.push(Binding {
                            parameter: parameter.symbol,
                            actual: Operand {
                                reference: argument,
                                frame,
                            },
                        });
                    }
                } else if !parameters.is_empty() || !definition.lifetime_parameters.is_empty() {
                    return Err(
                        "materialized initializer requires a closed declaration application".into(),
                    );
                }
                let bound_frame = self.bindings.len();
                let result = (|| {
                    let table = &program.tables.bodies.expressions;
                    let (actual_type, case, supplied) = match table.expression(expression) {
                        ExpressionNode::StructLiteral(literal) => (
                            literal.type_symbol,
                            literal.case_symbol,
                            table.struct_fields(literal.fields),
                        ),
                        ExpressionNode::Name(path)
                            if program.symbols.get(path.symbol).kind == SymbolKind::Variant =>
                        {
                            (
                                program.symbols.get(path.symbol).parent,
                                Some(path.symbol),
                                &[][..],
                            )
                        }
                        _ => {
                            return Err(
                                "materialized initializer lost its nominal constructor".into()
                            );
                        }
                    };
                    if actual_type != definition.symbol {
                        return Err(
                            "materialized initializer selects a different nominal carrier".into(),
                        );
                    }
                    let members = program.data_members(definition.members);
                    if let Some(case) = case {
                        let variant = members
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant) if variant.symbol == case => {
                                    Some(variant)
                                }
                                _ => None,
                            })
                            .ok_or(
                                "materialized initializer case belongs to a different declaration",
                            )?;
                        let fields = program
                            .data_payload_fields(variant.payload)
                            .iter()
                            .collect::<Vec<_>>();
                        Ok(Value::Variant {
                            type_name: label,
                            case_name: variant.name.as_str().to_owned(),
                            fields: self.fields(&fields, supplied, bound_frame, depth + 1)?,
                        })
                    } else {
                        let fields = members
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Field(field) => Some(field),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        if members
                            .iter()
                            .any(|member| matches!(member, DataMember::Variant(_)))
                        {
                            return Err("materialized initializer omitted its declared case".into());
                        }
                        Ok(Value::Record {
                            type_name: label,
                            fields: self.fields(&fields, supplied, bound_frame, depth + 1)?,
                        })
                    }
                })();
                self.bindings.truncate(start);
                result
            }

            fn fields(
                &mut self,
                declared: &[&'program DataField],
                supplied: &'program [TableStructLiteralField],
                frame: usize,
                depth: usize,
            ) -> Result<Vec<(String, Value)>, String> {
                if declared.len() != supplied.len() {
                    return Err("materialized initializer field roster drifted".into());
                }
                declared
                    .iter()
                    .map(|field| {
                        let mut matches = supplied
                            .iter()
                            .filter(|actual| actual.field_symbol == field.symbol);
                        let actual = matches.next().ok_or_else(|| {
                            format!("materialized initializer lost field `{}`", field.name)
                        })?;
                        if matches.next().is_some() {
                            return Err("materialized initializer repeats a declared field".into());
                        }
                        Ok((
                            field.name.as_str().to_owned(),
                            self.value(
                                Operand {
                                    reference: &field.type_reference,
                                    frame,
                                },
                                actual.value,
                                depth,
                            )?,
                        ))
                    })
                    .collect()
            }

            fn scalar(
                &self,
                atom: BuiltinTypeAtom,
                expression: ExpressionHandle,
            ) -> Result<Value, String> {
                let node = self
                    .program
                    .tables
                    .bodies
                    .expressions
                    .expression(expression);
                if atom == BuiltinTypeAtom::Bool {
                    return match node {
                        ExpressionNode::Boolean(value) => Ok(Value::Boolean(*value)),
                        _ => Err("materialized initializer conflicts with bool".into()),
                    };
                }
                if let Some(format) = float_format(Some(atom)) {
                    let literal = match node {
                        ExpressionNode::Float(literal) => literal.clone(),
                        ExpressionNode::Integer(literal) if literal.landing().is_none() => {
                            numerics::literals::FloatLiteral::parse(
                                &literal
                                    .value_bignum()
                                    .ok_or(
                                        "materialized floating initializer lost its integer value",
                                    )?
                                    .to_string(),
                            )
                            .ok_or("materialized floating initializer is not exact")?
                        }
                        _ => {
                            return Err(
                                "materialized initializer conflicts with floating carrier".into()
                            );
                        }
                    };
                    if literal.landing().is_some_and(|actual| actual != format) {
                        return Err(
                            "materialized initializer conflicts with floating landing".into()
                        );
                    }
                    let bits =
                match format {
                    FloatFormat::F32
                        if literal.presented_bits().is_some() || !literal.value_f32().is_nan() =>
                    {
                        u64::from(literal.value_f32().to_bits())
                    }
                    FloatFormat::F64
                        if literal.presented_bits().is_some() || !literal.value_f64().is_nan() =>
                    {
                        literal.value_f64().to_bits()
                    }
                    _ => return Err(
                        "materialized floating initializer lacks determined representation bits"
                            .into(),
                    ),
                };
                    return Ok(Value::Float { format, bits });
                }
                let carrier = (atom.is_fixed_integer() || atom == BuiltinTypeAtom::Address)
                    .then_some(atom.symbol_name())
                    .ok_or("materialized initializer has an unsupported builtin carrier")?;
                let ExpressionNode::Integer(literal) = node else {
                    return Err("materialized initializer conflicts with integer carrier".into());
                };
                if literal.landing().is_some_and(|landing| {
                    landing.landed_type.name() != carrier
                        || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
                }) {
                    return Err("materialized initializer conflicts with integer landing".into());
                }
                let value = literal
                    .value_i64()
                    .map(i128::from)
                    .or_else(|| literal.value_u64().map(i128::from))
                    .ok_or("materialized initializer integer exceeds canonical storage")?;
                let integer_type = numerics::literals::LandedIntegerType::from_name(carrier)
                    .ok_or("materialized integer initializer lost its selected carrier")?;
                let (minimum, maximum) = integer_type.bounds();
                if value < minimum || value > maximum {
                    return Err(format!(
                        "materialized const value `{value}` does not fit `{carrier}`"
                    ));
                }
                Ok(Value::Integer {
                    type_name: carrier.to_owned(),
                    value,
                })
            }

            fn array_length(
                &self,
                length: &FixedArrayLength,
                frame: usize,
            ) -> Result<usize, String> {
                match length {
                    FixedArrayLength::Literal(length) => Ok(*length),
                    FixedArrayLength::Expression(argument) => {
                        let TypeReference::ConstExpression(value) =
                            self.program.child_type_reference(*argument)
                        else {
                            return Err("materialized array extent is not a completed value".into());
                        };
                        let ExpressionNode::Integer(literal) = self
                            .program
                            .tables
                            .bodies
                            .expressions
                            .expression(value.expression)
                        else {
                            return Err(
                                "materialized array extent is not a completed integer literal"
                                    .into(),
                            );
                        };
                        literal
                            .value_u64()
                            .and_then(|value| usize::try_from(value).ok())
                            .ok_or_else(|| {
                                "materialized array extent is not a nonnegative host-sized integer"
                                    .into()
                            })
                    }
                    FixedArrayLength::ConstParameter { symbol, .. } => {
                        if symbol.is_valid()
                            && self.program.symbols.get(*symbol).kind == SymbolKind::Const
                        {
                            let (_, value) = self.declaration_integer(*symbol)?;
                            return usize::try_from(value).map_err(|_| {
                        "materialized array extent is not a nonnegative host-sized integer".into()
                    });
                        }
                        let binding = self.bindings[..frame]
                            .iter()
                            .rev()
                            .find(|binding| binding.parameter == *symbol)
                            .ok_or("materialized array extent has no exact declaration binding")?;
                        let argument = self.resolve(binding.actual)?;
                        let TypeReference::Named { symbol, name, .. } = argument.reference else {
                            return Err(
                                "materialized array extent is not a completed integer".into()
                            );
                        };
                        if symbol.is_valid()
                            && self.program.symbols.get(*symbol).kind == SymbolKind::Const
                        {
                            let (_, value) = self.declaration_integer(*symbol)?;
                            return usize::try_from(value).map_err(|_| {
                        "materialized array extent is not a nonnegative host-sized integer".into()
                    });
                        }
                        name.as_str().parse().map_err(|_| {
                            "materialized array extent is not a closed nonnegative integer".into()
                        })
                    }
                    _ => Err("materialized array extent is not complete".into()),
                }
            }

            /*
                An extent's selected declaration owns both its integer carrier and
                completed initializer. Never read a same-spelled caller constant or a
                provisional receipt encoding to determine geometry.
            */
            fn declaration_integer(
                &self,
                mut symbol: SymbolHandle,
            ) -> Result<(BuiltinTypeAtom, i128), String> {
                let mut active = Vec::new();
                let mut declared_carrier = None;
                loop {
                    if active.contains(&symbol) {
                        return Err(
                            "materialized array extent has a cyclic constant dependency".into()
                        );
                    }
                    active.push(symbol);
                    let declaration = self
                        .program
                        .const_declaration(symbol)
                        .ok_or("materialized array extent lost its exact constant declaration")?;
                    let carrier = self.unconstrained(Operand {
                        reference: &declaration.declared_type,
                        frame: 0,
                    })?;
                    let TypeReference::Named {
                        symbol: carrier_symbol,
                        ..
                    } = carrier.reference
                    else {
                        return Err(
                            "materialized array extent constant must have an integer carrier"
                                .into(),
                        );
                    };
                    let atom = self
                        .program
                        .symbols
                        .builtin_type_atom(*carrier_symbol)
                        .filter(|atom| atom.is_fixed_integer())
                        .ok_or("materialized array extent constant must have an integer carrier")?;
                    if declared_carrier.is_some_and(|expected| expected != atom) {
                        return Err(
                            "materialized array extent alias changes its declared integer carrier"
                                .into(),
                        );
                    }
                    declared_carrier = Some(atom);
                    let table = &self.program.tables.bodies.expressions;
                    if let ExpressionNode::Name(path) = table.expression(declaration.initializer)
                        && path.symbol.is_valid()
                        && self.program.symbols.get(path.symbol).kind == SymbolKind::Const
                    {
                        symbol = path.symbol;
                        continue;
                    }
                    return match self.scalar(atom, declaration.initializer)? {
                        Value::Integer { value, .. } => Ok((atom, value)),
                        _ => Err(
                            "materialized array extent constant is not a completed integer".into(),
                        ),
                    };
                }
            }

            fn label(&self, operand: Operand<'program>, depth: usize) -> Result<String, String> {
                if depth >= 64 {
                    return Err("materialized carrier label exceeds canonical depth".into());
                }
                let operand = self.unconstrained(operand)?;
                match operand.reference {
                    TypeReference::ConstExpression(recipe) => {
                        match self
                            .program
                            .tables
                            .bodies
                            .expressions
                            .expression(recipe.expression)
                        {
                            ExpressionNode::Integer(literal) => literal
                                .value_bignum()
                                .map(|value| value.to_string())
                                .ok_or_else(|| {
                                    "materialized carrier argument has no integer value".into()
                                }),
                            _ => {
                                Err("materialized carrier argument is not a completed scalar"
                                    .into())
                            }
                        }
                    }
                    TypeReference::Named { symbol, name, .. } => {
                        Ok(self.program.data_definition_by_symbol(*symbol).map_or_else(
                            || name.as_str().to_owned(),
                            |definition| definition.name.as_str().to_owned(),
                        ))
                    }
                    TypeReference::FixedArray(array) => Ok(format!(
                        "[{}; {}]",
                        self.label(
                            Operand {
                                reference: self.program.child_type_reference(array.element_type),
                                frame: operand.frame
                            },
                            depth + 1
                        )?,
                        self.array_length(&array.length, operand.frame)?
                    )),
                    TypeReference::Generic(application) => {
                        let name = self
                            .program
                            .data_definition_by_symbol(application.base_symbol)
                            .map_or_else(
                                || self.program.symbols.name(application.base_symbol),
                                |definition| definition.name.as_str(),
                            );
                        let arguments = self
                            .program
                            .child_type_references(application.arguments)
                            .iter()
                            .map(|reference| {
                                self.label(
                                    Operand {
                                        reference,
                                        frame: operand.frame,
                                    },
                                    depth + 1,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(format!("{name}<{}>", arguments.join(", ")))
                    }
                    _ => Err("materialized initializer carrier has no canonical label".into()),
                }
            }
        }

        fn is_layout_storage(program: &SymbolResolvedTrees, definition: &DataDefinition) -> bool {
            program
                .symbols
                .symbol_source_span(definition.symbol)
                .is_some_and(|reference| {
                    crate::preparation::declaration_selection::is_core_layout_storage_source(
                        &program.symbols,
                        definition.name.as_str(),
                        definition.supply_mode,
                        reference,
                    )
                })
        }

        fn float_format(atom: Option<BuiltinTypeAtom>) -> Option<FloatFormat> {
            match atom {
                Some(BuiltinTypeAtom::F32) => Some(FloatFormat::F32),
                Some(BuiltinTypeAtom::F64) => Some(FloatFormat::F64),
                _ => None,
            }
        }
    }

    /*
        The three existing initializer inventories: original value use, complete
        declaration preparation, and the materialized value's realized selections.
    */
    #[derive(Clone, Copy)]
    pub enum InitializerCustodyPurpose {
        Authored,
        Declaration,
        Value,
    }

    /*
        Read a completed predecessor's original graph, never its source syntax or
        a guessed summary. The receiving evaluator still checks every new use.
    */
    pub(crate) fn retained_constant_custody(
        program: &SymbolResolvedTrees,
        owner: &symbol_resolved_trees::constant::ConstDeclaration,
        purpose: InitializerCustodyPurpose,
    ) -> Result<ConstInitializerCustody, Diagnostic> {
        use super::initializer_dependencies::{DependencyPurpose, collect_declaration};
        let source = program
            .symbols
            .symbol_source_span(owner.symbol)
            .ok_or_else(|| {
                error(
                    owner.initializer_source_span,
                    "retained constant lost its declaration source",
                )
            })?;
        let mut custody = ConstInitializerCustody::default();
        if matches!(purpose, InitializerCustodyPurpose::Value) {
            append_retained_custody(
                program,
                owner,
                source,
                &mut custody,
                DependencyPurpose::DeclarationReadiness,
            )?;
            return Ok(custody);
        }
        let mut retained = ConstInitializerCustody::default();
        append_retained_custody(
            program,
            owner,
            source,
            &mut retained,
            DependencyPurpose::Authored,
        )?;
        let dependencies = collect_declaration(
            program,
            if owner.authored_initializer.is_valid() {
                owner.authored_initializer
            } else {
                owner.initializer
            },
            &owner.declared_type,
            if matches!(purpose, InitializerCustodyPurpose::Authored) {
                DependencyPurpose::Authored
            } else {
                DependencyPurpose::DeclarationReadiness
            },
            None,
            Some(&retained.call_selections),
        )
        .map_err(|reason| error(source, &reason))?;
        if !dependencies.pending_calls.is_empty() {
            return Err(error(
                source,
                "retained initializer has unresolved source call choices",
            ));
        }
        custody.builtin_operators = dependencies.initializer_operators;
        custody.call_selections = dependencies.calls;
        for (reference, declaration_source) in dependencies.constants {
            let declaration = program
                .const_declaration_at_source_span(declaration_source)
                .ok_or_else(|| {
                    error(
                        reference,
                        "retained constant dependency lost its declaration",
                    )
                })?;
            if !declaration.is_public
                && !program
                    .symbols
                    .same_source_package(reference, declaration_source)
            {
                return Err(error(
                    reference,
                    "retained constant selects a private foreign declaration",
                ));
            }
            let origin = ConstArgumentOrigin {
                reference,
                declaration: declaration_source,
                initializer: declaration.initializer_source_span,
                evaluated_value: declaration.evaluated_value.clone().ok_or_else(|| {
                    error(reference, "retained dependency has no completed value")
                })?,
            };
            if !custody.selections.contains(&origin) {
                custody.selections.push(origin);
            }
        }
        Ok(custody)
    }

    pub(crate) struct PendingInitializer {
        declaration: SourceSpan,
        materialized: ExpressionHandle,
        pub(crate) authored: ExpressionHandle,
        references: Vec<(SourceSpan, String)>,
        operators: Vec<SourceSpan>,
        receipt: syntax_trees::item::ConstInitializerNormalization,
    }

    /*
        The exact inventory validated before constant substitution. Substitution may
        inherit another declaration's realized rows; this owner rejoins the same
        root afterward without re-resolving or manufacturing occurrence identities.
    */
    pub(crate) struct InitializerRootCustody {
        root: ExpressionHandle,
        reference: SourceSpan,
        declaration: SourceSpan,
        occurrences: Vec<symbol_resolved_trees::AuthoredDeclarationSelectionOccurrenceId>,
    }

    pub(crate) fn rejoin_after_substitution(
        program: &mut SymbolResolvedTrees,
        custody: &[InitializerRootCustody],
    ) -> Result<(), Diagnostic> {
        for owner in custody {
            replace_initializer_lanes(program, owner)?;
        }
        Ok(())
    }

    fn error(reference: SourceSpan, reason: &str) -> Diagnostic {
        Diagnostic::error(format!("constant initializer normalization: {reason}"))
            .with_source_span(reference)
    }

    pub(crate) fn retain(
        lowerer: &mut crate::resolution::lowerer::Lowerer,
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        materialized: ExpressionHandle,
    ) -> Result<(), Diagnostic> {
        let Some(receipt) = &definition.normalization else {
            return Ok(());
        };
        let reference = syntax.expressions.source_span(definition.value);
        if !syntax_expressions(syntax).contains_expression(receipt.authored_expression)
            || receipt.authored_expression == definition.value
            || syntax.expressions.source_span(receipt.authored_expression) != reference
        {
            return Err(error(reference, "lost its exact authored expression"));
        }
        if definition
            .pending_qualification
            .as_ref()
            .is_some_and(|obligation| {
                obligation.declaration != definition.name.source_span()
                    || obligation.initializer != definition.value
            })
            || definition
                .qualification
                .as_ref()
                .is_some_and(|qualification| {
                    qualification.destination != definition.type_reference
                        || qualification.initializer != definition.value
                })
        {
            return Err(error(
                reference,
                "initializer lost its exact destination obligation",
            ));
        }
        let mut references = Vec::new();
        let mut operators = Vec::new();
        let mut pending = vec![receipt.authored_expression];
        let mut visited = Vec::new();
        while let Some(expression) = pending.pop() {
            if !syntax.expressions.contains_expression(expression) {
                return Err(error(reference, "authored expression has an invalid child"));
            }
            if visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            use syntax_trees::expression::{ExpressionNode, MatchPattern};
            match syntax.expressions.expression(expression) {
                ExpressionNode::Integer(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::String(_) => {}
                ExpressionNode::Name(path) => {
                    let members = syntax.expressions.identifier_path_members(*path);
                    let (Some(first), Some(last)) = (members.first(), members.last()) else {
                        return Err(error(reference, "authored dependency lost its name"));
                    };
                    let reference = SourceSpan::new(
                        first.source_span().source_id,
                        source::Span::new(
                            first.source_span().span.start,
                            last.source_span().span.end,
                        ),
                    );
                    let name = members.joined_path_in(syntax);
                    let identifier =
                        syntax_trees::identifier::Identifier::new(name.clone(), reference);
                    if lowerer
                        .constant_selection
                        .as_ref()
                        .is_some_and(|selection| {
                            selection
                                .qualified_payload_free_case(syntax, &identifier)
                                .is_ok_and(|selected| selected.is_some())
                        })
                    {
                        continue;
                    }
                    references.push((reference, name));
                }
                ExpressionNode::Binary(binary) => {
                    operators.push(syntax.expressions.source_span(expression));
                    pending.push(binary.right);
                    pending.push(binary.left);
                }
                ExpressionNode::Member(member) => pending.push(member.receiver),
                ExpressionNode::Unary(unary) => {
                    operators.push(syntax.expressions.source_span(expression));
                    pending.push(unary.operand);
                }
                ExpressionNode::Cast(cast) => pending.push(cast.value),
                ExpressionNode::Call(call) => {
                    let arguments = syntax.expressions.expression_handles(call.arguments);
                    if arguments.len() != call.arguments.len() {
                        return Err(error(reference, "authored call has a stale argument span"));
                    }
                    pending.extend(arguments.iter().rev().copied());
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    pending.extend(
                        syntax_expressions(syntax)
                            .expression_handles(*elements)
                            .iter()
                            .rev()
                            .copied(),
                    );
                }
                ExpressionNode::StructLiteral(literal) => {
                    let fields = syntax.expressions.struct_fields(literal.fields);
                    if fields.len() != literal.fields.len() {
                        return Err(error(
                            reference,
                            "authored constructor has a stale field span",
                        ));
                    }
                    pending.extend(fields.iter().rev().map(|field| field.value));
                }
                ExpressionNode::Match(dispatch) => {
                    for arm in syntax.expressions.match_arms(dispatch.arms).iter().rev() {
                        pending.push(arm.value);
                        if let MatchPattern::Value(pattern) = arm.pattern {
                            pending.push(pattern);
                        }
                    }
                    pending.push(dispatch.subject);
                }
                _ => {
                    return Err(error(
                        reference,
                        "authored expression is outside constant leaf evaluation",
                    ));
                }
            }
        }
        let original = lowerer
            .with_authored_expression_exposure(Exposure::PrivateImplementation, |lowerer| {
                lower_expression_into_table(lowerer, syntax, receipt.authored_expression)
            })?;
        lowerer.pending_const_values.push(original);
        lowerer.pending_const_initializers.push(PendingInitializer {
            declaration: definition.name.source_span(),
            materialized,
            authored: original,
            references,
            operators,
            receipt: receipt.clone(),
        });
        Ok(())
    }

    /*
        Reconstruct the transitive roster from original references, not receipt
        membership. Even omitted or unused dependencies must rejoin current values.
        Rejoin the receipt to its exact retained owner before traversing geometry.
        A same-spelled declaration or a sibling instance cannot supply this type.
    */
    fn declaration_custody(
        program: &SymbolResolvedTrees,
        record: &PendingInitializer,
        purpose: DependencyPurpose,
    ) -> Result<super::initializer_dependencies::ConstInitializerDependencies, Diagnostic> {
        let mut owners = program.const_declarations.iter().filter(|declaration| {
            program.symbols.symbol_source_span(declaration.symbol) == Some(record.declaration)
                && declaration.initializer == record.materialized
        });
        let owner = owners.next().ok_or_else(|| {
            error(
                record.declaration,
                "normalization lost its exact declaration owner",
            )
        })?;
        if owners.next().is_some() {
            return Err(error(
                record.declaration,
                "normalization has ambiguous declaration custody",
            ));
        }
        let dependencies = super::initializer_dependencies::collect_declaration(
            program,
            record.authored,
            &owner.declared_type,
            purpose,
            None,
            Some(if purpose == DependencyPurpose::Authored {
                &record.receipt.authored_custody.call_selections
            } else {
                &record.receipt.declaration_custody.call_selections
            }),
        )
        .map_err(|reason| error(record.declaration, &reason))?;
        if !dependencies.pending_calls.is_empty() {
            return Err(error(
                record.declaration,
                "normalization has unresolved source call choices",
            ));
        }
        Ok(dependencies)
    }

    fn reconstruct(
        program: &SymbolResolvedTrees,
        records: &[PendingInitializer],
        root: usize,
        purpose: DependencyPurpose,
    ) -> Result<(Vec<ConstArgumentOrigin>, Vec<SourceSpan>), Diagnostic> {
        let mut origins = Vec::new();
        let mut operators = Vec::new();
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(ordinal) = pending.pop() {
            if visited.contains(&ordinal) {
                continue;
            }
            visited.push(ordinal);
            let record = &records[ordinal];
            for operator in &record.operators {
                if !operators.contains(operator) {
                    operators.push(*operator);
                }
            }
            let mut dependencies = Vec::new();
            for (reference, name) in &record.references {
                let selected = program
                    .symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        name,
                        &[SymbolKind::Const],
                        *reference,
                    )
                    .ok_or_else(|| {
                        error(*reference, "dependency no longer selects an exact constant")
                    })?;
                dependencies.push((*reference, selected));
            }
            let closure = declaration_custody(program, record, purpose)?;
            for operator in &closure.initializer_operators {
                if !operators.contains(operator) {
                    operators.push(*operator);
                }
            }

            for (reference, declaration) in closure.constants {
                let selected = program
                    .const_declarations
                    .iter()
                    .find(|candidate| {
                        program.symbols.symbol_source_span(candidate.symbol) == Some(declaration)
                    })
                    .ok_or_else(|| {
                        error(reference, "closure dependency lost its exact declaration")
                    })?
                    .symbol;
                if !dependencies.contains(&(reference, selected)) {
                    dependencies.push((reference, selected));
                }
            }
            for (reference, selected) in &dependencies {
                let declaration = program
                    .const_declaration(*selected)
                    .ok_or_else(|| error(*reference, "selected dependency lost its declaration"))?;
                let source = program
                    .symbols
                    .symbol_source_span(*selected)
                    .ok_or_else(|| error(*reference, "selected dependency lost its source"))?;
                if !declaration.is_public
                    && !program.symbols.same_source_package(*reference, source)
                {
                    return Err(error(
                        *reference,
                        "dependency selects a private foreign declaration",
                    ));
                }
                let origin = ConstArgumentOrigin {
                    reference: *reference,
                    declaration: source,
                    initializer: declaration.initializer_source_span,
                    evaluated_value: declaration.evaluated_value.clone().ok_or_else(|| {
                        error(*reference, "dependency has no completed canonical value")
                    })?,
                };
                if !origins.contains(&origin) {
                    origins.push(origin);
                }
                if let Some(dependency) = records
                    .iter()
                    .position(|record| record.declaration == source)
                {
                    if dependency == root {
                        return Err(error(*reference, "normalized initializer dependency cycle"));
                    }
                    pending.push(dependency);
                } else {
                    let mut retained = ConstInitializerCustody::default();
                    append_retained_custody(
                        program,
                        declaration,
                        records[root].declaration,
                        &mut retained,
                        purpose,
                    )?;
                    for origin in retained.selections {
                        if !origins.contains(&origin) {
                            origins.push(origin);
                        }
                    }
                    for operator in retained.builtin_operators {
                        if !operators.contains(&operator) {
                            operators.push(operator);
                        }
                    }
                }
            }
        }
        Ok((origins, operators))
    }

    /*
        A seeded base already owns the complete transitive roster on its resolved
        initializer. Rejoin those exact selections rather than resolving its source
        again under an extension's namespace.
    */
    fn append_retained_custody(
        program: &SymbolResolvedTrees,
        owner: &symbol_resolved_trees::constant::ConstDeclaration,
        root: SourceSpan,
        custody: &mut ConstInitializerCustody,
        purpose: DependencyPurpose,
    ) -> Result<(), Diagnostic> {
        let owner_source = program.symbols.symbol_source_span(owner.symbol);
        for occurrence in body_expressions(program).authored_selection_occurrences(
            if purpose == DependencyPurpose::Authored && owner.authored_initializer.is_valid() {
                owner.authored_initializer
            } else {
                owner.initializer
            },
        ) {
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| error(root, "retained initializer lost its exact selection"))?;
            let reference = selection.source_span();
            match (selection.kind(), selection.target()) {
                (Kind::Call, Target::Resolved(selected))
                    if matches!(
                        program.symbols.get(selected.selected_symbol()).kind,
                        SymbolKind::Machine | SymbolKind::State | SymbolKind::Operator
                    ) =>
                {
                    let declaration =
                        selected_declaration_source(program, selected.selected_symbol())
                            .map_err(|reason| error(reference, &reason))?;
                    if !custody.call_selections.contains(&(reference, declaration)) {
                        custody.call_selections.push((reference, declaration));
                    }
                }
                (Kind::Operator, Target::Intrinsic(Intrinsic::BuiltinOperator)) => {
                    if !custody.builtin_operators.contains(&reference) {
                        custody.builtin_operators.push(reference);
                    }
                }
                (Kind::StaticPathSegment, Target::Resolved(selected))
                    if matches!(
                        program.symbols.get(selected.selected_symbol()).kind,
                        SymbolKind::Module | SymbolKind::Data | SymbolKind::Variant
                    ) => {}
                (Kind::StaticPathSegment, Target::Resolved(selected)) => {
                    let declaration = program
                        .const_declaration(selected.selected_symbol())
                        .ok_or_else(|| {
                            error(
                                reference,
                                "retained initializer dependency is not a constant",
                            )
                        })?;
                    let source = program
                        .symbols
                        .symbol_source_span(declaration.symbol)
                        .ok_or_else(|| {
                            error(reference, "retained initializer dependency lost its source")
                        })?;
                    if source == root || Some(source) == owner_source {
                        return Err(error(reference, "normalized initializer dependency cycle"));
                    }
                    if !declaration.is_public
                        && !program.symbols.same_source_package(reference, source)
                    {
                        return Err(error(
                            reference,
                            "retained initializer selects a private foreign declaration",
                        ));
                    }
                    let origin = ConstArgumentOrigin {
                        reference,
                        declaration: source,
                        initializer: declaration.initializer_source_span,
                        evaluated_value: declaration.evaluated_value.clone().ok_or_else(|| {
                            error(
                                reference,
                                "retained dependency has no completed canonical value",
                            )
                        })?,
                    };
                    if !custody.selections.contains(&origin) {
                        custody.selections.push(origin);
                    }
                }
                (
                    Kind::StructLiteralType
                    | Kind::StructLiteralCase
                    | Kind::StructLiteralField
                    | Kind::CaseReference,
                    Target::Resolved(_),
                ) => {}
                _ => {
                    return Err(error(
                        reference,
                        "retained initializer selection has no checked scalar meaning",
                    ));
                }
            }
        }
        Ok(())
    }

    /*
        After ordinary authored selections have been recorded, close duplicate
        declaration-side operator obligations using the receipt's evaluated meaning.
        Instantiated partitions and unrelated source occurrences retain their own
        selection obligations.
    */
    pub(crate) fn finalize_operator_obligations(
        program: &mut SymbolResolvedTrees,
        records: &[PendingInitializer],
    ) -> Result<(), Diagnostic> {
        let occurrences = program
            .authored_declaration_selections()
            .iter()
            .filter(|selection| {
                selection.kind() == Kind::Operator
                    && selection.exposure() == Exposure::PrivateImplementation
                    && selection.compiler_partition().is_none()
                    && matches!(
                        selection.target(),
                        Target::LateBound(LateBinding::CheckedOperator)
                    )
                    && records.iter().any(|record| {
                        record
                            .receipt
                            .builtin_operators
                            .contains(&selection.source_span())
                    })
            })
            .map(|selection| (selection.occurrence_id(), selection.source_span()))
            .collect::<Vec<_>>();
        for (occurrence, reference) in occurrences {
            program
                .finalize_intrinsic_authored_declaration_selection(
                    occurrence,
                    LateBinding::CheckedOperator,
                    Intrinsic::BuiltinOperator,
                )
                .map_err(|reason| {
                    error(
                        reference,
                        &format!("retained builtin selection finalization failed: {reason:?}"),
                    )
                })?;
        }
        Ok(())
    }

    fn attach_custody(
        program: &mut SymbolResolvedTrees,
        root: ExpressionHandle,
        custody: &ConstInitializerCustody,
        available: &super::initializer_dependencies::ConstInitializerDependencies,
        declaration: SourceSpan,
    ) -> Result<InitializerRootCustody, Diagnostic> {
        let mut occurrences = Vec::new();
        for (reference, selected_source) in &custody.call_selections {
            let mut candidates = available.call_targets.iter().filter(|candidate| {
                candidate.reference == *reference && candidate.declaration == *selected_source
            });
            let candidate = candidates.next().ok_or_else(|| {
                error(
                    *reference,
                    "realized call has no exact source declaration dependency",
                )
            })?;
            if candidates.next().is_some() {
                return Err(error(
                    *reference,
                    "realized call has ambiguous source declaration custody",
                ));
            }
            let original = {
                let mut retained = program
                    .authored_declaration_selections()
                    .iter()
                    .filter(|row| {
                        row.source_span() == *reference
                            && row.exposure() == Exposure::PrivateImplementation
                            && row.kind() == Kind::Call
                            && row.compiler_partition().is_none()
                    });
                let original = retained.next().copied();
                if retained.next().is_some() {
                    return Err(error(
                        *reference,
                        "initializer call has duplicate source occurrences",
                    ));
                }
                original
            };
            let occurrence = if let Some(original) = original {
                match original.target() {
                    Target::LateBound(LateBinding::CheckedCall) => program
                        .finalize_authored_declaration_selection(
                            original.occurrence_id(),
                            LateBinding::CheckedCall,
                            candidate.target,
                        )
                        .map_err(|reason| {
                            error(
                                *reference,
                                &format!(
                                    "initializer call selection finalization failed: {reason:?}"
                                ),
                            )
                        })?,
                    Target::Resolved(selected)
                        if selected.selected_symbol() == candidate.target => {}
                    _ => {
                        return Err(error(
                            *reference,
                            "initializer call conflicts with its retained source selection",
                        ));
                    }
                }
                original.occurrence_id()
            } else {
                program
                    .record_resolved_authored_declaration_selection(
                        *reference,
                        Exposure::PrivateImplementation,
                        Kind::Call,
                        candidate.target,
                    )
                    .map_err(crate::constant::selections::const_selection_record_diagnostic)?
            };
            occurrences.push(occurrence);
        }
        for origin in &custody.selections {
            let mut candidates = program.const_declarations.iter().filter(|candidate| {
                program.symbols.symbol_source_span(candidate.symbol) == Some(origin.declaration)
            });
            let selected = candidates
                .next()
                .ok_or_else(|| error(origin.reference, "selected declaration disappeared"))?
                .symbol;
            if candidates.next().is_some() {
                return Err(error(
                    origin.reference,
                    "selected declaration has ambiguous source custody",
                ));
            }
            occurrences.push(
                program
                    .record_resolved_authored_declaration_selection(
                        origin.reference,
                        Exposure::PrivateImplementation,
                        Kind::StaticPathSegment,
                        selected,
                    )
                    .map_err(crate::constant::selections::const_selection_record_diagnostic)?,
            );
        }
        for operator in &custody.builtin_operators {
            let occurrence = program
                .record_late_bound_authored_declaration_selection(
                    *operator,
                    Exposure::PrivateImplementation,
                    Kind::Operator,
                    LateBinding::CheckedOperator,
                )
                .map_err(crate::constant::selections::const_selection_record_diagnostic)?;
            program
                .finalize_intrinsic_authored_declaration_selection(
                    occurrence,
                    LateBinding::CheckedOperator,
                    Intrinsic::BuiltinOperator,
                )
                .map_err(|reason| {
                    error(
                        declaration,
                        &format!("builtin selection finalization failed: {reason:?}"),
                    )
                })?;
            occurrences.push(occurrence);
        }
        let owner = InitializerRootCustody {
            root,
            reference: program.tables.bodies.expressions.source_span(root),
            declaration,
            occurrences,
        };
        replace_initializer_lanes(program, &owner)?;
        Ok(owner)
    }

    fn replace_initializer_lanes(
        program: &mut SymbolResolvedTrees,
        owner: &InitializerRootCustody,
    ) -> Result<(), Diagnostic> {
        if !body_expressions(program).expression_is_valid(owner.root)
            || program.tables.bodies.expressions.source_span(owner.root) != owner.reference
        {
            return Err(error(
                owner.declaration,
                "initializer substitution changed its exact custody root",
            ));
        }
        let mut retained = Vec::new();
        for occurrence in body_expressions(program).authored_selection_occurrences(owner.root) {
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| {
                    error(
                        owner.declaration,
                        "initializer root lost an attached selection",
                    )
                })?;
            let initializer_lane = matches!(selection.kind(), Kind::Call | Kind::Operator)
                || (selection.kind() == Kind::StaticPathSegment
                    && matches!(selection.target(), Target::Resolved(selected)
                    if program.symbols.get(selected.selected_symbol()).kind == SymbolKind::Const));
            if !initializer_lane {
                retained.push(occurrence);
            }
        }
        retained.extend(owner.occurrences.iter().copied());
        program
            .tables
            .bodies
            .expressions
            .replace_authored_selection_occurrences(owner.root, retained);
        Ok(())
    }

    fn same_custody(expected: &ConstInitializerCustody, actual: &ConstInitializerCustody) -> bool {
        expected.selections.len() == actual.selections.len()
            && expected
                .selections
                .iter()
                .all(|value| actual.selections.contains(value))
            && expected.builtin_operators.len() == actual.builtin_operators.len()
            && expected
                .builtin_operators
                .iter()
                .all(|value| actual.builtin_operators.contains(value))
            && expected.call_selections.len() == actual.call_selections.len()
            && expected
                .call_selections
                .iter()
                .all(|value| actual.call_selections.contains(value))
    }

    /*
        Describe a failed inventory comparison without printing constant payloads.
        Counts stay complete; only the first four source coordinates are retained.
    */
    fn missing_dependency_prefix<Value: PartialEq, Description>(
        required: &[Value],
        available: &[Value],
        mut describe: impl FnMut(&Value) -> Description,
    ) -> (usize, Vec<Description>) {
        let mut count = 0usize;
        let mut prefix = Vec::new();
        for value in required {
            if !available.contains(value) {
                count += 1;
                if prefix.len() < 4 {
                    prefix.push(describe(value));
                }
            }
        }
        (count, prefix)
    }

    pub(crate) fn finalize(
        program: &mut SymbolResolvedTrees,
        records: &[PendingInitializer],
    ) -> Result<Vec<InitializerRootCustody>, Diagnostic> {
        use super::initializer_dependencies::DependencyPurpose;
        use syntax_trees::item::ConstInitializerCustody;
        let mut roots = Vec::new();
        for (ordinal, record) in records.iter().enumerate() {
            let mut owners = program.const_declarations.iter().filter(|declaration| {
                program.symbols.symbol_source_span(declaration.symbol) == Some(record.declaration)
                    && declaration.initializer == record.materialized
            });
            let owner = owners.next().ok_or_else(|| {
                error(
                    record.declaration,
                    "normalization lost its exact declaration owner",
                )
            })?;
            if owners.next().is_some() {
                return Err(error(
                    record.declaration,
                    "normalization has ambiguous declaration custody",
                ));
            }
            resolved_payload::validate(
                program,
                &owner.declared_type,
                record.materialized,
                &record.receipt.result_value,
            )
            .map_err(|reason| {
                error(
                    record.declaration,
                    &format!("materialized value is not canonical: {reason}"),
                )
            })?;
            let (authored_origins, authored_operators) =
                reconstruct(program, records, ordinal, DependencyPurpose::Authored)?;
            let authored_calls = declaration_custody(program, record, DependencyPurpose::Authored)?;
            let authored = ConstInitializerCustody {
                selections: authored_origins,
                builtin_operators: authored_operators,
                call_selections: authored_calls.calls.clone(),
            };
            let (origins, operators) = reconstruct(
                program,
                records,
                ordinal,
                DependencyPurpose::DeclarationReadiness,
            )?;
            let declaration_calls =
                declaration_custody(program, record, DependencyPurpose::DeclarationReadiness)?;
            let preparation = ConstInitializerCustody {
                selections: origins,
                builtin_operators: operators,
                call_selections: declaration_calls.calls.clone(),
            };
            if !same_custody(&authored, &record.receipt.authored_custody)
                || !same_custody(&preparation, &record.receipt.declaration_custody)
            {
                return Err(error(
                    record.declaration,
                    "authored or declaration readiness custody drifted",
                ));
            }
            let realized = ConstInitializerCustody {
                selections: record.receipt.selections.clone(),
                builtin_operators: record.receipt.builtin_operators.clone(),
                call_selections: record.receipt.call_selections.clone(),
            };
            if realized
                .selections
                .iter()
                .enumerate()
                .any(|(index, value)| realized.selections[..index].contains(value))
                || realized
                    .builtin_operators
                    .iter()
                    .enumerate()
                    .any(|(index, value)| realized.builtin_operators[..index].contains(value))
                || realized
                    .call_selections
                    .iter()
                    .enumerate()
                    .any(|(index, value)| realized.call_selections[..index].contains(value))
            {
                return Err(error(
                    record.declaration,
                    "realized custody contains duplicate occurrences",
                ));
            }
            if realized
                .selections
                .iter()
                .any(|value| !preparation.selections.contains(value))
                || realized
                    .builtin_operators
                    .iter()
                    .any(|value| !preparation.builtin_operators.contains(value))
                || realized
                    .call_selections
                    .iter()
                    .any(|value| !preparation.call_selections.contains(value))
            {
                let missing_selections = missing_dependency_prefix(
                    &realized.selections,
                    &preparation.selections,
                    |origin| (origin.reference, origin.declaration, origin.initializer),
                );
                let missing_operators = missing_dependency_prefix(
                    &realized.builtin_operators,
                    &preparation.builtin_operators,
                    |source| *source,
                );
                let missing_calls = missing_dependency_prefix(
                    &realized.call_selections,
                    &preparation.call_selections,
                    |call| *call,
                );
                return Err(error(
                    record.declaration,
                    &format!(
                        "realized custody has no exact source preparation dependency: missing selections (count, source prefix) {missing_selections:?}; missing operators (count, source prefix) {missing_operators:?}; missing calls (count, source prefix) {missing_calls:?}"
                    ),
                ));
            }
            roots.push(attach_custody(
                program,
                record.authored,
                &authored,
                &authored_calls,
                record.declaration,
            )?);
            roots.push(attach_custody(
                program,
                record.materialized,
                &realized,
                &declaration_calls,
                record.declaration,
            )?);
        }
        Ok(roots)
    }
}
mod normalized_expressions {
    /* Validation of normalized expressions and const arguments. */

    use crate::tables::syntax_expressions;
    use diagnostics::Diagnostic;

    /*
        Named origins already retain their selected declaration's value. Direct
        structured atoms instead require the original constructor expression, even
        when a malformed input clears its handle.
    */
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
        if normalization.materialized_expression.is_valid()
            && (!normalization.authored_expression.is_valid()
                || !syntax
                    .expressions
                    .contains_expression(normalization.materialized_expression)
                || syntax_expressions(syntax).source_span(normalization.materialized_expression)
                    != normalization.reference)
        {
            return Err(Diagnostic::error(
                "constant argument lost its exact materialized result expression",
            )
            .with_source_span(normalization.reference));
        }
        if normalization_requires_expression(normalization)
            && (!syntax
                .expressions
                .contains_expression(normalization.authored_expression)
                || syntax_expressions(syntax).source_span(normalization.authored_expression)
                    != normalization.reference)
        {
            return Err(Diagnostic::error(
                "direct constant argument lost its exact authored expression",
            )
            .with_source_span(normalization.reference));
        }
        Ok(())
    }

    /*
        Check the rewritten payload against its captured canonical value before
        binding the selected declaration to a final symbol.
    */
    pub(crate) fn validate_normalized_const_argument(
        syntax_trees: &syntax_trees::SyntaxTrees,
        argument: &syntax_trees::types::TypeReferenceNode,
        normalization: &syntax_trees::types::ConstArgumentNormalization,
    ) -> Result<(), Diagnostic> {
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        let encoded = CanonicalConstValue::new("", &normalization.canonical_result_encoding, "");
        let matches = match (argument, encoded.decode_encoding()) {
            (
                syntax_trees::types::TypeReferenceNode::Named(name),
                Some(DecodedCanonicalConstValue::Integer { value, .. }),
            ) => name.as_str(syntax_trees) == value.to_string(),
            (syntax_trees::types::TypeReferenceNode::Named(name), Some(decoded)) => {
                CanonicalConstValue::from_atom(name.as_str(syntax_trees)).is_some_and(|value| {
                    let carrier = match &decoded {
                        DecodedCanonicalConstValue::Float { .. } => return false,
                        DecodedCanonicalConstValue::Boolean(_) => "bool",
                        DecodedCanonicalConstValue::Array { type_name, .. }
                        | DecodedCanonicalConstValue::Record { type_name, .. }
                        | DecodedCanonicalConstValue::Variant { type_name, .. }
                        | DecodedCanonicalConstValue::Integer { type_name, .. } => {
                            type_name.as_str()
                        }
                    };
                    value.encoding == normalization.canonical_result_encoding
                        && value.type_name == carrier
                        && value.decode_encoding() == Some(decoded)
                })
            }
            _ => false,
        };
        matches.then_some(()).ok_or_else(|| {
            Diagnostic::error(
                "normalized constant argument value drifted from its retained canonical result",
            )
            .with_source_span(normalization.reference)
        })
    }
}
mod scalar_carriers {
    /*
        Builtin scalar carrier spellings, recognized before any symbol exists.

        The integer carriers are exactly the `LandedIntegerType` names. With `bool`
        they form the structural scalars: carriers whose values have a canonical
        const-index identity. The float formats complete the builtin scalars; their
        values land at a declared format but carry no structural identity.
    */

    use numerics::literals::LandedIntegerType;

    /* `bool` or an integer carrier. */
    pub(crate) fn is_structural_scalar_carrier(name: &str) -> bool {
        name == "bool" || LandedIntegerType::from_name(name).is_some()
    }

    /* A structural scalar or a float format. */
    pub(crate) fn is_builtin_scalar_carrier(name: &str) -> bool {
        is_structural_scalar_carrier(name) || matches!(name, "f32" | "f64")
    }
}
mod selections {
    /* Finalizing const selections, declarations and argument selections. */

    use crate::constant::carrier;
    use carrier::retain_declared_carrier;
    use carrier::same_resolved_carrier;
    use diagnostics::Diagnostic;
    use language_semantics::const_value::CanonicalConstValue;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionRecordError,
    };
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::SymbolKind;

    /*
        Attach the authored const selection to the substituted expression. The
        value remains fully erased; only declaration custody survives.
    */
    pub(crate) fn finalize_const_selections(
        program: &mut SymbolResolvedTrees,
        pending: &[crate::resolution::lowerer::PendingConstSelection],
    ) -> Result<(), Diagnostic> {
        for selection in pending {
            let symbol = selection.declaration;
            retain_declared_carrier(program, selection.expression, symbol, selection.source_span)?;
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

    /*
        Finalize const attachment and visibility using the identity its allocation
        already published. Historical symbol-table roots are not this declaration
        roster and must never be zipped back onto it.
    */
    pub(crate) fn finalize_const_declarations(
        syntax_trees: &syntax_trees::SyntaxTrees,
        program: &mut SymbolResolvedTrees,
        pending: &[crate::resolution::lowerer::PendingConstDeclaration],
    ) -> Result<(), Diagnostic> {
        for declaration in pending {
            if !program
                .roots
                .const_declarations
                .contains(declaration.declaration)
            {
                return Err(Diagnostic::error(
                    "pending constant lost its owned declaration",
                ));
            }
            let constant = program
                .roots
                .const_declarations
                .get(declaration.declaration);
            let constant_symbol = constant.symbol;
            let is_public = constant.is_public;
            if !constant_symbol.is_valid()
                || program.symbols.get(constant_symbol).kind != SymbolKind::Const
                || declaration.is_public != is_public
            {
                return Err(Diagnostic::error(
                    "const declaration identity or visibility drifted before finalization",
                ));
            }
            let module = program.symbols.symbol_module(constant_symbol);
            if declaration.scope.as_str(syntax_trees).is_empty() || !module.is_valid() {
                continue;
            }
            let carrier = program
                .symbols
                .lookup_top_level_by_name_and_kinds_from_source_matching(
                    declaration.scope.as_str(syntax_trees),
                    &[SymbolKind::Data],
                    declaration.scope.source_span(),
                    |_| true,
                );
            let carrier = match carrier {
                symbols::SymbolLookup::Unique(carrier) => carrier,
                symbols::SymbolLookup::Ambiguous { first, second } => {
                    return Err(Diagnostic::error(format!(
                    "module type-scoped constant selects an ambiguous data carrier between `{}` and `{}`",
                    program.symbols.display_path(first, "::"),
                    program.symbols.display_path(second, "::"),
                ))
                .with_source_span(declaration.scope.source_span()));
                }
                symbols::SymbolLookup::NotFound => {
                    return Err(Diagnostic::error(
                    "module type-scoped constants require an exact accessible data carrier selected in their declaring source",
                ).with_source_span(declaration.scope.source_span()));
                }
            };
            if !program.data_definitions.iter().any(|definition| {
                definition.symbol == carrier
                    && (definition.is_public
                        || program
                            .symbols
                            .same_symbol_source_package(carrier, constant_symbol))
            }) {
                return Err(Diagnostic::error(
                "module type-scoped constants require an exact accessible data carrier selected in their declaring source",
            ).with_source_span(declaration.scope.source_span()));
            }
            let exposure = if declaration.is_public {
                AuthoredDeclarationSelectionExposure::PublicInterface
            } else {
                AuthoredDeclarationSelectionExposure::PrivateImplementation
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
        Ok(())
    }

    pub(crate) fn const_selection_record_diagnostic(
        error: AuthoredDeclarationSelectionRecordError,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "failed to retain const declaration selection: {error:?}"
        ))
    }

    /* Bind exact selected declaration custody after ordinary symbol allocation. */
    pub(crate) fn finalize_const_argument_selections(
        program: &mut SymbolResolvedTrees,
        pending: &[crate::resolution::lowerer::PendingConstArgumentSelection],
        slots: &[crate::resolution::lowerer::PendingConstArgumentSlot],
        lexical_inputs: &[crate::lowering::state::StateLexicalInputs],
    ) -> Result<(), Diagnostic> {
        let mut selected_arguments = Vec::new();
        for (selection_ordinal, selection) in pending.iter().enumerate() {
            let origin = &selection.origin;
            let mut declarations = program.const_declarations.iter().filter(|declaration| {
                program.symbols.symbol_source_span(declaration.symbol) == Some(origin.declaration)
            });
            let declaration = declarations
                .next()
                .ok_or_else(|| {
                    Diagnostic::error(
                        "normalized constant argument lost its exact selected declaration",
                    )
                    .with_source_span(origin.reference)
                })?
                .clone();
            if declarations.next().is_some()
                || declaration.initializer_source_span != origin.initializer
                || declaration.evaluated_value.as_ref() != Some(&origin.evaluated_value)
            {
                return Err(Diagnostic::error(
                "normalized constant argument declaration or value custody drifted before resolution",
            ).with_source_span(origin.reference));
            }
            let requires_nominal_slot = selection.is_argument_value
                && (carrier::has_nominal_carrier(program, &declaration.declared_type)
                    || origin
                        .evaluated_value
                        .encoding()
                        .is_some_and(carrier::encoding_has_nominal_carrier));
            let structured_argument = selection.is_argument_value
                && origin.evaluated_value.encoding().is_some_and(|encoding| {
                    use language_semantics::const_value::{
                        CanonicalConstValue, DecodedCanonicalConstValue,
                    };
                    matches!(
                        CanonicalConstValue::new("", encoding, "").decode_encoding(),
                        Some(
                            DecodedCanonicalConstValue::Array { .. }
                                | DecodedCanonicalConstValue::Record { .. }
                                | DecodedCanonicalConstValue::Variant { .. }
                        )
                    )
                });
            let mut matched = false;
            for slot in slots
                .iter()
                .filter(|slot| slot.selection == selection_ordinal)
            {
                let parameters =
                    carrier::receiving_parameters(program, slot.arguments, lexical_inputs)
                        .ok_or_else(|| {
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
                let TypeParameterKind::Const { type_reference } = &parameter.kind else {
                    return Err(
                        Diagnostic::error("constant did not enter a const parameter")
                            .with_source_span(origin.reference),
                    );
                };
                if selection.is_argument_value
                    && (structured_argument
                        || requires_nominal_slot
                        || carrier::has_nominal_carrier(program, type_reference))
                {
                    if (requires_nominal_slot
                        || carrier::has_nominal_carrier(program, type_reference))
                        && !same_resolved_carrier(
                            program,
                            &declaration.declared_type,
                            type_reference,
                        )
                    {
                        return Err(Diagnostic::error("selected constant nominal carrier differs from its receiving generic parameter").with_source_span(origin.reference));
                    }
                    let TypeReference::Named { symbol, name, .. } = argument else {
                        return Err(Diagnostic::error(
                            "nominal constant receiving argument lost its canonical value",
                        )
                        .with_source_span(origin.reference));
                    };
                    if (symbol.is_valid() && *symbol != declaration.symbol)
                        || !CanonicalConstValue::from_atom(name.as_str()).is_some_and(|value| {
                            Some(value.encoding.as_str()) == origin.evaluated_value.encoding()
                        })
                    {
                        return Err(Diagnostic::error(
                            "nominal constant receiving argument differs from its selected value",
                        )
                        .with_source_span(origin.reference));
                    }
                    selected_arguments.push((
                        slot.arguments,
                        slot.ordinal,
                        declaration.symbol,
                        DiagnosticName::from_str(
                            program.symbols.name(declaration.symbol),
                            origin.reference,
                        ),
                    ));
                }
                matched = true;
            }
            if requires_nominal_slot && !matched {
                return Err(Diagnostic::error(
                    "nominal constant argument has no retained receiving generic slot",
                )
                .with_source_span(origin.reference));
            }
            if let Some(exposure) = selection.exposure {
                program
                    .record_resolved_authored_declaration_selection(
                        origin.reference,
                        exposure,
                        AuthoredDeclarationSelectionKind::StaticPathSegment,
                        declaration.symbol,
                    )
                    .map_err(const_selection_record_diagnostic)?;
            }
        }
        for (arguments, ordinal, selected, selected_name) in selected_arguments {
            let argument = &mut program
                .tables
                .declarations
                .child_type_references
                .span_mut_or_empty(arguments)[ordinal];
            if let TypeReference::Named { symbol, name, .. } = argument {
                *symbol = selected;
                *name = selected_name;
            }
        }
        Ok(())
    }
}
mod substitution {
    /* Substituting resolved constants into expressions. */

    use crate::constant::declaration_values::selected_expression_constant;
    use crate::tables::body_expressions;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::SymbolKind;
    use syntax_trees::item::ConstDefinition;

    pub(crate) fn substitute_resolved_constants(
        program: &mut SymbolResolvedTrees,
        authored: &[crate::resolution::lowerer::PendingAuthoredExpression],
        selections: &mut Vec<crate::resolution::lowerer::PendingConstSelection>,
        mode: crate::resolution::lowerer::ConstResolutionMode,
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
                .data_definitions
                .iter()
                .map(|declaration| declaration.symbol)
                .chain(program.machines.iter().map(|machine| machine.symbol))
                .any(|other| {
                    program.symbols.name(other) == program.symbols.name(*symbol)
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
        let unsupported_array_projection_sources = body_expressions(program)
            .iter_expressions()
            .filter_map(|(_, node)| {
                let mut collection = match node {
                    ExpressionNode::Indexed(indexed)
                        if !matches!(
                            program.tables.bodies.expressions.expression(indexed.index),
                            ExpressionNode::Integer(_)
                        ) || matches!(
                            body_expressions(program).expression(indexed.collection),
                            ExpressionNode::Borrow(_)
                        ) =>
                    {
                        indexed.collection
                    }
                    ExpressionNode::Borrow(borrow)
                        if borrow.access != language_semantics::ReferenceAccess::Shared
                            || matches!(
                                body_expressions(program).expression(borrow.target),
                                ExpressionNode::Indexed(_) | ExpressionNode::Member(_)
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
                        ExpressionNode::Member(member) => collection = member.receiver,
                        _ => break,
                    }
                }
                Some(collection)
            })
            .collect::<Vec<_>>();
        for occurrence in authored {
            let Some((reference, selected)) =
                selected_expression_constant(program, occurrence.expression)
            else {
                continue;
            };
            let Some(declaration) = program.roots.const_declaration(selected) else {
                return Err(Diagnostic::error(
                    "resolved constant selection has no retained declaration",
                ));
            };
            if mode == crate::resolution::lowerer::ConstResolutionMode::ArgumentSelection
                || (mode == crate::resolution::lowerer::ConstResolutionMode::InitializerSelection
                    && declaration.evaluated_value.is_none())
            {
                selections.push(crate::resolution::lowerer::PendingConstSelection {
                    expression: occurrence.expression,
                    source_span: reference,
                    declaration: selected,
                    exposure: occurrence.exposure,
                });
                continue;
            }
            let initializer = declaration.initializer;
            if !body_expressions(program).expression_is_valid(initializer)
                || program.tables.bodies.expressions.source_span(initializer)
                    != declaration.initializer_source_span
            {
                return Err(Diagnostic::error(
                    "constant substitution lost its exact declaration initializer",
                )
                .with_source_span(reference));
            }
            if matches!(
                program.tables.bodies.expressions.expression(initializer),
                ExpressionNode::ArrayLiteral(_)
            ) && unsupported_array_projection_sources.contains(&occurrence.expression)
            {
                return Err(Diagnostic::error(
                "array constant projection currently requires unborrowed literal integer selectors; dynamic indexing, borrowing and slicing require value-based array projection"
            ).with_source_span(reference));
            }
            let initializer = if matches!(
                program.tables.bodies.expressions.expression(initializer),
                ExpressionNode::ArrayLiteral(_)
                    | ExpressionNode::StructLiteral(_)
                    | ExpressionNode::Name(_)
            ) {
                program
                    .tables
                    .bodies
                    .expressions
                    .copy_from_self(initializer)
            } else {
                initializer
            };
            let initializer_selections = body_expressions(program)
                .authored_selection_occurrences(initializer)
                .collect::<Vec<_>>();
            program
                .tables
                .bodies
                .expressions
                .attach_authored_selection_occurrences(
                    occurrence.expression,
                    initializer_selections,
                );
            let value = body_expressions(program).expression(initializer).clone();
            *program
                .tables
                .bodies
                .expressions
                .expression_mut(occurrence.expression) = value;
            program
                .tables
                .bodies
                .expressions
                .set_source_span(occurrence.expression, reference);
            selections.push(crate::resolution::lowerer::PendingConstSelection {
                expression: occurrence.expression,
                source_span: reference,
                declaration: selected,
                exposure: occurrence.exposure,
            });
        }
        Ok(())
    }

    pub(crate) fn semantic_const_name(
        syntax_trees: &syntax_trees::SyntaxTrees,
        definition: &ConstDefinition,
    ) -> String {
        if definition.scope.as_str(syntax_trees).is_empty() {
            definition.name.as_str(syntax_trees).to_owned()
        } else {
            format!(
                "{}::{}",
                definition.scope.as_str(syntax_trees),
                definition.name.as_str(syntax_trees)
            )
        }
    }
}

pub(crate) use declaration_values::{
    evaluated_declaration_value, retain_const_initializer, selected_expression_constant,
    validate_const_definition, validate_scalar_initializer,
};
pub use initializer_leaves::PendingConstInitializerLeaf;
pub(crate) use initializer_leaves::pending_const_initializer_leaves;
pub(crate) use normalized_expressions::{
    normalization_requires_expression, validate_normalized_const_argument,
    validate_normalized_expression,
};
pub(crate) use scalar_carriers::{is_builtin_scalar_carrier, is_structural_scalar_carrier};
pub(crate) use selections::{
    finalize_const_argument_selections, finalize_const_declarations, finalize_const_selections,
};
pub(crate) use substitution::{semantic_const_name, substitute_resolved_constants};

use crate::resolution::lowerer::Lowerer;
use crate::symbols::assign_constant_expression_symbols;
use crate::tables::syntax_expressions;
use diagnostics::Diagnostic;
use initializer_dependencies::bind_named_implementation_const_calls;
use numerics::arithmetic::ArithmeticDomain;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::ConstDefinition;
use syntax_trees::types::TypeReferenceNode;

/*
    A direct implementation selection is a symbolic literal, not an evaluated
    table address. Ordinary resolution and dynamic selection checking retain
    and authenticate its implementation and complete target application, even
    when the declaration has no use. Public canonical identity remains separate.
*/
pub(crate) fn direct_selector_literal(syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Cast(cast) = syntax.expressions.expression(expression) else {
        return false;
    };
    !cast.form.is_recast()
        && cast.domain == ArithmeticDomain::Exact
        && cast.semantic_domain.is_empty()
        && cast.semantic_domain_arguments.is_empty()
        && matches!(
            syntax.type_references.type_reference(cast.target_type),
            TypeReferenceNode::DynamicTrait { .. }
        )
        && matches!(
            syntax.expressions.expression(cast.value),
            ExpressionNode::Name(_)
        )
}

/*
    Find unfinished values without claiming a carrier or constructor selection.
    Preparation separately admits exact scalar leaves in their declared owners.
*/
pub fn requires_const_initializer_evaluation(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> bool {
    use syntax_trees::expression::ExpressionNode;
    if definition.pending_target_observation {
        return false;
    }
    let mut pending = vec![definition.value];
    while let Some(expression) = pending.pop() {
        if direct_selector_literal(syntax, expression) {
            continue;
        }
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_)
            | ExpressionNode::UndeterminedNaN(_) => {}
            ExpressionNode::Float(text)
                if text
                    .literal(syntax)
                    .is_some_and(|literal| literal.landing().is_some()) => {}
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(
                    syntax_expressions(syntax)
                        .expression_handles(*elements)
                        .iter()
                        .copied(),
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                pending.extend(
                    syntax_expressions(syntax)
                        .struct_fields(literal.fields)
                        .iter()
                        .map(|field| field.value),
                );
            }
            _ => return true,
        }
    }
    false
}

/*
    Finalize argument selections, initializer normalization, substitution, and
    use-site selections after resolution has joined constant declarations. The
    declaration join precedes receipt helper demand and must not repeat here:
    it also records the declaration's selected carrier and visibility custody.
    Constant-expression symbol and selection passes run inside because they
    consume only these pending values. Selections already name exact declaration
    handles, including retained constants; no cohort-relative ordinal is rebased.
*/
pub(crate) fn finalize(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    let program = &mut lowerer.symbol_resolved_trees;
    finalize_const_argument_selections(
        program,
        &lowerer.pending_const_argument_selections,
        &lowerer.pending_const_argument_slots,
        &lowerer.resolved_lexical_inputs,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let const_expression_roots = lowerer
        .pending_const_values
        .iter()
        .copied()
        .chain(lowerer.pending_const_argument_expressions.iter().copied())
        .collect::<Vec<_>>();
    assign_constant_expression_symbols(program, const_expression_roots.iter().copied());
    bind_named_implementation_const_calls(program, &const_expression_roots)
        .map_err(|message| vec![Diagnostic::error(message)])?;
    if lowerer.const_resolution_mode == crate::resolution::lowerer::ConstResolutionMode::Complete {
        declaration_values::validate_nominal_destinations(program)
            .map_err(|diagnostic| vec![diagnostic])?;
    }
    crate::selection::authored_selections::finalize_constant_expression_selections(
        program,
        lowerer
            .pending_const_values
            .iter()
            .copied()
            .chain(lowerer.pending_const_argument_expressions.iter().copied()),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let initializer_custody =
        initializer_normalization::finalize(program, &lowerer.pending_const_initializers)
            .map_err(|diagnostic| vec![diagnostic])?;
    substitute_resolved_constants(
        program,
        &lowerer.pending_authored_expressions,
        &mut lowerer.pending_const_selections,
        lowerer.const_resolution_mode,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    finalize_const_selections(program, &lowerer.pending_const_selections)
        .map_err(|diagnostic| vec![diagnostic])?;
    initializer_normalization::rejoin_after_substitution(program, &initializer_custody)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/*
    Close duplicate declaration-side operator obligations once the authored
    selection ledger has recorded every ordinary selection.
*/
pub(crate) fn finalize_operator_obligations(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    initializer_normalization::finalize_operator_obligations(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_initializers,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}
