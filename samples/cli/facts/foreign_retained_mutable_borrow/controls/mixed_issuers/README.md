# Mixed retained issuers

One nominal `Pending` is produced by the shared `Reader::register` and mutable `Writer::register`. Both registrations pass through the same checked helper and an `Envelope` field. The shared backing remains readable while registered; mutable backing stays exclusive. The mutable provider writes 41 on registration and 73 on completion, while the shared provider observes 37. The earlier mutable scalar copy stays 37.

The two providers have separate capacity types and lifecycle state. Their existing C implementations are unchanged. Every positive entry binds all four hosted targets.

The parent [retained mutable row](../../README.md) records the executable
qualification. Its gate runs this positive and the invalid twin with both
matching-host C providers. The invalid twin inserts only a read of the mutable
backing while its transported registration remains live.
