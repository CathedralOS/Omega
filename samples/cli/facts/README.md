# Fact samples

This directory contains retained fact samples and is subject to the repository's [freeze](../../../AGENTS.md#no-omg-file-bloat). The former `tests/omega/` corpus is retired ([tests/README.md](../../../tests/README.md#the-former-corpus)). Follow the current repository testing instructions when repairing a sample. Shared dependency packages are described below.

## Dependency packages

Named decision: `fact-sample-dependency-packages`, ratified 2026-11-13.

A fact sample that crosses a package or component boundary keeps its dependency packages under `samples/cli/support/fact_dependencies/`, outside this frozen directory, and each place reaches its package through a relative `Source::Path`. There is one package per distinct dependency contract: equivalent copies are shared, and a variant that distinguishes a test, such as a negative twin's weaker published contract, stays a separate package. A missing dependency is a repository defect, not an intended negative outcome. Restoring one keeps the boundary the sample exists to cross: no same-package helper and no fusing of independently produced components. This directory's freeze is unchanged.
