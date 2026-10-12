"""Original UnitClass::Should_Crush_It 0x007438F0 on scratch objects.

The body executes natively, with its Sqrt_Approx 0x004CAC40 and ftol 0x007C5F00, on a
scratch Unit, its type and house, a scratch source and a scratch Rules. It is the test
UnitClass::ReceiveDamage's computer-owner response (0x00738583) asks before a unit charges
the object that hit it: a crusher, a crushable source within [CombatDamage] Crush=, an
owner not on Easy and with an IQ of at least [IQ] AutoCrush=, and a source not disguised
to that owner. Rows cover each gate and the distance threshold, in 2D and 3D.

A row records the returned byte and the ordered names of the supplied calls. Supplied:
    ability       TechnoClass::HasWeaponAbility 0x0070D0D0 (ECX = the unit, arg 0x11)
    crushable     ObjectClass::Is_Crushable_By 0x005F6CD0 (ECX = the source, arg the unit)
    human         HouseClass::IsControlledByHuman 0x0050B730 (ECX = the owner)
    source_coords source vt+0x48 GetCoords
    unit_coords   unit vt+0x48 GetCoords
    disguised     source vt+0xC8 IsDisguisedTo (arg the owner)
Every row's owner is a computer's: the only caller turns a human's away first, so the
human arm (PlayerAutoCrush= Rules+0x17EB, AutoCrush= type +0xD2D) is not exercised.

Run python -m tools.unit_crush_oracle --check (or explicit --write).
"""
import struct
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP,
                               UC_X86_REG_FPCW)

from tools.native_oracle import (load_image, run_checked, SCRATCH, SCRATCH_SIZE, STACK_BASE,
                                 STACK_SIZE, RET_MAGIC, NATIVE_FPCW, finish_vectors, provenance)

SHOULD_CRUSH = 0x7438F0
ABILITY = 0x70D0D0
CRUSHABLE = 0x5F6CD0
HUMAN = 0x50B730
RULES_POINTER = 0x8871E0

UNIT, SOURCE = SCRATCH, SCRATCH + 0x800
UNIT_VTABLE, SOURCE_VTABLE = SCRATCH + 0x1000, SCRATCH + 0x1400
UNIT_TYPE, HOUSE, RULES = SCRATCH + 0x2000, SCRATCH + 0x3000, SCRATCH + 0x4000
UNIT_COORDS, SOURCE_COORDS, DISGUISED = SCRATCH + 0x6000, SCRATCH + 0x6100, SCRATCH + 0x6200
SP = STACK_BASE + STACK_SIZE - 0x1000


def dword(value):
    return struct.pack('<I', value & 0xFFFFFFFF)


def coords_stub(xyz):
    """GetCoords(CoordStruct* out): fill *out, return it, RET 4."""
    code = b'\x8B\x44\x24\x04'  # mov eax, [esp+4]
    for offset, value in zip((0, 4, 8), xyz):
        code += (b'\xC7\x00' if offset == 0 else b'\xC7\x40' + bytes([offset])) + dword(value)
    return code + b'\xC2\x04\x00'


class Fixture:
    def __init__(self):
        self.u = Uc(UC_ARCH_X86, UC_MODE_32)
        load_image(self.u)
        self.u.mem_map(SCRATCH, SCRATCH_SIZE)
        self.u.mem_map(STACK_BASE, STACK_SIZE)
        self.u.mem_map(RET_MAGIC, 0x1000)
        self.calls = []
        self.row = {}
        self.u.hook_add(UC_HOOK_CODE, self.observe)

    def ret(self, pop, value):
        u = self.u
        sp = u.reg_read(UC_X86_REG_ESP)
        back = struct.unpack('<I', u.mem_read(sp, 4))[0]
        u.reg_write(UC_X86_REG_EAX, value)
        u.reg_write(UC_X86_REG_ESP, sp + 4 + pop)
        u.reg_write(UC_X86_REG_EIP, back)

    def argument(self):
        sp = self.u.reg_read(UC_X86_REG_ESP)
        return struct.unpack('<I', self.u.mem_read(sp + 4, 4))[0]

    def observe(self, u, pc, _size, _data):
        ecx = u.reg_read(UC_X86_REG_ECX)
        if pc == ABILITY:
            assert ecx == UNIT and self.argument() == 0x11
            self.calls.append('ability')
            self.ret(4, int(self.row['ability']))
        elif pc == CRUSHABLE:
            assert ecx == SOURCE and self.argument() == UNIT
            self.calls.append('crushable')
            self.ret(4, int(self.row['crushable']))
        elif pc == HUMAN:
            assert ecx == HOUSE
            self.calls.append('human')
            self.ret(0, 0)
        elif pc == UNIT_COORDS:
            self.calls.append('unit_coords')
        elif pc == SOURCE_COORDS:
            self.calls.append('source_coords')
        elif pc == DISGUISED:
            assert ecx == SOURCE and self.argument() == HOUSE
            self.calls.append('disguised')

    def execute(self, row):
        u = self.u
        self.row = row
        u.mem_write(SCRATCH, bytes(SCRATCH_SIZE))
        u.mem_write(UNIT, dword(UNIT_VTABLE))
        u.mem_write(UNIT + 0x6C4, dword(UNIT_TYPE))
        u.mem_write(UNIT + 0x21C, dword(HOUSE))
        u.mem_write(UNIT_VTABLE + 0x48, dword(UNIT_COORDS))
        u.mem_write(UNIT_COORDS, coords_stub(row['unit']))
        u.mem_write(SOURCE, dword(SOURCE_VTABLE))
        u.mem_write(SOURCE_VTABLE + 0x48, dword(SOURCE_COORDS))
        u.mem_write(SOURCE_COORDS, coords_stub(row['source']))
        u.mem_write(SOURCE_VTABLE + 0xC8, dword(DISGUISED))
        u.mem_write(DISGUISED, b'\xB8' + dword(int(row['disguised'])) + b'\xC2\x04\x00')
        u.mem_write(UNIT_TYPE + 0xD28, bytes([int(row['crusher'])]))
        u.mem_write(HOUSE + 0x184, dword(row['difficulty']))
        u.mem_write(HOUSE + 0x24C, dword(row['iq']))
        u.mem_write(RULES + 0x1728, dword(row['crush']))
        u.mem_write(RULES + 0x1448, dword(row['iq_auto_crush']))
        u.mem_write(RULES_POINTER, dword(RULES))
        self.calls = []
        u.mem_write(SP, dword(RET_MAGIC))
        u.mem_write(SP + 4, dword(SOURCE))
        u.reg_write(UC_X86_REG_ESP, SP)
        u.reg_write(UC_X86_REG_ECX, UNIT)
        u.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
        run_checked(u, SHOULD_CRUSH, RET_MAGIC, count=20000)
        assert u.reg_read(UC_X86_REG_ESP) == SP + 8
        return dict(admits=bool(u.reg_read(UC_X86_REG_EAX) & 0xFF), calls=self.calls)


# A Rhino at the centre of cell (10, 10) and a GI a cell east of it, both on the
# ground, for a Normal computer house of the skirmish IQ, on stock Crush= and AutoCrush=.
BASE = dict(crusher=True, ability=False, crushable=True, unit=[2688, 2688, 0],
            source=[2944, 2688, 0], crush=460, difficulty=1, iq=5, iq_auto_crush=2,
            disguised=False)


def inputs():
    rows = [dict(name='base')]
    rows += [dict(name='not_crusher', crusher=False),
             dict(name='veteran_crusher', crusher=False, ability=True),
             dict(name='not_crushable', crushable=False)]
    # The reach: offsets from the unit around stock Crush= 460 and the constructor's 0x180,
    # along an axis, diagonally and in 3D, where Sqrt_Approx and the chop decide.
    offsets = [(459, 0, 0), (460, 0, 0), (461, 0, 0), (-460, 0, 0), (0, -461, 0),
               (325, 325, 0), (326, 325, 0), (326, 326, 0), (0, 0, 460), (0, 0, 461),
               (265, 265, 265), (266, 265, 265), (266, 266, 266), (200, 380, 90),
               (201, 380, 90), (100, 100, 440)]
    for dx, dy, dz in offsets:
        rows.append(dict(name=f'offset_{dx}_{dy}_{dz}',
                         source=[2688 + dx, 2688 + dy, dz]))
    for dx in (383, 384, 385):
        rows.append(dict(name=f'reach_0x180_offset_{dx}', crush=0x180,
                         source=[2688 + dx, 2688, 0]))
    rows += [dict(name=f'difficulty_{d}', difficulty=d) for d in (0, 2)]
    rows += [dict(name='iq_below', iq=1), dict(name='iq_equal', iq=2),
             dict(name='auto_crush_above_iq', iq_auto_crush=6)]
    rows.append(dict(name='disguised', disguised=True))
    return [{**BASE, **row} for row in rows]


def generate():
    fixture = Fixture()
    return [dict(input=row, output=fixture.execute(row)) for row in inputs()]


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=lambda: provenance(
        scope='Original UnitClass::Should_Crush_It 0x007438F0 for a computer-owned unit on '
              'scratch objects: the crusher, crushable, Crush= distance (Sqrt_Approx and ftol '
              'executed natively), Easy, AutoCrush= IQ and disguise gates.',
        entry_points={'should_crush_it': SHOULD_CRUSH},
        assumptions=['Unit +0x6C4 type (+0xD28 Crusher), +0x21C house (+0x184 difficulty, '
                     '+0x24C CurrentIQ); Rules at 0x008871E0 (+0x1728 Crush, +0x1448 IQ '
                     'AutoCrush); the native FPU control word 0x0E7F.',
                     'Every owner is a computer\'s; the human arm is not exercised.'],
        substitutions=['HasWeaponAbility 0x0070D0D0, Is_Crushable_By 0x005F6CD0 and '
                       'IsControlledByHuman 0x0050B730 (always 0) are entry-hooked returns; '
                       'both GetCoords (vt+0x48) and the source\'s IsDisguisedTo (vt+0xC8) '
                       'are scratch stubs returning the row\'s values.'],
    ))
