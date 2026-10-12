# Calling application region scopes

Both `Reading` lifetime arguments and its `Value` reference use one region
owned by the supplying `Reader` declaration. The requirement method has a
separate lifetime named `provider`; equal spelling must not merge its scope
with the implementation's binder.

Package review must retain the repeated trait arguments, the region nested in
`Value`, and the independent method scope. From the repository root on macOS
ARM64, accept the proposed package review and run both engines:

```sh
omega update --project tests/omega/pass/providers/calling_application_region_scopes_exit --target macos_arm64
omega run --both tests/omega/pass/providers/calling_application_region_scopes_exit/main.omg
```

Both engines must exit 0. The explicit update exercises package projection and
recovery in addition to corpus compilation and execution. The invalid controls
`fail/providers/occurrence_region_binding_conflicting_inputs` and
`fail/traits/evidence_binder_distinct_lifetime_arguments` must still reject
incompatible region applications.
