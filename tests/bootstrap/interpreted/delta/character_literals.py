"""Delta character spellings shared by the checker and direct interpreter tests."""


def character_programs():
    """Two spellings of the same byte-producing program, plus its expected bytes."""
    values = bytes(range(32, 127)) + b'\t\n\r'
    escapes = {ord("'"): br"\'", ord("\\"): br"\\", 9: br"\t", 10: br"\n", 13: br"\r"}

    def program(atoms):
        def output(items):
            if len(items) == 1:
                return b'(bytes_single ' + items[0] + b')'
            middle = len(items) // 2
            return b'(bytes_concat ' + output(items[:middle]) + b' ' + output(items[middle:]) + b')'
        return b'(def main ((s Bytes)) Bytes ' + output(atoms) + b')\n'

    quoted = [b"'" + escapes.get(byte, bytes([byte])) + b"'" for byte in values]
    decimal = [str(byte).encode() for byte in values]
    return program(quoted), program(decimal), values


def invalid_characters():
    prefix = b'(def main ((s Bytes)) Bytes (bytes_single '
    for spelling in (b"''", b"'ab'", br"'\x41'", br"'\0'", b"'\n'", b"'A'tail", b"'A''B'"):
        yield prefix + spelling + b'))', 4, len(prefix)
    for unfinished in (b"'", b"'A", b"'\\", b"'\\n"):
        yield prefix + unfinished, 4, len(prefix)
    for source, code in ((b"(def 'x' ((s Bytes)) Bytes s)", 4),
                         (b"(def main ((s Bytes)) Bytes ('x' s))", 4),
                         (b"(def main ((s 'x')) Bytes s)", 4),
                         (b"(def main ((s Bytes)) Bytes 'x')", 15)):
        yield source, code, source.index(b"'")
    invalid_byte = prefix + b"''))\xff"
    yield invalid_byte, 3, invalid_byte.index(b'\xff')
