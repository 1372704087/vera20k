# Read-only telemetry dashboard

The dashboard observes a running VERA20k match: populations, production, AI
teams, map positions, selected paths, frame timing and process logs. The dashboard
design and initial implementation were contributed by
[1372704087](https://github.com/1372704087) in
[PR #2083](https://github.com/YuriPlanet/vera20k/pull/2083).

## Run

From the checkout root, with the ordinary retail data setup:

```sh
VERA_TELEMETRY_ADDR=127.0.0.1:9741 python -m tools.cargo_run -- run --profile fast --features telemetry-dashboard --bin vera20k
```

Open the local URL printed to stderr. Its fragment contains a fresh session
credential. The page consumes and removes that fragment; reloading needs the
original printed URL again. Treat the URL as access to the current match and its
logs. The server requires the matching local Host, Origin and credential, serves
only fixed dashboard assets, and accepts no game commands. Non-loopback binding
is rejected. Close the game to revoke the session.

The feature is off by default and absent from ordinary release builds. Without
`VERA_TELEMETRY_ADDR`, it starts no server. `VERA_TELEMETRY_WEB` can select the
dashboard asset directory when running outside the checkout root. CPU timing and
snapshot capture run only while an authenticated dashboard client is attached.
The existing `dev-overlays` feature and build profile are independent choices;
an optimized build can include these diagnostics without a separate debug branch.

For a synthetic demonstration without retail data:

```sh
python -m tools.cargo_run -- run --profile fast --features telemetry-dashboard --bin telemetry-serve -- --ticks 300
```

This fixture animates artificial entities. Its timing is not a gameplay benchmark.

## Limits and interpretation

Snapshots update at most ten times per second, including while paused. Population
totals cover the complete world; displayed detail is capped at 32 houses, 20,000
map objects, 16 roster types per house, 8 production lines, 16 AI teams and 32 base
nodes per house. Selected-object and cursor details each show at most 32 objects.
Names, log records, connections and network buffers are bounded. A slow client
may skip snapshots. Logs are a shared incremental stream; new tabs receive future
batches, not a complete historical replay.

Timing describes the current instrumented run and includes instrumentation cost.
Some frame work remains grouped under its existing owner rather than split into
new hooks. CPU share is normalized to all logical cores. The GPU memory field
counts the engine's palette-index atlas pages, not total driver allocation.
PNG and HTML exports are generated locally in the browser. No external services,
trainer, remote pause/step controls or gameplay mutations are included.

The original trainer and additional hooks are preserved on
[codex/pr-2083-original](https://github.com/YuriPlanet/vera20k/tree/codex/pr-2083-original)
for a separately reviewed, opt-in VERA20k debug build. They need explicit state,
replay and multiplayer rules before integration.

## Frontend changes

```sh
cd tools/telemetry-dashboard
npm ci --ignore-scripts --no-audit --no-fund
npm run build
```

Commit the TypeScript and generated JavaScript together. The manual Dashboard
workflow checks they agree. Vendored Three.js r180 is unchanged upstream code;
its license is in `vendor/THREE-LICENSE.txt`.
