//! Optimizer module role: stage group. Assigned scalar expression, control, frame, and call carriers.

mod boolean;
mod calls_and_locations;
mod integer;

pub use boolean::{
    AssignedBooleanControl, AssignedBooleanExpression, AssignedConditionalBooleanArm,
};
pub use calls_and_locations::{
    AssignedCallArgument, AssignedCallDestination, AssignedScalarExpression,
    AssignedScalarLocation, EntryRegisterSpill, ExpressionFrame,
};
pub use integer::{
    AssignedConditionalIntegerArm, AssignedIntegerControl, AssignedIntegerExpression,
};
