# Bounded-distance private rank

Parent: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` in [TASKS.md](../../../../../../../TASKS.md),
a prerequisite for its recursive slice-predicate customers. This is another
production of the existing joint private-rank fact:

A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

Every place carries the same ordinary slice of seven elements and cursor into
`count`. Starting at zero increments the caller-owned counter from 3 to 10;
starting at seven performs no work and preserves 10. The helper forwards its own
explicit slice and cursor and has no redundant ranking declaration. The twin
changes only `position + 1` to `position`.

| Place | Positive | Nonadvancing twin |
| --- | --- | --- |
| local | [source](../../local/bounded_distance/main.omg) | [source](../../local/bounded_distance/invalid/main.omg) |
| field | [source](../../field/bounded_distance/main.omg) | [source](../../field/bounded_distance/invalid/main.omg) |
| parameter | [source](../../parameter/bounded_distance/main.omg) | [source](../../parameter/bounded_distance/invalid/main.omg) |
| call_result | [source](../../call_result/bounded_distance/main.omg) | [source](../../call_result/bounded_distance/invalid/main.omg) |
| match_join | [source](../../match_join/bounded_distance/main.omg) | [source](../../match_join/bounded_distance/invalid/main.omg) |
| const_name | [source](../../const_name/bounded_distance/main.omg) | [source](../../const_name/bounded_distance/invalid/main.omg) |
| other_state | [source](../../other_state/bounded_distance/main.omg) | [source](../../other_state/bounded_distance/invalid/main.omg) |
| other_machine | [source](../../other_machine/bounded_distance/main.omg) | [source](../../other_machine/bounded_distance/invalid/main.omg) |
| target_body | [source](../../target_body/bounded_distance/main.omg) | [source](../../target_body/bounded_distance/invalid/main.omg) |

On fixed baseline `0e98dfe9d78a`, the frozen parameter positive wrongly refused
with `a member lacks a supported exact ranking witness`. The existing private
inference route recognized a descending scalar or descriptor length but dropped
the paired cursor/bound view. The repair retains the selected natural observation
function and reads either scalar or descriptor coordinates through the same
component judgment. Every call must conserve the upper coordinate; each distance
is independently formed before comparison. Fixed carrier bounds make the
executable complement total without assuming an authored interval or a pending goal.

- Establishment: the root supplies `cursor <= items.len`; the helper establishes
  its own `position < view.len`. Neither an enclosing invocation nor a ranking
  promise supplies private entry facts. Exact scalar field projections retain
  the ordinary borrowed-record authentication rule.
- Identity and lifetime: entry snapshots remain distinct from current reads.
  Each edge captures its actual operands after the completed prefix, and a
  helper's upper coordinate comes from its own invocation. The mutable counter
  remains owned by the ordinary caller throughout the cycle.
- Composition and soundness: every internal call is checked, and removing strict
  edges leaves an acyclic graph. Nonadvancement, an
  [inverted entry](../../controls/bounded_distance_inverted_entry/main.omg), and
  [upper drift](../../controls/bounded_distance_upper_drift/main.omg) reject.
  Existing range, alias, loan, non-tail and preserving-subcycle controls retain
  their separate obligations.
- Terminal Psi and native realization: admitted coordinates retain their natural
  observation function through checked retention, ordinary scalar computations
  and the constant-stack component. All four hosted entries are declared;
  execution evidence here is macOS ARM64 native/Terminal agreement with exit 0.

The [clamped-view overshoot](increasing_to_overshoot/main.omg) checks the
boundary where an increasing cursor passes its bound. Its
[twin](increasing_to_overshoot/invalid/main.omg) removes advancement. A
[post-clamp-only cycle](../../controls/increasing_to_post_clamp/main.omg) cannot
count complement movement as strict source-view descent once the source rank is
already zero. These focused controls do not establish complete IncreasingTo
coverage or close the parent task.

Positives and twins are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_joint_rank_bounded_distance_*`.
The named whole-view, selected-cell and store customers retain their original
31/41/40 canaries, both independent element traversals and the full store
postcondition. The store proof follows canonical call values and exact captured
exclusive-reference actuals; it executes the supported body without treating
callee contracts or a pending result as evidence. Bad terminal branches and
wrong forwarded storage remain reject controls. An overlapping write cannot
reuse an immutable record-field observation as current contents: this symbolic
body fragment declines that unrepresented record mutation. Ordinary record
reads and writes keep their existing owners; versioned record-field predicate
handback remains on the parent task. Arbitrary recursive mutation still needs
an actual invariant proof; revisiting its body grants no result.

Run records identify the exact producer and target in the qualification and PR.
Other hosted native execution, suffix-parser retirement, the remaining source
migration and the Omega-written compiler remain outside this acceptance.

The integrated producer `817db649b669` was measured on macOS ARM64:
all nine bounded-distance places and the overshoot control agree at exit 0,
and all nonadvancing twins reject for their own rank obligation. The focused
qualification also observes the three unchanged slice customers, preserves the
borrowed-record positive and erased-field refusal, and checks the independent
store-branch, backing-identity, arithmetic-formation and caller-bound controls.
