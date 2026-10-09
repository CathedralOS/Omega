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

Another route declares distinct input lifetimes and returns only the first
input. The unrelated second input can be mutated while that result stays live.
Both selector values exercise this declared-source restriction.

The enclosing-record route calls the helper inside a field initializer. Its
returned descriptor pins the exact input parents until the nested target's final
read; scalar siblings remain readable after both original backings are writable.

The projected-call routes keep either subtree of a returned pair with distinct
input lifetimes. Its unused sibling retires with the temporary result: that
sibling's backing is writable before the kept target's read. The kept backing
becomes writable after its target's final read, while its scalar stamp stays live.
