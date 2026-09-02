# provider-switchboard

Boundary-provider fixture. Its source supplies exact clock-service
reach/invocation evidence and a checked Omega `MonotonicClock` provider
realization. The canonical build machine selects that exact provider type for
`ClockHost`. The provider has an ordinary checked body: `via Binding` forms are
reserved for irreducible foreign leaves and are not a component-dispatch
mechanism. Authored `Binding::VtableSlot` is parser-rejected; foreign protocol
tables use validated named `Binding::VtableField` leaves. The numeric slot
variant remains downstream only for artifact compatibility decoding/reporting.

`Switchboard.clock` uses the settled affine `Service<ClockHost> in Bound`
carrier. In this fused fixture the compiler may erase that carrier only after
joining it to the exact build-selected clock provider.

Expected package evidence:

- provider requirement identity is recorded;
- selected provider origin and plan identity are recorded;
- the fixture-derived update canary selects `WallClock` instead and proves the
  exact selected-provider-set row changes under real resolver/compiler custody;
- that change is opaque-blocking and triage rejects the update.
