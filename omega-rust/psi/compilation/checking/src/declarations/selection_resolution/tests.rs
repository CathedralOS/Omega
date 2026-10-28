use super::*;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelections,
};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{MachineSpecialization, TypedTrees, machine::Machine, state::State};

fn fixture() -> (TypedTrees, [(SymbolHandle, [SymbolHandle; 2]); 3]) {
    let mut symbols = SymbolTableBuilder::default();
    let declarations = ["original", "instance", "unrelated"].map(|name| {
        let machine = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static(name));
        let states = symbols.insert_children(
            machine,
            [
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (SymbolKind::State, SymbolNameRef::Static("nested")),
            ],
        );
        (machine, [states.handle_at(0), states.handle_at(1)])
    });
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let machines = declarations.map(|(symbol, states)| {
        let states = program
            .machine_states
            .insert_many(states.map(|symbol| State {
                symbol,
                ..Default::default()
            }));
        Machine {
            symbol,
            states,
            ..Default::default()
        }
    });
    program.roots.machines = program.tables.machines.insert_many(machines);
    program.machine_specializations.push(MachineSpecialization {
        template: declarations[0].0,
        instance: declarations[1].0,
        ..Default::default()
    });
    (program, declarations)
}

#[test]
fn copied_call_selections_rejoin_only_their_authored_declaration() {
    let (program, [original, instance, unrelated]) = fixture();
    let mut ledger = AuthoredDeclarationSelections::default();
    let occurrence = ledger
        .record_late_bound(
            Default::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
        )
        .expect("test occurrence");
    let resolution = |target| AuthoredSelectionResolution {
        occurrence,
        binding: AuthoredDeclarationSelectionLateBinding::CheckedCall,
        target: AuthoredSelectionResolutionTarget::Declaration(target),
    };
    let mut resolutions = Vec::new();
    push_consistent_resolution(&program, &mut resolutions, resolution(original.1[0])).unwrap();
    push_consistent_resolution(&program, &mut resolutions, resolution(instance.1[0])).unwrap();
    assert_eq!(resolutions, vec![resolution(original.1[0])]);
    assert!(
        push_consistent_resolution(&program, &mut resolutions, resolution(unrelated.1[0])).is_err()
    );
    // A different state of the same template is still a different declaration.
    assert!(
        push_consistent_resolution(&program, &mut resolutions, resolution(instance.1[1])).is_err()
    );
    assert_eq!(authored_callable(&program, instance.0).unwrap(), original.0);
    assert_eq!(
        authored_callable(&program, instance.1[1]).unwrap(),
        original.1[1]
    );
}

#[test]
fn cyclic_specialization_provenance_cannot_finalize_a_source_selection() {
    let (mut program, [original, instance, _]) = fixture();
    program.machine_specializations.push(MachineSpecialization {
        template: instance.0,
        instance: original.0,
        ..Default::default()
    });
    assert!(authored_callable(&program, instance.1[0]).is_err());
}
