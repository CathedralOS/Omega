# NEW077 validation record

The complete Omega entrypoint passed source checking, native emission, all nine
valid fixtures and ten runtime-invalid fixtures. Every native output byte and
integer audit matched the independently frozen oracle (tolerance zero). Terminal
termination agreed for all 19 inputs before integration and for the maximum,
vertical-hole and extra-digit controls after integration. CLI comparison does
not compare Terminal stdout; native stdout was checked independently.

## Executor and identities

Linux x86_64, UID 1000, five exposed CPUs, approximately 18 GB RAM, no swap;
pinned nightly-2026-09-04. Original `/workspace/Omega`, branch `work`, HEAD
`03e477fa424a9e6a2db7f49e17bf896ab3f76b48` was preserved. Work occurred in
`/workspace/Omega/.codex/worktrees/new077`, local branch `sample/new077`.
One Rust build and one Omega compilation/execution ran at a time. Rust used four
jobs, leaving one exposed CPU for an independent Omega execution when concurrent.

The final optimized dev compiler was built at `57bf5322756fadbf195d7e1b97da39baf923b95a`,
integrating main `815d0508b03c7f47732da2a6bff7d957b9bd8fef` and the bounded repair.
A sample-only rebase onto main `71d6144cc88ba852d1dbd17327295d2eb1f0ffee`
produced validation tree `7223309ebb5ba056dea50081986116ff49ec853a`.
Compiler/library inputs are identical across that rebase: `omega-rust` tree
`122d9ae87f2810f502f51abb87982494c0b2081b`, `source/library` tree
`e172a07ddff369f5822b89f362d2e548c1bf8191`. Successful results are reused only
for unchanged inputs. The final queue receipt and publication identity are in
the supported review bundle.

Actual compiler build command (exit 0, 2m40s):

```sh
source /workspace/cloud-setup/activate.sh
CARGO_TARGET_DIR=/workspace/new077-current-target CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_BUILD_JOBS=4 mbx build -p omega
```

Compiler `/workspace/new077-current-target/debug/omega` SHA-256:
`7e58839bba9d9e75b93d93446e3bdee2b40b9d52594de755a97ab477b8b590b2`.
Native `build/integrated-native/omega-program` SHA-256:
`fceb626ed972980dc069185a4915296a81fac20bda929008ce7e722641bc5d26`.
The native program is byte-identical to the earlier repaired-base artifact.

## Preparation and source controls

All compiler invocations selected this checkout's stable library through
`OMEGA_LIBRARY_ROOT=/workspace/Omega/.codex/worktrees/new077/source/library`.
The normal sandbox exposed synthetic root ownership 65534 and the cache custody
guard rejected it. Approved per-command `require_escalated` executions retained
UID 1000 and real ownership. No ownership, custody guard or admission fence was
changed. No package acceptance action was denied, and none was retried after denial.

Ordinary preparation produced 285 pending decisions. Primary and independent
reviewer inspected the full inventory; exact pending review SHA-256:
`2b18ba72504f9999514fd9130673e5c5bba8a5dd4b49bc8337c43af43de99576`.
These comprised three root Console terminal permissions, 140 unchanged std
callable claims, 140 unchanged supplied externs, and two std dangerous-authority
classifications. Hash-bound edits changed only those reviewed decisions;
ordinary `update --resume` published the accepted lock. No accept-all flag,
root filesystem/network grant, extra dependency or assumed binding was used.
Subsequent compiler checkpoint updates reported zero source changes and retained
accepted policy; the integrated update exited 0. The bundle retains the review,
inventory, accepted lock and command logs. A consuming executor must inspect its
own fresh pending review rather than blindly accept different review bytes.

Primary corrected source authoring errors in dependency path, state-loop control,
blocking acknowledgements and mutable forwarding argument order. Oracles were
not relaxed. One preparation failed because the primary edited documentation
during its immutable snapshot; the entire app tree was then held fixed during
preparation and execution. These were authoring/workflow errors, not compiler gaps.

`omega --check main.omg` exited 0. The separate complete
`controls/out_of_bounds/main.omg` check exited 1 with exactly one diagnostic:
`cannot prove index __hoist_14 is within length 256` (identifier quoted by CLI).
Index 256 into a length-256 owner is the intended bounds violation; unrelated
package, parser, type, binding or provider rejection did not count as success.

## Witnessed compiler repair

Before editing compiler code, the whole unchanged application reached native
production on current main `a5d20d6a7faeaf56acabe903a4e78275760d0cee` using
compiler SHA-256 `24c9579d1cf78e94c52d8bc1330054594429e345773b91f1f06ddff876aa8d16`.
Native emission exited 1 with:

```text
RegisterAllocation(RuntimeSpill(Homes(NoCommonCandidate { function: 13, register: 1 })))
```

A bounded diagnostic trace located the renderer's original input pinned across
several destructive division/remainder uses and hosted byte writes. Function 13
was unchanged by the optional crossing probe; treating that probe as the cause
would have been incorrect. All temporary tracing was removed.

The repair in stage03 `scalar_graph/integer_arithmetic.rs` reads the selected
row's fixed-use and early-clobber constraints. It transports each pinned input
occurrence to a fresh short-lived register without changing the original source
value's mapping. Flexible equal inputs are separated where early clobber requires
it. Copies retain type/value/definition lineage; arithmetic obligations and fuel
remain on the arithmetic instruction. This replaces the prior source-equality
recognizers for division, multiplication and shifts. No home-assignment rejection,
proof rule, provider authority or runtime output check was weakened. Incoming
main's separate stage05 recovery changes were preserved without editing them.
Reviewed initial repair SHA-256:
`428714a51e0597fdd203923ff0a5aa081357c181e1b9967f70d911b5134ed487`.

The later integration on main `592fcca655158cbccbd93b30a68067eb3bcfcccf`
preserves four incoming x86 result-copy hunks alongside this input transport.
Independent review confirmed these are the only changes from the initially
reviewed file. The merged source SHA-256 is
`88e387b800db0a7e4bef2e5301f4f2669c82ec2008217914e93ae2bc5b5753e9`.

## Execution evidence and regression gates

`ORACLE.json` SHA-256 remains
`d4f566be2f960fa708f16d9cd039950c65c4c890969d258e3245ddd8d1950a6e`.
The exact rectangular-set oracle, fixture hashes, emitted-cell enumeration,
continuity/bounds, deposition/travel lengths and independent audits all passed.
The maximum covers 256 cells with deposition 256 and travel 300. Stride-two
undercoverage reports covered 8, missed 4 for a 12-cell region. Holes/islands
produce travel-only gaps; every valid fixture reports zero waste and duplicates.
Every invalid input emits only `ERROR input` and exits 64.

Actual integrated commands (repository root; the recorded compiler on PATH):

```sh
omega update --project samples/cli/independent/layered_toolpath_coverage --target linux_x86_64
omega --check samples/cli/independent/layered_toolpath_coverage/main.omg
omega --target linux_x86_64 --build-dir samples/cli/independent/layered_toolpath_coverage/build/integrated-native samples/cli/independent/layered_toolpath_coverage/main.omg
python3 samples/cli/independent/layered_toolpath_coverage/reference.py --cli /workspace/Omega/.codex/worktrees/new077/samples/cli/independent/layered_toolpath_coverage/build/integrated-native/omega-program --executable
omega run --both samples/cli/independent/layered_toolpath_coverage/main.omg < samples/cli/independent/layered_toolpath_coverage/data/maximum.in
omega run --both samples/cli/independent/layered_toolpath_coverage/main.omg < samples/cli/independent/layered_toolpath_coverage/data/vertical_hole.in
omega run --both samples/cli/independent/layered_toolpath_coverage/main.omg < samples/cli/independent/layered_toolpath_coverage/data/invalid_extra_digit.in
omega --check samples/cli/independent/layered_toolpath_coverage/controls/out_of_bounds/main.omg
```

The last command intentionally exits 1; malformed runtime input exits 64.
`build/native`, `build/both`, and `build/integrated-terminal` retain stdout,
stderr and machine-readable results. The review bundle preserves these ignored
artifacts and both pre-integration and integrated logs.

Final documentation edits changed the package's whole-source identity. A fresh
check correctly rejected the stale accepted checkpoint and requested ordinary
`omega update`; this was a source-freshness guard, not a denied acceptance action.
The bundle retains the documentation refresh and post-refresh check separately.

The new complete corpus fixture
`pass/arithmetic/reused_fixed_integer_operands_exit` derives its runtime input
from Console EOF, explicitly mapping ByteRead::Eof to -1. Before execution its
exact expected values were frozen at 4690, 111, 91, 10 and 30. Exit 0 requires all
five arithmetic/reuse checks. Independent source review corrected a ByteRead type
error without changing those expectations.

The already-built corpus runner used `OMEGA_CORPUS_JOBS=1`. Nine selected native
and Terminal fixtures built and agreed, with three expected exit-0 and six
expected exit-70 outcomes, zero lost successes and zero oracle failures. They
cover reused/equal operands, pressure, saturating/exact shifts, bounded remainder,
signed MIN/-1, vector growth and nested calls. A separate checking gate verified
the new fixture and the existing divide-by-zero rejection (`division by zero`
fragment satisfied). Native/Terminal mode excludes fail fixtures, hence the
separate rejection run. Initial gate exits 1 reported only absent golden rows;
reviewed successful observations were then recorded in scoped goldens. No failure
was blessed. Logs, exact filters and verdict JSON are retained in the bundle.
The recording helper sorted and deduplicated unrelated historical rows. Those
unselected byte blocks were preserved instead, and all winning record facts were
compared equal to the helper's output. Only selected observations and the current
header context changed; historical appended contexts were retained.

Rust formatting check passed. The affected package and omega dependency-closure
check passed through `tools/landing.py verify`. No release-wide or cross-platform
qualification is claimed; execution was Linux x86_64. Claims registry mutations
failed with Git (-9); successful reads showed the retained ticket absent. This
was advisory coordination failure, not permission denial or known scope conflict.
The original unique assignment remained locally recorded. Publication uses only
the normal main-only landing queue, with no remote worker branch.

Landing diagnostics isolated Git (-9) to fetching the independent coordination
history; kernel OOM-kill counters increased during those attempts. The stable
`tools/coordination.py` reachability-cache helper retained an already verified
coordination commit as a local negotiation hint. The next normal status fetch
completed in under a second, and normal claim/release calls succeeded. This
local hint changes no remote reservation, admission, custody or publication
guard. The bundle retains commands and trace evidence. These source records
describe pre-publication checkpoints; the final bundle records the actual
publication and Library-transfer outcome separately.
