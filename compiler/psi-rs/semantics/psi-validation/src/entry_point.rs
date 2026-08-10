use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolKind;
use psi_typed_trees::TypedTrees;

pub(crate) fn validate_entry_point(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    // Migration compatibility only. Program entry identity is a target-owned
    // build-slot fact and therefore unavailable to target-neutral Psi
    // validation. While conventionally named entries remain in the corpus,
    // validate their historical exported-callable laws when present; absence
    // is not an error here. Omega's build-slot validation and backend planning
    // reject an installable build with neither an explicit binding nor a
    // migration entry.
    let entry = [
        ("Main::main", "main"),
        ("Main::run", "run"),
        ("main", "entry"),
    ]
    .iter()
    .find_map(|(machine_name, state_name)| find_entry_point(program, machine_name, state_name));

    let Some((machine, state)) = entry else {
        return;
    };

    // THE EXPORTED-CALLABLE LAWS (settled 2026-07-04, boundary machines).
    // TRANSITIONAL SCOPE: the laws bind the CANONICAL entry (`Main::run`) and
    // any boundary-marked entry; legacy param'd `Main::main` test scaffolding
    // is exempt until the main-retirement sweep migrates it.
    let canonical = machine.name.as_str() == "Main::run";
    let is_boundary_declaration = machine.supply_mode.is_boundary_declaration();
    if !canonical && !is_boundary_declaration {
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
    if imposes_structure && !is_boundary_declaration {
        diagnostics.push(Diagnostic::error(format!(
            "the entry `{}` imposes structure on the platform's arrival bytes; declare it `boundary machine` (the exported-callable surface owns that claim)",
            machine.name.as_str()
        )));
    }

    // Arrival fit is a property of the evaluated boundary call plan, not a
    // global register-count rule. Parameters beyond the register bank are
    // legal when the selected plan places them in its incoming stack area.
}

fn find_entry_point<'trees>(
    program: &'trees TypedTrees,
    machine_name: &str,
    state_name: &str,
) -> Option<(
    &'trees psi_typed_trees::machine::Machine,
    &'trees psi_typed_trees::state::State,
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
    let state_symbol = program.symbols.find_child_by_name_and_kind(
        machine.symbol,
        state_name,
        SymbolKind::State,
    )?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    Some((machine, state))
}
