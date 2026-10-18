/**
 * VERA20k telemetry dashboard — browser client.
 *
 * Connects to the engine's telemetry server (same origin, /ws), receives
 * `TelemetrySnapshot` JSON at ~10 Hz and renders:
 *  - a 3D battlefield overview (three.js InstancedMesh),
 *  - a streaming tick-cost chart (canvas),
 *  - the former egui debug panels (hotkeys, terrain, event history),
 *  - per-house population and roster panels.
 *
 * Schema mirror of `src/sim/telemetry.rs`; keep both in step.
 */
import { applyStaticTranslations, onLocaleChange, setLocale, syncSwitcher, t, } from "./i18n.js";
import { createMap3D } from "./map3d.js";
import { SCHEMA_VERSION, } from "./types.js";
/** Chart window: the ring keeps the last N samples. */
const CHART_WINDOW = 240;
let map3d = null;
/** Latest snapshot, kept so a language switch can re-render every panel. */
let lastSnapshot = null;
/** The live socket; switches send their commands back through it. */
let socket = null;
/** One toggle chip: the engine switch it drives, its label, its state flag. */
const TOGGLE_SWITCHES = [
    { name: "terrain_costs", key: "sw.terrainCosts", flag: "terrain_costs" },
    { name: "heightmap", key: "sw.heightmap", flag: "heightmap" },
    { name: "cell_grid", key: "sw.cellGrid", flag: "cell_grid" },
    { name: "unit_inspector", key: "sw.inspector", flag: "unit_inspector" },
    { name: "debug_pause", key: "sw.pause", flag: "paused" },
];
let switchesBuilt = false;
/** Ask the engine to act on one switch (fire-and-forget JSON frame). */
function sendCommand(command) {
    if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify(command));
    }
}
function buildSwitches() {
    const container = document.getElementById("switches");
    if (!container)
        return;
    container.textContent = "";
    const toggles = document.createElement("div");
    toggles.className = "switches";
    for (const spec of TOGGLE_SWITCHES) {
        const button = document.createElement("button");
        button.className = "switch";
        button.dataset.switch = spec.name;
        button.addEventListener("click", () => sendCommand({ cmd: "toggle", switch: spec.name }));
        toggles.append(button);
    }
    const steps = document.createElement("div");
    steps.className = "switches";
    const previous = document.createElement("button");
    previous.className = "switch step";
    previous.id = "speed-prev";
    previous.textContent = "\u2039";
    previous.addEventListener("click", () => sendCommand({ cmd: "cycle_speed_type", forward: false }));
    const speedValue = document.createElement("span");
    speedValue.className = "switch-value";
    speedValue.id = "speed-value";
    const next = document.createElement("button");
    next.className = "switch step";
    next.id = "speed-next";
    next.textContent = "\u203a";
    next.addEventListener("click", () => sendCommand({ cmd: "cycle_speed_type", forward: true }));
    const frameStep = document.createElement("button");
    frameStep.className = "switch";
    frameStep.id = "frame-step";
    frameStep.addEventListener("click", () => sendCommand({ cmd: "frame_step" }));
    const hint = document.createElement("span");
    hint.className = "switch-value";
    hint.id = "switch-state";
    steps.append(previous, speedValue, next, frameStep, hint);
    container.append(toggles, steps);
    switchesBuilt = true;
}
/** Reflect the engine's real switch state (hotkeys move it too). */
function renderSwitches(debug) {
    if (!switchesBuilt)
        buildSwitches();
    const container = document.getElementById("switches");
    if (!container)
        return;
    const overlays = debug?.overlays ?? null;
    for (const spec of TOGGLE_SWITCHES) {
        const button = container.querySelector(`[data-switch="${spec.name}"]`);
        if (!button)
            continue;
        button.textContent = t(spec.key);
        button.classList.toggle("on", overlays ? overlays[spec.flag] : false);
        button.disabled = overlays === null;
    }
    const previous = document.getElementById("speed-prev");
    const next = document.getElementById("speed-next");
    const speedValue = document.getElementById("speed-value");
    const frameStep = document.getElementById("frame-step");
    const hint = document.getElementById("switch-state");
    // The SpeedType notch only drives the terrain-cost wash while it is on —
    // the same guard the `[` `]` hotkeys use.
    const speedEnabled = overlays?.terrain_costs ?? false;
    if (previous)
        previous.disabled = !speedEnabled;
    if (next)
        next.disabled = !speedEnabled;
    if (speedValue)
        speedValue.textContent = debug ? debug.active_speed_type : "-";
    if (frameStep) {
        frameStep.textContent = t("sw.frameStep");
        frameStep.disabled = !(overlays?.paused ?? false);
    }
    if (hint)
        hint.textContent = overlays ? "" : t("sw.noMatch");
}
// ---------------------------------------------------------------- map
/** Latched once WebGL proves unavailable, so a retry on every snapshot does not
 *  rebuild a renderer that cannot work or append the notice repeatedly. */
let map3dUnavailable = false;
function ensureMap3D() {
    if (map3d || map3dUnavailable)
        return map3d;
    const canvas = document.getElementById("map3d");
    if (!canvas)
        return null;
    const callouts = document.getElementById("map-callouts");
    map3d = createMap3D(canvas, callouts);
    if (!map3d) {
        map3dUnavailable = true;
        const note = document.createElement("div");
        note.className = "map-fallback";
        note.textContent = t("map.noWebGL");
        canvas.parentElement?.append(note);
    }
    return map3d;
}
function drawMapLegend(snapshot) {
    const legend = document.getElementById("map-legend");
    if (!legend)
        return;
    const palette = ["#8a94a3", "#b04aff", "#e8c34a", "#37e0b8", "#ff5555", "#4a9eff"];
    legend.textContent = "";
    snapshot.houses.forEach((house, index) => {
        const swatch = document.createElement("span");
        const dot = document.createElement("i");
        const color = house.color;
        dot.style.background = color
            ? `rgb(${color[0]},${color[1]},${color[2]})`
            : palette[index % palette.length];
        swatch.append(dot, document.createTextNode(house.name));
        legend.append(swatch);
    });
}
// ---------------------------------------------------------------- chart
const chartSamples = [];
function drawChart(timing) {
    const canvas = document.getElementById("chart");
    const dpr = Math.min(window.devicePixelRatio, 2);
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
        canvas.width = width * dpr;
        canvas.height = height * dpr;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx)
        return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const budget = timing?.budget_ms ?? 16.67;
    const peak = Math.max(budget * 1.4, ...chartSamples.map((s) => s.p99), 1);
    const y = (ms) => height - 4 - (ms / peak) * (height - 10);
    // Palette matches the tech-wall chrome: cyan traces, amber budget.
    ctx.strokeStyle = "rgba(0, 190, 255, 0.10)";
    ctx.lineWidth = 1;
    for (let fraction = 0; fraction <= 1; fraction += 0.25) {
        const gy = 4 + fraction * (height - 10);
        ctx.beginPath();
        ctx.moveTo(0, gy);
        ctx.lineTo(width, gy);
        ctx.stroke();
    }
    ctx.save();
    ctx.strokeStyle = "#ffb648";
    ctx.shadowColor = "rgba(255, 182, 72, 0.8)";
    ctx.shadowBlur = 6;
    ctx.setLineDash([4, 4]);
    ctx.beginPath();
    ctx.moveTo(0, y(budget));
    ctx.lineTo(width, y(budget));
    ctx.stroke();
    ctx.restore();
    ctx.setLineDash([]);
    ctx.fillStyle = "#ffb648";
    ctx.font = "10px monospace";
    ctx.fillText(t("chart.budget", { ms: budget.toFixed(1) }), 6, y(budget) - 3);
    if (chartSamples.length > 1) {
        const step = width / (CHART_WINDOW - 1);
        const base = width - (chartSamples.length - 1) * step;
        ctx.save();
        ctx.strokeStyle = "#28f0ff";
        ctx.lineWidth = 1.6;
        ctx.shadowColor = "rgba(40, 240, 255, 0.75)";
        ctx.shadowBlur = 8;
        ctx.beginPath();
        chartSamples.forEach((sample, index) => {
            const px = base + index * step;
            const py = y(sample.tick);
            if (index === 0)
                ctx.moveTo(px, py);
            else
                ctx.lineTo(px, py);
        });
        ctx.stroke();
        ctx.restore();
        ctx.strokeStyle = "rgba(255, 90, 110, 0.55)";
        ctx.lineWidth = 1;
        ctx.beginPath();
        chartSamples.forEach((sample, index) => {
            const px = base + index * step;
            const py = y(sample.p99);
            if (index === 0)
                ctx.moveTo(px, py);
            else
                ctx.lineTo(px, py);
        });
        ctx.stroke();
    }
}
// ---------------------------------------------------------------- panels
function fmt(value, digits = 2) {
    return value.toFixed(digits);
}
/** Group thousands so a long match's tick count stays readable. Both callers
 *  pass counts (ticks, seconds), so rounding first is honest. */
function thousands(value) {
    return Math.round(value).toLocaleString("en-US");
}
/** House names come from the rules INI, so they are text, not markup — escape
 *  before any of them reaches an `innerHTML` template. */
function escapeHtml(value) {
    return value
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
function budgetClass(utilization) {
    if (utilization >= 1)
        return "bad";
    if (utilization >= 0.7)
        return "warn";
    return "good";
}
/** One `label: value` row of a key/value panel. */
function kv(label, value, cls) {
    const row = document.createElement("div");
    row.className = "kv";
    const key = document.createElement("span");
    key.className = "k";
    key.textContent = label;
    const val = document.createElement("span");
    val.className = cls ? `v ${cls}` : "v";
    val.textContent = value;
    row.append(key, val);
    return row;
}
function updateHeader(snapshot) {
    document.getElementById("version").textContent = String(snapshot.version);
    document.getElementById("tick").textContent = String(snapshot.tick);
    document.getElementById("entities").textContent = String(snapshot.entities_total);
    document.getElementById("mapsize").textContent =
        `${snapshot.map_width}x${snapshot.map_height}`;
}
function updateCost(snapshot) {
    const timing = snapshot.timing;
    document.getElementById("last").textContent = timing ? fmt(timing.tick_ms) : "-";
    document.getElementById("p50").textContent = timing ? fmt(timing.p50_ms) : "-";
    document.getElementById("p99").textContent = timing ? fmt(timing.p99_ms) : "-";
    const budgetCell = document.getElementById("budget");
    if (timing) {
        budgetCell.textContent = `${Math.round(timing.budget_utilization * 100)}`;
        budgetCell.className = `value ${budgetClass(timing.budget_utilization)}`;
    }
    else {
        budgetCell.textContent = "-";
    }
    // Two different clocks: presentation frames vs committed sim ticks. While
    // paused the tick rate is a true 0, not missing data — the hyphen is
    // reserved for "no sample at all".
    document.getElementById("fps").textContent = timing
        ? `${Math.round(timing.fps)}`
        : "-";
    document.getElementById("tickhz").textContent = timing
        ? fmt(timing.tick_hz, 1)
        : "-";
}
/** The egui Terrain Debug panel's content, from the snapshot's debug block. */
function updateTerrain(debug) {
    const panel = document.getElementById("terrain");
    panel.textContent = "";
    if (!debug) {
        // The synthetic demo streams population but no cursor/selection capture;
        // the real game fills the whole panel.
        panel.append(kv(t("terr.status"), t("terr.noData"), "dim"));
        return;
    }
    panel.append(kv(t("terr.grid"), debug.grid ? `${debug.grid[0]}x${debug.grid[1]}` : t("terr.none")), kv(t("terr.overlay"), `${debug.active_speed_type}  ${t("terr.cycleHint")}`), kv(t("terr.vxl"), debug.vxl_sprites === null ? "-" : `${debug.vxl_sprites}`));
    const cursor = debug.cursor_cell;
    panel.append(kv(t("terr.cursor"), cursor ? `(${cursor[0]}, ${cursor[1]})  z=${debug.cursor_elevation}` : t("terr.offMap")), kv(t("terr.pathgrid"), debug.cursor_walkable === null
        ? "-"
        : debug.cursor_walkable
            ? t("terr.walkable")
            : t("terr.blocked"), debug.cursor_walkable === false ? "bad" : debug.cursor_walkable ? "good" : undefined));
    if (debug.land_rows.length > 0) {
        const rows = debug.land_rows
            .map((row) => `${row.speed_type} ${row.cost === 0 ? t("terr.blocked") : `row=${row.cost}%`}`)
            .join("   ");
        panel.append(kv(t("terr.landRows"), rows));
    }
    if (debug.footprint) {
        panel.append(kv(t("terr.footprint"), debug.footprint, "warn"));
    }
    const at = document.createElement("div");
    at.className = "sub";
    at.textContent = t("terr.atCursor", { n: debug.entities_at_cursor.length });
    panel.append(at);
    if (debug.entities_at_cursor.length === 0) {
        panel.append(kv("", t("terr.none"), "dim"));
    }
    for (const entity of debug.entities_at_cursor) {
        panel.append(kv("", describeEntity(entity), "mono"));
    }
    const sel = document.createElement("div");
    sel.className = "sub";
    sel.textContent = t("terr.selected", { n: debug.selected.length });
    panel.append(sel);
    if (debug.selected.length === 0) {
        panel.append(kv("", t("terr.none"), "dim"));
    }
    for (const selected of debug.selected) {
        panel.append(...selectedRows(selected));
    }
}
function describeEntity(entity) {
    return `${entity.type_name}(${entity.category}) [${entity.owner}] hp=${entity.hp}/${entity.max_hp}`;
}
/** One selected entity's rows: identity, path and miner state. */
function selectedRows(selected) {
    const rows = [
        kv("", `${selected.type_name} @ (${selected.cell[0]},${selected.cell[1]}) sub=(${selected.sub[0]},${selected.sub[1]})`, "mono"),
    ];
    if (selected.moving) {
        rows.push(kv(t("sel.path"), t("sel.stepsLeft", { n: selected.path_steps_left, b: String(selected.path_blocked) }), selected.path_blocked ? "warn" : undefined));
        if (selected.goal) {
            rows.push(kv(t("sel.goal"), `(${selected.goal[0]},${selected.goal[1]})`, "mono"));
        }
        for (const step of selected.path_preview) {
            rows.push(kv("", `${step.first ? ">" : " "}[${step.cell[0]},${step.cell[1]}]${step.walkable ? "" : ` ${t("terr.blocked")}`}`, step.walkable ? "mono" : "mono bad"));
        }
    }
    else {
        rows.push(kv(t("sel.path"), t("sel.idle"), "dim"));
    }
    if (selected.miner) {
        const miner = selected.miner;
        rows.push(kv(t("sel.miner"), `${miner.kind} — ${miner.state}`), kv(t("sel.cargo"), `${miner.cargo}/${miner.capacity}`), kv(t("sel.stage"), `${miner.stage} (rate ${miner.rate}) ${t("sel.harvesting")}: ${miner.harvesting}`));
        if (miner.archived_ore) {
            rows.push(kv(t("sel.ore"), `(${miner.archived_ore[0]},${miner.archived_ore[1]})`, "mono"));
        }
        rows.push(kv(t("sel.movement"), `${selected.moving ? t("sel.active") : t("sel.idleWord")} · ${t("sel.teleport")} ${miner.teleport_active ? t("sel.active") : t("sel.idleWord")}`));
    }
    return rows;
}
/** A house's side, from the loaded rules' own `[Sides]` key.
 *
 * The numeric index is only a position in that section (retail order is not
 * the test-fixture order), so the engine spelling is authoritative; known
 * names are localised, unknown ones are shown as-is. */
function sideName(house) {
    const known = {
        allied: "side.allied",
        allies: "side.allied",
        gdi: "side.allied",
        soviet: "side.soviet",
        soviets: "side.soviet",
        nod: "side.soviet",
        thirdside: "side.yuri",
        yuri: "side.yuri",
        yuricountry: "side.yuri",
        civilian: "side.civilian",
        civilians: "side.civilian",
        mutant: "side.mutant",
    };
    const raw = house.side_name ?? "";
    const key = known[raw.toLowerCase()];
    if (key)
        return t(key);
    return raw || t("side.unknown", { n: house.side_index });
}
/** The egui Event History panel: the selected entities' event logs. */
function updateEvents(debug) {
    const panel = document.getElementById("events");
    panel.textContent = "";
    if (!debug || debug.selected.length === 0) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent = t("ev.pick");
        panel.append(empty);
        return;
    }
    for (const selected of debug.selected) {
        const header = document.createElement("div");
        header.className = "sub";
        header.textContent = `${selected.type_name} (id=${selected.stable_id})`;
        panel.append(header);
        if (!selected.inspector_active) {
            panel.append(kv("", t("ev.noInspector"), "dim"));
            continue;
        }
        if (selected.events.length === 0) {
            panel.append(kv("", t("ev.noneRecorded"), "dim"));
            continue;
        }
        const list = document.createElement("div");
        list.className = "event-list";
        for (const event of selected.events) {
            const line = document.createElement("div");
            line.className = "event";
            line.textContent = event.text;
            list.append(line);
        }
        panel.append(list);
    }
}
function updatePopulation(snapshot) {
    const population = document.getElementById("population");
    population.textContent = "";
    for (const house of snapshot.houses) {
        const counts = house.counts;
        const total = counts.unit + counts.infantry + counts.aircraft + counts.structure;
        const row = document.createElement("div");
        row.className = "pop-row";
        const segments = [
            [counts.infantry, "#3f8f6f"],
            [counts.unit, "#2f6fb0"],
            [counts.aircraft, "#8a5fc0"],
            [counts.structure, "#b0763a"],
        ];
        const bar = document.createElement("div");
        bar.className = "bar";
        for (const [value, css] of segments) {
            if (value <= 0)
                continue;
            const segment = document.createElement("div");
            segment.className = "seg";
            segment.style.width = `${(value / Math.max(total, 1)) * 100}%`;
            segment.style.background = css;
            bar.append(segment);
        }
        // The dot repeats the battlefield's team colour, so a row and its units
        // read as the same house at a glance. Segments stay category-coloured:
        // they encode composition, which is this panel's whole point.
        const dot = document.createElement("i");
        dot.className = "house-dot";
        const css = house.color
            ? `rgb(${house.color[0]},${house.color[1]},${house.color[2]})`
            : "#6b7a8c";
        dot.style.background = css;
        dot.style.boxShadow = `0 0 8px ${css}`;
        const name = document.createElement("span");
        name.className = "name";
        name.textContent = house.name;
        name.title = house.name;
        const totalCell = document.createElement("span");
        totalCell.className = "total";
        totalCell.textContent = String(total);
        row.append(dot, name, bar, totalCell);
        population.append(row);
    }
}
const FRAME_COLORS = {
    objects: "#2f6fb0",
    tail: "#d94f5c",
    vision: "#4fb8a8",
    logic: "#3f9f5f",
    render: "#e08b2a",
    unmarked: "#9a9a9a",
};
/** Render one pie: inline SVG wedges with in-slice percentages, plus a
 *  legend carrying the millisecond values. */
function drawPie(canvasId, legendId, slices, total) {
    const canvas = document.getElementById(canvasId);
    const legend = document.getElementById(legendId);
    if (!canvas || !legend)
        return;
    canvas.textContent = "";
    legend.textContent = "";
    const size = 132;
    const center = size / 2;
    const radius = size / 2 - 2;
    /** A ring rather than a full disc: the hollow centre carries the headline
     *  number, and a band shows a 6% wedge beside an 80% one far better than two
     *  wedges sharing a point at the middle. */
    const inner = radius * 0.58;
    /** Angular gap between wedges, applied only where the wedge is wide enough to
     *  spare it, so a sliver is never eaten by its own separator. */
    const gap = 1.8;
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", `0 0 ${size} ${size}`);
    /** `scale` shrinks the radius: 1 is the outer arc, `inner / radius` the hole. */
    const polar = (degrees, scale = 1) => {
        const radians = (degrees * Math.PI) / 180;
        const reach = radius * scale;
        return [center + reach * Math.cos(radians), center + reach * Math.sin(radians)];
    };
    let angle = -90;
    let drew = false;
    let dominant = 0;
    for (const slice of slices) {
        if (slice.ms <= 0 || total <= 0)
            continue;
        drew = true;
        const fraction = Math.min(1, slice.ms / total);
        dominant = Math.max(dominant, fraction);
        const sweep = fraction * 360;
        const pad = sweep > gap * 3 ? gap / 2 : 0;
        const a0 = angle + pad;
        const a1 = angle + sweep - pad;
        const arc = a1 - a0 > 180 ? 1 : 0;
        const [ox0, oy0] = polar(a0);
        const [ox1, oy1] = polar(a1);
        const [ix1, iy1] = polar(a1, inner / radius);
        const [ix0, iy0] = polar(a0, inner / radius);
        const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        path.setAttribute("d", `M ${ox0} ${oy0} A ${radius} ${radius} 0 ${arc} 1 ${ox1} ${oy1}` +
            ` L ${ix1} ${iy1} A ${inner} ${inner} 0 ${arc} 0 ${ix0} ${iy0} Z`);
        path.setAttribute("fill", slice.color);
        path.setAttribute("stroke", "rgba(4, 8, 20, 0.5)");
        path.setAttribute("stroke-width", "0.6");
        const title = document.createElementNS("http://www.w3.org/2000/svg", "title");
        title.textContent = `${t(slice.key)} ${fmt(slice.ms, 2)} ms`;
        path.append(title);
        svg.append(path);
        // Only label wedges wide enough to hold the text: a sliver's label would
        // spill across its neighbours, and the legend already carries the number.
        if (fraction >= 0.05) {
            // The band's own midline, so the text sits inside the wedge it names.
            const [lx, ly] = polar((a0 + a1) / 2, (inner / radius + 1) / 2);
            const label = document.createElementNS("http://www.w3.org/2000/svg", "text");
            label.setAttribute("x", String(lx.toFixed(2)));
            label.setAttribute("y", String(ly.toFixed(2)));
            label.setAttribute("text-anchor", "middle");
            label.setAttribute("dominant-baseline", "central");
            label.setAttribute("font-size", "10.5");
            label.setAttribute("font-weight", "600");
            label.setAttribute("fill", "#ffffff");
            // A dark halo rather than a colour gamble: the wedge palette comes from
            // the engine, so both a pale and a saturated fill must stay readable.
            label.setAttribute("stroke", "rgba(4, 8, 20, 0.6)");
            label.setAttribute("stroke-width", "2.6");
            label.setAttribute("paint-order", "stroke");
            label.setAttribute("pointer-events", "none");
            label.textContent = `${fmt(fraction * 100, 0)}%`;
            svg.append(label);
        }
        angle += sweep;
    }
    if (drew) {
        // The hole answers the question this panel exists for — what dominates the
        // frame. Milliseconds and the per-phase split are already in the caption and
        // the legend, so only the share goes here.
        const head = document.createElementNS("http://www.w3.org/2000/svg", "text");
        head.setAttribute("x", String(center));
        head.setAttribute("y", String(center));
        head.setAttribute("text-anchor", "middle");
        head.setAttribute("dominant-baseline", "central");
        head.setAttribute("class", "pie-dominant");
        head.textContent = `${fmt(dominant * 100, 0)}%`;
        svg.append(head);
    }
    // An empty pie would still be a 200px box on the report wall; leave the
    // container empty instead so the panel collapses to its text.
    if (drew)
        canvas.append(svg);
    for (const slice of slices) {
        const row = document.createElement("li");
        const swatch = document.createElement("span");
        swatch.className = "sw";
        swatch.style.background = slice.color;
        swatch.style.color = slice.color;
        const label = document.createElement("span");
        label.className = "lb";
        label.textContent = t(slice.key);
        const value = document.createElement("span");
        value.className = "ms";
        const percent = total > 0 ? (slice.ms / total) * 100 : 0;
        value.textContent = `${fmt(slice.ms, 2)} ms · ${fmt(percent, 1)}%`;
        row.append(swatch, label, value);
        legend.append(row);
    }
}
/**
 * Slice set for one frame split: the simulation phases on their own, and the
 * whole frame with the presentation cost (whatever the wall spent outside the
 * simulation) in front of them. Both the main wall and the report wall draw
 * from this, so the two cannot drift apart.
 */
function buildFrameSlices(profile, frameMs) {
    const renderMs = Math.max(0, frameMs - profile.mean_frame_ms);
    const sim = [
        { key: "rep.objects", ms: profile.objects_ms, color: FRAME_COLORS.objects },
        { key: "rep.tail", ms: profile.tail_ms, color: FRAME_COLORS.tail },
        { key: "rep.vision", ms: profile.vision_ms, color: FRAME_COLORS.vision },
        { key: "rep.logic", ms: profile.logic_ms, color: FRAME_COLORS.logic },
    ];
    if (profile.unmarked_ms > 0.01) {
        sim.push({
            key: "rep.unmarked",
            ms: profile.unmarked_ms,
            color: FRAME_COLORS.unmarked,
        });
    }
    const frame = [
        { key: "rep.render", ms: renderMs, color: FRAME_COLORS.render },
        ...sim,
    ];
    return {
        sim,
        frame,
        simTotal: sim.reduce((sum, slice) => sum + slice.ms, 0),
        frameTotal: frame.reduce((sum, slice) => sum + slice.ms, 0),
    };
}
/** Per-subsystem ledger keys, in the engine's `PHASE_KEYS` order (schema v13).
 *  The report table lists them in full; the pies only sum four slices of them. */
const PHASE_KEYS = [
    "commands_ingress",
    "commands_admit",
    "triggers",
    "shroud",
    "lighting",
    "ore_growth",
    "bombs",
    "superweapons",
    "radiation",
    "team_ai",
    "objects",
    "wall_crush",
    "wakes",
    "vision",
    "combat",
    "passengers",
    "ion_blasts",
    "crates",
    "production",
    "docks",
    "spawn_fog",
    "late_ai",
    "overlays",
    "debug_asserts",
    "anims",
    "score",
    "hash",
];
/** A phase's milliseconds: three decimals while it is under a tenth, so the
 *  small ones do not all collapse into `0.00`. */
function fmtPhase(ms) {
    if (ms <= 0)
        return "—";
    if (ms < 0.01)
        return "<0.01";
    if (ms < 0.1)
        return fmt(ms, 3);
    return fmt(ms, 2);
}
/** The slice of the render wedge `render_game` does not own: surface
 *  acquisition, encoder creation, submit, present and the loop around them.
 *  Always emitted, so the render group keeps a fixed row count. */
function outsideRow(ms, frameTotal) {
    const percent = ms > 0.01 ? `${fmt((ms / Math.max(frameTotal, 0.0001)) * 100, 1)}%` : "—";
    return (`<tr><td class="ph-name">${t("rnd.outside")}</td>` +
        `<td class="ph-bar"></td>` +
        `<td class="ph-ms"><b>${fmtPhase(ms)}</b></td>` +
        `<td class="ph-pct">—</td>` +
        `<td class="ph-pct">${percent}</td></tr>`);
}
/**
 * The full frame ledger: every stamped subsystem, heaviest first.
 *
 * `simTotal` / `frameTotal` come from `buildFrameSlices`, so the percentages
 * here are the pies' own denominators — the table can explain a wedge but can
 * never contradict it.
 */
function renderPhaseTable(table, profile, render, simTotal, frameTotal) {
    if (!table)
        return;
    // Rebuilding the table must not move the reader's scroll position.
    const scroller = table.closest(".table-wrap");
    const scrollTop = scroller?.scrollTop ?? 0;
    const values = profile.phases_ms ?? [];
    // Engine and dashboard disagree on the ledger: say so rather than drawing
    // rows under the wrong names.
    if (values.length !== PHASE_KEYS.length) {
        table.innerHTML =
            `<tr><td class="ph-empty">phase ledger mismatch: ` +
                `${values.length} phases on the wire, ${PHASE_KEYS.length} expected</td></tr>`;
        return;
    }
    const rows = PHASE_KEYS.map((key, index) => ({
        label: t(`phase.${key}`),
        ms: values[index] ?? 0,
        order: index,
    }));
    // Aborted frames leave a residue the phases never stamped. The row is always
    // present — letting it appear and vanish resizes the table under the reader —
    // and reads "—" when there is nothing to report.
    rows.push({ label: t("rep.unmarked"), ms: profile.unmarked_ms, order: PHASE_KEYS.length });
    // Heaviest first, but rows below a hundredth of a millisecond are all noise:
    // keep those in their declared order so they stop trading places on every
    // refresh, which is what the eye reads as jitter.
    rows.sort((a, b) => {
        const quiet = 0.01;
        if (a.ms < quiet && b.ms < quiet)
            return a.order - b.order;
        return b.ms - a.ms;
    });
    const share = (ms, total) => total > 0 ? Math.min(100, (ms / total) * 100) : 0;
    const body = rows
        .map(({ label, ms }) => {
        const ofSim = share(ms, simTotal);
        const ofFrame = share(ms, frameTotal);
        // The bar measures the phase against its own neighbours (share of the
        // simulation), not against the frame: a 9 %-of-frame row would draw a bar
        // too short to compare at a glance.
        return (`<tr><td class="ph-name">${label}</td>` +
            `<td class="ph-bar" style="--share:${ofSim.toFixed(2)}%"><span class="ph-fill"></span></td>` +
            `<td class="ph-ms"><b>${fmtPhase(ms)}</b></td>` +
            `<td class="ph-pct">${ms > 0 ? `${fmt(ofSim, 1)}%` : "—"}</td>` +
            `<td class="ph-pct">${ms > 0 ? `${fmt(ofFrame, 1)}%` : "—"}</td></tr>`);
    })
        .join("");
    // The render's own phases, under a group header. Their share of the
    // simulation would be meaningless, so that column reads "—" here.
    const renderMs = Math.max(0, frameTotal - simTotal);
    const renderTotal = render
        ? render.world_ms + render.debug_ms + render.shroud_ms + render.ui_ms +
            render.sidebar_ms + render.upload_ms + render.other_ms
        : 0;
    const renderRows = render
        ? [
            { label: t("rnd.world"), ms: render.world_ms },
            { label: t("rnd.debug"), ms: render.debug_ms },
            { label: t("rnd.shroud"), ms: render.shroud_ms },
            { label: t("rnd.ui"), ms: render.ui_ms },
            { label: t("rnd.sidebar"), ms: render.sidebar_ms },
            { label: t("rnd.upload"), ms: render.upload_ms },
            { label: t("rnd.other"), ms: render.other_ms },
        ].sort((a, b) => b.ms - a.ms)
        : [];
    const renderBody = renderRows.length === 0
        ? ""
        : `<tbody><tr class="ph-group"><td colspan="5">${t("rnd.group")}</td></tr>` +
            renderRows
                .map(({ label, ms }) => `<tr><td class="ph-name" title="${label}">${label}</td>` +
                `<td class="ph-bar" style="--share:${share(ms, renderTotal).toFixed(2)}%">` +
                `<span class="ph-fill"></span></td>` +
                `<td class="ph-ms"><b>${fmtPhase(ms)}</b></td>` +
                `<td class="ph-pct">—</td>` +
                `<td class="ph-pct">${ms > 0 ? `${fmt(share(ms, frameTotal), 1)}%` : "—"}</td></tr>`)
                .join("") +
            // `render_game` is not the whole render wedge: surface acquisition, the
            // command encoder, submit, present and the rest of the loop sit outside
            // it. Showing the difference keeps this group summing to the footer.
            outsideRow(renderMs - renderTotal, frameTotal) +
            `</tbody>`;
    // Close the ledger with the frame's own account: simulation, the render /
    // submit remainder, and the frame those two add up to. The footer prints the
    // same totals the pies do, so a reader can arrive from either direction.
    const foot = `<tfoot>` +
        `<tr><td class="ph-name">${t("rep.phaseTotal")}</td>` +
        `<td class="ph-bar"></td>` +
        `<td class="ph-ms"><b>${fmtPhase(simTotal)}</b></td>` +
        `<td class="ph-pct">100%</td>` +
        `<td class="ph-pct">${fmt(share(simTotal, frameTotal), 1)}%</td></tr>` +
        `<tr><td class="ph-name">${t("rep.render")}</td>` +
        `<td class="ph-bar"></td>` +
        `<td class="ph-ms"><b>${fmtPhase(renderMs)}</b></td>` +
        `<td class="ph-pct">—</td>` +
        `<td class="ph-pct">${fmt(share(renderMs, frameTotal), 1)}%</td></tr>` +
        `<tr><td class="ph-name">${t("rep.whole")}</td>` +
        `<td class="ph-bar"></td>` +
        `<td class="ph-ms"><b>${fmtPhase(frameTotal)}</b></td>` +
        `<td class="ph-pct">—</td><td class="ph-pct">100%</td></tr>` +
        `</tfoot>`;
    table.innerHTML =
        `<thead><tr><th>${t("th.phase")}</th><th class="ph-bar-col"></th><th>${t("th.ms")}</th>` +
            `<th>${t("th.shareSim")}</th><th>${t("th.shareFrame")}</th></tr></thead>` +
            `<tbody>${body}</tbody>${renderBody}${foot}`;
    if (scroller)
        scroller.scrollTop = scrollTop;
}
const EXPORT_FONT = "'Microsoft YaHei', 'PingFang SC', 'Segoe UI', sans-serif";
/** One design token off `:root`, or the fallback it was introduced with. */
function token(name, fallback) {
    const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return value || fallback;
}
/** `#rrggbb` at reduced strength — canvas has no `color-mix()` to soften a token. */
function soften(color, alpha) {
    const hex = /^#([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i.exec(color);
    if (!hex)
        return color;
    const [r, g, b] = [1, 2, 3].map((group) => parseInt(hex[group], 16));
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}
function exportPalette() {
    return {
        top: token("--bg-hi", "#101d3e"),
        bottom: token("--bg", "#070b18"),
        title: token("--title", "#9fd8ff"),
        strong: token("--strong", "#f0f6ff"),
        ink: token("--text", "#dce7f8"),
        dim: token("--dim", "#8ea6c6"),
        accent: token("--cyan", "#6fb7ff"),
        violet: token("--blue", "#a884ff"),
    };
}
/** Read the rendered ledger back out of the DOM. Both the standalone snapshot
 *  and the full report draw from this, so they can never list different rows. */
function readLedgerRows() {
    const table = document.getElementById("report-phases");
    const rows = [];
    if (!table)
        return rows;
    table.querySelectorAll("tr").forEach((tr) => {
        if (tr.closest("thead"))
            return;
        const cells = tr.querySelectorAll("td");
        if (cells.length === 0)
            return;
        const barCell = tr.querySelector("td.ph-bar");
        const bar = barCell ? Number.parseFloat(barCell.style.getPropertyValue("--share")) : 0;
        const offset = cells.length === 1 ? 0 : 1;
        rows.push({
            name: (cells[0]?.textContent ?? "").trim(),
            ms: (cells[1 + offset]?.textContent ?? "").trim(),
            sim: (cells[2 + offset]?.textContent ?? "").trim(),
            frame: (cells[3 + offset]?.textContent ?? "").trim(),
            bar: Number.isFinite(bar) ? bar : 0,
            group: tr.classList.contains("ph-group"),
            foot: tr.closest("tfoot") !== null,
        });
    });
    return rows;
}
function buildPhaseLedgerCanvas() {
    const rows = readLedgerRows();
    if (rows.length === 0)
        return null;
    const dpr = Math.min(3, Math.max(2, window.devicePixelRatio || 1));
    const width = 900;
    const pad = 22;
    const barX = 300;
    const barW = 240;
    const msRight = 650;
    const simRight = 780;
    const frameRight = width - pad;
    const titleH = 54;
    const headH = 24;
    const rowH = 23;
    const groupH = 30;
    const noteH = 28;
    const height = titleH + headH + rows.reduce((sum, row) => sum + (row.group ? groupH : rowH), 0) + noteH;
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx)
        return null;
    ctx.scale(dpr, dpr);
    ctx.textBaseline = "middle";
    const pal = exportPalette();
    const background = ctx.createLinearGradient(0, 0, 0, height);
    background.addColorStop(0, pal.top);
    background.addColorStop(1, pal.bottom);
    ctx.fillStyle = background;
    ctx.fillRect(0, 0, width, height);
    const rule = (y, color, weight = 1) => {
        ctx.strokeStyle = color;
        ctx.lineWidth = weight;
        ctx.beginPath();
        ctx.moveTo(pad, y);
        ctx.lineTo(width - pad, y);
        ctx.stroke();
    };
    // Title block: what this is, and which tick it was measured on.
    ctx.textAlign = "left";
    ctx.fillStyle = pal.title;
    ctx.font = `600 15px ${EXPORT_FONT}`;
    ctx.fillText(t("rep.phases"), pad, 24);
    ctx.fillStyle = pal.dim;
    ctx.font = `11.5px ${EXPORT_FONT}`;
    ctx.fillText(`${t("rep.tickNow")} ${thousands(lastSnapshot?.tick ?? 0)} · ${t("rep.phaseHint")}`, pad, 43);
    let y = titleH;
    ctx.fillStyle = pal.dim;
    ctx.font = `600 10px ${EXPORT_FONT}`;
    ctx.textAlign = "left";
    ctx.fillText(t("th.phase"), pad, y + headH / 2);
    ctx.textAlign = "right";
    ctx.fillText(t("th.ms").toUpperCase(), msRight, y + headH / 2);
    ctx.fillText(t("th.shareSim").toUpperCase(), simRight, y + headH / 2);
    ctx.fillText(t("th.shareFrame").toUpperCase(), frameRight, y + headH / 2);
    y += headH;
    rule(y, soften(pal.accent, 0.34));
    for (const row of rows) {
        const h = row.group ? groupH : rowH;
        const mid = y + h / 2;
        if (row.group) {
            // Group header: a rule above and the label sitting on the lower edge.
            rule(y, soften(pal.accent, 0.24));
            ctx.textAlign = "left";
            ctx.fillStyle = pal.dim;
            ctx.font = `600 11px ${EXPORT_FONT}`;
            ctx.fillText(row.name, pad, y + groupH - 8);
            y += h;
            continue;
        }
        // No minimum width: a phase at 0.01 % should leave no bar rather than a
        // row of identical 2 px stubs. The fill matches `.ph-fill` in the wall.
        const barWidth = (row.bar / 100) * barW;
        if (barWidth >= 1) {
            const barPaint = ctx.createLinearGradient(barX, 0, barX + barW, 0);
            barPaint.addColorStop(0, soften(pal.accent, 0.5));
            barPaint.addColorStop(1, soften(pal.violet, 0.22));
            ctx.fillStyle = barPaint;
            if (typeof ctx.roundRect === "function") {
                ctx.beginPath();
                ctx.roundRect(barX, y + 4, barWidth, h - 8, 3);
                ctx.fill();
            }
            else {
                ctx.fillRect(barX, y + 4, barWidth, h - 8);
            }
        }
        ctx.textAlign = "left";
        ctx.font = `${row.foot ? "600 " : ""}12px ${EXPORT_FONT}`;
        ctx.fillStyle = row.foot ? pal.strong : pal.ink;
        ctx.fillText(row.name, pad, mid);
        ctx.textAlign = "right";
        ctx.fillStyle = row.foot ? pal.strong : pal.ink;
        ctx.fillText(row.ms, msRight, mid);
        ctx.fillStyle = pal.dim;
        ctx.fillText(row.sim, simRight, mid);
        ctx.fillText(row.frame, frameRight, mid);
        rule(y + h, soften(pal.accent, row.foot ? 0.28 : 0.09));
        y += h;
    }
    ctx.textAlign = "left";
    ctx.font = `11px ${EXPORT_FONT}`;
    ctx.fillStyle = soften(pal.dim, 0.8);
    ctx.fillText(`VERA20k · schema v${SCHEMA_VERSION} · ${new Date().toLocaleString()}`, pad, y + noteH / 2 + 2);
    return canvas;
}
/** The PNG behind the preview, kept so copy and save act on exactly the bytes
 *  the user is looking at. */
let shotBlob = null;
let shotUrl = null;
/** Build the ledger and show it in the preview dialog. Nothing is downloaded
 *  until the user asks — the table is taller than its panel, so seeing the
 *  whole thing is the point. */
/** One pie wedge as an SVG path — the same geometry the canvas pie uses. */
function pieWedge(cx, cy, r, start, sweep) {
    const at = (deg) => {
        const rad = (deg * Math.PI) / 180;
        return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)];
    };
    const [x0, y0] = at(start);
    const [x1, y1] = at(start + sweep);
    return (`M ${cx} ${cy} L ${x0.toFixed(2)} ${y0.toFixed(2)} ` +
        `A ${r} ${r} 0 ${sweep > 180 ? 1 : 0} 1 ${x1.toFixed(2)} ${y1.toFixed(2)} Z`);
}
/** The report's own palette: light, for pasting into a document or an issue. */
const REPORT_INK = "#1d2733";
const REPORT_DIM = "#5b6b7d";
const REPORT_RULE = "#e2e8f0";
/**
 * The whole report as one image: header facts, both pies, the phase ledger and
 * the participants.
 *
 * Light theme on purpose — this is a document to paste into an issue, not a
 * screenshot of the dashboard. The live panel keeps its dark styling.
 */
function buildReportCanvas() {
    const snapshot = lastSnapshot;
    const profile = snapshot?.profile ?? null;
    if (!snapshot || !profile || profile.frames === 0)
        return null;
    const frameMs = snapshot.timing ? 1000 / Math.max(1, snapshot.timing.fps) : 0;
    const { sim: simSlices, frame: frameSlices, simTotal, frameTotal } = buildFrameSlices(profile, frameMs);
    const ledger = readLedgerRows();
    const parties = snapshot.houses.filter((house) => !house.passive);
    const width = 1000;
    const pad = 34;
    const headerH = 118;
    const pieH = 280;
    const tableHeadH = 28;
    const rowH = 23;
    const groupH = 30;
    const ledgerH = ledger.reduce((sum, row) => sum + (row.group ? groupH : rowH), 0);
    const partiesH = parties.length === 0 ? 0 : 48 + parties.length * 26;
    // Leaves room between the last participant row and the colophon.
    const footerH = 52;
    const height = headerH + pieH + tableHeadH + ledgerH + 26 + partiesH + footerH;
    const dpr = Math.min(3, Math.max(2, window.devicePixelRatio || 1));
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx)
        return null;
    ctx.scale(dpr, dpr);
    ctx.textBaseline = "middle";
    ctx.textAlign = "left";
    ctx.fillStyle = "#f7f9fc";
    ctx.fillRect(0, 0, width, height);
    const rule = (y, color = REPORT_RULE, weight = 1) => {
        ctx.strokeStyle = color;
        ctx.lineWidth = weight;
        ctx.beginPath();
        ctx.moveTo(pad, y);
        ctx.lineTo(width - pad, y);
        ctx.stroke();
    };
    // ---------------------------------------------------------------- header
    ctx.fillStyle = REPORT_INK;
    ctx.font = `700 22px ${EXPORT_FONT}`;
    ctx.fillText(t("rep.title"), pad, 40);
    ctx.fillStyle = REPORT_DIM;
    ctx.font = `12.5px ${EXPORT_FONT}`;
    const tickHz = snapshot.timing?.tick_hz ?? 0;
    const duration = tickHz > 0 ? snapshot.tick / tickHz : 0;
    ctx.fillText(`tick ${thousands(snapshot.tick)} · ${t("rep.duration")} ${fmt(duration, 0)} s · ` +
        `${t("rep.fps")} ${fmt(snapshot.timing?.fps ?? 0, 0)} fps · ${fmt(tickHz, 1)} tick/s · ` +
        `${t("hdr.map")} ${snapshot.map_width}x${snapshot.map_height}`, pad, 68);
    ctx.fillText(`${t("rep.frame")} ${fmt(frameTotal, 2)} ms · ${t("rep.sim")} ${fmt(profile.mean_frame_ms, 2)} ms ` +
        `(${fmt(frameTotal > 0 ? (profile.mean_frame_ms / frameTotal) * 100 : 0, 0)}%) · ` +
        `${t("rep.peak")} ${fmt(profile.peak_frame_ms, 2)} ms`, pad, 88);
    rule(headerH - 14);
    // ------------------------------------------------------------------ pies
    const radius = 88;
    const paintPie = (cx, cy, slices, total, caption) => {
        ctx.textAlign = "left";
        ctx.fillStyle = REPORT_DIM;
        ctx.font = `600 12px ${EXPORT_FONT}`;
        ctx.fillText(`${caption}  ${fmt(total, 2)} ms`, cx - radius, cy - radius - 24);
        let angle = -90;
        for (const slice of slices) {
            if (slice.ms <= 0 || total <= 0)
                continue;
            const sweep = Math.min(360, (slice.ms / total) * 360);
            ctx.beginPath();
            ctx.moveTo(cx, cy);
            ctx.arc(cx, cy, radius, (angle * Math.PI) / 180, ((angle + sweep) * Math.PI) / 180);
            ctx.closePath();
            ctx.fillStyle = slice.color;
            ctx.fill();
            ctx.strokeStyle = "#ffffff";
            ctx.lineWidth = 1.5;
            ctx.stroke();
            if (sweep >= 20) {
                const mid = ((angle + sweep / 2) * Math.PI) / 180;
                ctx.fillStyle = "#ffffff";
                ctx.font = `600 10.5px ${EXPORT_FONT}`;
                ctx.textAlign = "center";
                ctx.fillText(`${fmt((slice.ms / total) * 100, 0)}%`, cx + radius * 0.62 * Math.cos(mid), cy + radius * 0.62 * Math.sin(mid));
            }
            angle += sweep;
        }
        // Legend beside the pie, in the same order the wedges were drawn.
        let ly = cy - radius + 6;
        ctx.textAlign = "left";
        for (const slice of slices) {
            ctx.fillStyle = slice.color;
            ctx.fillRect(cx + radius + 22, ly - 4, 9, 9);
            ctx.fillStyle = "#3d4f63";
            ctx.font = `11px ${EXPORT_FONT}`;
            ctx.fillText(`${t(slice.key)}  ${fmt(slice.ms, 2)} ms · ${fmt(total > 0 ? (slice.ms / total) * 100 : 0, 1)}%`, cx + radius + 37, ly);
            ly += 17;
        }
    };
    const pieTop = headerH + 46;
    paintPie(pad + radius + 4, pieTop + radius, frameSlices, frameTotal, t("rep.whole"));
    paintPie(width / 2 + radius + 40, pieTop + radius, simSlices, simTotal, t("rep.simOnly"));
    // ---------------------------------------------------------------- ledger
    let y = headerH + pieH;
    ctx.fillStyle = REPORT_INK;
    ctx.font = `600 14px ${EXPORT_FONT}`;
    ctx.fillText(t("rep.phases"), pad, y + 8);
    y += 22;
    const nameX = pad;
    const barX = 330;
    const barW = 250;
    const msRight = 690;
    const simRight = 820;
    const frameRight = width - pad;
    ctx.fillStyle = "#6b7b8d";
    ctx.font = `600 10.5px ${EXPORT_FONT}`;
    ctx.textAlign = "left";
    ctx.fillText(t("th.phase"), nameX, y + 10);
    ctx.textAlign = "right";
    ctx.fillText(t("th.ms").toUpperCase(), msRight, y + 10);
    ctx.fillText(t("th.shareSim"), simRight, y + 10);
    ctx.fillText(t("th.shareFrame"), frameRight, y + 10);
    y += tableHeadH;
    rule(y, "#cbd5e1");
    for (const row of ledger) {
        const h = row.group ? groupH : rowH;
        const mid = y + h / 2;
        if (row.group) {
            rule(y, "#d8e0ea");
            ctx.textAlign = "left";
            ctx.fillStyle = "#4a5b6e";
            ctx.font = `600 11.5px ${EXPORT_FONT}`;
            ctx.fillText(row.name, nameX, y + groupH - 8);
            y += h;
            continue;
        }
        const barWidth = (row.bar / 100) * barW;
        if (barWidth >= 1) {
            ctx.fillStyle = "rgba(47, 111, 176, 0.18)";
            ctx.fillRect(barX, y + 4, barWidth, h - 8);
        }
        ctx.textAlign = "left";
        ctx.font = `${row.foot ? "600 " : ""}11.5px ${EXPORT_FONT}`;
        ctx.fillStyle = row.foot ? REPORT_INK : "#2f3f52";
        ctx.fillText(row.name, nameX, mid);
        ctx.textAlign = "right";
        ctx.fillText(row.ms, msRight, mid);
        ctx.fillStyle = REPORT_DIM;
        ctx.fillText(row.sim, simRight, mid);
        ctx.fillText(row.frame, frameRight, mid);
        rule(y + h, row.foot ? "#cbd5e1" : "#edf1f6");
        y += h;
    }
    // ------------------------------------------------------------ participants
    y += 26;
    if (parties.length > 0) {
        ctx.textAlign = "left";
        ctx.fillStyle = REPORT_INK;
        ctx.font = `600 14px ${EXPORT_FONT}`;
        ctx.fillText(t("rep.houses"), pad, y);
        y += 24;
        ctx.fillStyle = "#6b7b8d";
        ctx.font = `600 10.5px ${EXPORT_FONT}`;
        ctx.fillText(t("th.house"), nameX, y);
        ctx.fillText(t("rep.kind"), 330, y);
        ctx.textAlign = "right";
        ctx.fillText(t("rep.buildings"), 640, y);
        ctx.fillText(t("rep.credits"), 800, y);
        ctx.fillText(t("rep.score"), frameRight, y);
        y += 16;
        rule(y, "#cbd5e1");
        y += 8;
        for (const house of parties) {
            ctx.textAlign = "left";
            ctx.fillStyle = "#2f3f52";
            ctx.font = `11.5px ${EXPORT_FONT}`;
            ctx.fillText(house.name, nameX, y + 8);
            ctx.fillText(house.is_human ? t("tag.human") : t("tag.ai"), 330, y + 8);
            ctx.textAlign = "right";
            ctx.fillText(fmt(house.counts.structure, 0), 640, y + 8);
            ctx.fillStyle = REPORT_DIM;
            ctx.fillText(fmt(house.credits, 0), 800, y + 8);
            ctx.fillText(fmt(house.score, 0), frameRight, y + 8);
            y += 26;
        }
    }
    // ---------------------------------------------------------------- footer
    ctx.textAlign = "left";
    ctx.font = `10.5px ${EXPORT_FONT}`;
    ctx.fillStyle = "#8b9aab";
    ctx.fillText(`VERA20k · schema v${SCHEMA_VERSION} · ${new Date().toLocaleString()}`, pad, height - 18);
    return canvas;
}
/**
 * The same report as a self-contained HTML file: inline SVG pies, no scripts,
 * no network — it opens years later and can be attached to a bug report.
 */
function buildReportHtml() {
    const snapshot = lastSnapshot;
    const profile = snapshot?.profile ?? null;
    if (!snapshot || !profile || profile.frames === 0)
        return null;
    const frameMs = snapshot.timing ? 1000 / Math.max(1, snapshot.timing.fps) : 0;
    const { sim: simSlices, frame: frameSlices, simTotal, frameTotal } = buildFrameSlices(profile, frameMs);
    const ledger = readLedgerRows();
    const parties = snapshot.houses.filter((house) => !house.passive);
    const tickHz = snapshot.timing?.tick_hz ?? 0;
    const duration = tickHz > 0 ? snapshot.tick / tickHz : 0;
    const pie = (slices, total, caption) => {
        const radius = 88;
        const size = 180;
        let angle = -90;
        const wedges = slices
            .filter((slice) => slice.ms > 0 && total > 0)
            .map((slice) => {
            const sweep = Math.min(360, (slice.ms / total) * 360);
            const path = pieWedge(size / 2, size / 2, radius, angle, sweep);
            angle += sweep;
            return `<path d="${path}" fill="${slice.color}" stroke="#fff" stroke-width="1.5"/>`;
        })
            .join("");
        const legend = slices
            .map((slice) => `<li><span class="sw" style="background:${slice.color}"></span>` +
            `<span class="lb">${escapeHtml(t(slice.key))}</span>` +
            `<span class="ms">${fmt(slice.ms, 2)} ms · ` +
            `${fmt(total > 0 ? (slice.ms / total) * 100 : 0, 1)}%</span></li>`)
            .join("");
        return (`<figure class="pie"><figcaption>${escapeHtml(caption)} <b>${fmt(total, 2)} ms</b></figcaption>` +
            `<div class="body"><svg viewBox="0 0 ${size} ${size}" width="${size}" height="${size}">${wedges}</svg>` +
            `<ul>${legend}</ul></div></figure>`);
    };
    const rows = ledger
        .map((row) => row.group
        ? `<tr class="group"><td colspan="5">${escapeHtml(row.name)}</td></tr>`
        : `<tr class="${row.foot ? "foot" : ""}"><td>${escapeHtml(row.name)}</td>` +
            `<td class="n">${row.ms}</td><td class="n">${row.sim}</td>` +
            `<td class="n">${row.frame}</td></tr>`)
        .join("");
    const partyRows = parties
        .map((house) => `<tr><td>${escapeHtml(house.name)}</td>` +
        `<td>${house.is_human ? t("tag.human") : t("tag.ai")}</td>` +
        `<td class="n">${fmt(house.counts.structure, 0)}</td>` +
        `<td class="n">${fmt(house.credits, 0)}</td>` +
        `<td class="n">${fmt(house.score, 0)}</td></tr>`)
        .join("");
    const share = frameTotal > 0 ? (profile.mean_frame_ms / frameTotal) * 100 : 0;
    return `<!doctype html>
<html lang="${document.documentElement.lang || "en"}">
<head><meta charset="utf-8"><title>${escapeHtml(t("rep.title"))} · tick ${thousands(snapshot.tick)}</title>
<style>
*{box-sizing:border-box;margin:0}
body{font:14px/1.5 -apple-system,'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif;color:#1d2733;background:#f7f9fc;padding:28px 32px}
h1{font-size:24px;font-weight:700}
h2{font-size:14px;font-weight:600;color:#4a5b6e;margin:26px 0 10px}
.sub{color:#5b6b7d;font-size:13px;margin-top:6px}
.pies{display:flex;gap:34px;flex-wrap:wrap;align-items:flex-start}
figure.pie{flex:1 1 430px;min-width:0;background:#fff;border:1px solid #e2e8f0;border-radius:8px;padding:18px 20px;box-shadow:0 1px 3px rgba(16,24,40,.06)}
figcaption{font-size:13px;color:#4a5b6e;margin-bottom:10px}
.body{display:flex;align-items:center;gap:22px}
ul{list-style:none}
li{display:flex;align-items:center;gap:8px;margin:5px 0;font-size:12.5px}
.sw{width:10px;height:10px;border-radius:2px;flex:none}
.lb{min-width:118px}
.ms{color:#5b6b7d;font-variant-numeric:tabular-nums}
table{border-collapse:collapse;font-variant-numeric:tabular-nums;width:100%}
th,td{text-align:left;padding:4px 22px 4px 0;border-bottom:1px solid #e8edf3;font-size:13px}
th{color:#6b7b8d;font-weight:600;font-size:12px}
th.n,td.n{text-align:right;padding-right:0}
tr.group td{color:#4a5b6e;font-weight:600;padding-top:14px}
tr.foot td{font-weight:600;color:#1d2733;border-bottom:none}
.note{margin-top:22px;color:#7b8b9d;font-size:12px}
</style></head>
<body>
<h1>${escapeHtml(t("rep.title"))}</h1>
<div class="sub">tick ${thousands(snapshot.tick)} · ${t("rep.duration")} ${fmt(duration, 0)} s · ${t("rep.fps")} ${fmt(snapshot.timing?.fps ?? 0, 0)} fps · ${fmt(tickHz, 1)} tick/s · ${t("hdr.map")} ${snapshot.map_width}x${snapshot.map_height}<br>
${t("rep.frame")} ${fmt(frameTotal, 2)} ms · ${t("rep.sim")} ${fmt(profile.mean_frame_ms, 2)} ms（${fmt(share, 0)}%） · ${t("rep.peak")} ${fmt(profile.peak_frame_ms, 2)} ms</div>
<div class="pies">${pie(frameSlices, frameTotal, t("rep.whole"))}${pie(simSlices, simTotal, t("rep.simOnly"))}</div>
<h2>${escapeHtml(t("rep.phases"))}</h2>
<table><thead><tr><th>${escapeHtml(t("th.phase"))}</th><th class="n">${escapeHtml(t("th.ms").toUpperCase())}</th><th class="n">${escapeHtml(t("th.shareSim"))}</th><th class="n">${escapeHtml(t("th.shareFrame"))}</th></tr></thead><tbody>${rows}</tbody></table>
${parties.length > 0 ? `<h2>${escapeHtml(t("rep.houses"))}</h2><table><thead><tr><th>${escapeHtml(t("th.house"))}</th><th>${escapeHtml(t("rep.kind"))}</th><th class="n">${escapeHtml(t("rep.buildings"))}</th><th class="n">${escapeHtml(t("rep.credits"))}</th><th class="n">${escapeHtml(t("rep.score"))}</th></tr></thead><tbody>${partyRows}</tbody></table>` : ""}
<p class="note">VERA20k · schema v${SCHEMA_VERSION} · ${new Date().toLocaleString()}</p>
</body></html>`;
}
function openShotPreview(kind = "ledger") {
    const canvas = kind === "report" ? buildReportCanvas() : buildPhaseLedgerCanvas();
    if (!canvas)
        return;
    // The report can also be saved as HTML; the ledger snapshot cannot.
    shotHtml = kind === "report" ? buildReportHtml() : null;
    canvas.toBlob((blob) => {
        if (!blob)
            return;
        if (shotUrl)
            URL.revokeObjectURL(shotUrl);
        shotBlob = blob;
        shotUrl = URL.createObjectURL(blob);
        const image = document.getElementById("shot-img");
        if (image)
            image.src = shotUrl;
        const title = document.getElementById("shot-title");
        if (title)
            title.textContent = t(kind === "report" ? "rep.reportShot" : "rep.shotTitle");
        const htmlButton = document.getElementById("shot-html");
        if (htmlButton)
            htmlButton.hidden = shotHtml === null;
        const modal = document.getElementById("shot-modal");
        if (modal)
            modal.hidden = false;
        shotNote("", "ok");
    }, "image/png");
}
function closeShot() {
    const modal = document.getElementById("shot-modal");
    if (modal)
        modal.hidden = true;
    const image = document.getElementById("shot-img");
    if (image)
        image.removeAttribute("src");
    if (shotUrl) {
        URL.revokeObjectURL(shotUrl);
        shotUrl = null;
    }
    shotBlob = null;
    shotHtml = null;
}
function shotNote(text, tone) {
    const note = document.getElementById("shot-note");
    if (!note)
        return;
    note.textContent = text;
    note.style.color = tone === "bad" ? "var(--amber)" : "var(--green)";
}
/** Put the image on the clipboard. Needs a secure context; 127.0.0.1 has one. */
async function copyShot() {
    if (!shotBlob)
        return;
    try {
        await navigator.clipboard.write([new ClipboardItem({ "image/png": shotBlob })]);
        shotNote(t("rep.copied"), "ok");
    }
    catch {
        // A denied clipboard is not fatal: the download path still works.
        shotNote(t("rep.copyFailed"), "bad");
    }
}
function saveShot() {
    if (!shotUrl)
        return;
    const link = document.createElement("a");
    link.href = shotUrl;
    link.download = `phase-ledger-tick${lastSnapshot?.tick ?? 0}.png`;
    link.click();
}
/** Save the report as a self-contained HTML file, built in this tab. */
function saveReportHtml() {
    if (!shotHtml)
        return;
    const blob = new Blob([shotHtml], { type: "text/html;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `vera20k-report-tick${lastSnapshot?.tick ?? 0}.html`;
    link.click();
    URL.revokeObjectURL(url);
}
/** The HTML behind a report preview, when the preview is a report. */
let shotHtml = null;
function updateFrameProfile(profile, frameMs) {
    const summary = document.getElementById("frame-summary");
    if (!summary)
        return;
    if (!profile || profile.frames === 0) {
        document.getElementById("pie-frame").textContent = "";
        document.getElementById("pie-sim").textContent = "";
        document.getElementById("pie-frame-legend").textContent = "";
        document.getElementById("pie-sim-legend").textContent = "";
        document.getElementById("pie-frame-total").textContent = "-";
        document.getElementById("pie-sim-total").textContent = "-";
        summary.innerHTML = `<div class='row'><span>${t("rep.waiting")}</span></div>`;
        return;
    }
    const { sim: simSlices, frame: frameSlices, simTotal, frameTotal } = buildFrameSlices(profile, frameMs);
    drawPie("pie-frame", "pie-frame-legend", frameSlices, frameTotal);
    drawPie("pie-sim", "pie-sim-legend", simSlices, simTotal);
    document.getElementById("pie-frame-total").textContent = `${fmt(frameTotal, 1)} ms`;
    document.getElementById("pie-sim-total").textContent = `${fmt(simTotal, 1)} ms`;
    const share = frameTotal > 0 ? (simTotal / frameTotal) * 100 : 0;
    const rows = [
        [t("rep.frame"), `${fmt(frameMs, 2)} ms`, false],
        [t("rep.sim"), `${fmt(profile.mean_frame_ms, 2)} ms`, false],
        [t("rep.objects"), `${fmt(profile.objects_ms, 2)} ms`, false],
        [t("rep.peak"), `${fmt(profile.peak_frame_ms, 2)} ms`, false],
        [t("rep.frames"), fmt(profile.frames, 0), false],
        [t("rep.share"), `${fmt(share, 0)}%`, false],
    ];
    summary.innerHTML = rows
        .map(([key, value, hot]) => `<div class='row${hot ? " hot" : ""}'><span>${key}</span><b>${value}</b></div>`)
        .join("");
}
// ---------------------------------------------------- process page (帧耗时 panel, page 2)
/**
 * Host resources: CPU share of the whole machine, resident and peak working
 * set, live thread count, and the adapter plus this process's own resident
 * atlas textures. Hardware GPU utilization is deliberately not claimed here —
 * it needs vendor driver counters this build does not read.
 */
function updateProcess(process) {
    const value = (id, text, cls = "proc-value") => {
        const cell = document.getElementById(id);
        if (!cell)
            return;
        cell.textContent = text;
        cell.className = cls;
    };
    const meter = (id, fraction, hot) => {
        const bar = document.getElementById(id);
        if (!bar)
            return;
        bar.style.width = `${Math.min(100, Math.max(0, fraction * 100))}%`;
        bar.classList.toggle("hot", hot);
    };
    if (!process) {
        for (const id of [
            "proc-cpu",
            "proc-mem",
            "proc-threads",
            "proc-gpu",
            "proc-cores",
            "proc-peak",
            "proc-texture",
        ]) {
            value(id, "-");
        }
        meter("proc-cpu-bar", 0, false);
        meter("proc-mem-bar", 0, false);
        return;
    }
    // 80% of the machine is the interesting line: below it the host has slack,
    // above it the pacer is competing with everything else on the box.
    const cpuHot = process.cpu_percent >= 80;
    value("proc-cpu", `${fmt(process.cpu_percent, 1)}%`, `proc-value${cpuHot ? " hot" : ""}`);
    meter("proc-cpu-bar", process.cpu_percent / 100, cpuHot);
    value("proc-cores", `${process.cpu_cores} ${t("proc.cores")}`, "proc-sub");
    const memHot = process.peak_working_set_mb > 0 && process.working_set_mb >= process.peak_working_set_mb * 0.9;
    value("proc-mem", `${fmt(process.working_set_mb, 0)} MB`, `proc-value${memHot ? " hot" : ""}`);
    meter("proc-mem-bar", process.peak_working_set_mb > 0
        ? process.working_set_mb / process.peak_working_set_mb
        : 0, memHot);
    value("proc-peak", `${t("proc.peak")} ${fmt(process.peak_working_set_mb, 0)} MB`, "proc-sub");
    value("proc-threads", `${process.threads}`);
    value("proc-gpu", process.gpu_adapter ?? "-", "proc-value small");
    value("proc-texture", `${t("proc.texture")} ${fmt(process.gpu_texture_mb, 0)} MB`, "proc-sub");
}
/** Two panels paginate: the cost column switches between tick cost and host
 *  resources, the frame panel between whole frame and simulation only. Each
 *  panel carries its own `data-page`, so switching one never hides the other's
 *  content. A hidden page's canvas has zero size, so returning to the cost
 *  chart redraws it; the pies are inline SVG and need no redraw. */
function setupPager() {
    for (const panel of document.querySelectorAll(".panel[data-page]")) {
        for (const tab of panel.querySelectorAll(".pager-tab")) {
            tab.addEventListener("click", () => {
                const page = tab.dataset.page;
                if (!page)
                    return;
                panel.dataset.page = page;
                for (const sibling of panel.querySelectorAll(".pager-tab")) {
                    sibling.classList.toggle("on", sibling === tab);
                }
                if (panel.id === "cost" && page === "cost" && lastSnapshot) {
                    drawChart(lastSnapshot.timing);
                }
            });
        }
    }
}
// ------------------------------------------------------- detail wall (副屏)
/**
 * The player-detail view: economy over time, live cash, spend, score, base,
 * force composition and the unit-type roster — one card per house. It reads
 * the same snapshots as the wall; its per-house history accumulates even while
 * the wall view is showing, so switching over shows a continuous chart.
 */
/** Samples retained per house for the sparkline and the economy chart. */
const DETAIL_HISTORY = 240;
/** Per-house sampled series, keyed by house name (stable across a match). */
const houseHistories = new Map();
/** Cached card element handles, rebuilt only when the house set changes. */
let detailCardKeys = [];
/** Which wall is showing; the header tabs drive it. */
let currentView = "wall";
/** Category colours, shared with the main wall's composition bars. */
const DETAIL_CATEGORY_COLOR = {
    Infantry: "#3f8f6f",
    Unit: "#2f6fb0",
    Aircraft: "#8a5fc0",
    Structure: "#b0763a",
};
const DETAIL_ORPHAN_COLOR = "#6b7a8c";
function houseCss(house) {
    return house.color
        ? `rgb(${house.color[0]},${house.color[1]},${house.color[2]})`
        : DETAIL_ORPHAN_COLOR;
}
/** A house that never fights: Neutral / Special (MultiplayPassive in rules). */
function isPassiveHouse(house) {
    return !house.is_human && (house.side_name === "Civilian" || house.side_name === "Mutant");
}
function detailTotal(counts) {
    return counts.unit + counts.infantry + counts.aircraft + counts.structure;
}
function detailCategoryKey(category) {
    switch (category) {
        case "Infantry":
            return "cat.infantry";
        case "Unit":
            return "cat.vehicles";
        case "Aircraft":
            return "cat.aircraft";
        default:
            return "cat.structures";
    }
}
/** Push one sample per house; drop series whose house left the match. */
function sampleHouseEconomy(snapshot) {
    const live = new Set();
    for (const house of snapshot.houses) {
        live.add(house.name);
        let history = houseHistories.get(house.name);
        if (!history) {
            history = { credits: [] };
            houseHistories.set(house.name, history);
        }
        history.credits.push(house.credits);
        if (history.credits.length > DETAIL_HISTORY)
            history.credits.shift();
    }
    for (const name of [...houseHistories.keys()]) {
        if (!live.has(name))
            houseHistories.delete(name);
    }
}
function fitDetailCanvas(canvas) {
    const ratio = window.devicePixelRatio || 1;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    if (width === 0 || height === 0)
        return null;
    if (canvas.width !== Math.round(width * ratio) || canvas.height !== Math.round(height * ratio)) {
        canvas.width = Math.round(width * ratio);
        canvas.height = Math.round(height * ratio);
    }
    const ctx = canvas.getContext("2d");
    if (!ctx)
        return null;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, width, height);
    return ctx;
}
/** Multi-line credits chart: one coloured trace per fighting house. */
function drawEconomyChart(snapshot) {
    const canvas = document.getElementById("economy");
    if (!canvas)
        return;
    const ctx = fitDetailCanvas(canvas);
    if (!ctx)
        return; // hidden view: zero-sized, redrawn on activation
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    const series = snapshot.houses
        .filter((house) => !isPassiveHouse(house))
        .map((house) => ({ house, values: houseHistories.get(house.name)?.credits ?? [] }))
        .filter((entry) => entry.values.length > 0);
    const peak = Math.max(1, ...series.flatMap((entry) => entry.values));
    ctx.strokeStyle = "rgba(0, 190, 255, 0.10)";
    ctx.lineWidth = 1;
    for (let band = 0; band <= 4; band++) {
        const y = 6 + (band / 4) * (height - 18);
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
    }
    const span = DETAIL_HISTORY - 1;
    for (const { house, values } of series) {
        const color = houseCss(house);
        ctx.save();
        ctx.strokeStyle = color;
        ctx.lineWidth = 1.8;
        ctx.shadowColor = color;
        ctx.shadowBlur = 8;
        ctx.beginPath();
        values.forEach((value, index) => {
            // Newest sample sits at the right edge; a short series grows leftward.
            const x = width - ((values.length - 1 - index) / span) * width;
            const y = 6 + (1 - value / peak) * (height - 18);
            if (index === 0)
                ctx.moveTo(x, y);
            else
                ctx.lineTo(x, y);
        });
        ctx.stroke();
        ctx.restore();
    }
    ctx.fillStyle = "rgba(160, 210, 235, 0.75)";
    ctx.font = "10px monospace";
    ctx.fillText(fmt(peak), 4, 12);
    ctx.fillText("0", 4, height - 6);
    const legend = document.getElementById("economy-legend");
    if (legend) {
        legend.textContent = "";
        for (const { house } of series) {
            const item = document.createElement("span");
            const dot = document.createElement("i");
            dot.style.background = houseCss(house);
            item.append(dot, document.createTextNode(house.name));
            legend.append(item);
        }
    }
}
function drawSparkline(canvas, values, color) {
    const ctx = fitDetailCanvas(canvas);
    if (!ctx || values.length === 0)
        return;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    const peak = Math.max(1, ...values);
    ctx.save();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.4;
    ctx.shadowColor = color;
    ctx.shadowBlur = 6;
    ctx.beginPath();
    values.forEach((value, index) => {
        const x = (index / Math.max(1, values.length - 1)) * width;
        const y = 4 + (1 - value / peak) * (height - 8);
        if (index === 0)
            ctx.moveTo(x, y);
        else
            ctx.lineTo(x, y);
    });
    ctx.stroke();
    ctx.restore();
}
function buildDetailCard(house) {
    const root = document.createElement("section");
    root.className = "panel house-card";
    const head = document.createElement("div");
    head.className = "card-head";
    const dot = document.createElement("i");
    dot.className = "house-dot";
    const name = document.createElement("span");
    name.className = "card-name";
    name.textContent = house.name;
    name.title = house.name;
    const spacer = document.createElement("span");
    spacer.className = "spacer";
    const tags = document.createElement("span");
    head.append(dot, name, spacer, tags);
    const econ = document.createElement("div");
    econ.className = "card-econ";
    const big = document.createElement("div");
    big.className = "big";
    const credits = document.createElement("div");
    credits.className = "value";
    const creditsLabel = document.createElement("div");
    creditsLabel.className = "label";
    creditsLabel.textContent = t("dp.credits");
    const delta = document.createElement("div");
    delta.className = "delta";
    big.append(credits, creditsLabel, delta);
    const spark = document.createElement("canvas");
    spark.className = "spark";
    econ.append(big, spark);
    const stats = document.createElement("div");
    stats.className = "stat-grid";
    const forceTitle = document.createElement("div");
    forceTitle.className = "sub";
    forceTitle.textContent = t("sec.pop");
    const bar = document.createElement("div");
    bar.className = "comp-bar";
    const compLegend = document.createElement("div");
    compLegend.className = "comp-legend";
    const rosterTitle = document.createElement("div");
    rosterTitle.className = "sub";
    rosterTitle.textContent = t("dp.roster");
    // The full list lives in the click-through modal. What stays on the card is
    // one line of evidence that there is something to open: the top types as
    // chips, and the roster length, so a house that fields many types reads as
    // "worth opening" without anyone having to discover the scroll box first.
    const rosterPeek = document.createElement("button");
    rosterPeek.type = "button";
    rosterPeek.className = "roster-peek";
    root.append(head, econ, stats, forceTitle, bar, compLegend, rosterTitle, rosterPeek);
    root.tabIndex = 0;
    root.setAttribute("role", "button");
    root.title = t("dp.openDetail");
    return { root, dot, tags, credits, delta, spark, stats, bar, compLegend, rosterPeek };
}
function renderDetailCards(snapshot) {
    const container = document.getElementById("cards");
    if (!container)
        return;
    // Human players first, then AI, then the passive houses; name breaks ties.
    const houses = [...snapshot.houses].sort((a, b) => {
        const rank = (house) => house.is_human ? 0 : isPassiveHouse(house) ? 2 : 1;
        return rank(a) - rank(b) || a.name.localeCompare(b.name);
    });
    const keys = houses.map((house) => house.name);
    const views = new Map();
    if (keys.join("\u0000") !== detailCardKeys.join("\u0000")) {
        container.textContent = "";
        for (const house of houses) {
            const view = buildDetailCard(house);
            // The whole card is the target, and the roster line inside it is a real
            // button — so it also works by keyboard, and the peek line keeps the
            // same affordance for anyone who looks for it first.
            view.root.addEventListener("click", () => openHouseModal(house.name));
            view.root.addEventListener("keydown", (event) => {
                if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    openHouseModal(house.name);
                }
            });
            view.rosterPeek.addEventListener("click", (event) => {
                event.stopPropagation();
                openHouseModal(house.name);
            });
            container.append(view.root);
            views.set(house.name, view);
        }
        detailCardKeys = keys;
    }
    else {
        const existing = container.querySelectorAll(".house-card");
        houses.forEach((house, index) => {
            const root = existing[index];
            if (!root)
                return;
            views.set(house.name, {
                root,
                dot: root.querySelector(".house-dot"),
                tags: root.querySelector(".card-head span:last-child"),
                credits: root.querySelector(".card-econ .value"),
                delta: root.querySelector(".card-econ .delta"),
                spark: root.querySelector(".spark"),
                stats: root.querySelector(".stat-grid"),
                bar: root.querySelector(".comp-bar"),
                compLegend: root.querySelector(".comp-legend"),
                rosterPeek: root.querySelector(".roster-peek"),
            });
        });
    }
    for (const house of houses) {
        const view = views.get(house.name);
        if (!view)
            continue;
        const color = houseCss(house);
        const history = houseHistories.get(house.name)?.credits ?? [];
        const previous = history.length > 1 ? history[history.length - 2] : house.credits;
        const delta = house.credits - previous;
        view.root.classList.toggle("passive", isPassiveHouse(house));
        view.root.classList.toggle("defeated", house.defeated);
        view.dot.style.background = color;
        view.dot.style.boxShadow = `0 0 8px ${color}`;
        const badges = [
            house.is_human
                ? `<span class="tag human">${t("tag.human")}</span>`
                : `<span class="tag ai ${house.difficulty}">${t(`diff.${house.difficulty}`)}</span>`,
        ];
        if (isPassiveHouse(house))
            badges.push(`<span class="tag passive">${t("dp.passive")}</span>`);
        if (house.defeated)
            badges.push(`<span class="tag dead">${t("dp.defeated")}</span>`);
        view.tags.innerHTML = badges.join(" ");
        view.credits.textContent = fmt(house.credits, 0);
        view.delta.className = `delta ${delta > 0 ? "up" : delta < 0 ? "down" : ""}`;
        view.delta.textContent =
            delta === 0 ? "" : `${delta > 0 ? "▲" : "▼"} ${fmt(Math.abs(delta), 0)}`;
        const cells = [
            [t("dp.spent"), fmt(house.spent, 0)],
            [t("dp.score"), fmt(house.score, 0)],
            [t("dp.buildings"), fmt(house.counts.structure, 0)],
            [t("dp.base"), house.base_center ? `${house.base_center[0]}, ${house.base_center[1]}` : "-"],
            [t("dp.side"), sideName(house)],
            [t("dp.units"), fmt(detailTotal(house.counts), 0)],
        ];
        view.stats.innerHTML = cells
            .map(([key, value]) => `<div><span class="k">${key}</span><b>${value}</b></div>`)
            .join("");
        const total = Math.max(1, detailTotal(house.counts));
        view.bar.textContent = "";
        const segments = [
            ["infantry", DETAIL_CATEGORY_COLOR.Infantry],
            ["unit", DETAIL_CATEGORY_COLOR.Unit],
            ["aircraft", DETAIL_CATEGORY_COLOR.Aircraft],
            ["structure", DETAIL_CATEGORY_COLOR.Structure],
        ];
        for (const [key, css] of segments) {
            const count = house.counts[key];
            if (count === 0)
                continue;
            const seg = document.createElement("span");
            seg.className = "seg";
            seg.style.width = `${(count / total) * 100}%`;
            seg.style.background = css;
            view.bar.append(seg);
        }
        view.compLegend.innerHTML = segments
            .filter(([key]) => house.counts[key] > 0)
            .map(([key, css]) => `<span><i style="background:${css}"></i>${t(detailCategoryKey(key === "infantry" ? "Infantry" : key === "unit" ? "Unit" : key === "aircraft" ? "Aircraft" : "Structure"))} ${house.counts[key]}</span>`)
            .join("");
        // One line of evidence, not the list itself: the full roster is a click
        // away in the house modal, where the whole list can scroll.
        view.rosterPeek.textContent = "";
        if (house.roster.length === 0) {
            const empty = document.createElement("span");
            empty.className = "peek-empty";
            empty.textContent = t("dp.none");
            view.rosterPeek.append(empty);
        }
        else {
            // The wire caps the roster at TELEMETRY_MAX_ROSTER types, so "N types"
            // is the reported length; the engine truncates the tail, it does not
            // page it, and the modal is where that cap becomes visible.
            for (const entry of house.roster.slice(0, HOUSE_PEEK_TYPES)) {
                const chip = document.createElement("span");
                chip.className = "peek-chip";
                const dot = document.createElement("i");
                dot.style.background = DETAIL_CATEGORY_COLOR[entry.category] ?? DETAIL_ORPHAN_COLOR;
                const name = document.createElement("b");
                name.textContent = entry.type_name;
                const count = document.createElement("em");
                count.textContent = fmt(entry.count, 0);
                chip.append(dot, name, count);
                chip.title = `${entry.type_name} · ${t(detailCategoryKey(entry.category))}`;
                view.rosterPeek.append(chip);
            }
            const more = document.createElement("span");
            more.className = "peek-more";
            const hidden = house.roster.length - HOUSE_PEEK_TYPES;
            more.textContent =
                hidden > 0
                    ? `${t("dp.peekMore", { n: hidden })}`
                    : t("dp.peekOpen");
            view.rosterPeek.append(more);
        }
        drawSparkline(view.spark, history, color);
    }
    // A change of house set rebuilds the cards; the open modal follows the same
    // key, so it closes rather than showing a house that is no longer listed.
    if (houseModalName && !keys.includes(houseModalName))
        closeHouseModal();
}
/** How many roster types the card line shows before it counts the rest. */
const HOUSE_PEEK_TYPES = 3;
/** The house the detail modal is showing, or null when it is closed. */
let houseModalName = null;
/**
 * Open the house detail modal. The card is the only entry point, and the
 * content is rebuilt from the snapshot rather than cloned from the card, so
 * the modal can hold what the card has no room for: the whole roster, the
 * power reading and the per-category totals.
 */
function openHouseModal(name) {
    const snapshot = lastSnapshot;
    const house = snapshot?.houses.find((row) => row.name === name);
    const modal = document.getElementById("house-modal");
    if (!snapshot || !house || !modal)
        return;
    houseModalName = name;
    renderHouseModal();
    modal.hidden = false;
}
function closeHouseModal() {
    const modal = document.getElementById("house-modal");
    if (modal)
        modal.hidden = true;
    houseModalName = null;
}
/** Repaint the open modal from the live snapshot. Cheap; runs on every frame
 *  the snapshot arrives, which is what makes the modal a live view rather
 *  than a snapshot of the click. */
function renderHouseModal() {
    if (!houseModalName)
        return;
    const snapshot = lastSnapshot;
    const house = snapshot?.houses.find((row) => row.name === houseModalName);
    if (!snapshot || !house)
        return;
    const color = houseCss(house);
    const dot = document.getElementById("house-modal-dot");
    if (dot) {
        dot.style.background = color;
        dot.style.boxShadow = `0 0 10px ${color}`;
    }
    const title = document.getElementById("house-modal-title");
    if (title)
        title.textContent = house.name;
    const badges = [
        house.is_human
            ? `<span class="tag human">${t("tag.human")}</span>`
            : `<span class="tag ai ${house.difficulty}">${t(`diff.${house.difficulty}`)}</span>`,
    ];
    if (isPassiveHouse(house))
        badges.push(`<span class="tag passive">${t("dp.passive")}</span>`);
    if (house.defeated)
        badges.push(`<span class="tag dead">${t("dp.defeated")}</span>`);
    const tags = document.getElementById("house-modal-tags");
    if (tags)
        tags.innerHTML = badges.join(" ");
    // The economy strip and the composition bar are the same drawings the card
    // makes, so the modal cannot quote a different number than the card does.
    const stats = document.getElementById("house-modal-stats");
    if (stats) {
        const powerState = house.low_power
            ? t("cheat.powerLow")
            : t("cheat.powerOk");
        const cells = [
            [t("dp.credits"), fmt(house.credits, 0)],
            [t("dp.spent"), fmt(house.spent, 0)],
            [t("dp.score"), fmt(house.score, 0)],
            [t("cheat.power"), `${thousands(house.power_output)} / ${thousands(house.power_drain)}`],
            [t("dp.units"), fmt(detailTotal(house.counts), 0)],
            [t("dp.base"), house.base_center ? `${house.base_center[0]}, ${house.base_center[1]}` : "-"],
            [t("dp.side"), sideName(house)],
            [t("hdr.tick"), thousands(snapshot.tick)],
        ];
        stats.innerHTML = cells
            .map(([key, value]) => `<div><span class="k">${key}</span><b>${value}</b></div>`)
            .join("");
        const power = document.getElementById("house-modal-power");
        if (power)
            power.textContent = powerState;
    }
    const total = Math.max(1, detailTotal(house.counts));
    const bar = document.getElementById("house-modal-bar");
    if (bar) {
        bar.textContent = "";
        const segments = [
            ["infantry", DETAIL_CATEGORY_COLOR.Infantry],
            ["unit", DETAIL_CATEGORY_COLOR.Unit],
            ["aircraft", DETAIL_CATEGORY_COLOR.Aircraft],
            ["structure", DETAIL_CATEGORY_COLOR.Structure],
        ];
        for (const [key, css] of segments) {
            if (house.counts[key] === 0)
                continue;
            const seg = document.createElement("span");
            seg.className = "seg";
            seg.style.width = `${(house.counts[key] / total) * 100}%`;
            seg.style.background = css;
            bar.append(seg);
        }
    }
    // What the house is building. A factory that has finished and is waiting for
    // a placement click is the interesting state, so it reads as its own line
    // rather than as a full progress bar that stopped at the end.
    const production = document.getElementById("house-modal-production");
    if (production) {
        production.textContent = "";
        if (house.production.length === 0) {
            production.appendChild(emptyRow(t("dp.noProduction")));
        }
        else {
            for (const line of house.production) {
                const row = document.createElement("div");
                row.className = "prod-row";
                const cat = document.createElement("span");
                cat.className = "prod-cat";
                cat.textContent = t(`prod.${line.category.toLowerCase()}`);
                const name = document.createElement("span");
                name.className = "prod-name";
                name.textContent = line.active_type ?? "—";
                name.title = line.active_type ?? "";
                // One bar per factory, scaled by the engine's own 54-step progress so
                // the reading matches what the sidebar cameo shows.
                const gauge = document.createElement("span");
                gauge.className = "prod-gauge";
                const fill = document.createElement("i");
                fill.style.width = `${Math.min(100, (line.progress / TELEMETRY_PRODUCTION_STEPS) * 100)}%`;
                fill.style.background = color;
                gauge.append(fill);
                const state = document.createElement("span");
                state.className = "prod-state";
                if (line.suspended)
                    state.textContent = t("prod.suspended");
                else if (line.on_hold)
                    state.textContent = t("prod.onHold");
                else if (line.ready_to_place.length > 0)
                    state.textContent = t("prod.ready");
                else if (line.queue.length > 0)
                    state.textContent = t("prod.queued", { n: line.queue.length });
                else
                    state.textContent = "";
                row.append(cat, name, gauge, state);
                if (line.ready_to_place.length > 0) {
                    row.classList.add("ready");
                    const pending = document.createElement("em");
                    pending.className = "prod-pending";
                    pending.textContent = line.ready_to_place.join(" · ");
                    pending.title = line.ready_to_place.join(", ");
                    row.append(pending);
                }
                production.append(row);
            }
        }
    }
    const squads = document.getElementById("house-modal-squads");
    if (squads) {
        squads.textContent = "";
        if (house.squads.length === 0) {
            const empty = document.createElement("li");
            empty.className = "empty";
            empty.textContent = t("dp.noSquads");
            squads.append(empty);
        }
        else {
            for (const squad of house.squads) {
                const head = document.createElement("li");
                head.className = "roster-group";
                const label = squad.team_type ??
                    (squad.task_force ? squad.task_force : t("dp.squadUnnamed", { n: squad.id }));
                // The forming/suspended flag rides at the right of the team's own line
                // and the member count stays with its name: as a row beneath the header
                // the flag read as a separate entry, and after the count it read as
                // part of the number.
                const flag = squad.suspended
                    ? t("prod.suspended")
                    : !squad.formed
                        ? t("dp.squadForming")
                        : "";
                head.innerHTML =
                    `<span>${label}</span>` +
                        `<em>${squad.members}</em>` +
                        (flag ? `<i class="squad-flag">${flag}</i>` : "");
                squads.append(head);
                for (const entry of squad.composition) {
                    const row = document.createElement("li");
                    const count = document.createElement("span");
                    count.className = "cnt";
                    count.textContent = fmt(entry.count, 0);
                    const type = document.createElement("span");
                    type.className = "tname";
                    type.textContent = entry.type_name;
                    type.title = entry.type_name;
                    const category = document.createElement("span");
                    category.className = "cat";
                    category.style.color = DETAIL_CATEGORY_COLOR[entry.category] ?? "var(--dim)";
                    category.textContent = t(detailCategoryKey(entry.category));
                    row.append(count, type, category);
                    squads.append(row);
                }
            }
        }
    }
    const basePlan = document.getElementById("house-modal-baseplan");
    if (basePlan) {
        basePlan.textContent = "";
        const outstanding = house.base_plan.filter((node) => node.is_base);
        if (house.base_plan.length === 0) {
            basePlan.appendChild(emptyRow(t("dp.noBasePlan")));
        }
        else {
            const note = document.createElement("p");
            note.className = "note baseplan-note";
            // The engine's node carries a BuildingTypes registry index, not a name,
            // so the panel reports how much of the plan is outstanding and where —
            // and says so rather than inventing building names.
            note.textContent = t("dp.basePlanCount", {
                n: outstanding.length,
                m: house.base_plan.length,
            });
            basePlan.append(note);
            const wrap = document.createElement("div");
            wrap.className = "baseplan-cells";
            for (const node of house.base_plan) {
                const cell = document.createElement("span");
                cell.className = node.is_base ? "cell pending" : "cell";
                cell.textContent = `${node.x},${node.y}`;
                cell.title = node.is_base ? t("dp.basePlanPending") : t("dp.basePlanDone");
                wrap.append(cell);
            }
            basePlan.append(wrap);
        }
    }
    // The whole roster, grouped by category so a long list reads as four short
    // blocks instead of one sorted column. The engine caps the list at 16 types;
    // saying so here is better than letting a reader assume 16 is the whole
    // army.
    const list = document.getElementById("house-modal-roster");
    if (list) {
        list.textContent = "";
        if (house.roster.length === 0) {
            const empty = document.createElement("li");
            empty.className = "empty";
            empty.textContent = t("dp.none");
            list.append(empty);
        }
        else {
            const groups = new Map();
            for (const entry of house.roster) {
                const bucket = groups.get(entry.category);
                if (bucket)
                    bucket.push(entry);
                else
                    groups.set(entry.category, [entry]);
            }
            for (const [category, entries] of groups) {
                const head = document.createElement("li");
                head.className = "roster-group";
                head.innerHTML =
                    `<i class="cat-dot" style="background:${DETAIL_CATEGORY_COLOR[category] ?? DETAIL_ORPHAN_COLOR}"></i>` +
                        `<span>${t(detailCategoryKey(category))}</span>` +
                        `<em>${entries.reduce((sum, entry) => sum + entry.count, 0)}</em>`;
                list.append(head);
                for (const entry of entries) {
                    const row = document.createElement("li");
                    const count = document.createElement("span");
                    count.className = "cnt";
                    count.textContent = fmt(entry.count, 0);
                    const type = document.createElement("span");
                    type.className = "tname";
                    type.textContent = entry.type_name;
                    type.title = entry.type_name;
                    row.append(count, type);
                    list.append(row);
                }
            }
        }
        const note = document.getElementById("house-modal-note");
        if (note) {
            const types = house.roster.length;
            note.textContent =
                types >= TELEMETRY_ROSTER_CAP
                    ? t("dp.rosterCapped", { n: types })
                    : t("dp.rosterFull", { n: types });
        }
    }
    const spark = document.getElementById("house-modal-spark");
    if (spark) {
        const history = houseHistories.get(house.name)?.credits ?? [];
        drawSparkline(spark, history, color);
    }
}
/** The engine's per-house roster ceiling (`TELEMETRY_MAX_ROSTER`). Mirrored
 *  here because the modal has to say when a house is sitting at the cap. */
const TELEMETRY_ROSTER_CAP = 16;
/** The engine's production steps (`PRODUCTION_STEPS`): a factory's progress
 *  runs 0..=54. Mirrored so the gauge scales the way the sidebar cameo does. */
const TELEMETRY_PRODUCTION_STEPS = 54;
/** A "nothing here" line for the modal's block-level sections. */
function emptyRow(text) {
    const row = document.createElement("p");
    row.className = "empty-row";
    row.textContent = text;
    return row;
}
/** Full detail-wall refresh. Drawing only — `onmessage` owns the sampling,
 *  so a switch-time redraw never injects a phantom history point. */
function renderDetail(snapshot) {
    drawEconomyChart(snapshot);
    renderDetailCards(snapshot);
    // An open house modal is a live view, not a picture of the click: it repaints
    // from the same snapshot the cards just drew.
    renderHouseModal();
}
/** Header tabs: show one wall, hide the others. A switch into the detail view
 *  redraws immediately — its canvases were zero-sized while hidden. A switch
 *  into the report re-reads the sidecar, in case a newer match just ended. */
function setupViews() {
    const show = (view) => {
        currentView = view;
        document.body.dataset.view = view;
        document.getElementById("view-wall")?.classList.toggle("on", view === "wall");
        document.getElementById("view-detail")?.classList.toggle("on", view === "detail");
        document.getElementById("view-report")?.classList.toggle("on", view === "report");
        document.getElementById("view-logs")?.classList.toggle("on", view === "logs");
        if (view === "detail" && lastSnapshot)
            renderDetail(lastSnapshot);
        if (view === "report")
            renderReport();
        if (view === "logs")
            rebuildLogView();
    };
    document.getElementById("view-wall")?.addEventListener("click", () => show("wall"));
    document.getElementById("view-detail")?.addEventListener("click", () => show("detail"));
    document.getElementById("view-report")?.addEventListener("click", () => show("report"));
    document.getElementById("view-logs")?.addEventListener("click", () => show("logs"));
    bindCheats();
    document.getElementById("log-pause")?.addEventListener("click", toggleLogPause);
    document.getElementById("log-clear")?.addEventListener("click", clearLogs);
    document.getElementById("log-follow")?.addEventListener("change", (event) => {
        logFollow = event.target.checked;
        if (logFollow)
            rebuildLogView();
    });
    document.getElementById("log-level")?.addEventListener("change", (event) => {
        logLevel = Number(event.target.value) || 3;
        rebuildLogView();
    });
    const logSearch = document.getElementById("log-search");
    if (logSearch) {
        logSearch.placeholder = t("log.search");
        logSearch.addEventListener("input", () => {
            if (logSearchTimer !== null)
                window.clearTimeout(logSearchTimer);
            logSearchTimer = window.setTimeout(() => {
                logSearchTimer = null;
                logQuery = logSearch.value.trim().toLowerCase();
                rebuildLogView();
            }, 160);
        });
    }
    document.getElementById("make-report")?.addEventListener("click", () => openShotPreview("report"));
    document.getElementById("export-phases")?.addEventListener("click", () => openShotPreview("ledger"));
    // The map already answers the wheel and a drag; these exist because a trackpad
    // user has no way to discover how to get back to the framing it started in.
    document
        .getElementById("map-zoom-in")
        ?.addEventListener("click", () => ensureMap3D()?.zoomBy(1.25));
    document
        .getElementById("map-zoom-out")
        ?.addEventListener("click", () => ensureMap3D()?.zoomBy(1 / 1.25));
    document.getElementById("map-zoom-reset")?.addEventListener("click", () => ensureMap3D()?.resetView());
    document.getElementById("shot-close")?.addEventListener("click", closeShot);
    document.getElementById("shot-copy")?.addEventListener("click", () => void copyShot());
    document.getElementById("shot-save")?.addEventListener("click", saveShot);
    document.getElementById("shot-html")?.addEventListener("click", saveReportHtml);
    // Clicking the dimmed backdrop closes the preview; clicking its card does not.
    document.getElementById("shot-modal")?.addEventListener("click", (event) => {
        if (event.target === event.currentTarget)
            closeShot();
    });
    document.getElementById("house-modal-close")?.addEventListener("click", closeHouseModal);
    document.getElementById("house-modal")?.addEventListener("click", (event) => {
        if (event.target === event.currentTarget)
            closeHouseModal();
    });
    window.addEventListener("keydown", (event) => {
        if (event.key !== "Escape")
            return;
        // Both dialogs are modal, so the topmost one is the only one Escape
        // should close — otherwise Escape behind the house detail would also drop
        // the screenshot preview underneath it.
        if (houseModalName)
            closeHouseModal();
        else
            closeShot();
    });
}
// ------------------------------------------------------ match report wall
/**
 * The report wall: the match being played right now, not a file the engine
 * freezes at the end. It draws the same split the main wall's frame-occupancy
 * panel draws — built by the same `buildFrameSlices`, so the two can never
 * disagree — at report size, and pairs it with the house table so the numbers
 * can be read against who is on the map while they are being measured.
 */
// ------------------------------------------------------------ log console
/** Rows kept in memory and in the DOM. The engine's ring holds more, but this
 *  is what the browser renders without stuttering. */
const LOG_LIMIT = 3000;
const LOG_LEVEL_NAMES = ["", "ERR", "WRN", "INF", "DBG", "TRC"];
let logLines = [];
let logPaused = false;
let logFollow = true;
let logLevel = 3;
let logQuery = "";
let logDropped = 0;
let logSearchTimer = null;
function logVisible(line) {
    if (line.level > logLevel)
        return false;
    if (logQuery.length === 0)
        return true;
    return (line.message.toLowerCase().includes(logQuery) ||
        line.target.toLowerCase().includes(logQuery));
}
function logRow(line) {
    const row = document.createElement("div");
    row.className = `log-row lv${line.level}`;
    const time = document.createElement("span");
    time.className = "t";
    time.textContent = `${(line.t_ms / 1000).toFixed(3)}s`;
    const level = document.createElement("span");
    level.className = "lv";
    level.textContent = LOG_LEVEL_NAMES[line.level] ?? "?";
    const target = document.createElement("span");
    target.className = "tg";
    target.textContent = line.target;
    target.title = line.target;
    const message = document.createElement("span");
    message.className = "msg";
    message.textContent = line.message;
    message.title = line.message;
    row.append(time, level, target, message);
    return row;
}
function updateLogCount() {
    const count = document.getElementById("log-count");
    if (!count)
        return;
    const list = document.getElementById("log-list");
    count.textContent = t("log.count", {
        shown: thousands(list ? list.childElementCount : 0),
        total: thousands(logLines.length),
    });
}
function appendLogRows(lines) {
    const list = document.getElementById("log-list");
    if (!list)
        return;
    const fragment = document.createDocumentFragment();
    for (const line of lines) {
        if (logVisible(line))
            fragment.append(logRow(line));
    }
    if (fragment.childElementCount === 0)
        return;
    list.append(fragment);
    // Trim from the top: the tail is what a reader is watching.
    while (list.childElementCount > LOG_LIMIT)
        list.firstElementChild?.remove();
    if (logFollow)
        list.scrollTop = list.scrollHeight;
    updateLogCount();
}
/** Redraw every visible row from the buffer: filter changes, view entry, resume. */
function rebuildLogView() {
    const list = document.getElementById("log-list");
    if (!list)
        return;
    const fragment = document.createDocumentFragment();
    for (const line of logLines) {
        if (logVisible(line))
            fragment.append(logRow(line));
    }
    list.replaceChildren(fragment);
    if (logFollow)
        list.scrollTop = list.scrollHeight;
    updateLogCount();
}
/** Take one snapshot's worth of records. Paused only stops the drawing; the
 *  buffer keeps filling so resuming shows what arrived meanwhile. */
function acceptLogs(batch) {
    if (!batch)
        return;
    if (batch.dropped > 0) {
        logDropped += batch.dropped;
        const marker = document.getElementById("log-dropped");
        if (marker)
            marker.textContent = t("log.dropped", { n: thousands(logDropped) });
    }
    if (batch.lines.length === 0)
        return;
    logLines.push(...batch.lines);
    if (logLines.length > LOG_LIMIT) {
        logLines.splice(0, logLines.length - LOG_LIMIT);
    }
    if (!logPaused && currentView === "logs")
        appendLogRows(batch.lines);
    else
        updateLogCount();
}
function toggleLogPause() {
    logPaused = !logPaused;
    const button = document.getElementById("log-pause");
    if (button)
        button.textContent = t(logPaused ? "log.resume" : "log.pause");
    if (!logPaused)
        rebuildLogView();
}
function clearLogs() {
    logLines = [];
    logDropped = 0;
    const marker = document.getElementById("log-dropped");
    if (marker)
        marker.textContent = "";
    rebuildLogView();
}
// ------------------------------------------------------------ trainer panel
/** The switches the panel owns, in the order the engine names them. */
const CHEAT_SWITCHES = [
    "infinite_money",
    "infinite_power",
    "instant_build",
    "build_anywhere",
    "invincible",
    "reveal_map",
    "tech_all",
    "tech_boost",
];
function cheatInput(selector) {
    return document.querySelector(selector);
}
function cheatChecked(name) {
    return cheatInput(`input[data-cheat="${name}"]`)?.checked ?? false;
}
/**
 * Read the panel. Built from the DOM, not a cached object: the engine echoes
 * its own values back on every snapshot, and a cached copy would send stale
 * numbers the moment one field was clamped on arrival.
 */
function currentCheatFlags() {
    const money = Number(cheatInput("#cheat-money")?.value ?? 0);
    const damage = Number(cheatInput("#cheat-damage")?.value ?? 1);
    const flags = {};
    for (const name of CHEAT_SWITCHES)
        flags[name] = cheatChecked(name);
    flags.money_target = Math.max(0, Math.min(9_999_999, Math.round(money) || 0));
    flags.damage_scale = Math.max(1, Math.min(65_535, Math.round(damage) || 1));
    return flags;
}
function sendCheats() {
    sendCommand({ cmd: "set_cheats", flags: currentCheatFlags() });
    markCheatChip();
}
/** Highlight the preset that matches the current multiplier. */
function markCheatChip() {
    const value = Number(cheatInput("#cheat-damage")?.value ?? 1);
    document.querySelectorAll(".cheat-chip").forEach((chip) => {
        chip.classList.toggle("on", Number(chip.dataset.scale) === value);
    });
}
/** Take the engine's values (authoritative) and show them. */
function syncCheats(report) {
    if (!report)
        return;
    for (const name of CHEAT_SWITCHES) {
        const input = cheatInput(`input[data-cheat="${name}"]`);
        if (input)
            input.checked = report[name];
    }
    const money = cheatInput("#cheat-money");
    // A zero floor is the engine's "unset"; keeping the panel's default avoids
    // showing a field the user never typed into.
    if (money && document.activeElement !== money && report.money_target > 0) {
        money.value = String(report.money_target);
    }
    const damage = cheatInput("#cheat-damage");
    if (damage && document.activeElement !== damage) {
        // The engine stores 0 for "never set"; the panel speaks 1 = off, because
        // "damage ×0" reads like it disables the weapons.
        damage.value = String(Math.max(1, report.damage_scale));
    }
    const owner = document.getElementById("cheat-owner");
    if (owner) {
        if (!report.owner) {
            owner.textContent = "";
        }
        else {
            // Show the wallet beside the house: the money switch's effect is the
            // thing a reader checks first, and this saves a trip to the wall.
            const house = lastSnapshot?.houses.find((row) => row.name === report.owner);
            const wallet = house ? ` · ${t("cheat.wallet")} ${thousands(house.credits)}` : "";
            owner.textContent = `${t("cheat.owner")} ${report.owner}${wallet}`;
        }
        // The power reading is the only way to tell "the switch is doing its job"
        // from "the base was already powered and there was nothing to do".
        const power = document.getElementById("cheat-power");
        if (power) {
            const house = lastSnapshot?.houses.find((row) => row.name === report.owner);
            if (!house) {
                power.textContent = "";
            }
            else {
                const state = report.power_forced
                    ? t("cheat.powerForced")
                    : house.low_power
                        ? t("cheat.powerLow")
                        : t("cheat.powerOk");
                power.textContent =
                    `${t("cheat.power")} ${thousands(house.power_output)} / ${thousands(house.power_drain)}` +
                        ` · ${state}`;
            }
        }
        // An empty line above the cards reads as a rendering fault.
        owner.hidden = !report.owner;
    }
    markCheatChip();
}
function bindCheats() {
    for (const name of CHEAT_SWITCHES) {
        cheatInput(`input[data-cheat="${name}"]`)?.addEventListener("change", sendCheats);
    }
    // `change`, not `input`: a number field fires per keystroke and every send
    // re-asserts every switch.
    cheatInput("#cheat-money")?.addEventListener("change", sendCheats);
    cheatInput("#cheat-damage")?.addEventListener("change", sendCheats);
    document.querySelectorAll(".cheat-chip").forEach((chip) => {
        chip.addEventListener("click", () => {
            const damage = cheatInput("#cheat-damage");
            if (damage)
                damage.value = chip.dataset.scale ?? "1";
            sendCheats();
        });
    });
    const act = (id, action) => {
        document
            .getElementById(id)
            ?.addEventListener("click", () => sendCommand({ cmd: "cheat_action", action }));
    };
    act("cheat-heal", "heal_all");
    act("cheat-veteran", "veteran_all");
    act("cheat-supers", "superweapons_ready");
    document.getElementById("cheat-reset")?.addEventListener("click", () => {
        for (const name of CHEAT_SWITCHES) {
            const input = cheatInput(`input[data-cheat="${name}"]`);
            if (input)
                input.checked = false;
        }
        const damage = cheatInput("#cheat-damage");
        if (damage)
            damage.value = "1";
        sendCheats();
    });
}
/** The stream arrives ~11×/s. Rebuilding a forty-row table that often reads as
 *  flicker rather than as live data, so the wall redraws at most four times a
 *  second and coalesces whatever arrived in between. */
const REPORT_RENDER_INTERVAL_MS = 250;
let reportRenderedAt = 0;
let reportRenderTimer = null;
function renderReportSoon() {
    const wait = REPORT_RENDER_INTERVAL_MS - (performance.now() - reportRenderedAt);
    if (wait <= 0) {
        reportRenderedAt = performance.now();
        renderReport();
        return;
    }
    if (reportRenderTimer !== null)
        return;
    reportRenderTimer = window.setTimeout(() => {
        reportRenderTimer = null;
        reportRenderedAt = performance.now();
        renderReport();
    }, wait);
}
function renderReport() {
    const facts = document.getElementById("report-facts");
    const table = document.getElementById("report-houses");
    const foot = document.getElementById("report-foot");
    if (!facts || !table || !foot)
        return;
    const snapshot = lastSnapshot;
    const profile = snapshot?.profile ?? null;
    const headLine = document.getElementById("report-head");
    const note = document.getElementById("report-note");
    const housesPanel = table.closest("section");
    const phasesTable = document.getElementById("report-phases");
    const phasesPanel = phasesTable?.closest("section");
    if (!snapshot || !profile || profile.frames === 0) {
        drawPie("rep-pie-frame", "rep-frame-legend", [], 0);
        drawPie("rep-pie-sim", "rep-sim-legend", [], 0);
        document.getElementById("rep-frame-total").textContent = "-";
        document.getElementById("rep-sim-total").textContent = "-";
        facts.innerHTML = `<div class='row'><span>${t("rep.waiting")}</span></div>`;
        table.textContent = "";
        foot.textContent = "";
        if (phasesTable)
            phasesTable.textContent = "";
        if (headLine)
            headLine.textContent = "";
        if (note)
            note.textContent = "";
        // Nothing to list yet: an empty table panel reads as a fault, so hide it.
        if (housesPanel)
            housesPanel.hidden = true;
        if (phasesPanel)
            phasesPanel.hidden = true;
        document.getElementById("report")?.classList.add("only-pies");
        return;
    }
    if (housesPanel)
        housesPanel.hidden = false;
    if (phasesPanel)
        phasesPanel.hidden = false;
    document.getElementById("report")?.classList.remove("only-pies");
    const frameMs = snapshot.timing ? 1000 / Math.max(1, snapshot.timing.fps) : 0;
    const { sim: simSlices, frame: frameSlices, simTotal, frameTotal } = buildFrameSlices(profile, frameMs);
    drawPie("rep-pie-frame", "rep-frame-legend", frameSlices, frameTotal);
    drawPie("rep-pie-sim", "rep-sim-legend", simSlices, simTotal);
    document.getElementById("rep-frame-total").textContent = `${fmt(frameTotal, 1)} ms`;
    document.getElementById("rep-sim-total").textContent = `${fmt(simTotal, 1)} ms`;
    const share = frameTotal > 0 ? (simTotal / frameTotal) * 100 : 0;
    const rows = [
        [t("hdr.map"), `${snapshot.map_width}x${snapshot.map_height}`],
        [t("rep.frame"), `${fmt(frameMs, 2)} ms`],
        [t("rep.tickP50"), `${fmt(snapshot.timing?.p50_ms ?? 0, 2)} ms`],
        [t("rep.sim"), `${fmt(profile.mean_frame_ms, 2)} ms`],
        [t("rep.peak"), `${fmt(profile.peak_frame_ms, 2)} ms`],
        [t("rep.share"), `${fmt(share, 0)} %`],
    ];
    facts.innerHTML = rows
        .map(([key, value]) => `<div class='row'><span>${key}</span><b>${value}</b></div>`)
        .join("");
    renderPhaseTable(phasesTable, profile, snapshot.render ?? null, simTotal, frameTotal);
    const head = `<tr><th>${t("th.house")}</th><th>${t("rep.kind")}</th>` +
        `<th>${t("rep.buildings")}</th><th>${t("rep.credits")}</th><th>${t("rep.score")}</th></tr>`;
    table.innerHTML =
        head +
            snapshot.houses
                // `Neutral` and `Special` hold the map's own scenery and civilians; they
                // are not playing, so they have no place in the lineup.
                .filter((house) => !house.passive)
                .map((house) => `<tr><td>${escapeHtml(house.name)}</td>` +
                `<td>${house.is_human ? t("tag.human") : t("tag.ai")}</td>` +
                `<td>${fmt(house.counts.structure, 0)}</td>` +
                `<td>${fmt(house.credits, 0)}</td>` +
                `<td>${fmt(house.score, 0)}</td></tr>`)
                .join("");
    // One line of whole-match facts above the pies: how far the match has run,
    // how fast ticks are actually committing, and what a frame costs.
    const tickHz = snapshot.timing?.tick_hz ?? 0;
    const fps = snapshot.timing?.fps ?? 0;
    if (headLine) {
        // Scale and rates only: the cost figures each appear once, in the facts
        // row under the pies.
        const parts = [
            `tick <b>${thousands(snapshot.tick)}</b>`,
            `${t("rep.duration")} <b>${tickHz > 0 ? thousands(snapshot.tick / tickHz) : "-"}</b> s`,
            // Both rates side by side: the render loop's and the committed tick's.
            `${t("rep.fps")} <b>${fmt(fps, 0)}</b> fps`,
            `<b>${fmt(tickHz, 1)}</b> tick/s`,
        ];
        headLine.innerHTML = parts.join(`<span class="sep">·</span>`);
    }
    // The render wedge is nothing but that difference; saying so saves the
    // reader from subtracting two numbers off two different pies.
    if (note) {
        const renderMs = Math.max(0, frameMs - profile.mean_frame_ms);
        note.innerHTML = `${t("rep.note")} ≈ <b>${fmt(renderMs, 1)}</b> ms ${t("rep.noteTail")}`;
    }
    foot.textContent = t("rep.source");
}
// ---------------------------------------------------------------- socket
let statusOpen = false;
let statusOverride = null;
let everConnected = false;
function renderStatus() {
    document.getElementById("dot").classList.toggle("open", statusOpen);
    document.getElementById("statustext").textContent =
        statusOverride ??
            (statusOpen
                ? t("status.live")
                : everConnected
                    ? t("status.reconnecting")
                    : t("status.connecting"));
}
function setStatus(open, text) {
    statusOpen = open;
    if (open)
        everConnected = true;
    statusOverride = text ?? null;
    renderStatus();
}
function connect() {
    const next = new WebSocket(`ws://${location.host}/ws`);
    socket = next;
    next.onopen = () => setStatus(true);
    next.onclose = () => {
        setStatus(false);
        window.setTimeout(connect, 1000);
    };
    next.onerror = () => next.close();
    next.onmessage = (event) => {
        let snapshot;
        try {
            snapshot = JSON.parse(event.data);
        }
        catch {
            return;
        }
        if (snapshot.version !== SCHEMA_VERSION) {
            setStatus(true, t("status.unsupported", { v: snapshot.version }));
            return;
        }
        lastSnapshot = snapshot;
        acceptLogs(snapshot.logs);
        syncCheats(snapshot.cheats);
        setStatus(true);
        // The detail view's history must keep flowing while the data wall shows,
        // or its charts restart at every switch.
        sampleHouseEconomy(snapshot);
        if (currentView === "detail")
            renderDetailCards(snapshot);
        // The modal repaints on its own, not through `renderDetail`: that path only
        // runs on a view switch or a resize, which froze the production gauges and
        // the squad list until the modal was reopened. It self-guards on an open
        // modal, so this costs nothing while none is showing.
        renderHouseModal();
        ensureMap3D()?.draw(snapshot);
        drawMapLegend(snapshot);
        if (snapshot.timing) {
            chartSamples.push({ tick: snapshot.timing.tick_ms, p99: snapshot.timing.p99_ms });
            if (chartSamples.length > CHART_WINDOW)
                chartSamples.shift();
        }
        updateHeader(snapshot);
        updateCost(snapshot);
        updateProcess(snapshot.process);
        updateTerrain(snapshot.debug);
        renderSwitches(snapshot.debug);
        updateEvents(snapshot.debug);
        updatePopulation(snapshot);
        updateFrameProfile(snapshot.profile, snapshot.timing ? 1000 / Math.max(1, snapshot.timing.fps) : 0);
        drawChart(snapshot.timing);
        // Live snapshots, so the report wall just rides the stream: no polling,
        // and no waiting for a match to end before it has anything to show.
        if (currentView === "report")
            renderReportSoon();
    };
}
// ------------------------------------------------------------------ menu
/** The debug menu (switches + hotkeys) is a drawer, not dashboard content:
 *  the main view stays data-only. Esc, the backdrop and the close button all
 *  dismiss it. */
function setupMenus() {
    const showDebug = bindDrawer("menu", "menu-open", "menu-close");
    const showTrainer = bindDrawer("cheat-menu", "cheats-open", "cheats-close", () => syncCheats(lastSnapshot?.cheats));
    // Two drawers on one edge: opening one closes the other, so the header
    // buttons never leave a stack of panels behind each other.
    document.getElementById("menu-open")?.addEventListener("click", () => showTrainer(false));
    document.getElementById("cheats-open")?.addEventListener("click", () => showDebug(false));
}
/**
 * Wire one right-edge drawer. Returns its `show`, so callers can close it from
 * outside; `onOpen` refreshes state that may have moved since last time.
 */
function bindDrawer(id, openId, closeId, onOpen) {
    const drawer = document.getElementById(id);
    if (!drawer)
        return () => { };
    const show = (visible) => {
        if (visible)
            onOpen?.();
        drawer.classList.toggle("open", visible);
        drawer.setAttribute("aria-hidden", String(!visible));
    };
    document.getElementById(openId)?.addEventListener("click", () => show(true));
    document.getElementById(closeId)?.addEventListener("click", () => show(false));
    drawer.addEventListener("click", (event) => {
        // Only the backdrop itself (not the panel) dismisses.
        if (event.target === drawer)
            show(false);
    });
    window.addEventListener("keydown", (event) => {
        if (event.key === "Escape")
            show(false);
    });
    return show;
}
// ------------------------------------------------------------ bootstrap
applyStaticTranslations();
syncSwitcher();
renderStatus();
setupMenus();
setupViews();
setupPager();
// The switches exist (disabled) before the first snapshot arrives.
renderSwitches(null);
for (const button of document.querySelectorAll("#lang [data-locale]")) {
    button.addEventListener("click", () => {
        setLocale(button.dataset.locale);
    });
}
// A language switch re-renders every dynamic panel from the latest snapshot;
// the 3D scene is language-free and keeps running.
onLocaleChange(() => {
    syncSwitcher();
    renderStatus();
    renderSwitches(lastSnapshot?.debug ?? null);
    // The detail cards bake labels into their DOM; force a structural rebuild.
    detailCardKeys = [];
    if (currentView === "detail" && lastSnapshot)
        renderDetailCards(lastSnapshot);
    renderHouseModal();
    if (currentView === "report")
        renderReportSoon();
    if (!lastSnapshot)
        return;
    updateTerrain(lastSnapshot.debug);
    updateEvents(lastSnapshot.debug);
    updatePopulation(lastSnapshot);
    updateFrameProfile(lastSnapshot.profile, lastSnapshot.timing ? 1000 / Math.max(1, lastSnapshot.timing.fps) : 0);
    drawChart(lastSnapshot.timing);
});
window.addEventListener("resize", () => {
    if (currentView === "detail" && lastSnapshot)
        renderDetail(lastSnapshot);
});
connect();
/** Read-only handle for the screenshot harness: headless browsers refuse
 *  downloads, so this is the only way to assert on the generated report. It
 *  exposes no state the panel does not already render. */
window.veraReport = () => buildReportHtml();
