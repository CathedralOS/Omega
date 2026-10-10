# A guarded structural producer retains argument authority

Selecting a producer inside a scalar range expression does not grant mutable
access through a shared parent. The ordinary argument checker must reject the
exclusive loan of `envelope.counter`, before either execution engine runs it.
