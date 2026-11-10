# Activation storage plans

Ratified 2026-11-10. The spec defined a suspended activation as one fixed
stack, sized for the worst case of its whole call graph and kept for its whole
life, including while parked. That made one storage layout part of what
suspension means, and it meant a library that wanted to serve very many
waiting activities had to be written a second way, as an operation record with
an `advance` step. Suspension is now defined as preserving the unfinished
computation, the storage layout is a plan chosen where the activation is
created, and the machine being run never mentions it:

```text
Fixed                                  Split

task A: [ whole stack ........ ]       task A: [ kept frames ]--+
task B: [ whole stack ........ ]       task B: [ kept frames ]--+--> worker stack
task C: [ whole stack ........ ]       task C: [ kept frames ]--+    (borrowed while running)
```

```omega
machine serve(request: Request) -> Reply suspends { ... }

let a = TaskRuntime::start<Reply, Request, serve, ActivationStorage::Fixed>(move first);
let b = TaskRuntime::start<Reply, Request, serve, ActivationStorage::Split>(move second);
```

The normative owner is [storage plans](../../spec/build/task_runtime.md#storage-plans).
Track implementation under `ACTIVATION-STORAGE-PLANS` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

1. Suspension preserves the incomplete calls, control state, live values,
   claims, loans, pending operations and cleanup obligations of one activation.
   It prescribes no physical representation.
2. A storage plan has retained backing, which survives a park, execution
   backing, which is leased while running, and transition storage. Capacity is
   covered before execution can need it under every plan.
3. The plan is a compile-time `ActivationStorage` argument at the creation
   site: on `start` and `try_start`, and in the checked plan given to
   `prepare` for an owned activation.
4. Nothing else selects it. No machine, `suspends` clause, package, component
   or runtime option carries a storage mode, and one entry machine may be
   started under either plan.
5. The selection lasts for the activation's life, is not inherited by
   activations it starts, and is never substituted. A plan that cannot be
   realized rejects at build or admission.
6. A machine that starts activations for its caller takes the plan as
   `<const Storage: ActivationStorage>` and passes it on.
7. `Fixed` is the existing fixed nonmoving stack. `Split` keeps the frames and
   places that can be live across a park, and every object whose address must
   survive one, in retained backing, and borrows execution backing for call
   chains that finish without parking.
8. An object whose address must survive a park gets its retained home before
   the address is exposed.
9. Shared execution backing is counted by resident execution, including
   reentry, child stepping, nested adapters and blocking native calls. The
   build sums the demand; it does not choose the plan.
10. Only source suspension may release execution backing. An interpreter
    budget pause and architectural preemption keep the state they stopped.
11. Native code with no resumption interface keeps its real call state: a
    fixed segment for that call, an accounted adapter, or a rejected selection.

The parameter is added with the decision even though one value is realized
first. Adding it later would change every start site and every library that
starts activations, and that cost grows with each library written.

## What changes for source

`TaskRuntime::start` and `try_start` gain a fourth static argument:

```omega
machine start<T, Arguments, machine Target, const Storage: ActivationStorage>(
    arguments: Arguments
) -> Task<T>
```

Each existing start site adds `ActivationStorage::Fixed`, which is what it
means already. A text search found 16 start sites across `source`, `samples`
and `tests`. Machines that are started, and everything they call, do not
change.

## Implementation order

1. Declare `ActivationStorage` with `Fixed` and `Split` in core, add the
   static argument to `start`, `try_start` and the checked plan for `prepare`,
   and sweep the start sites. `Split` rejects at build with a diagnostic
   naming the missing realization.
2. Carry the selection in the activation description and reports, with
   retained and execution demand reported separately. Under `Fixed` the
   retained demand is the whole stack plan.
3. Realize `Split` conservatively: frames of machines that can park live in
   retained backing, and a call into code that cannot park switches to
   execution backing and back.
4. Add the per-target switching convention for the four hosted targets, and
   teach crash paths, backtraces and debug information to follow a call chain
   that spans two regions.
5. Publish retained and execution demand, and the resumption contract, in
   component interfaces. Give native boundaries without one the fixed-segment
   fallback.
6. Refine placement so that only state live at a park is retained.

Stage 1 is the only stage that touches source. Stages 3 to 6 are compiler and
runtime work.

## Acceptance

| Must pass | Must reject |
| --- | --- |
| One entry machine started under both plans in one program | A machine or `suspends` clause that names a plan |
| A library forwarding the plan as a static parameter | Selecting `Split` where no realization exists, with no fallback to `Fixed` |
| A buffer lent to pending I/O keeping its address across a park under `Split` | Releasing execution backing at an interpreter budget pause |
| A deep non-parking call under `Split` leaving no per-activation reservation after it returns | Sizing shared execution backing by worker count while a nested resident segment exists |
| A child activation started from a `Split` activation under `Fixed` | Resuming an activation on an executor that lacks its plan |

## Left open

- Frame layout, the stack-switch calling convention and the continuation
  representation of the `Split` realization.
- The exact shape of the reported retained demand and how reports label a
  conservative whole-frame bound.
- The published form of a native resumption interface.
- How a product supplies one plan to many start sites. An ordinary static
  value set from build configuration is enough; no role registry is specified.
