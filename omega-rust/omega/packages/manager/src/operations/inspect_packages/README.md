# Package inspection

`omega audit packages [--project <dir>] [--target <name>]... [--details]`
shows the project graph, exact source pins, and compiler-derived API/authority
findings beside accepted lock policy. It does not accept changes, certify an
audit, or publish native artifacts.

```text
inspect_packages.rs  public options, result, and storage entrances
inspect_packages/
├── execution.rs     load accepted state, resolve pinned sources, check, compare
├── report.rs        bounded graph and policy presentation
└── report/
    ├── policy.rs        typed policy summary and full-detail delegation
    ├── policy/callables.rs  callable reach, invocation, and checked behavior
    └── tests.rs         identity, history, unknown analysis, and output limits
```

With a lock, dependency requests and content stay pinned. Git acquisition may
fetch an absent exact accepted commit, never refresh its branch or tag. The
project root remains editable as in ordinary compilation. Without a lock,
inspection resolves the authored graph and checks it as unaccepted source;
it does not create a lock.

Omitting `--target` inspects every accepted target, or the host target when
there is no lock. An explicitly requested target missing from an existing lock
rejects before acquisition; `omega update --target <name>` owns fresh target
review.

Default output summarizes callable reach, authority, and assumptions. `--details`
includes the complete compiler-owned normalized policy. Equal accepted/fresh
policy is displayed once, with equality explicitly labeled. Accepted policy
remains historical meaning even when source cannot be reacquired or checked;
such a target reports `fresh-analysis unavailable`, not empty capabilities.
Different same-named packages retain their source-qualified identities, graph
edges, and aliases. Fresh reports include root-to-package dependency paths.

Exit status:

- `0`: selected targets checked; no policy decisions are required.
- `1`: inspection or fresh checking unavailable.
- `2`: invalid command arguments.
- `3`: checking completed, but policy changes require review through install/update.

Inspection holds the existing project transaction guard while loading and
checking accepted state. Pending publication intent must be recovered by an
ordinary package operation first; inspection never completes that write.
Pending review proposals and editable review files are left untouched. Builds
still execute through the scoped evaluator and may write disposable diagnostics
under `build/package-manager/audit-<target>/`. This is not a no-execution parser
mode. The complete multi-target report is bounded to 8 MiB and source-controlled
strings remain escaped data.
