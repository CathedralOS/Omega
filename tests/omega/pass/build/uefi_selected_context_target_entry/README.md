# Activation-local context entry

The physical adapter calls the selected entry with two distinct synthetic roots.
The middle parameter is an ordinary mutable context, initialized to zero for the
invocation. Its 160 KiB array forces generated context storage into measured
private-stack demand. An ordinary helper mutates the borrowed array. Debug port
0xe9 emits A only after initialization, mutation and both root geometries agree;
X indicates failure and the alternate entry emits B.

Synthetic grants test transport, not firmware authority. Cathedral's boot harness
separately exercises real allocation, firmware exit and retained backing.
