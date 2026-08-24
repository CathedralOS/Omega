# provider-switchboard

Boundary-provider fixture. Its source supplies exact clock-service
reach/invocation evidence and an ordinary `MonotonicClock` provider realization.
The canonical build machine selects that exact provider type for `ClockHost`.

Expected package evidence:

- provider requirement identity is recorded;
- selected provider origin and plan identity are recorded;
- update rejects if provider origin or selected-plan evidence changes.
