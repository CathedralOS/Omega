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

The owned-input route relays two existing descriptor carriers through nested
helper operands into selection inside an enclosing field initializer.
Its match transfers the selected carrier and disposes of the residual input.
The enclosing result retains the original parents, not loans of the consumed
input holders. Both backings become writable after the result's target read;
the selected stamp and enclosing scalar remain readable afterward.

The receiving-local variation uses the same nested helper operands, binds the
returned carrier, then moves it
into the enclosing record. Those transfers keep the same parents through both
receiving holders; argument preparation cannot disappear from lifetime analysis.

The replacement route moves an existing carrier into a mutable local, then
replaces it with the other carrier. Replacement ends the old descriptor after
evaluating the new value, so its backing becomes writable before the new
target's read. The new backing becomes writable after that read while the
replacement's scalar stamp remains live.

Two further routes replace with a fresh constructor borrow and move a stored
carrier out before refilling the slot from a third backing. The saved carrier's
backing becomes writable after its read even while the later occurrence stays
live. A local slot's reused name cannot merge those distinct loan occurrences.
The occupied-refill variation replaces the slot again while the moved-out carrier
stays live. That later replacement's cleanup must not retire the saved carrier.

Both selections also consume a refilled mutable local while its earlier value
survives in another local. Availability follows the receiving occurrence; an
earlier move from that variable does not consume its new value.

A copied-descriptor candidate keeps its original supplier independently live.
Retiring the selected result and unselected complement cannot consume that
supplier through shared ancestry; its later read still precedes restoration.
