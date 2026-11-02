//! Products beside the lowered machine: `program_roots` (the borrowed source
//! roots of one portable program), `external_provider_declarations` (the
//! declaration-only projection package review reads without an executable
//! root), `task_activation` (lowered task activation demands) and
//! `lowered_provenance` (diagnostic source attribution).

pub(crate) mod external_provider_declarations;
pub(crate) mod lowered_provenance;
pub(crate) mod program_roots;
pub(crate) mod task_activation;
