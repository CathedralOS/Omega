# Current-main measurements, 2026-10-29

The reference producer was freshly built from main at
`8badab9ee404808dbc2024bbc3534f4d08de8e49` using
`mbx build --locked -p omega --release --message-format=json` in an isolated
worktree. No compiler or workload source was changed. The build took 423.7 s;
that is producer construction, not a program compile-time measurement.

[Producer identity](producer.json) and the [effective-profile receipt](compiler-profile.json)
bind the executable hash and opt-level 3 to the Cargo artifacts and flags.
Measurements use Windows x86-64 on an AMD Ryzen 9 7950X (32 logical CPUs).
Other work was active on this shared machine. File-cache state is uncontrolled.

The normal recorder measures each compile in a fresh output directory. Package
review preparation precedes timing. Three compile samples and nine native runs
are requested. Runtime includes process startup. The packet workload checks its
checksum, repeatability, empty input and neighboring storage before exiting.
The square-sum accumulator is discarded, so that subject is compile-only.
The empty program is also compile-only, as documented by its author.

## Replay

Use the producer from `producer.json` (or build and identify a new one), then:

```text
python tools/benchmark/benchmark.py measure --root samples/cli/basics/standalone/main.omg --target windows_x86_64 --omega <producer> --compile-samples 3 --no-run
python tools/benchmark/benchmark.py measure --root samples/cli/arithmetic/wrapping_square_sum/main.omg --target windows_x86_64 --omega <producer> --compile-samples 3 --no-run
python tools/benchmark/benchmark.py measure --root samples/cli/proofs/structural_proofs/main.omg --target windows_x86_64 --omega <producer> --compile-samples 3 --no-run
python tools/benchmark/benchmark.py measure --root samples/cli/collections/segmented_packet_checksum/main.omg --target windows_x86_64 --omega <producer> --compile-samples 3 --run-samples 9 --expected-exit 0
python tools/benchmark/benchmark.py measure --root samples/cli/arithmetic/prime_counter/main.omg --target windows_x86_64 --omega <producer> --compile-samples 3 --run-samples 9 --expected-exit 8
```

Run the build/measurement chain under one `tools/build_slot.py` hold. Generated
`omega.lock` files remain host-local. Normal JSON records retain every measured
sample, selection receipt, binary hash, source revision, lock identity and exit.
The dashboard derives its project LOC from that source revision; it excludes
dependencies outside each project directory. No full-source throughput or
cross-host speedup is established by these observations.

## Rejected attempts

`structural_proofs` failed package checking with four diagnostics concerning
unjudged proof-only `Nat`/`Seq` claims. Its [raw output](structural_proofs.log)
is retained. No successful compile or runtime timing is claimed for it.

`prime_counter` did not finish its initial package-review preparation within
twenty minutes. Its owned `omega update` process was stopped at that deadline
(exit 124). The [deadline observation](prime_counter-timeout.json) and
[recorder output](prime_counter.log) are retained. This establishes preparation
latency greater than or equal to twenty minutes on this run, not a completed
compile time. No timed compile or native run was reached for that subject.

The initial square-sum attempt exposed a recorder bug: its authored-selection
scan only recognized historical method syntax. The recorder now reads canonical
`Optimizations::enable(&mut builder.optimizations, ...)` calls too. Selection
still must match the compiler's rollback receipt before a row can be published.
The [original refusal](wrapping_square_sum-initial.log) remains available.

## Measured results

| Workload | Compile median (range), ms | Native median, ms | Compile / native samples |
| --- | ---: | ---: | ---: |
| segmented_packet_checksum | 483.861 (477.607–1579.100) | 62.709 | 3 / 9 |
| standalone | 628.277 (559.044–1861.676) | Not run | 3 / 0 |
| wrapping_square_sum | 741.374 (637.834–1751.180) | Not run | 3 / 0 |

These are absolute observations, not a matched speedup. No sustained-runtime
throughput or other-host performance was measured. Failed attempts above have
no successful timing records. The newest batch warnings remain visible above
the dashboard charts; later measurement batches should supply their own warnings.
