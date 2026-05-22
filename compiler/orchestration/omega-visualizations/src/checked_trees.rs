use omega_checked_trees::Program;
use omega_core::symbols::SymbolHandle;
use omega_effects::EffectSet;

pub fn checked_trees_html(program: &Program) -> String {
    crate::phase_diagram::text_report_html("checked_trees", &checked_effects_report(program))
}

fn checked_effects_report(program: &Program) -> String {
    let mut report = String::new();
    report.push_str("Checked Facts\n");
    report.push_str("=============\n\n");
    report.push_str("Effects\n");
    report.push_str("-------\n");
    report.push_str("Effects are stored as propagated bitsets on checked-tree facts.\n");
    report.push_str("direct = declared or boundary effects at that node.\n");
    report.push_str("reached = direct effects plus effects reached through calls.\n\n");

    for machine_effects in program.facts.effects.machines() {
        let machine_name = machine_name(program, machine_effects.symbol);
        report.push_str("machine ");
        report.push_str(&machine_name);
        report.push('\n');
        report.push_str("  symbol: ");
        report.push_str(&symbol_label(machine_effects.symbol));
        report.push('\n');
        report.push_str("  direct:  ");
        report.push_str(&format_effect_set(machine_effects.direct));
        report.push('\n');
        report.push_str("  reached: ");
        report.push_str(&format_effect_set(machine_effects.transitive));
        report.push('\n');

        for state_effects in program
            .facts
            .effects
            .states
            .span_or_empty(machine_effects.states)
        {
            let current_state_name = state_name(program, state_effects.symbol);
            report.push_str("  state ");
            report.push_str(&current_state_name);
            report.push('\n');
            report.push_str("    symbol: ");
            report.push_str(&symbol_label(state_effects.symbol));
            report.push('\n');
            report.push_str("    direct:  ");
            report.push_str(&format_effect_set(state_effects.direct));
            report.push('\n');
            report.push_str("    reached: ");
            report.push_str(&format_effect_set(state_effects.transitive));
            report.push('\n');

            for call_effects in program
                .facts
                .effects
                .calls
                .span_or_empty(state_effects.calls)
            {
                report.push_str("    call ");
                report.push_str(&call_effects.statement_index.to_string());
                report.push('.');
                report.push_str(&call_effects.call_ordinal.to_string());
                report.push_str(" -> ");
                report.push_str(&state_name(program, call_effects.target_state_symbol));
                report.push('\n');
                report.push_str("      direct:  ");
                report.push_str(&format_effect_set(call_effects.direct));
                report.push('\n');
                report.push_str("      reached: ");
                report.push_str(&format_effect_set(call_effects.transitive));
                report.push('\n');
            }
        }

        report.push('\n');
    }

    report
}

fn machine_name(program: &Program, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .map(|machine| machine.name.as_str().to_owned())
        .unwrap_or_else(|| symbol_label(symbol))
}

fn state_name(program: &Program, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == symbol)
        .map(|state| state.name.as_str().to_owned())
        .unwrap_or_else(|| symbol_label(symbol))
}

fn format_effect_set(effects: EffectSet) -> String {
    if effects.is_empty() {
        return "<none> [0x0000000000000000]".to_owned();
    }

    format!(
        "{} [0x{:016x}]",
        effects.names().collect::<Vec<_>>().join(", "),
        effects.bits()
    )
}

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
    } else {
        "invalid".to_owned()
    }
}
