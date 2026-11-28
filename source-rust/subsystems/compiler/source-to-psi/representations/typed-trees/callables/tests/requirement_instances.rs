use super::*;
use crate::source_state::{expression, machine::Machine, name::Identifier, state::State};
use crate::source_symbols::SymbolTableBuilder;
use symbols::{SymbolKind, SymbolNameRef};

#[test]
fn anonymous_requirement_type_regions_belong_to_the_binding() {
    let mut symbols = SymbolTableBuilder::default();
    let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("select"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    program.push_machine(Machine {
        symbol: owner,
        ..Default::default()
    });
    let referee = program
        .type_reference_table
        .insert(types::TypeReferenceNode::Unit);
    let mut reference = |lifetime| {
        program
            .type_reference_table
            .insert(types::TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Mutable,
                lifetime,
            })
    };
    let anonymous = reference(None);
    let unknown = reference(Some(Identifier::generated("missing")));
    let bound = |reference, ordinal| {
        RequirementLocalArgument::Static(reference).portable_identity(
            &program,
            &data::TypeParameterKind::Type,
            &static_applications::StaticMachineBindingValue::Type(reference),
            owner,
            ordinal,
        )
    };
    let first = bound(anonymous, 0).expect("the type parameter binds its input region");
    assert_eq!(bound(anonymous, 0), Some(first));
    let second = bound(anonymous, 1).expect("a second binding has its own region");
    assert_ne!(first, second);
    assert!(bound(unknown, 0).is_none());
    assert!(
        type_identity::portable_applied_type_identity(&program, anonymous, owner, &[]).is_none()
    );
}

#[test]
fn runtime_binding_carriers_keep_each_applications_lifetime_scope() {
    let mut symbols = SymbolTableBuilder::default();
    let template = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("take"));
    let left = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("left"));
    let right = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("right"));
    let binder = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("Value"));
    let scalar = symbols.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u32"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    for (symbol, name) in [(left, "left"), (right, "right")] {
        program.push_machine(Machine {
            symbol,
            name: Identifier::generated(name),
            lifetime_parameters: vec![Identifier::generated("slot")],
            ..Default::default()
        });
    }
    let scalar = program
        .type_reference_table
        .insert(types::TypeReferenceNode::Named {
            symbol: scalar,
            name: Identifier::generated("u32"),
        });
    let carrier = program
        .type_reference_table
        .insert(types::TypeReferenceNode::Reference {
            referee: scalar,
            access: language_core::ReferenceAccess::Shared,
            lifetime: Some(Identifier::generated("slot")),
        });
    let mut application = |owner| {
        let application = program.expression_table.retain_static_machine_application(
            template,
            Identifier::generated("take"),
            &[],
        );
        let row = program
            .expression_table
            .static_machine_application_mut(application);
        row.binding_declaration = template;
        row.lexical_owner = owner;
        program
            .expression_table
            .set_static_machine_application_bindings(
                application,
                &[static_applications::StaticMachineBinding {
                    parameter: binder,
                    value: static_applications::StaticMachineBindingValue::RuntimeValue {
                        type_reference: carrier,
                        subject: Default::default(),
                        expression: Default::default(),
                    },
                }],
            );
        application
    };
    let first = application(left);
    let same_scope = application(left);
    let other_scope = application(right);
    assert!(program.requirement_application_bindings_match(first, same_scope));
    assert!(!program.requirement_application_bindings_match(first, other_scope));
    assert!(!program.requirement_application_bindings_match(other_scope, first));
}

#[test]
fn requirement_machine_operands_keep_callable_identity_across_entry_selection() {
    let mut symbols = SymbolTableBuilder::default();
    let selected = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Selected"));
    let other = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Other"));
    let missing = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Missing"));
    let symbolic =
        symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("Callback"));
    let states = symbols
        .insert_children(
            selected,
            [
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (SymbolKind::State, SymbolNameRef::Static("later")),
            ],
        )
        .handles()
        .collect::<Vec<_>>();
    let other_entry = symbols
        .insert_children(other, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
        .start();
    let orphan = symbols.insert_root(SymbolKind::State, SymbolNameRef::Static("orphan"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let unit = program
        .type_reference_table
        .insert(types::TypeReferenceNode::Unit);
    for (symbol, name, state_symbols) in [
        (selected, "Selected", states.as_slice()),
        (other, "Other", std::slice::from_ref(&other_entry)),
    ] {
        let mut machine = Machine {
            symbol,
            name: Identifier::generated(name),
            ..Default::default()
        };
        for state in state_symbols {
            program.push_machine_state(
                &mut machine,
                State {
                    symbol: *state,
                    name: Identifier::from(program.symbols.name(*state)),
                    return_type: unit,
                    ..Default::default()
                },
            );
        }
        program.push_machine(machine);
    }
    let callable_kind = data::TypeParameterKind::Machine {
        contract: data::MachineParameterContract::Structural(signature::StateSignature {
            return_type: unit,
            ..Default::default()
        }),
    };
    let machine_binding = static_applications::StaticMachineBindingValue::Machine(
        expression::StaticMachineArgument::default(),
    );
    let references = [
        selected,
        states[0],
        states[1],
        other,
        other_entry,
        missing,
        orphan,
        symbolic,
    ]
    .map(|symbol| {
        let name = Identifier::from(program.symbols.name(symbol));
        program
            .type_reference_table
            .insert(types::TypeReferenceNode::Named { symbol, name })
    });
    let identities = references.map(|reference| {
        RequirementLocalArgument::Static(reference).portable_identity(
            &program,
            &callable_kind,
            &machine_binding,
            selected,
            0,
        )
    });
    assert!(identities[0].is_some());
    assert_eq!(identities[0], identities[1]);
    assert_ne!(identities[0], identities[2]);
    assert_ne!(identities[0], identities[3]);
    assert_eq!(identities[3], identities[4]);
    assert!(
        identities[5].is_none(),
        "a missing machine is not an executable callable"
    );
    assert!(
        identities[6].is_none(),
        "a state without its machine must refuse"
    );
    assert!(
        identities[7].is_some(),
        "symbolic machine binders retain their identity"
    );
    assert_ne!(identities[0], identities[7]);

    let application = |program: &mut TypedTrees, argument| {
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles([argument]);
        program
            .type_reference_table
            .insert(types::TypeReferenceNode::Generic {
                base_symbol: selected,
                base_name: Identifier::generated("Selected"),
                lifetime_arguments: Vec::new(),
                arguments,
            })
    };
    let first = application(&mut program, unit);
    let second = application(&mut program, references[3]);
    let identity = |reference| {
        RequirementLocalArgument::Static(reference).portable_identity(
            &program,
            &callable_kind,
            &machine_binding,
            selected,
            0,
        )
    };
    assert!(identity(first).is_some());
    assert_ne!(identity(first), identity(second));
    assert_ne!(identity(first), identities[0]);
    let ordinary = static_applications::StaticMachineBindingValue::Type(references[0]);
    assert_eq!(
        RequirementLocalArgument::Static(references[0]).portable_identity(
            &program,
            &data::TypeParameterKind::Type,
            &ordinary,
            selected,
            0,
        ),
        type_identity::portable_applied_type_identity(&program, references[0], selected, &[])
    );

    // Intern the same real requirement twice through the two checked
    // selections. Equality of printed keys alone would miss duplicate
    // generated declarations that later lowering must refuse.
    let receiver =
        program
            .symbols
            .insert_generated_root_from(selected, SymbolKind::Trait, "Receiver");
    let template = program
        .symbols
        .insert_generated_children(receiver, [(SymbolKind::State, "invoke")])
        .start();
    let binder = program
        .symbols
        .insert_generated_children(template, [(SymbolKind::MachineParameter, "Callback")])
        .start();
    let parameters = program
        .data_type_parameters
        .insert_many([data::TypeParameter {
            symbol: binder,
            name: Identifier::generated("Callback"),
            kind: data::TypeParameterKind::Machine {
                contract: data::MachineParameterContract::Structural(signature::StateSignature {
                    return_type: unit,
                    ..Default::default()
                }),
            },
            ..Default::default()
        }]);
    let mut declaration = crate::source_state::trait_definition::TraitDefinition {
        symbol: receiver,
        name: Identifier::generated("Receiver"),
        is_boundary: true,
        ..Default::default()
    };
    program.push_trait_machine_signature(
        &mut declaration,
        signature::StateSignature {
            symbol: template,
            name: Identifier::generated("invoke"),
            type_parameters: parameters,
            return_type: unit,
            ..Default::default()
        },
    );
    let environment =
        crate::source_state::trait_definition::AppliedTraitScope::abstract_scope(&declaration)
            .environment();
    program.push_trait_definition(declaration);
    let mut close = |symbol| {
        let actual = expression::StaticMachineArgument {
            symbol,
            path: Box::from([Identifier::from(program.symbols.name(symbol))]),
            ..Default::default()
        };
        let application = program.expression_table.retain_static_machine_application(
            template,
            Identifier::generated("invoke"),
            std::slice::from_ref(&actual),
        );
        let row = program
            .expression_table
            .static_machine_application_mut(application);
        row.binding_declaration = template;
        row.lexical_owner = selected;
        program
            .expression_table
            .set_static_machine_application_bindings(
                application,
                &[static_applications::StaticMachineBinding {
                    parameter: binder,
                    value: static_applications::StaticMachineBindingValue::Machine(actual),
                }],
            );
        program
            .expression_table
            .set_static_machine_application_lifetimes(application, template, &[]);
        program
            .instantiate_requirement_application(&environment, template, &[], application)
            .unwrap()
    };
    let machine_instance = close(selected);
    assert_eq!(
        machine_instance,
        close(states[0]),
        "the entry spelling must reuse the SAME checked instance"
    );
    assert_ne!(machine_instance, close(states[1]));
    assert_ne!(machine_instance, close(other));
    let closed = program
        .closed_requirement_application(machine_instance)
        .unwrap();
    let binding = &program.static_machine_application_bindings(closed.application)[0];
    let (_, signature) = program.trait_machine_signature(template).unwrap();
    let parameter = &program.state_signature_type_parameters(signature)[0];
    for (symbol, expected) in [
        (selected, true),
        (states[0], true),
        (states[1], false),
        (other, false),
        (missing, false),
    ] {
        let actual = expression::StaticMachineArgument {
            symbol,
            path: Box::from([Identifier::from(program.symbols.name(symbol))]),
            ..Default::default()
        };
        assert_eq!(
            program.static_binding_matches_argument(
                &parameter.kind,
                &binding.value,
                &actual,
                selected,
                selected
            ),
            expected,
            "the source call must rejoin exactly its checked machine selection"
        );
    }
    let metadata_kind = data::TypeParameterKind::Machine {
        contract: data::MachineParameterContract::RequirementIdentity,
    };
    let metadata_argument = expression::StaticMachineArgument {
        symbol: template,
        path: Box::from([Identifier::generated("invoke")]),
        ..Default::default()
    };
    let metadata_binding =
        static_applications::StaticMachineBindingValue::Machine(metadata_argument.clone());
    assert!(
        program.static_binding_matches_argument(
            &metadata_kind,
            &metadata_binding,
            &metadata_argument,
            selected,
            selected
        ),
        "declaration identity does not require an executable body"
    );
    assert!(
        !program.static_binding_matches_argument(
            &callable_kind,
            &metadata_binding,
            &metadata_argument,
            selected,
            selected
        ),
        "a declaration-only requirement is not a concrete callable value"
    );
    let metadata_reference = program
        .static_operand_type_reference_in_scope(&metadata_argument, selected)
        .unwrap();
    assert_eq!(
        RequirementLocalArgument::Static(metadata_reference)
            .portable_identity(&program, &metadata_kind, &metadata_binding, selected, 0)
            .expect("metadata-only requirements retain their complete declaration identity"),
        type_identity::portable_applied_type_identity(&program, metadata_reference, selected, &[])
            .unwrap()
    );
}
