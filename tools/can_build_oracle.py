"""Native references for what a house may build.

Run python -m tools.can_build_oracle --check (or explicit --write).
Rust consumer: src/sim/production/can_build_tests.rs.
The separate --secret-lab profile also supplies sim::secret_lab and the
retained rules-reader tests; see can_build_secret_lab.md.

Sections, each case executed in a fresh emulator:
- can_build: HouseClass::CanBuild 0x4F7870 over its gates, the human
  prerequisite arm (the six [General] Prerequisite* lists,
  PrerequisiteProcAlternate, a BuildingType's on-map count) and the build
  limit with its factory scan.
- check_build_limit: HouseClass::CheckBuildLimit 0x50B370 over the
  AirportBound dock count and the build limits, with the house factory each
  class asks.

A row names its types: Q is the asked type (index 10 of its class), B0..B7
BuildingTypes 0..7, U0..U2 UnitTypes 0..2, I0..I1 InfantryTypes 0..1 and
A0..A2 AircraftTypes 0..2. Every factory holds an object, as VERA's do.
The fixture house has no secret labs, AltOwner bits or produced counts
(+0x55A0..+0x55DC), which VERA does not keep (residuals in
src/sim/production/can_build.rs); a row may give it stolen-tech bytes
(+0x2BC..+0x2BE). No BuildingType it reads has PowersUpBuilding= and no
InfantryType VehicleThief=.
"""
from pathlib import Path
import hashlib
import sys

from tools.ai_base_building_oracle import Emu, FAKE, HOUSE, HOUSE_TYPE, RULES, SCENARIO, STUBS
from tools import native_oracle as native
from tools.native_oracle import OracleError, finish_vectors, provenance

from unicorn.x86_const import UC_X86_REG_ECX

CAN_BUILD = 0x4F7870
CHECK_BUILD_LIMIT = 0x50B370

OTHER_HOUSE = FAKE + 0x24000
VTABLES = FAKE + 0x25000
TYPE_VTABLE = VTABLES
OBJECT_VTABLE = VTABLES + 0x100
COUNTER_VTABLE = VTABLES + 0x200
LISTS = FAKE + 0x40000
TYPES = FAKE + 0x100000
TYPE_SIZE = 0x2000
FACTORIES = FAKE + 0x300000
OBJECTS = FAKE + 0x310000
SUPERS = FAKE + 0x320000
SUPER_TYPE = FAKE + 0x321000

STUB_RTTI = STUBS + 0x100
STUB_INDEX = STUBS + 0x110
STUB_OBJECT_TYPE = STUBS + 0x120
# CounterClass growth (vtable +8) must not run: every counter is sized.
STUB_TRAP = STUBS + 0x200

GAME_MODE = 0xA8B238
SW_ALLOWED = 0xA8B263
BUILDING_TYPES = 0xA83C6C
FACTORY_ARRAY = (0xA83E34, 0xA83E40)
UNIT_ARRAY = (0x8B410C, 0x8B4118)

RTTI = {'aircraft': 0x03, 'building': 0x07, 'infantry': 0x10, 'unit': 0x28}
# HouseClass CounterClass bases: tracked (+0x5500..), on the map (+0x5550..).
OWNED = {'building': 0x5500, 'unit': 0x5514, 'infantry': 0x5528, 'aircraft': 0x553C}
ACTIVE = {'building': 0x5550, 'unit': 0x5564, 'infantry': 0x5578, 'aircraft': 0x558C}
PRODUCED = (0x55A0, 0x55B4, 0x55C8, 0x55DC)
COUNTER_CAPACITY = 64
# Rules DynamicVectorClass bases of the [General] Prerequisite* lists
# (items +4, count +0x10), read at 0x004F7C1F..0x004F7DD9.
GROUPS = {'POWER': (-1, 0x358), 'FACTORY': (-2, 0x374), 'BARRACKS': (-3, 0x390),
          'RADAR': (-4, 0x3AC), 'TECH': (-5, 0x3C8), 'PROC': (-6, 0x3E4)}
PROC_ALTERNATE = 0x400
BUILD_TECH = 0x91C
PAD_AIRCRAFT = 0xB58
# House factory slots (0x0050B3B9..0x0050B474).
SLOTS = {'aircraft': 0x53AC, 'infantry': 0x53B0, 'vehicle': 0x53B4, 'ship': 0x53B8,
         'building': 0x53BC}
QUERY_INDEX = 10
# The factory kind of each class, and another type of that class.
KIND = {'unit': 'vehicle', 'infantry': 'infantry', 'aircraft': 'aircraft',
        'building': 'building'}
OTHER = {'unit': 'U0', 'infantry': 'I0', 'aircraft': 'A0', 'building': 'B0'}


class BuildEmu(Emu):
    """An Emu with this oracle's house, rules, types and factories."""

    def __init__(self):
        super().__init__()
        self.rtti = {}
        self.index = {}
        self.object_type = {}
        self.types = {}
        self.list_top = LISTS
        self.write32(TYPE_VTABLE + 0x2C, STUB_RTTI)
        self.write32(TYPE_VTABLE + 0x40, STUB_INDEX)
        self.write32(OBJECT_VTABLE + 0x84, STUB_OBJECT_TYPE)
        self.write32(COUNTER_VTABLE + 8, STUB_TRAP)
        self.hook(STUB_RTTI, lambda e: e.rtti[e.uc.reg_read(UC_X86_REG_ECX)], 0)
        self.hook(STUB_INDEX, lambda e: e.index[e.uc.reg_read(UC_X86_REG_ECX)], 0)
        self.hook(STUB_OBJECT_TYPE,
                  lambda e: e.object_type[e.uc.reg_read(UC_X86_REG_ECX)], 0)
        self.write32(HOUSE + 0x34, HOUSE_TYPE)
        for base in (*OWNED.values(), *ACTIVE.values(), *PRODUCED):
            self.write32(HOUSE + base, COUNTER_VTABLE)
            self.write32(HOUSE + base + 4, self.list([0] * COUNTER_CAPACITY))
            self.write32(HOUSE + base + 8, COUNTER_CAPACITY)
        for _items, count in (FACTORY_ARRAY, UNIT_ARRAY):
            self.write32(count, 0)
        self.write32(BUILDING_TYPES, self.list([0] * 16))
        for name, index in [(f'B{i}', i) for i in range(8)]:
            self.techno_type(name, 'building', index)
        for name, index in [(f'U{i}', i) for i in range(3)]:
            self.techno_type(name, 'unit', index)
        for name, index in [(f'I{i}', i) for i in range(2)]:
            self.techno_type(name, 'infantry', index)
        for name, index in [(f'A{i}', i) for i in range(3)]:
            self.techno_type(name, 'aircraft', index)

    def list(self, values):
        """A fixture array of dwords; returns its address."""
        address = self.list_top
        for slot, value in enumerate(values):
            self.write32(address + 4 * slot, value)
        self.list_top += max(4 * len(values), 4) + 0x10
        return address

    def vector(self, address, values):
        """A DynamicVectorClass at `address` (+4 items, +8 capacity, +0x10
        count)."""
        self.write32(address + 4, self.list(values))
        self.write32(address + 8, len(values))
        self.write32(address + 0x10, len(values))

    def techno_type(self, name, kind, index):
        ty = TYPES + len(self.types) * TYPE_SIZE
        self.write32(ty, TYPE_VTABLE)
        self.write32(ty + 0xDF8, index)
        self.write32(ty + 0x3B8, 0x7FFFFFFF)
        self.write32(ty + 0x634, 1)
        self.write32(ty + 0xDA0, 0xFFFFFFFF)
        self.write32(ty + 0xDA4, 0xFFFFFFFF)
        self.vector(ty + 0x638, [])
        self.vector(ty + 0x654, [])
        if kind == 'building':
            self.write32(ty + 0x16F0, 0xFFFFFFFF)
            self.write32(self.read32(BUILDING_TYPES) + 4 * index, ty)
        self.rtti[ty] = RTTI[kind]
        self.index[ty] = index
        self.types[name] = ty
        return ty

    def counter(self, base, index, value):
        self.write32(self.read32(HOUSE + base + 4) + 4 * index, value)


def prerequisite_code(name):
    return GROUPS[name][0] if name in GROUPS else int(name[1:])


def run(row):
    """Execute one row; returns the native answer."""
    emu = BuildEmu()
    kind = row['class']
    spec = row['type']
    house = row['house']
    q = emu.techno_type('Q', kind, QUERY_INDEX)
    emu.write32(q + 0x3B8, spec.get('limit', 0x7FFFFFFF))
    emu.write32(q + 0x634, spec.get('tech', 1))
    emu.vector(q + 0x638, [prerequisite_code(p) for p in spec.get('prerequisite', [])])
    emu.vector(q + 0x654, [int(p[1:]) for p in spec.get('override', [])])
    mask = lambda countries: sum(1 << c for c in countries)
    if 'required' in spec:
        emu.write32(q + 0xDA0, mask(spec['required']))
    if 'forbidden' in spec:
        emu.write32(q + 0xDA4, mask(spec['forbidden']))
    emu.uc.mem_write(q + 0xD9B, bytes(int(b) for b in spec.get('stolen', [0, 0, 0])))
    emu.uc.mem_write(q + 0xCCE, bytes([int(spec.get('naval', False))]))
    if kind == 'aircraft':
        emu.uc.mem_write(q + 0xE0D, bytes([int(spec.get('airport_bound', False))]))
    super_weapon = spec.get('super_weapon')
    if super_weapon is not None:
        emu.write32(q + 0x16F0, 0)
        emu.write32(HOUSE + 0x258, emu.list([SUPERS]))
        emu.write32(SUPERS + 0x28, SUPER_TYPE)
        emu.uc.mem_write(SUPER_TYPE + 0xE7, bytes([int(super_weapon['disableable'])]))
        emu.vector(RULES + BUILD_TECH, [q] if super_weapon['build_tech'] else [])
    else:
        emu.vector(RULES + BUILD_TECH, [])
    emu.write32(GAME_MODE, house.get('game_mode', 1))
    emu.uc.mem_write(SW_ALLOWED, bytes([int(house.get('super_weapons', True))]))
    emu.write32(HOUSE_TYPE + 0xB8, house.get('country', 1))
    emu.write32(HOUSE + 0x1D4, house.get('tech', 10))
    emu.uc.mem_write(HOUSE + 0x1EC, bytes([int(house.get('human', True)),
                                           int(house.get('control', house.get('human', True)))]))
    emu.write32(HOUSE + 0x2D4, house.get('docks', 0))
    emu.uc.mem_write(HOUSE + 0x2BC, bytes(int(b) for b in house.get('stolen', [0, 0, 0])))
    for name, (_code, base) in GROUPS.items():
        emu.vector(RULES + base, [int(m[1:]) for m in row.get('groups', {}).get(name, [])])
    alternate = row.get('proc_alternate')
    emu.write32(RULES + PROC_ALTERNATE, emu.types[alternate] if alternate else 0)
    emu.vector(RULES + PAD_AIRCRAFT, [emu.types[p] for p in row.get('pads', [])])
    for counters, bases in (('owned', OWNED), ('active', ACTIVE)):
        for counted_kind, name, value in row.get(counters, []):
            emu.counter(bases[counted_kind], emu.index[emu.types[name]], value)
    factories = []
    for slot, factory in enumerate(row.get('factories', [])):
        address = FACTORIES + slot * 0x100
        owner = HOUSE if factory.get('house', 'self') == 'self' else OTHER_HOUSE
        emu.write32(address + 0x6C, owner)
        obj = OBJECTS + slot * 0x100
        emu.write32(obj, OBJECT_VTABLE)
        emu.object_type[obj] = emu.types[factory['object']]
        emu.write32(address + 0x58, obj)
        queued = [emu.types[name] for name in factory.get('queued', [])]
        emu.write32(address + 0x44, emu.list(queued))
        emu.write32(address + 0x50, len(queued))
        if owner == HOUSE:
            emu.write32(HOUSE + SLOTS[factory['kind']], address)
        factories.append(address)
    emu.write32(FACTORY_ARRAY[0], emu.list(factories))
    emu.write32(FACTORY_ARRAY[1], len(factories))
    labs = []
    for slot, lab in enumerate(row.get('secret_labs', [])):
        address = FAKE + 0x600000 + slot * 0x1000
        lab_type = FAKE + 0x680000 + slot * TYPE_SIZE
        emu.write32(address + 0x520, lab_type)
        for family, offset in SECRET_OVERRIDES.items():
            name = lab.get(family)
            emu.write32(lab_type + offset, emu.types[name] if name else 0)
        fallback = lab.get('fallback')
        emu.write32(address + 0x6F4, emu.types[fallback] if fallback else 0)
        labs.append(address)
    if labs:
        emu.vector(HOUSE + 0x110, labs)
    if row['entry'] == 'can_build':
        skip_tech, count = row['args']
        answer = emu.invoke(CAN_BUILD, ecx=HOUSE, args=(q, int(skip_tech), int(count)))
        answer = answer - (1 << 32) if answer & 0x80000000 else answer
        if answer not in (-1, 0, 1):
            raise OracleError(f'CanBuild answered {answer}')
        return answer
    return emu.invoke(CHECK_BUILD_LIMIT, ecx=HOUSE, args=(q,)) & 0xFF


# ---------------------------------------------------------------- can_build

def can_build_row(name, *, cls='unit', args=(False, True), type_=None, house=None, **fixture):
    row = dict(name=name, entry='can_build', args=list(args), **{'class': cls},
               type=type_ or {}, house=house or {}, **fixture)
    row['answer'] = run(row)
    return row


def can_build():
    rows = []
    add = rows.append
    computer = {'human': False}
    # The gates ahead of the prerequisites.
    add(can_build_row('override on the map skips TechLevel -1',
                      type_={'tech': -1, 'override': ['B3']},
                      active=[['building', 'B3', 1]]))
    add(can_build_row('override off the map', type_={'tech': -1, 'override': ['B3']},
                      owned=[['building', 'B3', 1]]))
    add(can_build_row('override skips missing prerequisites',
                      type_={'override': ['B3'], 'prerequisite': ['B1']},
                      active=[['building', 'B3', 2]]))
    add(can_build_row('TechLevel -1', type_={'tech': -1}))
    for slot in range(3):
        stolen = [False] * 3
        stolen[slot] = True
        add(can_build_row(f'stolen tech byte {slot}', type_={'stolen': stolen}))
        add(can_build_row(f'stolen tech byte {slot}, stolen', type_={'stolen': stolen},
                          house={'stolen': stolen}))
        add(can_build_row(f'stolen tech byte {slot}, the other two stolen',
                          type_={'stolen': stolen}, house={'stolen': [not b for b in stolen]}))
    add(can_build_row('RequiredHouses without the house', type_={'required': [0, 2]}))
    add(can_build_row('RequiredHouses with the house', type_={'required': [1, 2]}))
    add(can_build_row('ForbiddenHouses with the house', type_={'forbidden': [1]}))
    add(can_build_row('ForbiddenHouses without the house', type_={'forbidden': [0, 3]}))
    for allowed in (False, True):
        for disableable in (False, True):
            for build_tech in (False, True):
                add(can_build_row(
                    f'super weapon: allowed {allowed}, disableable {disableable}, '
                    f'BuildTech {build_tech}', cls='building',
                    type_={'super_weapon': {'disableable': disableable,
                                            'build_tech': build_tech}},
                    house={'super_weapons': allowed}))
    add(can_build_row('TechLevel above the house', type_={'tech': 11}))
    add(can_build_row('TechLevel at the house', type_={'tech': 10}))
    add(can_build_row('computer: TechLevel above', type_={'tech': 11}, house=computer))
    add(can_build_row('computer: prerequisites unmet', type_={'prerequisite': ['B1', 'POWER']},
                      house=computer, groups={'POWER': ['B0']}))
    add(can_build_row('computer: at its build limit', type_={'limit': 1}, house=computer,
                      owned=[['unit', 'Q', 3]]))
    add(can_build_row('campaign: PlayerControl is human', type_={'prerequisite': ['B1']},
                      house={'human': False, 'control': True, 'game_mode': 0}))
    add(can_build_row('skirmish: PlayerControl is not human', type_={'prerequisite': ['B1']},
                      house={'human': False, 'control': True, 'game_mode': 1}))
    # The human prerequisite arm.
    for group in GROUPS:
        add(can_build_row(f'{group} member on the map', type_={'prerequisite': [group]},
                          groups={group: ['B0', 'B1']}, active=[['building', 'B1', 1]]))
        add(can_build_row(f'{group} members off the map', type_={'prerequisite': [group]},
                          groups={group: ['B0', 'B1']}, owned=[['building', 'B1', 1]]))
        add(can_build_row(f'{group} empty', type_={'prerequisite': [group]}))
    add(can_build_row('POWER member count negative', type_={'prerequisite': ['POWER']},
                      groups={'POWER': ['B0']}, active=[['building', 'B0', -1]]))
    add(can_build_row('PROC by the alternate unit', type_={'prerequisite': ['PROC']},
                      groups={'PROC': ['B2']}, proc_alternate='U1',
                      active=[['unit', 'U1', 1]]))
    add(can_build_row('PROC alternate off the map', type_={'prerequisite': ['PROC']},
                      groups={'PROC': ['B2']}, proc_alternate='U1',
                      active=[['unit', 'U0', 1]], owned=[['unit', 'U1', 1]]))
    add(can_build_row('PROC alternate with an empty list', type_={'prerequisite': ['PROC']},
                      proc_alternate='U1', active=[['unit', 'U1', 2]]))
    add(can_build_row('PROC without an alternate', type_={'prerequisite': ['PROC']},
                      groups={'PROC': ['B2']}, active=[['unit', 'U1', 1]]))
    add(can_build_row('BuildingType on the map', type_={'prerequisite': ['B4']},
                      active=[['building', 'B4', 1]]))
    add(can_build_row('BuildingType off the map', type_={'prerequisite': ['B4']},
                      owned=[['building', 'B4', 1]]))
    add(can_build_row('BuildingType count negative', type_={'prerequisite': ['B4']},
                      active=[['building', 'B4', -1]]))
    add(can_build_row('every entry met', type_={'prerequisite': ['B4', 'POWER', 'B5']},
                      groups={'POWER': ['B0']},
                      active=[['building', 'B4', 1], ['building', 'B5', 1],
                              ['building', 'B0', 1]]))
    add(can_build_row('one entry unmet', type_={'prerequisite': ['B4', 'POWER', 'B5']},
                      groups={'POWER': ['B0']},
                      active=[['building', 'B4', 1], ['building', 'B0', 1]]))
    # The build limit, through the full path and alone.
    for cls in ('unit', 'infantry', 'aircraft', 'building'):
        kind, other = KIND[cls], OTHER[cls]
        for args in ((False, True), (True, True), (True, False)):
            add(can_build_row(f'{cls} below its limit {args}', cls=cls, args=args,
                              type_={'limit': 2}, owned=[[cls, 'Q', 1]]))
            add(can_build_row(f'{cls} at its limit {args}', cls=cls, args=args,
                              type_={'limit': 2}, owned=[[cls, 'Q', 2]]))
        add(can_build_row(f'{cls} at its limit, one in production', cls=cls,
                          type_={'limit': 2}, owned=[[cls, 'Q', 2]],
                          factories=[{'kind': kind, 'object': 'Q'}]))
        add(can_build_row(f'{cls} over its limit, one in production', cls=cls,
                          type_={'limit': 2}, owned=[[cls, 'Q', 3]],
                          factories=[{'kind': kind, 'object': 'Q'}]))
        add(can_build_row(f'{cls} at its limit, another type in production', cls=cls,
                          type_={'limit': 2}, owned=[[cls, 'Q', 2]],
                          factories=[{'kind': kind, 'object': other}]))
        add(can_build_row(f'{cls} at its limit, one queued', cls=cls,
                          type_={'limit': 2}, owned=[[cls, 'Q', 2]],
                          factories=[{'kind': kind, 'object': other, 'queued': ['Q']}]))
        add(can_build_row(f'{cls} at its limit, another house producing', cls=cls,
                          type_={'limit': 2}, owned=[[cls, 'Q', 2]],
                          factories=[{'kind': kind, 'house': 'other', 'object': 'Q'}]))
        add(can_build_row(f'{cls} BuildLimit 0', cls=cls, type_={'limit': 0}))
        add(can_build_row(f'{cls} BuildLimit -1', cls=cls, type_={'limit': -1},
                          owned=[[cls, 'Q', 5]]))
        add(can_build_row(f'{cls} BuildLimit INT_MIN', cls=cls,
                          type_={'limit': -0x80000000}))
    return rows


# ---------------------------------------------------------------- check_build_limit

def limit_row(name, *, cls='unit', type_=None, house=None, **fixture):
    row = dict(name=name, entry='check_build_limit', **{'class': cls}, type=type_ or {},
               house=house or {}, **fixture)
    row['answer'] = run(row)
    return row


def check_build_limit():
    rows = []
    add = rows.append
    # Pads A1 and A2; A2 has one on the map and Q (limit 1) five tracked.
    bound = {'airport_bound': True, 'limit': 1}
    for docks, on_map, in_factory, queued in ((4, 1, 'A2', []), (4, 2, 'A2', []),
                                              (0, 0, 'A0', []), (3, 1, 'A2', ['A1']),
                                              (4, 1, 'A0', ['A1']), (5, 0, 'Q', ['Q']),
                                              (2, 0, 'A1', ['A2'])):
        add(limit_row(f'AirportBound: docks {docks}, A1 on map {on_map}, factory '
                      f'{in_factory} {queued}', cls='aircraft', type_=bound,
                      house={'docks': docks}, pads=['A1', 'A2'],
                      active=[['aircraft', 'A1', on_map], ['aircraft', 'A2', 1]],
                      owned=[['aircraft', 'A1', 9], ['aircraft', 'Q', 5]],
                      factories=[{'kind': 'aircraft', 'object': in_factory,
                                  'queued': queued}]))
    add(limit_row('AirportBound without a factory', cls='aircraft', type_=bound,
                  house={'docks': 2}, pads=['A1'], active=[['aircraft', 'A1', 1]]))
    add(limit_row('AirportBound, pad counted in another house', cls='aircraft', type_=bound,
                  house={'docks': 2}, pads=['A1'], active=[['aircraft', 'A1', 1]],
                  factories=[{'kind': 'aircraft', 'house': 'other', 'object': 'A1'}]))
    for cls in ('unit', 'infantry', 'aircraft', 'building'):
        kind, other = KIND[cls], OTHER[cls]
        add(limit_row(f'{cls} below its limit', cls=cls, type_={'limit': 3},
                      owned=[[cls, 'Q', 1]],
                      factories=[{'kind': kind, 'object': 'Q'}]))
        add(limit_row(f'{cls} reaches its limit with the queue', cls=cls, type_={'limit': 3},
                      owned=[[cls, 'Q', 1]],
                      factories=[{'kind': kind, 'object': 'Q', 'queued': [other, 'Q']}]))
        add(limit_row(f'{cls} limit 0', cls=cls, type_={'limit': 0}))
        add(limit_row(f'{cls} limit -2, one queued', cls=cls, type_={'limit': -2},
                      owned=[[cls, 'Q', 7]],
                      factories=[{'kind': kind, 'object': other, 'queued': ['Q']}]))
        add(limit_row(f'{cls} limit -2, two in the factory', cls=cls, type_={'limit': -2},
                      factories=[{'kind': kind, 'object': 'Q', 'queued': ['Q']}]))
        add(limit_row(f'{cls} limit INT_MIN', cls=cls, type_={'limit': -0x80000000}))
        add(limit_row(f'{cls} counted in another factory', cls=cls, type_={'limit': 2},
                      owned=[[cls, 'Q', 1]],
                      factories=[{'kind': 'ship', 'object': 'Q', 'queued': ['Q']}]))
    add(limit_row('naval unit asks the ship factory', type_={'limit': 2, 'naval': True},
                  owned=[['unit', 'Q', 1]], factories=[{'kind': 'ship', 'object': 'Q'}]))
    add(limit_row('naval unit ignores the vehicle factory', type_={'limit': 2, 'naval': True},
                  owned=[['unit', 'Q', 1]], factories=[{'kind': 'vehicle', 'object': 'Q'}]))
    add(limit_row('limit reached by the tracked count alone', type_={'limit': 2},
                  owned=[['unit', 'Q', 2]], active=[['unit', 'Q', 0]]))
    add(limit_row('limit ignores the on-map count', type_={'limit': 2},
                  active=[['unit', 'Q', 5]]))
    return rows


def generate():
    return dict(can_build=can_build(), check_build_limit=check_build_limit())


# ---------------------------------------------------------- SecretLab profile

ASSIGN_SECRET_PRODUCTION = 0x68C050
GET_SECRET_PRODUCTION = 0x459840
SECRET_OVERRIDES = {'infantry': 0xEA4, 'unit': 0xEA8, 'building': 0xEAC}
SECRET_LISTS = {'infantry': 0xD00, 'unit': 0xD1C, 'building': 0xD38}
RNG_STREAMS = {'scenario': SCENARIO + 0x218, 'main': 0x886B88, 'mapgen': 0xABE890}


def secret_assignment(name, seed, lab_count, counts=(4, 0, 0), override=None,
                      initial_fallback=None):
    """Whole assignment/getter, original seed/ranged/raw RNG and vector bodies."""
    from unicorn import UC_HOOK_CODE
    from unicorn.x86_const import UC_X86_REG_ESP

    emu = Emu()
    u = emu.uc
    for pointer in RNG_STREAMS.values():
        emu.invoke(0x65C6D0, ecx=pointer, args=(seed,))
    families = []
    names = {0: None}
    for family, (offset, count) in enumerate(zip(SECRET_LISTS.values(), counts)):
        items = FAKE + 0x100000 + family * 0x1000
        pointers = [FAKE + 0x120000 + family * 0x10000 + index * 0x1000
                    for index in range(count)]
        emu.write32(RULES + offset + 4, items)
        emu.write32(RULES + offset + 0x10, count)
        for index, pointer in enumerate(pointers):
            emu.write32(items + index * 4, pointer)
            names[pointer] = f'{tuple(SECRET_LISTS)[family]}{index}'
        families.append(pointers)
    emu.write32(RULES + 0xD54, sum(counts))
    retained = FAKE + 0x1F0000
    names[retained] = 'retained'
    labs = [FAKE + 0x200000 + index * 0x1000 for index in range(lab_count)]
    emu.write32(0x8B41E4, FAKE + 0x180000)
    emu.write32(0x8B41F0, lab_count)
    for index, pointer in enumerate(labs):
        emu.write32(FAKE + 0x180000 + index * 4, pointer)
        lab_type = FAKE + 0x300000 + index * TYPE_SIZE
        emu.write32(pointer + 0x520, lab_type)
        emu.write32(pointer + 0x6F4, retained if initial_fallback else 0)
        if override is not None:
            family, member = override
            emu.write32(lab_type + tuple(SECRET_OVERRIDES.values())[family],
                        families[family][member])
    calls, raw_stores = [], []
    trace = native.NativeCallTrace(u, emu.read32, calls)

    def observe(_u, pc, _size, _data):
        sp = u.reg_read(UC_X86_REG_ESP)
        trace.returned(pc, sp)
        if pc == 0x65C7E0:
            trace.entered(pc, sp, ('RandomRanged', 2, 8))
        elif pc in (0x65C79D, 0x65C84B):
            raw_stores.append(hex(pc))

    hook = u.hook_add(UC_HOOK_CODE, observe)
    before = {key: bytes(u.mem_read(pointer, 0x3F4))
              for key, pointer in RNG_STREAMS.items()}
    emu.invoke(ASSIGN_SECRET_PRODUCTION, ecx=SCENARIO,
               required=(ASSIGN_SECRET_PRODUCTION,))
    trace.returned(native.RET_MAGIC, u.reg_read(UC_X86_REG_ESP))
    after = {key: bytes(u.mem_read(pointer, 0x3F4))
             for key, pointer in RNG_STREAMS.items()}
    u.hook_del(hook)
    assert not trace.pending
    assert before['main'] == after['main'] and before['mapgen'] == after['mapgen']
    assignments = [names[emu.read32(pointer + 0x6F4)] for pointer in labs]
    getters = [names[emu.invoke(GET_SECRET_PRODUCTION, ecx=pointer,
                               required=(GET_SECRET_PRODUCTION,))]
               for pointer in labs]
    return dict(name=name, seed=seed, lab_count=lab_count, counts=list(counts),
                override=override, initial_fallback=initial_fallback,
                assignments=assignments, getters=getters,
                ranged=[dict(low=call['args'][0], high=call['args'][1],
                             result=call['return_eax'], caller=call['caller'])
                        for call in calls], raw_store_count=len(raw_stores),
                raw_stores=raw_stores,
                rng_before={key: value.hex() for key, value in before.items()},
                rng_after={key: value.hex() for key, value in after.items()})


def secret_getter(name, overrides, fallback):
    emu = Emu()
    lab, lab_type = FAKE + 0x200000, FAKE + 0x300000
    names = {0: None}
    pointers = {}
    for value in (*overrides.values(), fallback):
        if value is not None and value not in pointers:
            pointer = FAKE + 0x100000 + len(pointers) * 0x1000
            pointers[value] = pointer
            names[pointer] = value
    emu.write32(lab + 0x520, lab_type)
    for family, offset in SECRET_OVERRIDES.items():
        emu.write32(lab_type + offset, pointers.get(overrides.get(family), 0))
    emu.write32(lab + 0x6F4, pointers.get(fallback, 0))
    result = emu.invoke(GET_SECRET_PRODUCTION, ecx=lab,
                        required=(GET_SECRET_PRODUCTION,))
    return dict(name=name, overrides=overrides, fallback=fallback, result=names[result])


def secret_admission():
    rows = []
    for human in (False, True):
        for label, spec in (
            ('negative tech', {'tech': -1}), ('high tech', {'tech': 11}),
            ('required country', {'required': [2]}),
            ('forbidden country', {'forbidden': [1]}),
            ('prerequisite absent', {'prerequisite': ['B3']}),
            ('stolen tech absent', {'stolen': [True, False, False]}),
            ('build limit reached', {'limit': 1}),
        ):
            for grant in (False, True):
                rows.append(can_build_row(
                    f'{"human" if human else "AI"}: {label}, grant {grant}',
                    cls='infantry', type_=spec, house={'human': human},
                    owned=[['infantry', 'Q', 1]],
                    secret_labs=[{'infantry': 'Q'}] if grant else []))
    for kind, member in (('unit', 'unit'), ('building', 'building')):
        rows.append(can_build_row(
            f'{kind}: matching typed override', cls=kind, type_={'tech': -1},
            secret_labs=[{member: 'Q'}]))
    rows.append(can_build_row('fallback grants the asked infantry', cls='infantry',
                              type_={'tech': -1}, secret_labs=[{'fallback': 'Q'}]))
    rows.append(can_build_row('first lab misses, second lab grants', cls='infantry',
                              type_={'tech': -1},
                              secret_labs=[{'infantry': 'I0'}, {'infantry': 'Q'}]))
    rows.append(can_build_row('higher priority override masks matching fallback',
                              cls='infantry', type_={'tech': -1},
                              secret_labs=[{'infantry': 'I0', 'fallback': 'Q'}]))
    return rows


def secret_readers():
    from tools.rules_oracle.bridge_anim_inputs import Reader
    from tools.projectile_oracle.bridge_render_inputs import lexical
    from tools.rules_oracle.weapon_laser import physical_layers
    from tools.spatial_oracle.building_body_rules import INI, SP, dwords
    from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP,
                                   UC_X86_REG_EBX, UC_X86_REG_EDI,
                                   UC_X86_REG_ESI, UC_X86_REG_ESP)

    class SecretReader(Reader):
        """Only this mechanism's objects and entry slices; shared VM/cache/heap."""

        def __init__(self):
            super().__init__(Path(__file__).with_name('no-assets'), {})
            # Declared registered identities. The whole FindOrAllocate searches
            # execute; none of these controls needs an unknown-type constructor.
            for array, names, vtable in (
                (0xA8E348, ('I0', 'I1', 'SNIPE', 'TERROR', 'DESO', 'YURI'), 0x7EAC28),
                (0xA83CE0, ('U0', 'U1', 'TNKD', 'TTNK', 'DTRUCK'), 0x7EAC48),
                (0xA83C68, ('B0', 'B1', 'GTGCAN'), 0x7ED90C),
            ):
                pointers = []
                for name in names:
                    pointer = self.alloc(0x1800)
                    self.u.mem_write(pointer + 0x24, name.encode('ascii') + b'\0')
                    pointers.append(pointer)
                items = self.alloc(4096)
                self.u.mem_write(items, dwords(*pointers))
                self.u.mem_write(array, dwords(vtable, items, 1024, 0x0101,
                                               len(pointers), 10))
            self.rules = self.alloc(0x2000)
            self.lab_type = self.alloc(0x1800)
            self.u.reg_write(UC_X86_REG_ESP, SP)
            self.u.reg_write(UC_X86_REG_ESI, self.rules)
            self.u.reg_write(UC_X86_REG_EBX, 0)
            self.u.reg_write(UC_X86_REG_EDI, 10)
            native.run_checked(self.u, 0x666773, 0x6667F8)
            self.u.reg_write(UC_X86_REG_ESI, self.lab_type)
            native.run_checked(self.u, 0x45DE94, 0x45DEA6)
            native.run_checked(self.u, 0x45E0EA, 0x45E0F0)

        def secret_state(self):
            lists = {}
            for family, offset in SECRET_LISTS.items():
                items = self.read32(self.rules + offset + 4)
                count = self.read32(self.rules + offset + 0x10)
                lists[family] = [self.string(self.read32(items + index * 4) + 0x24)
                                 for index in range(count)]
            overrides = {}
            for family, offset in SECRET_OVERRIDES.items():
                pointer = self.read32(self.lab_type + offset)
                overrides[family] = self.string(pointer + 0x24) if pointer else None
            return dict(lists=lists, total=self.read32(self.rules + 0xD54),
                        overrides=overrides,
                        secret_lab=bool(self.u.mem_read(self.lab_type + 0x16B0, 1)[0]))

        def read(self, sections, section='LAB'):
            self.make_ini(sections)
            u = self.u
            u.reg_write(UC_X86_REG_ESP, SP)
            u.reg_write(UC_X86_REG_ESI, self.rules)
            u.reg_write(UC_X86_REG_EDI, INI)
            native.run_checked(u, 0x66FA13, 0x66FAD6,
                               required_addresses=(0x67BB10, 0x67B720, 0x67B550, 0x528A10))
            assert u.reg_read(UC_X86_REG_ESP) == SP
            u.reg_write(UC_X86_REG_EBP, self.lab_type)
            u.reg_write(UC_X86_REG_EBX, self.cstring(section))
            u.reg_write(UC_X86_REG_ESI, INI)
            u.reg_write(UC_X86_REG_EAX, 0)
            native.run_checked(u, 0x4605BB, 0x460683,
                               required_addresses=(0x528A10,))
            native.run_checked(u, 0x4609B6, 0x4609D6,
                               required_addresses=(0x5295F0,))
            assert u.reg_read(UC_X86_REG_ESP) == SP
            return dict(sections=sections, state=self.secret_state())

    reader = SecretReader()
    constructor = reader.secret_state()
    history = []
    for name, keys in (
        ('seed lists', {'SecretInfantry': 'I0,I1,I0', 'SecretUnits': 'U1',
                        'SecretBuildings': 'B0'}),
        ('missing keys retain', {}),
        ('empty strings retain', {'SecretInfantry': '', 'SecretUnits': '',
                                   'SecretBuildings': ''}),
        ('whitespace strings retain', {'SecretInfantry': '   ', 'SecretUnits': '\t',
                                        'SecretBuildings': '   '}),
        ('case insensitive identity and duplicates', {'SecretInfantry': 'i1,I0,i1',
                                                      'SecretUnits': 'u0,U0',
                                                      'SecretBuildings': 'b1,B0,b1'}),
        ('none entries dropped', {'SecretInfantry': 'none,I0,<none>,I0',
                                  'SecretUnits': 'none,U1,<none>',
                                  'SecretBuildings': '<none>,B0'}),
        ('wrong case keys retain', {'secretinfantry': 'I1', 'secretunits': 'U0',
                                    'secretbuildings': 'B1'}),
        ('commas replace with empty', {'SecretInfantry': ',,,', 'SecretUnits': ',,,',
                                       'SecretBuildings': ',,,'}),
        ('empty after clear stays empty', {}),
    ):
        history.append(dict(name=name, **reader.read({'General': keys})))
    scalar_reader = SecretReader()
    scalar_history = []
    for name, keys in (
        ('defaults', {}),
        ('three overrides and flag', {'SecretInfantry': 'I0', 'SecretUnit': 'U0',
                                      'SecretBuilding': 'B0', 'SecretLab': 'yes'}),
        ('missing retains', {}),
        ('empty retains', {'SecretInfantry': '', 'SecretUnit': '',
                            'SecretBuilding': '', 'SecretLab': ''}),
        ('invalid bool retains', {'SecretLab': 'junk'}),
        ('none clears', {'SecretInfantry': 'none', 'SecretUnit': '<none>',
                         'SecretBuilding': 'NoNe'}),
        ('case insensitive references', {'SecretInfantry': 'i1', 'SecretUnit': 'u1',
                                         'SecretBuilding': 'b1', 'SecretLab': 'no'}),
        ('wrong case keys retain', {'secretinfantry': 'I0', 'secretunit': 'U0',
                                    'secretbuilding': 'B0', 'secretlab': 'yes'}),
        ('whitespace references retain', {'SecretInfantry': '   ', 'SecretUnit': '\t',
                                          'SecretBuilding': '   '}),
    ):
        scalar_history.append(dict(name=name, **scalar_reader.read({'LAB': keys})))
    physical_reader = SecretReader()
    physical = []
    general_keys = ('SecretInfantry', 'SecretUnits', 'SecretBuildings')
    type_keys = ('SecretLab', 'SecretInfantry', 'SecretUnit', 'SecretBuilding')
    for filename, archive, raw in physical_layers():
        if raw is None:
            physical.append(dict(file=filename, archive=archive, absent=True))
            continue
        selected, _ = lexical(raw, {'General', 'CASLAB'})
        sections = {name: {key: value for key, value in keys.items()
                            if key in (general_keys if name == 'General' else type_keys)}
                    for name, keys in selected.items()}
        physical.append(dict(file=filename, archive=archive, bytes=len(raw),
                             sha256=hashlib.sha256(raw).hexdigest(),
                             **physical_reader.read(sections, 'CASLAB')))
    return dict(constructor=constructor, retained_history=history,
                scalar_history=scalar_history, physical_layers=physical)


def secret_lab_generate():
    assignments = [
        secret_assignment('no_labs', 1, 0),
        secret_assignment('one_lab', 1, 1),
        secret_assignment('four_labs', 1, 4),
        secret_assignment('different_seed_four_labs', 0x12345678, 4),
        secret_assignment('more_labs_than_choices', 1, 5, initial_fallback='retained'),
        secret_assignment('mixed_family_boundary', 1, 6, (2, 2, 2)),
        secret_assignment('no_choices', 1, 1, (0, 0, 0), initial_fallback='retained'),
        secret_assignment('one_choice_equal_bound', 1, 1, (1, 0, 0)),
        secret_assignment('infantry_override_still_draws', 1, 2, override=(0, 3)),
        secret_assignment('unit_override_still_draws', 1, 2, (2, 2, 2), override=(1, 1)),
        secret_assignment('building_override_still_draws', 1, 2, (2, 2, 2), override=(2, 1)),
    ]
    getters = [
        secret_getter('all empty', {}, None),
        secret_getter('fallback only', {}, 'fallback'),
        secret_getter('infantry wins all', {'infantry': 'I', 'unit': 'U', 'building': 'B'}, 'fallback'),
        secret_getter('unit wins building and fallback', {'unit': 'U', 'building': 'B'}, 'fallback'),
        secret_getter('building wins fallback', {'building': 'B'}, 'fallback'),
        secret_getter('infantry only', {'infantry': 'I'}, None),
        secret_getter('unit only', {'unit': 'U'}, None),
        secret_getter('building only', {'building': 'B'}, None),
    ]
    return dict(schema_version=1, native_sha256=native.image_sha256(),
                assignment=assignments, getter=getters, admission=secret_admission(),
                readers=secret_readers())


def secret_lab_metadata():
    return provenance(
        scope='Original SecretLab startup assignment/RNG, grant priority, CanBuild '
              'early grant with build limits, and retained General/BuildingType key blocks',
        assumptions=[
            'Assignment uses declared registered typed lists/counts/total and the global '
            'lab array, not an on-map House list. Original whole68C050,459840,65C6D0 '
            'and65C7E0 execute; all three full0x3F4 RNG states and actual ranged/raw '
            'advance observations are recorded. Initial fallback is declared only in '
            'the skipped-assignment controls. Whole Post_Map_Init is outside this fixture.',
            'Admission reuses BuildEmu type identities/counters/factories; supplied '
            'House+110 membership and override/fallback pointers model prior lab lifecycle. '
            'Original CanBuild4F7870 and GetSecretProduction459840 execute. This does '
            'not execute capture, list registration/removal, factory production or sidebar.',
            'Reader reuses bridge_anim_inputs.Reader cached INI/heap/TLS owners. Original '
            'General constructors666773..6667F8, list block66FA13..66FAD6, three whole '
            'typed list readers, scalar override block4605BB..460683 and flag block '
            '4609B6..4609D6 execute. Registered names are supplied; these controls need '
            'no unknown-type constructor. Whole Rules Process/BuildingType reader, '
            'INI file loading and live archive precedence are outside this fixture.',
            'Physical key strings come from the existing weapon_laser.physical_layers '
            'archive reader: RULESMD(expandmd01), optional loose LANGRULE, Battle(localmd), '
            'XMP03T4(multimd), in that retained order. Payload pins file hashes/lengths. '
            'The existing lexical owner rejects selected duplicate sections/keys; no '
            'VERA-interpreted scalar or list result supplies a native output.',
        ],
        substitutions=[
            'Existing Emu bounded bump allocator, no-op delete/CRT registration and '
            'initial adjacent-cell CRT transport; no assignment/getter/RNG answer is supplied.',
            'Existing BuildEmu type RTTI/index/object-type virtual answers for admission; '
            'counter growth is excluded by supplied sized arrays.',
            'Existing Reader cached INI objects, bump allocator/delete and CRT TLS '
            'transport; no key parser, FindOrAllocate result, list copy or gameplay '
            'return is substituted.',
        ],
        entry_points=dict(assign_secret_production=ASSIGN_SECRET_PRODUCTION,
                          get_secret_production=GET_SECRET_PRODUCTION,
                          can_build=CAN_BUILD, check_build_limit=CHECK_BUILD_LIMIT,
                          random_seed=0x65C6D0, random_ranged=0x65C7E0,
                          general_lists=0x66FA13, infantry_list=0x67BB10,
                          unit_list=0x67B720, building_list=0x67B550,
                          scalar_overrides=0x4605BB, secret_lab_flag=0x4609B6,
                          read_string=0x528A10, read_bool=0x5295F0))


def metadata():
    return provenance(
        scope=('HouseClass::CanBuild over its gates, the human prerequisite arm and the build '
               'limit with its factory scan; HouseClass::CheckBuildLimit over AirportBound '
               'docks and build limits with the factory of each class. Synthetic fixtures '
               'only; see the module docstring for what the house never holds.'),
        assumptions=['Fixture objects carry only the fields the entries read.',
                     'Every HouseClass counter is sized past the indexes asked, so '
                     'CounterClass growth (vtable +8) never runs.'],
        substitutions=['WhatAmI (vtable +0x2C) and GetArrayIndex (+0x40) of a type, and '
                       'GetTechnoType (+0x84) of a factory\'s object, answer from the case.'],
        entry_points={'can_build': CAN_BUILD, 'check_build_limit': CHECK_BUILD_LIMIT},
    )


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    if '--secret-lab' in args:
        args.remove('--secret-lab')
        finish_vectors(secret_lab_generate, Path(__file__).with_name('can_build_secret_lab.json'),
                       provenance=secret_lab_metadata, argv=args)
    else:
        finish_vectors(generate, Path(__file__).with_suffix('.json'),
                       provenance=metadata, argv=args)


if __name__ == '__main__':
    main()
