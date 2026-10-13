A true guard completes before the following crash, through a mutable reference value, a terminal reference call, a scalar value, and a terminal Unit call. Reference reads retain the caller's value, and both engines must exit zero. The sibling crash fixture exercises the false guard.

The per-target Terminal oracle installs the same retained artifact for every
hosted target bound by this fixture and requires normal exit zero. The shared
stdout/stderr oracles require empty program streams in both engines. Together with
the false-guard sibling, it distinguishes selected reference, scalar and Unit
completions from falling through into an authored trap.
