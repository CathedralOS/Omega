# Applied ancestor binding with a borrowed result

Run `omega run --both main.omg`. Expected exit: 0 in both engines.

One selected Child map supplies Parent<i32>, including its borrowed operation.
The helper forwards a source loan and returns that same view. The caller reads
37 through it, then writes 43 through a forwarded mutable result and observes
43 in the original storage. The Binding's context borrow must not become the
result source, and the returned loan must survive until its actual last use.
