/*
We project exact selected-conformance applications and callable bounds.

We reconstruct selections against the original checked input. Reifying their
arguments appends type references to invocation-local typed trees rather than
mutating or cloning a checked compilation for scratch space, while the common
type-identity projector borrows the original exact source commitments.
*/

mod application;
mod bounds;
mod policy;
mod policy_arguments;
mod policy_callables;
mod projected_application;

pub(crate) use application::project_selected_conformance_application;
pub(crate) use bounds::project_conformance_bounds;
pub(crate) use policy::project_checked_conformance_policy;
pub(crate) use policy_callables::callable_identity as policy_callable_identity;

pub(crate) use policy_arguments::argument_type_reference;
