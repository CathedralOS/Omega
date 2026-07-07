# Nested-value-call transition guard reads pre-store ZII (native)

`main.omg` is `Duration::saturating_subtract` on a forward pair
(`{5s,400M} - {2s,100M}`): expected exit 70 (`{3s,300M}`), native exits 1
(result is `Duration::ZERO`). The interpreter is correct.

Mechanism: `saturating_subtract` is inlined into Main; its body calls the
sibling `checked_duration_since`/`checked_subtract` and TRANSITIONS on the
case-enum result. The transition guard is scheduled BEFORE the nested
callee's spliced result store, so it reads the result slot's ZII zero — tag
0 = `Overflow` — and takes the saturate-zero arm unconditionally. #2B splice
family; generalizes the fs `stat_rc` guard-ordering field note
(std/filesystem.omg) from host calls to ANY nested value call.

Promote to `canaries/pass/calls/` once the deferral machinery splices
guard-position nested-call result stores before the wrapper's guard
evaluation (filed in TASKS.md "Open latent bugs"). Then ALSO add non-ZII-arm
assertions to time/runtime_duration_core_exit — its current saturating and
ordering pins coincide with the ZII tags and pass over this bug.
