/*
We retain the disposition roster for staged selected-CFG rewrite families that
do not yet have an active catalog row in `rewrites::catalog`:
- `arm_relocation`, `boundary_boolean`, `boundary_branch`,
  `commuting_relocation`, `interchange`, `confluence_relocation`,
  `inflow_relocation`, `scheduled_relocation`, `relocation` — owned by
  `EXACT-MACHINE-SIMPLIFICATIONS`.
- `dead_store`, `store_motion` — owned by `ALIAS-AWARE-MEMORY`.
- `peepholes` (`projected_access`) — owned by `DECLARATIVE-PEEPHOLES`; the
  former `terminator_pair`, `condition_materialization`, and
  `copied_call_operand` descriptors now drive the executed `constant_branch`,
  `constant_boolean`, and `copied_call_operand` rules.
*/

pub(crate) mod peepholes {}
