# Canary corpus distribution

Measured census of the `tests/omega/{pass,fail}` fixture inventory on
linux x86-64 — how the authored corpus divides across capability groups and
outcome suffixes. This is a snapshot record: the corpus changes per landing,
so the counts carry the revision they were measured at. Refresh by re-running
the enumeration below on a later HEAD and replacing the tables wholesale —
do not append dated copies of the same table.

Revision: `bb192d7ea9eb` (origin/main at measurement time, 2026-09-21).
Host: linux x86-64. Method: directory enumeration of
`tests/omega/pass/<group>/<fixture>/` and `tests/omega/fail/<group>/<fixture>/`
(each fixture is a directory holding `main.omg`; suites may also carry
`build.omg`, `expected.txt`, or `README.md`).

```bash
python3 - <<'PY'
import os
for corpus in ("pass", "fail"):
    root = f"tests/omega/{corpus}"
    groups = {
        g: sum(
            1
            for e in os.listdir(os.path.join(root, g))
            if os.path.isdir(os.path.join(root, g, e))
        )
        for g in sorted(os.listdir(root))
        if os.path.isdir(os.path.join(root, g))
    }
    print(corpus, sum(groups.values()), groups)
PY
```

## Outcome-suffix split

Suffix conventions are informative, not enforced: `*_exit` fixtures are
runtime canaries that assert a process exit code, `*_compile` and `*_warns`
fixtures are compile-stage canaries, `*_traps` fixtures assert a trapping
outcome, and unsuffixed names are checked pass/fail members whose mode the
owning suite declares.

| Corpus | `*_exit` | `*_compile` | `*_warns` | `*_traps` | unsuffixed | total |
|--------|----------|-------------|-----------|-----------|------------|-------|
| pass   | 996      | 215         | 0         | 22        | 809        | 2042  |
| fail   | 1        | 6           | 0         | 0         | 1228       | 1235  |

## Per-group distribution

| Group | pass | fail |
|-------|------|------|
| arithmetic | 162 | 84 |
| atomics | 11 | 7 |
| backend | 2 | 0 |
| blockexec | 4 | 2 |
| borrow | 24 | 37 |
| borrows | 13 | 13 |
| boundary | 0 | 7 |
| build | 11 | 17 |
| calls | 190 | 35 |
| capabilities | 34 | 15 |
| collections | 99 | 14 |
| comptime | 5 | 6 |
| concurrency | 3 | 8 |
| constants | 6 | 5 |
| constraints | 16 | 19 |
| contracts | 1 | 3 |
| control_flow | 72 | 25 |
| core | 34 | 46 |
| data | 26 | 45 |
| dependent | 87 | 78 |
| domains | 92 | 72 |
| drops | 12 | 4 |
| dungeon | 19 | 0 |
| effects | 5 | 3 |
| entry | 1 | 1 |
| errors | 4 | 0 |
| expressions | 75 | 36 |
| ffi | 0 | 1 |
| filesystem | 86 | 0 |
| float | 51 | 10 |
| generics | 60 | 77 |
| host | 23 | 2 |
| inline_asm | 16 | 32 |
| layouts | 23 | 5 |
| memory | 6 | 13 |
| modules | 32 | 22 |
| objc | 18 | 0 |
| operators | 23 | 14 |
| optimizer | 2 | 1 |
| ownership | 23 | 13 |
| parameters | 4 | 0 |
| parse | 0 | 2 |
| parser | 2 | 1 |
| platform | 0 | 1 |
| ports | 0 | 1 |
| progress | 1 | 0 |
| proofs | 61 | 119 |
| providers | 36 | 32 |
| range | 6 | 14 |
| ranges | 3 | 12 |
| recast | 24 | 31 |
| references | 3 | 0 |
| relax | 0 | 2 |
| relevance | 5 | 10 |
| rewards | 3 | 0 |
| slices | 111 | 38 |
| storage | 15 | 0 |
| structural | 1 | 0 |
| structs | 13 | 0 |
| targets | 48 | 2 |
| tasks | 1 | 3 |
| terminal_psi | 10 | 0 |
| termination | 114 | 117 |
| text | 82 | 3 |
| time | 17 | 1 |
| traits | 46 | 36 |
| types | 8 | 4 |
| versioning | 9 | 1 |
| wire | 48 | 33 |
| **total** | **2042** | **1235** |

63 pass groups, 56 fail groups, 3277 fixtures total at this revision.

## Reading the distribution

- The heaviest pass groups (`calls` 190, `arithmetic` 162, `termination`
  114, `slices` 111, `collections` 99, `domains` 92) are where runtime and
  structural semantics concentrate; the heaviest fail groups
  (`termination` 117, `proofs` 119, `arithmetic` 84, `dependent` 78,
  `generics` 77) mirror the checker/admission surfaces that reject programs.
- Groups present in only one corpus (e.g. `filesystem` 86/0, `dungeon`
  19/0, `ffi` 0/1, `ports` 0/1) are asymmetric by surface, not by gap —
  a capability with no authored failure mode has nothing to reject.
- Suite registration is the real inventory check: `roster.rs`'s
  `registered_pass_canaries_have_source_on_every_host` /
  `registered_fail_canaries_have_source_and_their_owned_expectations`
  (CompleteCorpus scope) fail on any fixture directory missing from the
  roster lists in `canary_suite.rs` / `fixture_rosters/`, so this table and
  the registered roster are guaranteed to agree at a green HEAD.
