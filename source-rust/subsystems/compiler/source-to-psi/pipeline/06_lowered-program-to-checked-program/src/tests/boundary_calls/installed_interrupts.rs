//! Read the source enrollment before an installation gains native admission.
use super::super::build_purpose::frontier_with_core;
use crate::source_state::expression::ExpressionNode;
use source_provenance::DependencyScope;

#[test]
fn two_interrupt_installers_keep_their_actual_receiving_registrar_and_callback() {
    macro_rules! core {
        ($file:literal) => {
            (
                $file,
                "",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../../source/library/core/",
                    $file
                )),
            )
        };
    }
    let core = [
        core!("access/access.omg"),
        core!("algebra.omg"),
        core!("assembly.omg"),
        core!("content.omg"),
        core!("conversion.omg"),
        core!("device.omg"),
        core!("extent.omg"),
        core!("interrupt.omg"),
        core!("layout.omg"),
        core!("nat.omg"),
        core!("option.omg"),
        core!("reflect/reflect.omg"),
        core!("representation.omg"),
        core!("run.omg"),
        core!("static_interrupt.omg"),
    ];
    let prepare = |source: &str| {
        let program = frontier_with_core(source, "", DependencyScope::Product, &core);
        crate::preparation::prepare_typed_trees(program, &[], &[]).unwrap()
    };
    let source = include_str!("installed_interrupts.omg");
    let prepared = prepare(source);
    let program = prepared.trees();
    let mut rows = Vec::new();
    for (_, expression) in program.expression_table.iter_backing_expressions() {
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        let Some(request) = &call.installed_interrupt_operation else {
            continue;
        };
        assert!(request.registrar_selection.is_some());
        assert_eq!(program.symbols.name(request.timer_slot.symbol), "TimerSlot");
        let caller = program
            .expression_table
            .static_machine_application(call.authored_machine_application)
            .lexical_owner;
        let registrar = request.prepared.as_deref().unwrap_or_else(|| {
            panic!(
                "installation in {} has no prepared registrar: {:?}",
                program.symbols.name(caller),
                program.deferred_refusal(caller)
            )
        });
        let application = registrar.application;
        assert!(
            program
                .expression_table
                .static_machine_application_is_valid(application)
        );
        let supplied = program
            .expression_table
            .static_machine_application(application);
        assert_eq!(supplied.lexical_owner, caller);
        let selected = program
            .closed_requirement_application(registrar.target)
            .expect("each registrar selects a closed public requirement");
        assert_eq!(supplied.registration_operation, selected.template);
        let receiver = registrar.receiving_scope.scope(program).unwrap();
        assert_eq!(receiver.declaration.symbol, request.registrar_trait);
        let arguments = program
            .expression_table
            .static_machine_application_arguments(application);
        assert_eq!(arguments.len(), 1);
        let callback = program
            .machine_by_own_or_state_symbol(arguments[0].symbol)
            .unwrap();
        assert_eq!(callback.name.as_str(), "tick");
        let Some([satisfaction]) = program.machine_trait_conformances(callback).as_array() else {
            panic!("timer callback must satisfy exactly one requirement")
        };
        let requirement = crate::source_state::machine::resolve_satisfied_declaration(
            program,
            callback,
            satisfaction,
        )
        .unwrap()
        .symbol();
        let (owner, header) = program.trait_machine_signature(requirement).unwrap();
        registrar.timer_slot.validate_identity_encoding().unwrap();
        assert_eq!(
            registrar.timer_slot.requirement_identity,
            program.trait_requirement_callable_identity_text(owner, header)
        );
        rows.push((
            registrar.target,
            receiver.portable_application_identity(program).unwrap(),
            &registrar.timer_slot,
        ));
    }
    assert_eq!(rows.len(), 2);
    assert_ne!(
        rows[0].0, rows[1].0,
        "different Contexts choose different closed registrars"
    );
    assert_ne!(rows[0].1, rows[1].1);
    assert_eq!(
        rows[0].2, rows[1].2,
        "both explicitly cite the same slot declaration"
    );

    // TableLayout is a real named conformance. Only TimerSlot supplies a private
    // callback slot. Replacing one installation must leave the second intact.
    let impostor = source.replace(
        "TimerSlot, InterruptTable, First,",
        "TableLayout, InterruptTable, First,",
    );
    let refused = prepare(&impostor);
    let program = refused.trees();
    let mut states = Vec::new();
    for (_, expression) in program.expression_table.iter_backing_expressions() {
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        let Some(request) = &call.installed_interrupt_operation else {
            continue;
        };
        let owner = program
            .expression_table
            .static_machine_application(call.authored_machine_application)
            .lexical_owner;
        let machine = program.machine_by_own_or_state_symbol(owner).unwrap();
        let name = machine.name.as_str();
        if name == "install_first" {
            assert!(
                program
                    .deferred_refusals_of(machine.symbol)
                    .any(|row| row.detail.contains("PrivateCallbackSlot implementation"))
            );
        }
        states.push((name, request.prepared.is_some()));
    }
    states.sort_unstable();
    assert_eq!(states, [("install_first", false), ("install_second", true)]);
}
