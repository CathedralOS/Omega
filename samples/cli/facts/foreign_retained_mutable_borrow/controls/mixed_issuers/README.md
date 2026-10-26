# Mixed retained issuers

One nominal `Pending` is produced by the shared `Reader::register` and mutable `Writer::register`. Both registrations pass through the same checked helper and an `Envelope` field. The shared backing remains readable while registered; mutable backing stays exclusive. The mutable provider writes 41 on registration and 73 on completion, while the shared provider observes 37. The earlier mutable scalar copy stays 37.

The two providers have separate capacity types and lifecycle state. Their existing C implementations are unchanged. Every positive entry binds all four hosted targets.

Actual macOS ARM64 execution with frozen producer `f8d4d2c2c65e2304e535bfea3febb9b6e59d469e` reports native exit 0 and Terminal exit 0. The invalid twin inserts one read of mutable backing while its transported registration remains live and rejects for that active borrow. `positive.json`, `negative.json` and raw stdout/stderr preserve command, producer and source identities. This establishes the local host only; other-host execution belongs to the parent cohort qualification.
