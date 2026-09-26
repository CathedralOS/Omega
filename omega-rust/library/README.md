# Rust reference library packages

This directory holds ordinary library implementations for the Rust reference
producer. It corresponds to the library domain in `source/library/`; these
packages are neither compiler stages nor shared compiler foundations.

[Topology](topology/README.md) implements the build-only topology-plan package:
finite component graph composition, policy checking, plan codecs and installation
orchestration. Its independent Cargo package retains its own dependencies and
public API. The [package contract](../../wiki/spec/packages/topology.md) defines
what its checked plans establish.
