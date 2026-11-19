# Scientific sample validation

Each sample owns its Omega program, controls, inputs and numerical expectations. A recorded Terminal run of `pareto_experiment_selection` or `spatial_diffusion` can be checked against the frozen fields that sample's capture driver compared.

```sh
mbx run -p scientific-capture-check -- samples/cli/scientific/pareto_experiment_selection /path/to/terminal-captures
mbx run -p scientific-capture-check -- samples/cli/scientific/spatial_diffusion /path/to/terminal-captures
```

The capture directory must contain `NAME.stdout`, `NAME.stderr` and `NAME.exit` for every case in the sample. Each `.exit` contains the process exit code in decimal. The checker reads those files without changing them. Both suites require the expected exit code and empty stderr. Pareto also requires the exact stdout bytes for each of its 28 inputs. Diffusion reads the expected exit for each of its 20 cases from `data/oracle.json`. Numerical accuracy and the origin of a recorded run are separate questions from these capture comparisons.

## Running the programs through a project Build

The Rust drivers named in the individual sample READMEs used the retired package orchestration API. Their source and manifests are filed by sample in the [scientific archive](../../../_recycling/samples/cli/scientific/). That includes the shared runner and its eleven sample rosters, the dedicated spectrometer, travel-time, differential-expression and calorimetry drivers, and the individual Rust probes that were compiled outside the workspace. These sources record the original stdin, native child limits, Terminal work budgets, package-review behavior and comparisons needed for execution to resume.

To produce fresh captures, the application must supply a concrete `project_build::BuildHost` that resolves a local sample's `build.omg`, reviews and prepares its declared dependencies, runs the admitted Build stages, then returns the selected product for `linux_x86_64`. The real product must retain its published Psi, matched runtime provisions and native image. A driver can then execute the same product with the sample's allowed host services and publish its native executable. The saved-Psi compiler entry already accepts an independently authorized artifact. The samples here supply programs and package Builds, with no such prepublished artifact or manifest authorization.

The spectrometer's `--selection-identity` assertion additionally needs a checked product exposing the finalized equality selection occurrences. The Pareto driver's source-free schema identity regressions run with their owning `provider-plans` crate.

[Build execution and package review](../../../wiki/spec/build/execution.md) · [Project Build interface](../../../source-rust/subsystems/compiler/build/build.rs) · [Compiler requests and products](../../../source-rust/subsystems/compiler/compiler.rs)
