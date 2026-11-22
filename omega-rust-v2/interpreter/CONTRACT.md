# Interpreter: executing a checked closed entry

The compiler sometimes needs the value a machine returns, such as the access plan of `domain i32::Locked access Rule`. This crate runs the machine's closed Psi body on the actual arguments and returns that value with a record of what the run used. It executes only bodies it can account for exactly, and it refuses everything else before anything runs. The result is a value of the unit's own types. Nothing here validates it against a demand or grants any authority. [Entry points](src/lib.rs)

## What is admitted

`execute_policy` accepts a sealed L6 proof for the entry and its constructor. Internal admission takes the proof’s Psi unit and exact body identities. It follows every call and refuses the whole entry unless each reached body satisfies all of the following. [Admission](src/admission.rs)

- **Checked.** A `CheckedBody` names this exact unit and machine by address. Evidence for an equal body in another unit, such as the toolchain's defining package, does not apply.
- **Straight-line.** The body is one block that ends in a return or a terminal call. A branch, jump or external exit is refused.
- **Acyclic.** No body can reach itself through a call. An admitted entry therefore terminates, as [evaluation](../../wiki/spec/language/evaluation.md#termination-and-sponsorship) allows for a closed acyclic checked body.
- **Effect-free.** The machine has no realized service reach, cannot suspend or block, publishes no crash route, owes no precondition and stores into no mutable home.
- **Supported operations only.** A constant, a whole copy, a whole move, a record, a case, a whole-type zero, a structural or scalar array, and a call of a packaged machine. Arithmetic, reads through views, stores, descriptors and every other operation are refused by name.
- **Supported types only.** Primitive scalars, records and sums without an authored cleanup, shared byte views, shared references to records or sums, fixed arrays and by-value sequences.
- **Evaluable data predicates.** A record or case the body establishes may state `requires` clauses over its own fields, fixed elements, literals and the comparison and Boolean operators.
- **A value result.** The entry's result type holds no reference to a record or sum. Only a snapshot of values crosses back to the caller, as the [hermetic bridge](../../wiki/spec/language/evaluation.md#invocation-admission) requires. A byte view is allowed because it names the caller's own frozen bytes.

These are [invocation admission](../../wiki/spec/language/evaluation.md#invocation-admission) axes read from the Psi contract and body. The checker's evidence covers the proof obligations each operation owes; the executor does not re-prove them.

## What a run does

`AdmittedEntry::execute` takes the arguments and the root's ceilings. It checks that each argument is a completed value of its formal's type, then runs the bodies. [Execution](src/execution.rs)

The executor never stores into a completed value, so a shared reference always reads the value it was formed over. A byte view keeps the caller's frozen backing by reference count: the view in a returned plan is the same allocation the caller supplied.

When an operation establishes a record, a case or a whole-type zero, the executor evaluates that schema's `requires` clauses on the value it just formed. A false clause stops the run with `Stop::Inconsistent` and no value. The [evaluation rule](../../wiki/spec/language/evaluation.md#invocation-admission) calls this a consistency failure between the checker's evidence and the execution, not a machine result. [Predicates](src/predicates.rs)

## Work and storage accounting

**Work** follows [schedule 1](../../wiki/spec/resources/logical_work.md#work-is-an-accounting-domain): one unit for each executed Psi operation and one for each taken control-flow edge, charged before the site runs. Cleanup listed on an edge belongs to that edge. Because every admitted body is one path with no loop, `AdmittedEntry::certified_work` states the exact charge before the run, and a completed run uses exactly that amount.

**Live cells** have no definition in the shared specification, which says only that peak live cells receive the same treatment as work. This crate uses cell schedule 1, which follows V1's logical storage cells (one per stored scalar row and one per activation frame):

- one cell for each activation frame;
- one cell for each scalar value a frame binds;
- one cell for each scalar, case tag, sequence length and reference or view descriptor stored in a structural value that a frame or the caller holds.

A transfer moves cells and a copy takes new ones. A frame's cells are released when it returns, except those of the result it hands to its caller. A view's backing bytes are the caller's input and are not cells. Cells are reserved before the value that needs them exists. [Meter](src/meter.rs)

**Ceilings** are the root's choice. `Ceilings` holds an optional work budget and an optional live-cell ceiling. `None` means the root set no ceiling on that account, as the specification permits; it is not a large number. The meter counts either way, so the usage record is identical with and without ceilings, and a ceiling decides whether a result is obtained and never which result. A budget of zero is a budget: the first operation is refused.

## Outcomes

| Outcome | Meaning |
| --- | --- |
| `Refusal` | Admission refused the entry. Nothing ran. It names the body and the operation or edge site that stopped admission. |
| `Completed` | The entry returned. Carries the result and the usage: both schedule identities, work used, peak live cells and the executions of each site. |
| `Stop::Resource` | The work budget or the live-cell ceiling refused the next charge. Reports the amount used, the amount remaining and the unpaid charge. This is a build-resource outcome, not divergence. |
| `Stop::Argument`, `Stop::Arity` | The caller's arguments do not fit the entry. |
| `Stop::Inconsistent` | A value contradicts the admitted body or its checked evidence. |
| `Stop::Host` | The host could not allocate storage the ceilings allowed. |

A stop carries the chain of active machines, the charge site and its source occurrence.

## Evidence

- [Fixture bodies](src/closed_constructor_tests.rs) run `build` and a calling `plan` from the ordinary [nested record fixture](../layers/05_prepared-trees-to-terminal-psi/tests/fixtures/aggregate_regions/nested_record.omg): exact work and peak cells, ceilings one unit and one cell short, missing and foreign evidence, a false data predicate, wrong arguments, a reference result, and bodies outside the fragment.
- [Captured policies](src/captured_policies_tests.rs) run the two access-policy fixtures themselves: the closed `Rule::plan` and the toolchain's `access\new_plan`, with the checker's closed-policy proof as evidence and the schema image preparation froze as the argument. `Issuance::Unrestricted{}` uses 110 work units and 142 peak cells. `Issuance::Only{routes: []}` uses 111 and 144.

## Not done here

Validating a returned plan against its schema, regions and callable identities belongs to preparation. Recursion with ranking evidence, branches, preconditions discharged at the call, and the remaining Psi operations are refused until this crate can account for them.
