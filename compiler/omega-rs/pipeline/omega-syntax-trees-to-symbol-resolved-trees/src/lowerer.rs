use crate::item::lower_item;
use omega_core::diagnostics::Diagnostic;
use omega_core::source::SourceMap;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_syntax_trees::SyntaxTrees;
use std::sync::Arc;

pub fn lower_syntax_trees(syntax_trees: &SyntaxTrees) -> Result<SymbolResolvedTrees, Diagnostic> {
    lower_syntax_trees_with_optional_sources(syntax_trees, None)
}

pub fn lower_syntax_trees_with_sources(
    syntax_trees: &SyntaxTrees,
    sources: Arc<SourceMap>,
) -> Result<SymbolResolvedTrees, Diagnostic> {
    lower_syntax_trees_with_optional_sources(syntax_trees, Some(sources))
}

fn lower_syntax_trees_with_optional_sources(
    syntax_trees: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
) -> Result<SymbolResolvedTrees, Diagnostic> {
    let mut lowerer = Lowerer::new(sources);

    for item in syntax_trees.root_items() {
        lower_item(&mut lowerer, syntax_trees, item)?;
    }

    lowerer.finish()
}

pub(crate) struct Lowerer {
    pub(crate) symbol_resolved_trees: SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
    /// Per-lowering counter that mints unique names for synthetic `let`
    /// temporaries hoisted out of operand-position indexed reads (see
    /// `statement::hoist_indexed_operands`). `__hoist_` prefixed so the
    /// generated names cannot collide with source identifiers.
    hoist_counter: u32,
    /// Names of the CURRENT state's parameters declared as a shared reference
    /// to a NAMED type (`table: &EfiSystemTable`). A member read through one
    /// (`table.con_out`) must dereference the pointer slot; the flat fold reads
    /// frame garbage (the entry-ref-param face). The guard/operand hoists use
    /// this to materialize such reads into `let` temps, which lower through the
    /// boot-verified pointee path. Overwritten at each state's lowering.
    pub(crate) reference_struct_parameters: Vec<String>,
    /// ALL of the current state's parameter names -- the computed-index hoist
    /// gate uses this to tell a typeable bare-Name operand (a param, whose
    /// declared type the typed layer resolves via `parameter_type`) from a
    /// LOCAL (untypeable at the hoist-temp layer; hoisting would mint a Unit
    /// temp and a confusing layout error). Overwritten at each state.
    pub(crate) current_state_parameter_names: Vec<String>,
    /// Whether the machine currently being lowered is a BOUNDARY machine. Only
    /// boundary params are REAL pointer slots (the entry hand-off vouches
    /// them); a non-boundary `&Struct` param is a call-site ALIAS slot sharing
    /// the caller's storage, and materializing a pointee deref through one
    /// loads a non-pointer as an address (segfault -- probed 2026-07-04). The
    /// ref-param hoist is scoped to boundary machines.
    pub(crate) current_machine_is_boundary: bool,
    /// Maps a match SUBJECT syntax expression handle to the name of the single
    /// hoisted temp for it. All arms of one enum-variant match share the same
    /// syntax subject handle (the parser reuses it across arms), so the first
    /// arm mints `let __hoist_N = <subject>` and the siblings reuse the name --
    /// keeping ONE shared subject so match exhaustiveness still groups the arms
    /// (`statement::hoist_membership_match_subject`). Keyed by the subject
    /// syntax handle's arena index (`Handle` is not `Hash`).
    match_subject_temps: std::collections::HashMap<u32, String>,
    /// The CURRENT state's parameters (name + resolved type) -- the
    /// guarded-arm value-call rewrite copies parameter records into its
    /// synthesized continuation state. Overwritten at each state.
    pub(crate) current_state_parameters:
        Vec<(String, omega_symbol_resolved_trees::types::TypeReference)>,
    /// The CURRENT state's declared return type -- the synthesized
    /// continuation state returns the same type. Overwritten at each state.
    pub(crate) current_state_return_type: Option<omega_symbol_resolved_trees::types::TypeReference>,
    /// Continuation states synthesized by the guarded-arm value-call rewrite
    /// (`cond -> (call(a, b))` becomes `cond -> __arm_k_N(a, b)` plus a
    /// state whose Always terminal hoists the call). Drained by the machine
    /// lowering after the authored states.
    pub(crate) pending_synthesized_states: Vec<SynthesizedArmState>,
    arm_state_counter: u32,
}

/// One continuation state the guarded-arm value-call rewrite synthesizes.
pub(crate) struct SynthesizedArmState {
    pub(crate) name: String,
    pub(crate) parameters: Vec<(String, omega_symbol_resolved_trees::types::TypeReference)>,
    pub(crate) return_type: omega_symbol_resolved_trees::types::TypeReference,
    /// The original call expression -- its Name arguments resolve against
    /// the synthesized state's SAME-named parameters.
    pub(crate) call: omega_symbol_resolved_trees::expression::ExpressionHandle,
}

impl Lowerer {
    fn new(sources: Option<Arc<SourceMap>>) -> Self {
        Self {
            symbol_resolved_trees: SymbolResolvedTrees::default(),
            sources,
            hoist_counter: 0,
            reference_struct_parameters: Vec::new(),
            current_state_parameter_names: Vec::new(),
            current_machine_is_boundary: false,
            match_subject_temps: std::collections::HashMap::new(),
            current_state_parameters: Vec::new(),
            current_state_return_type: None,
            pending_synthesized_states: Vec::new(),
            arm_state_counter: 0,
        }
    }

    pub(crate) fn next_arm_state_name(&mut self) -> String {
        let name = format!("__arm_k_{}", self.arm_state_counter);
        self.arm_state_counter += 1;
        name
    }

    /// The shared hoist-temp name for a match subject syntax handle, if the first arm already
    /// minted one; otherwise records `name` as the temp for later arms and returns None.
    pub(crate) fn match_subject_temp(&mut self, subject_arena_index: u32) -> Option<String> {
        self.match_subject_temps.get(&subject_arena_index).cloned()
    }

    pub(crate) fn record_match_subject_temp(&mut self, subject_arena_index: u32, name: String) {
        self.match_subject_temps.insert(subject_arena_index, name);
    }

    pub(crate) fn next_hoist_name(&mut self) -> String {
        let name = format!("__hoist_{}", self.hoist_counter);
        self.hoist_counter += 1;
        name
    }

    pub(crate) fn finish(mut self) -> Result<SymbolResolvedTrees, Diagnostic> {
        crate::symbols::assign_symbols(&mut self.symbol_resolved_trees, self.sources);
        crate::service_reaches::normalize_service_reaches(&mut self.symbol_resolved_trees);
        self.symbol_resolved_trees.rebuild_tables();
        let SymbolResolvedTrees {
            roots,
            tables,
            symbols,
            service_reaches,
            service_reach_rows,
            semantic_domains,
            external_bindings,
        } = self.symbol_resolved_trees;

        let mut trees = SymbolResolvedTrees::with_roots(roots, tables, symbols);
        // The interned semantic rows/domains built during lowering survive
        // the rebuild.
        trees.service_reaches = service_reaches;
        trees.service_reach_rows = service_reach_rows;
        trees.semantic_domains = semantic_domains;
        trees.external_bindings = external_bindings;
        Ok(trees)
    }
}

#[cfg(test)]
mod tests;
