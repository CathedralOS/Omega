# Alias-qualified cases

Fact: A transparent generic alias retains its underlying case identity and payload type through qualified construction and matching.

The imported `Chosen<T [copy]> = Choice<T>` constructs and matches Some42
and Empty17, with explicit `Choice<u64>` interchange. A qualified generic
record pattern supplies42 too; it is a required disambiguation contrast to
case patterns, not permission to classify every long path as a case.
The sole twin changes only the Some payload from the carried u64 to true.
The expected refusal is the ordinary bool-to-u64 construction-field guard.

Historical baseline and current-source qualification are pending. No claim for
other places, hosts, Product or complete aliases is made by this local witness.
