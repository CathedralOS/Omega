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
    owner: SymbolHandle,
    requirement: SymbolHandle,
}

impl Fixture {
    fn new(declares_no_crash_route: bool) -> Self {
        let app_text = "machine entry() { Raw::read(); }";
        let supplier_text = format!(
            "boundary trait Raw {{ machine read() reaches none {}; }}\n\
             machine helper() {{ Raw::read(); }}",
            if declares_no_crash_route {
                "crashes none"
            } else {
                ""
            }
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
                    DependencyScope::Product,
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
                    SymbolNameRef::Source(span(ids[1], &supplier_text, "Raw")),
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
        let states = [app, helper].map(|machine| {
            symbols
                .insert_children(machine, [(SymbolKind::State, SymbolNameRef::Static("run"))])
                .start()
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
            owner,
            requirement,
        }
    }

    fn call_source(&self, caller: SymbolHandle) -> crate::SourceOrigin {
        crate::SourceOrigin::of(self.typed.symbols.symbol_source_span(caller).unwrap())
    }
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
