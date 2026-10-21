//! An exact owner hook uses its data declaration's telescope, not fresh method
//! binders. Ordinary callable qualification does not confer that cleanup role.

use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;
use typed_trees::{data::TypeParameter, machine::Machine, name::Identifier};

pub(crate) struct CallableDeclarationScope<'a> {
    pub(crate) type_parameters: &'a [TypeParameter],
    pub(crate) lifetime_parameters: &'a [Identifier],
    /// The table and source owner of handles inside this telescope.
    pub(crate) source: PackageReviewInput<'a>,
}

pub(crate) fn callable_declaration_scope<'a>(
    compilation: &'a PackageReviewInput<'_>,
    machine: &'a Machine,
) -> Result<CallableDeclarationScope<'a>, Vec<Diagnostic>> {
    let rejected = |reason| {
        vec![Diagnostic::error(format!(
            "package review callable scope for `{}` rejects {reason}",
            machine.name
        ))]
    };
    let owner = validation::nominal_cleanup_owner(compilation.checking_input, machine.symbol);
    let checked_owner = validation::nominal_cleanup_owner(&**compilation.custody, machine.symbol);
    if owner != checked_owner {
        return Err(rejected(
            "a nominal cleanup owner differing from checked declaration custody",
        ));
    }
    let Some(owner) = owner else {
        return Ok(CallableDeclarationScope {
            type_parameters: compilation.machine_type_parameters(machine),
            lifetime_parameters: &machine.lifetime_parameters,
            source: *compilation,
        });
    };
    let checked_machine = compilation
        .custody
        .machine(machine.symbol)
        .ok_or_else(|| rejected("a nominal cleanup hook absent from checked custody"))?;
    if !machine.lifetime_parameters.is_empty()
        || !compilation.machine_type_parameters(machine).is_empty()
        || !checked_machine.lifetime_parameters.is_empty()
        || !compilation
            .custody
            .machine_type_parameters(checked_machine)
            .is_empty()
    {
        return Err(rejected("method-local binders on a nominal cleanup hook"));
    }
    let definition = compilation
        .unique_data_definition_by_symbol(owner)
        .ok_or_else(|| rejected("a missing or ambiguous nominal cleanup data owner"))?;
    let checked_definition = compilation
        .custody
        .unique_data_definition_by_symbol(owner)
        .ok_or_else(|| rejected("a nominal cleanup data owner absent from checked custody"))?;
    let checked_parameters = compilation.custody.data_type_parameters(checked_definition);
    if definition.lifetime_parameters != checked_definition.lifetime_parameters
        || compilation.data_type_parameters(definition) != checked_parameters
    {
        return Err(rejected(
            "a nominal cleanup owner telescope differing from checked declaration custody",
        ));
    }
    // The declaration's order and original binder symbols are authoritative.
    // No lifetime is inferred from a receiver's spelling, and no captured
    // owner binder becomes a newly callable method parameter.
    Ok(CallableDeclarationScope {
        type_parameters: checked_parameters,
        lifetime_parameters: &checked_definition.lifetime_parameters,
        source: compilation.custody.into(),
    })
}
