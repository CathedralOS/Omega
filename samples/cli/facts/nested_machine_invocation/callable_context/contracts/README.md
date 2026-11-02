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
The [owned-reference carrier](owned_reference/main.omg) and its
[stale-entry twin](owned_reference/invalid/main.omg) carry the same obligation
through an owned record containing an exclusive reference. Owning the record
does not freeze its referent; post-state proof must retire that referent's entry
facts. The Rust reference already runs the positive in both engines and rejects
the twin; new product observations remain pending.
The [changed owned referent](owned_reference/changes/main.omg) observes nine
from an initial five; its [weaker-public-guarantee twin](owned_reference/changes/invalid/main.omg)
keeps the write but removes the promised equality. The
[eager-operand control](owned_reference/eager_operands/main.omg) passes an
initial scalar observation before a later owned-borrow call writes nine. Its
[twin](owned_reference/eager_operands/invalid/main.omg) requires the earlier
observation to be six. The positive traps unless the sum is fourteen and the
live value is nine, so omitted writes and reordered observations are visible.

A [weaker provider premise](controls/weaker_precondition/main.omg) is permitted.
[Forwarding a distinct binder](forwarded/main.omg) uses its public guarantees;
[its twin](forwarded/invalid/main.omg) removes the required equality while
keeping an exact visible supplier body. These use the same refinement judgment.

The [returned-view snapshot](returned_reference/main.omg) proves that a saved
scalar remains five while a store through the returned view changes the live
value to nine. Its [twin](returned_reference/invalid/main.omg) passes the live
value in place of that snapshot. The original frozen stale-entry controls
exercise both [calls](returned_reference/controls/stale_call/main.omg) and
[stores](returned_reference/controls/stale_store/main.omg). A logical result
subject is separate from the storage it may reach; copying the view retains
both, without asserting equality to every possible backing.

All positive programs retain four hosted Build entries and an exit-0 oracle.
These are contract-operation controls, not another nine-place matrix.

The frozen Rust reference checks accept the positives and reject the intended
contract refinements. The pre-repair native product checker exits 248 during
resolution because callable Ensures are unsupported; it emits no checking
observation. Candidate native/Terminal and product-checker qualification is
pending. The enclosing nine-place row retains its earlier Rust-only evidence;
this extension does not establish product parity for that entire row or close
NESTED-MACHINE-UNIFICATION.
