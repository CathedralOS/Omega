# Uniform projected reference selection

The subject call replaces a stored mutable reference before either arm moves
that same descriptor out of its containing Slot. Both choices execute and the
two returned references write the replacement backing. The containing home
must settle once, with no duplicate projected cleanup candidate.

`omega run --both main.omg` must exit 0. The mixed stored-source/fresh-loan
variation is `../runtime_selected_reference_mutation_complement_exit`; its
live-backing-write reject twin also retains the installed reference obligation.
