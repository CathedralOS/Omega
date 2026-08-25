# provider-switchboard

Boundary-provider fixture. Its source supplies exact clock-service
reach/invocation evidence and an ordinary `MonotonicClock` provider realization.
The canonical build machine selects that exact provider type for `ClockHost`.

Expected package evidence:

- provider requirement identity is recorded;
- selected provider origin and plan identity are recorded;
- the fixture-derived update canary selects `WallClock` instead and proves the
  exact selected-provider-set row changes under real resolver/compiler custody;
- that change is opaque-blocking and triage rejects the update.
