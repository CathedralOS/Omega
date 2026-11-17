# Service correspondence measurements

These drivers measure the check that ties each provider-interface method to
its original declaration, called source correspondence. They parse ordinary
declarations, prepare their service interfaces, warm the identity interner,
and time validation alone.
Parsing and projection are outside that interval. This does not measure whole
compilation or Console execution.

The one-method control represents a single lookup. The two-method program is
the unchanged `target_selected_implementation` fixture. The five-method input
uses the actual `ByteRead`, `LineReadResult`, and `Console` declaration block
from `source/library/std/console.omg`. Generated 16- and 64-method services
split their methods between a parent and child to check scaling; they are not
claims about the typical service size.
Each input is projected in an isolated source package; the Console input does
not load the standard library's complete dependency graph.

Build both drivers through mbx, in an isolated worktree with the host build
budget from `tools/docs/build_slot.md` and the same compiler profile for both
revisions:

```sh
mbx build -p terminal-psi-to-terminal-psi --example service_correspondence --example service_correspondence_allocations
mbx test -p terminal-psi-to-terminal-psi --example service_correspondence
```

Save copies of the two executables before changing the production validator,
then rebuild and save the candidate executables. Cargo places the drivers in
`target/debug/examples/`; Windows adds `.exe`. The timing driver's allocator
forwards directly to `System`: its counter updates are removed at compile
time. The other driver counts allocation requests, requested bytes and peak
extra live requested heap separately. Do not use that driver's timing column
as an uninstrumented latency measurement.

Compare the four saved files with the Rust repository tool:

```sh
mbx xtask measure-service-correspondence --before BEFORE --after AFTER --allocations-before ALLOCATIONS_BEFORE --allocations-after ALLOCATIONS_AFTER --pairs 9 --output comparison.json
```

The comparison alternates which revision runs first within each adjacent pair.
Its JSON retains every sample, binary hashes, allocation counters and summaries.
The existing bounded-process runner also records kernel resource observations,
including peak RSS when available. The receipt retains the provider, scope and
any observation error. Peak RSS includes parsing and setup and is not the
validator's heap size. Drivers run with closed stdin, a 300-second wall limit,
120-second CPU limit, 4 GiB memory/address-space limits, and an eight-process
ceiling. These are measurement-driver limits, not compiler product policy.

Build `xtask` before timing, then reserve one CPU permit around the built
`target/debug/xtask` command (Windows adds `.exe`). This excludes Cargo work
from the measurement interval and accounts for the single-threaded driver work
in the shared host budget.

The Rust drivers also accept `--only LABEL`, `--samples N`, and `--iterations N`.
The tests exercise missing source coordinates, package mismatches, ordered-row
loss, equal leaf names, applied parents, duplicate canonical requirements, and
ambiguous inherited declarations through the real parser, typer and public
projection API.
