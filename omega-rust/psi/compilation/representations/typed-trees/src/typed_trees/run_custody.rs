//! Empty-run retirement returns the existing backing claim, not a new grant.
//!
//! Unlike cell placement, this operation needs neither a layout nor a provider
//! profile: its checked preconditions establish that no cells or occupied bytes
//! remain. Authenticate the bundled declaration before admitting that intrinsic
//! relation. An identically named application declaration has no such privilege.

use crate::TypedTrees;
use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;

const RUN_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/run.omg"
));

/// The exact core entry whose requires clauses were checked at the call site.
/// Recognition does not consume its input or establish output custody; the
/// executable operation must transfer the input run's actual storage claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyRunReleaseDeclaration {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub template: SymbolHandle,
}

impl EmptyRunReleaseDeclaration {
    pub fn authenticate(
        program: &TypedTrees,
        symbol: SymbolHandle,
    ) -> Result<Option<Self>, Diagnostic> {
        let Some(machine) = program
            .machine(symbol)
            .or_else(|| program.machine_holding_state(symbol))
        else {
            return Ok(None);
        };
        let template = program
            .machine_specialization(machine.symbol)
            .map_or(machine.symbol, |application| application.template);
        let Some(source) = program
            .symbols
            .symbol_source_span(template)
            .and_then(|span| program.symbols.source_file(span))
        else {
            return Ok(None);
        };
        if source.origin != source::SourceOrigin::Toolchain
            || source.package_identity.is_some()
            || source.path.strip_prefix(&source.package_root).ok()
                != Some(std::path::Path::new("run.omg"))
            || source.source.as_bytes() != RUN_SOURCE
            || program
                .normalized_hermetic_symbol_identity(template)
                .ok()
                .as_deref()
                != Some("toolchain::Run::retire_empty")
        {
            return Ok(None);
        }
        let malformed = |reason: &str| Diagnostic::error(format!("empty-run release {reason}"));
        // A private bodyless boundary with ensures is classified as an
        // admission claim. Here its implementation is this authenticated core
        // operation, not a selectable host promising equal-shaped bytes.
        if machine.body_is_present
            || machine.supply_mode != MachineSupplyMode::AdmissionClaim
            || !machine.lifetime_parameters.is_empty()
            || machine.spelling.is_some()
            || machine.suspends
            || machine.blocks
            || machine.target.is_some()
            || machine.authored_target.is_some()
            || machine.target_family_interface
            || !program.machine_trait_conformances(machine).is_empty()
            || !machine.invokes.is_empty()
            || machine.service_reach_is_installation_bound
        {
            return Err(malformed(&format!(
                "changed its core envelope (body={}, supply={:?}, lifetimes={}, spelling={}, suspends={}, blocks={}, target={}, authored_target={}, family={}, conformances={}, invokes={}, installed_reach={})",
                machine.body_is_present,
                machine.supply_mode,
                machine.lifetime_parameters.len(),
                machine.spelling.is_some(),
                machine.suspends,
                machine.blocks,
                machine.target.is_some(),
                machine.authored_target.is_some(),
                machine.target_family_interface,
                program.machine_trait_conformances(machine).len(),
                machine.invokes.len(),
                machine.service_reach_is_installation_bound,
            )));
        }
        let [state] = program.machine_states(machine) else {
            return Err(malformed("must retain exactly one entry state"));
        };
        let [parameter] = program.state_parameters(state) else {
            return Err(malformed("must retain exactly one run parameter"));
        };
        if (symbol != machine.symbol && symbol != state.symbol)
            || parameter.is_const
            || parameter.is_mutable
            || parameter.relevance.is_erased()
        {
            return Err(malformed(&format!(
                "changed its entry parameter (entry={}, const={}, mutable={}, erased={})",
                symbol == machine.symbol || symbol == state.symbol,
                parameter.is_const,
                parameter.is_mutable,
                parameter.relevance.is_erased(),
            )));
        }
        Ok(Some(Self {
            machine: machine.symbol,
            state: state.symbol,
            template,
        }))
    }
}
