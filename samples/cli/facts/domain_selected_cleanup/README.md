# Domain-selected cleanup

Owning requirement: `STATIC-MACHINE-MIGRATION`, domain-selected automatic cleanup
in `wiki/spec/language/domains.md#domain-selected-automatic-cleanup` and the
cleanup contract in `wiki/spec/language/ownership.md#consumers-and-cleanup`.

The owner-selected independent cleanup application and its prerequisites survive
owned transport; normal cleanup writes captured code 7 through the retained
exclusive output reference exactly once.

The first witness is [local/main.omg](local/main.omg). Its runtime observer traps
if cleanup fails to write to the original Counter. `build.omg` binds all four
hosted entries, and the authored exit is 0. The first current-main observation is
from `e06334c63dc4a811b0bcf9f530cbc91b14b23eac`, built here with opt-level 1:

```
omega update --project samples/cli/facts/domain_selected_cleanup/local --target linux_x86_64
omega --check samples/cli/facts/domain_selected_cleanup/local/main.omg
```

Both commands reject line 6: `expected '{', found identifier 'dropped'`.
The producer's package-free environment smoke separately returned native 0 and
interpreter 0. The raw commands, producer digest, canonical packet, and logs are
retained in the lane's ignored `build/slot3-evidence` directory.

This fact is unfinished. Candidate 7, from local checkpoint
`ab4d7a0382f349cae4df6bc9f54d5e9e32107a89`, runs eight of the nine standard
places in both engines with exit 0, including an invocation counter asserting
exactly one cleanup. Named, repeated inherited, and conjoined identical selections
also execute once. Sensitive prerequisite twins reject; differing applications,
direct hook calls, crash contracts, and wrong receiver carriers reject.

Candidate 8 additionally executes the generic application in both engines with
exit 0 after structural static operand resolution was repaired. Its exact source
diff and producer digest are retained with the command evidence. A later candidate
is checking the generic prerequisite twin and debt-losing erasure controls.

The `other_state` place remains red: its true nested `machine` declaration is
not yet accepted by the parser. It is deliberately not rewritten into a legacy
`state`. General erased cleanup interface transport, lifetime and indexed
applications, all hook-effect/authority controls, product-producer parity,
legacy source retirement, pinned corpus fixtures and independent review remain
open. All nine standard place sources now exist; no parent acceptance is closed
or subtracted. Historical results are tied to their original producers and source.
