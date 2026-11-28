use super::*;
use crate::source_state::machine::Machine;
use crate::source_state::references::{MachineSidecarIndex, TypedReferences, visit_program};
use crate::source_symbols::SymbolTableBuilder;
use symbols::{SymbolKind, SymbolNameRef};

#[test]
fn invocation_owner_walk_preserves_views_order_and_unread_headers() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<MachineSidecarIndex>();
    let mut symbols = SymbolTableBuilder::default();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = symbols.insert_children(
        root,
        [
            (SymbolKind::Machine, SymbolNameRef::Static("inside")),
            (SymbolKind::Machine, SymbolNameRef::Static("outside")),
            (SymbolKind::Machine, SymbolNameRef::Static("requirement")),
            (SymbolKind::Data, SymbolNameRef::Static("First")),
            (SymbolKind::Data, SymbolNameRef::Static("Second")),
            (SymbolKind::Data, SymbolNameRef::Static("Outside")),
        ],
    );
    let inside = declarations.handle_at(0);
    let outside = declarations.handle_at(1);
    let requirement = declarations.handle_at(2);
    let stale = SymbolHandle::from_parts(inside.arena_index(), inside.generation() + 1);
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    for symbol in [inside, outside] {
        program.push_machine(Machine {
            symbol,
            ..Default::default()
        });
    }
    let types = [3, 4, 5].map(|ordinal| {
        program
            .type_reference_table
            .insert(types::TypeReferenceNode::Named {
                symbol: declarations.handle_at(ordinal),
                name: crate::source_state::name::Identifier::generated("result"),
            })
    });
    let application = program.expression_table.retain_static_machine_application(
        Default::default(),
        Default::default(),
        &[],
    );
    let invocation = |lexical_owner| AppliedCallableOwner::RequirementInvocation {
        declaration: requirement,
        application,
        lexical_owner,
        source: Default::default(),
        bindings: Default::default(),
        lifetimes: Default::default(),
        arguments: Default::default(),
    };
    let realization = |lexical_owner| AppliedCallableOwner::Realization {
        declaration: requirement,
        application: Default::default(),
        lexical_owner,
    };
    for (owner, return_type) in [
        (invocation(inside), types[0]),
        (realization(outside), types[2]),
        (realization(inside), types[1]),
        (invocation(stale), types[2]),
        (invocation(inside), types[0]),
        (AppliedCallableOwner::Absent, types[2]),
    ] {
        program
            .applied_callable_signatures
            .insert(AppliedCallableSignature {
                owner,
                signature: signature::StateSignature {
                    symbol: requirement,
                    return_type,
                    ..Default::default()
                },
            });
    }
    let index = MachineSidecarIndex::default();
    for (owner, expected) in [
        (inside, vec![types[0], types[1], types[0]]),
        (outside, vec![types[2]]),
        (stale, vec![types[2]]),
        (SymbolHandle::invalid(), vec![]),
    ] {
        let direct = program
            .requirement_invocation_signatures(owner)
            .collect::<Vec<_>>();
        let indexed = index
            .requirement_invocation_signatures(&program, owner)
            .collect::<Vec<_>>();
        assert_eq!(
            direct
                .iter()
                .map(|signature| signature.return_type)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(indexed.len(), direct.len());
        assert!(
            indexed
                .iter()
                .zip(direct)
                .all(|(left, right)| std::ptr::eq(*left, right))
        );
    }

    struct Seen {
        outside: SymbolHandle,
        derived: bool,
        scopes: Vec<SymbolHandle>,
        views: Vec<(SymbolHandle, types::TypeReferenceHandle)>,
        dependencies: Vec<SymbolHandle>,
    }
    impl Extend<SymbolHandle> for Seen {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
            self.dependencies.extend(symbols);
        }
    }
    impl TypedReferences for Seen {
        fn declaration(&mut self, symbol: SymbolHandle) -> bool {
            symbol == self.outside
        }
        fn machine_body(&mut self, _: &Machine) -> bool {
            false
        }
        fn derived_declaration_references(&self) -> bool {
            self.derived
        }
        fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
            self.scopes.push(owner);
        }
        fn leave_lexical_scope(&mut self) {
            self.scopes.pop();
        }
        fn callable_signature(&mut self, signature: &signature::StateSignature) {
            self.views
                .push((*self.scopes.last().unwrap(), signature.return_type));
        }
    }
    for derived in [true, false] {
        let mut seen = Seen {
            outside,
            derived,
            scopes: Vec::new(),
            views: Vec::new(),
            dependencies: Vec::new(),
        };
        visit_program(&program, &mut seen);
        if derived {
            assert_eq!(seen.views, [(outside, types[2])]);
            assert!(
                seen.dependencies.contains(&declarations.handle_at(5)),
                "unread outside headers keep their dependencies"
            );
        } else {
            assert!(seen.views.is_empty());
            assert!(!seen.dependencies.contains(&declarations.handle_at(5)));
        }
        assert!(seen.scopes.is_empty());
    }
}
