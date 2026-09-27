use crate::lowerer::seeded_continuation::{
    SeededContinuationError, lower_seeded_extension, retained_typed_base_is_exact_prefix,
    seeded_extension_shape_is_supported,
};
use crate::lowerer::tests::seeded_plain_data_inputs;

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
