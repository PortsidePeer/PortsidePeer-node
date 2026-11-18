/**
 * PortsidePeer — typed frontend API.
 *
 * Every invoke() the frontend makes goes through here; +page.svelte stops
 * importing @tauri-apps/api/core directly. Two jobs:
 *   1. one import surface for the UI;
 *   2. event-name constants, so a 'chat-msg' typo fails at compile time.
 *
 * Command names come from this week's session; argument shapes marked ⚠ are
 * best-effort reconstructions — port your real call sites over and keep
 * your names. Tauri v2 maps camelCase JS keys onto snake_case Rust args,
 * which is what the bodies below assume.
 */

import { invoke } from '@tauri-apps/api/core';
import { IMAGE_MARKER, FILE_MARKER, REACT_MARKER } from '$lib/markers';
import type { ChatMessage, FileInfo, PeerInfo, Room, SharedFileInfo } from '$lib/types';

// Rust-emitted events — use these constants in listen(), never raw strings.

export const EVENTS = {
  chatMessage: 'chat-msg',
  fileProgress: 'file-progress',
  fileReceived: 'file-received',
  relayStatus: 'relay-status-changed',
  relayListening: 'relay-listening',
  peerDiscovered: 'peer-discovered',
} as const;

// Chat

/** Fire-and-forget send; the optimistic push happens in the caller. */
export function sendMessage(chatId: string, text: string, replyTo?: string): Promise<void> {
  return invoke('send_chat_message', { chatId, text, replyTo });
}

export function getHistory(chatId: string): Promise<ChatMessage[]> {
  return invoke('get_history', { chatId });
}

// Rooms

export function getRooms(): Promise<Room[]> {
  return invoke('get_rooms');
}

export function createRoom(name: string): Promise<Room> {
  return invoke('create_room', { name });
}

export function renameRoom(roomId: string, name: string): Promise<void> {
  return invoke('rename_room', { roomId, name });
}

// Peers / allow list

export function getAllowList(): Promise<PeerInfo[]> {
  return invoke('get_allow_list');
}

export function acceptPeer(peerId: string): Promise<void> {
  return invoke('accept_peer', { peerId });
}

export function revokePeer(peerId: string): Promise<void> {
  return invoke('revoke_peer', { peerId });
}

// Media & files

/** Resolve a stored image to a servable local path. */
export function getLocalImagePath(fileName: string): Promise<string> {
  return invoke('get_local_image_path', { fileName });
}

/** Persist pasted/dragged media to temp storage. */
export function saveTempMedia(fileName: string, data: Uint8Array): Promise<FileInfo> {
  return invoke('save_temp_media', { fileName, data });
}

/** Progress arrives on EVENTS.fileProgress. */
export function sendFile(chatId: string, path: string): Promise<void> {
  return invoke('send_file', { chatId, path });
}

// Profile

export function updateNickname(nickname: string): Promise<void> {
  return invoke('update_nickname', { nickname });
}

// Config

export function getKlipyKey(): Promise<string | null> {
  return invoke('get_klipy_key');
}

export function saveKlipyKey(key: string): Promise<void> {
  return invoke('save_klipy_key', { key });
}

export function clearKlipyKey(): Promise<void> {
  return invoke('clear_klipy_key');
}

export function setRelayAddress(address: string): Promise<void> {
  return invoke('set_relay_address', { address });
}

export function isImageFile(name: string): boolean {
  return /\.(png|jpe?g|gif|webp|bmp|avif)$/i.test(name);
}

export function fileUrlToPath(line: string): string | null {
  if (!line.startsWith('file://')) return null;
  try {
    return decodeURIComponent(line.slice('file://'.length));
  } catch {
    return line.slice('file://'.length);
  }
}

export function buildMediaBody(info: SharedFileInfo): string {
  // send_file always broadcasts P2P_MEDIA_FILE: on the wire (hardcoded in Rust),
  // so the local echo must match or sender and receiver render differently.
  return `${FILE_MARKER}${JSON.stringify(info)}`;
}

export async function shareTempFile(
  file: File,
  room: string
): Promise<{ uid: string; path: string; info: SharedFileInfo }> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const ext = (file.name?.split('.').pop() ?? 'png').toLowerCase();
  const uid = crypto.randomUUID();
  const path = await invoke<string>('save_temp_media', { data: Array.from(bytes), ext });
  const info = await invoke<SharedFileInfo>('send_file', { path, uid, room });
  return { uid, path, info };
}
