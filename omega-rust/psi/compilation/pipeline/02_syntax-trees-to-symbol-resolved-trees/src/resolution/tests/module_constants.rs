use crate::resolution::{ResolutionRequest, resolve};
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use source::{SourceId, SourceSpan, Span};
use source_files_to_tokens::Lexer;
use syntax_trees::{
    SyntaxTrees,
    identifier::Identifier,
    types::{ConstArgumentOrigin, TypeReferenceNode},
};

fn domain_index_sources(sources: &[(SourceId, &str)]) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::default();
    for (source, text) in sources {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize domain indices");
        tokens_to_syntax_trees::parse(&mut syntax, *source, &tokens).expect("parse domain indices");
    }
    syntax
}

#[test]
fn domain_index_selection_cannot_fall_back_from_ambiguous_imports() {
    let syntax = crate::preparation::generic_data::normalize_generic_data(crate::preparation::generic_data::GenericDataRequest::new(domain_index_sources(&[
        (SourceId(1), "domain<T, const N: u64> T::Indexed<N>;"),
        (SourceId(2), "module combat; pub const SIZE: u64 = 2;"),
        (SourceId(3), "module rooms; pub const SIZE: u64 = 3;"),
        (SourceId(4), "module consumer; use combat::SIZE; use rooms::SIZE; data Use { value: u64 in Indexed<SIZE>; }"),
    ]))).expect("defer ambiguous selection");
    let constraints = syntax.type_references.domain_constraints();
    let argument = syntax
        .type_references
        .type_reference_handles(constraints[0].arguments)[0];
    assert!(
        syntax
            .type_references
            .const_argument_normalization(argument)
            .is_none()
    );
    assert!(
        matches!(syntax.type_references.type_reference(argument), TypeReferenceNode::Named(name) if name.as_str() == "SIZE")
    );
}

#[test]
fn named_domain_index_keeps_its_declared_integer_carrier() {
    let diagnostics = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(domain_index_sources(&[
            (SourceId(1), "domain<T, const N: u64> T::Indexed<N>;"),
            (
                SourceId(2),
                "module combat; const SIZE: u8 = 2; data Use { value: u64 in Indexed<SIZE>; }",
            ),
        ])),
    )
    .expect_err("u8 constant cannot silently become u64 index");
    assert!(format!("{diagnostics:?}").contains("declares type `u8`"));
}

fn normalized(public: bool) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::default();
    let module = format!(
        "module combat; pub const SIZE: u64 = 2; {}data Use {{ buffer: Buffer<SIZE>; }}",
        if public { "pub " } else { "" }
    );
    for (source_id, text) in [
        (SourceId(1), "pub data Buffer<const N: u64> { value: u64; }"),
        (SourceId(2), module.as_str()),
    ] {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize constant index custody");
        tokens_to_syntax_trees::parse(&mut syntax, source_id, &tokens)
            .expect("parse constant index custody");
    }
    crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("normalize exact module constant index")
}

fn origin(syntax: &SyntaxTrees) -> ConstArgumentOrigin {
    syntax
        .type_references
        .named_nodes_from(0)
        .into_iter()
        .find_map(|(handle, _)| {
            syntax
                .type_references
                .const_argument_normalization(handle)
                .and_then(|normalization| {
                    syntax
                        .type_references
                        .const_argument_origins(normalization.selections)
                        .first()
                })
                .cloned()
        })
        .expect("retained constant origin")
}

#[test]
fn normalized_constant_argument_rejects_missing_and_drifted_declaration_custody() {
    let syntax = normalized(false);
    let original = origin(&syntax);
    for corruption in ["missing", "initializer", "encoding", "duplicate"] {
        let mut program =
            resolve(ResolutionRequest::new(&syntax)).expect("resolve original custody");
        let mut origin = original.clone();
        match corruption {
            "missing" => origin.declaration = SourceSpan::new(SourceId(9), Span::new(10, 14)),
            "initializer" => origin.initializer.span.start += 1,
            "encoding" => origin.canonical_value_encoding.push('x'),
            "duplicate" => {
                let declaration = program
                    .const_declarations
                    .iter()
                    .next()
                    .expect("retained declaration")
                    .clone();
                program.roots.const_declarations.push(declaration);
            }
            _ => unreachable!(),
        }
        let selection = crate::resolution::lowerer::PendingConstArgumentSelection {
            origin,
            exposure: AuthoredDeclarationSelectionExposure::PrivateImplementation,
        };
        assert!(
            crate::constant::finalize_const_argument_selections(&mut program, &[selection], &[])
                .is_err(),
            "{corruption} custody must reject"
        );
    }
}

#[test]
fn rewritten_constant_payload_cannot_drift_from_its_origin() {
    let mut syntax = normalized(false);
    let arguments = syntax
        .type_references
        .named_nodes_from(0)
        .into_iter()
        .filter_map(|(handle, _)| {
            syntax
                .type_references
                .const_argument_normalization(handle)
                .is_some()
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert!(!arguments.is_empty());
    for argument in arguments {
        syntax.type_references.replace_type_reference(
            argument,
            TypeReferenceNode::Named(Identifier::generated("3")),
        );
    }
    assert!(
        resolve(ResolutionRequest::new(&syntax)).is_err(),
        "rewritten 3 cannot retain the declaration's exact 2 encoding"
    );
}

#[test]
fn nominal_constant_receiving_slot_rejects_carrier_and_parent_substitution() {
    use symbol_resolved_trees::types::TypeReference;
    let syntax = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(domain_index_sources(&[
            (
                SourceId(0),
                "use settings; data Value { value: u64; }
          data Pick<const V: settings::Value> { marker: u8; }
          data OtherPick<const V: Value> { marker: u8; }
          machine keep(value: Pick<settings::VALUE>) -> Pick<settings::VALUE> { value }",
            ),
            (
                SourceId(1),
                "module settings; pub data Value { value: u64; }
          pub const VALUE: Value = Value { value: 1 };",
            ),
        ])),
    )
    .expect("normalize nominal arguments");
    let original = syntax
        .type_references
        .generic_nodes()
        .into_iter()
        .find_map(|handle| {
            let TypeReferenceNode::Generic { arguments, .. } =
                syntax.type_references.type_reference(handle)
            else {
                return None;
            };
            syntax
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .find_map(|argument| {
                    let normalization = syntax
                        .type_references
                        .const_argument_normalization(*argument)?;
                    syntax
                        .type_references
                        .const_argument_origins(normalization.selections)
                        .first()
                        .cloned()
                })
        })
        .expect("retained nominal occurrence");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve nominal receiving slot");
    let (parameter_handle, application) = program
        .tables
        .declarations
        .state_parameters
        .iter()
        .find_map(|(handle, parameter)| {
            let TypeReference::Generic(application) = &parameter.type_reference else {
                return None;
            };
            Some((handle, application.clone()))
        })
        .expect("actual generic parameter");
    let selection = crate::resolution::lowerer::PendingConstArgumentSelection {
        origin: original,
        exposure: AuthoredDeclarationSelectionExposure::PrivateImplementation,
    };
    let slot = crate::resolution::lowerer::PendingConstArgumentSlot {
        selection: 0,
        arguments: application.arguments,
        ordinal: 0,
    };
    crate::constant::finalize_const_argument_selections(
        &mut program.clone(),
        std::slice::from_ref(&selection),
        &[slot],
    )
    .expect("unchanged receiving custody replays");
    let root_value = program
        .data_definitions
        .iter()
        .find(|definition| program.symbols.display_path(definition.symbol, "::") == "Value")
        .expect("root nominal lookalike");
    let other_template = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "OtherPick")
        .expect("other generic template")
        .symbol;
    for corruption in [
        "nominal",
        "scalar",
        "unit",
        "parent",
        "conflicting_parent",
        "argument",
        "arity",
        "ordinal",
        "missing_slot",
    ] {
        let mut changed = program.clone();
        let mut changed_slot = slot;
        match corruption {
            "nominal" | "scalar" | "unit" => {
                let replacement = match corruption {
                    "nominal" => TypeReference::Named {
                        symbol: root_value.symbol,
                        name: root_value.name.clone(),
                    },
                    "scalar" => TypeReference::Named {
                        symbol: changed
                            .symbols
                            .find_top_level_by_name_and_kinds_from_source(
                                "u64",
                                &[symbols::SymbolKind::BuiltinType],
                                selection.origin.reference,
                            )
                            .expect("integer carrier"),
                        name: symbol_resolved_trees::name::DiagnosticName::generated("u64"),
                    },
                    _ => TypeReference::Unit,
                };
                changed
                    .roots
                    .const_declarations
                    .for_each_mut(|declaration| declaration.declared_type = replacement.clone());
            }
            "parent" => {
                let TypeReference::Generic(application) = &mut changed
                    .tables
                    .declarations
                    .state_parameters
                    .get_mut(parameter_handle)
                    .type_reference
                else {
                    panic!("generic parameter");
                };
                application.base_symbol = other_template;
            }
            "conflicting_parent" => {
                let mut other = application.clone();
                other.base_symbol = other_template;
                changed
                    .tables
                    .declarations
                    .child_type_references
                    .insert(TypeReference::Generic(other));
            }
            "argument" => {
                let argument = changed
                    .tables
                    .declarations
                    .child_type_references
                    .span_mut_or_empty(slot.arguments)
                    .first_mut()
                    .expect("actual argument");
                *argument = TypeReference::Unit;
            }
            "arity" => {
                let TypeReference::Generic(application) = &mut changed
                    .tables
                    .declarations
                    .state_parameters
                    .get_mut(parameter_handle)
                    .type_reference
                else {
                    panic!("generic parameter");
                };
                application.arguments = arena::HandleSpan::empty();
                changed_slot.arguments = application.arguments;
            }
            "ordinal" => changed_slot.ordinal += 1,
            "missing_slot" => {}
            _ => unreachable!(),
        }
        let slots = if corruption == "missing_slot" {
            &[][..]
        } else {
            std::slice::from_ref(&changed_slot)
        };
        assert!(
            crate::constant::finalize_const_argument_selections(
                &mut changed,
                std::slice::from_ref(&selection),
                slots
            )
            .is_err(),
            "{corruption} retains value encoding but loses nominal receiving custody"
        );
    }
}
