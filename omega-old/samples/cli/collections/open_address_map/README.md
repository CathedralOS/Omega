# Open-address map

A fixed-capacity open-addressing hash map over generic key/value pairs with
linear probing and tombstone deletion. `Table<K, V>` stores sixteen `Slot`
entries tagged empty/live/tombstone; `probe` scans from a hashed home slot,
remembering the first tombstone so an absent key reuses it; `insert`,
`lookup`, and `remove` are parameterized on caller-supplied hash and equality
machines (`machine H`, `machine E` contracts) instantiated for `u64` keys and
fixed `[u8; 8]` byte keys.

The driver builds an integer table — inserts 5/21/37, removes 21 (tombstone),
inserts 53 and overwrites 37 — then a byte-keyed table with the same pattern.
Every operand is an anonymous shared borrow (`&5`, `&[65,0,…]`), exercising
expression-rooted temporary retention through machine-parameter calls.

Oracle: every lookup must return its expected value and the live counts must
be 3 and 2 — exit 83 on agreement, 1 otherwise.
