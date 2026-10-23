# Gameplay relay

The relay forwards authenticated datagrams between VERA clients. It owns neither
simulation nor lobby membership. Clients retain reliable delivery and lockstep.
The shared codec in `../relay-protocol` binds each ticket to a match and player,
authenticates packet contents, rejects replay, and challenges a new source address
before admitting it. A new match requires new tickets.

Build from the repository root:

```sh
python -m tools.cargo_run -- build --release -p vera20k-relay
```

Install the executable as `/usr/local/bin/vera20k-relay`, create the unprivileged
`vera20k-relay` user, and install `vera20k-relay.service`. Allow UDP port 5000 in
the cloud firewall. Limit administrative SSH access to the management network.

Generate a 32-byte cryptographic random key outside the repository and encode it
as 64 hexadecimal digits. Store it in root-owned mode-0600
`/etc/vera20k/relay-ticket-key`; the unit supplies it through systemd credentials.
Set the identical key as the Cloudflare Worker's `RELAY_TICKET_KEY` secret. Never
place it in a command argument, source, test result, or client configuration.
Rotate the key during maintenance after existing matches have ended.

Set the Worker's public `RELAY_ADDRESS` to the server's IPv4 address and port.
Clients obtain individual signed tickets and packet keys over authenticated HTTPS;
they never receive the server signing key. Restarting the relay requires matches
to reconnect or fail through the client's existing timeout handling.

The service journal reports aggregate traffic and rejection counts once a minute.
These counts contain no player, endpoint, room identity, ticket or packet contents.
Use them to distinguish rate limits, expired grants and authentication failures
from packet loss and client timeouts.
