"""Escaping shared by Rust diagnostic test reports."""
import json
import re
from urllib.parse import unquote_to_bytes


def readable(text):
    """Quote control characters for display while keeping the original text in evidence.json."""
    return text if text.isprintable() else json.dumps(text, ensure_ascii=True)


def unescape(field):
    """Recover bytes from a tab-separated diagnostic field emitted by a Rust test."""
    if malformed := re.search(r"%(?![0-9A-F]{2})", field):
        raise ValueError(f"malformed percent escape at character {malformed.start()}")
    if control := re.search(r"[\t\r\n]", field):
        raise ValueError(f"unescaped control character {control[0]!r}")
    try:
        return unquote_to_bytes(field)
    except UnicodeEncodeError as error:
        raise ValueError(f"field holds a lone surrogate at character {error.start}") from None
