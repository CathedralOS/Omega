// Package workflow modules: `package_commands` drives install/update review loops, `package_compilation` compiles admitted package plans, `package_inspection` runs `omega packages audit`, and `package_review` executes dependency-first review rounds.

pub(crate) mod package_commands;
pub(crate) mod package_compilation;
pub(crate) mod package_inspection;
pub(crate) mod package_review;
