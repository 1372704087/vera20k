# VERA20k lobby protocol v1

HTTPS JSON only. Authentication is `Authorization: Bearer <token>`; tokens, passwords,
relay tickets and packet keys must never be logged. Names are ASCII letters, digits,
`_` or `-`, 1–24 bytes, unique without case. Passwords are 12–128 bytes. Accounts
and session tokens are independent of the legacy WOL nickname/password format.

`POST /v1/accounts` creates an account and signs in. `POST /v1/sessions` signs in.
Both accept `{nickname,password}` and return
`{player_id,nickname,token,expires_at}`. `expires_at` is a Unix timestamp in seconds.
Failed requests return `{error:{code,message}}`; messages must not echo credentials.

`POST /v1/commands` accepts `{request_id,after,operation,payload}`. A request ID is unique
within a session and retries are idempotent. Responses and
`GET /v1/events?after=<cursor>` return `{events:[{sequence,kind,payload}],cursor}`.
Sequences increase within a session. An event appears once logically even when
responses are repeated. `GET /v1/channels?kind=0|41` returns `{kind,channels}`.
Clients poll at most once per second. Responses contain at most 256 events and
256 KiB from a 1,024-event history;
expired cursors fail explicitly rather than silently dropping state.

Channel: `{id,kind,max_users,users,tournament,flags,game_flags,ping,name,ex_info}`.
`kind` is 0 for chat and 41 for a game. The other fields are presentation metadata,
not authority. `name` identifies a channel uniquely for compatibility with the
existing shell. Member: `{player_id,name,flags,squad,clan_name,clan_tag}`. Flags bit 0
is the channel owner, bit 1 voice; the client adds its own local bit 32768. Servers
set identity and flags from the authenticated membership, never from user payloads.

Commands and their payloads:

| operation | payload |
| --- | --- |
| `join` | `{channel:<channel name>,key}` |
| `create` | `{channel:<Channel>,key}` |
| `leave` | `{}` |
| `logout` | `{}` |
| `options` | `{options:<HostOptions>}` |
| `choices` | `{row,slot:<SkirmishLocalSlot>,observer}` |
| `ready` | `{ready,revision,version,rules_digest,map_digest,effective_map_digest,staging_digest}` |
| `map_status` | `{status,revision,version,rules_digest,map_digest,effective_map_digest,staging_digest}` |
| `advertise` | `{channel:<Channel>}` |
| `start` | `{users:[name],revision,version,rules_digest,effective_map_digest,staging_digest}` |
| `finish` | `{match_id}` |
| `relay_ready` | `{match_id}` |
| `latency_ready` | `{match_id,pings:[{source_index,round_trip_ms}]}` |
| `renew_relay` | `{match_id}` |
| `map_upload` | `{digest,byte_length,offset,data,resource?}` |
| `chat` | `{text,to:[name],action}` |
| `ping` | `{}` |
| `locales` | `{names:[name]}` |
| `set_locale` | `{locale}` |
| `buddies` | `{}` |
| `add_buddy` / `remove_buddy` / `locate` | `{name}` |
| `kick` | `{name}` |

HostOptions is `{launch,map_metadata,vacancies,observer_row,cooperative_difficulty,seed}`.
Generated maps add `random_map:{start_staging,preview:{source_sha256,byte_length}}`.
`start_staging` is the canonical 37-number native `W` record, bounded to512bytes;
`staging_digest` is its lowercase SHA-256, or the empty string for authored maps.
`launch` is the existing serde representation of `SkirmishLaunchSession` (including
its human rows, AI rows and launch options). `map_metadata` is
`{display_name,player_capacity,official,byte_length,digest,source_sha256,effective_digest}`. The legacy
INI `digest` remains distinct from lowercase SHA-256 over the physical map bytes;
online `map_digest` agreements compare `source_sha256`. `effective_digest` and
`effective_map_digest` compare the parsed gameplay map after its applicable include
expansion, so identical root bytes cannot conceal different local dependencies. `vacancies` is an array
of `open`, `closed`, `observer` or `waiting`. A receiver resolves its own
`launch.local_player_index` from the authenticated player; it never uses the host's.
`slot` uses `SkirmishLocalSlot` serde. Options are authored only by the room owner;
the service increments `revision` and clears guest readiness on a material change.

Event kinds and payloads:

| kind | payload |
| --- | --- |
| `joined` / `created` | `{channel:<Channel>,members:[Member]}` |
| `channel_left` | `{}` |
| `member_joined` | `{member:<Member>}` |
| `member_left` | `{name}` |
| `options` | `{revision,options:<HostOptions>}` |
| `choices` | `{from,row,slot,observer}` |
| `ready` | `{from,ready,revision}` |
| `map_status` | `{from,status,revision}` |
| `chat` | `{from:<Member>,text,private,action}` |
| `game_started` | `{match_id,revision,version,rules_digest,seed,users:[{player_id,name,flags,source_index}],relay:<RelayGrant>}` |
| `start_confirmed` | `{match_id}` |
| `timing_confirmed` | `{match_id,pings:[integer]}` |
| `relay_renewed` | `{match_id,relay:<RelayGrant>}` |
| `map_available` | `{digest,byte_length,resource?}` |
| `start_rejected` | `{message}` |
| `locales` | `{locales:[{name,locale}]}` |
| `buddies` | `{names:[name]}` |
| `located` | `{channel:<Channel or null>}` |
| `kicked` | `{channel:<Channel>,user:<Member>,by:<Member>}` |
| `error` | `{code,message}` |

RelayGrant is `{address,ticket,packet_key,source_index}`. Address is `IPv4:port`;
ticket is the signed relay protocol ticket encoded base64url without padding and
packet_key is 32 bytes encoded the same way. Credentials are unique to each user
and each new match. `source_index` is the immutable canonical user index, 0–29.
The relay room uses the fresh `match_id`, including for rematches. The server sends
`game_started` only after all current participants agree to revision, version,
rules digest and map metadata and are ready. The client still performs authenticated
UDP admission and validates its local map/rules/version before it admits the start.
The client stages a Start click while it publishes the final assignments from
its existing assignment owner. It does not send `start` with an earlier revision.
A changed publication clears every prior ready/map acknowledgement, and guests
explicitly accept the final displayed settings. Only after the matching options
revision and every fresh acknowledgement arrive does the retained Start request
reach the server. A local optimistic choice cannot be accepted before its host
options echo. Later edits cancel the staged request; rejected or timed-out starts
release the normal setup screen for retry without rerunning assignment while a
request is pending. The service continues enforcing the same full agreement.

An HTTP reply alone never establishes a UDP peer connection. Each client sends
`relay_ready` only after the authenticated UDP admission completes. The server emits
`start_confirmed` after every admitted member has acknowledged that same match.
Clients then measure actual peer round trips through their reliable UDP owner.
Every `latency_ready` report contains each other canonical source exactly once;
the service sends the same per-source maxima in `timing_confirmed` to everyone.
Only then does game loading start. Each barrier has a 30-second timeout.
During play, clients renew their relay admission every 30 minutes. The server
selects the source from authenticated frozen membership, issues a fresh two-hour
grant and extends the active match lease. Clients authenticate the replacement
on the existing socket before promoting it; game/reliable queues remain intact.

Every live VERA event packet, including FRAMESYNC probes, starts with bytes
`56 43 4d 01` (`VCM`, version 1), then an immutable little-endian `u64` command
frontier for the source's sending frame, followed by the existing 14-byte event
header and body. The payload remains at most 558 bytes. Native `timing_word` is
only the low 16-bit projection; readiness compares the full frontier with
reliably decoded commands. Live packets have no raw-header fallback.

Reliable broadcast must accept every peer copy before the sender removes or
mirrors commands or commits compression state. A rejected send retains the same
frame target and retries while the existing receive/ACK service proceeds. Local
outstanding input is bounded to 32,768 source commands, reserving 128 slots for
control records. Receiver DoList allows 65,536 records per source to include one
FRAMEINFO per packet, with a 30-source total bound and allocation on demand.

The VCM1 event body also carries these fixed VERA records. Each retains the
ordinary seven-byte event header and authenticated source House. All numeric
payload fields are little-endian; unknown action/category values are rejected.

| Opcode | Payload |
| --- | --- |
| `0x30` | 21 bytes: action u8 (begin 0, complete 1, cancel 2), source and destination SuperWeaponTypes indices i32, one-use sequence u64, cell x/y i16 |
| `0x31` | 5 bytes: the existing TargetClass object token for bunker eject, stable object identity i32 and kind `0x34` |
| `0x32` | 1 byte: production category Building 0, Defense 1, Infantry 2, Vehicle 3, Aircraft 4, Ship 5 |

Paid selections require distinct nonnegative type indices and a nonzero
sequence. Target cells are within 0–511; cancel has canonical zero coordinates.
The existing payment, bunker and producer owners validate and execute these
records. They add no independent ticket, entity or factory state. Native SELL
`0x16`, PRIMARY `0x11` and SPECIAL_PLACE `0x12` retain their existing layouts.

Maps are limited to 8 MiB. Only the room owner can upload the currently selected
physical SHA-256, in contiguous 32,768-byte chunks (except the final chunk).
`data` is unpadded base64url; identical chunk retries are accepted. A completed
upload is hash-checked before `map_available`. Authenticated room members use
`GET /v1/maps/<source_sha256>?offset=<multiple of 32768>` to receive
`{digest,byte_length,offset,data,complete}`. An unfinished upload returns
`map_pending` (409); a changed selection returns `map_changed` (409). Changing
only roster/settings revisions preserves the blob. A map change or room cleanup
removes it. Uploads/downloads have a separate 600-request/minute/session budget;
ordinary commands and events share 180/minute. Clients verify and install into a
local cache, preserve existing installed maps, and still require user readiness.
Generated maps transfer the `.SED` bytes as the map and the separate native PCX
preview with `resource:"preview"` (512KiB maximum). Preview downloads use the
selected seed's URL plus `&resource=preview`; response `digest` names the preview.
Both resources use the same ownership, ordered chunk, hash and cleanup rules.
Generated capacity equals the staged start count (2–8); observers consume only
transport slots. Start admission also compares the canonical staging digest, so clients cannot
agree on seed bytes while using different generated start cells.

The existing shell leaves its game channel during the successful start. The service
therefore retains the match admission independently of channel membership until
finish/timeout. Postmatch the shell rejoins chat and may create or join another game;
this is a fresh match with fresh relay credentials. A channel owner leaving before
start closes the room and emits a failure/leave event to the guests. Membership,
authorization, readiness, sequence history and idle expiration are server-owned.
An authenticated session idle for 120 seconds is retired, including its service
match membership; gameplay peer loss remains the simulation's responsibility.
