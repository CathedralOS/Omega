# Captured supplier through parameter transport

Fact: A default trait member keeps an alias's captured const supplier separate from its same-named fresh invocation binder while forwarding the same array value.

The original imported alias, Selected3/Fresh17 bound and 17/42 array operation
remain unchanged. This place carries the alias through another callable’s incoming parameter before forwarding.
The sole twin changes only Selected3 to Selected4; the supplied four-byte value
stays unchanged and must reject required extent 5 versus actual 4.

Current-source checking and native/Terminal observation are pending. No result
for the already-qualified local place is being repeated or relabeled.
