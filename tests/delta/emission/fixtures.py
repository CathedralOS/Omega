"""Authored serialization bytes, not a host Gamma-plan interpreter."""


# name, mode, source bytes, published text, independently specified extent/cache
CASES = [
    ("integer_atom", 0, b"", b"17", 2, 0),
    ("one_byte_word", 1, b"", b"(x 9)", 5, 1),
    ("seven_byte_word", 2, b"", b"(abcdefg 9)", 11, 1),
    ("eight_byte_fallback", 3, b"", b"(abcdefgh 9)", 12, 0),
    ("empty_word_fallback", 4, b"", b"( 9)", 4, 0),
    ("ignored_high_byte", 5, b"", b"(x 9)", 5, 1),
    ("nul_word_nonzero_cache", 6, b"", b"(\x00 9)", 5, 1),
    ("maximum_packed_word", 7, b"", b"(\xff\xff\xff\xff\xff\xff\xff 9)", 11, 1),
    ("literal_head_fallback", 8, b"first", b"(first 9)", 9, 0),
    ("generated_head_fallback", 9, b"", b"($h3 9)", 7, 0),
    ("nullary_call_fallback", 10, b"", b"(x)", 3, 0),
    ("binary_call_fallback", 11, b"", b"(x 1 2)", 7, 0),
    ("mixed_unary_chain", 12, b"", b"(x (first (second 9)))", 22, 1),
    ("cached_siblings", 13, b"", b"(pair (first 1) (second 2))", 27, 0),
    ("let_initializer_and_body", 14, b"", b"(let $z7 Int (first 1) (x (second $z7)))", 40, 0),
    ("let_before_sibling", 15, b"", b"(pair (let $z7 Int (first 1) (x (second $z7))) (first 2))", 57, 0),
    ("capture_rebuilt_call", 16, b"", b"(first $c100)", 13, 1),
    ("capture_rebuilt_let", 17, b"", b"(let $z7 Int (first $c100) (pair $z7 (second $c100)))", 53, 0),
]
