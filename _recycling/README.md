# Recycling archive

This directory holds implementations and samples kept for reference or piecemeal migration. Start with the former owner’s path from the repository root, then the item name. For example, the native compiler’s archived derivation format is under `source-rust/subsystems/compiler/psi-to-native/native-derivation-record/`. To restore a piece, move it to its current owner and connect its callers there.

| Area | Archived material | Current owner |
| --- | --- | --- |
| Project builds | [V1 Build orchestration](source-rust/compiler/v1-build-orchestration/) | [Project Build](../source-rust/subsystems/compiler/build/README.md) |
| Source compilation | [Earlier evaluation, input and language implementations](source-rust/subsystems/compiler/source-to-psi/) | [Source to Psi](../source-rust/subsystems/compiler/source-to-psi/README.md) |
| Native compilation | [Native derivation records](source-rust/subsystems/compiler/psi-to-native/native-derivation-record/README.md) | [Psi to native](../source-rust/subsystems/compiler/psi-to-native/README.md) |
| Package review | [V1 Product capture](source-rust/subsystems/package-manager/review/evidence/v1-product-capture/) | [Package review evidence](../source-rust/subsystems/package-manager/review/evidence/README.md) |
| Scientific samples | [Retired sample programs](samples/cli/scientific/) | [Scientific sample catalog](../samples/cli/scientific/README.md) |

## Tests and developer tools

The [test archive](tests/) and [tool archive](tools/) mirror their former repository paths. Restore a fixture with its local Build inputs when an active test needs it. Restore a tool with its entry point and verify the command from its documented working directory. The [development guides](../wiki/development/) cover the current workflow. GitHub Pages serves the archived Atlas HTML and assets at their existing URLs.
