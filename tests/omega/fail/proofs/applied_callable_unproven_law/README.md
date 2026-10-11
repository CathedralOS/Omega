# Unproven applied law remains rejected

This complete intended-invalid application binds a recursive operation to a
generic requirement but supplies no ensures clause for its associativity law.
It must reject for `proves no ensures matching the declared law`, even while
logical Nat law views and Float::equal are visible. Source checking is the only
intended route; the control must never be accepted or executed.

This preserves the existing ring_law_unproven_rejected theorem obligation in a
complete std-free project with an ordinary resultless ProgramEntry binding.
