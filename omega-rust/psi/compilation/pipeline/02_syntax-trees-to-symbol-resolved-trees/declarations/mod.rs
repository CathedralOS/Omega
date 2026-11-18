/*
    Syntax to syntax, before any symbol exists.

    `declaration_selection` owns the partial namespace used to select constants,
    nominal carriers, domains and traits before complete symbol assignment.
    Preparation and resolution retain that shared selection while their own
    consumers validate modules, synthesize defaults and close applications.

    `module_normalization` validates the forest against the constant selector,
    `generic_data` closes eligible generic data applications, and
    `trait_defaults` materializes default trait machines as ordinary attached
    machines. All three consume syntax and return syntax; their templates,
    substitutions, and evaluation scratch are not program representations.
*/

pub(in crate::declarations) mod callable_signatures;

pub(crate) mod assembly;
mod builtin_type_names;
mod callable_types;
pub(crate) mod declaration_selection;
pub(crate) mod declared_types;
mod dispatches;
pub(crate) mod generic_data;
pub(crate) mod integer_widths;
pub(crate) mod machine_equations;
pub(crate) mod module_normalization;
pub(crate) mod target_families;
pub(crate) mod trait_defaults;
pub(crate) mod type_equations;

use declaration_selection::ConstantSelection;
use diagnostics::Diagnostic;
use module_normalization::validate_with_const_resolution_mode;
use source::SourceMap;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use trait_defaults::synthesize_trait_defaults_after_module_validation;

/// Expand source statements before any path can resolve machine bodies.
pub(crate) fn prepare_statement_bodies(syntax: &mut SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    assembly::prepare(syntax)?;
    dispatches::prepare(syntax);
    Ok(())
}

/*
    A forest ready to lower, with the constant selector its declarations
    resolved under.
*/
pub(crate) struct Prepared {
    pub(crate) syntax: SyntaxTrees,
    pub(crate) constant_selection: ConstantSelection<'static>,
}

/*
    Select constants, validate the forest for the requested constant
    resolution, and materialize default trait machines. Generic data
    normalization is the caller's earlier step, not part of this one.

    The forest arrives by value and is rewritten in place. Preparation used to
    clone the whole forest it was handed by reference, which charged every
    resolution one arena copy of the entire program to reach the same
    rewritten syntax the caller then stopped reading.
*/
pub(crate) fn prepare(
    mut syntax: SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    constants: crate::lowering::lowerer::ConstResolutionMode,
    base: Option<Arc<symbol_resolved_trees::SymbolResolvedTrees>>,
    base_declarations: Option<&SyntaxTrees>,
) -> Result<Prepared, Vec<Diagnostic>> {
    prepare_statement_bodies(&mut syntax)?;
    builtin_type_names::validate(&syntax)?;
    target_families::normalize(&mut syntax, sources.as_deref())?;
    let mut constant_selection = declaration_selection::ConstantSelection::with_shared_base(
        &syntax,
        sources,
        top_level_bindings,
        base,
    )?;
    declared_types::normalize(
        &mut syntax,
        &constant_selection,
        base_declarations.as_slice(),
    )?;
    // Closed literal widths must name carriers before constants land in them.
    // Named widths remain for resolution in their lexical scope.
    constant_selection.install_integer_types(integer_widths::normalize(&mut syntax)?);
    generic_data::domain_heads::normalize(&mut syntax, Some(&constant_selection))?;
    /*
        Only the complete route owns a constant's value. The selection routes
        exist to read initializers as their author wrote them, so they keep
        every initializer authored.
    */
    if constants == crate::lowering::lowerer::ConstResolutionMode::Complete {
        generic_data::fold_closed_scalar_initializers(&mut syntax, &constant_selection)?;
    }
    let integer_types = integer_widths::normalize(&mut syntax)?;
    let new_integer_types = !integer_types.is_empty();
    constant_selection.install_integer_types(integer_types);
    if new_integer_types && constants == crate::lowering::lowerer::ConstResolutionMode::Complete {
        // Rewritten literal widths can make another initializer's carrier
        // available without resolving named widths outside their lexical scope.
        generic_data::fold_closed_scalar_initializers(&mut syntax, &constant_selection)?;
    }
    validate_with_const_resolution_mode(&syntax, &constant_selection, constants)?;
    synthesize_trait_defaults_after_module_validation(&mut syntax, &constant_selection)?;
    Ok(Prepared {
        syntax,
        constant_selection,
    })
}
