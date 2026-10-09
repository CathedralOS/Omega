# Selected record reference returned by a helper

After ordinary package update and review, run:

```sh
omega run --both tests/omega/pass/borrows/runtime_selected_reference_record_helper_exit/main.omg
```

Both selections return a record pointing at the selected original input. Each
input has a distinct exclusive parent; observing the returned descriptor ends
its shared cohort before mutation of either original backing is restored.
The non-reference record stamp remains readable afterward. Both engines exit 0.

The second route puts selection inside a field, forwards it through a named
state and a helper call, and copies the returned shared descriptor. Mutation
waits for both the original returned descriptor and its copy to finish.
