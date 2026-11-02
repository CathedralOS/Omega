//! Every callable, including a cleanup hook selected with `dropped by`,
//! retains its own ordinary declaration scope.

use crate::capture::SourceDeclarationInput;
use diagnostics::Diagnostic;
use std::borrow::Cow;
use typed_trees::{data::TypeParameter, machine::Machine, name::Identifier};

pub(crate) struct CallableDeclarationScope<'a> {
    pub(crate) type_parameters: Cow<'a, [TypeParameter]>,
    pub(crate) lifetime_parameters: Cow<'a, [Identifier]>,
    /// The table and source owner of handles inside this telescope.
    pub(crate) source: SourceDeclarationInput<'a>,
}

pub(crate) fn callable_declaration_scope<'a>(
    compilation: &'a SourceDeclarationInput<'_>,
    machine: &'a Machine,
) -> Result<CallableDeclarationScope<'a>, Vec<Diagnostic>> {
    let rejected = |reason| {
        vec![Diagnostic::error(format!(
            "package review callable scope for `{}` rejects {reason}",
            machine.name
        ))]
    };
    let owner = validation::nominal_cleanup_owner(compilation.checking_input, machine.symbol);
    let checked_owner =
        validation::nominal_cleanup_owner(compilation.original_program, machine.symbol);
    if owner != checked_owner {
        return Err(rejected(
            "a nominal cleanup owner differing from checked declaration custody",
        ));
    }
    // Cleanup hooks are independent machines selected by `dropped by`; like
    // every ordinary member they retain the complete accepted lexical
    // telescope, including enclosing named implementations. Specialization
    // captures are not new binders; this is the existing declaration-scope query.
    let (type_parameters, lifetime_parameters) = compilation
        .machine_declaration_scope(machine, |existing, parameter| {
            existing.symbol == parameter.symbol
        });
    Ok(CallableDeclarationScope {
        type_parameters: Cow::Owned(type_parameters),
        lifetime_parameters: Cow::Owned(lifetime_parameters),
        source: *compilation,
    })
}
