use super::{MachineParameterSignatureQueries, SymbolHandle, TypedTrees};
use crate::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
use crate::name::Identifier;
use crate::signature::StateSignature;
use crate::typed_trees::MachineSpecialization;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

fn symbol(symbols: &mut SymbolTableBuilder, kind: SymbolKind, name: &'static str) -> SymbolHandle {
    symbols.insert_root(kind, SymbolNameRef::Static(name))
}

fn children(
    symbols: &mut SymbolTableBuilder,
    owner: SymbolHandle,
    members: &[(SymbolKind, &'static str)],
) -> Vec<SymbolHandle> {
    symbols
        .insert_children(
            owner,
            members
                .iter()
                .map(|(kind, name)| (*kind, SymbolNameRef::Static(*name))),
        )
        .handles()
        .collect()
}

fn parameter(symbol: SymbolHandle, name: &'static str, blocking: Option<bool>) -> TypeParameter {
    TypeParameter {
        symbol,
        name: Identifier::from(name),
        kind: blocking.map_or(TypeParameterKind::Type, |blocks| {
            TypeParameterKind::Machine {
                contract: MachineParameterContract::Structural(StateSignature {
                    symbol,
                    blocks,
                    ..StateSignature::default()
                }),
            }
        }),
        ..TypeParameter::default()
    }
}

fn machine(program: &mut TypedTrees, symbol: SymbolHandle, parameters: Vec<TypeParameter>) {
    let type_parameters = program.data_type_parameters.insert_many(parameters);
    program.push_machine(crate::machine::Machine {
        symbol,
        type_parameters,
        ..crate::machine::Machine::default()
    });
}

fn same(
    program: &TypedTrees,
    queries: &mut MachineParameterSignatureQueries<'_>,
    symbol: SymbolHandle,
) {
    match (
        program.machine_parameter_signature(symbol),
        queries.get(symbol),
    ) {
        (Some((old_owner, old_signature)), Some((owner, signature))) => {
            assert!(std::ptr::eq(old_owner, owner));
            assert!(std::ptr::eq(old_signature, signature));
        }
        (None, None) => {}
        _ => panic!("batched lookup changed the exact owner or signature for {symbol:?}"),
    }
}

#[test]
fn batch_preserves_first_owner_generations_and_negative_questions() {
    let mut symbols = SymbolTableBuilder::default();
    let first = symbol(&mut symbols, SymbolKind::Machine, "first");
    let second = symbol(&mut symbols, SymbolKind::Machine, "second");
    let first_members = children(
        &mut symbols,
        first,
        &[
            (SymbolKind::TypeParameter, "T"),
            (SymbolKind::TypeParameter, "Shared"),
        ],
    );
    let (ty, shared) = (first_members[0], first_members[1]);
    let selected = children(
        &mut symbols,
        second,
        &[(SymbolKind::TypeParameter, "Selected")],
    )[0];
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..TypedTrees::default()
    };
    machine(
        &mut program,
        first,
        vec![
            parameter(ty, "T", None),
            parameter(shared, "Shared", Some(false)),
        ],
    );
    machine(
        &mut program,
        second,
        vec![
            parameter(shared, "Shared", Some(true)),
            parameter(selected, "Selected", Some(true)),
        ],
    );
    let mut queries = program.machine_parameter_signature_queries();
    same(&program, &mut queries, ty);
    same(
        &program,
        &mut queries,
        SymbolHandle::from_parts(selected.arena_index(), selected.generation() + 1),
    );
    assert!(queries.scope_parents.is_empty());
    for _ in 0..8 {
        same(&program, &mut queries, shared);
        same(&program, &mut queries, selected);
    }
    assert_eq!(queries.get(shared).unwrap().0.symbol, first);
    assert!(!queries.get(shared).unwrap().1.blocks);
    assert_eq!(queries.get(selected).unwrap().0.symbol, second);
    assert!(queries.get(selected).unwrap().1.blocks);
    // A repeated miss in the first machine keeps one exact invalid-parent
    // answer, rather than walking its specialization/capture roster again.
    assert_eq!(queries.scope_parents.len(), 1);
    drop(queries);
    let mut projection = program.clone();
    projection.roots.machines = arena::HandleSpan::empty();
    let mut projected = projection.machine_parameter_signature_queries();
    same(&projection, &mut projected, selected);
    assert!(projected.get(selected).is_none());
}

#[test]
fn batch_keeps_conformance_shadowing_and_first_visible_owner() {
    let mut symbols = SymbolTableBuilder::default();
    let map = symbol(&mut symbols, SymbolKind::Conformance, "Map");
    let members = children(
        &mut symbols,
        map,
        &[
            (SymbolKind::Machine, "hidden"),
            (SymbolKind::Machine, "visible"),
            (SymbolKind::TypeParameter, "Callback"),
        ],
    );
    let (hidden, visible, callback) = (members[0], members[1], members[2]);
    let shadow = children(
        &mut symbols,
        hidden,
        &[(SymbolKind::TypeParameter, "Callback")],
    )[0];
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..TypedTrees::default()
    };
    let type_parameters =
        program
            .data_type_parameters
            .insert_many([parameter(callback, "Callback", Some(true))]);
    program.push_conformance(crate::trait_definition::Conformance {
        symbol: map,
        type_parameters,
        ..crate::trait_definition::Conformance::default()
    });
    machine(
        &mut program,
        hidden,
        vec![parameter(shadow, "Callback", None)],
    );
    machine(&mut program, visible, Vec::new());
    let mut queries = program.machine_parameter_signature_queries();
    for _ in 0..8 {
        same(&program, &mut queries, callback);
    }
    let (owner, signature) = queries.get(callback).unwrap();
    assert_eq!(owner.symbol, visible);
    assert!(signature.blocks);
    assert_eq!(queries.scope_parents.len(), 2);
}

#[test]
fn batches_do_not_carry_answers_across_changed_specialization_ancestry() {
    let mut symbols = SymbolTableBuilder::default();
    let map = symbol(&mut symbols, SymbolKind::Conformance, "Map");
    let members = children(
        &mut symbols,
        map,
        &[
            (SymbolKind::Machine, "template"),
            (SymbolKind::TypeParameter, "Callback"),
        ],
    );
    let (template, callback) = (members[0], members[1]);
    let instance = symbol(&mut symbols, SymbolKind::Machine, "instance");
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..TypedTrees::default()
    };
    let type_parameters =
        program
            .data_type_parameters
            .insert_many([parameter(callback, "Callback", Some(true))]);
    program.push_conformance(crate::trait_definition::Conformance {
        symbol: map,
        type_parameters,
        ..crate::trait_definition::Conformance::default()
    });
    machine(&mut program, instance, Vec::new());
    program.machine_specializations.push(MachineSpecialization {
        instance,
        template,
        ..MachineSpecialization::default()
    });
    {
        let mut queries = program.machine_parameter_signature_queries();
        same(&program, &mut queries, callback);
        assert!(queries.get(callback).is_some());
    }
    // Same-length mutation creates a cycle. An earlier successful batch must
    // not bless it, and a later repaired batch must not inherit its refusal.
    program.machine_specializations[0].template = instance;
    {
        let mut queries = program.machine_parameter_signature_queries();
        same(&program, &mut queries, callback);
        assert!(queries.get(callback).is_none());
    }
    program.machine_specializations[0].template = template;
    {
        let mut queries = program.machine_parameter_signature_queries();
        same(&program, &mut queries, callback);
        assert!(queries.get(callback).is_some());
    }
    let duplicate = program.machine_specializations[0].clone();
    program.machine_specializations.push(duplicate);
    let mut queries = program.machine_parameter_signature_queries();
    same(&program, &mut queries, callback);
    assert!(queries.get(callback).is_none());
}

#[test]
fn first_viewless_contract_does_not_hide_a_later_retained_view() {
    let mut symbols = SymbolTableBuilder::default();
    let first = symbol(&mut symbols, SymbolKind::Machine, "first");
    let second = symbol(&mut symbols, SymbolKind::Machine, "second");
    let callback = children(
        &mut symbols,
        first,
        &[(SymbolKind::TypeParameter, "Callback")],
    )[0];
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..TypedTrees::default()
    };
    let mut identity_only = parameter(callback, "Callback", Some(false));
    identity_only.kind = TypeParameterKind::Machine {
        contract: MachineParameterContract::RequirementIdentity,
    };
    machine(&mut program, first, vec![identity_only]);
    machine(
        &mut program,
        second,
        vec![parameter(callback, "Callback", Some(true))],
    );
    assert!(
        program
            .retained_static_machine_contract(callback)
            .unwrap()
            .signature()
            .blocks
    );
    {
        let mut queries = program.machine_parameter_signature_queries();
        same(&program, &mut queries, callback);
        assert_eq!(queries.get(callback).unwrap().0.symbol, second);
    }
    let later = program.machines()[1].type_parameters.start();
    program.data_type_parameters.get_mut(later).kind = TypeParameterKind::Machine {
        contract: MachineParameterContract::RequirementIdentity,
    };
    assert!(program.retained_static_machine_contract(callback).is_none());
    let mut queries = program.machine_parameter_signature_queries();
    same(&program, &mut queries, callback);
    assert!(queries.get(callback).is_none());
}

#[test]
fn logical_callable_derivation_uses_any_retained_view_and_refreshes_after_mutation() {
    let mut symbols = SymbolTableBuilder::default();
    let map = symbol(&mut symbols, SymbolKind::Conformance, "Map");
    let original = children(
        &mut symbols,
        map,
        &[(SymbolKind::TypeParameter, "Callback")],
    )[0];
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..TypedTrees::default()
    };
    let derived =
        program
            .symbols
            .insert_generated_root_from(original, SymbolKind::Machine, "derived");
    let type_parameters =
        program
            .data_type_parameters
            .insert_many([parameter(original, "Callback", Some(true))]);
    program.push_conformance(crate::trait_definition::Conformance {
        symbol: map,
        type_parameters,
        ..Default::default()
    });
    let mut identity_only = parameter(derived, "derived", Some(false));
    identity_only.kind = TypeParameterKind::Machine {
        contract: MachineParameterContract::RequirementIdentity,
    };
    program.data_type_parameters.insert(identity_only);
    let later = program
        .data_type_parameters
        .insert(parameter(derived, "derived", Some(false)));
    machine(&mut program, derived, Vec::new());
    assert_eq!(program.lifetime_declaration_owner(derived), original);
    {
        let mut queries = program.machine_parameter_signature_queries();
        same(&program, &mut queries, original);
        assert!(queries.get(original).unwrap().1.blocks);
    }
    program.data_type_parameters.get_mut(later).kind = TypeParameterKind::Machine {
        contract: MachineParameterContract::RequirementIdentity,
    };
    assert_eq!(program.lifetime_declaration_owner(derived), derived);
    let mut queries = program.machine_parameter_signature_queries();
    same(&program, &mut queries, original);
    assert!(queries.get(original).is_none());
}
