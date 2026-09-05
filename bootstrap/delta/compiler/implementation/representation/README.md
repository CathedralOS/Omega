# Expanded Gamma program

Start at [gamma.gamma](gamma.gamma). A program owns a counted, authored-order
list of definitions. Each definition retains its function name, ordered
parameter binding atoms, and fully expanded Gamma body. The existing fixed
byte helpers and profile adapters remain separately selected publication text;
they are not reconstructed as authored-function plans.

[gamma/expressions.gamma](gamma/expressions.gamma) defines expression nodes and
their constructors. [gamma/names.gamma](gamma/names.gamma) separates admitted
source atoms, source binding identities, function names, generated names,
integer constants, and fixed words.
[gamma/primitives.gamma](gamma/primitives.gamma) builds ordinary Gamma primitive
applications from those nodes; it does not write a textual template.

An expression retains its kind, expanded expression-list height, canonical
serialization byte extent, and kind-specific payload. Atoms have height zero. A call adds one to the maximum
height of its arguments; a let adds one to the maximum of its initializer and
body. This counts generated Gamma lists, including lowering wrappers, not
Delta expression levels or compiler continuation frames. Reused nodes remain
immutable snapshots; height describes their expansion at each occurrence.

The byte extent likewise describes one complete serialized occurrence, not
unique DAG storage. Construction derives it once from the
[serializer's shared formatting helpers](../emission/extents.gamma) and already
constructed child extents. Those count-only calls write no bytes and make no
Delta lowering decisions. Rebuilding a node after capture renaming or helper
extraction refreshes the extent automatically. Checked addition prevents wrapped
summaries. This costs one pair per node and keeps payload preflight from
repeatedly unfolding shared projection tails. Function spelling includes the
selected compilation profile; definition framing, fixed runtime text, and the
entry-owned final LF remain outside expression extents.

Admitted source atom payloads retain exact source spans through retained-node
accessors. A serializer may copy those bytes; it does not traverse Delta
expressions, classify source constructs, or resolve names.

Source parameters, lets, and pattern binders receive binding atoms. Every
lowered reference reuses its established binding atom, so equal spellings in
disjoint scopes do not erase declaration identity. Lowering-generated bindings
retain an explicit marker and source coordinate. Normalization uses that same
atom form with a marker and program-wide allocated identity for fresh helper
names and parameters. Binding references reuse their established atom; no
comparison observes the numeric provenance of a Gamma pair reference.

This representation is shared by source-dependent lowering,
[body-height normalization](../normalization/README.md), and Gamma
serialization. Normalization consumes its explicit heights and binding atoms
and produces another program in the same representation. The complete
normalized plan exists before receipt publication. This representation does
not itself establish complete Gamma-profile admission, compiler-owned resource
accounting, or internal-failure publication.
