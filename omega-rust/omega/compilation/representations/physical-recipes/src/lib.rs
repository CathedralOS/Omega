//! Selected instructions, target recipes and their producing origin.
#![forbid(unsafe_code)]

pub mod callable_entry;
pub mod frame_protocol;
pub mod hosted_entry;
pub mod hosted_program;
pub mod imports;
pub mod native_entry_bridge;
pub mod native_foreign_bridge;
pub mod physical_entry;
pub mod physical_stack_switch;
pub mod windows_internet;

/// Only the producer can construct a recipe with its own private origin.
/// The origin is consumed once. Reading or cloning a recipe never yields the
/// origin needed to issue another one with different instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalRecipe<Origin, Instructions> {
    instructions: Instructions,
    origin: std::marker::PhantomData<fn(Origin) -> Origin>,
}

impl<Origin, Instructions> PhysicalRecipe<Origin, Instructions> {
    pub fn from_planner(_origin: Origin, instructions: Instructions) -> Self {
        Self {
            instructions,
            origin: std::marker::PhantomData,
        }
    }
}
