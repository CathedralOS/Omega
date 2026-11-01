# Captured alias RHS supplier

Fact: A default trait member keeps an alias's captured const supplier separate from its same-named fresh invocation binder while forwarding the same array value.

The imported `Selected<3>` supplies `Padded<Count>` as the trait Value argument.
Its default member introduces a fresh Const `Count=17`, while the copied alias
must keep captured `Count=3` and declaration `Bias=1`. The same four-byte array
retains 17/42 through default forwarding, explicit/alias annotations and borrows.
The sole twin changes `Selected<3>` to `Selected<4>` with the unchanged four-byte input.

Current-source execution is pending. Source inspection identifies plain node
replacement in default-member copying, which loses prepared alias application
and normalization occurrence metadata; the existing prepared-occurrence API
is the first repair. No historical `090` run would isolate this later seam.
