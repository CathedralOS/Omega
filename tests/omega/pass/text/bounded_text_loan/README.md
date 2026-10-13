# bounded_text_loan

Bounded text local `[u8; 8] in Utf8 = "150"` — capacity 8, live len 3; `text.len`/`text[i]` across qualified borrowed loop states; LEARNING: `-> i32` needs explicit `as i32` off the u64 len.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
