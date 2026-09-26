# opaque-carrier

Claim-free opaque-representation fixture. `PlatformToken` has no source-visible
layout and `inspect` carries no postcondition. The package therefore introduces
representation-TCB review evidence without disguising opacity as a proof or an
accepted semantic claim.

Expected package evidence:

- `PlatformToken` is retained under the exact package identity with boundary-
  opaque supply;
- no fabricated layout or semantic guarantee is inferred;
- review records producer availability and explicit unbound representation;
- compiler checking must resolve the target mechanism/ABI when an actual
  by-value use demands it, independently of installation or lock acceptance.
