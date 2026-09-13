# Build evaluation

`src/lib.rs` admits and executes one selected companion build machine. Its
configuration, declarations, target/root selection, optimization, observation,
and replay modules own their respective results and checks.

## Root-binding implementation boundary

`builder.roots.bind(Target::Slot, Product::entry);` is retained as a dedicated
Psi statement with separate product-context operand paths. It is not an ordinary
machine invocation. The host resolver does not resolve those operands through
host imports; selected-target/product admission still owns that resolution.

Binding follows execution through the original compiler-issued `&mut Build`
cell. Same-package helpers (including imported local modules), reference aliases,
and state transitions use ordinary loan checking. Uncalled helpers and untaken
states contribute no bindings; binding a slot twice on an executed path rejects.
Build.target remains immutable through every alias.

The interpreter returns executed statement coordinates alongside its argument
results, separate from host observations. This owner rejoins them to the exact
prepared program and the occurrence's source/package before target selection.
Replay compares the executed requests as well as argument values and host
observations. Requests are not target-admission evidence.

Foreign helpers are admitted lexically: each `roots.bind` occurrence resolves
its implementation against the product machines of its own source package, so a
helper can bind its own package's entry but never the caller's private
declarations. Owner-selected restricted product-reference handoff to helpers
(no `ProductEntryRef`-style description yet), computed call-result receivers,
and same-named machines across packages remain explicit implementation limits;
the same-name fence holds until Terminal production rejoins the selected
identity by symbol.
[Evaluated build work](../../../../wiki/spec/build/declarations.md#evaluated-build-work)
defines the accepted language; the remaining restricted product-description and
lexical selection handoff is tracked by `BUILD-PRODUCT-REFERENCES` in `TASKS.md`.
