"""Fixed authored exact-name and persistent-scope controls, without parsing."""


def fixtures(rejection):
    type_name = b"T" + b"a" * 1023
    constructor = b"C" + b"a" * 1023
    function = b"function" + b"a" * 1016
    local = b"value" + b"a" * 1019
    identity = b"(def main ((source Bytes)) Bytes source)\n"
    accepted = [
        ("fresh 1024-byte type name",
         b"(data " + type_name + b" (Wrap Bytes))\n"
         b"(def extract ((value " + type_name + b")) Bytes "
         b"(match value ((Wrap contents) contents)))\n"
         b"(def main ((source Bytes)) Bytes (extract (Wrap source)))\n"),
        ("fresh 1024-byte constructor name",
         b"(data Box (" + constructor + b" Bytes))\n"
         b"(def main ((source Bytes)) Bytes (match (" + constructor
         + b" source) ((" + constructor + b" contents) contents)))\n"),
        ("fresh 1024-byte function name",
         b"(def " + function + b" ((value Bytes)) Bytes value)\n"
         b"(def main ((source Bytes)) Bytes (" + function + b" source))\n"),
        ("fresh 1024-byte local name",
         b"(def main ((source Bytes)) Bytes (let " + local
         + b" Bytes source " + local + b"))\n"),
    ]

    # Insert both a terminal and its extensions in each order. Every resulting
    # sibling is called; later updates must preserve earlier payloads and edges.
    for order, names in (
        ("terminal first", (function, function + b"left", function + b"right")),
        ("terminal last", (function + b"left", function + b"right", function)),
    ):
        values = {function: b"1", function + b"left": b"2", function + b"right": b"3"}
        definitions = b"".join(
            b"(def " + name + b" () Int " + values[name] + b")\n" for name in names
        )
        accepted.append(("long prefix functions " + order,
                         definitions + b"(def main ((source Bytes)) Bytes "
                         b"(if (eq (" + function + b") 1) "
                         b"(if (eq (" + function + b"left) 2) "
                         b"(if (eq (" + function + b"right) 3) source (bytes_empty)) "
                         b"(bytes_empty)) (bytes_empty)))\n"))

    accepted.extend((
        ("long sibling nominal names retain owners",
         b"(data " + type_name + b"left (" + constructor + b"left Bytes))\n"
         b"(data " + type_name + b"right (" + constructor + b"right Bytes))\n"
         b"(def extract_left ((value " + type_name + b"left)) Bytes "
         b"(match value ((" + constructor + b"left contents) contents)))\n"
         b"(def extract_right ((value " + type_name + b"right)) Bytes "
         b"(match value ((" + constructor + b"right contents) contents)))\n"
         b"(def main ((source Bytes)) Bytes (bytes_concat (extract_left ("
         + constructor + b"left source)) (extract_right (" + constructor
         + b"right (bytes_empty)))))\n"),
        ("long local roots restore after sibling scopes",
         b"(def main ((source Bytes)) Bytes (let " + local + b" Bytes source "
         b"(bytes_concat (let " + local + b"left Bytes " + local + b" "
         + local + b"left) (let " + local + b"left Bytes (bytes_empty) "
         + local + b"left))))\n"),
        ("long binder absent throughout its initializer",
         b"(def main ((source Bytes)) Bytes (let " + local + b" Bytes (let "
         + local + b" Bytes source " + local + b") " + local + b"))\n"),
        ("long pattern locals restore across arms",
         b"(data Choice (First Bytes) (Second Bytes))\n"
         b"(def extract ((value Choice)) Bytes (match value ((First " + local
         + b") " + local + b") ((Second " + local + b") " + local + b")))\n"
         b"(def main ((source Bytes)) Bytes (bytes_concat (extract (First source)) "
         b"(extract (Second (bytes_empty)))))\n"),
    ))

    # The diagnostic location is the length of the explicit authored prefix,
    # never a host search for a name or a reconstruction of compiler judgment.
    negatives = (
        ("long duplicate type", 6,
         b"(data " + type_name + b" (First))\n(data ",
         type_name + b" (Second))\n" + identity),
        ("long duplicate constructor", 7,
         b"(data First (" + constructor + b"))\n(data Second (",
         constructor + b"))\n" + identity),
        ("long duplicate function", 8,
         b"(def " + function + b" () Int 0)\n(def ",
         function + b" () Int 1)\n" + identity),
        ("long duplicate parameter", 9,
         b"(def probe ((" + local + b" Int) (",
         local + b" Int)) Int 0)\n" + identity),
        ("long active let conflict", 9,
         b"(def probe ((" + local + b" Int)) Int (let ",
         local + b" Int 0 0))\n" + identity),
        ("long repeated pattern binder", 10,
         b"(data Pair (Pair Int Int))\n(def probe ((value Pair)) Int "
         b"(match value ((Pair " + local + b" ",
         local + b") 0)))\n" + identity),
        ("long nonterminal local prefix is absent", 14,
         b"(def probe ((" + local + b"extension Int)) Int ",
         local + b")\n" + identity),
        ("long local extension is absent", 14,
         b"(def probe ((" + local + b" Int)) Int ",
         local + b"extension)\n" + identity),
        ("long local does not escape sibling scope", 14,
         b"(def probe () Int (+ (let " + local + b" Int 0 " + local + b") ",
         local + b"))\n" + identity),
        ("long nonterminal function prefix is absent", 13,
         b"(def " + function + b"extension () Int 0)\n(def probe () Int (",
         function + b"))\n" + identity),
    )
    rejected = [(name, prefix + suffix, rejection(code, len(prefix)))
                for name, code, prefix, suffix in negatives]
    return rejected, tuple(accepted)
