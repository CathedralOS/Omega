"""Read-only profiling of the pinned Gamma heap; never an admission check.

Gamma's PAIR layout comes from gamma_evaluator.beta. The two-field console
cursor constructor and Bytes constructors come from the pinned canonical
receipt. All nodes below the committed cursor are immutable and never reclaimed.
Public output and known read sequences independently validate this projection.
"""
import hashlib
import os
from pathlib import Path
import struct
import time

ARENA = 0x20400000
MEMORY_SIZE = 0x2000000000
PAIR = 0x50414952

class HeapPhases:
    def __init__(self, process_id, stdin, checkpoint_cursors, checkpoint_outputs, started):
        self.process_id = process_id
        self.stdin = stdin
        self.checkpoint_cursors = checkpoint_cursors
        self.checkpoint_outputs = checkpoint_outputs
        self.started = started
        self.memory = open(f'/proc/{process_id}/mem', 'rb', buffering=0)
        self.base = None
        self.next_address = ARENA
        self.input_root = None
        self.prefix = bytearray()
        self.output_root = None
        self.output = bytearray()
        self.last_input_cursor = -1
        self.events = []
        self.previous_wall = 0.0
        self.previous_cpu = 0.0
        self.chunk = b''
        self.chunk_start = ARENA
        self.committed = ARENA

    def cpu(self):
        fields = Path(f'/proc/{self.process_id}/stat').read_text().rsplit(') ', 1)[1].split()
        return (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK')

    def pair(self, address):
        if address < ARENA or address + 40 > self.committed or (address-ARENA) % 40:
            raise ValueError('invalid pair address')
        if self.chunk_start <= address and address + 40 <= self.chunk_start + len(self.chunk):
            result = struct.unpack_from('<5Q', self.chunk, address-self.chunk_start)
        elif self.prefix is not None and address + 40 <= ARENA + len(self.prefix):
            result = struct.unpack_from('<5Q', self.prefix, address-ARENA)
        else:
            result = struct.unpack('<5Q', os.pread(self.memory.fileno(), 40, self.base+address))
        if result[0] != PAIR:
            raise ValueError('invalid pair marker')
        return result

    def bytes_value(self, address, expected_length):
        result = bytearray()
        pending = [address]
        visited = 0
        while pending:
            current = pending.pop()
            node = self.pair(current)
            visited += 1
            if node[2] != 0 or node[4] != 1:
                raise ValueError('not a Bytes node')
            content = self.pair(node[3])
            if content[2] != 0:
                raise ValueError('invalid Bytes tag')
            if content[1] == 0 and node[1] == 0:
                continue
            if content[1] == 1 and node[1] == 1 and content[4] == 0 and content[3] < 256:
                result.append(content[3])
            elif content[1] == 2 and content[4] == 1:
                children = self.pair(content[3])
                if children[2] != 1 or children[4] != 1:
                    raise ValueError('invalid Bytes children')
                left, right = self.pair(children[1]), self.pair(children[3])
                if (left[1] + right[1] != node[1] or children[1] >= current
                        or children[3] >= current):
                    raise ValueError('invalid Bytes extents')
                pending.extend([children[3], children[1]])
            else:
                raise ValueError('unsupported Bytes shape')
            if (len(result) > expected_length or len(pending) > 256
                    or visited > max(256, expected_length*8+100)):
                raise ValueError('Bytes decode exceeded expected extent')
        if len(result) != expected_length:
            raise ValueError('Bytes length differs')
        return bytes(result)

    def event(self, name, address, wall, cpu):
        event = {'name': name, 'allocation_pair': (address-ARENA)//40+1,
                 'wall_lower': self.previous_wall, 'wall_upper': wall,
                 'cpu_lower': self.previous_cpu, 'cpu_upper': cpu}
        self.events.append(event)

    def scan(self):
        if self.base is None:
            for line in Path(f'/proc/{self.process_id}/maps').read_text().splitlines():
                start, end = (int(part, 16) for part in line.split()[0].split('-'))
                if end-start == MEMORY_SIZE:
                    self.base = start
                    break
            if self.base is None:
                return
        registers = os.pread(self.memory.fileno(), 16, 0x402500)
        committed, limit = struct.unpack('<QQ', registers)
        if committed < ARENA or limit != MEMORY_SIZE:
            return
        wall, cpu = time.monotonic()-self.started, self.cpu()
        self.committed = committed
        self.chunk_start = self.next_address
        self.chunk = os.pread(self.memory.fileno(), committed-self.next_address,
                              self.base+self.next_address)
        if len(self.chunk) != committed-self.next_address:
            raise ValueError('short heap read')
        if self.prefix is not None:
            self.prefix.extend(self.chunk)
        for index, node in enumerate(struct.iter_unpack('<5Q', self.chunk)):
            address = self.chunk_start + index*40
            if node[0] != PAIR:
                raise ValueError('uncommitted heap node')
            if self.input_root is None and node[2] == 0 and node[4] == 1 and node[1] == len(self.stdin):
                try:
                    decoded = self.bytes_value(address, len(self.stdin))
                except ValueError:
                    continue
                if decoded == self.stdin:
                    self.input_root = address
                    self.event('stdin-tree-ready', address, wall, cpu)
                    self.prefix = None
            if self.input_root is not None and node[1] == 0 and node[2] == 0 and node[4] == 1:
                payload = self.pair(node[3])
                if payload[1] == self.input_root and payload[2] == 1 and payload[4] == 0:
                    cursor = payload[3]
                    if cursor != self.last_input_cursor:
                        if cursor != self.last_input_cursor+1 or cursor > len(self.stdin):
                            raise ValueError(f'nonsequential console cursor {cursor}')
                        self.last_input_cursor = cursor
                        if cursor in self.checkpoint_cursors:
                            self.event(self.checkpoint_cursors[cursor], address, wall, cpu)
            if self.output_root is None and self.last_input_cursor == 1 and node[1] == 2 and node[2] == 0 and node[4] == 1:
                try:
                    decoded = self.bytes_value(address, 2)
                except ValueError:
                    continue
                if decoded == b'L0':
                    self.output_root = address
                    self.output.extend(decoded)
                    self.event('output-prefix', address, wall, cpu)
            elif self.output_root is not None and node[1] == len(self.output)+1 and node[2] == 0 and node[4] == 1:
                tag = self.pair(node[3])
                if tag[1] != 2 or tag[2] != 0 or tag[4] != 1:
                    continue
                children = self.pair(tag[3])
                if children[1] != self.output_root or children[2] != 1 or children[4] != 1:
                    continue
                try:
                    byte = self.bytes_value(children[3], 1)
                except ValueError:
                    continue
                self.output_root = address
                self.output.extend(byte)
                if len(self.output) in self.checkpoint_outputs:
                    self.event(self.checkpoint_outputs[len(self.output)], address, wall, cpu)
        self.next_address = committed
        self.previous_wall, self.previous_cpu = wall, cpu

    def result(self):
        return {'role': 'Read-only heap profiling; public output validates this projection.',
                'stdin_sha256': hashlib.sha256(self.stdin).hexdigest(),
                'input_root': self.input_root, 'last_input_cursor': self.last_input_cursor,
                'output_hex': self.output.hex(), 'events': self.events}
