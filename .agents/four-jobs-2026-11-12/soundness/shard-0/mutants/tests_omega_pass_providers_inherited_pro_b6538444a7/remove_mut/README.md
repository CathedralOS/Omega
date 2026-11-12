# Inherited provider with borrowed results

Run `omega run --both main.omg`. Expected exit: 0 in both engines.

One selected Child map supplies Parent and Child requirements. Static calls
name the declaring trait, `Parent::borrow` and `Parent::borrow_mut`. The helper
forwards a source loan and returns that same view. The caller reads 37 through
it, then writes 43 through a forwarded mutable result and observes 43 in the
original storage. Provider selection must not become the result source, and the
returned loan must survive until its actual last use.

The applied generic ancestor form remains outside this fixture: TASKS.md's
STATIC-MACHINE-MIGRATION records the missing applied requirement expression
syntax. No Child-qualified call or retired Binding carrier supplies that scope.
