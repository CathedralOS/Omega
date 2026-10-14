# Bounded resource issuance

`omega run --both main.omg` must report native and Terminal exit 0.

The private issuer encodes a nonnegative signed input as a nonzero unsigned
identity, returns an affine qualified resource inside an acquisition result,
and transfers that owner through a case match and state edge. The caller borrows
it to observe identity 1 and consumes it on release. This models the ownership
and descriptor-zero encoding used by the portable filesystem without invoking
an operating system or treating a numeric snapshot as issuance authority.

The issuer still proves the numeric predicate independently of its route.
The domain fixtures cover local, inline, call-result and record/state forms,
including a captured arithmetic child before a sibling mutates its source;
false, stale, wrapping, unknown-branch and nonissuer controls must reject.
