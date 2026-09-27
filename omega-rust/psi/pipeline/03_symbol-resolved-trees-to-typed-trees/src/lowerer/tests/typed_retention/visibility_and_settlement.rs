use crate::lowerer::lower_symbol_resolved_trees;

#[test]
fn expression_embedded_zero_value_types_keep_contract_exposure() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        data Marker {}
        pub proposition public_zero() =
            zero_value<Marker>() == zero_value<Marker>();
        proposition private_zero() =
            zero_value<Marker>() == zero_value<Marker>();
    "#;
    let resolved = crate::front_end::resolved_program(source);
    let marker = resolved
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Marker")
        .expect("Marker data")
        .symbol;
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let mut exposures = typed
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| {
            (selection.kind() == Kind::TypeReference
                && matches!(
                    selection.target(),
                    Target::Resolved(target) if target.selected_symbol() == marker
                ))
            .then_some(selection.exposure())
        })
        .collect::<Vec<_>>();
    exposures.sort_by_key(|exposure| match exposure {
        Exposure::PrivateImplementation => 0,
        Exposure::PublicInterface => 1,
    });
    assert_eq!(
        exposures,
        [
            Exposure::PrivateImplementation,
            Exposure::PrivateImplementation,
            Exposure::PublicInterface,
            Exposure::PublicInterface,
        ]
    );
}

#[test]
fn expression_embedded_cast_targets_keep_contract_exposure() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        data Marker {}
        pub proposition public_cast(value: Marker) = (value as Marker) == value;
        proposition private_cast(value: Marker) = (value as Marker) == value;
    "#;
    let resolved = crate::front_end::resolved_program(source);
    let marker = resolved
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Marker")
        .expect("Marker data")
        .symbol;
    let cast_targets = resolved
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let symbol_resolved_trees::expression::ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            Some(resolved.child_type_reference(cast.target_type).clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(cast_targets.len(), 2, "cast targets: {cast_targets:#?}");
    let cast_target_spans = cast_targets
        .iter()
        .filter_map(|target| {
            let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = target else {
                return None;
            };
            (*symbol == marker).then_some(name.source_span())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cast_target_spans.len(),
        2,
        "cast targets: {cast_targets:#?}"
    );
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let mut exposures = typed
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| {
            (selection.kind() == Kind::TypeReference
                && cast_target_spans.contains(&selection.source_span())
                && matches!(
                    selection.target(),
                    Target::Resolved(target) if target.selected_symbol() == marker
                ))
            .then_some(selection.exposure())
        })
        .collect::<Vec<_>>();
    exposures.sort_by_key(|exposure| match exposure {
        Exposure::PrivateImplementation => 0,
        Exposure::PublicInterface => 1,
    });
    assert_eq!(
        exposures,
        [Exposure::PrivateImplementation, Exposure::PublicInterface,]
    );
}

#[test]
fn retains_public_operator_visibility_and_signature_exposure() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        pub data Token [copy] { value: u64; }
        pub operator < Token::less(left: Token, right: Token) -> bool;
    "#;
    let resolved = crate::front_end::resolved_program(source);
    let token = resolved
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Token")
        .expect("Token data")
        .symbol;
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let [operator] = typed.operators() else {
        panic!("one root operator")
    };
    assert!(operator.is_public);
    assert!(
        typed
            .authored_declaration_selections()
            .iter()
            .filter(|selection| {
                selection.kind() == Kind::TypeReference
                    && matches!(
                        selection.target(),
                        Target::Resolved(target) if target.selected_symbol() == token
                    )
            })
            .all(|selection| selection.exposure() == Exposure::PublicInterface)
    );
    assert!(typed.snapshot().roots.operators[0].is_public);
}

#[test]
fn retains_public_data_trait_and_wire_visibility_in_typed_trees() {
    let typed = crate::front_end::typed_program(
        "pub data PublicRecord { value: u32; } pub data Packet { #1 value: u32; } pub trait PublicTrait {}",
    );
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "PublicRecord")
        .expect("typed public data");
    let wire_data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Packet")
        .expect("typed wire-derived data");
    let wire_schema = typed
        .wire_schemas()
        .iter()
        .find(|schema| schema.name.as_str() == "Packet")
        .expect("typed wire schema");
    let trait_definition = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "PublicTrait")
        .expect("typed public trait");

    assert!(data.is_public);
    assert!(wire_data.is_public);
    assert!(wire_schema.is_public);
    assert!(trait_definition.is_public);
    let snapshot = typed.snapshot();
    assert!(
        snapshot
            .roots
            .wire_schemas
            .iter()
            .any(|schema| schema.name == "Packet" && schema.is_public)
    );
    assert!(
        snapshot
            .roots
            .traits
            .iter()
            .any(|definition| definition.name == "PublicTrait" && definition.is_public)
    );
}

#[test]
fn top_level_requirement_settlement_rejects_an_exact_wrong_supply_machine() {
    let source = r#"
        pub boundary requirement InterruptAcknowledgement::complete();

        machine complete_provider()
        satisfies InterruptAcknowledgement::complete
        {
        }
    "#;
    let mut resolved = crate::front_end::resolved_program(source);
    let requirement = resolved
        .machines
        .find_mut(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("top-level requirement");
    let ordinary_symbol = requirement.symbol;
    requirement.supply_mode = language_semantics::MachineSupplyMode::Boundary;

    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let satisfier = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "complete_provider")
        .expect("typed checked satisfier");
    let [conformance] = typed.machine_trait_conformances(satisfier) else {
        panic!("one typed satisfies edge")
    };
    assert_eq!(conformance.symbol, ordinary_symbol);
    assert!(!conformance.requirement_symbol.is_valid());
    assert!(
        typed_trees::machine::resolve_satisfied_declaration(&typed, satisfier, conformance,)
            .is_none()
    );
}
