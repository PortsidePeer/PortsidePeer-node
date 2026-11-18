// Pure marker-protocol helpers. No runes, no component state —
// anything that lives in +page.svelte state arrives as a parameter.

import type {
  ChatMessage,
  Room,
  RoomEventPayload,
  ReactionActor,
  ReactionPayload,
  AvatarPayload,
  FileInfo,
  KlipyGif,
  ReplyInfo,
  SharedFileInfo
} from '$lib/types';

// Wire markers
export const REACT_MARKER = 'P2P_REACT:';
export const ROOM_MARKER = 'P2P_ROOM:';
export const REPLY_MARKER = 'P2P_REPLY:';
export const GIF_MARKER = 'P2P_MEDIA_GIF:';
export const FILE_MARKER = 'P2P_MEDIA_FILE:';
export const IMAGE_MARKER = 'P2P_MEDIA_IMAGE:';
export const AVATAR_MARKER = 'P2P_AVATAR:';

// Identity / formatting

export function senderName(msg: ChatMessage): string {
  const s = msg?.sender;
  return typeof s === 'string' && s.trim() ? s : 'Unknown';
}

export function senderInitials(msg: ChatMessage): string {
  const parts = senderName(msg).split(/\s+/).filter(Boolean);
  if (parts.length === 0) return '?';
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

export function shortPeer(pid: string, len = 6): string {
  if (typeof pid !== 'string' || !pid) return '';
  if (pid.length <= len + 5) return pid;
  return `${pid.slice(0, len)}…${pid.slice(-4)}`;
}

export function slugify(text: string): string {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

export function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

export function formatBytes(bytes: number | undefined | null): string {
  if (typeof bytes !== 'number' || !Number.isFinite(bytes) || bytes <= 0) return '';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const v = bytes / 1024 ** i;
  return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

export function stripVariation(emoji: string): string {
  return emoji.replace(/[\uFE0E\uFE0F]/g, '');
}

export function gifTileUrl(gif: KlipyGif | null | undefined): string {
  return gif?.file?.hd?.gif?.url || gif?.file?.sd?.gif?.url || '';
}

// Keys

export function messageKey(msg: ChatMessage): string {
  if (typeof msg?.uid === 'string' && msg.uid) return `uid:${msg.uid}`;
  return `legacy:${senderName(msg)}::${displayBody(msg)}`;
}

export function roomIsVisible(room: string, activeRoom: string): boolean {
  return room === activeRoom && document.hasFocus();
}

export function roomOf(msg: ChatMessage): string {
  return typeof msg?.room === 'string' && msg.room ? msg.room : 'sbb-lounge';
}

export function activeRoomNameFor(rooms: Room[], activeRoom: string): string {
  const found = rooms.find((r) => r.id === activeRoom);
  return found?.name ?? activeRoom;
}

// Body parsing

export function displayBody(msg: ChatMessage): string {
  const raw = typeof msg?.body === 'string' ? msg.body : '';
  const sender = typeof msg?.sender === 'string' ? msg.sender.trim() : '';
  if (sender && raw.startsWith(`${sender}: `)) {
    return raw.slice(`${sender}: `.length);
  }
  return raw;
}

export function replyParentOf(key: string, messages: ChatMessage[]): ChatMessage | null {
  if (typeof key !== 'string' || !key) return null;
  if (key.startsWith('uid:')) {
    const uid = key.slice(4);
    return messages.find((m) => m.uid === uid) ?? null;
  }
  if (key.startsWith('legacy:')) {
    const raw = key.slice(7);
    const idx = raw.indexOf('::');
    if (idx === -1) return null;
    return (
      messages.find(
        (m) =>
          !m.uid &&
          senderName(m) === raw.slice(0, idx) &&
          displayBody(m) === raw.slice(idx + 2)
      ) ?? null
    );
  }
  return null;
}

export function replyPreviewOf(msg: ChatMessage): string {
  const body = displayBody(msg);
  if (extractGifUrl(body)) return 'a GIF';
  if (extractImageUrl(body)) return 'an image';
  if (extractFileInfo(body)) return 'a file';
  const { parentKey, text } = displayReply(body);
  if (parentKey !== null) return text.slice(0, 80) || 'a reply';
  return body.slice(0, 80);
}

export function displayReply(body: string): ReplyInfo {
  if (typeof body !== 'string' || !body.startsWith(REPLY_MARKER)) {
    return { parentKey: null, text: typeof body === 'string' ? body : '' };
  }
  const rest = body.slice(REPLY_MARKER.length);
  if (rest.startsWith('{')) {
    let end = rest.indexOf('}');
    while (end !== -1) {
      try {
        const parsed = JSON.parse(rest.slice(0, end + 1)) as { t?: unknown } | null;
        return {
          parentKey: parsed && typeof parsed.t === 'string' && parsed.t ? parsed.t : null,
          text: rest.slice(end + 1).replace(/^\s+/, '')
        };
      } catch {
        end = rest.indexOf('}', end + 1);
      }
    }
  }
  return { parentKey: null, text: rest };
}

export function displayText(msg: ChatMessage): string {
  const body = displayBody(msg);
  if (extractGifUrl(body)) return 'a GIF';
  if (extractImageUrl(body)) return 'an image';
  if (extractFileInfo(body)) return 'a file';
  return displayReply(body).text;
}

export function previewFor(msg: ChatMessage): string {
  const body = displayBody(msg);
  if (extractGifUrl(body)) return 'Sent a GIF';
  if (extractImageUrl(body)) return 'Sent an image';
  if (extractImageInfo(body)) return 'Sent an image';
  const fileInfo = extractFileInfo(body);
  if (fileInfo) return `Sent a file: ${fileInfo.name}`;
  const { parentKey, text } = displayReply(body);
  if (parentKey !== null) return `↩ ${text.slice(0, 80) || 'Sent a reply'}`;
  return body.slice(0, 80) || 'Sent a message';
}

export function previewOf(msg: ChatMessage, _messages: ChatMessage[]): string {
  return replyPreviewOf(msg);
}

export function messageByKey(key: string, messages: ChatMessage[]): ChatMessage | null {
  if (typeof key !== 'string' || !key) return null;
  if (key.startsWith('uid:')) {
    const uid = key.slice(4);
    return messages.find((m) => m.uid === uid) ?? null;
  }
  if (key.startsWith('legacy:')) {
    const raw = key.slice(7);
    const idx = raw.lastIndexOf('::');
    if (idx === -1) return null;
    return (
      messages.find(
        (m) => !m.uid && senderName(m) === raw.slice(0, idx) && displayBody(m) === raw.slice(idx + 2)
      ) ?? null
    );
  }
  return null;
}

export function roomEventPayloadOf(msg: ChatMessage): RoomEventPayload | null {
  const body = displayBody(msg);
  if (typeof body !== 'string' || !body.startsWith(ROOM_MARKER)) return null;
  try {
    const parsed = JSON.parse(body.slice(ROOM_MARKER.length));
    if (parsed && typeof parsed === 'object' && typeof parsed.id === 'string' && typeof parsed.name === 'string') {
      return {
        a: typeof parsed.a === 'string' ? parsed.a : undefined,
        id: parsed.id,
        name: parsed.name
      };
    }
    return null;
  } catch {
    return null;
  }
}

export function reactionPayloadOf(msg: ChatMessage): ReactionPayload | null {
  const body = displayBody(msg);
  if (typeof body !== 'string' || !body.startsWith(REACT_MARKER)) return null;
  try {
    const parsed = JSON.parse(body.slice(REACT_MARKER.length));
    return parsed && typeof parsed === 'object' ? (parsed as ReactionPayload) : null;
  } catch {
    return null;
  }
}

export function avatarPayloadOf(msg: ChatMessage): AvatarPayload | null {
  const body = displayBody(msg);
  if (typeof body !== 'string' || !body.startsWith(AVATAR_MARKER)) return null;
  try {
    const parsed = JSON.parse(body.slice(AVATAR_MARKER.length));
    if (
      parsed &&
      typeof parsed === 'object' &&
      typeof parsed.p === 'string' &&
      typeof parsed.d === 'string'
    ) {
      return { p: parsed.p, d: parsed.d };
    }
    return null;
  } catch {
    return null;
  }
}

export function actorOf(payload: ReactionPayload, fallbackName: string): ReactionActor {
  return {
    p: typeof payload.p === 'string' && payload.p ? payload.p : 'unknown',
    n: payload.n ?? fallbackName
  };
}

const IMG_URL_RE = /^https?:\/\/\S+\.(png|jpe?g|webp|bmp|avif)(\?\S*)?$/i;
const GIF_URL_RE = /^https?:\/\/\S+\.gif(\?\S*)?$/i;

export function extractGifUrl(body: string): string | null {
  if (body.startsWith(GIF_MARKER)) {
    const rest = body.slice(GIF_MARKER.length).trim();
    if (/^https?:\/\//i.test(rest)) return rest;          // bare URL payload
    try {
      const raw = JSON.parse(rest) as { url?: unknown };   // {"url": "..."} payload
      if (raw && typeof raw.url === 'string') return raw.url;
    } catch { /* not JSON */ }
    return null;
  }
  const m = body.match(GIF_URL_RE);
  return m ? m[0] : null;
}

export function extractImageUrl(body: string): string | null {
  if (body.startsWith(IMAGE_MARKER)) {
    const rest = body.slice(IMAGE_MARKER.length).trim();
    if (/^(https?:|data:image\/)/i.test(rest)) return rest; // legacy URL/dataURL payload
    return null;                                            // JSON → extractImageInfo's job
  }
  const m = body.match(IMG_URL_RE);
  return m ? m[0] : null;
}

export function isImageFile(name: string): boolean {
  return /\.(png|jpe?g|gif|webp|bmp|avif)$/i.test(name);
}

function extractMediaInfo(body: string): SharedFileInfo | null {
  // Must accept BOTH markers: Rust hardcodes P2P_MEDIA_FILE: on the wire
  // for every send; P2P_MEDIA_IMAGE: is the legacy/older-peer form.
  for (const marker of [FILE_MARKER, IMAGE_MARKER]) {
    if (!body.startsWith(marker)) continue;
    try {
      const raw = JSON.parse(body.slice(marker.length)) as SharedFileInfo;
      if (raw && typeof raw.id === 'string' && typeof raw.name === 'string') return raw;
    } catch { /* not valid JSON → fall through */ }
  }
  return null;
}

export function extractImageInfo(body: string): SharedFileInfo | null {
  const info = extractMediaInfo(body);
  return info && isImageFile(info.name) ? info : null;
}

export function extractFileInfo(body: string): SharedFileInfo | null {
  const info = extractMediaInfo(body);
  return info && !isImageFile(info.name) ? info : null;
}

// Notifications

export function shouldNotify(
  msg: ChatMessage,
  activeRoom: string,
  mutedRooms: Record<string, boolean>,
  selfNickname?: string
): boolean {
  const room = roomOf(msg);
  if (room === activeRoom && document.hasFocus()) return false;
  if (mutedRooms[room]) return false;
  if (selfNickname && senderName(msg) === selfNickname) return false;
  return true;
}
