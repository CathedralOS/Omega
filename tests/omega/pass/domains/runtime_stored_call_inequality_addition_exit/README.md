# Stored call inequality before exact addition

`below(limit)` establishes `result < limit` for nonzero u64 limits. The
returned value is stored in an immutable local before exact `value +1`.
The caller must prove representability from that completed call's public
contract and the exact captured operand, without a caller equality guard.

The runtime entry exercises limits1 and18446744073709551615, observes the
corresponding returned limits, and exits0 only when both agree. ProgramEntry
is bound for all four hosted targets; the host must run both engines.

The optimized98eee full-pair baseline refuses the positive with an exact-u64
addition overflow diagnostic. Reuse the ordinary operation-entry fit query
after cheap local range proofs decline, preserving all child formation
obligations. The negative twin permits and actually returns the upper limit,
so its identical addition can overflow. Its expected diagnostic remains at
that operation. The existing data_where_standing nine-place row retains
standing-bound addition coverage; this fixture claims no new generic row.
