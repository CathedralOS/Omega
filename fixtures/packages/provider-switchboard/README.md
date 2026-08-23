# provider-switchboard

Provider-selection fixture. It is intended to model two possible providers for
one boundary requirement, with `build.omg` eventually selecting the provider.

Expected package evidence:

- provider requirement identity is recorded;
- selected provider origin and plan identity are recorded;
- update rejects if provider origin or selected-plan evidence changes.

