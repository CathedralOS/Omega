"""Recognize complete native executable file envelopes before profile admission.

This checks declared file tables and payload extents, not instructions or loader
semantics. Actual profile execution and engine observations remain independent.
"""


def integer(image, offset, width, order):
    return int.from_bytes(image[offset:offset + width], order)


def elf_image(image):
    if len(image) < 52 or image[:4] != b"\x7fELF":
        return False
    order = {1: "little", 2: "big"}.get(image[5])
    width = {1: 4, 2: 8}.get(image[4])
    header = 52 if width == 4 else 64
    if order is None or width is None or len(image) < header or image[6] != 1:
        return False
    if integer(image, 16, 2, order) not in {2, 3}:
        return False
    offset = integer(image, 28 if width == 4 else 32, width, order)
    record = integer(image, 42 if width == 4 else 54, 2, order)
    count = integer(image, 44 if width == 4 else 56, 2, order)
    if count == 0 or record < (32 if width == 4 else 56) or offset < header or offset + record * count > len(image):
        return False
    loaded = False
    for index in range(count):
        at = offset + index * record
        start = integer(image, at + (4 if width == 4 else 8), width, order)
        size = integer(image, at + (16 if width == 4 else 32), width, order)
        if size and start + size > len(image):
            return False
        loaded |= integer(image, at, 4, order) == 1 and size > 0
    return loaded


def thin_macho_image(image):
    magic = bytes(image[:4])
    orders = {b"\xce\xfa\xed\xfe": ("little", 28), b"\xcf\xfa\xed\xfe": ("little", 32),
              b"\xfe\xed\xfa\xce": ("big", 28), b"\xfe\xed\xfa\xcf": ("big", 32)}
    if magic not in orders:
        return False
    order, header = orders[magic]
    if len(image) < header or integer(image, 12, 4, order) != 2:
        return False
    count, size = integer(image, 16, 4, order), integer(image, 20, 4, order)
    end = header + size
    if count == 0 or size < count * 8 or end > len(image):
        return False
    at, loaded = header, False
    for _ in range(count):
        if at + 8 > end:
            return False
        command, size = integer(image, at, 4, order), integer(image, at + 4, 4, order)
        if size < 8 or at + size > end:
            return False
        if command in {1, 0x19}:
            width, minimum, field = (4, 56, 32) if command == 1 else (8, 72, 40)
            if size < minimum:
                return False
            offset = integer(image, at + field, width, order)
            length = integer(image, at + field + width, width, order)
            if length and offset + length > len(image):
                return False
            loaded |= length > 0
        at += size
    return at == end and loaded


def fat_macho_image(image):
    magic = bytes(image[:4])
    formats = {b"\xca\xfe\xba\xbe": ("big", 20, 4), b"\xbe\xba\xfe\xca": ("little", 20, 4),
               b"\xca\xfe\xba\xbf": ("big", 32, 8), b"\xbf\xba\xfe\xca": ("little", 32, 8)}
    if len(image) < 8 or magic not in formats:
        return False
    order, record, width = formats[magic]
    count = integer(image, 4, 4, order)
    end = 8 + count * record
    if count == 0 or end > len(image):
        return False
    slices = []
    for index in range(count):
        at = 8 + index * record
        offset, size = integer(image, at + 8, width, order), integer(image, at + 8 + width, width, order)
        if offset < end or size == 0 or offset + size > len(image):
            return False
        if not thin_macho_image(memoryview(image)[offset:offset + size]):
            return False
        slices.append((offset, offset + size))
    slices.sort()
    return all(left[1] <= right[0] for left, right in zip(slices, slices[1:]))


def pe_image(image):
    if len(image) < 64 or image[:2] != b"MZ":
        return False
    at = integer(image, 60, 4, "little")
    if at < 64 or at + 26 > len(image) or image[at:at + 4] != b"PE\0\0":
        return False
    flags, optional = integer(image, at + 22, 2, "little"), integer(image, at + 24, 2, "little")
    minimum = {0x10b: 96, 0x20b: 112}.get(optional)
    size, count = integer(image, at + 20, 2, "little"), integer(image, at + 6, 2, "little")
    sections = at + 24 + size
    if not flags & 2 or flags & 0x2000 or minimum is None or size < minimum or count == 0 or sections + count * 40 > len(image):
        return False
    headers = integer(image, at + 24 + 60, 4, "little")
    if headers < sections + count * 40 or headers > len(image):
        return False
    loaded = False
    for index in range(count):
        section = sections + index * 40
        size, offset = integer(image, section + 16, 4, "little"), integer(image, section + 20, 4, "little")
        if size and (offset < headers or offset + size > len(image)):
            return False
        loaded |= size > 0
    return loaded


def executable_image(image):
    return elf_image(image) or thin_macho_image(image) or fat_macho_image(image) or pe_image(image)
