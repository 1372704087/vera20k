"""Original high bridge Cell callers, retail SHP and RGB565/Z pixels.

Run with --check (default) or --write. The shared native image, lexical INI,
archive, Convert-construction and shape owners provide the fixture boundaries;
this module executes original bridge caller and raster instructions unchanged.
"""
from pathlib import Path
import hashlib
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EBP, UC_X86_REG_EDI, UC_X86_REG_ESP,
)
from tools.native_oracle import (
    NATIVE_SHA256, RET_MAGIC, configured_gamemd, finish_vectors, provenance,
    run_checked,
)
from tools.projectile_oracle.bridge_render_inputs import lexical
from tools.projectile_oracle.bridge_render_inputs_palette import PaletteReader, initialize
from tools.palette_oracle import oracle as palette_oracle
from tools.projectile_oracle.bridge_render_shape import (
    SURFACE, ZOBJ, AOBJ, ZSURF, ASURF, PIXELS, Z, A, packed_words,
)
from tools.sidebar_oracle.stock import mix, mix_hash
from tools.spatial_oracle.building_body_rules import RULES, SP, dwords
from tools.spatial_oracle.bridge_constructor import OriginalBridgeConstructor, D as CONSTRUCTOR_MEM

WIDTH, HEIGHT = 192, 160
COUNT = WIDTH * HEIGHT
MEM = 0x21000000
CELL, POINT, CLIP, RECT = [MEM + n for n in (0, 0x1000, 0x1020, 0x1040)]
LEAVES = {0x493830: 'plain_shadow', 0x497390: 'rle_shadow'}
BODY_LEAVES = {0x4990E0: 'rle_body'}
BRIDGE_TYPES = ('BRIDGE1', 'BRIDGE2', 'BRIDGEB1', 'BRIDGEB2')


def ints(u, address, count):
    return list(struct.unpack('<' + 'i' * count, u.mem_read(address, count * 4)))


def physical_assets(*, body=False):
    root = configured_gamemd().parent
    sources, assets = [], {}
    for outer_name, children in (
        ('ra2.mix', {'temperat.mix': ('bridge.tem', 'bridgb.tem'),
                     'snow.mix': ('bridge.sno', 'bridgb.sno'),
                     'urban.mix': ('bridge.urb', 'bridgb.urb'),
                     'cache.mix': ('palette.pal', 'anim.pal')
                         + (('isotem.pal', 'temperat.pal') if body else ())}),
        ('ra2md.mix', {'localmd.mix': ('RULESMD.INI', 'ARTMD.INI', 'MPBattleMD.ini'),
                       'urbann.mix': ('bridge.ubn', 'bridgb.ubn'),
                       'desert.mix': ('bridge.des', 'bridgb.des'),
                       'lunar.mix': ('bridge.lun', 'bridgb.lun')}),
    ):
        outer_raw = (root / outer_name).read_bytes()
        outer = mix(outer_raw)
        for inner_name, names in children.items():
            inner_raw = outer[mix_hash(inner_name)]
            inner = mix(inner_raw)
            for name in names:
                raw = inner[mix_hash(name)]
                assets[name.upper()] = raw
                sources.append(dict(name=name, outer=outer_name, inner=inner_name,
                                    outer_sha256=hashlib.sha256(outer_raw).hexdigest(),
                                    inner_sha256=hashlib.sha256(inner_raw).hexdigest(),
                                    sha256=hashlib.sha256(raw).hexdigest(), bytes=len(raw)))
    return assets, sources


def prepare(*, body=False):
    assets, sources = physical_assets(body=body)
    shadow_bytes, shape_aliases = {}, []
    for name, raw in assets.items():
        image_name = name.split('.')[0]
        if image_name not in ('BRIDGE', 'BRIDGB'):
            continue
        count = struct.unpack_from('<H', raw, 6)[0]
        normalized = bytearray()
        # Compare exact crop/format metadata and compressed bytes, omitting only
        # frame file offsets (body compression changes their absolute values).
        # This is byte equivalence, not a second source-image decoder.
        for frame in range(count // 2, count):
            normalized.extend(raw[8 + 24 * frame:28 + 24 * frame])
            start = struct.unpack_from('<I', raw, 28 + 24 * frame)[0]
            end = (struct.unpack_from('<I', raw, 28 + 24 * (frame + 1))[0]
                   if frame + 1 < count else len(raw))
            normalized.extend(raw[start:end])
        shadow_bytes.setdefault(image_name, []).append(bytes(normalized))
        shape_aliases.append(dict(name=name, raw_frame_count=count,
                                   shadow_frames_sha256=hashlib.sha256(normalized).hexdigest()))
    assert all(len(images) == 6 and all(raw == images[0] for raw in images)
               for images in shadow_bytes.values())
    art, art_lines = lexical(assets['ARTMD.INI'], {*BRIDGE_TYPES, 'BRIDGE', 'BRIDGB'})
    m = PaletteReader(art)
    m.assets.update(assets)
    initialize(m)
    u = m.u
    u.mem_map(MEM, 0x200000)
    # Existing registry constructor, then actual physical registry-prefix names.
    u.reg_write(UC_X86_REG_ESP, SP)
    run_checked(u, 0x4E71E0, 0x4E7216)
    declared, _ = lexical(assets['RULESMD.INI'], {'OverlayTypes'})
    names = list(declared['OverlayTypes'].values())
    names = names[:1 + max(names.index(name) for name in BRIDGE_TYPES)]
    types = {}
    for name in names:
        typ = m.alloc(0x300)
        m.invoke(0x5FE250, typ, (m.cstring(name),))
        types[name] = typ
    m.make_ini(art)
    layers = []
    for filename in ('RULESMD.INI', 'MPBattleMD.ini'):
        sections, lines = lexical(assets[filename.upper()], set(BRIDGE_TYPES))
        m.rules_cache(sections)
        for name in BRIDGE_TYPES:
            typ = types[name]
            mark = len(m.asset_loaded)
            admitted = m.invoke(0x5FE770, typ, (RULES,)) & 255
            layers.append(dict(file=filename, name=name, admitted=admitted,
                               physical_keys=sections.get(name), source_lines=lines,
                               image_name=m.string(typ + 0x1F8),
                               image_present=bool(m.read32(typ + 0xA4)),
                               index=m.read32(typ + 0x294), land=m.read32(typ + 0x298),
                               wall=u.mem_read(typ + 0x2A8, 1)[0],
                               tiberium=u.mem_read(typ + 0x2A9, 1)[0],
                               crate=u.mem_read(typ + 0x2AA, 1)[0],
                               asset_loads=m.asset_loaded[mark:]))
    for address, buffer in ((SURFACE, PIXELS), (ZSURF, Z), (ASURF, A)):
        u.mem_write(address, dwords(0x7E2070, WIDTH, HEIGHT, 0, 2,
                                    buffer, COUNT * 2, 0))
    u.mem_write(0x887314, dwords(SURFACE))
    for address, backing, buffer, global_ in (
        (ZOBJ, ZSURF, Z, 0x887644), (AOBJ, ASURF, A, 0x87E8A4),
    ):
        u.mem_write(address, dwords(0, 0, WIDTH, HEIGHT, 0, backing,
                                    buffer, buffer + COUNT * 2, COUNT * 2, 32768, WIDTH))
        u.mem_write(global_, dwords(address))
    u.mem_write(CELL, dwords(0x7E4EEC))
    u.mem_write(CELL + 0x34, dwords(m.read32(0x87F6C4)))
    inputs = dict(sources=sources, shape_aliases=shape_aliases, registry_prefix=names,
                  physical_art=art, art_source_lines=art_lines, layers=layers)
    if body:
        inputs['cell_convert'] = initialize_cell_convert(m)
    return m, types, inputs


def initialize_cell_convert(m):
    """Original theater palette expansion, default LightConvert and Cell binding.

    Physical palette IO is supplied; the original 5454DB expansion loop,
    545000..5450EF default converter publication and 483E30 Cell writer run.
    The full theater/TMP loader and scenario lighting graph are excluded.
    """
    u = m.u
    palettes = []
    for name, address in (('ISOTEM.PAL', 0xABBED0), ('TEMPERAT.PAL', 0x885780)):
        raw = m.assets[name]
        assert len(raw) == 768
        u.mem_write(address, raw)
        u.reg_write(UC_X86_REG_EAX, address)
        u.reg_write(UC_X86_REG_ESP, SP)
        run_checked(u, 0x5454DB, 0x5454EB,
                    required_addresses=(0x5454E0, 0x5454E2, 0x5454E5))
        expanded = bytes(u.mem_read(address, 768))
        palettes.append(dict(name=name, address=f'{address:08X}',
                             sha256=hashlib.sha256(raw).hexdigest(),
                             expanded_sha256=hashlib.sha256(expanded).hexdigest()))
    # Empty preallocated native registry; the original initializer appends
    # its freshly constructed unity converter before the stop boundary.
    u.mem_write(0x87F698, dwords(0x7EB6D4, m.alloc(32), 8, 1, 0, 10))
    # Same captured RGB565/MMX dispatch inputs as tools.palette_oracle.oracle;
    # the whole ctor/UpdateColors still generates this VM's actual table.
    u.mem_write(0x829D20, dwords(2))
    u.mem_write(0x84E860, bytes((0, 0, 1)))
    u.reg_write(UC_X86_REG_ESP, SP)
    reached = set()
    def observe(_u, pc, _size, _data):
        if pc in (0x54508F, 0x555DA0, 0x556090, 0x7DE200, 0x48EBF0):
            reached.add(pc)
    hook = u.hook_add(UC_HOOK_CODE, observe)
    run_checked(u, 0x545000, 0x5450EF, count=10000000, timeout_us=30000000,
                required_addresses=(0x54508F, 0x555DA0, 0x556090, 0x7DE200, 0x5450EC))
    u.hook_del(hook)
    assert m.read32(0x87F6A8) == 1
    convert = m.read32(m.read32(0x87F69C))
    assert [m.read32(convert + off) for off in (0x188, 0x18C)] == [0xABBED0, 0x885780]
    assert ints(u, convert + 0x198, 3) == [1000] * 3
    assert m.read32(convert + 0x16C) == 53
    palette_table = bytes(u.mem_read(m.read32(convert + 0x170), 53 * 512))
    assert palette_table == palette_oracle.palette_table(
        53, (1000, 1000, 1000), bytes(u.mem_read(0xABBED0, 768)), 'mmx')
    # Explicit unity cell-lighting input; the native owner binds the produced
    # converter, increments its reference and writes all brightness scalars.
    m.invoke(0x483E30, CELL, (convert, 0x10000, 0, 1000, 1000, 1000))
    assert m.read32(CELL + 0x34) == convert
    assert struct.unpack('<3h', u.mem_read(CELL + 0x10A, 6)) == (1000, 1000, 1000)
    return dict(palettes=palettes, rgb=[1000] * 3, shade_count=53,
                source_palette='ISOTEM.PAL', destination_palette='TEMPERAT.PAL',
                cell_brightness=[1000] * 3,
                palette_table_sha256=hashlib.sha256(palette_table).hexdigest(),
                palette_dispatch='RGB565/MMX',
                reached=[f'{pc:08X}' for pc in sorted(reached)])


def snapshot(u, background, old_z):
    colors = list(struct.unpack('<' + 'H' * COUNT, u.mem_read(PIXELS, COUNT * 2)))
    depths = list(struct.unpack('<' + 'H' * COUNT, u.mem_read(Z, COUNT * 2)))
    # Lossless horizontal runs of native output, omitting only unchanged input.
    runs = []
    for y in range(HEIGHT):
        x = 0
        while x < WIDTH:
            offset = y * WIDTH + x
            pair = colors[offset], depths[offset]
            if pair == (background, old_z):
                x += 1
                continue
            start = x
            while x < WIDTH and (colors[y * WIDTH + x], depths[y * WIDTH + x]) == pair:
                x += 1
            runs.append([start, y, x - start, *pair])
    return dict(runs=runs,
                color_sha256=hashlib.sha256(bytes(u.mem_read(PIXELS, COUNT * 2))).hexdigest(),
                depth_sha256=hashlib.sha256(bytes(u.mem_read(Z, COUNT * 2))).hexdigest())


def execute(m, types, case, *, body=False, boundary_only=False, reset_surface=True):
    assert body or not boundary_only
    case = dict(type='BRIDGE1', coords=[10, 20], level=0, state=0,
                flags=0x180, cell_point=[66, 81], clip=[0, 0, WIDTH, HEIGHT],
                viewport_y=0, background=0xFFFF, old_z=65535, repeat=1) | case
    u = m.u
    typ = types[case.get('type', 'BRIDGE1')]
    shp = m.read32(typ + 0xA4)
    assert shp, 'Physical bridge image must bind through original reader'
    u.mem_write(CELL + 0x44, dwords(m.read32(typ + 0x294)))
    u.mem_write(CELL + 0x24, struct.pack('<2h', *case['coords']))
    u.mem_write(CELL + 0x11B, bytes((case.get('level', 0) & 255,)))
    u.mem_write(CELL + 0x11E, bytes((case.get('state', 0),)))
    u.mem_write(CELL + 0x140, dwords(case.get('flags', 0x180)))
    u.mem_write(0x886FA0, dwords(0, case.get('viewport_y', 0), WIDTH, HEIGHT))
    u.mem_write(POINT, dwords(*case.get('cell_point', [66, 81])))
    u.mem_write(CLIP, dwords(*case.get('clip', [0, 0, WIDTH, HEIGHT])))
    background, old_z = case.get('background', 0xFFFF), case.get('old_z', 65535)
    if reset_surface:
        u.mem_write(PIXELS, packed_words([background] * COUNT))
        u.mem_write(Z, packed_words([old_z] * COUNT))
        u.mem_write(A, packed_words([127] * COUNT))
    draws, rows, reached, steps = [], [], set(), []
    leaves = BODY_LEAVES if body else LEAVES
    entry = 0x47F6A0 if body else 0x47F510

    def observe(_u, address, _size, _data):
        if address in (entry, 0x480110, 0x5FDCC0, 0x4AED70, 0x69E7E0,
                       0x69E740, 0x69E900, 0x490E50, 0x437A10, *leaves):
            reached.add(address)
        if address == 0x4AED70:
            sp = u.reg_read(UC_X86_REG_ESP)
            args = ints(u, sp + 4, 14)
            draws.append(dict(frame=args[1], point=ints(u, args[2], 2),
                              clip=ints(u, args[3], 4), flags=args[4],
                              z_adjust=args[6], gradient=args[7], brightness=args[8],
                              zshape_argument=args[12], raw_args=args))
        elif address in leaves:
            sp = u.reg_read(UC_X86_REG_ESP)
            args = ints(u, sp + 4, 10)
            offset = (args[0] - PIXELS) // 2
            rows.append(dict(leaf=leaves[address], x=offset % WIDTH,
                             y=offset // WIDTH, count=args[2], skip=args[3],
                             candidate=args[4], zshape_address=args[9],
                             zshape_values=sorted(set(struct.unpack(
                                 '<' + 'b' * args[2], u.mem_read(args[9], args[2]))))))

    hook = u.hook_add(UC_HOOK_CODE, observe)
    for _ in range(case.get('repeat', 1)):
        if body:
            # Supplied dirty-cache miss forces every repeat through the
            # original raster, rather than measuring its redundant-draw cache.
            u.mem_write(CELL + 0x64, dwords(0xFFFFFFFF))
        if boundary_only:
            u.mem_write(SP, dwords(RET_MAGIC, POINT, CLIP))
            u.reg_write(UC_X86_REG_ESP, SP)
            u.reg_write(UC_X86_REG_ECX, CELL)
            run_checked(u, entry, 0x4AED70)
            observe(u, 0x4AED70, 0, None)
        else:
            m.invoke(entry, CELL, (POINT, CLIP))
        if body and not boundary_only and case['repeat'] > 1:
            steps.append(snapshot(u, background, old_z))
    u.hook_del(hook)
    required = {entry, 0x480110, 0x5FDCC0, 0x4AED70}
    if not boundary_only:
        required |= {0x490E50, 0x437A10, 0x4990E0 if body else 0x497390}
    assert draws and required.issubset(reached)
    m.invoke(0x69E7E0, shp, (RECT, draws[0]['frame']))
    result = dict(name=case['name'], input=case, draws=draws, leaf_rows=rows,
                  image_name=m.string(typ + 0x1F8),
                  native_rect=ints(u, RECT, 4),
                  reached=[f'{p:08X}' for p in sorted(reached)])
    if not boundary_only:
        result['output'] = snapshot(u, background, old_z)
    result['canvas'] = list(struct.unpack('<2H', u.mem_read(shp + 2, 4)))
    if steps:
        result['steps'] = steps
    return result


def pixel_rows(m):
    """Use original constructor-owned selected leaves on explicit small spans."""
    u = m.u
    convert = m.read32(0x87F6C4)
    dest, source, depth, zshape = [MEM + n for n in (0x90000, 0x91000, 0x92000, 0x93000)]
    u.mem_write(ZOBJ + 0x1C, dwords(depth + 0x1000, 0x1000))
    controls = [
        dict(name='near_equal_far_transparent', candidate=1000,
             colors=[65535] * 4, depths=[999, 1000, 1001, 1001], indices=[1, 1, 1, 0]),
        dict(name='signed_zshape', candidate=1000, colors=[65535] * 3,
             depths=[1000] * 3, indices=[1] * 3, zshape=[1, 0, -1]),
    ]
    controls += [dict(name=f'signed_candidate_{candidate}', candidate=candidate,
                     colors=[65535, 0x39E7, 0x7E0, 0xF800],
                     depths=[0, 1000, 32768, 65535], indices=[1, 1, 1, 1])
                 for candidate in (-65537, -1, 0, 32768, 65535, 65536)]
    rows = []
    for format_, selector in (('plain', 0x490B90), ('rle', 0x490E50)):
        obj = m.invoke(selector, convert, (0x4601,))
        vtable = m.read32(obj)
        leaf = m.read32(vtable + 4)
        assert leaf == (0x493830 if format_ == 'plain' else 0x497390)
        for control in controls:
            if format_ == 'plain' and 'zshape' in control:
                continue
            n = len(control['indices'])
            encoded = (b''.join(bytes((i,)) if i else b'\0\1' for i in control['indices'])
                       if format_ == 'rle' else bytes(control['indices']))
            u.mem_write(source, encoded)
            u.mem_write(dest, packed_words(control['colors']))
            u.mem_write(depth, packed_words(control['depths']))
            u.mem_write(zshape, struct.pack('<' + 'b' * n, *control.get('zshape', [0] * n)))
            args = ((dest, source, n, 0, control['candidate'], depth, A, 1000, 0, zshape)
                    if format_ == 'rle' else
                    (dest, source, n, control['candidate'], depth, A, 1000, 0))
            steps = []
            for _ in range(2):
                m.invoke(leaf, obj, args)
                steps.append(dict(colors=list(struct.unpack('<' + 'H' * n, u.mem_read(dest, 2 * n))),
                                  depths=list(struct.unpack('<' + 'H' * n, u.mem_read(depth, 2 * n)))))
            rows.append(dict(format=format_, selector=f'{selector:08X}',
                             vtable=f'{vtable:08X}', leaf=f'{leaf:08X}', input=control, steps=steps))
    return rows


def traversal(m, *, origin=(10, 20), view_origin=(0, 0)):
    """Original Tactical loop order; map/visibility and draw calls are sinks."""
    u = m.u
    tactical = m.alloc(0xE20)
    u.mem_write(tactical + 0xB0, dwords(*view_origin))
    u.mem_write(0xB0CE30, dwords(WIDTH, HEIGHT))
    u.mem_write(0xB0CD48, struct.pack('<Q', 0x3FC25E5374344960))
    u.mem_write(SP, bytes(0x180))
    u.mem_write(SP + 0x28, struct.pack('<hh', *origin))
    u.mem_write(SP + 0x38, dwords(3))
    u.mem_write(SP + 0x64, dwords(0, 0, WIDTH, HEIGHT))
    u.mem_write(SP + 0x74, dwords(2))
    u.reg_write(UC_X86_REG_ESP, SP)
    u.reg_write(UC_X86_REG_EDI, tactical)
    u.reg_write(UC_X86_REG_EBP, 2)
    calls, coords = [], []

    def observe(_u, address, _size, _data):
        sp = u.reg_read(UC_X86_REG_ESP)
        if address == 0x568300:
            m.ret(1, 4)
        elif address == 0x5657A0:
            coords[:] = struct.unpack('<2h', u.mem_read(m.read32(sp + 4), 4))
            u.mem_write(CELL + 0x24, struct.pack('<2h', *coords))
            m.ret(CELL, 4)
        elif address in (0x47FB90, 0x47FDE0):
            output = m.read32(sp + 4)
            u.mem_write(output, dwords(0, 0, WIDTH, HEIGHT))
            m.ret(output, 4)
        elif address in (0x47F6A0, 0x47F510):
            calls.append(dict(piece='body' if address == 0x47F6A0 else 'shadow',
                              coords=list(coords), point=ints(u, m.read32(sp + 4), 2)))
            m.ret(0, 8)

    hook = u.hook_add(UC_HOOK_CODE, observe)
    run_checked(u, 0x6D6E5B, 0x6D71D3, count=200000,
                required_addresses=(0x6D7001, 0x6D7031, 0x6D719F))
    u.hook_del(hook)
    return dict(entry='006D6E5B', end='006D71D3', origin=list(origin),
                max_diagonal=2, cells_per_diagonal=3, calls=calls)


def generate():
    m, types, inputs = prepare()
    cases = [dict(name=f'state_{state}', state=state) for state in range(18)]
    cases += [dict(name=f'level_{level}', level=level) for level in (-1, 2, 6)]
    cases += [dict(name=f'flags_{flags}_state_{state}', flags=flags, state=state)
              for flags in (0, 0x80, 0x100) for state in (0, 8, 9, 17)]
    cases += [dict(name=f'depth_{depth}', old_z=depth) for depth in (32714, 32715, 32716)]
    cases += [dict(name='equal_repeat', repeat=2),
              dict(name='bridge2_state9', type='BRIDGE2', state=9),
              dict(name='viewport_y_23', viewport_y=23),
              dict(name='viewport_y_minus17', viewport_y=-17),
              dict(name='dirty_rebase', clip=[11, 13, 140, 100]),
              dict(name='top_left_clip', cell_point=[-18, 6]),
              dict(name='bottom_right_clip', cell_point=[150, 150])]
    cases += [dict(name=f'wood_state_{state}', type='BRIDGEB1', state=state,
                   cell_point=[36, 81]) for state in range(18)]
    cases += [dict(name=f'wood_type2_state_{state}', type='BRIDGEB2', state=state,
                   cell_point=[36, 81]) for state in (0, 9)]
    rows = [execute(m, types, case) for case in cases]
    shapes = []
    for row in (row for row in rows if row['name'].startswith(('state_', 'wood_state_'))):
        point = row['draws'][0]['point']
        canvas_top = [point[i] - row['canvas'][i] // 2 for i in range(2)]
        assert len(row['leaf_rows']) == row['native_rect'][3]
        assert all(leaf['skip'] == 0 and leaf['count'] == row['native_rect'][2]
                   for leaf in row['leaf_rows']), 'Mask witness must not be clipped'
        shapes.append(dict(image_name=row['image_name'], frame=row['draws'][0]['frame'], canvas=row['canvas'],
                           native_rect=row['native_rect'],
                           mask_runs=[[x - canvas_top[0], y - canvas_top[1], n]
                                      for x, y, n, _color, _depth in row['output']['runs']]))
    return dict(native_sha256=NATIVE_SHA256, inputs=inputs, shapes=shapes,
                surface=dict(width=WIDTH, height=HEIGHT, baseline=32768, bytes_per_pixel=2),
                rows=rows, traversal=traversal(m), pixel_rows=pixel_rows(m))


def metadata():
    return provenance(
        scope='43 original high concrete and 20 affected high wood bridge Cell shadow callers through physical bridge.tem/bridgb.tem SHP, clip, selector, RLE rowwalker, shadow color and depth writes; 15 selected plain/RLE leaf controls; original two-sweep Tactical traversal suffix with visibility/map and draw sinks. No complete scene, map loader, GPU or gameplay parity.',
        assumptions=[
            'Physical archives and lexical INI entries are supplied using existing archive/lexical owners. Original Overlay registry constructors, BRIDGE1/2 and BRIDGEB1/2 full rules readers, ART/image binding and Convert constructors execute. Selected RULESMD and MPBattleMD layers; no map overrides or LANGRULE are claimed.',
            'Prepared live Cell level/state/flags, projected zero-height top-left cell point, dirty rectangle, viewport Y and RGB565 surfaces are explicit inputs. Original Get_Draw_Offset, shadow frame selection, shape clipping, selector, rowwalker and pixel leaves execute unchanged.',
            'A/Z buffers use original BSurface virtual methods, baseline32768, A127. Packed output runs are lossless observations of original final colors/depth relative to the input background/oldZ.',
            'All 18 selected retail shadow frames in each family use native format3; native shape selects gradient0, optional Zshape0 and the rowwalker fallback89C568, whose observed bytes are zero. shapes.mask_runs are obtained from native admitted shadow pixels at oldZ65535, translated into physical180x180 BRIDGE or253x242 BRIDGB canvas; there is no Python SHP pixel decoder or expected-color implementation. Woodrows use a supplied36,81 point to keep their wider shadow crop uncut.',
            'Within each physical BRIDGE and BRIDGB family, shadow halves in all six theaters are byte-identical after excluding absolute frame-data offsets: exact frame crop/format metadata and compressed bytes compare equal, even where body bytes differ. This static source-equivalence witness does not execute the other five theater loaders.',
            'pixel_rows invoke constructor-owned Convert selectors490B90/490E50 with4601 and their selected original leaf methods, using explicit input source indices, RGB565 destinations, signed candidate depths and optional signedZshape. Negative candidate controls show strict signed comparison precedes the low16 store; a negative repeated candidate can remain admitted after wrapping.',
            'traversal starts at6D6E5B after the visible region bounds were calculated; supplied origin10,20/max diagonal2/three cells per diagonal, original coordinate arithmetic, projection and rectangle intersection execute. Original loops emit all body calls, then all shadow calls, descending cellX+cellY and ascending cellX within each diagonal. No native map visibility admission or full Display order outside this bounded suffix is claimed.',
        ],
        substitutions=[
            'Existing PaletteReader allocator, TLS and archive-resource boundaries supply memory and physical named asset bytes. No reached bridge caller, shape, selector, rowwalker or shadow leaf is substituted.',
            'Only traversal substitutes568300 map-bound admission(true),5657A0 cell lookup(prepared Cell with requested coordinates),47FB90/47FDE0 visibility rectangles(full target), and47F6A0/47F510 draw sinks recording arguments. These substitutions never apply to raster rows or pixel_rows.',
        ],
        entry_points={'cell_shadow':0x47F510, 'cell_draw_offset':0x480110,
                      'overlay_draw_offset':0x5FDCC0, 'shape':0x4AED70,
                      'overlay_type_ctor':0x5FE250, 'overlay_type_read':0x5FE770,
                      'convert_ctor':0x48E740, 'blitter_init':0x48EBF0,
                      'tactical_sweeps_begin':0x6D6E5B, 'tactical_sweeps_end':0x6D71D3,
                      'rle_selector':0x490E50, 'rle_rowwalker':0x437A10,
                      'plain_shadow_leaf':0x493830, 'rle_shadow_leaf':0x497390})


def body_overlap_scene(m, types):
    """Native constructor publications, Tactical order and retained raster.

    Reuses the existing constructor fixture's sparse map and resident Road TMP
    boundary. Only produced overlay/state/flags and supplied level cross VMs;
    the rendering VM retains the physical type/image/Cell Convert owners.
    """
    producer = OriginalBridgeConstructor('success', 25)
    for pointer in producer.ptrs.values():
        producer.uc.mem_write(pointer + 0x11B, bytes((1,)))
    producer.construct()
    producer.obj = CONSTRUCTOR_MEM + 0x8000
    producer.uc.mem_write(producer.coord, struct.pack('<2h', 16, 17))
    producer.construct()
    facts = {}
    for coords in ((16, 16), (16, 17)):
        pointer = producer.ptrs[coords]
        overlay = ints(producer.uc, pointer + 0x44, 1)[0]
        assert overlay == m.read32(types['BRIDGE2'] + 0x294)
        facts[coords] = dict(coords=list(coords), overlay_id=overlay, type='BRIDGE2',
                             flags=producer.read_u32(pointer + 0x140),
                             state=producer.uc.mem_read(pointer + 0x11E, 1)[0],
                             level=producer.uc.mem_read(pointer + 0x11B, 1)[0])
    view_origin = [-90, 375]
    native_traversal = traversal(m, origin=(16, 16), view_origin=view_origin)
    cells = [facts[tuple(call['coords'])] | dict(cell_point=call['point'])
             for call in native_traversal['calls']
             if call['piece'] == 'body' and tuple(call['coords']) in facts]
    assert len(cells) == 2 and len({tuple(cell['coords']) for cell in cells}) == 2
    shared = dict(background=65535, old_z=65535, clip=[0, 0, WIDTH, HEIGHT],
                  viewport_y=0, repeat=1)
    for index, cell in enumerate(cells):
        composed = execute(m, types, shared | {key: value for key, value in cell.items()
                                               if key != 'overlay_id'}
                           | dict(name=f'overlap_{index}'), body=True,
                           reset_surface=index == 0)
    # An observed source/old-Z tie certificate keeps this scene relevant to
    # strict body depth and order. These are original isolated/final outputs,
    # not a second SHP decoder or an expected-composition implementation.
    isolated = [execute(m, types, shared | {key: value for key, value in cell.items()
                                           if key != 'overlay_id'}
                        | dict(name=f'isolated_{index}'), body=True)
                for index, cell in enumerate(cells)]
    maps = [{(x + offset, y): (color, depth)
             for x, y, count, color, depth in row['output']['runs']
             for offset in range(count)} for row in isolated + [composed]]
    overlap = maps[0].keys() & maps[1].keys()
    ties = [pixel for pixel in sorted(overlap)
            if maps[0][pixel][1] == maps[1][pixel][1]
            and maps[0][pixel][0] != maps[1][pixel][0]]
    assert ties and all(maps[2][pixel] == maps[0][pixel] for pixel in ties)
    return dict(name='adjacent_concrete_anchors', input=shared, cells=cells,
                native_order=[cell['coords'] for cell in cells],
                native_traversal=native_traversal,
                producer=dict(entry='005FC380', owner='bridge_constructor.OriginalBridgeConstructor',
                              calls=[[16, 16], [16, 17]], supplied_level=1,
                              view_origin=view_origin,
                              facts=list(facts.values()),
                              boundary='Existing sparse real9x9/type-index25/synthetic Road TMP success fixture; original constructors/Mark/setters execute. No native retail map load or xbayopigs input claim.'),
                tie_witness=dict(opaque_overlap=len(overlap), different_color_equal_z=len(ties),
                                 samples=[dict(pixel=list(pixel), first=list(maps[0][pixel]),
                                               second=list(maps[1][pixel]),
                                               native_composed=list(maps[2][pixel]))
                                          for pixel in ties[:8]]),
                output=composed['output'])


def generate_body():
    m, types, inputs = prepare(body=True)
    controls = [execute(m, types, dict(name=f'raw_state_{state}', coords=[1, 2],
                                     state=state), body=True, boundary_only=True)
                for state in range(18)]
    controls += [execute(m, types, dict(name=f'level_{level}', level=level, state=1),
                         body=True, boundary_only=True) for level in (-1, 2, 6)]
    controls += [execute(m, types, dict(name=f'flags_{flags}', flags=flags, state=0,
                                      coords=[1, 2]), body=True, boundary_only=True)
                 for flags in (0, 0x80, 0x100, 0x400)]
    controls += [execute(m, types, dict(name=f'latin_{x}_{y}', state=0, coords=[x, y]),
                         body=True, boundary_only=True)
                 for y in range(4) for x in range(4)]
    cases = [
        dict(name='concrete_ns_healthy', coords=[1, 2], cell_point=[66, 111]),
        dict(name='concrete_ew_damaged', type='BRIDGE2', state=15, cell_point=[66, 111]),
        dict(name='wood_ns_partial', type='BRIDGEB1', state=7, cell_point=[66, 141]),
        dict(name='wood_ew_healthy', type='BRIDGEB2', state=9, cell_point=[66, 141]),
    ]
    rows = [execute(m, types, case, body=True) for case in cases]
    assert all(len(row['leaf_rows']) == row['native_rect'][3] for row in rows), 'Full crop controls'
    # Original raster itself supplies the candidate for three one-row Z
    # controls. No Rust depth formula or hand-computed expected pixel is used.
    probe_case = dict(name='depth_probe', clip=[0, 0, WIDTH, 1])
    probe = execute(m, types, probe_case, body=True)
    x, y, _count, _color, candidate = next(
        run for run in probe['output']['runs'] if run[3] not in (0, 65535))
    assert 0 < candidate < 65535 and y == 0
    for name, old_z in (('near', candidate + 1), ('equal', candidate), ('far', candidate - 1)):
        row = execute(m, types, dict(name=f'depth_{name}', old_z=old_z,
                                    clip=[0, 0, WIDTH, 1]), body=True)
        row['depth_probe'] = dict(pixel=[x, y], candidate=candidate,
                                  source='original depth_probe raster output')
        rows.append(row)
    assert rows[-3]['output']['runs'] and not rows[-2]['output']['runs'] and not rows[-1]['output']['runs']
    repeat = execute(m, types, dict(name='equal_repeat', repeat=2,
                                   clip=[0, 0, WIDTH, 24]), body=True)
    assert len(repeat['steps']) == 2 and repeat['steps'][0] == repeat['steps'][1]
    rows.append(repeat)
    return dict(native_sha256=NATIVE_SHA256, inputs=inputs,
                surface=dict(width=WIDTH, height=HEIGHT, baseline=32768, bytes_per_pixel=2),
                rows=rows, controls=controls, scenes=[body_overlap_scene(m, types)])


def body_metadata():
    return provenance(
        scope='Eight original high-bridge body caller/raster controls and one adjacent-anchor composition on physical BRIDGE/BRIDGB art, native default Cell LightConvert and strict RGB565/Z leaves; 41 caller-boundary controls including all16 LatinSquare positions. No whole scenario, GPU, snapshot drawing or complete47F6A0 parity.',
        assumptions=[
            'Shares physical_assets, prepare, execute and snapshot with the retained shadow oracle. Original four OverlayType constructors/full rules readers and ART/image binding execute on physical RULESMD/MPBattleMD and ARTMD lexical inputs. No map override, LANGRULE or complete map-loader execution is claimed.',
            'Physical ISOTEM.PAL and TEMPERAT.PAL are supplied at original theater source/destination storage. Original5454DB..5454EB expansion loop executes for each palette, then545000..5450EF constructs/publishes the default ISO-source LightConvert through555DA0/556090 and native blitter initialization. Original483E30 binds it to the Cell and writes explicitly supplied unity lighting/brightness1000. Full scenario lighting, type/tile loading and asynchronous lighting history are excluded.',
            'RGB565 format2 and MMX dispatch bytes are supplied as established by the existing palette owner capture. Actual full-constructor table is compared byte-for-byte to tools.palette_oracle.oracle.palette_table original execution with the same physical expanded source and unity triple. No generated table is injected into the body VM.',
            'Cell current overlay/type, raw state, signed byte level, flags, coordinates, z0-projected point, clip and viewport are explicit component inputs. All raster rows use ordinary0x180 high-anchor flags and valid stock state/orientation/material combinations; no natural producer is claimed for extra boundary flags/signed-level controls.',
            'Original47F6A0,480110,5FDCC0,4AED70, shape metadata/pixels, selector490E50, rowwalker437A10 and opaque4990E0 execute unchanged. controls stop BEFORE4AED70 and therefore make no raster claim. Body dirty cache+64 is explicitly invalidated before each invocation, including repeats, so both repeats reach the original raster.',
            'A127, backgroundFFFF and oldZ are supplied RGB565 surface state. Final color/Z runs are lossless original output. Three one-row near/equal/far oldZ controls derive their candidate from an original probe raster pixel, then rerun the caller; equality/far emit no stores. Full shape crop is retained in the four material/orientation controls. Repeat output remains unchanged after an actual second body raster.',
            'Physical body artwork is temperate; the shared loader also records other theater asset identities but body equivalence across theaters is not claimed. Ordinary0x80-clear fallback is caller-boundary research only; its renderer and FoggedObjectClass4D1AB5 consumer remain separate open branches.',
            'The adjacent-anchor scene reuses OriginalBridgeConstructor success fixture: two original5FC380/Mark/setter calls at16,16 and16,17 on its supplied sparse9x9 map, rawtype25 and synthetic resident Road TMP, with supplied level1. Produced overlay/state/flags cross into the existing physical-body raster VM. Its existing original Tactical suffix6D6E5B..6D71D3 supplies body order and call points with origin16,16/vieworigin-90,375; map lookup/admission/visibility/draws are sinks only for this traversal. Raster executes separately in that observed order on one retained framebuffer/Z. It is not a native map loader or xbayopigs whole-scene claim.',
        ],
        substitutions=[
            'Existing PaletteReader allocator, TLS, archive-resource and RGB565 primary surface transport are retained. Spare native registries and explicit Cell/A/Z/surface state are supplied. No body caller, palette expansion/color generation, shape, selector, rowwalker or opaque pixel leaf is substituted.',
        ],
        entry_points=dict(cell_body=0x47F6A0, cell_draw_offset=0x480110,
                          overlay_draw_offset=0x5FDCC0, shape=0x4AED70,
                          theater_palette_expand=0x5454DB, default_cell_convert=0x545000,
                          default_cell_convert_stop=0x5450EF, cell_bind=0x483E30,
                          light_convert_ctor=0x555DA0, light_convert_colors=0x556090,
                          mmx_rgb565=0x7DE200, rle_selector=0x490E50,
                          rle_rowwalker=0x437A10, opaque_body_leaf=0x4990E0,
                          overlay_constructor=0x5FC380, overlay_mark=0x5FC570,
                          tactical_sweeps_begin=0x6D6E5B, tactical_sweeps_end=0x6D71D3))


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument('--body', action='store_true')
    selected, remaining = parser.parse_known_args()
    finish_vectors(generate_body if selected.body else generate,
                   Path(__file__).with_name('bridge_body_render.json') if selected.body
                   else Path(__file__).with_suffix('.json'),
                   provenance=body_metadata if selected.body else metadata, argv=remaining)
