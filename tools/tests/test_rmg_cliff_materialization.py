"""Portable producer/transport contract tests; no native execution or goldens."""
from copy import deepcopy
from pathlib import Path
import struct
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from tools.rmg_oracle import cliff_materialization as cliff
from tools.spatial_oracle import tmp_clear_render as tmp


class CliffControlTests(unittest.TestCase):
    def test_controls_are_inputs_with_complete_bounded_dispatch_coverage(self):
        rows = cliff.controls()
        self.assertEqual(len({r['id'] for r in rows}), len(rows))
        offsets = {r['offset'] for r in rows if r['kind'] == 'variant'}
        self.assertTrue(set(range(-1, 41)) <= offsets)
        self.assertTrue(any(r.get('advance') == 249 for r in rows))
        self.assertTrue(any(r.get('locked') for r in rows))
        self.assertTrue(any(r.get('raw') == [0xFFFFFFFF, 0] for r in rows))
        self.assertTrue(all(not {'expected', 'result', 'mask', 'selected_piece'} & r.keys()
                            for r in rows))
        for row in rows:
            for item in row['cells']:
                x, y = item['coords']
                self.assertTrue(0 <= x < 32 and 0 <= y < 32)
        missing = [r for r in rows if r.get('missing')]
        self.assertTrue(missing)
        self.assertTrue(all(r['allow_dummy'] for r in missing))
        joined = [r for r in rows if r['kind'] == 'joined']
        self.assertTrue(joined)
        self.assertTrue(all(not any('family' in c or 'subtile' in c for c in r['cells'])
                            for r in joined), 'Joined restamp must receive native materialization, not supplied cliff metadata')
        self.assertTrue(all(r.get('physical_valid_subtiles')
                            for r in rows if r['kind'] == 'restamp' and r['cells']))

    def test_rejects_ambiguous_missing_and_out_of_domain_inputs(self):
        invalid = [
            [dict(id='bad', kind='mask', cells=[], missing=[[17, 16]])],
            [dict(id='bad', kind='mask', cells=[cliff.cell(32, 16)])],
            [dict(id='bad', kind='mask', cells=[cliff.cell(16, 16, expected=7)])],
            [dict(id='bad', kind='variant', offset=4, cells=[], raw=[-1])],
            [dict(id='same', kind='mask'), dict(id='same', kind='mask')],
            [dict(id='bad', kind='joined', cells=[], scratch=False)],
        ]
        for rows in invalid:
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                cliff.validate_controls(rows)

    def test_rng_interning_preserves_all_bytes_and_stream_identity(self):
        pool = cliff.StatePool()
        before = dict(main='00ff', mapgen='1234', scenario='00ff')
        saved = deepcopy(before)
        refs = pool.refs(before)
        self.assertEqual(before, saved)
        self.assertEqual({name: pool.states[index] for name, index in refs.items()}, before)
        again = pool.refs(dict(main='00fe', mapgen='1234', scenario='00ff'))
        self.assertEqual(refs['mapgen'], again['mapgen'])
        self.assertNotEqual(refs['main'], again['main'])
        self.assertEqual(len(pool.states), 3)

    def test_call_compaction_requires_every_return_and_preserves_effect_order(self):
        def observed(name):
            return dict(name=name, entry='0x579620', return_pc='0x30000000',
                        args=[1, 7], return_eax=1)
        calls = [observed('clear'), observed('next'), observed('shared_stamp'), observed('clear')]
        saved = deepcopy(calls)
        compact = cliff.compact_calls(calls)
        self.assertEqual(calls, saved)
        self.assertEqual([r['name'] for r in compact['ordered']], ['next', 'shared_stamp'])
        self.assertEqual(next(r['count'] for r in compact['counts'] if r['name'] == 'clear'), 2)
        with self.assertRaisesRegex(ValueError, 'Unreturned'):
            cliff.compact_calls([dict(name='mask', entry='0x579b70')])


class Memory:
    def __init__(self):
        self.bytes = {}

    def mem_write(self, address, value):
        self.bytes.update((address + i, b) for i, b in enumerate(value))

    def mem_read(self, address, size):
        return bytes(self.bytes.get(address + i, 0) for i in range(size))

    def hook_add(self, *_args):
        return 1

    def reg_write(self, *_args):
        pass


class RetainedCellReadTests(unittest.TestCase):
    def test_observation_reads_pointer_fields_without_a_fresh_lookup(self):
        fixture = cliff.CliffFixture.__new__(cliff.CliffFixture)
        fixture.u = Memory()
        fixture.pointer_coords = {0x10000: (16, 17)}
        fixture.native = SimpleNamespace(OracleError=ValueError)
        for pointer, xy, tile, fields in ((0x10000, (16, 17), 53, (2, 7, 19)),
                                          (cliff.DUMMY, (31, 18), 65535, (0, 0, 0))):
            fixture.u.mem_write(pointer + 0x24, struct.pack('<hh', *xy))
            fixture.u.mem_write(pointer + 0x38, struct.pack('<i', tile))
            fixture.u.mem_write(pointer + 0x11A, bytes(fields))
            actual = fixture.cell_observation(pointer)
            self.assertEqual(actual['coords'], list(xy))
            self.assertEqual(actual['tile'], tile)
            self.assertEqual([actual[k] for k in ('subtile', 'level', 'slope')], list(fields))
        self.assertEqual(fixture.cell_observation(cliff.DUMMY)['owner'], 'dummy')
        with self.assertRaisesRegex(ValueError, 'Unenrolled'):
            fixture.cell_observation(0x20000)


class TmpOwner:
    """Only IO/constructor-call observations; it does not emulate TMP logic."""
    def __init__(self, files):
        self.u, self.files, self.cursor = Memory(), files, 0x10000
        self.constructors = []
        self.names = {}

    def alloc(self, size):
        address = self.cursor
        self.cursor += (size + 15) & ~15
        return address

    def cstring(self, value):
        address = self.alloc(len(value) + 1)
        self.names[address] = value
        return address

    def read32(self, address):
        return struct.unpack('<I', self.u.mem_read(address, 4))[0]

    def invoke(self, entry, receiver, args=()):
        if entry == 0x5447C0:
            self.constructors.append((receiver, args))
            return 0
        if entry != 0x544CB0:
            raise AssertionError(f'Unexpected producer call {entry:08X}')
        name = bytes(self.u.mem_read(receiver + 0x2F5, 128)).split(b'\0')[0].decode()
        image = self.alloc(16)
        self.u.mem_write(image, self.files[name][:16])
        self.u.mem_write(receiver + 0xA4, tmp.dwords(image, 1))
        return image


class Platform:
    def __init__(self, *_args):
        self.file_io = []

    def configure_transport(self, **_args):
        pass

    def hook(self, *_args):
        pass


class TmpIdentityTests(unittest.TestCase):
    def load(self, files, **kwargs):
        owner = TmpOwner(files)
        with patch.object(tmp, 'NativeAudioPlatform', Platform), \
             patch.object(tmp, 'configured_gamemd', return_value=Path('/unused/gamemd.exe')), \
             patch.object(tmp, 'run_checked'):
            tmp.load_tmp_files(owner, files, **kwargs)
        return [args for _, args in owner.constructors]

    def test_legacy_constructor_ids_and_interleave_are_preserved(self):
        files = {'a.tem': bytes(range(16)), 'b.tem': bytes(range(16, 32))}
        args = self.load(files)
        self.assertEqual([a[0] for a in args], [0, 0])
        self.assertEqual([a[1:3] for a in args], [(-65, 0), (-65, 0)])
        self.assertEqual([a[4] for a in args], [False, False])

    def test_per_registration_ids_reach_first_constructor_argument(self):
        files = {'a.tem': bytes(range(16)), 'b.tem': bytes(range(16, 32))}
        args = self.load(files, chain=True, tile_ids={'a.tem': 49, 'b.tem': 50})
        self.assertEqual([a[0] for a in args], [49, 50])
        self.assertEqual([a[4] for a in args], [False, True])
        # Partial identities retain the old default for every unspecified file.
        args = self.load(files, tile_ids={'b.tem': 50})
        self.assertEqual([a[0] for a in args], [0, 50])

    def test_invalid_identity_fails_before_any_platform_or_native_call(self):
        files = {'a.tem': bytes(16)}
        for ids in ({'missing.tem': 1}, {'a.tem': -1}, {'a.tem': True}, {'a.tem': 1 << 31}):
            with self.subTest(ids=ids), patch.object(tmp, 'NativeAudioPlatform') as platform:
                with self.assertRaises(ValueError):
                    tmp.load_tmp_files(TmpOwner(files), files, tile_ids=ids)
                platform.assert_not_called()


if __name__ == '__main__':
    unittest.main()
