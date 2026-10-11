# Nominal constant lexical ownership

Run `omega run --both main.omg`. Native and Terminal execution return 0 only
when imported nominal constants retain their declaring constructor through
aliases, a constant call, a match and an exact aggregate index, while runtime
parameters and prior locals keep their own values. A later local and a
same-spelled value parameter cannot capture earlier declaration-owned names.

This combines the valid shadowing coverage from `lexical_aggregate_values`
with `selected_constant_nominal_alias_process_exit`. It does not depend on
implicit root-to-module visibility or std process-exit providers; the Build
declaration binds every supported hosted entry explicitly.
