# provider-switchboard

Boundary-provider fixture. Its source supplies exact clock-service
reach/invocation evidence and a checked Omega `MonotonicClock` provider
realization. The canonical build machine selects that exact provider type for
`ClockHost`. The provider has an ordinary checked body: `VtableSlot` and other
`via Binding` forms are reserved for irreducible foreign leaves and are not a
component-dispatch mechanism.

`Switchboard.clock: ClockHost` remains a transitional compiler-surface fence.
The settled runtime carrier is `Service<ClockHost> in Bound`; the fixture must
migrate when that carrier and its routed installation establishment land.

Expected package evidence:

- provider requirement identity is recorded;
- selected provider origin and plan identity are recorded;
- the fixture-derived update canary selects `WallClock` instead and proves the
  exact selected-provider-set row changes under real resolver/compiler custody;
- that change is opaque-blocking and triage rejects the update.
