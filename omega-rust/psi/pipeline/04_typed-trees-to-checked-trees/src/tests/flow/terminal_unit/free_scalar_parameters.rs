//! Free Unit signatures retain scalar parameters without a fabricated attachment.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

const SOURCE: &str = r#"
    boundary trait Host {
        machine send(first: u16, second: u16) reaches Host;
    }
    data Sink {}
    machine Sink::finish(first: u16, second: u16) reaches Host {
        Host::send(first, second);
    }
    machine consume(first: u16, second: u16) reaches Host {
        Sink::finish(second, first);
    }
    machine empty() {}
"#;

#[test]
fn free_unit_scalar_admission_rejects_parameter_shape_drift() {
    let original = checked(SOURCE);
    let symbol = machine_named(&original, "consume");
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let parameter_symbol = original.state_parameters(state)[0].symbol;
    for mutation in 0..4 {
        let mut changed = original.clone();
        let handle = changed
            .typed
            .state_parameters
            .iter()
            .find_map(|(handle, parameter)| {
                (parameter.symbol == parameter_symbol).then_some(handle)
            })
            .unwrap();
        let parameter = changed.typed.state_parameters.get_mut(handle);
        match mutation {
            0 => parameter.is_self = true,
            1 => parameter.is_const = true,
            2 => parameter.is_mutable = true,
            _ => parameter.type_reference = arena::Handle::invalid(),
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(symbol).is_none(),
            "parameter mutation {mutation} rejects"
        );
    }
}
