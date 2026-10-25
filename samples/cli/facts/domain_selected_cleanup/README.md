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

This fact is unfinished. Parsing and retained source-tree selection are a local
implementation checkpoint, not runtime acceptance. Cleanup selection, exact
application substitution, inherited/default/alias/conjoined selection, obligation
transport and erasure, authority, hook effects, and Terminal/native execution
must all be demonstrated. Only the initial positive currently exists. The nine
transport places and sensitive reject twins are not yet established or pinned.
No parent-task acceptance has been subtracted.
