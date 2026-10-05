# Application

[cli/](cli/README.md) is Cargo package and binary `omega`. It owns command
parsing, dispatch and complete compile, run and inspect workflows.

Commands use [compilation](../compilation/README.md) and
[package](../packages/README.md) services. The CLI supplies policy admissions to
the compiler in memory; compilation does not load or replace application policy
files. Both packages retain their existing manifests and public APIs.
