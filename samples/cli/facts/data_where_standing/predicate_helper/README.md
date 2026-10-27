# Checked predicate helper production

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. This is another production
of the parent fact: the default domain calls an ordinary checked Boolean machine
rather than spelling its comparison directly. The exclusive field write still
establishes the same bound before the produced record crosses each place.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [missing standing predicate](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [missing standing predicate](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [missing standing predicate](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [missing standing predicate](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [missing standing predicate](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [missing standing predicate](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [missing standing predicate](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [missing standing predicate](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [missing standing predicate](target_body/invalid/main.omg) |

Each positive computes the addition before checking transported neighbor fields
17 and 53. A lost field, stale value or incorrect match choice therefore traps;
the neighbor checks never establish the arithmetic bound. Both match choices
execute, and the lower signed value retains its distinct authored result.
Every twin removes only the record's `where` clause.

The [alias](controls/alias/main.omg) and [literal](controls/literal/main.omg)
controls exercise independent constant/index membership judgments. Their
[alias reject](controls/alias_invalid/main.omg) and
[literal reject](controls/literal_invalid/main.omg) respectively remove the
standing predicate and supply 61. The [stronger predicate](controls/stronger/main.omg)
rejects a write guarantee that omits its nonconvex condition; a necessary bound
cannot replace the full predicate. The [qualified formal](controls/qualified_formal/main.omg)
rejects an invocation whose argument does not establish the helper's domain.

Runtime handback uses a complete interval envelope only when the existing
checked predicate owner proves both the consequence and the full carrier
converse. Constant materialization instead retains its exact destination debt
until an ordinary full-type constructor/return judgment completes. The receipt
cannot be stamped while that exact checked probe owes a target proof condition.
Index use independently proves every predicate from decoded own fields through
the existing admitted predicate judgment; canonical bytes supply values, not
membership or authority. False and unknown results refuse publication. The
helper/index judgment currently covers complete positional calls over bare
integer formals.

Destination probes wait for their exact named type inputs to complete.
Declaration-owned ranking terms retain their typed source ownership during
deferred body preparation; the termination witness and proof remain body work.

The fixed discovery base is `0e81d85357829393c3382cea3d3382237a807dc0`.
The ordinary helper spelling refused restricted-field handback on that base,
while the equivalent direct comparison passed. Subsequent constant observations
exposed destination qualification, authored-call custody and index-use proof
boundaries on the same frozen fact. The qualification record preserves each
producer and diagnostic separately. The four pure-endpoint destination controls
close before the conditional receipt fence; they establish ordinary proof
behavior and do not claim to exercise that fence. A focused source-model check
covers the actual qualification publication boundary with demanded and unrelated
retained conditions and a missing probe root.

The parent's [requirements and limits](../README.md#establishment-and-preservation)
apply. Actual native qualification is macOS ARM64 plus Terminal agreement;
four authored entry bindings do not claim native execution on other hosts.
General relational/default-domain and generic predicate support remains open.
