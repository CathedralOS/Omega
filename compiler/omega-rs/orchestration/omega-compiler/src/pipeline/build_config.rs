//! THE BUILD CONFIG (build_and_package_model.md + its 2026-07-04 addendum):
//! image facts come from `build.omg`'s augmenting machine, never from an
//! invented config grammar. When the program (build.omg is ordinary source,
//! auto-included next to main.omg) defines the conventionally-named free
//! machine `build(b: &mut Build)`, the compiler evaluates it at build time
//! (purity-gated, the L0 engine) with a ZII `Build` and reads the augmented
//! value back:
//!
//! ```omega
//! data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
//! data Build { subsystem: Subsystem; freestanding: bool; }
//! machine build(b: &mut Build) {
//!     b.subsystem = Subsystem::EfiApplication;
//!     b.freestanding = true;
//! }
//! ```
//!
//! - `subsystem` is loader METADATA (a PE header u16 the compiler copies; it
//!   does not select the emitter). The ZII zero case is `Console` -- the
//!   correct default falls out of the type. `Unspecified(value)` is the
//!   escape hatch: any loader value a platform invents, with no compiler
//!   release.
//! - `freestanding` ("trust no host packages" -> the empty host-ABI plan) is
//!   stated as itself -- previously fused into the `efi_application` name.
//! - Absent build.omg == an empty `build` machine == the zero `Build`: the
//!   hosted console default.

use omega_core::diagnostics::Diagnostic;
use omega_interpreter::BuildTimeValue;
use omega_typed_trees::TypedTrees;

const BUILD_MACHINE: &str = "build";

/// The image facts the pipeline consumes, extracted from the augmented
/// `Build`. ZII: the default IS the zero value's meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfig {
    /// PE optional-header Subsystem word (console 3 when unstated).
    pub subsystem: u16,
    /// Freestanding image: empty host-ABI plan, no import thunks.
    pub freestanding: bool,
    /// CH10 ROOT GRANTS (GR3): the symbol paths the final build accepted
    /// via `b.accept_boundary<pkg::symbol>();` -- harvested STATICALLY
    /// from the build machine's marker calls (grants are declarations,
    /// not runtime effects; the evaluator serves the marker as a no-op).
    pub grants: Vec<String>,
    /// PRV4c: explicit provider-type choices for boundary slots. These are
    /// declarations harvested from the authoritative build machine; they are
    /// validated against derived candidates before selection grants anything.
    pub provider_selections: Vec<ProviderSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    pub boundary_trait: String,
    pub provider_type: String,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            subsystem: 3, // IMAGE_SUBSYSTEM_WINDOWS_CUI -- the Console case's meaning
            freestanding: false,
            grants: Vec::new(),
            provider_selections: Vec::new(),
        }
    }
}

/// Whether a machine is the program's build machine: named `build` (the
/// FREE pure-config shape) or `<Component>::build` (the dependency-
/// injection shape, owner answer #2) AND declared at a build.omg root --
/// the FILE is the identity (owner answer #3: build.omg is the home;
/// `MazeBuilder::build` in ordinary source is just a machine). The caller
/// threads the build-file machine names from the syntax stage, where
/// per-file item attribution exists; typed machines carry no source file.
/// A wrong-arity build machine still refuses at evaluation with the arity
/// error (pinned by fail/build/build_machine_wrong_arity).
pub(crate) fn is_build_machine(
    machine: &omega_typed_trees::machine::Machine,
    build_file_machines: &[String],
) -> bool {
    let name = machine.name.as_str();
    if name != BUILD_MACHINE && !name.ends_with("::build") {
        return false;
    }
    build_file_machines.iter().any(|declared| declared == name)
}

/// Evaluate the program's `build` machine (if any) and extract the config.
/// No `build` machine -> the default. Every failure names the machine.
pub(crate) fn compute_build_config(
    typed: &TypedTrees,
    build_file_machines: &[String],
) -> Result<BuildConfig, Vec<Diagnostic>> {
    // MP6: build.omg is interpreted before the ordinary checked-tree stage,
    // but compile-time machine parameters are consumed by that stage's
    // monomorphizer. Specialize a private clone first so a build helper such
    // as `apply<chosen>(value)` executes the same direct call that runtime
    // lowering sees. Keep the caller's tree untouched; it still needs the
    // complete checked validation and specialization-contract artifacts.
    let mut specialized = typed.clone();
    omega_typed_trees_to_checked_trees::specialize_static_machine_calls(&mut specialized)?;
    let typed = &specialized;

    let mut build_machines = typed
        .machines()
        .iter()
        .filter(|machine| is_build_machine(machine, build_file_machines));
    let Some(machine) = build_machines.next() else {
        return Ok(BuildConfig::default());
    };
    if let Some(second) = build_machines.next() {
        return Err(vec![Diagnostic::error(format!(
            "two build machines exist (`{}` and `{}`); a program declares at most one",
            machine.name.as_str(),
            second.name.as_str(),
        ))]);
    }
    let machine_name = machine.name.as_str();

    // The effect gate (owner answers, OWNER_QUESTIONS #4/#5, 2026-07-11i):
    // `build` MAY reach `filesystem_io` (staging assets is the point) and
    // console output (`stdout_io`/`stderr_io` -- "harmless and everyone
    // wants it") -- as DECLARED effects on the build machine itself,
    // forbidden anywhere else by the ordinary effect system. Anything
    // outside that set still refuses; a row-less boundary trait surfaces
    // as `host_boundary`, so the message points at declaring rows.
    let effect_plan = omega_effects::infer_effects(typed);
    let transitive = effect_plan
        .machines()
        .iter()
        .find(|entry| entry.symbol == machine.symbol)
        .map(|entry| entry.transitive)
        .unwrap_or_else(omega_effects::EffectSet::empty);
    if std::env::var_os("OMEGA_DEBUG_BUILD_CONFIG").is_some() {
        eprintln!(
            "BUILDCFG: machine `{}` found, transitive effects [{}]",
            machine.name.as_str(),
            transitive.names().collect::<Vec<_>>().join(", "),
        );
    }
    const ALLOWED_BUILD_EFFECTS: &[&str] = &["filesystem_io", "stdout_io", "stderr_io"];
    let disallowed: Vec<&str> = transitive
        .names()
        .filter(|name| !ALLOWED_BUILD_EFFECTS.contains(name))
        .collect();
    if !disallowed.is_empty() {
        let boundary_hint = if disallowed.contains(&"host_boundary") {
            " (a boundary trait without declared effect rows surfaces as \
             `host_boundary`; declare rows -- e.g. `effects stdout_io;` on a \
             console method -- so the gate can tell what the call does)"
        } else {
            ""
        };
        return Err(vec![Diagnostic::error(format!(
            "`{machine_name}` reaches effects `{}` -- build.omg may declare only \
             `{}`{boundary_hint}",
            disallowed.join(", "),
            ALLOWED_BUILD_EFFECTS.join("`, `"),
        ))]);
    }
    // The allowed effects must be DECLARED on the build machine (the owner's
    // "declared effect on the main func within build" -- reaching the
    // filesystem silently is refused even though it would be allowed).
    let declared = typed.machine_effects(machine);
    let undeclared: Vec<&str> = transitive
        .names()
        .filter(|name| !declared.iter().any(|effect| effect.as_str() == *name))
        .collect();
    if !undeclared.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "`{machine_name}` reaches effects `{}` without declaring them; add \
             `effects {}` to the build machine's signature",
            undeclared.join(", "),
            undeclared.join(", "),
        ))]);
    }

    let zero_build = BuildTimeValue::Struct {
        type_name: "Build".to_owned(),
        fields: vec![
            (
                "subsystem".to_owned(),
                BuildTimeValue::Case {
                    variant: "Console".to_owned(),
                    payload: Vec::new(),
                },
            ),
            ("freestanding".to_owned(), BuildTimeValue::Bool(false)),
        ],
    };

    // Effect-free builds keep the pure engine; a declared-filesystem build
    // runs the GRANTED entry (real filesystem, unscoped -- the owner
    // explicitly de-scoped permission ceremony: "I don't think we give a
    // shit about permissions at this point").
    let mut arguments = if transitive.is_empty() {
        omega_interpreter::evaluate_build_time_machine_arguments(
            typed,
            machine_name,
            vec![zero_build],
        )
    } else {
        omega_interpreter::evaluate_build_machine_with_filesystem(
            typed,
            machine_name,
            vec![zero_build],
            omega_interpreter::InterpretOptions {
                filesystem: omega_interpreter::FilesystemAccess::RealUnscoped,
            },
        )
    }
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` failed: {reason}"
        ))]
    })?;
    let augmented = arguments.pop().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` returned no argument values (expected the augmented Build)"
        ))]
    })?;

    let mut config = extract_build_config(&augmented).map_err(|reason| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` produced an invalid Build: {reason}"
        ))]
    })?;
    config.grants = harvest_root_grants(typed, machine);
    config.provider_selections = harvest_provider_selections(typed, machine)?;
    Ok(config)
}

/// PRV4c: collect `b.select_provider<BoundaryTrait, ProviderType>();` from
/// the one authoritative build machine. Merely spelling either type elsewhere
/// grants nothing; selection authority comes from this file-scoped root.
fn harvest_provider_selections(
    typed: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut selections: Vec<ProviderSelection> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |target: &str| {
        let Some(encoded) = target.strip_prefix("select_provider#") else {
            return;
        };
        let Some((boundary_trait, provider_type)) = encoded.split_once('#') else {
            diagnostics.push(Diagnostic::error(format!(
                "malformed provider-selection declaration `{target}`"
            )));
            return;
        };
        if let Some(existing) = selections
            .iter()
            .find(|selection| selection.boundary_trait == boundary_trait)
        {
            if existing.provider_type != provider_type {
                diagnostics.push(Diagnostic::error(format!(
                    "build selects two provider types for slot `{boundary_trait}`: `{}` and `{provider_type}`",
                    existing.provider_type,
                )));
            }
            return;
        }
        selections.push(ProviderSelection {
            boundary_trait: boundary_trait.to_owned(),
            provider_type: provider_type.to_owned(),
        });
    };
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                omega_typed_trees::statement::StatementNode::Expression(expression) => {
                    if let omega_typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    {
                        record(call.target.as_str());
                    }
                }
                omega_typed_trees::statement::StatementNode::Call(call) => {
                    record(call.target.as_str());
                }
                _ => {}
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(selections)
    } else {
        Err(diagnostics)
    }
}

/// PRV4c: collect the defaults declared by the selected target package(s).
/// `target_machines` records the authoritative machine names before erasing
/// their target markers. The declarations use the same type-per-slot marker as
/// build overrides, but retain distinct provenance so selection can apply the
/// precedence `build override > target default > compatibility fallback`.
pub(crate) fn compute_target_provider_defaults(
    typed: &TypedTrees,
    target_default_machine_names: &[String],
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut defaults = Vec::new();
    let mut diagnostics = Vec::new();
    for machine_name in target_default_machine_names {
        let Some(machine) = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected target provider-default machine `{machine_name}` did not survive lowering"
            )));
            continue;
        };
        match harvest_provider_selections(typed, machine) {
            Ok(mut machine_defaults) => defaults.append(&mut machine_defaults),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(defaults)
    } else {
        Err(diagnostics)
    }
}

/// The static grant harvest: every `accept_boundary#<path>` marker call in
/// the build machine's states (the postfix carve's desugar of
/// `b.accept_boundary<path>();`). Order-preserving, deduplicated.
fn harvest_root_grants(
    typed: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> Vec<String> {
    let mut grants: Vec<String> = Vec::new();
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            let handles: Vec<omega_typed_trees::expression::ExpressionHandle> = match statement {
                omega_typed_trees::statement::StatementNode::Expression(expression) => {
                    vec![*expression]
                }
                omega_typed_trees::statement::StatementNode::Call(call) => {
                    // A statement-level call keeps the marker in its target.
                    if let Some(path) = call.target.as_str().strip_prefix("accept_boundary#")
                        && !grants.iter().any(|existing| existing == path)
                    {
                        grants.push(path.to_owned());
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for handle in handles {
                if let omega_typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(handle)
                    && let Some(path) = call.target.as_str().strip_prefix("accept_boundary#")
                    && !grants.iter().any(|existing| existing == path)
                {
                    grants.push(path.to_owned());
                }
            }
        }
    }
    grants
}

fn extract_build_config(build: &BuildTimeValue) -> Result<BuildConfig, String> {
    let BuildTimeValue::Struct { fields, .. } = build else {
        return Err(format!("expected a Build struct, got {build:?}"));
    };
    let field = |name: &str| -> Result<&BuildTimeValue, String> {
        fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value)
            .ok_or_else(|| format!("the Build carries no `{name}` field"))
    };

    let subsystem = match field("subsystem")? {
        BuildTimeValue::Case { variant, payload } => {
            match variant.rsplit("::").next().unwrap_or(variant) {
                "Console" => 3u16,
                "Gui" => 2,
                "EfiApplication" => 10,
                "Unspecified" => match payload.iter().find(|(name, _)| name == "value") {
                    Some((_, BuildTimeValue::Int(value))) => {
                        u16::try_from(*value).map_err(|_| {
                            format!("Unspecified subsystem value {value} exceeds a u16")
                        })?
                    }
                    other => {
                        return Err(format!(
                            "Unspecified subsystem carries no integer value: {other:?}"
                        ));
                    }
                },
                other => return Err(format!("unknown Subsystem case `{other}`")),
            }
        }
        other => {
            return Err(format!(
                "Build.subsystem is not a Subsystem case: {other:?}"
            ));
        }
    };

    let freestanding = match field("freestanding")? {
        BuildTimeValue::Bool(value) => *value,
        other => return Err(format!("Build.freestanding is not a bool: {other:?}")),
    };

    Ok(BuildConfig {
        subsystem,
        freestanding,
        grants: Vec::new(),
        provider_selections: Vec::new(),
    })
}
