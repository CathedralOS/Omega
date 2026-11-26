# Program baseline at `bafc9420b0f4` (linux_x86_64)

At `main` revision `bafc9420b0f4`, the source check accepts 1,947 of the 5,210
programs in the repository that are meant to compile (37.4%) for
`linux_x86_64`. The unpublished baseline at `f76875c4106c`, two days earlier,
counted 1,817 of 5,165 (35.2%). This is the first whole-compiler count measured
at two revisions. Only one target was swept, and a pass here means the program
checked, not that it was built or run.

| Result | `f76875c4106c` (2026-11-23) | `bafc9420b0f4` (2026-11-25) |
| --- | ---: | ---: |
| Should compile, and the source check accepts it | 1,817 | 1,947 |
| Should compile, but the compiler stops it | 3,348 | 3,263 |
| Should be refused, and is refused with the expected error | 1,192 | 1,302 |
| Should be refused, but fails with a different error | 673 | 770 |
| Should be refused, but stops earlier on an unsupported construct | 2,177 | 2,104 |
| Should be refused, but the source check accepts it | 113 | 1 |
| Programs swept | 9,320 | 9,387 |

The sweep infers "should compile" or "should be refused" from each program's
directory (`fail/`, `invalid_*` and similar paths mean refuse). The earlier
audit found that this inference is stale for some fixtures, so a single row can
be wrong in either direction.

## What changed between the two revisions

Of the 9,309 programs present at both revisions, 507 changed result:

- 152 programs the compiler used to stop now pass the source check, and 46 of
  the 78 programs added since `f76875c4106c` pass it too.
- 112 of the 113 negatives the source check used to accept are now refused.
  Only 38 of those reach their expected error; 69 now stop earlier on a
  construct the compiler does not support yet. The one negative still accepted
  is `source-rust/application/tests/fixtures/providers/fail/registered_test_host_binding/main.omg`.
- 63 programs that passed the source check are now stopped. Most of them stop
  in Build and provider handling:

| Programs | New first error | Example |
| ---: | --- | --- |
| 12 | this call has no visible declaration in its source scope | `samples/cli/facts/independent_call_result_bound` |
| 12 | Build import has no supplied source | `samples/cli/probes/build_depend_unaliased` |
| 9 | boundary requirement has no provider on the target | `samples/cli/facts/foreign_indirect_float_record_result/call_result` |
| 9 | target has no unique checked entry contract | `samples/cli/probes/build_dup_product_edge` |
| 7 | not supported yet: whole-trait supply with parameters or refinements | `samples/cli/facts/build_target_provider/call_result` |
| 14 | five other causes | |

Nobody has yet checked whether these 63 are regressions or stricter checks the
specification requires.

## Where programs stop now

The compiler stops 5,367 programs before a verdict. The five most common first
errors are:

| Programs | First error |
| ---: | --- |
| 460 | a data declaration cannot own a machine; declare an ordinary static machine |
| 271 | this call has no visible declaration in its source scope |
| 218 | import does not select one declaration, callable family, or module in its checked package |
| 175 | not supported yet: write-only reference parameter |
| 143 | builtin operator takes unqualified operands only |

By the sweep's stage label, 2,240 stop on checker coverage, 2,146 are labelled
`Other`, 520 stop in typing, 264 in name resolution, 158 on a checker
verdict, 38 in product routing and 1 in package roots.

## Limits

- **One target.** `linux_arm64`, `macos_arm64`, `windows_x86_64` and
  `windows_arm64` were not swept at this revision.
- **Source check only.** The sweep runs the source check for each program. It
  does not compile native code, run Psi, or compare native/Psi agreement.
- **Directory expectations.** See the note under the first table.

## How it was run

`mbx xtask sweep fleet start --target linux_x86_64 --commit bafc9420b0f46c561a5dc592016f766e3e86ee1e`
split the 9,387 programs into twelve units across four hosts. The
workstation's job died after 320 programs without a failure status (cause not
found), and a restart reached 400 before its three units were moved. They ran
with `xtask sweep --target linux_x86_64 --shard <K>/12` on the other hosts,
from each host's fleet checkout and build at the same revision:

| Host | Units | Jobs at once | Programs |
| --- | --- | ---: | ---: |
| `omega-linux` (Linux x86-64 VM) | 1, 4–5 | 2 | 2,347 |
| `omega-win` (Windows x86-64 VM) | 2, 6–7 | 2 | 2,347 |
| Mac (Darwin arm64) | 3, 8–12 | 3 | 4,693 |

`mbx xtask sweep merge` combined the six shard files and confirmed exactly one
row per selected program. [The merged results](sweep-linux_x86_64-bafc9420b0f4.zip)
(`sweep-linux_x86_64.json`, SHA-256
`f8ca1e9bd112ca5bf9edd33d12839a6def0e42957b50d9dce6a8c48319d93f05`) can be
passed to `mbx xtask sweep --changed <rev> --previous` for later comparisons.
The earlier `f76875c4106c` sweep, the shard files, run receipts and the
comparison scripts are kept on the owner's workstation under
`~/omega-agent/baselines/`.
