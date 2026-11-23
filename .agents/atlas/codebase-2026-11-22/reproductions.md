# Replaying three Atlas findings

These are the regression tests kept from the November 23 follow-up to the November 22 survey. Run them from an Omega checkout that includes the linked fixes.

## Generated infinity text

The compiler's numeric literal formatter emits `inf` and `-inf`. Its reader used to strip the final `f`, preventing its own text from round-tripping. The fixture checks both signs, explicit float formats and a finite neighbor. The second command passes through the syntax tree that consumes the generated text. [Atlas unit 005](records/unit_005.md) · [Fix #5239](https://github.com/CathedralOS/Omega/pull/5239)

```sh
mbx test --locked -p numeric-literals --lib formatted_infinities_round_trip_through_literal_text
mbx test --locked -p syntax-trees --lib generated_infinity_spelling_keeps_its_sign_and_landing
```

## Changed-program selection through a child package

An app depends on a library, the library names a child package `vendor`, and the child depends on a shared package outside the library. Changing only the shared package used to select zero programs. The public `sweep --changed` test expects exactly the app and excludes an independent sample. The traversal test compares `vendor` with `./vendor` and includes a dependency cycle. [Atlas tools digest](digests/16-tools.md) · [Fix #5242](https://github.com/CathedralOS/Omega/pull/5242)

```sh
mbx test --locked -p xtask --bin xtask dependencies_are_followed_to_the_end
mbx test --locked -p xtask --test repository_root changed_sweep_selects_the_consumer_of_a_child_packages_dependency
```

## Repeated build output directory

Supplying two `--build-dir` options used to produce an executable in the second directory. The CLI now reports a usage error (exit 64) before creating output. The control supplies one build directory. [Atlas CLI digest](digests/01-cli-foundation.md) · [Fix #5245](https://github.com/CathedralOS/Omega/pull/5245)

```sh
mbx test --locked -p omega --bin omega --test build_output_directory build_dir
```

## Observed turnaround on Windows

| Case | Investigation start to first failure | First production edit complete to first passing check |
| --- | ---: | ---: |
| Generated infinity text | 3m 18s | 2.76s |
| Changed-program selection | 4m 06s | 23.79s |
| Repeated build output directory | 2m 16s | 1m 38.74s |

The sweep and CLI first failures used existing executables. Each failing case was also checked against a build from the starting source revision. The CLI also reproduced 19m 57s after investigation began when built from the pinned source revision, including 18m 36s spent building. Post-edit times cover the focused check and any rebuild it triggered. Later integration runs, review and merge time are outside those intervals. These are individual observations from concurrent work on one host.
