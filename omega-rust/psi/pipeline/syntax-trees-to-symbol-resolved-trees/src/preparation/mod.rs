//! Syntax to syntax, before any symbol exists.
//!
//! `module_normalization` validates the forest against the constant selector,
//! `generic_data` closes eligible generic data applications, and
//! `trait_defaults` materializes default trait machines as ordinary attached
//! machines. All three consume syntax and return syntax; their templates,
//! substitutions, and evaluation scratch are not program representations.

pub(crate) mod generic_data;
pub(crate) mod machine_equations;
pub(crate) mod module_normalization;
pub(crate) mod trait_defaults;
pub(crate) mod type_equations;

use diagnostics::Diagnostic;
use source::SourceMap;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;

/// A forest ready to lower, with the constant selector its declarations
/// resolved under.
pub(crate) struct Prepared {
    pub(crate) syntax: SyntaxTrees,
    pub(crate) constant_selection: generic_data::constant_selection::ConstantSelection<'static>,
}

/// Select constants, validate the forest for the requested constant
/// resolution, and materialize default trait machines. Generic data
/// normalization is the caller's earlier step, not part of this one.
pub(crate) fn prepare(
    syntax: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    constants: crate::resolution::lowerer::ConstResolutionMode,
) -> Result<Prepared, Vec<Diagnostic>> {
    let constant_selection = generic_data::constant_selection::ConstantSelection::new(
        syntax,
        sources,
        top_level_bindings,
    )?;
    module_normalization::validate_with_const_resolution_mode(
        syntax,
        &constant_selection,
        constants,
    )?;
    let mut syntax = syntax.clone();
    trait_defaults::synthesize_trait_defaults_after_module_validation(
        &mut syntax,
        &constant_selection,
    )?;
    Ok(Prepared {
        syntax,
        constant_selection,
    })
}
