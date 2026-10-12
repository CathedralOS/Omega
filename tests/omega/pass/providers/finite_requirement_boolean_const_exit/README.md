# Public generic requirement review

The public `Service::echo` requirement has two Boolean applications and a
guarded `Trap` ceiling. Its authored generic contract must remain available to
package review after the executable applications have been prepared. Each
application keeps its own callable coordinate despite the identical runtime ABI.

From the repository root on macOS ARM64:

```sh
omega update --project tests/omega/pass/providers/finite_requirement_boolean_const_exit --target macos_arm64
omega run --both tests/omega/pass/providers/finite_requirement_boolean_const_exit/main.omg
```

Both engines must exit 0. The corpus checks execution; the explicit package
update also exercises public contract projection. The related fixtures
`applied_binding_field_telescopes_exit` and
`occurrence_region_binding_projected_type_exit` exercise the same review path
with type and lifetime applications.

`fail/generics/contextual_static_machine_applied_evidence_effect_ceiling` and
`fail/operators/selected_crash_invocation` preserve rejection of uncovered
crash effects. A provider body's narrower effects do not replace the published
requirement ceiling.
