"""Post-lexical name roles and exact reserved spellings, without host parsing."""


def fixtures(rejection):
    identity = b"(def main ((source Bytes)) Bytes source)\n"
    rejected = []
    unfinished = b"(def main ((source Bytes)) Bytes source"
    rejected.extend((
        ("identifier at EOF reaches unmatched-list diagnosis", unfinished,
         rejection(4, len(unfinished))),
        ("later forbidden byte before malformed identifier", b"bad@\x00",
         rejection(3, 4)),
    ))
    for name, prefix, token, suffix in (
        ("malformed upper tail before later name role", b"(data ", b"Item@",
         b" (Item))\n(def Main () Int 0)\n"),
        ("malformed lower tail before later name role", b"(def ", b"helper@",
         b" () Int 0)\n(def Main () Int 0)\n"),
        ("later malformed identifier before earlier name role",
         b"(def Main () Int 0)\n(def ", b"helper@", b" () Int 0)\n"),
        ("malformed underscore tail before later name role", b"(def ", b"_bad@",
         b" () Int 0)\n(def Main () Int 0)\n"),
    ):
        rejected.append((name, prefix + token + suffix, rejection(4, len(prefix))))

    for spelling in (b"Int", b"Bytes"):
        rejected.append(("built-in spelling cannot declare type " + spelling.decode(),
                         b"(data " + spelling + b" (Item))\n" + identity,
                         rejection(4, 6)))
        rejected.append(("built-in spelling cannot declare constructor " + spelling.decode(),
                         b"(data Item (" + spelling + b"))\n" + identity,
                         rejection(4, 12)))

    # Existing run.sh controls already cover exact `if` as function and let binder.
    for spelling in (b"data", b"def", b"let", b"match", b"eq", b"lt",
                     b"bytes_empty", b"bytes_single", b"bytes_length",
                     b"bytes_get", b"bytes_concat"):
        rejected.append(("exact reserved value name " + spelling.decode(),
                         b"(def " + spelling + b" () Int 0)\n" + identity,
                         rejection(4, 5)))

    accepted = (
        ("built-in prefixes remain nominal names",
         b"(data IntX (IntX Bytes))\n(data Bytes_ (Bytes_ IntX))\n"
         b"(def unwrap ((value Bytes_)) Bytes "
         b"(match value ((Bytes_ inner) (match inner ((IntX bytes) bytes)))))\n"
         b"(def main ((source Bytes)) Bytes (unwrap (Bytes_ (IntX source))))\n"),
        ("keyword and bytes built-in prefixes remain value names",
         b"(def if_ ((value Bytes)) Bytes value)\n"
         b"(def bytes_get_extra ((value Bytes)) Bytes (if_ value))\n"
         b"(def main ((source Bytes)) Bytes (bytes_get_extra source))\n"),
        ("underscore names remain ordinary value names",
         b"(def _ ((_input Bytes)) Bytes (let _local Bytes _input _local))\n"
         b"(def main ((_ Bytes)) Bytes (_ _))\n"),
        ("identifier leaves adjacent parentheses for syntax",
         b"(def main((source Bytes))Bytes source)"),
    )
    for label, ending in (("LF", b"\n"), ("CR", b"\r"), ("CRLF", b"\r\n")):
        accepted += (("identifier leaves semicolon for " + label + " comment",
                      b"(def main ((source Bytes)) Bytes source;bad@ 999suffix"
                      + ending + b")"),)
    return rejected, accepted
