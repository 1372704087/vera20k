// The service owns accounts, room membership and agreement, never simulation.
// Storage is synchronous inside a SQLite Durable Object transaction. Requests
// are serialized across asynchronous password hashing and relay signing too.
const encoder = new TextEncoder();
const MAX_EVENTS = 1024;
const EVENT_BATCH = 256;
const SESSION_SECONDS = 24 * 60 * 60;
const MATCH_SECONDS = 2 * 60 * 60;
const MAX_PLAYERS = 30;
const MAP_CHUNK_BYTES = 32768;
const MAX_MAP_BYTES = 8 * 1024 * 1024;
const copy = (value) => structuredClone(value);
const pendingMatch = (value) => value?.phase === 'connecting' || value?.phase === 'probing';

export class ApiError extends Error {
  constructor(status, code, message) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

function requireThat(value, code, message, status = 400) {
  if (!value) throw new ApiError(status, code, message);
}

export function base64url(bytes) {
  return btoa(String.fromCharCode(...bytes)).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '');
}

function mapChunk(value) {
  requireThat(typeof value === 'string' && value.length > 0 && value.length <= 43691 &&
    /^[A-Za-z0-9_-]+$/.test(value), 'invalid_map_chunk', 'Invalid map chunk.');
  try {
    const bytes = Uint8Array.from(atob(value.replaceAll('-', '+').replaceAll('_', '/')), (char) => char.charCodeAt(0));
    requireThat(bytes.length <= MAP_CHUNK_BYTES && base64url(bytes) === value, 'invalid_map_chunk', 'Invalid map chunk.');
    return bytes;
  } catch { throw new ApiError(400, 'invalid_map_chunk', 'Invalid map chunk.'); }
}

function randomBytes(length) {
  return crypto.getRandomValues(new Uint8Array(length));
}

async function sha256(text) {
  return base64url(new Uint8Array(await crypto.subtle.digest('SHA-256', encoder.encode(text))));
}

async function passwordHash(password, salt) {
  const key = await crypto.subtle.importKey('raw', encoder.encode(password), 'PBKDF2', false, ['deriveBits']);
  return base64url(new Uint8Array(await crypto.subtle.deriveBits({
    name: 'PBKDF2', hash: 'SHA-256', salt: encoder.encode(salt), iterations: 100000,
  }, key, 256)));
}

function sameSecret(a, b) {
  if (typeof a !== 'string' || typeof b !== 'string' || a.length !== b.length) return false;
  let difference = 0;
  for (let i = 0; i < a.length; i++) difference |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return difference === 0;
}

function memberView(member, room) {
  return { player_id: member.player_id, name: member.name,
    flags: member.player_id === room.owner ? 1 : 0, squad: 0, clan_name: '', clan_tag: '' };
}

function channelView(room) {
  return { ...room.channel, id: room.id, users: room.members.length, max_users: room.capacity };
}

// This codec is shared with services/relay-protocol: fixed-width little-endian
// ticket, domain-separated HMACs. The server key never leaves secret bindings.
export async function relayGrant(secretHex, address, matchId, sourceIndex, now) {
  requireThat(/^[0-9a-f]{64}$/i.test(secretHex ?? ''), 'relay_unavailable', 'Gameplay service is unavailable.', 503);
  requireThat(/^\d{1,3}(\.\d{1,3}){3}:\d{1,5}$/.test(address ?? ''), 'relay_unavailable', 'Gameplay service is unavailable.', 503);
  const secret = Uint8Array.from(secretHex.match(/../g), (pair) => parseInt(pair, 16));
  const key = await crypto.subtle.importKey('raw', secret, { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  const ticket = new Uint8Array(74);
  ticket[0] = 1;
  ticket.set(Uint8Array.from(matchId.replaceAll('-', '').match(/../g), (pair) => parseInt(pair, 16)), 1);
  ticket[17] = sourceIndex;
  new DataView(ticket.buffer).setBigUint64(18, BigInt(now + MATCH_SECONDS), true);
  ticket.set(randomBytes(16), 26);
  const sign = async (domain, bytes) => {
    const prefix = encoder.encode(domain);
    const input = new Uint8Array(prefix.length + bytes.length);
    input.set(prefix); input.set(bytes, prefix.length);
    return new Uint8Array(await crypto.subtle.sign('HMAC', key, input));
  };
  ticket.set(await sign('VERA20k relay ticket v1\0', ticket.subarray(0, 42)), 42);
  const packetKey = await sign('VERA20k relay packet key v1\0', ticket);
  return { address, ticket: base64url(ticket), packet_key: base64url(packetKey), source_index: sourceIndex };
}

export class SqlStore {
  constructor(sql, transaction) {
    this.sql = sql;
    this.transaction = transaction;
    sql.exec('CREATE TABLE IF NOT EXISTS state (key TEXT PRIMARY KEY, value TEXT NOT NULL)');
    sql.exec('CREATE TABLE IF NOT EXISTS events (session TEXT NOT NULL, sequence INTEGER NOT NULL, value TEXT NOT NULL, PRIMARY KEY(session, sequence))');
  }
  get(key) {
    const row = this.sql.exec('SELECT value FROM state WHERE key = ?', key).toArray()[0];
    return row ? JSON.parse(row.value) : null;
  }
  put(key, value) {
    this.sql.exec('INSERT INTO state(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value', key, JSON.stringify(value));
  }
  delete(key) { this.sql.exec('DELETE FROM state WHERE key = ?', key); }
  deletePrefix(prefix) { this.sql.exec('DELETE FROM state WHERE key >= ? AND key < ?', prefix, `${prefix}\uffff`); }
  list(prefix) {
    return this.sql.exec('SELECT value FROM state WHERE key >= ? AND key < ? ORDER BY key', prefix, `${prefix}\uffff`).toArray().map((row) => JSON.parse(row.value));
  }
  event(sessionId, sequence, value) {
    this.sql.exec('INSERT INTO events VALUES (?,?,?)', sessionId, sequence, JSON.stringify(value));
    this.sql.exec('DELETE FROM events WHERE session = ? AND sequence <= ?', sessionId, sequence - MAX_EVENTS);
  }
  events(sessionId, after) {
    return this.sql.exec(`SELECT value FROM events WHERE session = ? AND sequence > ? ORDER BY sequence LIMIT ${EVENT_BATCH}`, sessionId, after).toArray().map((row) => JSON.parse(row.value));
  }
  deleteEvents(sessionId) { this.sql.exec('DELETE FROM events WHERE session = ?', sessionId); }
  expireRates(now) {
    this.sql.exec("DELETE FROM state WHERE key IN (SELECT key FROM state WHERE key LIKE 'rate:%' AND json_extract(value,'$.until') <= ? LIMIT 256)", now);
  }
}

export class LobbyService {
  constructor(store, env, clock = () => Math.floor(Date.now() / 1000), bodyTimeoutMs = 5000) {
    this.store = store;
    this.env = env;
    this.clock = clock;
    this.bodyTimeoutMs = bodyTimeoutMs;
    this.tail = Promise.resolve();
    if (!store.get('room:public')) store.put('room:public', {
      id: 'public', owner: null, capacity: 4096, kind: 0,
      channel: { name: 'Lob_41_0', kind: 0, tournament: 0, flags: 0, game_flags: 0, ping: -1, ex_info: '' },
      members: [], revision: 0, phase: 'lobby', ready: {}, maps: {}, agreements: {}, created: this.clock(),
    });
  }

  async fetch(request) {
    try {
      // Ingress reads cannot hold the actor's mutation queue. A client that
      // never finishes its body loses only its own request at the deadline.
      if (request.method === 'POST') {
        if (['/v1/accounts', '/v1/sessions'].includes(new URL(request.url).pathname)) {
          const key = await sha256(request.headers.get('CF-Connecting-IP') ?? 'local');
          this.store.transaction(() => this.rateLimit(`ingress-auth:${key}`, 120, 60));
        } else {
          const { sessionId } = await this.authenticated(request);
          this.store.transaction(() => this.rateLimit(`ingress-session:${sessionId}`, 900, 60));
        }
      }
      const body = request.method === 'POST' ? await this.readBody(request) : null;
      const result = this.tail.then(() => this.handle(request, body));
      this.tail = result.catch(() => {});
      return await result;
    } catch (error) {
      const known = error instanceof ApiError;
      return this.json({ error: { code: known ? error.code : 'service_unavailable',
        message: known ? error.message : 'The lobby service is unavailable. Please try again.' } }, known ? error.status : 503);
    }
  }

  maintenance() {
    const result = this.tail.then(() => this.reap());
    this.tail = result.catch(() => {});
    return result;
  }

  json(body, status = 200) {
    return new Response(JSON.stringify(body), { status, headers: {
      'Content-Type': 'application/json', 'Cache-Control': 'no-store',
      'X-Content-Type-Options': 'nosniff',
    } });
  }

  async readBody(request) {
    requireThat((request.headers.get('content-type') ?? '').startsWith('application/json'), 'invalid_request', 'JSON is required.', 415);
    requireThat(Number(request.headers.get('content-length') ?? 0) <= 65536, 'too_large', 'Request is too large.', 413);
    const reader = request.body?.getReader();
    requireThat(reader, 'invalid_request', 'JSON is required.');
    let length = 0;
    const chunks = [];
    let timer;
    const deadline = new Promise((_, reject) => {
      timer = setTimeout(() => {
        reject(new ApiError(408, 'request_timeout', 'Request body timed out.'));
        void reader.cancel().catch(() => {});
      }, this.bodyTimeoutMs);
    });
    try {
      while (true) {
        const { value, done } = await Promise.race([reader.read(), deadline]);
        if (done) break;
        length += value.byteLength;
        if (length > 65536) { void reader.cancel().catch(() => {}); throw new ApiError(413, 'too_large', 'Request is too large.'); }
        chunks.push(value);
      }
    } finally { clearTimeout(timer); }
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
    try {
      const body = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
      requireThat(body && typeof body === 'object' && !Array.isArray(body), 'invalid_request', 'A JSON object is required.');
      return body;
    } catch { throw new ApiError(400, 'invalid_request', 'Invalid JSON request.'); }
  }

  rateLimit(key, maximum, seconds) {
    const now = this.clock();
    const old = this.store.get(`rate:${key}`);
    const rate = old && old.until > now ? old : { count: 0, until: now + seconds };
    requireThat(rate.count < maximum, 'rate_limited', 'Please wait before trying again.', 429);
    rate.count++;
    this.store.put(`rate:${key}`, rate);
  }

  async authenticated(request) {
    const authorization = request.headers.get('authorization') ?? '';
    requireThat(/^Bearer [A-Za-z0-9_-]{43}$/.test(authorization), 'unauthorized', 'Sign in again.', 401);
    const sessionId = await sha256(authorization.slice(7));
    const session = this.store.get(`session:${sessionId}`);
    requireThat(session && session.expires_at > this.clock(), 'unauthorized', 'Sign in again.', 401);
    return { sessionId, session };
  }

  async handle(request, body) {
    const url = new URL(request.url);
    if (url.pathname === '/health' && request.method === 'GET') return this.json({ status: 'ok', protocol: 1 });
    if (request.method === 'POST' && ['/v1/accounts', '/v1/sessions'].includes(url.pathname)) {
      const origin = request.headers.get('CF-Connecting-IP') ?? 'local';
      this.rateLimit(`auth:${await sha256(origin)}`, 60, 300);
      this.reap();
      return this.authenticate(body, url.pathname === '/v1/accounts');
    }
    const { sessionId, session } = await this.authenticated(request);
    const mapRequest = url.pathname.startsWith('/v1/maps/') || body?.operation === 'map_upload';
    this.rateLimit(`${mapRequest ? 'map' : 'session'}:${sessionId}`, mapRequest ? 600 : 180, 60);
    session.seen = this.clock(); this.store.put(`session:${sessionId}`, session);
    this.reap();
    if (url.pathname.startsWith('/v1/maps/') && request.method === 'GET')
      return this.downloadMap(session, url);
    if (url.pathname === '/v1/channels' && request.method === 'GET') {
      const kind = Number(url.searchParams.get('kind'));
      requireThat(kind === 0 || kind === 41, 'invalid_request', 'Unknown channel type.');
      return this.json({ kind, channels: this.store.list('room:').filter((room) => room.kind === kind && room.phase === 'lobby').map(channelView) });
    }
    if (url.pathname === '/v1/events' && request.method === 'GET') {
      const after = Number(url.searchParams.get('after') ?? 0);
      return this.json(this.events(sessionId, after));
    }
    if (url.pathname === '/v1/commands' && request.method === 'POST') {
      requireThat(typeof body.request_id === 'string' && /^[A-Za-z0-9_-]{1,80}$/.test(body.request_id), 'invalid_request', 'A request ID is required.');
      requireThat(typeof body.operation === 'string' && body.payload && typeof body.payload === 'object', 'invalid_request', 'An operation and payload are required.');
      const duplicate = session.requests.find((item) => item.id === body.request_id);
      const fingerprint = await sha256(JSON.stringify([body.operation, body.payload]));
      const after = body.after ?? session.ack;
      this.validateCursor(session, after);
      if (duplicate) {
        requireThat(duplicate.fingerprint === fingerprint, 'request_conflict', 'Request ID was already used.', 409);
        return this.json(this.events(sessionId, after));
      }
      // Hold already-pending events before the mutation can retire the oldest
      // retained row. A reply never reports failure after committing a command
      // just because that command appended one more event at the history edge.
      const pendingResponse = this.events(sessionId, after);
      const prepared = await this.prepare(session, body.operation, body.payload);
      this.store.transaction(() => {
        this.command(sessionId, body.operation, body.payload, prepared);
        const current = this.store.get(`session:${sessionId}`);
        if (current) {
          current.requests.push({ id: body.request_id, fingerprint });
          current.requests = current.requests.slice(-256);
          this.store.put(`session:${sessionId}`, current);
        }
      });
      return this.json(this.store.get(`session:${sessionId}`)
        ? (pendingResponse.events.length ? pendingResponse : this.events(sessionId, after))
        : { events: [], cursor: 0 });
    }
    throw new ApiError(404, 'not_found', 'Endpoint not found.');
  }

  async authenticate(body, create) {
    const { nickname, password } = body;
    requireThat(typeof nickname === 'string' && /^[A-Za-z0-9_-]{1,24}$/.test(nickname), 'invalid_name', 'Use 1–24 letters, digits, underscores or hyphens.');
    requireThat(typeof password === 'string' && encoder.encode(password).length >= 12 && encoder.encode(password).length <= 128,
      'invalid_password', 'Use a password of 12–128 bytes.');
    const accountKey = `account:${nickname.toLowerCase()}`;
    let account = this.store.get(accountKey);
    if (create) {
      requireThat(!account, 'name_unavailable', 'That player name is unavailable.', 409);
      const salt = base64url(randomBytes(16));
      account = { player_id: crypto.randomUUID(), nickname, salt, verifier: await passwordHash(password, salt), locale: 0, buddies: [] };
    } else {
      const candidate = await passwordHash(password, account?.salt ?? 'unknown-account-dummy-salt');
      requireThat(account && sameSecret(candidate, account.verifier), 'unauthorized', 'The name or password is incorrect.', 401);
    }
    const token = base64url(randomBytes(32));
    const id = await sha256(token);
    const now = this.clock();
    const session = { id, player_id: account.player_id, nickname: account.nickname, account_key: accountKey,
      expires_at: now + SESSION_SECONDS, seen: now, channel_id: null, match_id: null, sequence: 0, ack: 0, requests: [] };
    this.store.transaction(() => {
      if (create) this.store.put(accountKey, account);
      // A new sign-in replaces prior device sessions of this account. It cannot
      // silently take over a participant in an admitted match.
      for (const previous of this.store.list('session:').filter((value) => value.player_id === account.player_id)) {
        requireThat(!previous.match_id, 'match_active', 'This account already has an active match.', 409);
        this.leave(previous); this.store.delete(`session:${previous.id}`); this.store.deleteEvents(previous.id);
      }
      this.store.put(`session:${id}`, session);
    });
    return this.json({ player_id: account.player_id, nickname: account.nickname, token, expires_at: session.expires_at });
  }

  emit(sessionId, kind, payload) {
    const session = this.store.get(`session:${sessionId}`);
    if (!session) return;
    session.sequence++;
    this.store.event(sessionId, session.sequence, { sequence: session.sequence, kind, payload });
    this.store.put(`session:${sessionId}`, session);
  }

  broadcast(room, kind, payload, except = null) {
    for (const member of room.members) if (member.session_id !== except) this.emit(member.session_id, kind, payload);
  }

  events(sessionId, after) {
    const session = this.store.get(`session:${sessionId}`);
    this.validateCursor(session, after);
    if (after > session.ack) { session.ack = after; this.store.put(`session:${sessionId}`, session); }
    let length = 0;
    const events = [];
    for (const event of this.store.events(sessionId, after)) {
      length += JSON.stringify(event).length;
      if (length > 256 * 1024 && events.length) break;
      events.push(event);
    }
    return { events, cursor: events.at(-1)?.sequence ?? after };
  }

  validateCursor(session, after) {
    requireThat(Number.isSafeInteger(after) && after >= 0 && after <= session.sequence, 'invalid_cursor', 'Invalid event cursor.', 409);
    requireThat(after >= session.sequence - MAX_EVENTS, 'history_expired', 'Lobby history expired. Sign in again.', 409);
  }

  room(session, host = false) {
    const room = session.channel_id && this.store.get(`room:${session.channel_id}`);
    requireThat(room && room.members.some((member) => member.session_id === session.id), 'not_in_room', 'Join a room first.', 409);
    requireThat(!host || room.owner === session.player_id, 'forbidden', 'Only the host can do that.', 403);
    return room;
  }

  clearMap(roomId) {
    this.store.delete(`map-upload:${roomId}`);
    this.store.deletePrefix(`map-chunk:${roomId}:`);
  }

  deleteRoom(roomId) {
    this.clearMap(roomId);
    this.store.delete(`room:${roomId}`);
  }

  downloadMap(session, url) {
    const room = this.room(session);
    const digest = url.pathname.slice('/v1/maps/'.length);
    requireThat(/^[0-9a-f]{64}$/.test(digest) && digest === room.options?.map_metadata?.source_sha256,
      'map_changed', 'The selected map changed.', 409);
    const upload = this.store.get(`map-upload:${room.id}`);
    requireThat(upload?.digest === digest && upload.complete, 'map_pending', 'The host is preparing the map.', 409);
    const offset = Number(url.searchParams.get('offset'));
    requireThat(Number.isSafeInteger(offset) && offset >= 0 && offset < upload.byte_length && offset % MAP_CHUNK_BYTES === 0,
      'invalid_map_chunk', 'Invalid map offset.');
    const chunk = this.store.get(`map-chunk:${room.id}:${offset}`);
    requireThat(chunk, 'map_pending', 'The host is preparing the map.', 409);
    return this.json({ digest, byte_length: upload.byte_length, offset, data: chunk.data,
      complete: offset + chunk.length === upload.byte_length });
  }

  leave(session) {
    let room = session.channel_id && this.store.get(`room:${session.channel_id}`);
    if (!room) {
      // The final finish may already have retired this room. The shell still
      // waits for its leave acknowledgement before joining the public lobby.
      if (session.channel_id) {
        session.channel_id = null; this.store.put(`session:${session.id}`, session);
        this.emit(session.id, 'channel_left', {});
      }
      return;
    }
    if (pendingMatch(room)) {
      this.abortPending(this.store.get(`match:${room.match_id}`), 'A player left before the game started.');
      session = this.store.get(`session:${session.id}`);
      room = this.store.get(`room:${room.id}`);
    }
    session.channel_id = null;
    this.store.put(`session:${session.id}`, session);
    this.emit(session.id, 'channel_left', {});
    room.members = room.members.filter((member) => member.session_id !== session.id);
    if (room.phase === 'playing' || pendingMatch(room)) {
      // Gameplay still owns the frozen membership. This is only room presence.
      this.store.put(`room:${room.id}`, room);
    } else if (room.owner === session.player_id) {
      for (const member of room.members) {
        const other = this.store.get(`session:${member.session_id}`);
        if (other) { other.channel_id = null; this.store.put(`session:${other.id}`, other); }
        this.emit(member.session_id, 'error', { code: 'host_left', message: 'The host left the room.' });
        this.emit(member.session_id, 'channel_left', {});
      }
      this.deleteRoom(room.id);
    } else {
      delete room.ready[session.player_id]; delete room.maps[session.player_id]; delete room.agreements[session.player_id];
      this.broadcast(room, 'member_left', { name: session.nickname });
      this.invalidate(room);
      this.store.put(`room:${room.id}`, room);
    }
  }

  invalidate(room) {
    room.revision++;
    room.ready = {}; room.maps = {}; room.agreements = {};
    const names = room.options?.launch?.humans?.map((human) => human.name);
    if (names?.length === room.members.length && room.members.every((member) => names.includes(member.name)))
      this.broadcast(room, 'options', { revision: room.revision, options: room.options });
  }

  abortPending(match, message) {
    if (!match) return;
    for (const session of this.store.list('session:').filter((session) => session.match_id === match.id)) {
      session.match_id = null; this.store.put(`session:${session.id}`, session);
      this.emit(session.id, 'start_rejected', { message });
    }
    this.store.delete(`match:${match.id}`);
    const room = this.store.get(`room:${match.room_id}`);
    if (room) {
      room.phase = 'lobby'; delete room.match_id; this.invalidate(room); this.store.put(`room:${room.id}`, room);
    }
  }

  finish(session) {
    const match = this.store.get(`match:${session.match_id}`);
    if (pendingMatch(match)) {
      this.abortPending(match, 'A player cancelled the game start.');
      return;
    }
    if (match) {
      match.finished = [...new Set([...match.finished, session.player_id])];
      if (match.finished.length === match.users.length) {
        this.store.delete(`match:${match.id}`); this.deleteRoom(match.room_id);
      } else this.store.put(`match:${match.id}`, match);
    }
    session.match_id = null; this.store.put(`session:${session.id}`, session);
  }

  async prepare(session, operation, payload) {
    if (operation === 'map_upload') {
      const room = this.room(session, true);
      const metadata = room.options?.map_metadata;
      requireThat(room.phase === 'lobby' && metadata && payload.digest === metadata.source_sha256 &&
        payload.byte_length === metadata.byte_length, 'map_changed', 'The selected map changed.', 409);
      requireThat(Number.isSafeInteger(payload.byte_length) && payload.byte_length > 0 && payload.byte_length <= MAX_MAP_BYTES &&
        Number.isSafeInteger(payload.offset) && payload.offset >= 0 && payload.offset < payload.byte_length &&
        payload.offset % MAP_CHUNK_BYTES === 0, 'invalid_map_chunk', 'Invalid map size or offset.');
      const bytes = mapChunk(payload.data);
      requireThat(bytes.length === Math.min(MAP_CHUNK_BYTES, payload.byte_length - payload.offset),
        'invalid_map_chunk', 'Map chunks must cover the complete file in order.');
      const upload = this.store.get(`map-upload:${room.id}`);
      const received = upload?.digest === payload.digest ? upload.received : 0;
      if (payload.offset < received) {
        const previous = this.store.get(`map-chunk:${room.id}:${payload.offset}`);
        requireThat(previous?.data === payload.data, 'map_changed', 'This map chunk was already accepted.', 409);
        return { duplicate: true, complete: upload.complete };
      }
      requireThat(payload.offset === received, 'map_offset', 'Map chunks must arrive in order.', 409);
      const complete = payload.offset + bytes.length === payload.byte_length;
      if (complete) {
        const all = new Uint8Array(payload.byte_length);
        for (let offset = 0; offset < payload.offset; offset += MAP_CHUNK_BYTES) {
          const previous = this.store.get(`map-chunk:${room.id}:${offset}`);
          requireThat(previous, 'map_offset', 'A map chunk is missing.', 409);
          all.set(mapChunk(previous.data), offset);
        }
        all.set(bytes, payload.offset);
        const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', all));
        const hex = Array.from(digest, (byte) => byte.toString(16).padStart(2, '0')).join('');
        requireThat(hex === payload.digest, 'map_digest', 'The map bytes do not match the selected map.');
      }
      return { complete, length: bytes.length };
    }
    if (operation === 'join' || operation === 'create') {
      requireThat(typeof (payload.key ?? '') === 'string' && (payload.key ?? '').length <= 128, 'invalid_request', 'Invalid room password.');
      return { key: payload.key ? await sha256(payload.key) : '' };
    }
    if (operation !== 'start') return {};
    const room = this.room(session, true);
    requireThat(room.kind === 41 && room.phase === 'lobby' && room.options, 'not_ready', 'The room is not ready.', 409);
    requireThat(room.members.length >= 2 && room.members.length <= MAX_PLAYERS, 'not_ready', 'At least two players are required.', 409);
    requireThat(payload.revision === room.revision && Array.isArray(payload.users), 'stale_revision', 'The room changed. Accept the settings again.', 409);
    const names = room.options.launch?.humans?.map((human) => human.name);
    requireThat(names && names.length === room.members.length && new Set(names).size === names.length &&
      names.every((name, index) => name === payload.users[index] && room.members.some((member) => member.name === name)),
      'roster_mismatch', 'The player list changed. Accept the settings again.', 409);
    requireThat(typeof payload.version === 'string' && payload.version.length > 0 && payload.version.length <= 128 &&
      typeof payload.rules_digest === 'string' && payload.rules_digest.length > 0 && payload.rules_digest.length <= 128,
      'agreement_mismatch', 'The game version and rules must agree.', 409);
    for (const member of room.members) {
      const agreement = room.agreements[member.player_id];
      requireThat(member.player_id === room.owner || room.ready[member.player_id] === true, 'not_ready', 'Every player must accept the settings.', 409);
      requireThat(room.maps[member.player_id] === 1 && agreement && agreement.revision === room.revision &&
        agreement.version === payload.version && agreement.rules_digest === payload.rules_digest &&
        agreement.map_digest === room.options.map_metadata.source_sha256, 'agreement_mismatch', 'Every player must have the same map, rules and game version.', 409);
    }
    const matchId = crypto.randomUUID();
    const seed = new DataView(randomBytes(4).buffer).getUint32(0, true);
    const users = names.map((name, index) => {
      const member = room.members.find((member) => member.name === name);
      return { player_id: member.player_id, name, source_index: index, flags: member.player_id === room.owner ? 1 : 0 };
    });
    const grants = await Promise.all(users.map((user) => relayGrant(this.env.RELAY_TICKET_KEY, this.env.RELAY_ADDRESS, matchId, user.source_index, this.clock())));
    return { matchId, seed, users, grants };
  }

  command(sessionId, operation, payload, prepared) {
    let session = this.store.get(`session:${sessionId}`);
    if (operation === 'logout') {
      requireThat(!session.match_id, 'match_active', 'Finish the active match before signing out.', 409);
      this.leave(session); this.store.delete(`session:${sessionId}`); this.store.deleteEvents(sessionId); return;
    }
    if (operation === 'leave') { this.leave(session); return; }
    if (operation === 'create' || operation === 'join') {
      requireThat(!session.match_id, 'match_active', 'Finish the active match before joining another.', 409);
      let room;
      if (operation === 'create') {
        const channel = payload.channel;
        requireThat(channel && channel.kind === 41 && typeof channel.name === 'string' && channel.name.length > 0 && channel.name.length <= 80,
          'invalid_room', 'Invalid room settings.');
        requireThat(!this.store.list('room:').some((room) => room.channel.name.toLowerCase() === channel.name.toLowerCase()), 'name_unavailable', 'That room name is already in use.', 409);
        requireThat(this.store.list('room:').length < 1024, 'service_full', 'The lobby service is full.', 503);
        room = { id: crypto.randomUUID(), kind: 41, owner: session.player_id,
          capacity: Math.max(2, Math.min(MAX_PLAYERS, Number(channel.max_users) || 6)),
          channel: { kind: 41, name: channel.name, tournament: 0, flags: Number(channel.flags) || 0,
            game_flags: Number(channel.game_flags) || 0, ping: -1, ex_info: String(channel.ex_info ?? '').slice(0, 512) },
          key: prepared.key, members: [], revision: 0, options: null, ready: {}, maps: {}, agreements: {}, phase: 'lobby', created: this.clock() };
      } else {
        requireThat(typeof payload.channel === 'string', 'invalid_room', 'Invalid room.');
        room = this.store.list('room:').find((room) => room.channel.name === payload.channel);
        requireThat(room && room.phase === 'lobby', 'room_unavailable', 'This room is no longer available.', 409);
        requireThat(sameSecret(room.key ?? '', prepared.key), 'room_password', 'The room password is incorrect.', 403);
        requireThat(room.members.length < room.capacity || session.channel_id === room.id, 'room_full', 'This room is full.', 409);
      }
      if (session.channel_id === room.id) {
        this.emit(sessionId, 'joined', { channel: channelView(room), members: room.members.map((member) => memberView(member, room)) }); return;
      }
      this.leave(session);
      session = this.store.get(`session:${sessionId}`);
      const member = { player_id: session.player_id, name: session.nickname, session_id: sessionId };
      this.broadcast(room, 'member_joined', { member: memberView(member, room) });
      room.members.push(member); session.channel_id = room.id;
      this.store.put(`session:${sessionId}`, session);
      this.emit(sessionId, operation === 'create' ? 'created' : 'joined', { channel: channelView(room), members: room.members.map((member) => memberView(member, room)) });
      if (room.kind === 41) this.invalidate(room);
      this.store.put(`room:${room.id}`, room); return;
    }
    if (operation === 'finish') {
      requireThat(session.match_id === payload.match_id, 'unknown_match', 'No matching active game.', 409);
      this.finish(session); return;
    }
    if (operation === 'relay_ready') {
      requireThat(session.match_id === payload.match_id, 'unknown_match', 'No matching active game.', 409);
      const match = this.store.get(`match:${session.match_id}`);
      requireThat(match && match.expires > this.clock(), 'unknown_match', 'This game session expired.', 409);
      if (match.phase === 'connecting') {
        match.ready = [...new Set([...match.ready, session.player_id])];
        if (match.ready.length === match.users.length) {
          match.phase = 'probing'; match.expires = this.clock() + 30;
          const room = this.store.get(`room:${match.room_id}`);
          room.phase = 'probing'; this.store.put(`room:${room.id}`, room);
          for (const other of this.store.list('session:').filter((value) => value.match_id === match.id))
            this.emit(other.id, 'start_confirmed', { match_id: match.id });
        }
        this.store.put(`match:${match.id}`, match);
      }
      return;
    }
    if (operation === 'latency_ready') {
      requireThat(session.match_id === payload.match_id, 'unknown_match', 'No matching active game.', 409);
      const match = this.store.get(`match:${session.match_id}`);
      requireThat(match?.phase === 'probing' && match.expires > this.clock(), 'not_ready', 'The game is not measuring peer latency.', 409);
      const source = match.users.find((user) => user.player_id === session.player_id)?.source_index;
      const reports = payload.pings;
      requireThat(Array.isArray(reports) && reports.length === match.users.length - 1 &&
        new Set(reports.map((report) => report?.source_index)).size === reports.length &&
        reports.every((report) => Number.isInteger(report?.source_index) && report.source_index >= 0 &&
          report.source_index < match.users.length && report.source_index !== source &&
          Number.isInteger(report.round_trip_ms) && report.round_trip_ms >= 0 && report.round_trip_ms <= 30000),
        'invalid_latency', 'A measured round trip for every other player is required.');
      const canonical = copy(reports).sort((left, right) => left.source_index - right.source_index);
      match.latencies ??= {};
      requireThat(!match.latencies[session.player_id] ||
        JSON.stringify(match.latencies[session.player_id]) === JSON.stringify(canonical),
        'latency_changed', 'The latency report was already accepted.', 409);
      match.latencies[session.player_id] = canonical;
      if (Object.keys(match.latencies).length === match.users.length) {
        // Every client receives the same complete timing input. Local RTTs
        // alone can select different native latency-fudge branches.
        const pings = match.users.map(() => 0);
        for (const report of Object.values(match.latencies))
          for (const peer of report) pings[peer.source_index] = Math.max(pings[peer.source_index], peer.round_trip_ms);
        match.phase = 'playing'; match.expires = this.clock() + MATCH_SECONDS;
        const room = this.store.get(`room:${match.room_id}`);
        room.phase = 'playing'; this.store.put(`room:${room.id}`, room);
        for (const other of this.store.list('session:').filter((value) => value.match_id === match.id))
          this.emit(other.id, 'timing_confirmed', { match_id: match.id, pings });
      }
      this.store.put(`match:${match.id}`, match);
      return;
    }
    if (operation === 'ping') return;
    if (['locales', 'set_locale', 'buddies', 'add_buddy', 'remove_buddy', 'locate'].includes(operation)) {
      const account = this.store.get(session.account_key);
      if (operation === 'set_locale') { requireThat(Number.isInteger(payload.locale), 'invalid_request', 'Invalid locale.'); account.locale = payload.locale; this.store.put(session.account_key, account); }
      if (operation === 'locales') {
        requireThat(Array.isArray(payload.names) && payload.names.length <= 64, 'invalid_request', 'Invalid player list.');
        const locales = payload.names.filter((name) => typeof name === 'string').map((name) => ({ name, locale: this.store.get(`account:${name.toLowerCase()}`)?.locale ?? 0 }));
        this.emit(sessionId, 'locales', { locales });
      }
      if (operation === 'add_buddy' || operation === 'remove_buddy') {
        requireThat(typeof payload.name === 'string' && /^[A-Za-z0-9_-]{1,24}$/.test(payload.name), 'invalid_request', 'Invalid player name.');
        account.buddies = account.buddies.filter((name) => name.toLowerCase() !== payload.name.toLowerCase());
        if (operation === 'add_buddy') account.buddies.push(payload.name);
        requireThat(account.buddies.length <= 100, 'buddy_limit', 'The buddy list is full.');
        this.store.put(session.account_key, account);
      }
      if (operation === 'buddies' || operation.endsWith('_buddy')) this.emit(sessionId, 'buddies', { names: account.buddies });
      if (operation === 'locate') {
        const room = this.store.list('room:').find((room) => room.members.some((member) => member.name === payload.name));
        this.emit(sessionId, 'located', { channel: room ? channelView(room) : null });
      }
      return;
    }
    const room = this.room(session, ['options', 'advertise', 'start', 'kick', 'map_upload'].includes(operation));
    requireThat(room.phase === 'lobby', 'match_active', 'The game has already started.', 409);
    if (operation === 'chat') {
      requireThat(typeof payload.text === 'string' && payload.text.length > 0 && encoder.encode(payload.text).length <= 2048 &&
        Array.isArray(payload.to) && payload.to.length <= MAX_PLAYERS, 'invalid_request', 'Invalid chat message.');
      const from = memberView(room.members.find((member) => member.session_id === sessionId), room);
      for (const member of room.members) {
        if (member.session_id !== sessionId && (!payload.to.length || payload.to.includes(member.name)))
          this.emit(member.session_id, 'chat', { from, text: payload.text, private: payload.to.length > 0, action: payload.action === true });
      }
      return;
    }
    if (operation === 'options') {
      const options = payload.options;
      const humans = Array.isArray(options?.launch?.humans) ? options.launch.humans : [];
      const playingHumans = humans.filter((human) => human?.role !== 'Observer' && human?.observer !== true).length;
      const computers = Array.isArray(options?.launch?.opponents) ? options.launch.opponents.length : 0;
      requireThat(room.kind === 41 && options?.launch && Array.isArray(options.launch.humans) &&
        options.launch.humans.length >= 1 && options.launch.humans.length <= MAX_PLAYERS && options.map_metadata &&
        humans.every((human) => human && typeof human.name === 'string' && /^[A-Za-z0-9_-]{1,24}$/.test(human.name)) &&
        (options.launch.opponents === undefined || Array.isArray(options.launch.opponents)) &&
        typeof options.map_metadata.digest === 'string' && options.map_metadata.digest.length <= 128 &&
        /^[0-9a-f]{64}$/.test(options.map_metadata.source_sha256 ?? '') &&
        Number.isSafeInteger(options.map_metadata.byte_length) && options.map_metadata.byte_length > 0 && options.map_metadata.byte_length <= MAX_MAP_BYTES &&
        Number.isInteger(options.map_metadata.player_capacity) && options.map_metadata.player_capacity >= playingHumans + computers &&
        options.map_metadata.player_capacity > 0 && options.map_metadata.player_capacity <= MAX_PLAYERS &&
        Array.isArray(options.vacancies) && options.vacancies.length <= MAX_PLAYERS &&
        options.vacancies.every((item) => ['open', 'closed', 'observer', 'waiting'].includes(item)), 'invalid_options', 'Invalid game settings.');
      const canonical = copy(options);
      canonical.launch.local_player_index = 0;
      if (canonical.map_metadata.source_sha256 !== room.options?.map_metadata?.source_sha256) this.clearMap(room.id);
      if (JSON.stringify(canonical) !== JSON.stringify(room.options)) { room.options = canonical; this.invalidate(room); }
    } else if (operation === 'map_upload') {
      if (!prepared.duplicate) {
        this.store.put(`map-chunk:${room.id}:${payload.offset}`, { data: payload.data, length: prepared.length });
        this.store.put(`map-upload:${room.id}`, { digest: payload.digest, byte_length: payload.byte_length,
          received: payload.offset + prepared.length, complete: prepared.complete });
      }
      if (prepared.complete) this.broadcast(room, 'map_available', { digest: payload.digest, byte_length: payload.byte_length });
    } else if (operation === 'advertise') {
      const channel = payload.channel;
      requireThat(channel && typeof channel.ex_info === 'string' && channel.ex_info.length <= 512, 'invalid_request', 'Invalid advertisement.');
      room.channel.ex_info = channel.ex_info;
      room.channel.game_flags = Number(channel.game_flags) || 0;
    } else if (operation === 'choices') {
      requireThat(room.kind === 41 && Number.isInteger(payload.row) && payload.row >= 0 && payload.row < MAX_PLAYERS && payload.slot &&
        typeof payload.slot === 'object' && typeof payload.observer === 'boolean', 'invalid_request', 'Invalid player settings.');
      room.ready[session.player_id] = false;
      this.broadcast(room, 'choices', { from: session.nickname, row: payload.row, slot: payload.slot, observer: payload.observer }, sessionId);
    } else if (operation === 'ready' || operation === 'map_status') {
      requireThat(room.kind === 41 && room.options && payload.revision === room.revision, 'stale_revision', 'The room changed. Accept the settings again.', 409);
      const agreement = { revision: payload.revision, version: payload.version, rules_digest: payload.rules_digest, map_digest: payload.map_digest };
      requireThat(typeof agreement.version === 'string' && agreement.version.length <= 128 && typeof agreement.rules_digest === 'string' &&
        agreement.rules_digest.length <= 128 && typeof agreement.map_digest === 'string' && agreement.map_digest.length <= 128,
        'invalid_request', 'Version, rules and map agreement are required.');
      room.agreements[session.player_id] = agreement;
      if (operation === 'ready') {
        requireThat(typeof payload.ready === 'boolean', 'invalid_request', 'Invalid readiness.');
        room.ready[session.player_id] = payload.ready;
        this.broadcast(room, 'ready', { from: session.nickname, ready: payload.ready, revision: room.revision }, sessionId);
      } else {
        requireThat([0, 1, 2].includes(payload.status), 'invalid_request', 'Invalid map status.');
        room.maps[session.player_id] = payload.status;
        this.broadcast(room, 'map_status', { from: session.nickname, status: payload.status, revision: room.revision }, sessionId);
      }
    } else if (operation === 'start') {
      const { matchId, seed, users, grants } = prepared;
      const match = { id: matchId, room_id: room.id, users, finished: [], ready: [], latencies: {}, phase: 'connecting',
        expires: this.clock() + 30, options: room.options, revision: room.revision };
      this.store.put(`match:${matchId}`, match);
      room.phase = 'connecting'; room.match_id = matchId;
      for (const user of users) {
        const member = room.members.find((member) => member.player_id === user.player_id);
        const other = this.store.get(`session:${member.session_id}`); other.match_id = matchId; this.store.put(`session:${other.id}`, other);
        this.emit(other.id, 'game_started', { match_id: matchId, revision: room.revision, version: payload.version,
          rules_digest: payload.rules_digest, seed, users, relay: grants[user.source_index] });
      }
    } else if (operation === 'kick') {
      const target = room.members.find((member) => member.name === payload.name && member.player_id !== room.owner);
      requireThat(target, 'invalid_player', 'That player is not in the room.');
      this.emit(target.session_id, 'kicked', { channel: channelView(room), user: memberView(target, room),
        by: memberView(room.members.find((member) => member.player_id === room.owner), room) });
      this.leave(this.store.get(`session:${target.session_id}`)); return;
    } else throw new ApiError(400, 'unknown_operation', 'Unknown lobby operation.');
    this.store.put(`room:${room.id}`, room);
  }

  reap() {
    const now = this.clock();
    this.store.transaction(() => {
      for (const match of this.store.list('match:')) if (match.expires <= now) {
        if (pendingMatch(match)) {
          this.abortPending(match, match.phase === 'connecting' ? 'A player could not connect to the gameplay relay.' :
            'A player could not complete the peer latency check.');
          continue;
        }
        for (const session of this.store.list('session:').filter((session) => session.match_id === match.id)) {
          session.match_id = null; this.store.put(`session:${session.id}`, session);
          this.emit(session.id, match.phase === 'connecting' ? 'start_rejected' : 'error',
            match.phase === 'connecting' ? { message: 'A player could not connect to the gameplay relay.' } :
              { code: 'match_expired', message: 'The game session expired.' });
        }
        this.store.delete(`match:${match.id}`);
        const room = this.store.get(`room:${match.room_id}`);
        if (room && match.phase === 'connecting') {
          room.phase = 'lobby'; delete room.match_id; this.invalidate(room); this.store.put(`room:${room.id}`, room);
        } else this.deleteRoom(match.room_id);
      }
      this.store.expireRates(now);
      for (const session of this.store.list('session:')) {
        if (session.expires_at <= now || session.seen + 120 <= now) {
          // Polling continues in the HTTPS worker while the game loads/plays.
          // A dead process must not strand its account or the match's final
          // cleanup until the two-hour relay credential expires. Gameplay
          // disconnect/timeout decisions remain with the peer simulation.
          if (session.match_id) this.finish(session);
          const current = this.store.get(`session:${session.id}`);
          if (current) this.leave(current);
          this.store.delete(`session:${session.id}`); this.store.deleteEvents(session.id);
        }
      }
    });
  }
}
