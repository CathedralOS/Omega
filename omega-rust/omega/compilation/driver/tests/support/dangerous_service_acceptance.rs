//! Which dangerous package-owned services a checked fixture requires.
//!
//! Acceptance is this repository's test policy: every fixture whose program
//! requires a selected dangerous service is accepted for the canary
//! run. The requirement is read from the preliminary checked graph — the
//! provider plans the program selected and the boundary calls it resolved
//! against them — never from the fixture's spelling. A service imported under
//! a package alias or the bundled `omega::language::std` path, or mentioned
//! only in a comment, therefore cannot change what is accepted. This is not
//! evidence that an audit occurred and is not production accepted-lock
//! recovery; every admitted row is still derived from and replayed against
//! the exact preliminary checked graph by the binding constructors.

use compiler::CheckedCompilation;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;

/// The `Console` operations a program resolves against the selected package's
/// provider. Each flag admits the terminal-authority class
/// its methods exercise; an operation the program never resolves is not
/// permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConsoleUse {
    /// `exit_process`.
    pub exit: bool,
    /// `write`, `write_byte` or `write_line`.
    pub output: bool,
    /// `read_byte` or `read_line`.
    pub input: bool,
}

impl ConsoleUse {
    pub const fn is_empty(self) -> bool {
        !(self.exit || self.output || self.input)
    }
}

/// The dangerous package-owned services one checked program requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequiredDangerousServices {
    /// The selected entry's receiver demands the standard library's
    /// `FilesystemHost` boundary through a `Binding<FilesystemHost>` field,
    /// directly or inside a nested record such as `Filesystem`.
    pub filesystem: bool,
    /// The selected entry's receiver demands the standard library's
    /// `TimeHost` boundary through a `Binding<TimeHost>` field, directly or
    /// inside a nested record such as `Time`.
    pub time: bool,
    /// The operations the program resolves against this package's
    /// selected `Console` provider; `None` when it selected no such provider
    /// or resolves no operation against it.
    pub console: Option<ConsoleUse>,
    /// The program resolves `exit_process` against this package's
    /// selected `ProcessExit` provider.
    pub process_exit: bool,
}

/// Project the dangerous services `checked` requires from the package
/// bound under `owner`.
///
/// `Console` counts through a selected provider plan whose schema the
/// nominated package owns, and `ProcessExit`, which is toolchain-owned,
/// through the selected provider type instead, exactly as the binding
/// constructors attribute it.
///
/// `FilesystemHost` and `TimeHost` are canonical toolchain-settled slots and
/// cannot count through a selected plan: no package may author a conformance
/// for them, and provider settlement mints their plan only after the build
/// has accepted the slot's semantic binding (`provider_settlement` in
/// build-evaluation). Keying their acceptance on a selected plan therefore
/// could never fire, and every entry holding one of these carriers stopped
/// at establishment's missing-Fused-provider rejection even on the targets
/// whose settlement table does realize the slot. Their demand is instead the
/// selected entry's own `Binding<R>` field requirements — the same surface
/// consumer package review nominates these bindings from, walked by the same
/// nested-record traversal establishment uses — which is why the preliminary
/// compile tolerates the still-unsettled fields.
///
/// Console and ProcessExit operations are the trait signatures the program's
/// resolved calls select — a `Binding<R>` receiver resolves its call
/// target to `R`'s signature whatever the receiver's shape — so a fixture that imports a service and never calls
/// it accepts nothing.
pub fn required_dangerous_services(
    checked: &CheckedCompilation,
    owner: PackageKeyIdentity,
) -> RequiredDangerousServices {
    let program = &checked.typed;
    let owned_by_package =
        |symbol: SymbolHandle| program.symbols.symbol_package_identity(symbol) == Some(owner);
    let mut services = RequiredDangerousServices::default();
    let mut console = ConsoleUse::default();
    for (plan, provenance) in checked
        .execution()
        .selected_provider_plan_facts
        .plans()
        .iter()
        .zip(checked.execution().selected_provider_provenance.as_slice())
    {
        let schema = provenance.provider.schema.symbol();
        match plan.schema.trait_name.as_str() {
            "Console" if owned_by_package(schema) => {
                for method in resolved_boundary_call_names(checked, schema) {
                    match method {
                        "exit_process" => console.exit = true,
                        "write" | "write_byte" | "write_line" => console.output = true,
                        "read_byte" | "read_line" => console.input = true,
                        _ => {}
                    }
                }
            }
            "ProcessExit"
                if provenance
                    .provider
                    .provider_type
                    .is_some_and(owned_by_package) =>
            {
                services.process_exit |=
                    resolved_boundary_call_names(checked, schema).contains(&"exit_process");
            }
            _ => {}
        }
    }
    for requirement in checked.execution().program_entry_service_requirements() {
        let Some(definition) = program
            .traits()
            .iter()
            .find(|definition| definition.is_boundary && definition.symbol == requirement)
        else {
            continue;
        };
        if !owned_by_package(definition.symbol) {
            continue;
        }
        match program
            .symbols
            .display_path(definition.symbol, "::")
            .as_str()
        {
            "FilesystemHost" => services.filesystem = true,
            "TimeHost" => services.time = true,
            _ => {}
        }
    }
    if !console.is_empty() {
        services.console = Some(console);
    }
    services
}

/// The names of the boundary trait `requirement`'s signatures selected by
/// resolved calls. The checked occurrence roster covers statements,
/// expressions and transition targets through the same exact target identity.
fn resolved_boundary_call_names(
    checked: &CheckedCompilation,
    requirement: SymbolHandle,
) -> Vec<&str> {
    let program = &checked.typed;
    let Some(definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == requirement)
    else {
        return Vec::new();
    };
    let signatures = program.trait_machine_signatures(definition);
    checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .filter_map(|(_, call)| {
            signatures
                .iter()
                .find(|signature| signature.symbol == call.target_symbol)
                .map(|signature| signature.name.as_str())
        })
        .collect()
}
