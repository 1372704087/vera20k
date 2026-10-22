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

Compile the game with `VERA20K_LOBBY_URL` set to the resulting HTTPS endpoint.
Clients also accept that environment variable for staging. They use verified TLS
and decline redirects, keep login sessions in memory, and never persist passwords.

The source schema is configured in `cloudflare.config.ts` using the official
[Cloudflare configuration API](https://developers.cloudflare.com/cf/projects/cloudflare-config/).
Deployments preserve the existing SQLite data. Health is exposed at `/health`;
credentials and request bodies are excluded from application logging.
