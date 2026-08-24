# provider-switchboard

Boundary-provider fixture. Its current source supplies exact clock-service
reach/invocation evidence. A later revision will model two possible providers
for that requirement, with `build.omg` selecting the provider.

Expected package evidence:

- provider requirement identity is recorded;
- selected provider origin and plan identity are recorded;
- update rejects if provider origin or selected-plan evidence changes.
