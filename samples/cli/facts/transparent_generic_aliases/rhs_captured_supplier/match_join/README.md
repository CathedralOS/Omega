# Captured supplier through match join transport

Fact: A default trait member keeps an alias's captured const supplier separate from its same-named fresh invocation binder while forwarding the same array value.

The imported `Selected<3>::forward<17>` operation, `Padded<3>` array and
17/42 observations are unchanged. This place transports that array through
a match join of alias and explicit array arms; both boolean branches execute.
The sole twin changes only `Selected<3>` to `Selected<4>` and requires the
same exact extent-5 versus extent-4 refusal. Current-source checking and
native/Terminal qualification are pending. Previously qualified local,
field and parameter places are excluded from this batch.
