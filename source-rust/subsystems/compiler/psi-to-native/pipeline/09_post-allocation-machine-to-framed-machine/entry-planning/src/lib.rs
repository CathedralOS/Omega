//! Plan native entry and adapter instructions from the admitted calling plans and measured frames.
#![forbid(unsafe_code)]
pub use frame_planning::PhysicalFrameProgram;

/// Identifies recipes produced by physical planning. Its constructor stays
/// private to the planner; the encoder retains the identity on executable bridges.
///
/// An existing recipe cannot be reused as the key to issue different instructions.
///
/// ```compile_fail,E0308
/// use entry_planning::native_foreign_bridge::NativeForeignBridgePlan;
/// use physical_recipes::native_foreign_bridge::NativeForeignInstructions;
/// fn replace(original: NativeForeignBridgePlan, other: NativeForeignInstructions) -> NativeForeignBridgePlan {
///     NativeForeignBridgePlan::from_planner(original.clone(), other)
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalRecipeOrigin(());
pub mod body_cpu;
mod bridge_values;
pub mod callable_entry;
pub mod hosted_entry;
pub mod hosted_program;
pub mod imports;
pub mod installation_binding;
pub mod native_entry_bridge;
pub mod native_foreign_bridge;
pub mod physical_entry;
pub mod physical_stack_switch;
pub mod windows_internet;
