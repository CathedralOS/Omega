# Measuring a compiler change

Use `omega-bench compare <plan.json> <new-directory>` to compare two executables on one declared program. A compiler measurement includes process startup, compilation and publication. The runner executes each native output separately and checks its exit status, stdout and stderr against the plan.

Set `measurement.stage_timings` to `false` for an uninstrumented compiler comparison. Omitting the field preserves stage timing collection in existing plans. To benchmark the Build entry point, put `"build"` first in each command’s `arguments`. The plan supplies the source, library, target and output directory.

Run `omega-bench profile <plan.json> candidate stages <new-directory>` on the same input to locate the work. Open `stages.html` in the result directory for the command timeline, gaps and stage durations. This diagnostic run enables `--timings`, runs the native oracle and saves the following evidence in `profile.json`:

- `sample.compiler_stage_ms` gives the measured time by stage, including the input and output forms. The captured `subject.stderr` also gives each phase’s measurement count.
- `timeline.spans` shows when each command phase began and ended. `covered_ms` counts overlapping intervals once. `gaps` lists unassigned intervals within the compiler’s own elapsed time.
- The subject process observation includes startup and exit. The profile retains the input and executable identities so a separate comparison can use the same workload.

Set `measurement.cold_client_root` to the common ancestor of the working directory, declared inputs and library for a cold comparison. Each warmup, measured sample and stage profile copies the declared inputs and library into a fresh project, preserving their relative paths. Include every source, package and authored lock the project needs. The runner gives that client its own application cache, temporary and output directories before starting the compiler timer. The OS may still retain filesystem pages. Record concurrent host work when reporting wall time. Stage durations can include nested or parallel work.
