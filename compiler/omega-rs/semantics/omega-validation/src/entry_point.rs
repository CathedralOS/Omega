use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolKind;
use omega_typed_trees::TypedTrees;

pub(crate) fn validate_entry_point(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    // Canonical: `machine Main::run(&self, args: &[u8])` -- Main's members are
    // the program's statics, args is the platform handoff as raw bytes.
    // `Main::main` is the accepted legacy spelling during migration.
    // PRECEDENCE MIRRORS resolve_backend_entry_point EXACTLY: `Main::main`
    // while it exists (7 corpus programs have `Main::run` HELPERS -- applying
    // entry laws to a helper broke them once already), then the canonical
    // `Main::run`, then the legacy `main`.
    let entry = [
        ("Main::main", "main"),
        ("Main::run", "run"),
        ("main", "entry"),
    ]
    .iter()
    .find_map(|(machine_name, state_name)| find_entry_point(program, machine_name, state_name));

    let Some((machine, state)) = entry else {
        diagnostics.push(Diagnostic::error(
            "missing runtime entry point `Main::run`",
        ));
        return;
    };

    // THE EXPORTED-CALLABLE LAWS (settled 2026-07-04, boundary machines).
    // TRANSITIONAL SCOPE: the laws bind the CANONICAL entry (`Main::run`) and
    // any boundary-marked entry; legacy param'd `Main::main` test scaffolding
    // is exempt until the main-retirement sweep migrates it.
    let canonical = machine.name.as_str() == "Main::run";
    if !canonical && !machine.boundary {
        return;
    }
    let parameters: Vec<_> = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.name.as_str() != "self")
        .collect();

    // (1) THE MARKING LAW: an entry that imposes STRUCTURE on the platform's
    // arrival bytes (any parameter beyond raw `&[u8]`) is a boundary -- the
    // shape claim must be declared where lies can happen. Raw bytes carry no
    // claim, so a plain `machine` taking `&[u8]` stays legal.
    let imposes_structure = parameters
        .iter()
        .any(|parameter| !program.is_borrowed_byte_slice(parameter.type_reference));
    if imposes_structure && !machine.boundary {
        diagnostics.push(Diagnostic::error(format!(
            "the entry `{}` imposes structure on the platform's arrival bytes; declare it `boundary machine` (the exported-callable surface owns that claim)",
            machine.name.as_str()
        )));
    }

    // (2) THE ARRIVAL-FIT LAW (v1): the platform hands the entry at most four
    // register arguments (MS-x64). A fifth declared parameter would read
    // garbage -- a clear error, never a silently-unpopulated slot.
    if parameters.len() > 4 {
        diagnostics.push(Diagnostic::error(format!(
            "the entry `{}` declares {} parameters; the platform's arrival delivers at most 4 register arguments",
            machine.name.as_str(),
            parameters.len()
        )));
    }
}

fn find_entry_point<'trees>(
    program: &'trees TypedTrees,
    machine_name: &str,
    state_name: &str,
) -> Option<(
    &'trees omega_typed_trees::machine::Machine,
    &'trees omega_typed_trees::state::State,
)> {
    let machine_symbol = program.symbols.find_child_by_name_and_kind(
        program.symbols.root(),
        machine_name,
        SymbolKind::Machine,
    )?;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let state_symbol =
        program
            .symbols
            .find_child_by_name_and_kind(machine.symbol, state_name, SymbolKind::State)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    Some((machine, state))
}

