use super::{seeded_normalized_plain_data_inputs, seeded_plain_data_inputs};
use crate::lowerer::seeded_continuation::{
    SeededContinuationError, lower_seeded_extension, plain_data_extension_shape_is_supported,
    resolved_root_shape_is_supported, retained_typed_base_is_exact_prefix,
    seeded_extension_shape_is_supported,
};

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

#[test]
fn seeded_continuation_appends_a_generated_machine_without_relowering_the_base() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; } machine authored() -> u32 { 1 }",
        "pub machine generated() -> u64 { 3 }",
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("ordinary generated machine should append from the retained base");

    assert_eq!(typed.machines().len(), expected.machines().len() + 1);
    assert_eq!(
        &typed.machines()[..expected.machines().len()],
        expected.machines()
    );
    assert_eq!(typed.machines().last().unwrap().name.as_str(), "generated");
    assert_eq!(typed.data_definitions(), expected.data_definitions());
}

#[test]
fn seeded_continuation_appends_a_plain_type_generic_machine_exactly_once() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; } machine authored() -> u32 { 1 }",
        "pub machine generated<T>(value: &T) {}",
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("plain Type-generic generated machine should append from the retained base");

    assert!(retained_typed_base_is_exact_prefix(&expected, &typed));
    assert_eq!(typed.machines().len(), expected.machines().len() + 1);
    let generated = typed.machines().last().expect("generated generic machine");
    assert_eq!(generated.name.as_str(), "generated");
    let [type_parameter] = typed.machine_type_parameters(generated) else {
        panic!("generated machine retains one exact Type parameter")
    };
    assert!(matches!(
        type_parameter.kind,
        typed_trees::data::TypeParameterKind::Type
    ));
    let [state] = typed.machine_states(generated) else {
        panic!("generated machine retains one entry state")
    };
    let [value] = typed.state_parameters(state) else {
        panic!("generated machine retains one value parameter")
    };
    let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = typed
        .type_reference_table
        .type_reference(value.type_reference)
    else {
        panic!("value remains a reference to the generic binder")
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(*referee),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == type_parameter.symbol && name.as_str() == "T"
    ));
}

#[test]
fn seeded_continuation_retains_generic_machine_type_property_bounds() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; } machine authored() -> u32 { 1 }",
        "pub machine generated<T [copy]>(value: &T) {}",
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("a property-bounded generated machine should append from the retained base");

    assert!(retained_typed_base_is_exact_prefix(&expected, &typed));
    let generated = typed.machines().last().expect("generated generic machine");
    let [type_parameter] = typed.machine_type_parameters(generated) else {
        panic!("generated machine retains one exact Type parameter")
    };
    assert!(matches!(
        type_parameter.kind,
        typed_trees::data::TypeParameterKind::Type
    ));
    assert_eq!(
        type_parameter.bounds,
        typed_trees::data::DataProperties {
            carry: None,
            multiplicity: language_semantics::Multiplicity::Unrestricted,
        },
        "the seeded continuation must preserve the authored [copy] contract",
    );
    assert_eq!(
        typed.symbols.get(type_parameter.symbol).parent,
        generated.symbol,
        "the bounded Type binder remains owned by its exact generated machine",
    );
    let [state] = typed.machine_states(generated) else {
        panic!("generated machine retains one entry state")
    };
    let [value] = typed.state_parameters(state) else {
        panic!("generated machine retains one value parameter")
    };
    let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = typed
        .type_reference_table
        .type_reference(value.type_reference)
    else {
        panic!("value remains a reference to the property-bounded binder")
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(*referee),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == type_parameter.symbol && name.as_str() == "T"
    ));
}

#[test]
fn seeded_continuation_retains_generic_machine_four_axis_carry_bound() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; } machine authored() -> u32 { 1 }",
        r#"
            pub machine generated<T [carry(
                suspension: allowed,
                cpu: any,
                thread: any,
                address: movable,
            )]>(value: &T) {}
        "#,
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("a carry-bounded generated machine should append from the retained base");

    assert!(retained_typed_base_is_exact_prefix(&expected, &typed));
    let generated = typed.machines().last().expect("generated generic machine");
    let [type_parameter] = typed.machine_type_parameters(generated) else {
        panic!("generated machine retains one exact Type parameter")
    };
    assert_eq!(
        type_parameter.bounds,
        typed_trees::data::DataProperties {
            carry: Some(language_semantics::CarryPolicy::PERMISSIVE),
            multiplicity: language_semantics::Multiplicity::Affine,
        },
        "the seeded continuation must preserve all four authored carry axes",
    );
}

#[test]
fn seeded_continuation_retains_a_structural_static_machine_binder() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; } machine authored() -> u32 { 1 }",
        r#"
            pub machine generated<machine Selected>(value: u64) -> u64
            where machine Selected(value: u64) -> u64;
            {
                Selected(value)
            }
        "#,
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("a structural static-machine binder should append from the retained base");

    assert!(retained_typed_base_is_exact_prefix(&expected, &typed));
    let generated = typed.machines().last().expect("generated generic machine");
    let [type_parameter] = typed.machine_type_parameters(generated) else {
        panic!("generated machine retains one exact static-machine parameter")
    };
    assert_eq!(type_parameter.name.as_str(), "Selected");
    assert_eq!(
        typed.symbols.get(type_parameter.symbol).parent,
        generated.symbol
    );
    let typed_trees::data::TypeParameterKind::Machine { contract } = &type_parameter.kind else {
        panic!("Selected remains a static-machine binder")
    };
    let typed_trees::data::MachineParameterContract::Structural(signature) = contract else {
        panic!("Selected retains its structural callback contract")
    };
    assert_eq!(signature.symbol, type_parameter.symbol);
    assert_eq!(signature.name.as_str(), "Selected");
    let [contract_value] = typed.state_signature_parameters(signature) else {
        panic!("Selected retains one contract value")
    };
    assert_eq!(contract_value.name.as_str(), "value");
    assert!(matches!(
        typed
            .type_reference_table
            .type_reference(contract_value.type_reference),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if typed.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::U64)
                && name.as_str() == "u64"
    ));
    assert!(matches!(
        typed
            .type_reference_table
            .type_reference(signature.return_type),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if typed.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::U64)
                && name.as_str() == "u64"
    ));
    assert!(
        typed
            .expression_table
            .iter_expressions()
            .any(|(_, expression)| {
                matches!(
                    expression,
                    typed_trees::expression::ExpressionNode::Call(call)
                        if call.target_symbol == type_parameter.symbol
                )
            })
    );
}

#[test]
fn seeded_continuation_retains_a_base_owned_nominal_static_machine_binder() {
    let (base, extension) = seeded_plain_data_inputs(
        r#"
            pub trait GeneratedOperation {
                machine apply(value: u64) -> u64;
            }
            data Authored { value: u32; }
            machine authored() -> u32 { 1 }
        "#,
        r#"
            pub machine generated<machine Selected>(value: u64) -> u64
            where machine Selected satisfies GeneratedOperation::apply;
            {
                Selected(value)
            }
        "#,
    );
    let expected = base.typed().clone();
    let operation = expected
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "GeneratedOperation")
        .expect("base owns the selected trait");
    let requirement = expected
        .trait_machine_signatures(operation)
        .first()
        .expect("base owns the selected requirement");
    let typed = lower_seeded_extension(extension, base)
        .expect("an exact base-owned nominal binder should append from the retained base");

    assert!(retained_typed_base_is_exact_prefix(&expected, &typed));
    let generated = typed.machines().last().expect("generated generic machine");
    let [type_parameter] = typed.machine_type_parameters(generated) else {
        panic!("generated machine retains one exact static-machine parameter")
    };
    let typed_trees::data::TypeParameterKind::Machine { contract } = &type_parameter.kind else {
        panic!("Selected remains a static-machine binder")
    };
    let typed_trees::data::MachineParameterContract::Nominal {
        trait_definition,
        requirement: retained_requirement,
    } = contract
    else {
        panic!("Selected retains its nominal requirement contract")
    };
    assert_eq!(*trait_definition, operation.symbol);
    assert_eq!(*retained_requirement, requirement.symbol);
    let typed_trees::data::MachineParameterContractView::Nominal {
        trait_definition: retained_trait,
        requirement: retained_signature,
    } = typed
        .machine_parameter_contract_view(contract)
        .expect("the nominal pair rejoins its retained base declarations")
    else {
        panic!("nominal contract view")
    };
    assert_eq!(retained_trait.symbol, operation.symbol);
    assert_eq!(retained_signature.symbol, requirement.symbol);
    assert!(
        typed
            .expression_table
            .iter_expressions()
            .any(|(_, expression)| {
                matches!(
                    expression,
                    typed_trees::expression::ExpressionNode::Call(call)
                        if call.target_symbol == type_parameter.symbol
                )
            })
    );
}

#[test]
fn seeded_continuation_retains_an_extension_owned_nominal_static_machine_binder() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; } machine authored() -> u32 { 1 }",
        r#"
            pub trait GeneratedOperation {
                machine apply(value: u64) -> u64;
            }
            pub machine generated<machine Selected>(value: u64) -> u64
            where machine Selected satisfies GeneratedOperation::apply;
            {
                Selected(value)
            }
        "#,
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("an exact extension-owned nominal binder should append transactionally");

    assert!(retained_typed_base_is_exact_prefix(&expected, &typed));
    assert_eq!(typed.traits().len(), expected.traits().len() + 1);
    assert_eq!(typed.machines().len(), expected.machines().len() + 1);
    let operation = typed.traits().last().expect("generated trait");
    assert_eq!(operation.name.as_str(), "GeneratedOperation");
    assert_eq!(
        typed.symbols.get(operation.symbol).parent,
        typed.symbols.root()
    );
    let [requirement] = typed.trait_machine_signatures(operation) else {
        panic!("generated trait retains one exact requirement")
    };
    assert_eq!(requirement.name.as_str(), "apply");

    let generated = typed.machines().last().expect("generated generic machine");
    let [type_parameter] = typed.machine_type_parameters(generated) else {
        panic!("generated machine retains one exact static-machine parameter")
    };
    let typed_trees::data::TypeParameterKind::Machine { contract } = &type_parameter.kind else {
        panic!("Selected remains a static-machine binder")
    };
    let typed_trees::data::MachineParameterContractView::Nominal {
        trait_definition,
        requirement: selected_requirement,
    } = typed
        .machine_parameter_contract_view(contract)
        .expect("the nominal pair rejoins its extension-owned declarations")
    else {
        panic!("nominal contract view")
    };
    assert_eq!(trait_definition.symbol, operation.symbol);
    assert_eq!(selected_requirement.symbol, requirement.symbol);
}

#[test]
fn seeded_extension_trait_continuation_rejects_broader_shapes_transactionally() {
    for (name, extension_source) in [
        (
            "boundary_trait",
            "boundary trait GeneratedOperation { machine apply(value: u64) -> u64; }",
        ),
        (
            "lifetime_trait",
            "trait GeneratedOperation<'scope> { machine apply(value: u64) -> u64; }",
        ),
        (
            "generic_trait",
            "trait GeneratedOperation<T> { machine apply(value: T) -> T; }",
        ),
        (
            "proof_contract",
            "trait GeneratedOperation { machine apply(value: bool) requires value; }",
        ),
        (
            "default_body",
            "trait GeneratedOperation { machine apply(value: u64) -> u64 { value } }",
        ),
        (
            "self_return",
            "trait GeneratedOperation { machine make() -> Self; }",
        ),
        ("empty_trait", "trait GeneratedOperation {}"),
    ] {
        let (base, extension) = seeded_plain_data_inputs(
            "data Authored { value: u32; } machine authored() -> u32 { 1 }",
            extension_source,
        );
        let expected = base.typed().clone();
        let (returned, error) = lower_seeded_extension(extension, base)
            .expect_err("broader generated traits remain outside this exact cohort");
        assert_eq!(
            error,
            SeededContinuationError::UnsupportedExtensionShape,
            "{name}"
        );
        assert_eq!(returned.into_typed(), expected, "{name}");
    }
}

#[test]
fn seeded_nominal_machine_gate_replays_the_exact_base_requirement_pair() {
    let (base, extension) = seeded_plain_data_inputs(
        "trait GeneratedOperation { machine apply(value: u64) -> u64; }",
        "machine generated<machine Selected>(value: u64) -> u64 where machine Selected satisfies GeneratedOperation::apply; { Selected(value) }",
    );
    let resolved_base = base.resolved_base_for_extension();
    let data_frontier = resolved_base.data_definitions.len();
    let machine_frontier = resolved_base.machines.len();
    let trait_frontier = resolved_base.traits.len();
    let resolved = extension.trees().clone();
    assert!(seeded_extension_shape_is_supported(
        &resolved,
        data_frontier,
        machine_frontier,
        trait_frontier,
    ));
    let generated = &resolved.machines[machine_frontier];
    let [parameter] = resolved.data_type_parameters(generated.type_parameters) else {
        panic!("generated machine retains one nominal binder")
    };

    let mut requirement_drift = resolved.clone();
    let [drifted] = requirement_drift
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(generated.type_parameters)
    else {
        unreachable!()
    };
    let symbol_resolved_trees::data::TypeParameterKind::Machine {
        contract: symbol_resolved_trees::data::MachineParameterContract::Nominal { requirement, .. },
    } = &mut drifted.kind
    else {
        panic!("Selected retains a resolved nominal contract")
    };
    assert_ne!(*requirement, parameter.symbol);
    *requirement = symbols::SymbolHandle::invalid();
    assert!(
        !seeded_extension_shape_is_supported(
            &requirement_drift,
            data_frontier,
            machine_frontier,
            trait_frontier,
        ),
        "a nominal requirement symbol cannot be detached from its base trait",
    );

    let mut path_drift = resolved.clone();
    let [drifted] = path_drift
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(generated.type_parameters)
    else {
        unreachable!()
    };
    let symbol_resolved_trees::data::TypeParameterKind::Machine {
        contract:
            symbol_resolved_trees::data::MachineParameterContract::Nominal { authored_path, .. },
    } = &mut drifted.kind
    else {
        unreachable!()
    };
    *authored_path.last_mut().expect("Trait::requirement path") =
        symbol_resolved_trees::name::DiagnosticName::generated("other");
    assert!(
        !seeded_extension_shape_is_supported(
            &path_drift,
            data_frontier,
            machine_frontier,
            trait_frontier,
        ),
        "authored nominal path drift cannot reuse the retained symbol pair",
    );

    let mut declaration_identity = resolved.clone();
    let [drifted] = declaration_identity
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(generated.type_parameters)
    else {
        unreachable!()
    };
    let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } = &mut drifted.kind
    else {
        unreachable!()
    };
    *contract = symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity;
    assert!(
        !seeded_extension_shape_is_supported(
            &declaration_identity,
            data_frontier,
            machine_frontier,
            trait_frontier,
        ),
        "a callable nominal binder cannot be substituted with declaration identity",
    );
}

#[test]
fn seeded_nominal_machine_continuation_preserves_broader_contract_fences() {
    for (name, base_source, extension_source) in [
        (
            "boundary_requirement",
            "boundary trait GeneratedOperation { machine apply(value: u64) -> u64; }",
            "machine generated<machine Selected>(value: u64) -> u64 where machine Selected satisfies GeneratedOperation::apply; { Selected(value) }",
        ),
        (
            "nested_machine_telescope",
            "trait GeneratedOperation { machine apply<machine Inner>(value: u64) -> u64 where machine Inner(value: u64) -> u64; ; }",
            "machine generated<machine Selected>(value: u64) -> u64 where machine Selected satisfies GeneratedOperation::apply; { Selected(value) }",
        ),
        (
            "proof_contract",
            "trait GeneratedOperation { machine apply(value: bool) requires value; }",
            "machine generated<machine Selected>(value: bool) where machine Selected satisfies GeneratedOperation::apply; { Selected(value) }",
        ),
    ] {
        let (base, extension) = seeded_plain_data_inputs(base_source, extension_source);
        let expected = base.typed().clone();
        let (returned, error) = lower_seeded_extension(extension, base)
            .expect_err("broader nominal contract must remain outside the checkpoint cohort");
        assert_eq!(
            error,
            SeededContinuationError::UnsupportedExtensionShape,
            "{name}"
        );
        assert_eq!(returned.into_typed(), expected, "{name}");
    }
}

#[test]
fn seeded_continuation_appends_a_lifetime_generic_machine_with_exact_binder_custody() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; } machine authored() -> u32 { 1 }",
        "pub machine generated<'loan, T>(value: &'loan T) -> &'loan T { value }",
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("lifetime-generic generated machine should append from the retained base");

    assert!(retained_typed_base_is_exact_prefix(&expected, &typed));
    assert_eq!(typed.machines().len(), expected.machines().len() + 1);
    let generated = typed.machines().last().expect("generated generic machine");
    assert_eq!(generated.name.as_str(), "generated");
    assert_eq!(
        generated
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["loan"]
    );
    let [type_parameter] = typed.machine_type_parameters(generated) else {
        panic!("generated machine retains one exact Type parameter")
    };
    let [state] = typed.machine_states(generated) else {
        panic!("generated machine retains one entry state")
    };
    let [value] = typed.state_parameters(state) else {
        panic!("generated machine retains one value parameter")
    };
    for type_reference in [value.type_reference, state.return_type] {
        let typed_trees::types::TypeReferenceNode::Reference {
            referee, lifetime, ..
        } = typed.type_reference_table.type_reference(type_reference)
        else {
            panic!("parameter and result remain lifetime-bearing references")
        };
        assert_eq!(lifetime.as_ref().map(|name| name.as_str()), Some("loan"));
        assert!(matches!(
            typed.type_reference_table.type_reference(*referee),
            typed_trees::types::TypeReferenceNode::Named { symbol, name }
                if *symbol == type_parameter.symbol && name.as_str() == "T"
        ));
    }
}

#[test]
fn seeded_continuation_appends_a_scalar_const_generic_machine_with_exact_binder_custody() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; } machine authored() -> u32 { 1 }",
        "pub machine generated<const N: u64>(value: [u8; N]) {}",
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("scalar-const generated machine should append from the retained base");

    assert!(retained_typed_base_is_exact_prefix(&expected, &typed));
    assert_eq!(typed.machines().len(), expected.machines().len() + 1);
    let generated = typed.machines().last().expect("generated generic machine");
    let [const_parameter] = typed.machine_type_parameters(generated) else {
        panic!("generated machine retains one exact const parameter")
    };
    assert_eq!(const_parameter.name.as_str(), "N");
    assert_eq!(
        typed.symbols.get(const_parameter.symbol).parent,
        generated.symbol
    );
    let typed_trees::data::TypeParameterKind::Const { type_reference } = &const_parameter.kind
    else {
        panic!("generated machine retains the const binder kind")
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(*type_reference),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if typed.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::U64)
                && name.as_str() == "u64"
    ));
    let [state] = typed.machine_states(generated) else {
        panic!("generated machine retains one entry state")
    };
    let [value] = typed.state_parameters(state) else {
        panic!("generated machine retains one value parameter")
    };
    let typed_trees::types::TypeReferenceNode::FixedArray { length, .. } = typed
        .type_reference_table
        .type_reference(value.type_reference)
    else {
        panic!("value remains a const-sized fixed array")
    };
    assert!(matches!(
        length,
        typed_trees::types::FixedArrayLength::ConstParameter { symbol, name }
            if *symbol == const_parameter.symbol && name.as_str() == "N"
    ));
}

#[test]
fn seeded_continuation_appends_a_structured_const_generic_machine_with_exact_custody() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Config { count: u8; enabled: bool; } data Indexed<const C: Config> { marker: u8; } machine authored() -> u32 { 1 }",
        "pub machine generated<const C: Config>(value: Indexed<C>) {}",
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("structured-const generated machine should append from the retained base");

    assert!(retained_typed_base_is_exact_prefix(&expected, &typed));
    assert_eq!(typed.machines().len(), expected.machines().len() + 1);
    let generated = typed.machines().last().expect("generated generic machine");
    let [const_parameter] = typed.machine_type_parameters(generated) else {
        panic!("generated machine retains one exact const parameter")
    };
    assert_eq!(const_parameter.name.as_str(), "C");
    assert_eq!(
        typed.symbols.get(const_parameter.symbol).parent,
        generated.symbol
    );
    let typed_trees::data::TypeParameterKind::Const { type_reference } = &const_parameter.kind
    else {
        panic!("generated machine retains the structured const binder kind")
    };
    let config = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Config")
        .expect("retained structured const carrier");
    assert!(matches!(
        typed.type_reference_table.type_reference(*type_reference),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == config.symbol && name.as_str() == "Config"
    ));
    let indexed = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Indexed")
        .expect("retained indexed data template");
    let [state] = typed.machine_states(generated) else {
        panic!("generated machine retains one entry state")
    };
    let [value] = typed.state_parameters(state) else {
        panic!("generated machine retains one value parameter")
    };
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = typed
        .type_reference_table
        .type_reference(value.type_reference)
    else {
        panic!("value retains the structured const application")
    };
    assert_eq!(*base_symbol, indexed.symbol);
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("structured const application retains one argument")
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(*argument),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == const_parameter.symbol && name.as_str() == "C"
    ));
}

#[test]
fn seeded_structured_const_machine_gate_replays_carrier_and_value_occurrence_exactly() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Config { count: u8; enabled: bool; } data Indexed<const C: Config> { marker: u8; } machine authored() -> u32 { 1 }",
        "pub machine generated<const C: Config>(value: Indexed<C>) {}",
    );
    let resolved_base = base.resolved_base_for_extension();
    let data_frontier = resolved_base.data_definitions.len();
    let machine_frontier = resolved_base.machines.len();
    let trait_frontier = resolved_base.traits.len();
    let resolved = extension.trees().clone();
    assert!(seeded_extension_shape_is_supported(
        &resolved,
        data_frontier,
        machine_frontier,
        trait_frontier,
    ));

    let generated = &resolved.machines[machine_frontier];
    let [const_parameter] = resolved.data_type_parameters(generated.type_parameters) else {
        panic!("generated machine retains one structured const parameter")
    };
    let mut carrier_name_drift = resolved.clone();
    let [parameter] = carrier_name_drift
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(generated.type_parameters)
    else {
        unreachable!()
    };
    let symbol_resolved_trees::data::TypeParameterKind::Const { type_reference } =
        &mut parameter.kind
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Named { name, .. } = type_reference else {
        unreachable!()
    };
    *name = symbol_resolved_trees::name::DiagnosticName::generated("WrongConfig");
    assert!(
        !seeded_extension_shape_is_supported(
            &carrier_name_drift,
            data_frontier,
            machine_frontier,
            trait_frontier,
        ),
        "a structured const carrier name cannot drift from its exact symbol",
    );

    let [state_handle] = resolved.machine_state_handles(generated.states) else {
        panic!("generated machine retains one state")
    };
    let state = resolved.machine_state(*state_handle);
    let [value] = resolved.state_parameters(state.parameters) else {
        panic!("generated machine retains one value parameter")
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) = &value.type_reference
    else {
        panic!("value remains an indexed generic application")
    };
    let mut occurrence_name_drift = resolved.clone();
    let [argument] = occurrence_name_drift
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(application.arguments)
    else {
        panic!("indexed application retains one structured const argument")
    };
    let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = argument else {
        unreachable!()
    };
    assert_eq!(*symbol, const_parameter.symbol);
    *name = symbol_resolved_trees::name::DiagnosticName::generated("WrongC");
    assert!(
        !seeded_extension_shape_is_supported(
            &occurrence_name_drift,
            data_frontier,
            machine_frontier,
            trait_frontier,
        ),
        "a structured const value occurrence cannot drift from its exact binder",
    );
}

#[test]
fn seeded_generic_machine_continuation_rejects_unadmitted_binder_kinds_transactionally() {
    for extension_source in [
        "data Recursive { value: Recursive; } machine generated<const S: Recursive>() {}",
        "machine generated<machine Selected>() where machine Selected<T>(value: T) -> T; {}",
    ] {
        let (base, extension) = seeded_plain_data_inputs(
            "data Authored { value: u32; } machine authored() -> u32 { 1 }",
            extension_source,
        );
        let expected = base.typed().clone();
        let (returned, error) = lower_seeded_extension(extension, base)
            .expect_err("only settled generic-machine binder cohorts enter this continuation rung");
        assert_eq!(error, SeededContinuationError::UnsupportedExtensionShape);
        assert_eq!(returned.into_typed(), expected);
    }
}

#[test]
fn seeded_continuation_attaches_a_monomorphic_method_to_its_exact_generated_data() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; }",
        "data Generated { value: u32; } machine Generated::read(&self) -> u32 { self.value }",
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("ordinary attached method should append from the retained base");

    let generated = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Generated")
        .expect("generated data");
    let method = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Generated::read")
        .expect("generated attached method");
    assert_eq!(method.attached_data_symbol, generated.symbol);
    assert_eq!(
        typed.symbols.get(method.symbol).parent,
        typed.symbols.root()
    );
    assert_eq!(
        &typed.data_definitions()[..expected.data_definitions().len()],
        expected.data_definitions()
    );
}

#[test]
fn seeded_plain_data_continuation_fences_runtime_generic_and_invalid_lifetime_fields() {
    for extension_source in [
        "data Generated { value: Missing; }",
        "data Generated { value: Generic<u32 in Wrapping>; }",
        "data Generated<'scope> { value: &'missing Plain; }",
        "data Generated { value: Borrowed; }",
        "data Generated<'scope> { value: Borrowed<'scope, 'scope>; }",
        "data Generated<'scope> { value: Plain<'scope>; }",
        "data Generated<'scope> { value: Generic<'scope>; }",
    ] {
        let (base, extension) = seeded_plain_data_inputs(
            r#"
                data Plain {}
                data Borrowed<'scope> { value: &'scope Plain; }
                data Generic<T> { value: T; }
            "#,
            extension_source,
        );
        let expected = base.typed().clone();
        let (returned, error) = lower_seeded_extension(extension, base).expect_err(
            "runtime-generic or invalid lifetime fields are rejected by the retained continuation",
        );
        assert_eq!(error, SeededContinuationError::UnsupportedExtensionShape);
        assert_eq!(returned.into_typed(), expected);
    }
}

#[test]
fn seeded_plain_data_continuation_rejects_cross_paired_resolved_base_transactionally() {
    let (left, _) = seeded_plain_data_inputs("data Left {}", "data Added {}");
    let (_, right_extension) = seeded_plain_data_inputs("data Right {}", "data Added {}");
    let expected = left.typed().clone();

    let (returned, error) = lower_seeded_extension(right_extension, left)
        .expect_err("resolved and typed bases cannot be cross-paired");

    assert_eq!(error, SeededContinuationError::CrossPairedResolvedBase);
    assert_eq!(returned.into_typed(), expected);
}
