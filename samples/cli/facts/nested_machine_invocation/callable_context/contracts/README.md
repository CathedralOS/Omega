# Callable public contracts in nested context

Owner: **NESTED-MACHINE-UNIFICATION**, under the existing
[nested-machine invocation fact](../../README.md). This production extends
[the callable-context witness](../main.omg): a nested machine returns its
explicit input under its enclosing static callable binder. The binder's
Requires and Ensures, not a particular caller or supplier body, justify the
same return. The compatible program calls the selected identity with 42 and
exits 0 only if the returned value is 42.

The [positive](main.omg) and [stronger-premise twin](invalid/main.omg) differ
only in the provider's premise (`value > 0` versus `value > 1`). The
[weaker-guarantee control](controls/weaker_guarantee/main.omg) changes only
its public equality to `result >= value` while retaining its identity body.
The caller's concrete 42 and visible body cannot justify either substitution.

The same public state relation also has
[preserved-storage](controls/preserves_poststate/main.omg) and
[changed-storage](controls/changes_poststate/main.omg) positives. The
[stale-entry twin](controls/stale_entry/main.omg) leaves the binder's required
post-state at five while the provider writes and promises nine. Entry facts
cannot supply a mutable postcondition. Unit calls retain their guarantees.
All positive programs retain four hosted Build entries and an exit-0 oracle.
These are contract-operation controls, not another nine-place matrix.

The frozen Rust reference checks accept the positives and reject the intended
contract refinements. The pre-repair native product checker exits 248 during
resolution because callable Ensures are unsupported; it emits no checking
observation. Candidate native/Terminal and product-checker qualification is
pending. The enclosing nine-place row retains its earlier Rust-only evidence;
this extension does not establish product parity for that entire row or close
NESTED-MACHINE-UNIFICATION.
