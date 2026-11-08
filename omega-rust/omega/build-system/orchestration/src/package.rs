//! Package commands and package Builds: `package_commands` schedules review
//! work, `package_compilation` schedules admitted package Builds,
//! `package_inspection` runs an observational target roster, and
//! `package_review` runs a review pass dependency-first.

pub(crate) mod package_commands;
pub(crate) mod package_compilation;
pub(crate) mod package_inspection;
pub(crate) mod package_review;
