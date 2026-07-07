# Cross-callee let-name collision: the Mutation fallback write resolves the
# wrong callee's local (native)

`main.omg`: TWO different callees (`first_read`, `second_read`) with
SAME-NAMED lets (`freq`, `shifted`) value-called from ONE caller state.
Expected exit 70; native exits 1 — `self.a` (the FIRST call's result) is
clobbered.

Op-stream mechanism (backend_report on this repro): the leaf/call-result
path delivers call 1 CORRECTLY (result slot -> `self.a`), but the Mutation
op's own fallback storage-write then emits an EXTRA capture that resolves
the callee terminal `shifted` BY NAME through the cross-source-key fallback
ladder (`find_runtime_frame_slot_for_path`'s `.or_else` chain) and lands on
the SECOND callee's still-ZII `shifted` slot — `copy fr@40 -> Main.a`
overwrites the correct value with 0. The same-name slots are legitimate
(different source_keys); the RESOLUTION crossing source keys is not.

This is the third flavor of the by-NAME resolution disease (siblings:
two-site struct-result smear; nested-guard ZII — both fixed via slot
naming). Locals cannot be renamed at mint (user names); the fix is on the
resolution/emission side: either the Mutation fallback write must not run
when the call-result capture already delivered (emission-tracked, not
shape-guessed — note writes/mod.rs's "single-terminal mutation write stays"
comment relies on it for fs terminal-value completion), or its terminal-name
resolution must stay pinned to the CALLEE's source_key with NO cross-key
fallback.

Until then the std authoring dodge: unique let names across std wrapper
machines that can be called from one caller state (time.omg's
`elapsed_since` uses `stopwatch_*`-prefixed lets for exactly this reason).

Promote to canaries/pass/calls/ when the emission-side fix lands (filed in
TASKS.md "Open latent bugs").
