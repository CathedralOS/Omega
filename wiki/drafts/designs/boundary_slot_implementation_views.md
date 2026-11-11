# Boundary slots as static implementations

Ratified 2026-11-10. Generic code takes the operations it needs as an
implementation parameter so that a test can supply a fake. Supplying the real
service through the same parameter needed a hand-written implementation whose
only job was to forward each call to the boundary trait, because a boundary
trait's name worked in call position and nowhere else. A boundary trait that
inherits an ordinary trait can now be used as a static implementation of it:

```omega
trait ClockOps<S> {
    machine now(state: &mut S) -> u64
        reaches TimeObservation
        crashes none;
}

// before: a bridge that only forwards
FromWallClock satisfies ClockOps<NoState> {
    machine now(state: &mut NoState) -> u64 { WallClock::now() }
}
stamp<NoState, FromWallClock>(&mut none, &mut entry);

// after: the slot itself
boundary trait WallClock: ClockOps<NoState> {}
stamp<NoState, WallClock>(&mut none, &mut entry);

const RealClock: ClockOps<NoState> = WallClock;
stamp<NoState, RealClock>(&mut none, &mut entry);
```

The normative owner is
[boundary slots as static implementations](../../spec/language/conformances.md#boundary-slots-as-static-implementations).
Track implementation under `BOUNDARY-SLOT-VIEWS` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

1. Declared inheritance supplies the relationship. The view is of an exact
   ordinary-parent application, direct or transitive. An unrelated trait with
   matching signatures rejects, nothing is searched for, and an ambiguous
   position takes an annotation.
2. The annotation selects a declared parent and creates no binding; the
   inheritance clause is the declared edge.
3. The view is a static value: a static argument, a `const`, a forwarded
   parameter, a static record field or a projected member. It never
   materializes at run time; a runtime callable over a boundary operation
   still uses an authored local proxy machine.
4. Identity is the slot, the parent application and the member mapping. Two
   slots viewed as one parent differ even with one provider selected for both.
   Aliases of one view are one value.
5. The provider is not part of the value. Selecting another provider keeps the
   application and its guarantees and may change results within the slot's
   contract; realization, artifact and admission dependencies still follow
   the provider.
6. A projected member call is the same boundary demand as a call by path.
   Naming, forwarding and storing the view reach nothing.
7. Members carry the slot's complete contract, never a narrower fact about the
   selected provider. A parent used this way writes `reaches` and `crashes`;
   an open inherited contract is not closed by inspecting a provider.
8. A written reach is a ceiling. An ordinary implementation of the same parent
   may publish less, so a fake still reaches nothing.
9. The view carries the parent's laws and evidence obligations, tied to the
   slot. A relevant witness the slot's public contract does not fix makes the
   view unavailable.
10. The parent's signature is the slot's signature. A consumer needing another
    signature still writes an implementation that calls the slot.

## Acceptance

- `stamp<NoState, WallClock>` and `stamp<NoState, RealClock>` are the same
  application, pass `--both`, and publish `TimeObservation` reach.
- `stamp<SimClock, Simulated>` publishes empty reach under the same parent.
- A view through a transitive ordinary parent is accepted.
- `WallClock` and `SteadyClock` passed to one generic give two applications
  when Build selects one provider for both.
- A boundary trait passed for a trait it does not inherit rejects, with
  matching signatures.
- A parent whose requirement leaves `reaches` or `crashes` open rejects at the
  boundary trait's declaration.
- A parent with a law the provider's admission does not cover rejects the
  provider, not the view's consumers.
- Placing the view in a runtime record rejects and names the proxy route.
- Selecting another provider does not change the generic application's
  identity and does change its recorded realization dependency.
