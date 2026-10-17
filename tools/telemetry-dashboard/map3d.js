/**
 * 3D battlefield overview (three.js).
 *
 * Layout decisions carried over from the verified iteration:
 *  - **Rectangular map.** Map cell (rx, ry) maps straight to world (X, Z) and
 *    elevation to world Y — no isometric projection (the diamond experiment
 *    was rejected; the user wants the game-radar orientation).
 *  - **Radar-aligned camera.** Azimuth pi/2 puts the camera on the +Z side, so
 *    +rx reads right and +ry reads down-screen exactly like the in-game
 *    minimap. Default elevation 0.85 rad is the angle at which the building
 *    layout was confirmed to match the game radar.
 *  - Per-cell grid lines and a rectangular boundary, as the game draws them.
 *
 * Schema mirror of `src/sim/telemetry.rs`; keep both in step.
 */
import * as THREE from "three";
/** InstancedMesh capacity: the project's 20k-unit target. */
const MAX_SPOTS = 20000;
/** One elevation level's visual offset, in world units. */
const LEVEL_STEP = 0.5;
/** Per-category box scale and base height (world units, one cell = 1). */
const CATEGORY_SCALE = {
    Infantry: 0.38,
    Unit: 0.62,
    Aircraft: 0.5,
    Structure: 1.25,
};
const CATEGORY_HEIGHT = {
    Infantry: 0.3,
    Unit: 0.5,
    Aircraft: 0.45,
    Structure: 0.9,
};
/** Fallback palette for houses the engine gave no colour (neutral, orphan). */
const HOUSE_PALETTE = ["#8a94a3", "#b04aff", "#e8c34a", "#37e0b8", "#ff5555", "#4a9eff"];
/** Entities with no house row (owner_index sentinel). */
const ORPHAN_COLOR = "#6b7a8c";
/** `[r, g, b]` (0-255) -> a CSS colour three.js converts from sRGB. */
function houseColor(house, index) {
    if (house?.color) {
        const [r, g, b] = house.color;
        return `rgb(${r},${g},${b})`;
    }
    return HOUSE_PALETTE[index % HOUSE_PALETTE.length];
}
export function createMap3D(canvas) {
    let renderer;
    try {
        renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
    }
    catch {
        // No WebGL (headless preview, blocklisted GPU): the data panels still work.
        return null;
    }
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    // Transparent: the panel's CSS gradient is the sky.
    renderer.setClearColor(0x000000, 0);
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(50, 1, 0.5, 4000);
    scene.add(new THREE.AmbientLight(0xb8c8dd, 1.4));
    const sun = new THREE.DirectionalLight(0xffffff, 1.6);
    sun.position.set(60, 120, 40);
    scene.add(sun);
    const orbit = { azimuth: Math.PI / 2, elevation: 0.85, radius: 200 };
    const orbitTarget = new THREE.Vector3();
    let extent = { w: 64, h: 64 };
    // --- ground: plate, per-cell grid, boundary (built per map extent) -------
    let ground = null;
    function buildGround(w, h) {
        if (ground) {
            scene.remove(ground);
            ground.traverse((object) => {
                if (object instanceof THREE.Mesh || object instanceof THREE.LineSegments) {
                    object.geometry.dispose();
                    object.material.dispose();
                }
            });
        }
        ground = new THREE.Group();
        const plate = new THREE.Mesh(new THREE.PlaneGeometry(w, h), new THREE.MeshBasicMaterial({ color: 0x04121f }));
        plate.rotation.x = -Math.PI / 2;
        plate.position.set(w / 2, -0.02, h / 2);
        ground.add(plate);
        // One line per row/column, the game's Ctrl+L cell grid.
        const points = [];
        for (let i = 0; i <= w; i++)
            points.push(i, 0, 0, i, 0, h);
        for (let j = 0; j <= h; j++)
            points.push(0, 0, j, w, 0, j);
        const grid = new THREE.LineSegments(new THREE.BufferGeometry().setAttribute("position", new THREE.Float32BufferAttribute(points, 3)), new THREE.LineBasicMaterial({ color: 0x0e3a52 }));
        ground.add(grid);
        const corners = [
            [0, 0],
            [w, 0],
            [w, h],
            [0, h],
        ].map(([x, z]) => new THREE.Vector3(x, 0, z));
        const outline = new THREE.LineLoop(new THREE.BufferGeometry().setFromPoints(corners), new THREE.LineBasicMaterial({ color: 0x1f7fa8 }));
        ground.add(outline);
        scene.add(ground);
    }
    // --- entities ------------------------------------------------------------
    const entities = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshLambertMaterial(), MAX_SPOTS);
    entities.count = 0;
    entities.frustumCulled = false;
    scene.add(entities);
    const dummy = new THREE.Object3D();
    const color = new THREE.Color();
    // --- cursor marker (the Terrain Debug panel's cell) ----------------------
    const cursorMarker = new THREE.Mesh(new THREE.BoxGeometry(1.02, 0.06, 1.02), new THREE.MeshBasicMaterial({ color: 0x28f0ff, transparent: true, opacity: 0.9 }));
    cursorMarker.visible = false;
    scene.add(cursorMarker);
    function applyCamera() {
        orbitTarget.set(extent.w / 2, 0, extent.h / 2);
        const r = orbit.radius;
        camera.position.set(orbitTarget.x + r * Math.cos(orbit.azimuth) * Math.cos(orbit.elevation), orbitTarget.y + r * Math.sin(orbit.elevation), orbitTarget.z + r * Math.sin(orbit.azimuth) * Math.cos(orbit.elevation));
        camera.lookAt(orbitTarget);
        scene.fog = new THREE.Fog(0x051524, r * 0.7, r * 2.4);
    }
    function updateSpots(snapshot) {
        const spots = snapshot.spots;
        const count = Math.min(spots.length, MAX_SPOTS);
        // One colour per house row, from the engine's own [Colors] scheme.
        const houseColors = snapshot.houses.map((house, index) => houseColor(house, index));
        for (let i = 0; i < count; i++) {
            const spot = spots[i];
            const scale = CATEGORY_SCALE[spot.category] ?? 0.6;
            // Map (rx, ry) -> world (X, Z); elevation -> Y.
            dummy.position.set(spot.x, spot.z * LEVEL_STEP + CATEGORY_HEIGHT[spot.category] * scale, spot.y);
            dummy.scale.set(scale, scale, scale);
            dummy.rotation.set(0, 0, 0);
            dummy.updateMatrix();
            entities.setMatrixAt(i, dummy.matrix);
            const owner = spot.owner_index;
            color.setStyle(owner === 0xffff ? ORPHAN_COLOR : (houseColors[owner] ?? ORPHAN_COLOR));
            entities.setColorAt(i, color);
        }
        entities.count = count;
        entities.instanceMatrix.needsUpdate = true;
        if (entities.instanceColor)
            entities.instanceColor.needsUpdate = true;
    }
    function updateCursor(debug) {
        const cell = debug?.cursor_cell;
        if (!cell) {
            cursorMarker.visible = false;
            return;
        }
        cursorMarker.visible = true;
        cursorMarker.position.set(cell[0] + 0.5, 0.05, cell[1] + 0.5);
    }
    // --- orbit: drag rotate, wheel zoom --------------------------------------
    function bindOrbit() {
        let dragging = false;
        let lastX = 0;
        let lastY = 0;
        canvas.addEventListener("pointerdown", (event) => {
            dragging = true;
            lastX = event.clientX;
            lastY = event.clientY;
            canvas.setPointerCapture(event.pointerId);
        });
        canvas.addEventListener("pointerup", (event) => {
            dragging = false;
            canvas.releasePointerCapture(event.pointerId);
        });
        canvas.addEventListener("pointermove", (event) => {
            if (!dragging)
                return;
            orbit.azimuth -= (event.clientX - lastX) * 0.008;
            orbit.elevation = Math.min(1.45, Math.max(0.12, orbit.elevation + (event.clientY - lastY) * 0.006));
            lastX = event.clientX;
            lastY = event.clientY;
            applyCamera();
        });
        canvas.addEventListener("wheel", (event) => {
            event.preventDefault();
            orbit.radius = Math.min(600, Math.max(24, orbit.radius * (1 + Math.sign(event.deltaY) * 0.12)));
            applyCamera();
        }, { passive: false });
    }
    function resize() {
        const width = canvas.clientWidth;
        const height = canvas.clientHeight;
        if (width === 0 || height === 0)
            return;
        renderer.setSize(width, height, false);
        camera.aspect = width / height;
        camera.updateProjectionMatrix();
    }
    function render() {
        requestAnimationFrame(render);
        renderer.render(scene, camera);
    }
    buildGround(extent.w, extent.h);
    orbit.radius = (extent.w + extent.h) * 0.68;
    applyCamera();
    bindOrbit();
    new ResizeObserver(resize).observe(canvas.parentElement);
    resize();
    render();
    return {
        draw(snapshot) {
            const w = snapshot.map_width || 64;
            const h = snapshot.map_height || 64;
            if (w !== extent.w || h !== extent.h) {
                extent = { w, h };
                buildGround(w, h);
                orbit.radius = (w + h) * 0.68;
                applyCamera();
            }
            updateSpots(snapshot);
            updateCursor(snapshot.debug);
        },
        dispose() {
            renderer.dispose();
        },
    };
}
