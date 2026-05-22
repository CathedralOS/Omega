use omega_checked_trees::Program;
use omega_core::symbols::SymbolHandle;
use omega_effects::EffectSet;

pub fn checked_trees_html(program: &Program) -> String {
    crate::phase_diagram::text_report_html("checked_trees", &checked_effects_report(program))
}

pub fn capability_manifest_html(program: &Program) -> String {
    crate::phase_diagram::text_report_html(
        "capability_manifest",
        &capability_manifest_text(program),
    )
}

pub fn capability_manifest_json(program: &Program) -> String {
    let manifest = entry_capability_manifest(program);
    let effect_names = manifest.effects.names().collect::<Vec<_>>();

    let mut json = String::new();
    json.push_str("{\n");
    json.push_str("  \"entry_machine\": ");
    push_json_string(&mut json, &manifest.entry_machine);
    json.push_str(",\n  \"entry_state\": ");
    push_json_string(&mut json, &manifest.entry_state);
    json.push_str(",\n  \"effect_bits\": \"0x");
    json.push_str(&format!("{:016x}", manifest.effects.bits()));
    json.push_str("\",\n  \"effects\": [");
    for (index, effect) in effect_names.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(&mut json, effect);
    }
    json.push_str("]\n}\n");
    json
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

fn capability_manifest_text(program: &Program) -> String {
    let manifest = entry_capability_manifest(program);
    let mut report = String::new();

    report.push_str("Executable Capability Manifest\n");
    report.push_str("==============================\n\n");
    report.push_str("entry machine: ");
    report.push_str(&manifest.entry_machine);
    report.push('\n');
    report.push_str("entry state:   ");
    report.push_str(&manifest.entry_state);
    report.push('\n');
    report.push_str("effects:       ");
    report.push_str(&format_effect_set(manifest.effects));
    report.push('\n');

    report
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryCapabilityManifest {
    entry_machine: String,
    entry_state: String,
    effects: EffectSet,
}

fn entry_capability_manifest(program: &Program) -> EntryCapabilityManifest {
    let Some((machine_symbol, machine_name, state_name)) = entry_machine(program) else {
        return EntryCapabilityManifest {
            entry_machine: "<missing>".to_owned(),
            entry_state: "<missing>".to_owned(),
            effects: EffectSet::empty(),
        };
    };

    let effects = program
        .facts
        .effects
        .machines()
        .iter()
        .find(|effects| effects.symbol == machine_symbol)
        .map(|effects| effects.transitive)
        .unwrap_or_else(EffectSet::empty);

    EntryCapabilityManifest {
        entry_machine: machine_name,
        entry_state: state_name,
        effects,
    }
}

fn entry_machine(program: &Program) -> Option<(SymbolHandle, String, String)> {
    entry_machine_with_state(program, "Main::main", "main")
        .or_else(|| entry_machine_with_state(program, "main", "entry"))
}

fn entry_machine_with_state(
    program: &Program,
    machine_name: &str,
    state_name: &str,
) -> Option<(SymbolHandle, String, String)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)?;
    program
        .machine_states(machine)
        .iter()
        .any(|state| state.name.as_str() == state_name)
        .then(|| {
            (
                machine.symbol,
                machine.name.as_str().to_owned(),
                state_name.to_owned(),
            )
        })
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

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch.is_control() => output.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => output.push(ch),
        }
    }
    output.push('"');
}
