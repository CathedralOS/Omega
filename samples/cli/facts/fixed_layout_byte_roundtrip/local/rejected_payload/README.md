The same byte roundtrip writes `second = 41` while the payload requires
`second >= 42`. Decoding must take `None`; the program exits 0 and checks the
same source/destination canaries. This detects a missing or inverted runtime
guard and checks abandonment of an initially unestablished copy-record view.
The original local witness and static guard-removal twin are unchanged.
Linux x86_64 native/Terminal agreement is recorded with the [parent fact](../../README.md).
