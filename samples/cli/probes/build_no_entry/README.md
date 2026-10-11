# build_no_entry

An `application` package needs no `entry` machine under `--check`:
ProgramEntry binds when the compiler realizes a runnable artifact or
`omega run` selects a program, never when Psi checks the source.
