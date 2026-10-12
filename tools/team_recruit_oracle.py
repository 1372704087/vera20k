"""Native references for a computer team's recruitment, centre and scripts.

Run python -m tools.team_recruit_oracle --check (or explicit --write).
Rust consumers: src/sim/team_script_vm/recruit_oracle_tests.rs,
src/sim/building_entry_tests.rs (the seeks) and src/sim/docking/bunker_link.rs
(the tank bunker admission).

Sections, each executed in a fresh emulator per case:
- recalc: TeamClass::Recalc 0x6EA3E0 with the TaskForce total 0x6E8160 (the
  full and under-strength bytes, a Reinforce= TeamType's third, the empty
  team's destruction).
- center: TeamClass::Calc_Center 0x6EAEE0 off action 10 (the member tests,
  the GuardSlower= double count, the mean, the closest member by 0x5F6500
  and the head fallback).
- recruit: TeamClass::Recruit 0x6EAA90 over the Infantry, Unit and Aircraft
  arrays (the group filter and penalty, the 0x5F6560 key, the Unit search's
  house and type test).
- distance: ObjectClass::Distance 0x5F6360 (Sqrt_Approx 0x4CAC40, ftol
  0x7C5F00, a building's foundation deduction over every foundation).
- guard: TeamClass::AI's action 5 timer 0x6E97CE..0x6E97EC.
- regroup: action 54 0x6EFA10 up to its FNPC call 0x6EFC3A (the enemy's
  base centre and the atan2 facing 0x4CAE30, or the RandomRanged(0, 255)
  facing; the sin/cos step 0x4CACB0/0x4CAD00 and the cell quotients).
- own_building: FindOwnBuilding 0x6EEEA0.
- target_building: Find_Best_Target_Building 0x6EEBD0, actions 46/47's
  pick over BuildingClass::Array (the type, enemy, IsAlliedWith 0x4F9A50 and
  MultiplayPassive tests, the GetCoords cell each threat read takes, the
  Sqrt_Approx distance, the two bests and OnlyTargetHouseEnemy=).
- gather: action 53 0x6EF700 with `first` up to its FNPC call 0x6EF98A, or
  to its return when it finishes at once (the enemy's and own base centres,
  the leader's location without a base, the same atan2/sin/cos step as
  action 54 from the enemy's centre).
- quarry: Quarry_To_Threat 0x645BB0 and its jump table 0x645BF8.
- attack_cadence: Coordinate_Attack's signed Frame % 8 == 4 test
  0x6EB59A..0x6EB5AE.
- bio_reactor: FootClass::EnterBioReactor 0x4DFB70, action 62's Foot seek
  (the house's absorber list walked from its end, the CAN_LOAD answers, the
  Sqrt_Approx distance and its ties, the latch +0x68F, the radio-contact and
  pending-entry test, the Enter and destination calls).
- battle_bunker: FootClass::EnterBattleBunker 0x4DFCB0, action 63's Foot
  seek (the house's occupiable list walked from its end, the distance, the
  CanBeOccupiedBy 0x457CE0 answers asked only of a nearer building, with the
  Foot or NULL, the latch +0x690, the NavCom-and-Capture test, the
  destination and Capture calls).
- tank_bunker: FootClass::EnterTankBunker 0x4DFF40, action 61's Foot seek
  (the admission 0x70FB50 run as original, each of its tests refusing
  alone; the house's tank bunker list walked from its end, the distance and
  its ties, the first radio contact 0x65AD30 and occupant +0x2E4 read only
  of a nearer bunker; the destination, HELLO and Enter calls).
- load: action 14 0x6ED200 (the TaskForce's aircraft test, the transport
  pick, the radio-contact wait, the join-up, one Enter order per update,
  the completion and the TransportsReturnOnUnload= archive).
- wait_loaded: action 43's inline test 0x6E9C22..0x6E9C5C.
- unload: action 8 0x6EF110 (the join-up, the Unload orders with their
  building and mission tests, then the sort into transports and others by
  the argument and TransportsReturnOnUnload=).

Control flow the rows do not vary (Calc_Center's action-10 branch, the
TeamType waypoint origin, the recruited unit's passengers, action 54's leader
loop) rests on instruction reading; see src/sim/team_script_vm/.
"""
from pathlib import Path
import random
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP,
                               UC_X86_REG_FPCW)

from tools.ai_base_building_oracle import FAKE, FRAME, RULES, STUBS, Emu, u32
from tools.native_oracle import (NATIVE_FPCW, RET_MAGIC, STACK_BASE, STACK_SIZE,
                                 finish_vectors, provenance, run_checked)

TEAM = FAKE + 0x100000
TEAM_VTABLE = FAKE + 0x100800
TEAM_TYPE = FAKE + 0x101000
TASK_FORCE = FAKE + 0x102000
SCRIPT = FAKE + 0x103000
HOUSES = FAKE + 0x104000
HOUSE_SIZE = 0x6000
TYPES = FAKE + 0x120000
TYPE_SIZE = 0x2000
TYPE_VTABLE = FAKE + 0x160000
OBJECTS = FAKE + 0x200000
OBJECT_SIZE = 0x800
OBJECT_VTABLE = FAKE + 0x300000
BUILDING_VTABLE = FAKE + 0x300800
BUILDING_TYPES = FAKE + 0x310000
ARRAYS = FAKE + 0x340000
CELL = FAKE + 0x350000

STUB_RTTI = STUBS + 0x100
STUB_TECHNO_TYPE = STUBS + 0x110
STUB_WEIGHT = STUBS + 0x120
STUB_CAN_ENTER = STUBS + 0x130
STUB_SET_TARGET = STUBS + 0x140
STUB_DELETE = STUBS + 0x150
STUB_BUILDING_COORDS = STUBS + 0x160
STUB_TRANSMIT = STUBS + 0x170
STUB_QUEUE_MISSION = STUBS + 0x180
STUB_ASSIGN_DESTINATION = STUBS + 0x190

GET_COORDS = 0x5F65A0
SCENARIO_INIT = 0xA8E7AC
HOUSE_ARRAY = 0xA8022C
INFANTRY = (0xA83DEC, 0xA83DF8)
UNITS = (0x8B410C, 0x8B4118)
AIRCRAFT = (0xA8E394, 0xA8E3A0)

RTTI_UNIT, RTTI_AIRCRAFT, RTTI_BUILDING, RTTI_INFANTRY = 1, 2, 6, 15
TYPE_RTTI = {'infantry': 0x10, 'unit': 0x28, 'aircraft': 0x03}
CLASS_ARRAY = {'infantry': INFANTRY, 'unit': UNITS, 'aircraft': AIRCRAFT}


def i32(value):
    return struct.unpack('<i', u32(value))[0]


class TeamEmu(Emu):
    """A fresh image with one team, its TeamType and TaskForce, and objects
    whose GetCoords (vt+0x48) is the original 0x5F65A0."""

    def __init__(self):
        super().__init__()
        self.rtti = {}
        self.techno_type = {}
        self.weight = {}
        self.can_enter = {}
        for slot, stub in ((0x2C, STUB_RTTI), (0x84, STUB_TECHNO_TYPE), (0x4E0, STUB_WEIGHT),
                           (0x1AC, STUB_CAN_ENTER), (0x3C8, STUB_SET_TARGET)):
            self.write32(OBJECT_VTABLE + slot, stub)
            self.write32(BUILDING_VTABLE + slot, stub)
        self.write32(OBJECT_VTABLE + 0x48, GET_COORDS)
        self.write32(BUILDING_VTABLE + 0x48, STUB_BUILDING_COORDS)
        self.write32(TYPE_VTABLE + 0x2C, STUB_RTTI)
        self.write32(TEAM_VTABLE + 0x20, STUB_DELETE)
        self.write32(TEAM, TEAM_VTABLE)
        self.write32(TEAM + 0x24, TEAM_TYPE)
        self.write32(TEAM + 0x28, SCRIPT)
        self.write32(TEAM_TYPE + 0xE4, TASK_FORCE)
        self.hook(STUB_RTTI, lambda e: e.rtti[e.uc.reg_read(UC_X86_REG_ECX)], 0)
        self.hook(STUB_TECHNO_TYPE, lambda e: e.techno_type[e.uc.reg_read(UC_X86_REG_ECX)], 0)
        self.hook(STUB_WEIGHT, lambda e: e.weight[e.uc.reg_read(UC_X86_REG_ECX)], 0)
        self.hook(STUB_CAN_ENTER, self.can_enter_stub, 20)
        self.hook(STUB_SET_TARGET, lambda e: e.events.append(
            ['set_target', self.index_of(e.uc.reg_read(UC_X86_REG_ECX)), e.arg(0)]), 4)
        self.hook(STUB_DELETE, lambda e: e.events.append(['delete', e.arg(0)]), 4)

    def can_enter_stub(self, e):
        member = e.uc.reg_read(UC_X86_REG_ECX)
        e.events.append(['can_enter', self.index_of(member), e.arg(0) == CELL,
                         i32(e.arg(1)), i32(e.arg(2)), e.arg(3), e.arg(4)])
        return e.can_enter[member]

    @staticmethod
    def index_of(address):
        return (address - OBJECTS) // OBJECT_SIZE

    def write8(self, address, value):
        self.uc.mem_write(address, bytes([value & 0xFF]))

    def read8(self, address):
        return self.uc.mem_read(address, 1)[0]

    def object(self, index, *, xyz=(0, 0, 0), vtable=OBJECT_VTABLE, rtti=RTTI_INFANTRY):
        address = OBJECTS + index * OBJECT_SIZE
        self.write32(address, vtable)
        for offset, value in zip((0x9C, 0xA0, 0xA4), xyz):
            self.write32(address + offset, value)
        self.rtti[address] = rtti
        return address

    def techno_type_at(self, index, *, rtti=None, passengers=0, naval=False):
        address = TYPES + index * TYPE_SIZE
        self.write32(address, TYPE_VTABLE)
        self.write32(address + 0x5E0, passengers)
        self.write8(address + 0xCCE, int(naval))
        if rtti is not None:
            self.rtti[address] = rtti
        return address

    def start(self, entry, end, *, ecx, args=()):
        """Run `entry` (thiscall) from a fresh stack to `end` (one address or
        several); answer the address reached."""
        uc = self.uc
        sp = STACK_BASE + STACK_SIZE - 0x1000
        for value in reversed(args):
            sp -= 4
            uc.mem_write(sp, u32(value))
        sp -= 4
        uc.mem_write(sp, u32(RET_MAGIC))
        uc.reg_write(UC_X86_REG_ESP, sp)
        uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
        uc.reg_write(UC_X86_REG_ECX, ecx)
        return run_checked(uc, entry, end)


# ---------------------------------------------------------------- recalc

RECALC = 0x6EA3E0
TEAM_BYTES = (0x76, 0x78, 0x79, 0x7A, 0x7B, 0x7D, 0x7E)


def recalc_row(*, entries, total, reinforce, guard_slower, has_been, under):
    emu = TeamEmu()
    emu.write32(TASK_FORCE + 0x9C, len(entries))
    for slot, amount in enumerate(entries):
        emu.write32(TASK_FORCE + 0xA4 + slot * 8, amount)
    emu.write8(TEAM_TYPE + 0xAB, int(reinforce))
    emu.write8(TEAM_TYPE + 0xA7, int(guard_slower))
    emu.write32(TEAM + 0x48, total)
    emu.write32(TEAM + 0x34, CELL)
    for offset, value in ((0x76, 0), (0x78, has_been), (0x79, 0), (0x7A, under), (0x7B, 0),
                          (0x7D, 1), (0x7E, 1), (0x82, 0)):
        emu.write8(TEAM + offset, int(value))
    result = emu.invoke(RECALC, ecx=TEAM) & 0xFF
    return dict(entries=entries, total=total, reinforce=reinforce, guard_slower=guard_slower,
                has_been=has_been, under=under, result=result,
                bytes={f'{offset:#x}': emu.read8(TEAM + offset) for offset in TEAM_BYTES},
                zone_cleared=emu.read32(TEAM + 0x34) == 0,
                deleted=any(event[0] == 'delete' for event in emu.events))


def recalc():
    rows = []
    for entries in ([], [0], [1], [2], [3], [2, 1], [1, 1, 1], [4, 2], [7], [9], [5, 4, 3],
                    [-1, 3], [-9]):
        desired = sum(entries)
        totals = sorted({0, 1, 2, 3, desired - 1, desired, desired + 1} & set(range(0, 13)))
        for total in totals:
            for reinforce in (False, True):
                for has_been in (False, True):
                    for under in (False, True):
                        rows.append(recalc_row(entries=entries, total=total,
                                               reinforce=reinforce,
                                               guard_slower=(total + len(entries)) % 2 == 1,
                                               has_been=has_been, under=under))
    return rows


# ---------------------------------------------------------------- center

CALC_CENTER = 0x6EAEE0
CURRENT_ACTION = 0x691500
GET_CELL_AT = 0x565730


def center_row(*, members, focus=None, guard_slower=False, action=0):
    """`members`: dicts of location, flags, the weight and cell answers."""
    emu = TeamEmu()
    emu.write8(TEAM_TYPE + 0xA7, int(guard_slower))

    def current(e):
        out = e.arg(0)
        e.write32(out, action)
        e.write32(out + 4, 0)
        return out
    emu.hook(CURRENT_ACTION, current, 4)

    def cell_at(e):
        coord = e.arg(0)
        e.events.append(['cell_at', [i32(e.read32(coord + offset)) for offset in (0, 4, 8)]])
        return CELL
    emu.hook(GET_CELL_AT, cell_at, 4)
    previous = None
    for index, member in enumerate(members):
        address = emu.object(index, xyz=member['xyz'],
                             rtti=RTTI_AIRCRAFT if member['aircraft'] else RTTI_INFANTRY)
        emu.write8(address + 0x90, int(member['active']))
        emu.write32(address + 0x6C, member['health'])
        emu.write8(address + 0x81, int(member['limbo']))
        emu.write8(address + 0x689, int(member['initiated']))
        emu.write8(address + 0x3D5, int(member['in_playfield']))
        emu.techno_type[address] = emu.techno_type_at(index, passengers=member['passengers'],
                                                      naval=member['naval'])
        emu.weight[address] = int(member['weight'])
        emu.can_enter[address] = member['can_enter']
        if previous is None:
            emu.write32(TEAM + 0x54, address)
        else:
            emu.write32(previous + 0x5D8, address)
        previous = address
    if focus is not None:
        emu.write32(TEAM + 0x40, emu.object(len(members), xyz=focus))
    outputs = FAKE + 0x360000
    emu.write32(outputs, 0x1234)
    emu.write32(outputs + 4, 0x1234)
    emu.invoke(CALC_CENTER, ecx=TEAM, args=(outputs, outputs + 4))
    center, closest = emu.read32(outputs), emu.read32(outputs + 4)

    def reference(address):
        if address == 0:
            return None
        if address == CELL:
            return 'cell'
        return TeamEmu.index_of(address)
    return dict(members=members, focus=focus, guard_slower=guard_slower,
                center=reference(center), closest=reference(closest), events=emu.events)



def center_member(rng, *, far=False):
    span = 0x7FFF00 if far else 0x6000
    return dict(xyz=[rng.randrange(0x100, span), rng.randrange(0x100, span),
                     rng.randrange(0, 0x400)],
                active=rng.random() > 0.1, health=rng.choice((0, 1, 100, 100, 100)),
                limbo=rng.random() < 0.1, initiated=rng.random() > 0.3,
                aircraft=rng.random() < 0.15, in_playfield=rng.random() > 0.1,
                passengers=rng.choice((0, 0, 0, 5)), naval=rng.random() < 0.3,
                weight=rng.random() < 0.5, can_enter=rng.choice((0, 0, 0, 1, 2)))


def center():
    rng = random.Random(0x6EAEE0)
    rows = []
    for case in range(120):
        far = case % 4 == 0
        members = [center_member(rng, far=far) for _ in range(rng.randrange(1, 7))]
        focus = None if case % 3 == 0 else [rng.randrange(0, 0x7FFF00 if far else 0x6000),
                                            rng.randrange(0, 0x7FFF00 if far else 0x6000),
                                            0]
        rows.append(center_row(members=members, focus=focus, guard_slower=case % 2 == 1))
    # Equal distances keep the first; a zero distance is replaced by the next.
    same = dict(active=True, health=100, limbo=False, initiated=True, aircraft=False,
                in_playfield=True, passengers=0, naval=False, weight=False, can_enter=0)
    rows.append(center_row(members=[dict(same, xyz=[1000, 1000, 0]),
                                    dict(same, xyz=[3000, 1000, 0]),
                                    dict(same, xyz=[1000, 3000, 0])],
                           focus=[2000, 2000, 0]))
    rows.append(center_row(members=[dict(same, xyz=[2000, 2000, 0]),
                                    dict(same, xyz=[2100, 2000, 0])],
                           focus=[2000, 2000, 0]))
    rows.append(center_row(members=[dict(same, xyz=[1000, 1000, 0], passengers=5, naval=True),
                                    dict(same, xyz=[1200, 1000, 0], passengers=5, naval=True)],
                           focus=[2000, 2000, 0]))
    rows.append(center_row(members=[dict(same, xyz=[1000, 1000, 0], can_enter=1),
                                    dict(same, xyz=[1500, 1000, 0], can_enter=1)]))
    return rows


# ---------------------------------------------------------------- recruit

RECRUIT = 0x6EAA90
CAN_ADD = 0x6EA610
ADD_MEMBER = 0x6EA500
GET_GROUP = 0x6F1870
TEAM_WAYPOINT = 0x6F18A0
FIRST_PASSENGER = 0x473450


def recruit_row(*, kind, candidates, zone=None, group=-1, recruiter=False, amount=2, count=0,
                slot=0):
    """`candidates`: dicts of location, group, house/type match and the
    Can_Add answer, in array order."""
    emu = TeamEmu()
    house, other_house = HOUSES, HOUSES + HOUSE_SIZE
    emu.write32(TEAM + 0x2C, house)
    entry_type = emu.techno_type_at(0, rtti=TYPE_RTTI[kind])
    other_type = emu.techno_type_at(1, rtti=TYPE_RTTI[kind])
    emu.write32(TASK_FORCE + 0x9C, slot + 1)
    emu.write32(TASK_FORCE + 0xA4 + slot * 8, amount)
    emu.write32(TASK_FORCE + 0xA8 + slot * 8, entry_type)
    emu.write32(TEAM + 0x88 + slot * 4, count)
    emu.write8(TEAM_TYPE + 0xA8, int(recruiter))
    emu.hook(GET_GROUP, lambda _e: group, 0)

    def waypoint(e):
        out = e.arg(0)
        e.write32(out, 0)
        return out
    emu.hook(TEAM_WAYPOINT, waypoint, 4)
    answers = {}

    def can_add(e):
        candidate = e.arg(0)
        e.events.append(['can_add', TeamEmu.index_of(candidate), e.arg(2)])
        return int(answers[candidate])
    emu.hook(CAN_ADD, can_add, 12)
    emu.hook(ADD_MEMBER, lambda e: (e.events.append(['add', TeamEmu.index_of(e.arg(0)),
                                                     e.arg(1)]), 1)[1], 8)
    emu.hook(FIRST_PASSENGER, lambda _e: 0, 0)
    array = ARRAYS
    for index, candidate in enumerate(candidates):
        address = emu.object(index, xyz=candidate['xyz'])
        emu.write32(address + 0x214, candidate['group'])
        emu.write32(address + 0x21C, house if candidate['own_house'] else other_house)
        emu.write32(address + 0x6C4, entry_type if candidate['entry_type'] else other_type)
        answers[address] = candidate['can_add']
        emu.write32(array + index * 4, address)
    pointer, length = CLASS_ARRAY[kind]
    emu.write32(pointer, array)
    emu.write32(length, len(candidates))
    if zone is not None:
        emu.write32(TEAM + 0x34, emu.object(len(candidates), xyz=zone))
    result = emu.invoke(RECRUIT, ecx=TEAM, args=(slot,))
    return dict(kind=kind, candidates=candidates, zone=zone, group=group, recruiter=recruiter,
                amount=amount, count=count, slot=slot, result=result, events=emu.events)


def recruit_candidate(rng, *, far=False, groups=(-1,)):
    span = 0x7FFF00 if far else 0x6000
    return dict(xyz=[rng.randrange(0, span), rng.randrange(0, span), 0],
                group=rng.choice(groups), own_house=rng.random() > 0.2,
                entry_type=rng.random() > 0.2, can_add=rng.random() > 0.3)


def recruit():
    rng = random.Random(0x6EAA90)
    rows = []
    for case in range(150):
        kind = ('infantry', 'unit', 'aircraft')[case % 3]
        far = case % 5 == 0
        groups = (-1,) if case % 4 else (-1, 3, 7)
        group = rng.choice((-1, -1, 3, -2)) if case % 4 == 0 else -1
        candidates = [recruit_candidate(rng, far=far, groups=groups)
                      for _ in range(rng.randrange(0, 8))]
        zone = None if case % 6 == 0 else [rng.randrange(0, 0x7FFF00 if far else 0x6000),
                                           rng.randrange(0, 0x7FFF00 if far else 0x6000), 0]
        rows.append(recruit_row(kind=kind, candidates=candidates, zone=zone, group=group,
                                recruiter=case % 7 == 3))
    # Equal keys keep the first; a full entry recruits nobody.
    same = dict(group=-1, own_house=True, entry_type=True, can_add=True)
    rows.append(recruit_row(kind='infantry', zone=[0x1000, 0x1000, 0],
                            candidates=[dict(same, xyz=[0x1100, 0x1000, 0]),
                                        dict(same, xyz=[0x1000, 0x1100, 0])]))
    rows.append(recruit_row(kind='unit', zone=[0x1000, 0x1000, 0], amount=2, count=2,
                            candidates=[dict(same, xyz=[0x1100, 0x1000, 0])]))
    rows.append(recruit_row(kind='unit', zone=[0x1000, 0x1000, 0], amount=-1, count=-3,
                            candidates=[dict(same, xyz=[0x1100, 0x1000, 0])]))
    return rows


# ---------------------------------------------------------------- distance

DISTANCE = 0x5F6360
FOUNDATIONS = 22


def distance_row(*, member, target, building=None):
    """`building`: the target's foundation index, else a Foot target."""
    emu = TeamEmu()
    self_address = emu.object(0, xyz=member)
    if building is None:
        target_address = emu.object(1, xyz=target, rtti=RTTI_UNIT)
    else:
        target_address = emu.object(1, vtable=BUILDING_VTABLE, rtti=RTTI_BUILDING)
        building_type = BUILDING_TYPES
        emu.write32(target_address + 0x520, building_type)
        emu.write32(building_type + 0xEF0, building)

        def coords(e):
            out = e.arg(0)
            for offset, value in zip((0, 4, 8), target):
                e.write32(out + offset, value)
            return out
        emu.hook(STUB_BUILDING_COORDS, coords, 4)
    result = i32(emu.invoke(DISTANCE, ecx=self_address, args=(target_address,)))
    return dict(member=member, target=target, building=building, result=result)


def distance():
    rng = random.Random(0x5F6360)
    rows = []
    for case in range(80):
        span = (0x400, 0x4000, 0x40000, 0x7FFF00)[case % 4]
        member = [rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x800)]
        target = [rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x800)]
        rows.append(distance_row(member=member, target=target))
    for foundation in range(FOUNDATIONS):
        for offset in (0, 64, 200, 700):
            rows.append(distance_row(member=[0x2000, 0x2000, 0],
                                     target=[0x2000 + offset, 0x2000, 0], building=foundation))
    return rows


# ---------------------------------------------------------------- guard

GUARD_CASE = 0x6E97CE
GUARD_END = 0x6E97EC


def guard_row(argument, frame=1234):
    emu = TeamEmu()
    emu.write32(FRAME, frame)
    uc = emu.uc
    sp = STACK_BASE + STACK_SIZE - 0x1000
    emu.write32(sp + 0x24, 0x5A5A5A5A)
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.reg_write(UC_X86_REG_ESI, TEAM)
    uc.reg_write(UC_X86_REG_ECX, argument & 0xFFFFFFFF)
    uc.reg_write(UC_X86_REG_EBX, 1)
    run_checked(uc, GUARD_CASE, GUARD_END)
    return dict(argument=argument, frame=frame, start=i32(emu.read32(TEAM + 0x58)),
                duration=i32(emu.read32(TEAM + 0x60)))


def guard():
    return [guard_row(argument) for argument in
            (0, 1, 2, 4, 20, 60, -1, -3, 0x08888888, 0x08888889, 0x7FFFFFFF, -0x80000000)]


# ---------------------------------------------------------------- regroup

REGROUP_AT_BASE = 0x6EFA10
REGROUP_FNPC = 0x6EFC3A
BASE_CENTER = 0x50DF30


def regroup_row(*, own, enemy=None, safe_distance, draw=None):
    emu = TeamEmu()
    house, enemy_house = HOUSES, HOUSES + HOUSE_SIZE
    leader = emu.object(0, xyz=[0x3000, 0x3000, 0])
    emu.write32(TEAM + 0x54, leader)
    emu.write8(leader + 0x90, 1)
    emu.write32(leader + 0x6C, 100)
    emu.write8(leader + 0x689, 1)
    emu.write32(leader + 0x21C, house)
    emu.techno_type[leader] = emu.techno_type_at(0)
    emu.write32(RULES + 0xD74, safe_distance)
    emu.write32(HOUSE_ARRAY, ARRAYS)
    emu.write32(ARRAYS + 4, enemy_house)
    emu.write32(house + 0x5600, 1 if enemy is not None else -1)
    centers = {house: own, enemy_house: enemy}

    def base_center(e):
        out = e.arg(0)
        for offset, value in zip((0, 4, 8), centers[e.uc.reg_read(UC_X86_REG_ECX)] + [0]):
            e.write32(out + offset, value)
        return out
    emu.hook(BASE_CENTER, base_center, 4)
    emu.draws([] if draw is None else [draw])
    emu.start(REGROUP_AT_BASE, REGROUP_FNPC, ecx=TEAM, args=(0, 1))
    # At the call, the seed cell is FNPC's second argument.
    seed_pointer = emu.read32(emu.uc.reg_read(UC_X86_REG_ESP) + 4)
    seed = struct.unpack('<hh', emu.uc.mem_read(seed_pointer, 4))
    return dict(own=own, enemy=enemy, safe_distance=safe_distance, draw=draw, seed=list(seed),
                draws=[event[1:] for event in emu.events if event[0] == 'draw'])


def regroup():
    rng = random.Random(0x6EFA10)
    rows = []
    for draw in (0, 1, 31, 32, 63, 64, 96, 127, 128, 160, 191, 192, 224, 255):
        for safe_distance in (10, 50):
            rows.append(regroup_row(own=[0x4080, 0x4080], safe_distance=safe_distance,
                                    draw=draw))
    for _ in range(60):
        own = [rng.randrange(0x80, 0x30000), rng.randrange(0x80, 0x30000)]
        enemy = [rng.randrange(0x80, 0x30000), rng.randrange(0x80, 0x30000)]
        rows.append(regroup_row(own=own, enemy=enemy,
                                safe_distance=rng.choice((0, 1, 10, 30, 50, 100))))
    for enemy in ([0x4080, 0x4080], [0x5080, 0x4080], [0x4080, 0x5080], [0x3080, 0x4080],
                  [0x4080, 0x3080], [0x5080, 0x5080], [0x3080, 0x3080]):
        rows.append(regroup_row(own=[0x4080, 0x4080], enemy=enemy, safe_distance=50))
    rows.append(regroup_row(own=[0, 0], safe_distance=50, draw=128))
    rows.append(regroup_row(own=[0x80, 0x80], enemy=[0x30080, 0x80], safe_distance=-5))
    return rows


# ---------------------------------------------------------------- gather

GATHER_AT_ENEMY_BASE = 0x6EF700
GATHER_FNPC = 0x6EF98A


def gather_row(*, own, enemy, safe_distance, leader=(0x3000, 0x3000, 0), has_enemy=True):
    """`own`/`enemy`: a base centre's XY, [0, 0] for none."""
    emu = TeamEmu()
    house, enemy_house = HOUSES, HOUSES + HOUSE_SIZE
    leader_address = emu.object(0, xyz=list(leader))
    emu.write32(TEAM + 0x54, leader_address)
    emu.write8(leader_address + 0x90, 1)
    emu.write32(leader_address + 0x6C, 100)
    emu.write8(leader_address + 0x689, 1)
    emu.write32(leader_address + 0x21C, house)
    emu.techno_type[leader_address] = emu.techno_type_at(0)
    emu.write32(RULES + 0xD74, safe_distance)
    emu.write32(HOUSE_ARRAY, ARRAYS)
    emu.write32(ARRAYS + 4, enemy_house)
    emu.write32(house + 0x5600, 1 if has_enemy else -1)
    centers = {house: own, enemy_house: enemy}

    def base_center(e):
        out = e.arg(0)
        for offset, value in zip((0, 4, 8), centers[e.uc.reg_read(UC_X86_REG_ECX)] + [0]):
            e.write32(out + offset, value)
        return out
    emu.hook(BASE_CENTER, base_center, 4)
    stop = emu.start(GATHER_AT_ENEMY_BASE, (GATHER_FNPC, RET_MAGIC), ecx=TEAM, args=(0, 1))
    seed = None
    if stop == GATHER_FNPC:
        seed_pointer = emu.read32(emu.uc.reg_read(UC_X86_REG_ESP) + 4)
        seed = list(struct.unpack('<hh', emu.uc.mem_read(seed_pointer, 4)))
    return dict(own=own, enemy=enemy, leader=list(leader), has_enemy=has_enemy,
                safe_distance=safe_distance, seed=seed, finished=bool(emu.read8(TEAM + 0x80)))


def gather():
    rng = random.Random(0x6EF700)
    rows = []
    for _ in range(60):
        own = [rng.randrange(0x80, 0x30000), rng.randrange(0x80, 0x30000)]
        enemy = [rng.randrange(0x80, 0x30000), rng.randrange(0x80, 0x30000)]
        rows.append(gather_row(own=own, enemy=enemy,
                               safe_distance=rng.choice((0, 1, 10, 30, 50, 100))))
    for own in ([0x4080, 0x4080], [0x5080, 0x4080], [0x4080, 0x5080], [0x3080, 0x4080],
                [0x4080, 0x3080], [0x5080, 0x5080], [0x3080, 0x3080]):
        rows.append(gather_row(own=own, enemy=[0x4080, 0x4080], safe_distance=50))
    for leader in ((0x2345, 0x6789, 0x100), (0x4080, 0x4080, 0)):
        rows.append(gather_row(own=[0, 0], enemy=[0x4080, 0x4080], leader=leader,
                               safe_distance=30))
    rows.append(gather_row(own=[0x80, 0x80], enemy=[0x30080, 0x80], safe_distance=-5))
    rows.append(gather_row(own=[0x4080, 0x4080], enemy=[0, 0], safe_distance=50))
    rows.append(gather_row(own=[0x4080, 0x4080], enemy=[0x8080, 0x8080], safe_distance=50,
                           has_enemy=False))
    return rows


# ---------------------------------------------------------------- quarry

QUARRY_TO_THREAT = 0x645BB0


def quarry():
    return [dict(quarry=argument,
                 mask=TeamEmu().invoke(QUARRY_TO_THREAT, ecx=argument & 0xFFFFFFFF))
            for argument in list(range(-2, 14)) + [0x7FFFFFFF, -0x80000000]]


# ---------------------------------------------------------------- attack_cadence

CADENCE = 0x6EB59A
CADENCE_ASKS, CADENCE_SKIPS = 0x6EB5B0, 0x6EB5D9


def attack_cadence():
    rows = []
    for frame in (list(range(0, 20)) + list(range(-12, 0))
                  + [0x7FFFFFFC, 0x7FFFFFFF, -0x80000000, -0x7FFFFFFC]):
        emu = TeamEmu()
        emu.write32(FRAME, frame)
        emu.uc.reg_write(UC_X86_REG_ESP, STACK_BASE + STACK_SIZE - 0x1000)
        stop = run_checked(emu.uc, CADENCE, (CADENCE_ASKS, CADENCE_SKIPS))
        rows.append(dict(frame=frame, asks=stop == CADENCE_ASKS))
    return rows


# ---------------------------------------------------------------- own_building

FIND_OWN_BUILDING = 0x6EEEA0
THREAT_AT = 0x56BCD0


def own_building_row(*, leader, buildings, mode):
    """`buildings`: (location, of the searched type) in the house's order."""
    emu = TeamEmu()
    house = HOUSES
    searched, other = BUILDING_TYPES, BUILDING_TYPES + 0x2000
    leader_address = emu.object(0, xyz=leader)
    emu.write32(leader_address + 0x21C, house)
    array = ARRAYS
    for index, (location, of_type) in enumerate(buildings):
        address = emu.object(index + 1, xyz=location, vtable=OBJECT_VTABLE,
                             rtti=RTTI_BUILDING)
        emu.write32(address + 0x520, searched if of_type else other)
        emu.write32(array + index * 4, address)
    emu.write32(house + 0x6C, array)
    emu.write32(house + 0x78, len(buildings))
    emu.hook(THREAT_AT, lambda _e: 0, 8)
    result = emu.invoke(FIND_OWN_BUILDING, ecx=searched, args=(leader_address, mode, 0))
    return dict(leader=leader, buildings=buildings, mode=mode,
                result=None if result == 0 else TeamEmu.index_of(result) - 1)


def own_building():
    rng = random.Random(0x6EEEA0)
    rows = []
    for case in range(60):
        span = 0x7FFF00 if case % 5 == 0 else 0x8000
        leader = [rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x400)]
        buildings = [([rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x400)],
                      rng.random() > 0.3) for _ in range(rng.randrange(0, 7))]
        rows.append(own_building_row(leader=leader, buildings=buildings,
                                     mode=(0, 1, 2, 3, 2, 3, 4, 0x10000)[case % 8]))
    tied = [([0x2100, 0x2000, 0], True), ([0x2000, 0x2100, 0], True)]
    for mode in (2, 3):
        rows.append(own_building_row(leader=[0x2000, 0x2000, 0], buildings=tied, mode=mode))
    return rows


# ---------------------------------------------------------------- target_building

FIND_BEST_TARGET_BUILDING = 0x6EEBD0
BUILDING_ARRAY = (0xA8EB44, 0xA8EB50)
HOUSE_TYPES = BUILDING_TYPES + 0x8000


def target_building_row(*, unit, buildings, mode, only_enemy, enemy=1, allies=(),
                        passive=(3,)):
    """`buildings`: dicts of Location, GetCoords answer, type match, owner
    (house 0..3) and threat answer, in BuildingClass::Array order. House 0
    owns the unit; `enemy` is the house the original receives as the enemy
    (None for NULL); `allies` are the houses house 0's Allies bits name;
    `passive` the houses whose type is MultiplayPassive."""
    emu = TeamEmu()
    houses = [HOUSES + index * HOUSE_SIZE for index in range(4)]
    for index, house in enumerate(houses):
        house_type = HOUSE_TYPES + index * 0x400
        emu.write32(house + 0x30, index)
        emu.write32(house + 0x34, house_type)
        emu.write8(house_type + 0x1A6, int(index in passive))
    emu.write32(houses[0] + 0x5788, sum(1 << ally for ally in allies))
    unit_address = emu.object(0, xyz=unit)
    emu.write32(unit_address + 0x21C, houses[0])
    searched, other = BUILDING_TYPES, BUILDING_TYPES + 0x2000
    coords, threats = {}, {}
    for index, building in enumerate(buildings):
        address = emu.object(index + 1, xyz=building['location'], vtable=BUILDING_VTABLE,
                             rtti=RTTI_BUILDING)
        emu.write32(address + 0x520, searched if building['of_type'] else other)
        emu.write32(address + 0x21C, houses[building['owner']])
        emu.write32(ARRAYS + index * 4, address)
        coords[address] = building['coords']
        threats[address] = building['threat']
    emu.write32(BUILDING_ARRAY[0], ARRAYS)
    emu.write32(BUILDING_ARRAY[1], len(buildings))

    def building_coords(e):
        out = e.arg(0)
        for offset, value in zip((0, 4, 8), coords[e.uc.reg_read(UC_X86_REG_ECX)]):
            e.write32(out + offset, value)
        return out

    def threat_at(e):
        # The building being scored is in ESI throughout the walk.
        building = e.uc.reg_read(UC_X86_REG_ESI)
        cell = e.uc.mem_read(e.arg(0), 4)
        e.events.append(['threat', TeamEmu.index_of(building) - 1,
                         struct.unpack('<h', cell[:2])[0], struct.unpack('<h', cell[2:])[0],
                         houses.index(e.arg(1))])
        return threats[building]
    emu.hook(STUB_BUILDING_COORDS, building_coords, 4)
    emu.hook(THREAT_AT, threat_at, 8)
    result = emu.invoke(FIND_BEST_TARGET_BUILDING, ecx=searched,
                        edx=0 if enemy is None else houses[enemy],
                        args=(unit_address, mode, int(only_enemy)))
    return dict(unit=unit, buildings=buildings, mode=mode, only_enemy=only_enemy, enemy=enemy,
                allies=list(allies), passive=list(passive),
                result=None if result == 0 else TeamEmu.index_of(result) - 1,
                events=emu.events)


def target_building():
    rng = random.Random(0x6EEBD0)
    rows = []
    for case in range(160):
        span = (0x400, 0x8000, 0x40000, 0x7FFF00)[case % 4]

        def xyz():
            return [rng.randrange(-span, span), rng.randrange(-span, span),
                    rng.randrange(0, 0x800)]
        buildings = [dict(location=xyz(), coords=xyz(), of_type=rng.random() > 0.25,
                          owner=rng.randrange(4),
                          threat=rng.choice((0, 0, rng.randrange(1, 0x400),
                                             rng.randrange(0, 0x7FFFFFFF), -rng.randrange(1, 9))))
                     for _ in range(rng.randrange(0, 8))]
        rows.append(target_building_row(
            unit=xyz(), buildings=buildings, mode=(0, 1, 2, 3, 2, 0, 4, 0xFFFF)[case % 8],
            only_enemy=case % 3 == 1, enemy=None if case % 9 == 4 else 1,
            allies=[ally for ally in (1, 2, 3) if rng.random() > 0.6],
            passive=(3,) if case % 5 else (2, 3)))
    near = dict(location=[0x2100, 0x2000, 0], coords=[0x2180, 0x2080, 0], of_type=True,
                owner=2, threat=7)
    far = dict(location=[0x3000, 0x2000, 0], coords=[0x3080, 0x2080, 0], of_type=True,
               owner=1, threat=3)
    for mode in (0, 1, 2, 3):
        for only_enemy in (False, True):
            # The enemy's building is not the best overall: OnlyTargetHouseEnemy=
            # takes it, otherwise the other house's.
            rows.append(target_building_row(unit=[0x2000, 0x2000, 0], buildings=[near, far],
                                            mode=mode, only_enemy=only_enemy))
    # Ties keep the earlier building; an allied, passive owner's building is
    # a candidate, an allied, active one is not; an own building is allied.
    tied = [dict(near, owner=3), dict(near, location=[0x2000, 0x2100, 0], owner=1)]
    for mode in (0, 2):
        rows.append(target_building_row(unit=[0x2000, 0x2000, 0], buildings=tied, mode=mode,
                                        only_enemy=False, allies=(3,)))
    rows.append(target_building_row(unit=[0x2000, 0x2000, 0], mode=2, only_enemy=False,
                                    allies=(2,), buildings=[near, dict(near, owner=0),
                                                            dict(far, owner=3)]))
    # No enemy: OnlyTargetHouseEnemy= finds none.
    rows.append(target_building_row(unit=[0x2000, 0x2000, 0], buildings=[near, far], mode=2,
                                    only_enemy=True, enemy=None))
    # Coordinates whose /256 rounds toward zero, and a threat of -1 that no
    # building beats.
    rows.append(target_building_row(unit=[0, 0, 0], mode=1, only_enemy=False, buildings=[
        dict(near, coords=[-0x80, -0x101, 0], threat=-1),
        dict(near, coords=[-0x100, 0x7F, 0], threat=-1)]))
    return rows


# ---------------------------------------------------------------- bio_reactor

ENTER_BIO_REACTOR = 0x4DFB70
BIO_REACTOR_PICKED = 0x4DFC52
CAN_LOAD = 0xF


def bio_reactor_row(*, foot, absorbers, contact=None, pending=None, latch=False):
    """`absorbers`: dicts of location and CAN_LOAD answer, in House+0xB0
    order; `contact`/`pending`: an absorber's index, else none."""
    emu = TeamEmu()
    house = HOUSES
    foot_address = emu.object(0, xyz=foot)
    emu.write32(foot_address + 0x21C, house)
    emu.write8(foot_address + 0x68F, int(latch))
    for slot, stub in ((0x278, STUB_TRANSMIT), (0x1E8, STUB_QUEUE_MISSION),
                       (0x480, STUB_ASSIGN_DESTINATION)):
        emu.write32(OBJECT_VTABLE + slot, stub)
    addresses = []
    answers, coords = {}, {}
    for index, absorber in enumerate(absorbers):
        address = emu.object(index + 1, vtable=BUILDING_VTABLE, rtti=RTTI_BUILDING)
        addresses.append(address)
        answers[address] = absorber['answer']
        coords[address] = absorber['xyz']
        emu.write32(ARRAYS + index * 4, address)
    emu.write32(house + 0xB4, ARRAYS)
    emu.write32(house + 0xC0, len(absorbers))
    contacts = ARRAYS + 0x1000
    emu.write32(contacts, 0 if contact is None else addresses[contact])
    emu.write32(foot_address + 0xE4, contacts)
    emu.write32(foot_address + 0x500, 0 if pending is None else addresses[pending])

    def index_of(address):
        return None if address == 0 else TeamEmu.index_of(address) - 1

    def transmit(e):
        message, to = e.arg(0), e.arg(1)
        e.events.append(['transmit', message, index_of(to)])
        return answers[to]

    def building_coords(e):
        out = e.arg(0)
        for offset, value in zip((0, 4, 8), coords[e.uc.reg_read(UC_X86_REG_ECX)]):
            e.write32(out + offset, value)
        return out
    emu.hook(STUB_TRANSMIT, transmit, 8)
    emu.hook(STUB_BUILDING_COORDS, building_coords, 4)
    emu.hook(STUB_QUEUE_MISSION, lambda e: e.events.append(
        ['queue_mission', e.arg(0), e.arg(1)]) or 1, 8)
    emu.hook(STUB_ASSIGN_DESTINATION, lambda e: e.events.append(
        ['assign_destination', index_of(e.arg(0)), e.arg(1)]), 8)
    emu.uc.hook_add(UC_HOOK_CODE, lambda uc, *_: emu.events.append(
        ['picked', index_of(uc.reg_read(UC_X86_REG_EDI))]),
        begin=BIO_REACTOR_PICKED, end=BIO_REACTOR_PICKED)
    result = emu.invoke(ENTER_BIO_REACTOR, ecx=foot_address) & 0xFF
    return dict(foot=foot, absorbers=absorbers, contact=contact, pending=pending, latch=latch,
                result=result, latch_after=emu.read8(foot_address + 0x68F),
                events=emu.events)


def bio_reactor():
    rng = random.Random(0x4DFB70)
    rows = []
    for case in range(120):
        span = (0x400, 0x4000, 0x40000, 0x7FFF00)[case % 4]
        foot = [rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x800)]
        absorbers = [dict(xyz=[rng.randrange(0, span), rng.randrange(0, span),
                               rng.randrange(0, 0x800)],
                          answer=rng.choice((1, 1, 1, 10, 0)))
                     for _ in range(rng.randrange(0, 6))]
        contact = pending = None
        if absorbers and case % 3 == 1:
            contact = rng.randrange(len(absorbers))
        if absorbers and case % 5 == 2:
            pending = rng.randrange(len(absorbers))
        rows.append(bio_reactor_row(foot=foot, absorbers=absorbers, contact=contact,
                                    pending=pending, latch=case % 2 == 1))
    taker = dict(answer=1)
    # Equal distances: the walk from the end meets the later entry first and
    # keeps it; a nearer one later in the walk replaces it.
    rows.append(bio_reactor_row(foot=[0x2000, 0x2000, 0],
                                absorbers=[dict(taker, xyz=[0x2100, 0x2000, 0]),
                                           dict(taker, xyz=[0x2000, 0x2100, 0])]))
    rows.append(bio_reactor_row(foot=[0x2000, 0x2000, 0],
                                absorbers=[dict(taker, xyz=[0x2080, 0x2000, 0]),
                                           dict(taker, xyz=[0x2000, 0x2100, 0])]))
    # Distances that differ below a lepton tie after ftol.
    rows.append(bio_reactor_row(foot=[0, 0, 0],
                                absorbers=[dict(taker, xyz=[300, 400, 0]),
                                           dict(taker, xyz=[0, 500, 1])]))
    # Only a refusing absorber is nearest; none takes it; none at all.
    rows.append(bio_reactor_row(foot=[0x2000, 0x2000, 0],
                                absorbers=[dict(taker, xyz=[0x2800, 0x2000, 0]),
                                           dict(answer=10, xyz=[0x2010, 0x2000, 0])]))
    rows.append(bio_reactor_row(foot=[0x2000, 0x2000, 0], latch=True,
                                absorbers=[dict(answer=10, xyz=[0x2800, 0x2000, 0]),
                                           dict(answer=0, xyz=[0x2010, 0x2000, 0])]))
    rows.append(bio_reactor_row(foot=[0x2000, 0x2000, 0], latch=True, absorbers=[]))
    # The pick is already the radio contact or the pending entry; another
    # absorber is.
    two = [dict(taker, xyz=[0x2400, 0x2000, 0]), dict(taker, xyz=[0x3000, 0x2000, 0])]
    rows.append(bio_reactor_row(foot=[0x2000, 0x2000, 0], absorbers=two, contact=0))
    rows.append(bio_reactor_row(foot=[0x2000, 0x2000, 0], absorbers=two, pending=0))
    rows.append(bio_reactor_row(foot=[0x2000, 0x2000, 0], absorbers=two, contact=1,
                                pending=1))
    return rows


# ---------------------------------------------------------------- battle_bunker

ENTER_BATTLE_BUNKER = 0x4DFCB0
CAN_BE_OCCUPIED_BY = 0x457CE0


def battle_bunker_row(*, foot, bunkers, infantry=True, nav=None, mission=0, latch=False):
    """`bunkers`: dicts of location and CanBeOccupiedBy answer, in House+0xE0
    order; `nav`: a bunker's index as the Foot's NavCom (+0x5A4), else none;
    `mission`: the Foot's current mission (+0xAC)."""
    emu = TeamEmu()
    house = HOUSES
    foot_address = emu.object(0, xyz=foot, rtti=RTTI_INFANTRY if infantry else RTTI_UNIT)
    emu.write32(foot_address + 0x21C, house)
    emu.write8(foot_address + 0x690, int(latch))
    emu.write32(foot_address + 0xAC, mission)
    for slot, stub in ((0x1E8, STUB_QUEUE_MISSION), (0x480, STUB_ASSIGN_DESTINATION)):
        emu.write32(OBJECT_VTABLE + slot, stub)
    addresses = []
    answers, coords = {}, {}
    for index, bunker in enumerate(bunkers):
        address = emu.object(index + 1, vtable=BUILDING_VTABLE, rtti=RTTI_BUILDING)
        addresses.append(address)
        answers[address] = bunker['answer']
        coords[address] = bunker['xyz']
        emu.write32(ARRAYS + index * 4, address)
    emu.write32(house + 0xE4, ARRAYS)
    emu.write32(house + 0xF0, len(bunkers))
    emu.write32(foot_address + 0x5A4, 0 if nav is None else addresses[nav])

    def index_of(address):
        return None if address == 0 else TeamEmu.index_of(address) - 1

    def can_be_occupied_by(e):
        # NULL is refused by the original's first test (0x457CEB).
        building, occupant = e.uc.reg_read(UC_X86_REG_ECX), e.arg(0)
        e.events.append(['can_be_occupied_by', index_of(building),
                         {0: None, foot_address: 'foot'}[occupant]])
        return int(occupant != 0 and answers[building])

    def building_coords(e):
        out = e.arg(0)
        for offset, value in zip((0, 4, 8), coords[e.uc.reg_read(UC_X86_REG_ECX)]):
            e.write32(out + offset, value)
        return out
    emu.hook(CAN_BE_OCCUPIED_BY, can_be_occupied_by, 4)
    emu.hook(STUB_BUILDING_COORDS, building_coords, 4)
    emu.hook(STUB_QUEUE_MISSION, lambda e: e.events.append(
        ['queue_mission', e.arg(0), e.arg(1)]) or 1, 8)
    emu.hook(STUB_ASSIGN_DESTINATION, lambda e: e.events.append(
        ['assign_destination', index_of(e.arg(0)), e.arg(1)]), 8)
    result = emu.invoke(ENTER_BATTLE_BUNKER, ecx=foot_address) & 0xFF
    return dict(foot=foot, bunkers=bunkers, infantry=infantry, nav=nav, mission=mission,
                latch=latch, result=result, latch_after=emu.read8(foot_address + 0x690),
                events=emu.events)


def battle_bunker():
    rng = random.Random(0x4DFCB0)
    rows = []
    for case in range(120):
        span = (0x400, 0x4000, 0x40000, 0x7FFF00)[case % 4]
        foot = [rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x800)]
        bunkers = [dict(xyz=[rng.randrange(0, span), rng.randrange(0, span),
                             rng.randrange(0, 0x800)],
                        answer=rng.choice((True, True, False)))
                   for _ in range(rng.randrange(0, 6))]
        nav = rng.randrange(len(bunkers)) if bunkers and case % 3 == 1 else None
        rows.append(battle_bunker_row(foot=foot, bunkers=bunkers, infantry=case % 11 != 5,
                                      nav=nav, mission=(0, 8, 5)[case % 3] if case % 7 else 8,
                                      latch=case % 2 == 1))
    taker = dict(answer=True)
    # Equal distances: the walk from the end meets the later entry first and
    # keeps it; a refusing building nearer than the best is asked and passed.
    rows.append(battle_bunker_row(foot=[0x2000, 0x2000, 0],
                                  bunkers=[dict(taker, xyz=[0x2100, 0x2000, 0]),
                                           dict(taker, xyz=[0x2000, 0x2100, 0])]))
    rows.append(battle_bunker_row(foot=[0x2000, 0x2000, 0],
                                  bunkers=[dict(taker, xyz=[0x2080, 0x2000, 0]),
                                           dict(answer=False, xyz=[0x2000, 0x2040, 0]),
                                           dict(taker, xyz=[0x2000, 0x2100, 0])]))
    # Distances that differ below a lepton tie after ftol.
    rows.append(battle_bunker_row(foot=[0, 0, 0],
                                  bunkers=[dict(taker, xyz=[300, 400, 0]),
                                           dict(taker, xyz=[0, 500, 1])]))
    # A vehicle: each nearer building is asked with NULL and refuses.
    rows.append(battle_bunker_row(foot=[0x2000, 0x2000, 0], infantry=False, latch=True,
                                  bunkers=[dict(answer=False, xyz=[0x2100, 0x2000, 0])]))
    # None at all; the pick already NavCom on Capture; the pick NavCom on
    # another mission; Capture toward another building.
    rows.append(battle_bunker_row(foot=[0x2000, 0x2000, 0], latch=True, bunkers=[]))
    two = [dict(taker, xyz=[0x2400, 0x2000, 0]), dict(taker, xyz=[0x3000, 0x2000, 0])]
    rows.append(battle_bunker_row(foot=[0x2000, 0x2000, 0], bunkers=two, nav=0, mission=8))
    rows.append(battle_bunker_row(foot=[0x2000, 0x2000, 0], bunkers=two, nav=0, mission=5))
    rows.append(battle_bunker_row(foot=[0x2000, 0x2000, 0], bunkers=two, nav=1, mission=8))
    return rows


# ---------------------------------------------------------------- tank_bunker

ENTER_TANK_BUNKER = 0x4DFF40
TANK_BUNKER_ADMITTED = 0x4DFF54
RADIO_CONTACT = 0x65AD30
STUB_CURRENT_WEAPON = STUBS + 0x1D0
WEAPON = FAKE + 0x352400
ADMITTED = dict(bunkerable=True, turret=True, weapon=True, speed_type=1, parasite=False)


def tank_bunker_row(*, foot, bunkers, admission=ADMITTED):
    """`bunkers`: dicts of location, a first radio contact (`contact`) and an
    occupant (+0x2E4, `occupied`), in House+0xCC order; `admission`: the
    Foot's 0x70FB50 inputs (its type's Bunkerable= +0xD2E and Turret= +0xCA1,
    a type on its current weapon (vt+0x3F4), its SpeedType +0x67C, a parasite
    +0x694)."""
    emu = TeamEmu()
    house = HOUSES
    held = OBJECTS + 0x40 * OBJECT_SIZE
    foot_address = emu.object(0, xyz=foot, rtti=RTTI_UNIT)
    emu.write32(foot_address + 0x14, 7)  # AbstractFlags: Techno, Object, Foot
    emu.write32(foot_address + 0x21C, house)
    emu.write32(foot_address + 0x694, held if admission['parasite'] else 0)
    foot_type = emu.techno_type_at(0)
    emu.techno_type[foot_address] = foot_type
    emu.write8(foot_type + 0xD2E, int(admission['bunkerable']))
    emu.write8(foot_type + 0xCA1, int(admission['turret']))
    emu.write32(foot_type + 0x67C, admission['speed_type'])
    emu.write32(WEAPON, TYPES + TYPE_SIZE if admission['weapon'] else 0)
    for slot, stub in ((0x278, STUB_TRANSMIT), (0x1E8, STUB_QUEUE_MISSION),
                       (0x480, STUB_ASSIGN_DESTINATION), (0x3F4, STUB_CURRENT_WEAPON)):
        emu.write32(OBJECT_VTABLE + slot, stub)
    coords = {}
    for index, bunker in enumerate(bunkers):
        address = emu.object(index + 1, vtable=BUILDING_VTABLE, rtti=RTTI_BUILDING)
        coords[address] = bunker['xyz']
        contacts = ARRAYS + 0x1000 + index * 0x40
        emu.write32(contacts, held if bunker['contact'] else 0)
        emu.write32(address + 0xE4, contacts)
        emu.write32(address + 0x2E4, held if bunker['occupied'] else 0)
        emu.write32(ARRAYS + index * 4, address)
    emu.write32(house + 0xCC, ARRAYS)
    emu.write32(house + 0xD8, len(bunkers))

    def index_of(address):
        return None if address == 0 else TeamEmu.index_of(address) - 1

    def building_coords(e):
        building = e.uc.reg_read(UC_X86_REG_ECX)
        e.events.append(['coords', index_of(building)])
        out = e.arg(0)
        for offset, value in zip((0, 4, 8), coords[building]):
            e.write32(out + offset, value)
        return out
    emu.hook(STUB_BUILDING_COORDS, building_coords, 4)
    emu.hook(STUB_CURRENT_WEAPON, lambda e: WEAPON, 0)
    emu.hook(STUB_TRANSMIT, lambda e: e.events.append(
        ['transmit', e.arg(0), index_of(e.arg(1))]) or 1, 8)
    emu.hook(STUB_QUEUE_MISSION, lambda e: e.events.append(
        ['queue_mission', e.arg(0), e.arg(1)]) or 1, 8)
    emu.hook(STUB_ASSIGN_DESTINATION, lambda e: e.events.append(
        ['assign_destination', index_of(e.arg(0)), e.arg(1)]), 8)
    emu.uc.hook_add(UC_HOOK_CODE, lambda uc, *_: emu.events.append(
        ['contact', index_of(uc.reg_read(UC_X86_REG_ECX)), emu.arg(0)]),
        begin=RADIO_CONTACT, end=RADIO_CONTACT)
    admitted = []
    emu.uc.hook_add(UC_HOOK_CODE, lambda uc, *_: admitted.append(
        uc.reg_read(UC_X86_REG_EAX) & 0xFF),
        begin=TANK_BUNKER_ADMITTED, end=TANK_BUNKER_ADMITTED)
    result = emu.invoke(ENTER_TANK_BUNKER, ecx=foot_address) & 0xFF
    return dict(foot=foot, bunkers=bunkers, admission=admission, admitted=admitted[0],
                result=result, events=emu.events)


def tank_bunker():
    rng = random.Random(0x4DFF40)
    rows = []
    for case in range(120):
        span = (0x400, 0x4000, 0x40000, 0x7FFF00)[case % 4]
        foot = [rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x800)]
        bunkers = [dict(xyz=[rng.randrange(0, span), rng.randrange(0, span),
                             rng.randrange(0, 0x800)],
                        contact=rng.random() < 0.3, occupied=rng.random() < 0.3)
                   for _ in range(rng.randrange(0, 6))]
        rows.append(tank_bunker_row(foot=foot, bunkers=bunkers))
    free = dict(contact=False, occupied=False)
    # Equal distances: the walk from the end meets the later bunker first and
    # keeps it; held bunkers nearer than the best are passed.
    rows.append(tank_bunker_row(foot=[0x2000, 0x2000, 0],
                                bunkers=[dict(free, xyz=[0x2100, 0x2000, 0]),
                                         dict(free, xyz=[0x2000, 0x2100, 0])]))
    rows.append(tank_bunker_row(foot=[0x2000, 0x2000, 0],
                                bunkers=[dict(free, xyz=[0x2080, 0x2000, 0]),
                                         dict(free, contact=True, xyz=[0x2000, 0x2040, 0]),
                                         dict(free, occupied=True, xyz=[0x2040, 0x2000, 0]),
                                         dict(free, xyz=[0x2000, 0x2100, 0])]))
    # Distances that differ below a lepton tie after ftol.
    rows.append(tank_bunker_row(foot=[0, 0, 0],
                                bunkers=[dict(free, xyz=[300, 400, 0]),
                                         dict(free, xyz=[0, 500, 1])]))
    # None at all; every bunker held.
    rows.append(tank_bunker_row(foot=[0x2000, 0x2000, 0], bunkers=[]))
    rows.append(tank_bunker_row(foot=[0x2000, 0x2000, 0],
                                bunkers=[dict(free, contact=True, xyz=[0x2400, 0x2000, 0]),
                                         dict(free, occupied=True, xyz=[0x3000, 0x2000, 0])]))
    # Each admission test refuses alone, a free bunker in reach.
    two = [dict(free, xyz=[0x2400, 0x2000, 0]), dict(free, xyz=[0x3000, 0x2000, 0])]
    for refusal in (dict(bunkerable=False), dict(turret=False), dict(weapon=False),
                    dict(speed_type=3), dict(parasite=True)):
        rows.append(tank_bunker_row(foot=[0x2000, 0x2000, 0], bunkers=two,
                                    admission=dict(ADMITTED, **refusal)))
    return rows


# ---------------------------------------------------------------- transports

LOAD_ONTO_TRANSPORT = 0x6ED200
WAIT_LOADED = 0x6E9C22
WAIT_LOADED_ENDS = {0x6E95AB: False, 0x6E953E: True, 0x6E9C55: True}
UNLOAD = 0x6EF110
REMOVE_MEMBER = 0x6EA870
LOOK_UP_BUILDING = 0x47C520
STUB_MISSION = STUBS + 0x1A0
STUB_READY = STUBS + 0x1B0
STUB_COMMENCE = STUBS + 0x1C0
ACTION = FAKE + 0x352000
PASSENGER = FAKE + 0x352100
CONTACTS = FAKE + 0x352200
NAV = FAKE + 0x352300
ZONE_INDEX = 0x40
CLASS_RTTI = {'infantry': RTTI_INFANTRY, 'unit': RTTI_UNIT, 'aircraft': RTTI_AIRCRAFT}


def transport_team(*, members, entries, zone, stray, returns, action):
    """A team for the transport actions. `members`, in list order: dicts of
    class, type `Passengers=`, passenger count (+0x114, with a first
    passenger +0x118 when not 0), the live bytes (+0x90, +0x6C, +0x81),
    +0x689, a NavCom (+0x5A4), the current mission (vt+0x184), Ready_To_
    Commence's answer, radio contact, an archive target (+0x218), whether a
    building stands in its cell, and its location. `entries`: the TaskForce's
    (class, `Passengers=`) types. The zone (+0x34) is an object at `zone`;
    Stray= is Rules+0x171C, ScriptClass::Current answers `action`."""
    emu = TeamEmu()
    for slot, stub in ((0x184, STUB_MISSION), (0x1E8, STUB_QUEUE_MISSION),
                       (0x480, STUB_ASSIGN_DESTINATION), (0x200, STUB_READY),
                       (0x1EC, STUB_COMMENCE)):
        emu.write32(OBJECT_VTABLE + slot, stub)
    emu.write32(RULES + 0x171C, stray)
    emu.write32(RULES + 0x1720, 0x7FFFFFFF)
    emu.write8(TEAM_TYPE + 0xF4, int(returns))
    emu.write32(TASK_FORCE + 0x9C, len(entries))
    for slot, (kind, passengers) in enumerate(entries):
        emu.write32(TASK_FORCE + 0xA4 + slot * 8, 1)
        emu.write32(TASK_FORCE + 0xA8 + slot * 8,
                    emu.techno_type_at(0x20 + slot, rtti=TYPE_RTTI[kind], passengers=passengers))
    zone_address = emu.object(ZONE_INDEX, xyz=zone)
    emu.write32(TEAM + 0x34, zone_address)
    emu.write32(TEAM + 0x48, len(members))
    addresses, missions, ready, buildings = [], {}, {}, {}
    previous = None
    for index, member in enumerate(members):
        address = emu.object(index, xyz=member['xyz'], rtti=CLASS_RTTI[member['class']])
        addresses.append(address)
        emu.write8(address + 0x90, int(member['active']))
        emu.write32(address + 0x6C, member['health'])
        emu.write8(address + 0x81, int(member['limbo']))
        emu.write8(address + 0x689, int(member['initiated']))
        emu.write32(address + 0x114, member['count'])
        emu.write32(address + 0x118, PASSENGER if member['count'] else 0)
        emu.write32(address + 0x5A4, NAV if member['nav'] else 0)
        if member['radio']:
            emu.write32(address + 0xE4, CONTACTS)
            emu.write32(address + 0xE8, 2)
            emu.write32(CONTACTS + 4, NAV)
        emu.write32(address + 0x218, CELL if member['archive'] else 0)
        emu.techno_type[address] = emu.techno_type_at(index, passengers=member['passengers'])
        missions[address] = member['mission']
        ready[address] = int(member['ready'])
        buildings[tuple(member['xyz'])] = member['on_building']
        if previous is None:
            emu.write32(TEAM + 0x54, address)
        else:
            emu.write32(previous + 0x5D8, address)
        previous = address

    def ref(address):
        if address == 0:
            return None
        if address == CELL:
            return 'cell'
        if address == zone_address:
            return 'zone'
        return TeamEmu.index_of(address)

    def member(e):
        return ref(e.uc.reg_read(UC_X86_REG_ECX))

    def current(e):
        out = e.arg(0)
        e.write32(out, action)
        e.write32(out + 4, 0)
        return out
    last_cell = []

    def cell_at(e):
        coord = e.arg(0)
        xyz = [i32(e.read32(coord + offset)) for offset in (0, 4, 8)]
        e.events.append(['cell_at', xyz])
        last_cell[:] = [tuple(xyz)]
        return CELL

    def building(e):
        found = buildings[last_cell[0]]
        e.events.append(['building', found])
        return 0x1234 if found else 0
    emu.hook(CURRENT_ACTION, current, 4)
    emu.hook(GET_CELL_AT, cell_at, 4)
    emu.hook(LOOK_UP_BUILDING, building, 0)
    emu.hook(STUB_MISSION, lambda e: missions[e.uc.reg_read(UC_X86_REG_ECX)], 0)
    emu.hook(STUB_QUEUE_MISSION, lambda e: e.events.append(
        ['queue_mission', member(e), e.arg(0), e.arg(1)]), 8)
    emu.hook(STUB_ASSIGN_DESTINATION, lambda e: e.events.append(
        ['assign_destination', member(e), ref(e.arg(0)), e.arg(1)]), 8)

    def ready_stub(e):
        e.events.append(['ready', member(e)])
        return ready[e.uc.reg_read(UC_X86_REG_ECX)]
    emu.hook(STUB_READY, ready_stub, 0)
    emu.hook(STUB_COMMENCE, lambda e: e.events.append(['commence', member(e)]) or 1, 0)
    emu.hook(REMOVE_MEMBER, lambda e: e.events.append(
        ['remove_member', ref(e.arg(0)), i32(e.arg(1)), e.arg(2)]) or 1, 12)
    return emu, addresses


def transport_outputs(emu, addresses):
    return dict(finished=emu.read8(TEAM + 0x80),
                initiated=[emu.read8(address + 0x689) for address in addresses],
                archive=[{0: None, CELL: 'cell'}[emu.read32(address + 0x218)]
                         for address in addresses],
                events=emu.events)


def transport_member(rng, *, kind=None):
    kind = kind or rng.choice(('infantry', 'infantry', 'infantry', 'unit', 'unit', 'aircraft'))
    if kind == 'infantry':
        passengers = rng.choice((0, 0, 0, 0, 2))
    elif kind == 'aircraft':
        passengers = rng.choice((0, 2, 2))
    else:
        passengers = rng.choice((0, 1, 2, 2, 5, -1))
    count = rng.randrange(0, passengers + 2) if passengers > 0 else rng.choice((0, 0, 0, 1))
    return {'class': kind, 'passengers': passengers, 'count': count,
            'active': rng.random() > 0.08, 'health': rng.choice((0, 1, 100, 100, 100, 100)),
            'limbo': rng.random() < 0.08, 'initiated': rng.random() > 0.3,
            'nav': rng.random() < 0.3, 'mission': rng.choice((5, 5, 7, 2, 0x10, 1)),
            'ready': rng.random() < 0.6, 'radio': rng.random() < 0.15,
            'archive': rng.random() < 0.4, 'on_building': rng.random() < 0.2,
            'xyz': [rng.randrange(0x80, 0x1F80), rng.randrange(0x80, 0x1F80), 0]}


def transport_case(rng):
    members = [transport_member(rng) for _ in range(rng.randrange(0, 6))]
    # Distinct cells, so the building answer is per member.
    for member, cell in zip(members, rng.sample(range(1, 31 * 31), len(members))):
        member['xyz'][0] = (cell % 31) * 256 + rng.randrange(0, 256)
        member['xyz'][1] = (cell // 31) * 256 + rng.randrange(0, 256)
    entries = [(kind, passengers) for kind, passengers in
               rng.sample([('unit', 5), ('infantry', 0), ('aircraft', 0), ('aircraft', 2),
                           ('unit', 0)], rng.randrange(0, 4))]
    return dict(members=members, entries=entries,
                zone=[rng.randrange(0x80, 0x1F80), rng.randrange(0x80, 0x1F80), 0],
                stray=rng.choice((0x100, 0x200, 0x400, 0x800)), returns=rng.random() < 0.5)


def load_row(*, members, entries, zone, stray, returns):
    emu, addresses = transport_team(members=members, entries=entries, zone=zone, stray=stray,
                                    returns=returns, action=14)
    emu.write32(ACTION, 14)
    emu.invoke(LOAD_ONTO_TRANSPORT, ecx=TEAM, args=(ACTION, 0))
    return dict(members=members, entries=entries, zone=zone, stray=stray, returns=returns,
                **transport_outputs(emu, addresses))


def load():
    rng = random.Random(0x6ED200)
    rows = []
    for case in range(80):
        fields = transport_case(rng)
        if case % 2 and fields['members']:
            # A vehicle with free seats, so passengers are sent aboard.
            member = rng.choice(fields['members'])
            member.update({'class': 'unit', 'passengers': 2, 'count': 0, 'radio': False})
        rows.append(load_row(**fields))

    def plain(kind, passengers=0, count=0, **fields):
        return dict({'class': kind, 'passengers': passengers, 'count': count, 'active': True,
                     'health': 100, 'limbo': False, 'initiated': True, 'nav': False,
                     'mission': 5, 'ready': True, 'radio': False, 'archive': False,
                     'on_building': False, 'xyz': [0x800, 0x800, 0]}, **fields)

    def at(index, member):
        member['xyz'] = [0x480 + 0x200 * index, 0x900, 0]
        return member
    base = dict(entries=[('unit', 2), ('infantry', 0)], zone=[0x800, 0x900, 0], stray=0x200)
    # A full transport before the free one; the first passenger is ordered,
    # the next waits for the update after.
    rows.append(load_row(members=[at(0, plain('infantry')), at(1, plain('unit', 2, 2)),
                                  at(2, plain('unit', 2, 1)), at(3, plain('infantry'))],
                         returns=False, **base))
    # The first passenger is already on Enter, the second is ordered.
    rows.append(load_row(members=[at(0, plain('infantry', mission=7)),
                                  at(1, plain('unit', 2)), at(2, plain('infantry'))],
                         returns=False, **base))
    # Every passenger on Enter: the action waits.
    rows.append(load_row(members=[at(0, plain('infantry', mission=7)),
                                  at(1, plain('unit', 2)), at(2, plain('infantry', mission=7))],
                         returns=False, **base))
    # The transport in radio contact; alone (finished, archived or not).
    rows.append(load_row(members=[at(0, plain('unit', 2, radio=True)), at(1, plain('infantry'))],
                         returns=False, **base))
    for returns in (False, True):
        rows.append(load_row(members=[at(0, plain('unit', 2))], returns=returns, **base))
    # A far member without a NavCom is sent to the zone; one with a NavCom is
    # not; both stay unjoined, so the action finishes.
    rows.append(load_row(members=[at(0, plain('unit', 2)),
                                  dict(plain('infantry', initiated=False), xyz=[0x1E00, 0x1E00, 0]),
                                  dict(plain('infantry', initiated=False, nav=True),
                                       xyz=[0x1E00, 0x1C00, 0])],
                         returns=True, **base))
    # A near unjoined member joins and is ordered at once.
    rows.append(load_row(members=[at(0, plain('unit', 2)),
                                  dict(plain('infantry', initiated=False), xyz=[0x880, 0x900, 0])],
                         returns=False, **base))
    # No vehicle with a free seat; Passengers= 0 and negative; an aircraft
    # entry with Passengers= wants an aircraft.
    rows.append(load_row(members=[at(0, plain('unit', 0)), at(1, plain('unit', -1)),
                                  at(2, plain('infantry', 2))], returns=False, **base))
    rows.append(load_row(members=[at(0, plain('unit', 2)), at(1, plain('infantry'))],
                         returns=False, entries=[('aircraft', 2)], zone=base['zone'],
                         stray=base['stray']))
    # Dead, inactive and limbo members are not ordered.
    rows.append(load_row(members=[at(0, plain('unit', 2)), at(1, plain('infantry', health=0)),
                                  at(2, plain('infantry', active=False)),
                                  at(3, plain('infantry', limbo=True))],
                         returns=True, **base))
    return rows


def wait_loaded_row(*, members):
    emu, _ = transport_team(members=members, entries=[], zone=[0, 0, 0], stray=0, returns=False,
                            action=43)
    emu.uc.reg_write(UC_X86_REG_ESI, TEAM)
    reached = emu.start(WAIT_LOADED, tuple(WAIT_LOADED_ENDS), ecx=TEAM)
    return dict(members=members, finished=WAIT_LOADED_ENDS[reached])


def wait_loaded():
    rng = random.Random(0x6E9C22)
    rows = []
    for _ in range(40):
        rows.append(wait_loaded_row(members=[transport_member(rng)
                                             for _ in range(rng.randrange(0, 5))]))
    return rows


def unload_row(*, members, entries, zone, stray, returns, argument):
    emu, addresses = transport_team(members=members, entries=entries, zone=zone, stray=stray,
                                    returns=returns, action=8)
    emu.write32(ACTION, 8)
    emu.write32(ACTION + 4, argument)
    emu.invoke(UNLOAD, ecx=TEAM, args=(ACTION, 0))
    return dict(members=members, entries=entries, zone=zone, stray=stray, returns=returns,
                argument=argument, **transport_outputs(emu, addresses))


def unload():
    rng = random.Random(0x6EF110)
    rows = []
    for case in range(100):
        fields = transport_case(rng)
        if case % 2:
            # Nobody carries a passenger, so the members are sorted.
            for member in fields['members']:
                member['count'] = 0
        rows.append(unload_row(argument=rng.choice((0, 1, 2, 3, 4, -1)), **fields))
    return rows


def generate():
    return dict(recalc=recalc(), center=center(), recruit=recruit(), distance=distance(),
                guard=guard(), regroup=regroup(), own_building=own_building(),
                target_building=target_building(), gather=gather(), quarry=quarry(), attack_cadence=attack_cadence(),
                bio_reactor=bio_reactor(), battle_bunker=battle_bunker(),
                tank_bunker=tank_bunker(), load=load(), wait_loaded=wait_loaded(),
                unload=unload())


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=lambda: provenance(
        scope=('TeamClass::Recalc over TaskForce totals, member counts and strength bytes; '
               'Calc_Center off action 10 over synthetic members and move targets; Recruit '
               'over one class array with supplied Can_Add answers; ObjectClass::Distance to '
               'a Foot or a building of every foundation; the action 5 guard timer; action '
               '54\'s point up to its FNPC call; FindOwnBuilding over one house\'s buildings; '
               'Find_Best_Target_Building over BuildingClass::Array with four houses; '
               'action 53 up to its FNPC call or its early return; Quarry_To_Threat; '
               'Coordinate_Attack\'s frame test; EnterBioReactor over one house\'s '
               'absorber list with supplied CAN_LOAD answers; EnterBattleBunker over one '
               'house\'s occupiable list with supplied CanBeOccupiedBy answers; '
               'EnterTankBunker over one house\'s tank bunker list; actions 14 '
               'and 8 and action 43\'s inline test over up to five synthetic infantry, '
               'vehicle and aircraft members.'),
        assumptions=['x87 control word 0x0E7F (PC53, chop), the harness default.',
                     'Fixture objects carry only the fields the entries read.',
                     'ScenarioInit (0xA8E7AC), the empty coordinate 0xB0E968 and the empty '
                     'cell 0xB0E948 read zero, as at load (BSS).'],
        substitutions=['WhatAmI (vt+0x2C), GetTechnoType (vt+0x84), the GuardSlower weight '
                       '(vt+0x4E0), Can_Enter_Cell (vt+0x1AC) and SetTarget (vt+0x3C8) answer '
                       'from the case; objects\' GetCoords is the original 0x5F65A0, a '
                       'building\'s answers the case\'s coordinate.',
                       'ScriptClass::Current 0x691500 answers action 0; '
                       'MapClass::GetCellAt 0x565730 records its coordinate and answers one '
                       'cell; the team destructor (vt+0x20) records its call.',
                       'Can_Add 0x6EA610 answers from the case and Add_Member 0x6EA500 '
                       'records its call; Get_Group 0x6F1870 answers the case\'s group; the '
                       'TeamType waypoint 0x6F18A0 answers the empty cell; the first '
                       'passenger 0x473450 answers none.',
                       'Base_Center 0x50DF30 answers the case\'s centres ([0, 0] for none, '
                       'z 0); RandomRanged 0x65C7E0 answers from the case and records its '
                       'range.',
                       'FindOwnBuilding\'s threat map read 0x56BCD0 answers 0.',
                       'Find_Best_Target_Building: a building\'s GetCoords (vt+0x48) answers '
                       'the case\'s coordinate; the threat map read 0x56BCD0 answers the '
                       'scored building\'s case value and records its cell and house; '
                       'IsAlliedWith 0x4F9A50 runs on the houses\' ArrayIndex (+0x30) and '
                       'Allies bits (+0x5788).',
                       'EnterBioReactor\'s Transmit_Message (vt+0x278) answers the case\'s '
                       'CAN_LOAD reply and records it; Queue_Mission (vt+0x1E8) and '
                       'Assign_Destination (vt+0x480) record their calls; the pick is read '
                       'from EDI at 0x4DFC52.',
                       'EnterBattleBunker\'s BuildingClass::CanBeOccupiedBy 0x457CE0 '
                       'answers the case\'s admission for the Foot, refuses NULL as its '
                       'first test (0x457CEB) does, and records its building and occupant; '
                       'Queue_Mission and Assign_Destination record their calls.',
                       'EnterTankBunker: the current weapon (vt+0x3F4) answers a weapon '
                       'with or without a type; Transmit_Message, Queue_Mission and '
                       'Assign_Destination record their calls; RadioClass::Contact 0x65AD30 '
                       'runs as original and records its building; the admission\'s answer '
                       'is read from AL at 0x4DFF54.',
                       'The transport actions: Get_Mission (vt+0x184) and Ready_To_Commence '
                       '(vt+0x200) answer from the case; Queue_Mission, Assign_Destination, '
                       'Assign_Target, Commence (vt+0x1EC) and TeamClass::Remove_Member '
                       '0x6EA870 record their calls; MapClass::GetCellAt 0x565730 records its '
                       'coordinate and answers one cell, for which Look_up_building_in_cell '
                       '0x47C520 answers the member\'s case (its game-active gate 0xA8E9A0 is '
                       'set in play); In_Radio_Contact 0x65AE30 and Set_ArchiveTarget '
                       '0x70C610 run as original; action 43 runs from 0x6E9C22 to its '
                       'branch targets.'],
        entry_points={'recalc': RECALC, 'calc_center': CALC_CENTER, 'recruit': RECRUIT,
                      'distance': DISTANCE, 'guard_case': GUARD_CASE,
                      'regroup_at_base': REGROUP_AT_BASE, 'find_own_building':
                      FIND_OWN_BUILDING, 'find_best_target_building':
                      FIND_BEST_TARGET_BUILDING, 'gather_at_enemy_base': GATHER_AT_ENEMY_BASE,
                      'quarry_to_threat': QUARRY_TO_THREAT, 'attack_cadence': CADENCE,
                      'enter_bio_reactor': ENTER_BIO_REACTOR,
                      'enter_battle_bunker': ENTER_BATTLE_BUNKER,
                      'enter_tank_bunker': ENTER_TANK_BUNKER,
                      'load_onto_transport': LOAD_ONTO_TRANSPORT, 'wait_loaded': WAIT_LOADED,
                      'unload': UNLOAD},
    ))
