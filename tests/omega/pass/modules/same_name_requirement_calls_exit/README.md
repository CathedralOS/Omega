# Module-owned service calls

The two modules deliberately reuse `Sink`, `Provider`, `Runner` and `emit`.
The selected right-hand runner must call its own provider from both its entry
and named state, print `R` followed by a newline and exit 70. The left provider
exits 66, making a wrong-owner call observable.

Prepare the package through ordinary review, then run both engines:

```sh
omega update --project tests/omega/pass/modules/same_name_requirement_calls_exit --target macos_arm64
omega run --both tests/omega/pass/modules/same_name_requirement_calls_exit/main.omg
```

Before the service publication repair, this program failed with `Unit services
have empty or duplicate canonical identities`. The checked service table had
distinct resolved symbols; lowering used their diagnostic labels as artifact
identities. Both executable service publication and retained callback reach
contracts now encode the resolved declaration's package and module identity.

`providers/same_name_requirement{,_right}_dispatch_exit` exercise the two
scalar-result selections. `modules/same_name_service_callback_exit` retains
same-spelled callback ceilings, including unused callbacks; the paired
`fail/modules/same_name_service_callback_wrong_owner` rejects a callback whose
service belongs to the other module. Those tests preserve declaration identity
without requiring distinct authored names.
