# Scientific sample validation

Each sample owns its Omega program, controls, inputs and numerical expectations. A recorded Terminal run of `pareto_experiment_selection` or `spatial_diffusion` can be checked against the frozen fields that sample's capture driver compared.

```sh
mbx run -p scientific-capture-check -- samples/cli/scientific/pareto_experiment_selection /path/to/terminal-captures
mbx run -p scientific-capture-check -- samples/cli/scientific/spatial_diffusion /path/to/terminal-captures
```

The capture directory must contain `NAME.stdout`, `NAME.stderr` and `NAME.exit` for every case in the sample. Each `.exit` contains the process exit code in decimal. The checker reads those files without changing them. Both suites require the expected exit code and empty stderr. Pareto also requires the exact stdout bytes for each of its 28 inputs. Diffusion reads the expected exit for each of its 20 cases from `data/oracle.json`. Numerical accuracy and the origin of a recorded run are separate questions from these capture comparisons.

Use `omega run` to lower source to Psi and interpret it. To collect a native observation, compile with `omega <root.omg>` and launch the published image on its target host. Compare the two captures for termination, stdout and stderr. Earlier sample reports that recorded only agreement on exit establish that result for the compiler used at the time. A frozen output file can also establish whether both engines printed the expected answer. [CLI execution](../../../source-rust/application/README.md)

## Running the programs through a project Build

The Rust drivers named in the individual sample READMEs used the retired package orchestration API. Their source and manifests are filed by sample in the [scientific archive](../../../_recycling/samples/cli/scientific/). That includes the shared runner and its eleven sample rosters, the dedicated spectrometer, travel-time, differential-expression and calorimetry drivers, and the individual Rust probes that were compiled outside the workspace. These sources record the original stdin, native child limits, Terminal work budgets, package-review behavior and comparisons needed for execution to resume.

A capture driver can use the same project-planning route as the CLI. The application opens the local `build.omg`, prepares its declared dependencies and opens the selected Product for `linux_x86_64`. The driver needs the published Psi, matched runtime provisions and native image for that Product so it can run the sample with its allowed host services and compare the native execution. These samples provide the authored programs and their package Builds.

The spectrometer's `--selection-identity` assertion additionally needs a checked product exposing the finalized equality selection occurrences. The Pareto driver's source-free schema identity regressions run with their owning `provider-plans` crate.

[Build execution and package review](../../../wiki/spec/build/execution.md) · [Project Build interface](../../../source-rust/subsystems/compiler/build/project_build.rs) · [Compiler requests and products](../../../source-rust/subsystems/compiler/compiler.rs)
