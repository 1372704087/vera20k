"""Three bounded Ares supplemental CSF histories on synthetic file bytes.

The installed enumeration/header/override hooks and original ReadFile execute.
File/heap/formatting services and initialized base-table storage are fixtures;
no host CSF decoder or merge algorithm supplies the observed results.
"""
import argparse
import hashlib
import os
from pathlib import Path
import struct

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP,
    UC_X86_REG_EIP, UC_X86_REG_EFLAGS,
)
from tools import native_oracle as native

ARES_SHA256 = '93ce890f387a6c1b3240a2dd7b674f25fec5a0e96f0b0d0306796b2edf7b5b83'
REGISTERS = [UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_EBP,
             UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_EDX,
             UC_X86_REG_ECX, UC_X86_REG_EAX]


def csf(entries, language=0, version=3, counts=None):
    n = len(entries) if counts is None else counts
    data = struct.pack('<6I', 0x43534620, version, n, n, 0, language & 0xffffffff)
    for key, value in entries:
        raw = value.encode('utf-16le')
        data += struct.pack('<3I', 0x4c424c20, 1, len(key)) + key.encode('ascii')
        data += struct.pack('<2I', 0x53545220, len(raw)//2) + bytes(b ^ 255 for b in raw)
    return data


class CsfProbe:
    def __init__(self, ares, files, language=0):
        u = self.u = Uc(UC_ARCH_X86, UC_MODE_32)
        native.map_pe32_image(u, ares, image_base=0x10000000,
                             image_size=0xe0000, header_size=0x400)
        native.map_pe32_image(u, native.image_bytes(), image_base=native.IMAGE_BASE,
                             image_size=native.IMAGE_SIZE, header_size=0x1000)
        u.mem_map(0x20000000, 0x400000)
        u.mem_map(0x30000000, 0x1000)
        self.heap = 0x20010000
        self.files = files
        self.open_files = {}
        self.attempts = []
        self.accepted = []
        self.transport = None
        self.vtable = self.alloc(0x40)
        for offset, service in [(0, 0x30000100), (0x14, 0x30000110),
                                (0x1c, 0x30000120), (0x24, 0x30000130)]:
            self.write(self.vtable + offset, service)
        self.labels = self.alloc(20000*40)
        self.values = self.alloc(20000*4)
        self.extra = self.alloc(20000*4)
        for at, value in [(0xb1cf74, self.labels), (0xb1cf78, self.values),
                          (0xb1cf7c, self.extra), (0xb1cf6c, 2),
                          (0xb1cf70, 2), (0x845728, language), (0xb1cf80, 1)]:
            self.write(at, value)
        for index, (key, value) in enumerate([('A', 'base'), ('KEEP', 'keep')]):
            u.mem_write(self.labels + index*40, key.encode() + b'\0')
            self.write(self.labels + index*40 + 0x20, 1)
            self.write(self.labels + index*40 + 0x24, index)
            ptr = self.alloc(2*len(value)+2)
            u.mem_write(ptr, value.encode('utf-16le') + b'\0\0')
            self.write(self.values + index*4, ptr)
        self.original = [(0x10001000, bytes(u.mem_read(0x10001000, 0x92000))),
                         (0x401000, bytes(u.mem_read(0x401000, 0x3e1000)))]
        u.hook_add(UC_HOOK_CODE, self.hook)

    def alloc(self, n):
        at = self.heap
        self.heap += (n+15) & ~15
        assert self.heap < 0x20400000
        return at

    def read(self, at):
        return struct.unpack('<I', self.u.mem_read(at, 4))[0]

    def write(self, at, value):
        self.u.mem_write(at, struct.pack('<I', value & 0xffffffff))

    def text(self, text):
        ptr = self.alloc(len(text)+1)
        self.u.mem_write(ptr, text.encode('ascii') + b'\0')
        return ptr

    def string(self, at):
        return bytes(self.u.mem_read(at, 4096)).split(b'\0', 1)[0].decode('ascii')

    def hook(self, u, pc, _size, _data):
        if pc == 0x30000000:
            u.emu_stop()
            return
        if pc in (0x734a5f, 0x734a97):
            assert self.transport is None
            frame = self.alloc(0x28)
            self.write(frame, pc)
            self.write(frame+4, u.reg_read(UC_X86_REG_EFLAGS))
            for index, reg in enumerate(REGISTERS):
                self.write(frame+8+4*index, u.reg_read(reg))
            self.transport = pc, frame
            self.write(0x20007000, 0x30000010)
            self.write(0x20007004, frame)
            u.reg_write(UC_X86_REG_ESP, 0x20007000)
            u.reg_write(UC_X86_REG_EIP, 0x10060c80 if pc == 0x734a5f else 0x10060ee0)
            return
        if pc == 0x30000010:
            original, frame = self.transport
            continuation = u.reg_read(UC_X86_REG_EAX)
            for index, reg in enumerate(REGISTERS):
                u.reg_write(reg, self.read(frame+8+4*index))
            u.reg_write(UC_X86_REG_EFLAGS, self.read(frame+4))
            if original == 0x734a5f:
                assert continuation == 0
                u.reg_write(UC_X86_REG_EDI, 0xb1bf38)
                continuation = 0x734a64
            self.transport = None
            u.reg_write(UC_X86_REG_EIP, continuation)
            return
        sp = u.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.read(sp+4+4*i)
        owner = u.reg_read(UC_X86_REG_ECX)
        cleanup = 0
        if pc in (0x10002ba0, 0x4739f0):
            name = self.string(arg(0))
            self.open_files[owner] = [name, 0]
            self.write(owner, self.vtable)
            result, cleanup = owner, 4
        elif pc in (0x30000110, 0x473c50):
            name = self.open_files[owner][0]
            self.attempts.append(name)
            result, cleanup = int(name in self.files), 4
        elif pc in (0x30000120, 0x473d10):
            result, cleanup = 1, 4
        elif pc in (0x30000130, 0x473b10):
            target, count = arg(0), arg(1)
            name, offset = self.open_files[owner]
            raw = self.files[name][offset:offset+count]
            u.mem_write(target, raw)
            self.open_files[owner][1] += len(raw)
            result, cleanup = len(raw), 8
        elif pc == 0x30000100:
            result, cleanup = 0, 4
        elif pc in (0x43ae50, 0x431b80, 0x100033c0):
            result = 0
        elif pc == 0x7c8e17:
            result = self.alloc(arg(0))
        elif pc == 0x7c8d20:
            a, b = self.string(arg(0)).lower(), self.string(arg(1)).lower()
            result = (a>b)-(a<b)
        elif pc == 0x10003a40:
            target, capacity, pattern = arg(0), arg(1), self.string(arg(3))
            value = arg(4)
            raw = (pattern % (value if '%02d' in pattern else self.string(value))).encode()
            assert len(raw) < capacity
            u.mem_write(target, raw+b'\0')
            result = len(raw)
        elif pc == 0x100609c0:
            self.accepted.append(self.string(owner))
            return  # original ReadFile executes
        else:
            return
        u.reg_write(UC_X86_REG_EAX, result & 0xffffffff)
        u.reg_write(UC_X86_REG_ESP, sp+4+cleanup)
        u.reg_write(UC_X86_REG_EIP, self.read(sp))

    def run(self):
        frame = self.alloc(0x28)
        self.write(0x2000f000, 0x30000000)
        self.write(0x2000f004, frame)
        self.u.reg_write(UC_X86_REG_ESP, 0x2000f000)
        native.run_checked(self.u, 0x10060df0, 0x30000000, count=1000000,
                           context={'scope': 'Ares supplemental CSF'})
        assert all(bytes(self.u.mem_read(at, len(raw))) == raw for at, raw in self.original)
        values = {}
        for index in range(self.read(0xb1cf6c)):
            label = self.labels + index*40
            ptr = self.read(self.values + 4*self.read(label+0x24))
            if ptr:
                raw = bytearray()
                while bytes(self.u.mem_read(ptr+len(raw), 2)) != b'\0\0':
                    raw += self.u.mem_read(ptr+len(raw), 2)
                text = raw.decode('utf-16le')
            else:
                text = None
            values[self.string(label)] = text
        # Exists is called again when ReadFile opens an admitted file.
        attempts = self.attempts.copy()
        for name in self.accepted:
            attempts.remove(name)
        return dict(attempts=attempts, accepted=self.accepted,
                    base_language=self.read(0x845728), entries=values)


def generate(ares):
    rows = []
    files = {'ares.csf': csf([('A', 'generic')], language=9),
             'ares_us.csf': csf([('a', 'locale'), ('NEW', 'new')]),
             'stringtable00.csf': csf([('A', 'zero')], language=-1),
             'stringtable01.csf': csf([('A', 'one')])}
    rows.append(dict(name='ordered_overrides', result=CsfProbe(ares, files).run()))
    files = {'ares.csf': csf([('A', 'generic')], language=9),
             'ares_us.csf': csf([('A', 'wrong')], language=9),
             'stringtable00.csf': csf([('A', 'zero_counts')], counts=0),
             'stringtable01.csf': csf([('A', 'bad_version')], version=1),
             'stringtable02.csf': b'bad',
             'stringtable03.csf': b'BAD!' + csf([('A', 'bad_magic')])[4:],
             'stringtable04.csf': csf([('A', 'signed_version')], version=0x80000000)}
    rows.append(dict(name='language_and_header_admission', result=CsfProbe(ares, files).run()))
    broken = csf([('A', 'clear_old')])
    broken = broken[:24+12+1] + struct.pack('<I', 0)
    files = {'stringtable00.csf': csf([('NEW', 'committed')]) + broken[24:]}
    rows.append(dict(name='partial_body_keeps_prior_writes', result=CsfProbe(ares, files).run()))
    return dict(schema_version=1, controls=rows)


def metadata():
    result = native.provenance(
        scope='Three native Ares3.0 supplemental CSF histories, synthetic files only.',
        assumptions=['Initialized sorted two-label base table and base language0 are explicit inputs.',
                     'Actual enumeration10060DF0, admission100608C0, override10060C80, index10060EE0 and originalReadFile734990 execute; Syringe register transport is supplied without patching code.',
                     'File bytes/read cursors, heap allocation/free, ASCII strcmpi and printf formatting are supplied services. Native record parser, normalization, label search and sort execute.',
                     'No mod assets, physical MIX priority, arbitrary duplicate new labels, multiple values, overflow or whole startup equivalence.'],
        substitutions=['CCFile services, initialized base storage, heap/ASCII CRT leaves and Syringe register transport only; no decoded labels or merge results injected.'],
        entry_points=dict(extra=0x10060df0, admission=0x100608c0,
                          override=0x10060c80, index=0x10060ee0, reader=0x734990))
    result['ares_sha256'] = ARES_SHA256
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ares-dll', type=Path, default=os.environ.get('VERA20K_ARES_DLL'))
    parser.add_argument('--gamemd-exe', type=Path)
    args, remaining = parser.parse_known_args()
    if not args.ares_dll:
        parser.error('Set VERA20K_ARES_DLL or --ares-dll')
    if args.gamemd_exe:
        os.environ['VERA20K_GAMEMD_EXE'] = str(args.gamemd_exe)
    ares = args.ares_dll.read_bytes()
    if hashlib.sha256(ares).hexdigest() != ARES_SHA256:
        raise native.OracleError('Unrecognized Ares image')
    native.finish_vectors(lambda: generate(ares), Path(__file__).with_suffix('.json'),
                          provenance=metadata, argv=remaining,
                          source_paths={'producer': Path(__file__), 'runner': Path(native.__file__)})


if __name__ == '__main__':
    main()
