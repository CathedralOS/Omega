//! Machines, calls and the applications that specialize their signatures.

pub(crate) mod callable_applications;
pub(crate) mod callable_copies;
pub mod callable_lifetimes;
pub mod callable_signature_shapes;
pub mod callable_values;
pub(crate) mod calls_and_control_flow;
pub(crate) mod machine_instances;
pub(crate) mod machine_specializations;
pub(crate) mod private_callback_slots;
pub mod runtime_callable_identity;
pub mod static_operand_binding;
