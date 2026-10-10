# Explicit lifetime selection in an applied callback

Run `omega run --both tests/omega/pending/generics/contextual_static_machine_explicit_lifetime_application_exit/main.omg`.
Both engines must eventually return 0. This application explicitly selects the
provider's invocation lifetime from the caller's borrow binder, independently of
its type and conformance operands. Retain and admit that region mapping before
forming a callable view or executable identity; keeping the provider's original
universal lifetime telescope would discard the authored selection.

The producer currently refuses explicit nested lifetime operands until that
mapping is represented. This is an implementation dependency, not an owner
decision or an arbitrary expansion limit.
