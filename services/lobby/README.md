# Online lobby service

The Cloudflare Worker serves `protocol.md`. Its SQLite Durable Object owns player
accounts, authenticated sessions, room membership, settings revisions, readiness
and start agreement. The relay and clients own UDP connectivity and simulation.

```sh
npm ci
npm test
cf deploy --dry-run
```

For deployment, use the authenticated `cf` profile and set
`VERA20K_RELAY_ADDRESS` to the public relay IPv4 address and port. Keep a temporary
mode-0600 JSON file outside the repository containing `RELAY_TICKET_KEY`, then run
`cf deploy --secrets-file /private/path/to/secrets.json`. The key must match the
relay's systemd credential. The CLI's authentication stays in its own credential
storage. No provider token or server key belongs in the build or client.

The client defaults to the deployed lobby at
`https://vera20k-lobby.hnr-enok.workers.dev`. Normal players use
**Internet → Custom Match** without configuring a server address; see the
[player instructions](../../README.md#multiplayer).

For a separate deployment, set `VERA20K_LOBBY_URL` at build time or use its runtime
environment override for staging. Clients require HTTPS, verify TLS and decline
redirects. The client keeps login tokens in memory and does not save passwords.
The lobby stores account password verifiers and session records in SQLite.

The source schema is configured in `cloudflare.config.ts` using the official
[Cloudflare configuration API](https://developers.cloudflare.com/cf/projects/cloudflare-config/).
Deployments preserve the existing SQLite data. Health is exposed at `/health`;
credentials and request bodies are excluded from application logging.

Protocol 2 is a coordinated client/lobby/relay upgrade. Deploy the v2 relay and
matching clients with this service; v1 clients and stored v1 sessions receive
`upgrade_required`. Initial grants last 120 seconds and playing grants 300 seconds.
Each account has one monotonically ordered relay admission, so cancelled starts
cannot retain a pool of valid rooms. Payload encryption keys are delivered only
to the match participants over HTTPS and are never sent to the relay.
The HTTPS lobby is trusted with these keys. Relay-blind encryption does not
protect against a compromised lobby or participant.
