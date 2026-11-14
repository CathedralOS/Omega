# Witness tree

This directory is the old witness tree. It is not the pin set. Do not add a program here for a fact that tests/omega already pins. A new file here is only for an obligation that has no corpus pin yet.

The pin set is tests/omega/pass/facts, tests/omega/fail/facts, and tests/omega/run/facts. A fact lives there once. compiler-check keeps those pins.

## Dependency packages

Named decision: `fact-sample-dependency-packages`, ratified 2026-11-13.

A fact sample that crosses a package or component boundary keeps its dependency packages under `samples/cli/support/fact_dependencies/`, outside this frozen directory, and each place reaches its package through a relative `Source::Path`. There is one package per distinct dependency contract: equivalent copies are shared, and a variant that distinguishes a test, such as a negative twin's weaker published contract, stays a separate package. A missing dependency is a repository defect, not an intended negative outcome. Restoring one keeps the boundary the sample exists to cross: no same-package helper and no fusing of independently produced components. This directory's freeze is unchanged.
