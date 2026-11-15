# Package inspection

`omega packages audit [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...`
shows the project graph, exact source pins, and the compiler's API and
authority findings beside the lock's accepted policy.
It does not accept changes, certify an audit or publish native artifacts.

```text
inspect_packages.rs  transaction, target admission, policy/report joins and completion
inspect_packages/
├── sources.rs       executed source custody, review and comparison
├── report.rs        bounded graph and policy presentation
└── report/
    ├── policy.rs        typed policy summary and full-detail delegation
    └── policy_callables.rs  callable reach, invocation, and checked behavior
```

Application Build's [`package_inspection.rs`](../../../../../build-system/orchestration/src/package/package_inspection.rs)
creates and disposes of the session, runs shared source Builds and loops over
targets in order. Each selected source Build runs once, before any target's
review; review passes read its kept result and request no new execution or
grants. It uses the same source resolver and review
executor as install/update. Inspection finishes every admitted target in
order and rechecks the original project files before returning the report. A
failed target reports unavailable findings without hiding later targets; a
reporting failure stops further work.

With a lock, dependency requests and content stay pinned. Git may fetch a
missing exact accepted commit but never refreshes its branch or tag. The
project root stays editable, as in ordinary compilation. Without a lock,
inspection resolves the written graph and checks it as unaccepted source; it
does not create a lock.

`--build-input <path>` and `--optional-build-input <path>` choose the root's
fixed input files, with compilation's canonical relative paths and capture
checks. A directory includes its subtree; include every
root source file that checking reads. Repeat a flag for each entry. The set
applies to every requested target and both binding-discovery and final
review passes; dependencies keep their own package inputs. A missing required
entry reports unavailable analysis, not an empty policy or a wider fallback.
Give audit and compilation the same set. Inspection still cannot accept a
policy change, supply restricted host grants or publish staged outputs.

`--offline` uses cached exact accepted Git pins and allows local source work.
Missing cached content reports fresh analysis unavailable, without network
access or lock changes. Without a lock, any Git request needs
an online resolution first; a cached branch name is not an accepted exact pin.

Without `--target`, inspection covers every accepted target, or the host
target without a lock. A requested target that an existing lock lacks
is rejected before acquisition; review a new target with
`omega packages update --target <name>`.

Default output summarizes callable reach, authority and assumptions.
`--details` adds the complete fresh normalized policy. The lock stores only
compact risk-bearing acceptance rows, not a past API or representation
snapshot. The comparison checks those rows against the fresh report's risk
rows, not the whole report. Accepted policy keeps its recorded meaning even
when source cannot be refetched or checked; such a target reports `fresh-analysis unavailable`, not
empty capabilities. Different packages with the same name keep their
source-qualified identities, graph edges and aliases. Fresh reports include
root-to-package dependency paths.

Exit status:

- `0`: selected targets checked; no policy decisions are required.
- `1`: inspection or fresh checking unavailable.
- `64`: invalid command arguments.
- `3`: checking completed, but policy changes require review through install/update.

Inspection holds the project transaction lock while it loads and checks
accepted state. An unfinished publication must be recovered by an ordinary
package operation first; inspection never completes that write. Pending review
proposals and editable review files are left alone. Builds still run through
the scoped evaluator and may write throwaway diagnostics in private staging
under `build/package-manager/`, deleted on return; inspection is not
parse-only. The full multi-target report is capped at 64 MiB,
and strings taken from source stay escaped data.
