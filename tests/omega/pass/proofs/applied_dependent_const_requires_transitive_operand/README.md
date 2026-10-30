The preceding equations establish `value == 7` before forming the mathematical
application `identity<u32, 7>(value)`. The selected callee constant remains `7`
while the unrelated caller parameter `Expected` equals `12`. This pins exact
type and constant substitution during call-precondition formation.
