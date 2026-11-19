# Pass a saved field reference to a helper

These programs save a reference to `Counter.value`, pass it to a helper, and
use the field's bound afterward. The helper temporarily writes 61 and then
restores a value no greater than 60. The caller's addition needs no extra guard.
The record states its bound through an ordinary Boolean helper, `bounded`.

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS** in
[TASKS.md](../../../../../TASKS.md). This is another way of producing the same
fact in the [parent folder](../README.md).

| Place | Positive | Missing predicate | Unfinished restoration |
| --- | --- | --- | --- |
| Local | [program](local/main.omg) | [control](invalid_local/main.omg) | [control](invalid_local_window/main.omg) |
| Field | [program](field/main.omg) | [control](invalid_field/main.omg) | [control](invalid_field_window/main.omg) |
| Parameter | [program](parameter/main.omg) | [control](invalid_parameter/main.omg) | [control](invalid_parameter_window/main.omg) |
| Call result | [program](call_result/main.omg) | [control](invalid_call_result/main.omg) | [control](invalid_call_result_window/main.omg) |
| Match join | [program](match_join/main.omg) | [control](invalid_match_join/main.omg) | [control](invalid_match_join_window/main.omg) |
| Const name | [program](const_name/main.omg) | [control](invalid_const_name/main.omg) | [control](invalid_const_name_window/main.omg) |
| Nested machine | [program](other_state/main.omg) | [control](invalid_other_state/main.omg) | [control](invalid_other_state_window/main.omg) |
| Other machine | [program](other_machine/main.omg) | [control](invalid_other_machine/main.omg) | [control](invalid_other_machine_window/main.omg) |
| Selected body | [program](target_body/main.omg) | [control](invalid_target_body/main.omg) | [control](invalid_target_body_window/main.omg) |

The first control removes only the record's predicate. The unfinished-restoration
control keeps that predicate but lets the helper return 61 under a matching,
weaker guarantee. The helper remains valid; its caller cannot hand 61 back to a
field whose enclosing record still requires at most 60.

Every positive checks the updated field and nonzero neighbors 17 and 53. High
and low inputs produce different results. Both match arms run with distinct
values, and the selected implementations check different tags. Omitting the
write, changing the selected path, or overwriting a neighbor must trap. Those
runtime checks do not supply the earlier arithmetic proof.

Construction proves the record's requirements before borrowing its field.
The helper's checked return guarantee restores the field bound, and the receiving
body uses that declared bound directly. Calls and joins must keep the same value
and its justified requirements; a later result guard supplies no missing proof.

Proof clauses disappear from executable output. Reference formation, writes,
calls, matches and canary reads must remain. Comparing Terminal and native runs
checks those effects through record layout, argument and return placement,
instruction selection and image emission. The ordinary Build declarations select
the target implementations; declaring another host does not establish execution
on that host.

The compiler may omit a local variable that only names borrowed storage. It
must still record which storage and field it names, its read/write permission,
and any index evaluated when the borrow was formed. Calls consume that record;
they do not evaluate the initializer again. Real reference-value initializers
retain their own evaluation and storage. The
[loan rules](../../../../../wiki/spec/terminal-psi/loans.md) continue to decide
when the parent may be used again.

The extra [alias](controls/alias/main.omg) and [literal](controls/literal/main.omg)
controls retain constant and index checks. Their
[alias reject](controls/alias_invalid/main.omg) and
[literal reject](controls/literal_invalid/main.omg) remove the predicate or
supply 61. The [stronger predicate](controls/stronger/main.omg) requires the
complete condition, including `value != 13`. The
[qualified formal](controls/qualified_formal/main.omg) requires its argument to
establish the helper's declared domain.

When a Boolean helper defines a domain, an interval can replace its condition
only after proving both directions. A necessary bound alone is insufficient.
Constants and indices still owe every predicate of their receiving type;
decoded bytes supply values, not domain membership or access permission. The
current helper judgment covers complete positional calls over plain integer
parameters. General relational and generic predicates remain open.

The [earlier direct-field version and qualification details](https://github.com/CathedralOS/Omega/blob/f42397bd26d8e3b0ec77c78abac15f26563c4560/samples/cli/facts/data_where_standing/predicate_helper/README.md)
retain their actual sources and limitations. Some earlier endpoint controls
reject before the later constant-publication check and do not establish that
later check. Type arguments must be ready before constant publication;
termination proofs still belong to the function body.

The frozen baseline uses optimized source
`2713116252386e98ce5e87961aca764f8760b217`. Eight saved-reference places fail while
producing executable compiler output with "computed borrow lost its established
local"; the const-name place already checks. The first loan-record repair,
`d6448cfae742e9142043bacd0a2be6a3891820b6`, still failed those eight places because
constructing another reference skipped the saved loan.

Optimized producer `cd5340c6d0348e868e34d68ee85f250c431f79ba` runs all nine current
positives with native and Terminal exits of zero on macOS ARM64. Their eighteen
controls reject for the intended missing-bound or unfinished-restoration reason.
The [saved-reference report](../../../../../wiki/drafts/measurements/saved_field_reference_calls.md)
records the source, producer and evidence, including the later removal of a
redundant lowering check. The checked producer already records the reference
crossing that lowering consumes.

All programs declare the four hosted entry bindings; other-host execution is
unverified. The parent migration remains open, and GUI qualification remains
deferred as requested.
