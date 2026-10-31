//! Every conformance row uses the ordinary callable borrow judgment. A named
//! map and an exact edge differ in supplying custody, not in whether results
//! must preserve regions. Borrow the original nodes and simultaneous scopes;
//! an executable carrier view cannot represent mixed captured/fresh binders.

use crate::borrow_contracts::{CallableBorrowContract, validate_callable_borrow_refinement};
use typed_trees::data::TypeParameter;
use typed_trees::machine::Machine;
use typed_trees::signature::{StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::trait_definition::AppliedTraitScope;

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    application: &AppliedTraitScope<'_>,
    requirement: &StateSignature,
    machine: &Machine,
    state: &State,
    actual_parameters: &[StateParameter],
    actual_type_parameters: &[TypeParameter],
) -> Result<(), &'static str> {
    let binders = program
        .state_signature_type_parameters(requirement)
        .iter()
        .zip(actual_type_parameters)
        .enumerate()
        .flat_map(|(ordinal, (required, actual))| {
            let name = format!("$conformance{ordinal}");
            [(required.symbol, name.clone()), (actual.symbol, name)]
        })
        .collect::<Vec<_>>();
    application.with_identity_scopes(program.typed_program(), |scopes| {
        validate_callable_borrow_refinement(
            program,
            CallableBorrowContract {
                lexical_owner: requirement.symbol,
                lifetime_parameters: &requirement.lifetime_parameters,
                parameters: program.state_signature_parameters(requirement),
                return_type: requirement.return_type,
                substitutions: &application.substitutions,
                substitution_scope_starts: &application.substitution_scope_starts,
                application_scopes: scopes,
                binders: &binders,
                lifetime_bindings: &[],
                argument_owner: application.argument_owner,
                supplied_telescope: false,
            },
            CallableBorrowContract {
                lexical_owner: machine.symbol,
                lifetime_parameters: &machine.lifetime_parameters,
                parameters: actual_parameters,
                return_type: state.return_type,
                substitutions: &[],
                substitution_scope_starts: &[],
                application_scopes: &[],
                binders: &binders,
                lifetime_bindings: &[],
                argument_owner: machine.symbol,
                supplied_telescope: false,
            },
        )
    })
}
