# Std/core machine checking, 2026-10-24

Std/core machine checking costs about 44 ms of wall time per compile, not the
228 ms that the [whole-compile profile](../resolution_boundary_2026_10_24/README.md)
reported. That 228 ms is thread time summed over parallel workers. Caching
std/core checks across compiles would remove at most those ~44 ms of a
~1.27 s compile, so no cache was built.

Source base `9e49ea3065f489b4ef9fa87b588571d1679b5604`, unmodified. Dev profile,
effective opt-level 1. Binary SHA-256:
`ab607845845d7734747cfae7b2f5babcffd00f9051d99d6ae921b539510c690f`. Host:
Windows 11, AMD64 Family 25 Model 97 Stepping 2, 32 logical CPUs. Workload
`samples/cli/facts/dyn_dispatch_selector_payload_bound/main.omg`, compiled in
place with `OMEGA_PHASE_PROFILE=1 omega --timings`. Three runs, no warmup;
every run published and exited 0.

## Why the row overstates

`validate` checks machines with
`checking_thread_pool().install(|| machines.par_iter()...)`
(`omega-rust/psi/compilation/checking/src/program_validation.rs`). Each
worker opens `05.d.2.a.2.machine.std_or_core` or `machine.user_or_other` for
one machine. `phase_profile` records wall time on the recording thread and
adds rows with the same name, so 153 std/core machines spread across workers
sum to ~200 ms of thread time. `05.d.2.a.2.machine_loop` encloses the parallel
loop on the calling thread and is the wall cost. It covers both checks and
both std/core and user machines.

| Run | std/core, summed | user, summed | Machine loop, wall | Stage 05 | Compile |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 228 ms | 2.3 ms | 47 ms | 968 ms | 1,274 ms |
| 2 | 197 ms | 1.5 ms | 39 ms | 953 ms | 1,261 ms |
| 3 | 217 ms | 1.5 ms | 44 ms | 1,026 ms | 1,350 ms |

153 std/core and 4 user machine entries per compile, across the Build check
and the product check.

## Stage 05 wall time

Rows entered once per check run on the calling thread, so they are wall
time. Medians of the three runs, both checks combined. Rows nest; do not add
them.

| Scope | Median |
| --- | ---: |
| `05.d.3.build_check_facts` | 454 ms |
| `05.d.3.b.flow_fixed_point` | 352 ms |
| `05.d.3.b.flow_fixed_point.build_flow_facts` | 271 ms |
| `05.d.7.check_checked_facts` | 144 ms |
| `05.d.2.prepare_typed_program` | 113 ms |
| `05.d.2.a.validate` | 110 ms |
| `05.d.2.a.2.machine_loop` | 44 ms |

The flow pass was closed in the
[saturation record](../flow_facts_2026_10_24/saturated.md).
`check_checked_facts`, at 144 ms, is the largest Stage 05 scope with no
attribution record yet.
