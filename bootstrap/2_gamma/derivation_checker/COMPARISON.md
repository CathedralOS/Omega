# Structural ground comparison

[Ground terms](GROUND.md) | [Inner format](FORMAT.md#comparison-and-finite-execution) | [Calculus](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md)

This component compares the syntax of validated ground terms. It does not
interpret definitions, substitute templates, check proof rows, or accept a
certificate. Structurally different terms may still be provably equal under the
formed theory: `identity(zero)` and `zero` are different syntax here.

## Source-owned comparison session

`comparison_start(grounded)` creates one private session retaining the exact
Grounded payload, zero consumed steps, and an empty completed-comparison memo.
`compare_ground_terms(session, left, right, left_coordinate, right_coordinate)`
compares two global term identities in that session's owner-plus-witness tables.
Coordinates are supplied by the source-owned caller for its checked request
fields; they do not authorize reads or change term identity.

The caller initializes once per checked request and threads each returned session
into the next call. Gamma's immutable pairs do not enforce linear use: reusing
an old session or starting another would repeat allocations without retaining
the cumulative counter. The [proof coordinator](CHECKING.md) enforces this
invariant. Neither sessions nor their internal fields are certificate data.
After a rejected or incomplete result, abandon the request; there is no returned
continuation to resume as though the failed comparison succeeded.

Validate the left identity and then the right identity with checked global
lookup before any identity shortcut, memo lookup, or charged transition. An
invalid identity selects tag 1, code 9 `comparison_reference`, the corresponding
supplied coordinate, and zero limit/requested. This validation also precedes
step exhaustion. Equal invalid integers, including zero, are not reflexivity.

## Comparison and completed memo

For a valid pair, compare identical scalar identities first, then look for a
completed memo entry. Otherwise compare application tag, then symbol identity.
A differing tag or symbol establishes structural difference immediately. Equal
heads have the same arity and sorts under the already checked signatures.
Compare all children in argument order. No definition-order restriction is
imposed on ground applications, and no function is silently evaluated.

Use explicit depth-first pending frames. A frame retains the parent comparison
identity, the next child cursors, remaining children, and the preceding stack.
Visits and resumptions are tail calls; logical tree depth never becomes native
call depth. On a differing child, discard the pending stack and return false.
Already completed child entries may remain in the session, but no unfinished
ancestor is added. No in-progress or interrupted operation can mean equal.

Let `N` be the session's complete ground count. A memo key encodes both ordered
identities as `(left-1)*N+(right-1)`. Ground admission ensures
`1 <= N < 2^19`, so this encoding is injective and lies in `0..N*N-1`, below
2^38. Reversing a pair is a different key; this is not a hash or a pair address.
Use a sparse immutable tagged tree over `[0,N*N)`, splitting the known range
in half. Empty and true leaves are shared tagged pairs. Insert true only after
all children agree (or immediately for equal nullary heads). Identical identity
and memo shortcuts do not insert again. Tree descent is bounded by 38 edges;
updates may use bounded native recursion without recursive term expansion.

Backward ground references strictly decrease both child identities. They rule
out pending comparison cycles, while completed memoization preserves sharing
even for separately encoded DAGs. False entries are not needed for soundness;
repeated failed comparisons still consume the shared work budget.

## Work and allocation provision

Each `visit` and `resume` transition checks and increments the session's consumed
steps before performing its work. This includes the terminal resume of an empty
stack. A same-identity or completed-memo comparison therefore consumes two steps.
A head mismatch consumes one. Every child comparison follows the same rules;
completion of a pending parent is a charged resume, not an unchecked insertion.

The private limit is 655,360 cumulative transitions per session.
Attempting another transition when that count is exhausted selects tag 2,
resource 4 `checking_work`, the current call's original left coordinate,
limit 655,360, and requested 655,361. No Boolean result or session escapes on
exhaustion, including exhaustion between a successful visit and terminal resume.
This is an adjustable implementation provision, not a calculus restriction.
Measure the complete Beta certificate before claiming that this profile fits it.

A memo update copies at most 38 internal nodes at two pairs each; helpers must
not allocate an extra carrier per tree level. The cumulative bound is amortized,
not the maximum allocation of one transition: 48 pairs per consumed work unit
plus 128 once-per-request pairs. Combined with formation and ground admission,
`7,864,346 + 655,360*48 + 128 = 39,321,754` pairs fits the selected arena of
40,265,318. This counts unreachable allocations, not just live storage.

The current implementation uses seven setup pairs: three for the shared context,
two shared memo constants, and two for the initial session. A charged transition
allocates either a memo path of at most 76 pairs, a four-pair pending frame, or a
four-pair terminal result including its replacement session; those branches do
not allocate together within one transition. ID checks and all other helpers
are scalar-only. Terminal failure uses four reserved pairs. A completed-parent
insertion and the subsequent resume are separately charged transitions.

### Amortized allocation argument

The expensive insertion cannot repeat independently of the traversal that
prepares it. Count every allocated pair, including abandoned frames and local
memos. Define accounting credit as `48*consumed_units - allocated_pairs`, excluding
once-per-request setup/failure carriers. This is a proof quantity, not runtime
state. The following bounds also include substitution's reservation carriers:

| Transition branch | Ground pairs | Template pairs |
| --- | ---: | ---: |
| Enter children or advance to the next child | 4 | 8 |
| Insert a completed memo entry | at most 76 | at most 82 |
| Identity/completed-memo shortcut | 0 | 4 |
| Head mismatch or terminal successful resume | 4 | 8 |

At entry to a visit at pending depth `d`, credit is at least `34*d`; at entry
to a resume it is at least `34*(d-1)`. A child-entry transition earns at least
40 credits before increasing depth. Leaf insertion spends at most 34 before
resuming at the same depth. Parent completion spends at most 34 before resuming
one level lower. Advancing to another child earns at least 40 before visiting at
the same depth. Shortcuts earn at least 44; a terminal resume earns at least 40
and returns with nonnegative credit. Mismatch also terminates with nonnegative
credit. These cases cover arbitrary arity, repeated DAG visits, and immutable
frame replacement, not just unary or successful inputs.

A substituted variable suspends its visit after allocating four reservation
pairs, retaining 44 credits. Ground comparison starts at its own depth zero;
any interrupted prefix has deficit at most 34, covered by those 44. A completed
ground call returns nonnegative credit; the subsequent at-most-78-pair local
insertion then leaves exactly the permitted template-visit deficit. Ground never
calls substitution, so there is no unbounded nesting of suspended deficits.
Dropping a local memo does not refund allocation or reset credit.

Thus any charged traversal prefix has deficit at most 34, and every completed
call has nonnegative credit. Positive bulk reservations, proof indexes, rows,
and premises allocate at most eight pairs per unit under their separate source
ledgers, so do not introduce deficits. The fixed 128 pairs cover the single
unfinished-prefix deficit, seven session-setup pairs, and terminal outcome and
boundary carriers, including an uncharged refusal. Failure stops checking;
there is no per-call or per-row renewal of this allowance. A future allocation
or control-flow change must reestablish these inequalities before using this bound.

Live pending depth and completed memo entries are each bounded by consumed
steps; a comparison pair has at most the admitted ground depth. There is no
additional logical-depth cutoff or unbounded lookup in a linked memo list.
Host watchdogs and outer evaluator failures are not owned comparison outcomes.
[Checked substitution](SUBSTITUTION.md) shares this counter and adds positive
bulk reservations for clause scans and template indexes; a bulk refusal reports
the exact `used+amount`, which may exceed the limit by more than one.
Its local memo is separate from the ground memo. [Proof processing](CHECKING.md)
also charges its index, rows, and ordered congruence premises to this counter.

## Private results and diagnostics

Success is tag 6 `Compared`, with payload `(pair equal session)`. `equal` is
scalar 0 or 1, not a proof judgment. `comparison_equal(payload)` and
`comparison_session(payload)` own those projections; `comparison_steps(session)`
returns the consumed step count. A false result still returns the current session
so source-owned coordination can continue without resetting work or memo state.

Diagnostic entries first call `check_derivation_ground()` and forward failures
unchanged. After successful admission they select explicit source-owned comparison
calls. Each Compared observation is tag 6 plus Boolean and consumed steps as two
little-endian u64 words (17 bytes); failures are tag 1/2 plus the existing four
u64 fields (33 bytes). A diagnostic may publish a documented sequence of these
observations, but process status zero only attests that diagnostic publication.
No proof-accepting production `main` is supplied.
