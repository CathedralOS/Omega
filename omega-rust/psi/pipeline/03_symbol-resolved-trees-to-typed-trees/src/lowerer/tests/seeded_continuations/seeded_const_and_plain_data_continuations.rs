use crate::lowerer::seeded_continuation::{
    SeededContinuationError, lower_seeded_extension, plain_data_extension_shape_is_supported,
    resolved_root_shape_is_supported,
};
use crate::lowerer::tests::seeded_normalized_plain_data_inputs;

#[test]
fn seeded_integer_const_instance_gate_rejects_carrier_origin_and_shape_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Block<T, const N: u64> { values: [T; N]; } data Nested<T, const N: u64> { value: Block<T, N>; } data Generated { value: Nested<u16, 2>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let block_template_index = index("Block");
    let nested_template_index = index("Nested");
    let block_instance_index = index("Block<u16, 2>");
    let block_parameters = resolved.data_definitions[block_template_index].type_parameters;
    let nested_template_members = resolved.data_definitions[nested_template_index].members;
    let block_instance_members = resolved.data_definitions[block_instance_index].members;
    let block_origin_arguments = match resolved.data_definitions[block_instance_index]
        .generic_instance
        .as_ref()
    {
        Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };

    let mut unsupported_carrier = resolved.clone();
    unsupported_carrier
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(block_parameters)[1]
        .kind = symbol_resolved_trees::data::TypeParameterKind::Const {
        type_reference: symbol_resolved_trees::types::TypeReference::Unit,
    };
    assert!(
        !plain_data_extension_shape_is_supported(&unsupported_carrier, frontier),
        "the scalar const rung cannot silently widen to an unsupported carrier"
    );

    let mut noncanonical_origin = resolved.clone();
    noncanonical_origin
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(block_origin_arguments)[1] =
        symbol_resolved_trees::types::TypeReference::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: symbol_resolved_trees::name::DiagnosticName::generated("02"),
        };
    assert!(
        !plain_data_extension_shape_is_supported(&noncanonical_origin, frontier),
        "a closed const origin must retain canonical decimal spelling"
    );

    let mut wrong_substituted_length = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(field) = &mut wrong_substituted_length
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(block_instance_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::FixedArray(array) = &mut field.type_reference
    else {
        unreachable!()
    };
    array.length = symbol_resolved_trees::types::FixedArrayLength::Literal(3);
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_substituted_length, frontier),
        "the instance array length must replay the exact const argument"
    );

    let mut wrong_forwarded_binder = resolved;
    let symbol_resolved_trees::data::DataMember::Field(field) = &mut wrong_forwarded_binder
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_template_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) = &field.type_reference
    else {
        unreachable!()
    };
    let arguments = application.arguments;
    let arguments = wrong_forwarded_binder
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(arguments);
    arguments[1] = arguments[0].clone();
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_forwarded_binder, frontier),
        "a const slot cannot be redirected to an ordinary Type binder"
    );

    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Tiny<const N: u8> { tag: u8; } data Generated { value: Tiny<256>; }",
    );
    assert!(
        !plain_data_extension_shape_is_supported(
            extension.trees(),
            base.typed().data_definitions().len(),
        ),
        "a closed scalar const argument must fit its exact declared carrier"
    );
}

#[test]
fn seeded_boolean_const_instance_gate_rejects_carrier_origin_and_forwarding_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Flag<T, const ENABLED: bool> { marker: u8; } data Nested<T, const ENABLED: bool> { value: Flag<T, ENABLED>; } data Generated { value: Nested<u16, true>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let flag_template_index = index("Flag");
    let nested_template_index = index("Nested");
    let flag_parameters = resolved.data_definitions[flag_template_index].type_parameters;
    let nested_template_members = resolved.data_definitions[nested_template_index].members;
    let flag_instance_index = (frontier..resolved.data_definitions.len())
        .find(|index| {
            resolved.data_definitions[*index]
                .generic_instance
                .as_ref()
                .is_some_and(|origin| {
                    matches!(
                        origin,
                        symbol_resolved_trees::types::TypeReference::Generic(origin)
                            if origin.base_name.as_str() == "Flag"
                    )
                })
        })
        .expect("closed Flag instance");
    let flag_origin_arguments = match resolved.data_definitions[flag_instance_index]
        .generic_instance
        .as_ref()
    {
        Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };

    let mut unsupported_carrier = resolved.clone();
    unsupported_carrier
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(flag_parameters)[1]
        .kind = symbol_resolved_trees::data::TypeParameterKind::Const {
        type_reference: symbol_resolved_trees::types::TypeReference::Unit,
    };
    assert!(
        !plain_data_extension_shape_is_supported(&unsupported_carrier, frontier),
        "the Boolean const rung cannot silently widen to an unsupported carrier"
    );

    let mut noncanonical_origin = resolved.clone();
    noncanonical_origin
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(flag_origin_arguments)[1] =
        symbol_resolved_trees::types::TypeReference::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: symbol_resolved_trees::name::DiagnosticName::generated(
                language_semantics::const_value::CanonicalConstValue::new(
                    "bool",
                    "boolean4:true",
                    "TRUE",
                )
                .atom(),
            ),
        };
    assert!(
        !plain_data_extension_shape_is_supported(&noncanonical_origin, frontier),
        "a Boolean const origin must retain the exact canonical atom"
    );

    let mut wrong_forwarded_binder = resolved;
    let symbol_resolved_trees::data::DataMember::Field(field) = &mut wrong_forwarded_binder
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_template_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) = &field.type_reference
    else {
        unreachable!()
    };
    let arguments = application.arguments;
    let arguments = wrong_forwarded_binder
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(arguments);
    arguments[1] = arguments[0].clone();
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_forwarded_binder, frontier),
        "a Boolean const slot cannot be redirected to an ordinary Type binder"
    );

    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Wrong<const N: bool> { values: [u8; N]; } data Generated { value: Wrong<true>; }",
    );
    assert!(
        !plain_data_extension_shape_is_supported(
            extension.trees(),
            base.typed().data_definitions().len(),
        ),
        "a Boolean const binder cannot become an array length"
    );
}

#[test]
fn seeded_structured_const_instance_gate_replays_declarations_values_and_carriers_exactly() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Config { count: u8; enabled: bool; } data Configs {} const Configs::PRIMARY: Config = Config { count: 7, enabled: true }; data Indexed<const C: Config> { marker: u8; } data Generated { value: Indexed<Configs::PRIMARY>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(resolved_root_shape_is_supported(
        &resolved,
        &base.resolved_base_for_extension()
    ));
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let config_index = (frontier..resolved.data_definitions.len())
        .find(|index| resolved.data_definitions[*index].name.as_str() == "Config")
        .expect("Config carrier");
    let config_symbol = resolved.data_definitions[config_index].symbol;
    let config_members = resolved.data_definitions[config_index].members;
    let instance_index = (frontier..resolved.data_definitions.len())
        .find(|index| {
            matches!(
                resolved.data_definitions[*index].generic_instance.as_ref(),
                Some(symbol_resolved_trees::types::TypeReference::Generic(origin))
                    if origin.base_name.as_str() == "Indexed"
            )
        })
        .expect("closed Indexed instance");
    let origin_arguments = match resolved.data_definitions[instance_index]
        .generic_instance
        .as_ref()
    {
        Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };
    let original_atom = match &resolved
        .tables
        .declarations
        .child_type_references
        .span_or_empty(origin_arguments)[0]
    {
        symbol_resolved_trees::types::TypeReference::Named { symbol, name }
            if !symbol.is_valid() =>
        {
            language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                .expect("canonical structured const atom")
        }
        _ => unreachable!(),
    };

    let mut display_drift = resolved.clone();
    display_drift
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(origin_arguments)[0] =
        symbol_resolved_trees::types::TypeReference::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: symbol_resolved_trees::name::DiagnosticName::generated(
                language_semantics::const_value::CanonicalConstValue::new(
                    original_atom.type_name.clone(),
                    original_atom.encoding.clone(),
                    "Config { enabled: true, count: 7 }",
                )
                .atom(),
            ),
        };
    assert!(
        !plain_data_extension_shape_is_supported(&display_drift, frontier),
        "diagnostic display cannot drift from the decoded canonical value"
    );

    let mut type_claim_drift = resolved.clone();
    type_claim_drift
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(origin_arguments)[0] =
        symbol_resolved_trees::types::TypeReference::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: symbol_resolved_trees::name::DiagnosticName::generated(
                language_semantics::const_value::CanonicalConstValue::new(
                    "Other",
                    original_atom.encoding.clone(),
                    original_atom.display.clone(),
                )
                .atom(),
            ),
        };
    assert!(
        !plain_data_extension_shape_is_supported(&type_claim_drift, frontier),
        "the encoded value must claim the exact resolved carrier"
    );

    let mut recursive_carrier = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(first_field) = &mut recursive_carrier
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(config_members)[0]
    else {
        unreachable!()
    };
    first_field.type_reference = symbol_resolved_trees::types::TypeReference::Named {
        symbol: config_symbol,
        name: symbol_resolved_trees::name::DiagnosticName::generated("Config"),
    };
    assert!(
        !plain_data_extension_shape_is_supported(&recursive_carrier, frontier),
        "recursive structured const carriers remain fenced"
    );

    let mut public_support_const = resolved;
    public_support_const.const_declarations[0].is_public = true;
    assert!(
        !resolved_root_shape_is_supported(
            &public_support_const,
            &base.resolved_base_for_extension()
        ),
        "the bounded data continuation cannot grow the public const surface"
    );
}

#[test]
fn seeded_plain_data_continuation_accepts_local_instance_collections() {
    for (name, extension_source) in [
        (
            "multiple_instances",
            "data Cell<T> { value: T; } data Generated { left: Cell<u32>; right: Cell<u64>; }",
        ),
        (
            "one_wrapper_use",
            "data Cell<T> { value: T; } data Generated { value: Cell<u32>; }",
        ),
        (
            "template_wrapper_cycle",
            "data Cell<T> { value: T; companion: Generated; } data Generated { first: Cell<u32>; second: Cell<u32>; }",
        ),
        (
            "wrapper_self_cycle",
            "data Cell<T> { value: T; } data Generated { first: Cell<u32>; second: Cell<u32>; next: Generated; }",
        ),
        (
            "nominal_argument",
            "data Cell<T> { value: T; } data Generated { value: Cell<Authored>; }",
        ),
        (
            "multiple_parameters",
            "data Cell<T, U> { left: T; right: U; } data Generated { value: Cell<u32, u64>; }",
        ),
        (
            "phantom_parameter",
            "data Cell<T> { tag: u8; } data Generated { value: Cell<u32>; }",
        ),
        (
            "indirect_wrapper_use",
            "data Cell<T> { value: T; } data Generated { values: [Cell<u32>; 2]; }",
        ),
        (
            "multiple_templates_instances_and_wrappers",
            "data Cell<T> { value: T; } data Pair<A, B> { first: A; second: B; } data Item { value: u8; } data First { one: Cell<u32>; pair: Pair<u16, u64>; indirect: [Cell<u32>; 2]; } data Second { nominal: Cell<Item>; repeated: Pair<u16, u64>; }",
        ),
        (
            "nested_instances",
            "data Cell<T> { value: T; } data Outer<T> { value: T; } data Generated { value: Outer<Cell<u32>>; }",
        ),
        (
            "indirect_template_parameter",
            "data Cell<T> { values: [T; 2]; } data Generated { value: Cell<u32>; }",
        ),
        (
            "nested_fixpoint_and_indirect_substitution",
            "data Cell<T> { values: [T; 2]; } data Outer<T> { inner: Cell<T>; direct: T; } data Generated { nested: Outer<u32>; repeated: Outer<u32>; }",
        ),
        (
            "nondefault_bound",
            "data Cell<T [copy]> [copy] { value: T; } data Generated { value: Cell<u32>; }",
        ),
        (
            "nested_bound_forwarding",
            "data Cell<T [copy]> [copy] { value: T; } data Outer<U [copy]> [copy] { cell: Cell<U>; direct: U; } data Generated { value: Outer<u32>; }",
        ),
        (
            "generic_sum_instance",
            "data Maybe<T> { case None; case Some(value: T); } data Generated { value: Maybe<u32>; }",
        ),
        (
            "generic_mixed_instance",
            "data Outcome<T> { tag: u8; case Empty; case Value(value: T); } data Generated { value: Outcome<u32>; }",
        ),
        (
            "nested_generic_sum_instances",
            "data Maybe<T> { case None; case Some(values: [T; 2]); } data Outer<T> { case Empty; case Nested(value: Maybe<T>); } data Generated { value: Outer<u32>; }",
        ),
        (
            "reference_and_slice_parameter_shells",
            "data Shell<T> { shared: &T; values: [T]; nested: &[T; 2]; } data Generated { value: Shell<u32>; }",
        ),
        (
            "lifetime_bearing_reference_instance",
            "data Borrowed<'scope, T> { value: &'scope T; } data Generated<'scope> { value: Borrowed<'scope, u32>; }",
        ),
        (
            "nested_lifetime_instance_graph",
            "data Borrowed<'scope, T> { value: &'scope T; } data Nested<'scope, T> { value: Borrowed<'scope, T>; } data Generated<'scope> { value: Nested<'scope, u32>; }",
        ),
        (
            "deep_permuted_lifetime_instance_graph",
            "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Middle<'outer, 'inner, T> { values: [Borrowed<'inner, 'outer, T>; 2]; } data Outer<'first, 'second, T> { value: Middle<'second, 'first, T>; } data Generated<'one, 'two> { value: Outer<'one, 'two, u32>; }",
        ),
        (
            "nested_lifetime_sum_payload",
            "data Borrowed<'scope, T> { value: &'scope T; } data MaybeBorrow<'scope, T> { case None; case Some(value: Borrowed<'scope, T>); } data Generated<'scope> { value: MaybeBorrow<'scope, u32>; }",
        ),
        (
            "lifetime_instance_as_type_argument",
            "data Borrowed<'borrow, T> { value: &'borrow T; } data BorrowBox<'boxed, T> { value: T; } data Generated<'call> { value: BorrowBox<'call, Borrowed<'call, u32>>; }",
        ),
        (
            "nested_lifetime_instances_as_type_arguments",
            "data Borrowed<'borrow, T> { value: &'borrow T; } data BorrowBox<'boxed, T> { value: T; } data Outer<'outer, T> { value: T; } data Generated<'call> { value: Outer<'call, BorrowBox<'call, Borrowed<'call, u32>>>; }",
        ),
        (
            "ordered_multi_lifetime_instance_as_type_argument",
            "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Holder<'first, 'second, T> { value: T; } data Generated<'one, 'two> { value: Holder<'one, 'two, Borrowed<'one, 'two, u32>>; }",
        ),
        (
            "integer_const_instance_graph",
            "data Block<T, const N: u64> { values: [T; N]; } data Nested<T, const N: u64> { value: Block<T, N>; } data Generated { value: Nested<u16, 2>; }",
        ),
        (
            "signed_integer_const_instance",
            "data Offset<const N: i64> { tag: u8; } data Generated { value: Offset<-2>; }",
        ),
        (
            "zero_integer_const_array_instance",
            "data Block<const N: u64> { values: [u8; N]; } data Generated { value: Block<0>; }",
        ),
        (
            "closed_expression_const_instance",
            "data Block<const N: u64> { values: [u8; N]; } data Generated { value: Block<1 + 1>; }",
        ),
        (
            "const_instance_as_type_argument",
            "data Block<const N: u64> { values: [u8; N]; } data Box<T> { value: T; } data Generated { value: Box<Block<2> >; }",
        ),
        (
            "boolean_const_instance_graph",
            "data Flag<T, const ENABLED: bool> { marker: u8; } data Nested<T, const ENABLED: bool> { value: Flag<T, ENABLED>; } data Generated { value: Nested<u16, true>; }",
        ),
        (
            "boolean_const_instance_as_type_argument",
            "data Flag<const ENABLED: bool> { marker: u8; } data Box<T> { value: T; } data Generated { value: Box<Flag<false> >; }",
        ),
        (
            "structured_record_const_instance_graph",
            "data Leaf { count: u8; enabled: bool; } data Config { leaves: [Leaf; 2]; } data Configs {} const Configs::PRIMARY: Config = Config { leaves: [Leaf { count: 1, enabled: true }, Leaf { count: 2, enabled: false }] }; data Indexed<const C: Config> { marker: u8; } data Nested<const C: Config> { value: Indexed<C>; } data Generated { value: Nested<Configs::PRIMARY>; }",
        ),
        (
            "structured_sum_const_instance_as_type_argument",
            "data Mode { case Left(value: u8); case Right; } data Modes {} const Modes::LEFT: Mode = Mode::Left { value: 7 }; data Indexed<const M: Mode> { marker: u8; } data Box<T> { value: T; } data Generated { value: Box<Indexed<Modes::LEFT> >; }",
        ),
        (
            "arithmetic_domain_constrained_argument",
            "data Cell<T> { value: T; } data Generated { value: Cell<u32 in Wrapping>; }",
        ),
    ] {
        let (base, extension) =
            seeded_normalized_plain_data_inputs("data Authored { value: u16; }", extension_source);
        let before = base.typed().data_definitions().len();
        let const_before = base.typed().const_declarations().len();
        let typed = lower_seeded_extension(extension, base)
            .unwrap_or_else(|_| panic!("{name} should use the seeded continuation"));
        assert!(typed.data_definitions().len() > before, "{name}");
        assert_eq!(
            typed.const_declarations().len(),
            const_before + usize::from(name.starts_with("structured_")),
            "{name} retains only its exact supporting const provenance"
        );
    }
}

#[test]
fn seeded_plain_data_continuation_fences_unsupported_normalized_generic_instances() {
    for (name, extension_source) in [
        (
            "cyclic_instances",
            "data Left<T> { right: Right<T>; } data Right<T> { left: Left<T>; } data Generated { value: Left<u32>; }",
        ),
        (
            "attached_method",
            "data Cell<T> { value: T; } machine Cell::clear<T>(&self) {} data Generated { value: Cell<u32>; }",
        ),
    ] {
        let (base, extension) =
            seeded_normalized_plain_data_inputs("data Authored { value: u16; }", extension_source);
        let expected = base.typed().clone();
        let Err((returned, error)) = lower_seeded_extension(extension, base) else {
            panic!("{name} must reject transactionally")
        };
        assert_eq!(
            error,
            SeededContinuationError::UnsupportedExtensionShape,
            "{name}"
        );
        assert_eq!(returned.into_typed(), expected, "{name}");
    }
}

#[test]
fn seeded_plain_data_continuation_retains_base_owned_type_application_graph() {
    let (mut base, extension) = seeded_normalized_plain_data_inputs(
        "data Cell<T> { value: T; } data Pair<A, B> { first: A; second: B; } data Main { value: u8; }",
        "data Generated { one: Cell<u32>; two: Cell<u64>; nested: Pair<Cell<u32>, u64>; indirect: [Cell<u16>; 2]; base: Main; } data AlsoGenerated { only: Cell<u8>; }",
    );
    base.typed_mut()
        .evidence_forwardings
        .push(typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: symbols::SymbolHandle::invalid(),
            state_symbol: symbols::SymbolHandle::invalid(),
            statement_index: 37,
            source_statement_index: 41,
            target: typed_trees::name::Identifier::generated_static("base-application-target"),
            source: typed_trees::name::Identifier::generated_static("base-application-source"),
            source_conformance: None,
        });
    let before = base.typed().clone();
    let before_type_count = before.type_reference_table.type_reference_count();
    let before_symbols = before
        .symbols
        .symbols()
        .nodes()
        .iter()
        .map(|(handle, symbol)| (handle, symbol.clone()))
        .collect::<Vec<_>>();
    let resolved_ledger = extension.trees().authored_declaration_selections().clone();

    let typed = lower_seeded_extension(extension, base)
        .expect("the complete base-owned type-application graph should append");

    assert_eq!(
        &typed.data_definitions()[..before.data_definitions().len()],
        before.data_definitions()
    );
    assert_eq!(typed.evidence_forwardings, before.evidence_forwardings);
    for arena_index in 1..=u32::try_from(before_type_count).expect("type count") {
        let handle = arena::Handle::from_arena_index(arena_index);
        assert_eq!(
            typed.type_reference_table.type_reference(handle),
            before.type_reference_table.type_reference(handle)
        );
    }
    assert_eq!(
        typed
            .symbols
            .symbols()
            .nodes()
            .iter()
            .take(before_symbols.len())
            .map(|(handle, symbol)| (handle, symbol.clone()))
            .collect::<Vec<_>>(),
        before_symbols
    );
    assert!(
        typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(resolved_ledger.as_slice())
    );
    let template = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Cell")
        .expect("retained base template");
    let pair = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Pair")
        .expect("retained two-parameter base template");
    let wrapper = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("generated wrapper");
    assert!(
        typed
            .data_definitions()
            .iter()
            .all(|definition| definition.generic_instance.is_none()),
        "a cross-unit application stays structurally generic instead of inventing an extension-local instance"
    );
    let applications = typed
        .data_members(wrapper)
        .iter()
        .filter_map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            let typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } = typed
                .type_reference_table
                .type_reference(field.type_reference)
            else {
                return None;
            };
            Some((*base_symbol, lifetime_arguments, *arguments))
        })
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 3);
    assert_eq!(
        applications
            .iter()
            .filter(|(base_symbol, _, _)| *base_symbol == template.symbol)
            .count(),
        2
    );
    let (_, pair_lifetimes, pair_arguments) = applications
        .iter()
        .find(|(base_symbol, _, _)| *base_symbol == pair.symbol)
        .expect("nested pair application remains explicit");
    assert!(pair_lifetimes.is_empty());
    let pair_arguments = typed
        .type_reference_table
        .type_reference_handles(*pair_arguments);
    assert_eq!(pair_arguments.len(), 2);
    let pair_argument_nodes = pair_arguments
        .iter()
        .map(|argument| typed.type_reference_table.type_reference(*argument))
        .collect::<Vec<_>>();
    assert!(matches!(
        pair_argument_nodes[0],
        typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
            if *base_symbol == template.symbol
    ));
    assert!(matches!(
        pair_argument_nodes[1],
        typed_trees::types::TypeReferenceNode::Named { symbol, .. }
            if typed.symbols.name(*symbol) == "u64"
    ));

    let indirect = typed
        .data_members(wrapper)
        .iter()
        .find_map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == "indirect").then_some(field.type_reference)
        })
        .expect("indirect generic field");
    let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
        typed.type_reference_table.type_reference(indirect)
    else {
        panic!("indirect application retains its fixed-array shell")
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(*element_type),
        typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
            if *base_symbol == template.symbol
    ));

    let second_wrapper = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "AlsoGenerated")
        .expect("second generated wrapper");
    let [typed_trees::data::DataMember::Field(only)] = typed.data_members(second_wrapper) else {
        panic!("second wrapper retains one field")
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(only.type_reference),
        typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
            if *base_symbol == template.symbol
    ));
}

#[test]
fn seeded_base_owned_type_application_validator_rejects_identity_and_arity_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Cell<T> { value: T; } data Main { value: u8; }",
        "data Generated { first: Cell<u32>; second: Cell<u32>; base: Main; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));
    let wrapper = resolved.data_definitions.iter().nth(frontier).unwrap();
    let wrapper_members = wrapper.members;

    let mut wrong_base_name = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(first) = wrong_base_name
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut first.type_reference
    else {
        unreachable!()
    };
    application.base_name = symbol_resolved_trees::name::DiagnosticName::generated("Other");
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_base_name,
        frontier
    ));

    let mut wrong_base_symbol = resolved.clone();
    let wrapper_symbol = wrong_base_symbol
        .data_definitions
        .iter()
        .nth(frontier)
        .unwrap()
        .symbol;
    let symbol_resolved_trees::data::DataMember::Field(first) = wrong_base_symbol
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut first.type_reference
    else {
        unreachable!()
    };
    application.base_symbol = wrapper_symbol;
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_base_symbol,
        frontier
    ));

    let mut missing_argument = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(first) = missing_argument
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut first.type_reference
    else {
        unreachable!()
    };
    application.arguments = arena::HandleSpan::empty();
    assert!(!plain_data_extension_shape_is_supported(
        &missing_argument,
        frontier
    ));

    let mut wrong_parameter_name = resolved;
    let parameter_span = wrong_parameter_name.data_definitions[0].type_parameters;
    wrong_parameter_name
        .tables
        .declarations
        .data_type_parameters
        .get_mut(parameter_span.start())
        .name = symbol_resolved_trees::name::DiagnosticName::generated("U");
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_parameter_name,
        frontier
    ));
}

#[test]
fn seeded_plain_data_continuation_accepts_broader_base_owned_generic_applications() {
    for (name, base_source, extension_source) in [
        (
            "single_use",
            "data Cell<T> { value: T; }",
            "data Generated { value: Cell<u32>; }",
        ),
        (
            "distinct_arguments",
            "data Cell<T> { value: T; }",
            "data Generated { first: Cell<u32>; second: Cell<u64>; }",
        ),
        (
            "indirect_use",
            "data Cell<T> { value: T; }",
            "data Generated { first: [Cell<u32>; 2]; second: Cell<u32>; }",
        ),
        (
            "attached_method",
            "data Cell<T> { value: T; } machine Cell::clear<T>(&self) {}",
            "data Generated { first: Cell<u32>; second: Cell<u32>; }",
        ),
        (
            "indirect_parameter",
            "data Cell<T> { values: [T; 2]; }",
            "data Generated { first: Cell<u32>; second: Cell<u32>; }",
        ),
        (
            "nominal_argument",
            "data Item { value: u8; } data Cell<T> { value: T; }",
            "data Generated { value: Cell<Item>; }",
        ),
        (
            "nondefault_bound",
            "data Cell<T [copy]> { value: T; }",
            "data Generated { value: Cell<u32>; }",
        ),
        (
            "lifetime_and_type_arguments",
            "data Cell<'item, T> { value: &'item T; }",
            "data Generated<'owner> { value: Cell<'owner, u32>; }",
        ),
    ] {
        let (base, extension) = seeded_normalized_plain_data_inputs(base_source, extension_source);
        let before = base.typed().data_definitions().len();
        let typed = lower_seeded_extension(extension, base)
            .unwrap_or_else(|(_, error)| panic!("{name} should continue: {error:?}"));
        assert!(typed.data_definitions().len() > before, "{name}");
    }
}
