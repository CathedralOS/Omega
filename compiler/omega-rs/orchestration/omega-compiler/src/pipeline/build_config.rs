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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildConfig {
    /// PE optional-header Subsystem word (console 3 when unstated).
    pub subsystem: u16,
    /// Freestanding image: empty host-ABI plan, no import thunks.
    pub freestanding: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            subsystem: 3, // IMAGE_SUBSYSTEM_WINDOWS_CUI -- the Console case's meaning
            freestanding: false,
        }
    }
}

/// Evaluate the program's `build` machine (if any) and extract the config.
/// No `build` machine -> the default. Every failure names the machine.
pub(crate) fn compute_build_config(typed: &TypedTrees) -> Result<BuildConfig, Vec<Diagnostic>> {
    let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == BUILD_MACHINE)
    else {
        return Ok(BuildConfig::default());
    };

    // The purity gate: decision 12's transitive effect surface must be empty
    // (same discipline as every build-time policy evaluation).
    let effect_plan = omega_effects::infer_effects(typed);
    let transitive = effect_plan
        .machines()
        .iter()
        .find(|entry| entry.symbol == machine.symbol)
        .map(|entry| entry.transitive)
        .unwrap_or_else(omega_effects::EffectSet::empty);
    if !transitive.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "`{BUILD_MACHINE}` is not effect-free: it reaches effects `{}`; build.omg is \
             interpreted at build time and may only describe, never do",
            transitive.names().collect::<Vec<_>>().join(", ")
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

    let mut arguments = omega_interpreter::evaluate_build_time_machine_arguments(
        typed,
        BUILD_MACHINE,
        vec![zero_build],
    )
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "build-time evaluation of `{BUILD_MACHINE}` failed: {reason}"
        ))]
    })?;
    let augmented = arguments.pop().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "`{BUILD_MACHINE}` returned no argument values (expected the augmented Build)"
        ))]
    })?;

    extract_build_config(&augmented).map_err(|reason| {
        vec![Diagnostic::error(format!(
            "`{BUILD_MACHINE}` produced an invalid Build: {reason}"
        ))]
    })
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
        other => return Err(format!("Build.subsystem is not a Subsystem case: {other:?}")),
    };

    let freestanding = match field("freestanding")? {
        BuildTimeValue::Bool(value) => *value,
        other => return Err(format!("Build.freestanding is not a bool: {other:?}")),
    };

    Ok(BuildConfig {
        subsystem,
        freestanding,
    })
}
