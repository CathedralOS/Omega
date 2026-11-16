//! A retained private boundary schema grants no source-call visibility.

use super::Program;
use semantic_vocabulary::PackageKeyIdentity;
use source::{DependencyScope, SourceMap, SourceOrigin, SourceSpan, Span};
use std::sync::Arc;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::machine::{Machine, MachineBodyStatus};
use typed_trees::name::Identifier;
use typed_trees::signature::{AuthoredServiceReachRow, StateSignature};
use typed_trees::state::State;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::TypeReferenceNode;

struct Fixture {
    typed: TypedTrees,
    app: SymbolHandle,
    helper: SymbolHandle,
    callback_binder: SymbolHandle,
    owner: SymbolHandle,
    requirement: SymbolHandle,
}

impl Fixture {
    fn static_callback(&mut self, forwarded: bool) -> (SymbolHandle, SymbolHandle) {
        use typed_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
        let binder = self.callback_binder;
        let unit = self
            .typed
            .machine_states(self.typed.machine_by_symbol(self.app).unwrap())[0]
            .return_type;
        let parameters = self.typed.data_type_parameters.insert_many([TypeParameter {
            symbol: binder,
            name: Identifier::from("Visit"),
            kind: TypeParameterKind::Machine {
                contract: MachineParameterContract::Structural(StateSignature {
                    symbol: binder,
                    return_type: unit,
                    ..Default::default()
                }),
            },
            ..Default::default()
        }]);
        self.typed.machine_mut(1).type_parameters = parameters;
        let occurrence = self
            .typed
            .record_resolved_authored_declaration_selection_once(
                self.typed.symbols.symbol_source_span(self.app).unwrap(),
                typed_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                typed_trees::AuthoredDeclarationSelectionKind::StaticArgument,
                self.app,
            )
            .unwrap();
        let argument = StaticMachineArgument {
            symbol: self.app,
            authored_selection: Some(occurrence),
            ..Default::default()
        };
        let template_state = self
            .typed
            .machine_states(self.typed.machine_by_symbol(self.helper).unwrap())[0]
            .clone();
        let mut supplier = self.app;
        for _ in 0..if forwarded { 2 } else { 1 } {
            let instance = self.typed.symbols.insert_generated_root_from(
                self.helper,
                SymbolKind::Machine,
                "selected",
            );
            let state = self
                .typed
                .symbols
                .insert_generated_children_from(
                    instance,
                    [(template_state.symbol, SymbolKind::State, "run")],
                )
                .start();
            let application = self
                .typed
                .expression_table
                .retain_static_machine_application(
                    self.helper,
                    Identifier::from("helper"),
                    std::slice::from_ref(&argument),
                );
            let source = self
                .typed
                .expression_table
                .static_machine_application_mut(application);
            source.lexical_owner = supplier;
            source.binding_declaration = self.helper;
            self.typed
                .expression_table
                .set_static_machine_application_bindings(
                    application,
                    &[StaticMachineBinding {
                        parameter: binder,
                        value: StaticMachineBindingValue::Machine(argument.clone()),
                    }],
                );
            let states = self.typed.machine_states.insert_many([State {
                symbol: state,
                ..template_state.clone()
            }]);
            self.typed.push_machine(Machine {
                symbol: instance,
                states,
                body_is_present: true,
                body_status: MachineBodyStatus::Complete,
                ..Default::default()
            });
            let correspondence = self.typed.captured_machine_states.insert_many([
                typed_trees::CapturedMachineState {
                    template: template_state.symbol,
                    instance: state,
                },
            ]);
            self.typed
                .machine_specializations
                .push(typed_trees::MachineSpecialization {
                    template: self.helper,
                    instance,
                    template_parameters: parameters,
                    binding_application: application,
                    states: correspondence,
                    ..Default::default()
                });
            supplier = instance;
        }
        (supplier, binder)
    }
    fn new(declares_no_crash_route: bool) -> Self {
        Self::with_effects(declares_no_crash_route, false, false)
    }

    fn with_effects(declares_no_crash_route: bool, suspends: bool, blocks: bool) -> Self {
        Self::with_scope_and_effects(
            declares_no_crash_route,
            suspends,
            blocks,
            DependencyScope::Product,
        )
    }

    fn with_scope_and_effects(
        declares_no_crash_route: bool,
        suspends: bool,
        blocks: bool,
        scope: DependencyScope,
    ) -> Self {
        Self::with_owner_provenance(declares_no_crash_route, suspends, blocks, scope, true)
    }

    fn with_owner_provenance(
        declares_no_crash_route: bool,
        suspends: bool,
        blocks: bool,
        scope: DependencyScope,
        retain_provenance: bool,
    ) -> Self {
        let app_text = "machine entry() { Raw::read(); }";
        let supplier_text = format!(
            "boundary trait Raw {{ machine read() reaches none {} {} {}; }}\n\
             machine helper() {{ Raw::read(); }}",
            if declares_no_crash_route {
                "crashes none"
            } else {
                ""
            },
            if suspends { "suspends" } else { "" },
            if blocks { "blocks" } else { "" },
        );
        let mut sources = SourceMap::default();
        let ids = [
            (1, "app/main.omg", "app", app_text),
            (2, "dep/raw.omg", "dep", &supplier_text),
        ]
        .into_iter()
        .map(|(package, path, root, text)| {
            sources
                .add_checked_instance(
                    path.into(),
                    text,
                    root.into(),
                    Some(PackageKeyIdentity::from_digest([package; 32]).unwrap()),
                    SourceOrigin::User,
                    source::SourceResolutionStratum::Base,
                    scope,
                )
                .source_id
        })
        .collect::<Vec<_>>();
        let span = |source, text: &str, name: &str| {
            let start = text.find(name).unwrap();
            SourceSpan::new(source, Span::new(start, start + name.len()))
        };
        let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = symbols.insert_children(
            root,
            [
                (
                    SymbolKind::Machine,
                    SymbolNameRef::Source(span(ids[0], app_text, "entry")),
                ),
                (
                    SymbolKind::Machine,
                    SymbolNameRef::Source(span(ids[1], &supplier_text, "helper")),
                ),
                (
                    SymbolKind::Trait,
                    if retain_provenance {
                        SymbolNameRef::Source(span(ids[1], &supplier_text, "Raw"))
                    } else {
                        SymbolNameRef::Static("Raw")
                    },
                ),
            ],
        );
        let [app, helper, owner] = [0, 1, 2].map(|index| declarations.handle_at(index));
        let requirement = symbols
            .insert_children(
                owner,
                [(
                    SymbolKind::State,
                    SymbolNameRef::Source(span(ids[1], &supplier_text, "read")),
                )],
            )
            .start();
        let mut callback_binder = SymbolHandle::invalid();
        let states = [app, helper].map(|machine| {
            let mut children = vec![(SymbolKind::State, SymbolNameRef::Static("run"))];
            if machine == helper {
                children.push((SymbolKind::MachineParameter, SymbolNameRef::Static("Visit")));
            }
            let children = symbols.insert_children(machine, children);
            if machine == helper {
                callback_binder = children.handle_at(1);
            }
            children.start()
        });
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let unit = typed.type_reference_table.insert(TypeReferenceNode::Unit);
        for ((symbol, state), name) in [app, helper]
            .into_iter()
            .zip(states)
            .zip(["entry", "helper"])
        {
            let states = typed.machine_states.insert_many([State {
                symbol: state,
                return_type: unit,
                ..Default::default()
            }]);
            typed.push_machine(Machine {
                symbol,
                name: Identifier::generated(name),
                states,
                body_is_present: true,
                body_status: MachineBodyStatus::Complete,
                ..Default::default()
            });
        }
        let machines = typed.trait_machine_signatures.insert_many([StateSignature {
            symbol: requirement,
            name: Identifier::generated("read"),
            return_type: unit,
            declares_no_crash_route,
            suspends,
            blocks,
            ..Default::default()
        }]);
        typed.push_trait_definition(TraitDefinition {
            symbol: owner,
            name: Identifier::generated("Raw"),
            is_boundary: true,
            machines,
            ..Default::default()
        });
        typed
            .authored_service_reach_rows
            .push(AuthoredServiceReachRow {
                owner: requirement,
                keyword_source_spans: Vec::new(),
                targets: Vec::new(),
                installation_bound: false,
            });
        Self {
            typed,
            app,
            helper,
            callback_binder,
            owner,
            requirement,
        }
    }

    fn call_source(&self, caller: SymbolHandle) -> crate::SourceOrigin {
        crate::SourceOrigin::of(self.typed.symbols.symbol_source_span(caller).unwrap())
    }
}

#[test]
fn forwarded_private_callbacks_keep_the_original_selection_visibility() {
    for forwarded in [false, true] {
        let mut fixture = Fixture::new(true);
        let (instance, binder) = fixture.static_callback(forwarded);
        let mut program = Program::new(&fixture.typed, false);
        let caller = program.machine(instance).unwrap();
        let (authored, selected) = program
            .static_machine_target(caller, fixture.app, binder)
            .unwrap();
        assert_eq!(authored, binder);
        let retargeted = program
            .call_selected(
                caller,
                &[],
                fixture.helper,
                &[],
                fixture.call_source(fixture.helper),
                None,
                selected,
            )
            .unwrap_err();
        assert_eq!(
            retargeted.detail,
            "call static call changed its selected target"
        );
        let call = program
            .call_selected(
                caller,
                &[],
                fixture.app,
                &[],
                fixture.call_source(fixture.app),
                None,
                selected,
            )
            .unwrap();
        program.retain_static_reach(caller, call, selected).unwrap();
        let refused = program
            .call(
                caller,
                &[],
                fixture.app,
                &[],
                fixture.call_source(fixture.app),
                None,
            )
            .unwrap_err();
        assert_eq!(refused.detail, "call private machine of another package");
        assert!(
            program
                .static_machine_target(caller, fixture.helper, binder)
                .is_err(),
            "a binder cannot redirect its target"
        );
        assert!(
            program
                .static_machine_target(caller, fixture.app, fixture.helper)
                .is_err(),
            "an unrelated declaration is not a callback binder"
        );
    }
}

#[test]
fn a_callback_receipt_cannot_retarget_its_authored_selection() {
    use typed_trees::static_applications::StaticMachineBindingValue;
    use typed_trees::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
    };
    for (kind, exposure, wrong_target) in [
        (Kind::Call, Exposure::PrivateImplementation, false),
        (Kind::StaticArgument, Exposure::PublicInterface, false),
        (Kind::StaticArgument, Exposure::PrivateImplementation, true),
    ] {
        let mut fixture = Fixture::new(true);
        let (instance, binder) = fixture.static_callback(true);
        let occurrence = fixture
            .typed
            .record_resolved_authored_declaration_selection_once(
                fixture
                    .typed
                    .symbols
                    .symbol_source_span(fixture.app)
                    .unwrap(),
                exposure,
                kind,
                if wrong_target {
                    fixture.helper
                } else {
                    fixture.app
                },
            )
            .unwrap();
        let application = fixture
            .typed
            .machine_specialization(instance)
            .unwrap()
            .binding_application;
        let StaticMachineBindingValue::Machine(argument) = &mut fixture
            .typed
            .expression_table
            .static_machine_application_bindings_mut(application)[0]
            .value
        else {
            unreachable!()
        };
        argument.authored_selection = Some(occurrence);
        let mut program = Program::new(&fixture.typed, false);
        let caller = program.machine(instance).unwrap();
        assert!(
            program
                .static_machine_target(caller, fixture.app, binder)
                .is_err()
        );
    }
}

#[test]
fn a_callback_selection_in_another_package_does_not_grant_private_visibility() {
    use typed_trees::static_applications::StaticMachineBindingValue;
    let mut fixture = Fixture::new(true);
    let (instance, binder) = fixture.static_callback(true);
    let occurrence = fixture
        .typed
        .record_resolved_authored_declaration_selection_once(
            fixture
                .typed
                .symbols
                .symbol_source_span(fixture.helper)
                .unwrap(),
            typed_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            typed_trees::AuthoredDeclarationSelectionKind::StaticArgument,
            fixture.app,
        )
        .unwrap();
    let application = fixture
        .typed
        .machine_specialization(instance)
        .unwrap()
        .binding_application;
    let StaticMachineBindingValue::Machine(argument) = &mut fixture
        .typed
        .expression_table
        .static_machine_application_bindings_mut(application)[0]
        .value
    else {
        unreachable!()
    };
    argument.authored_selection = Some(occurrence);
    let mut program = Program::new(&fixture.typed, false);
    let caller = program.machine(instance).unwrap();
    let refused = program
        .static_machine_target(caller, fixture.app, binder)
        .unwrap_err();
    assert_eq!(refused.detail, "call private machine of another package");
}

#[test]
fn a_public_interface_can_select_a_public_callback() {
    use typed_trees::static_applications::StaticMachineBindingValue;
    let mut fixture = Fixture::new(true);
    let (instance, binder) = fixture.static_callback(true);
    fixture.typed.machine_mut(0).is_public = true;
    let occurrence = fixture
        .typed
        .record_resolved_authored_declaration_selection_once(
            fixture
                .typed
                .symbols
                .symbol_source_span(fixture.app)
                .unwrap(),
            typed_trees::AuthoredDeclarationSelectionExposure::PublicInterface,
            typed_trees::AuthoredDeclarationSelectionKind::StaticArgument,
            fixture.app,
        )
        .unwrap();
    let application = fixture
        .typed
        .machine_specialization(instance)
        .unwrap()
        .binding_application;
    let StaticMachineBindingValue::Machine(argument) = &mut fixture
        .typed
        .expression_table
        .static_machine_application_bindings_mut(application)[0]
        .value
    else {
        unreachable!()
    };
    argument.authored_selection = Some(occurrence);
    let mut program = Program::new(&fixture.typed, false);
    let caller = program.machine(instance).unwrap();
    let (_, selected) = program
        .static_machine_target(caller, fixture.app, binder)
        .unwrap();
    program
        .call_selected(
            caller,
            &[],
            fixture.app,
            &[],
            fixture.call_source(fixture.app),
            None,
            selected,
        )
        .unwrap();
}

#[test]
fn reusing_a_specialization_checks_each_callers_static_selection() {
    use typed_trees::expression::StaticMachineArgument;
    let mut fixture = Fixture::new(true);
    let (instance, _) = fixture.static_callback(false);
    let original = fixture
        .typed
        .machine_specialization(instance)
        .unwrap()
        .binding_application;
    let mut argument = fixture
        .typed
        .expression_table
        .static_machine_application_arguments(original)[0]
        .clone();
    let foreign = fixture
        .typed
        .record_resolved_authored_declaration_selection_once(
            fixture
                .typed
                .symbols
                .symbol_source_span(fixture.helper)
                .unwrap(),
            typed_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            typed_trees::AuthoredDeclarationSelectionKind::StaticArgument,
            fixture.app,
        )
        .unwrap();
    argument.authored_selection = Some(foreign);
    let forged = fixture
        .typed
        .expression_table
        .retain_static_machine_application(fixture.helper, Identifier::from("helper"), &[argument]);
    let missing = fixture
        .typed
        .expression_table
        .retain_static_machine_application(
            fixture.helper,
            Identifier::from("helper"),
            &[StaticMachineArgument {
                symbol: fixture.app,
                ..Default::default()
            }],
        );
    let mut program = Program::new(&fixture.typed, false);
    let caller = program.machine(fixture.app).unwrap();
    program
        .applies_nothing(caller, instance, original, &[])
        .unwrap();
    let refused = program
        .applies_nothing(caller, instance, forged, &[])
        .unwrap_err();
    assert_eq!(refused.detail, "call private machine of another package");
    assert!(
        program
            .applies_nothing(caller, instance, missing, &[])
            .is_err()
    );
}

#[test]
fn selecting_a_private_native_schema_does_not_require_app_call_visibility() {
    let fixture = Fixture::new(true);
    let mut program = Program::new(&fixture.typed, false);
    let app = program.machine(fixture.app).unwrap();
    program
        .selected_declarations(app, &[fixture.owner, fixture.requirement])
        .unwrap();
    assert_eq!(program.boundary_symbols.len(), 1);
}

#[test]
fn a_private_boundary_call_is_allowed_inside_its_package() {
    let fixture = Fixture::new(true);
    let mut program = Program::new(&fixture.typed, false);
    let helper = program.machine(fixture.helper).unwrap();
    program
        .call(
            helper,
            &[],
            fixture.requirement,
            &[],
            fixture.call_source(fixture.helper),
            None,
        )
        .unwrap();
}

#[test]
fn a_selected_private_boundary_still_refuses_another_packages_call() {
    let fixture = Fixture::new(true);
    let mut program = Program::new(&fixture.typed, false);
    let app = program.machine(fixture.app).unwrap();
    let helper = program.machine(fixture.helper).unwrap();
    program
        .call(
            helper,
            &[],
            fixture.requirement,
            &[],
            fixture.call_source(fixture.helper),
            None,
        )
        .unwrap();
    program
        .selected_declarations(app, &[fixture.owner, fixture.requirement])
        .unwrap();
    assert_eq!(program.boundary_symbols.len(), 1);
    let refused = program
        .call(
            app,
            &[],
            fixture.requirement,
            &[],
            fixture.call_source(fixture.app),
            None,
        )
        .unwrap_err();
    assert_eq!(refused.detail, "call private machine of another package");
}

#[test]
fn selected_native_schemas_still_require_their_complete_contract() {
    let fixture = Fixture::new(false);
    let mut program = Program::new(&fixture.typed, false);
    let app = program.machine(fixture.app).unwrap();
    let refused = program
        .selected_declarations(app, &[fixture.owner, fixture.requirement])
        .unwrap_err();
    assert_eq!(
        refused.detail,
        "call boundary requirement must declare its crash ceiling"
    );
    assert!(program.boundary_symbols.is_empty());
}

#[test]
fn selected_native_declarations_preserve_each_operational_flag() {
    for (suspends, blocks) in [(true, false), (false, true)] {
        let fixture = Fixture::with_effects(true, suspends, blocks);
        // Selecting a declaration does not invoke it. Read the declaration
        // module through ordinary elaboration and its publication metadata.
        let elaboration = crate::elaborate(
            &fixture.typed,
            crate::Compilation {
                entries: &[],
                subjects: &[],
                purpose: DependencyScope::Product,
                selected: &[fixture.owner, fixture.requirement],
                declaration_metadata: &[],
                bindings: &[],
                supplies: &[],
            },
        )
        .expect("the selected boundary retains its declared operational contract");
        assert_eq!(elaboration.module.boundary_machines.len(), 1);
        let published = &elaboration.module.boundary_machines[0]
            .declaration
            .as_ref()
            .expect("ordinary publication retains the checked boundary declaration")
            .source;
        assert_eq!(
            (
                published.published_may_suspend,
                published.published_may_block
            ),
            (suspends, blocks)
        );
    }
}

#[test]
fn a_requirement_signature_retains_operational_promises_without_an_executable_body() {
    for (suspends, blocks) in [(true, false), (false, true)] {
        let fixture = Fixture::with_effects(true, suspends, blocks);
        let (_, requirement) = fixture
            .typed
            .trait_machine_signature(fixture.requirement)
            .unwrap();
        let mut program = Program::new(&fixture.typed, false);
        let identity = program
            .witnessed(
                fixture.owner,
                fixture.requirement,
                &[],
                fixture.helper,
                None,
            )
            .expect("retain the signature for ordinary operational checking");
        assert_eq!(program.symbols.get(identity), Some(&requirement.symbol));
        let signature = program.signatures.get(identity).unwrap();
        assert_eq!(signature.operational.suspension, Some(suspends));
        assert_eq!(signature.operational.blocking, Some(blocks));
        // Retaining an interface neither manufactures its body nor admits an
        // executable call. Invocation acknowledgements and ceilings are checked
        // by the ordinary lowered-form checker.
        let refused = program.body(identity).unwrap_err();
        assert_eq!(
            refused.detail,
            "call target is not a machine declaration while reading the body of `Raw::read`"
        );
    }
}

#[test]
fn a_build_boundary_publishes_its_own_complete_service_contract() {
    let fixture = Fixture::with_scope_and_effects(true, false, false, DependencyScope::Build);
    let owner = fixture
        .typed
        .trait_definition_by_symbol(fixture.owner)
        .expect("the exact declaring trait");
    let project = |scope| {
        typed_trees::service_declarations::projection::from_typed(
            typed_trees::provider_declarations::ProviderPlanningContext::new(&fixture.typed, scope),
            owner,
        )
    };
    let refused = project(DependencyScope::Product)
        .err()
        .expect("the Product catalog must not claim a Build source instance");
    assert!(refused.message.contains("Raw"), "{}", refused.message);
    assert!(
        refused
            .message
            .contains("outside the Product dependency scope")
    );
    assert_eq!(
        refused.source_span,
        fixture
            .typed
            .symbols
            .symbol_provenance_source_span(fixture.owner)
    );
    let expected = project(DependencyScope::Build)
        .expect("the Build declaration has a complete schema")
        .declaration
        .declaration_commitment();
    let elaboration = crate::elaborate(
        &fixture.typed,
        crate::Compilation {
            entries: &[],
            subjects: &[],
            purpose: DependencyScope::Build,
            selected: &[fixture.owner, fixture.requirement],
            declaration_metadata: &[],
            bindings: &[],
            supplies: &[],
        },
    )
    .expect("ordinary declaration publication keeps the Build contract");
    let [boundary] = elaboration.module.boundary_machines.as_slice() else {
        panic!("one selected requirement");
    };
    let published = boundary
        .declaration
        .as_ref()
        .expect("the checked declaration remains present");
    assert_eq!(published.service_declaration, expected);
    assert_ne!(published.service_declaration, [0; 32]);
}

#[test]
fn service_metadata_refuses_lost_provenance_in_a_tree_with_sources() {
    let fixture = Fixture::with_owner_provenance(true, false, false, DependencyScope::Build, false);
    assert!(fixture.typed.symbols.source_files().next().is_some());
    assert_eq!(
        crate::declare::service_declaration_scope(&fixture.typed, fixture.owner),
        Err("boundary declaration lost its retained source provenance")
    );
    assert_eq!(
        crate::declare::service_declaration_scope(&TypedTrees::default(), SymbolHandle::invalid()),
        Ok(DependencyScope::Product)
    );
}
