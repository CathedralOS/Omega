//! Optimizer module role: stage group. Fragment emission, placement, object-container, and final artifact stages.

pub(crate) mod function_fragment_emission;
pub(crate) mod function_fragment_object_container;
pub(crate) mod function_fragment_text_section;
pub(crate) mod object_artifact;

pub use function_fragment_emission::*;
pub use function_fragment_object_container::*;
pub use function_fragment_text_section::*;
pub use object_artifact::*;
