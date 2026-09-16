"""Generic formed-theory stepper: evaluates ground terms and emits the
explicit derivation rows an ordinary checker request needs.

This is not a Beta parser and does not search for proofs.  It replays the
conservative definitions in a parsed GTH1 theory section: every application
is unfolded through its stated clause, constructor children are normalized
under congruence, and equalities are chained with transitivity.  The emitted
proof is still an untrusted certificate; the checker revalidates every row,
premise, clause ordinal, and substituted environment on its own.

Term references are interned structural DAG nodes.  The proposition table
carries only the closure of the two owner roots; every intermediate term
produced while stepping lands in the certificate's witness table, exercising
the owner-plus-witness global reference scheme.
"""

import struct


def _parse_records(data, offset, count):
    records = []
    for _ in range(count):
        (length,) = struct.unpack_from("<I", data, offset)
        end = offset + 4 + length * 4
        if end > len(data):
            raise ValueError("record escapes section")
        records.append(struct.unpack_from(f"<{length}I", data, offset + 4))
        offset = end
    return records, offset


class Theory:
    """A parsed GTH1 section: sorts, constructor and function signatures."""

    def __init__(self, data):
        if data[:4] != b"GTH1":
            raise ValueError("not a theory section")
        (self.sort_count,) = struct.unpack_from("<I", data, 4)
        offset = 8
        (ctor_count,) = struct.unpack_from("<I", data, offset)
        self.constructors, offset = _parse_records(data, offset + 4,
                                                 ctor_count)
        (fn_count,) = struct.unpack_from("<I", data, offset)
        raw_functions, offset = _parse_records(data, offset + 4, fn_count)
        if offset != len(data):
            raise ValueError("trailing theory bytes")
        self.functions = []
        for raw in raw_functions:
            result, argc = raw[0], raw[1]
            arguments = raw[2:2 + argc]
            mode, selected = raw[2 + argc], raw[3 + argc]
            clause_count = raw[4 + argc]
            cursor = 5 + argc
            clauses = []
            for _ in range(clause_count):
                length = raw[cursor]
                payload = raw[cursor + 1:cursor + 1 + length]
                cursor += 1 + length
                constructor, template_count, body = (payload[0],
                                                     payload[1],
                                                     payload[-1])
                templates = []
                inner = 2
                for _ in range(template_count):
                    tlen = payload[inner]
                    templates.append(payload[inner + 1:inner + 1 + tlen])
                    inner += 1 + tlen
                if inner != len(payload) - 1:
                    raise ValueError("clause template table mismatch")
                clauses.append((constructor, tuple(templates), body))
            if cursor != len(raw):
                raise ValueError("function clause table mismatch")
            self.functions.append((result, arguments, mode, selected,
                                   tuple(clauses)))

    def function(self, identity):
        return self.functions[identity - 1]

    def constructor(self, identity):
        return self.constructors[identity - 1]


class Stepper:
    """Evaluate ground terms under a theory and record an explicit proof.

    Nodes are interned ``(tag, symbol, children)`` tuples with tag 1 for
    constructor and 2 for function applications.  ``prove`` returns the
    normalized value node and the proof-row index concluding
    ``term = value``; repeated subterms reuse one equation row.
    """

    def __init__(self, theory):
        self.theory = theory
        self.nodes = []
        self.node_ids = {}
        self.sorts = []
        self.values = set()
        self.eval_memo = {}
        self.proof_memo = {}
        self.reflexive = {}
        self.proofs = []

    # -- term construction -------------------------------------------------

    def intern(self, tag, symbol, children=()):
        key = (tag, symbol, tuple(children))
        found = self.node_ids.get(key)
        if found is not None:
            return found
        identity = len(self.nodes) + 1
        self.node_ids[key] = identity
        self.nodes.append(key)
        if tag == 1:
            record = self.theory.constructor(symbol)
            result_sort, expected = record[0], record[2:2 + record[1]]
        else:
            result_sort, expected = (self.theory.function(symbol)[0],
                                     self.theory.function(symbol)[1])
        if len(children) != len(expected):
            raise ValueError(f"arity mismatch on symbol {symbol}")
        for child, want in zip(children, expected):
            if self.sorts[child - 1] != want:
                raise ValueError(f"sort mismatch below symbol {symbol}")
        self.sorts.append(result_sort)
        if tag == 1 and all(child in self.values for child in children):
            self.values.add(identity)
        return identity

    def byte(self, value):
        return self.intern(1, value + 1)

    def word(self, value):
        digits = [(value >> (8 * slot)) & 0xFF for slot in range(8)]
        return self.intern(1, 277, tuple(self.byte(digit)
                                         for digit in digits))

    def byte_list(self, values):
        node = self.intern(1, 278)
        for value in reversed(values):
            node = self.intern(1, 279, (self.byte(value), node))
        return node

    def source(self, data):
        node = self.intern(1, 285)
        for value in reversed(data):
            node = self.intern(1, 287, (self.intern(1, 286, (self.byte(value),)),
                                        node))
        return node

    # -- evaluation ----------------------------------------------------------

    def is_value(self, node):
        return node in self.values

    def instantiate(self, templates, body, environment):
        built = []
        for row in templates:
            if row[0] == 0:
                built.append(environment[row[1]])
            else:
                children = tuple(built[child - 1] for child in row[3:])
                built.append(self.intern(row[0], row[1], children))
        return built[body - 1]

    def unfold(self, symbol, arguments):
        """One definitional step: pick the stated clause and instantiate."""
        _, _, mode, selected, clauses = self.theory.function(symbol)
        if mode == 0:
            constructor, templates, body = clauses[0]
            environment = dict(enumerate(arguments))
            ordinal = 1
        else:
            chosen = arguments[selected]
            tag, constructor, children = self.nodes[chosen - 1]
            if tag != 1:
                raise ValueError("selected argument is not a constructor")
            environment = {slot: arguments[slot]
                           for slot in range(len(arguments))
                           if slot != selected}
            base = len(arguments)
            environment.update({base + index: child
                                for index, child in enumerate(children)})
            ordinal = None
            for index, (clause_ctor, templates, body) in enumerate(clauses):
                if clause_ctor == constructor:
                    ordinal = index + 1
                    break
            if ordinal is None:
                raise ValueError(f"no clause for constructor {constructor}")
        instance = self.instantiate(templates, body, environment)
        return instance, ordinal

    def evaluate(self, node):
        cached = self.eval_memo.get(node)
        if cached is not None:
            return cached
        tag, symbol, children = self.nodes[node - 1]
        if tag == 1:
            result = self.intern(1, symbol, tuple(self.evaluate(child)
                                                  for child in children))
        else:
            values = tuple(self.evaluate(child) for child in children)
            instance, _ = self.unfold(symbol, values)
            result = self.evaluate(instance)
        self.eval_memo[node] = result
        return result

    # -- proof emission ------------------------------------------------------

    def proof(self, rule, left, right, *fields):
        self.proofs.append((rule, left, right) + tuple(fields))
        return len(self.proofs)

    def same(self, node):
        row = self.reflexive.get(node)
        if row is None:
            row = self.proof(1, node, node)
            self.reflexive[node] = row
        return row

    def prove(self, node):
        """Append rows concluding ``node = evaluate(node)``; return both."""
        cached = self.proof_memo.get(node)
        if cached is not None:
            return cached
        tag, symbol, children = self.nodes[node - 1]
        if self.is_value(node):
            result = (node, self.same(node))
        elif tag == 1:
            values, premises = zip(*(self.prove(child)
                                     for child in children))
            normalized = self.intern(1, symbol, tuple(values))
            row = self.proof(4, node, normalized, len(children), *premises)
            result = (normalized, row)
        else:
            if all(self.is_value(child) for child in children):
                normalized = node
                congruence = None
            else:
                values, premises = zip(*(self.prove(child)
                                         for child in children))
                normalized = self.intern(2, symbol, tuple(values))
                congruence = self.proof(4, node, normalized,
                                        len(children), *premises)
            arguments = self.nodes[normalized - 1][2]
            instance, ordinal = self.unfold(symbol, arguments)
            unfold_row = self.proof(5, normalized, instance, ordinal)
            if self.is_value(instance):
                chain = unfold_row
                value = instance
            else:
                value, body_row = self.prove(instance)
                chain = self.proof(3, normalized, value, unfold_row,
                                   body_row)
            if congruence is None:
                result = (value, chain)
            else:
                result = (value, self.proof(3, node, value, congruence,
                                            chain))
        self.proof_memo[node] = result
        return result

    # -- request assembly ------------------------------------------------------

    def owner_closure(self, roots):
        marked = set()
        stack = list(roots)
        while stack:
            node = stack.pop()
            if node in marked:
                continue
            marked.add(node)
            stack.extend(self.nodes[node - 1][2])
        return marked

    def encode(self, left_root, right_root):
        """Split interned terms into owner/witness tables and emit the
        ``(proposition, certificate)`` sections for the claimed root."""
        owner = self.owner_closure((left_root, right_root))
        owner_terms = [node for node in range(1, len(self.nodes) + 1)
                       if node in owner]
        witness_terms = [node for node in range(1, len(self.nodes) + 1)
                         if node not in owner]
        references = {}
        for index, node in enumerate(owner_terms):
            references[node] = index + 1
        base = len(owner_terms)
        for index, node in enumerate(witness_terms):
            references[node] = base + index + 1

        def term_record(node):
            tag, symbol, children = self.nodes[node - 1]
            return (tag, symbol, len(children),
                    *(references[child] for child in children))

        owner_records = [term_record(node) for node in owner_terms]
        witness_records = [term_record(node) for node in witness_terms]
        encoded_proofs = []
        for row in self.proofs:
            rule, left, right, *fields = row
            encoded_proofs.append(
                (rule, references[left], references[right], *fields))
        return (owner_records, references[left_root],
                references[right_root], witness_records, encoded_proofs)
