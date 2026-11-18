<script lang="ts">

import type {
  ChatMessage,
  Room,
  FileInfo,
  ReactionActor,
  FileProgressInfo,
  ReactTarget,
  SharedFileInfo
} from '$lib/types';

import {
  REACT_MARKER, ROOM_MARKER, REPLY_MARKER, FILE_MARKER, AVATAR_MARKER,
  messageKey, displayBody, previewFor, previewOf, replyPreviewOf,
  roomEventPayloadOf, reactionPayloadOf, avatarPayloadOf, actorOf, extractImageInfo,
  senderName, roomOf, roomIsVisible, activeRoomNameFor, shouldNotify
} from '$lib/markers';

import { shareTempFile, buildMediaBody, fileUrlToPath } from '$lib/api';

import MessageItem from '$lib/components/MessageItem.svelte';

import Composer from '$lib/components/Composer.svelte';

import RoomSidebar from '$lib/components/RoomSidebar.svelte';

  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { convertFileSrc } from '@tauri-apps/api/core';

  import {
    sendNotification
  } from '@tauri-apps/plugin-notification';

  let messages = $state<ChatMessage[]>([]);
  let userNickname = $state('Loading...');
  let userPeerId = $state('');

  // Notifications
  let notificationsEnabled = $state(true);
  let mutedRooms = $state<Record<string, boolean>>({});
  let unreadByRoom = $state<Record<string, number>>({});

  let allowedPeers = $state<string[]>([]);
  let discoveredPeers = $state<string[]>([]);

  let showEmojiPicker = $state(false);
  let emojiMode = $state<'compose' | 'react'>('compose');

  let relaySaved = $state(false);
  let relayConnected = $state(false);
  let relayListening = $state(false);
  let myCircuitAddress = $state('');
  let reactions = $state<Record<string, Record<string, ReactionActor[]>>>({});
  let reactionPickerFor = $state<string | null>(null);
  let reactionPickerFlip = $state(false);
  let recentEmoji  = $state<string[]>([]);

  const RECENT_EMOJI_KEY = 'portsidepeer-recent-emoji';
  const RECENT_EMOJI_MAX = 16;

  let reactTarget = $state<ReactTarget | null>(null);

  let sentFiles = $state<Record<string, string>>({});
  let receivedFiles = $state<Record<string, string>>({});
  let fileProgress = $state<Record<string, FileProgressInfo>>({});

  let klipyApiKey = $state('');
  let klipySaved = $state(false);

  let messageLogEl = $state<HTMLDivElement | null>(null);
  let messageLogInnerEl = $state<HTMLDivElement | null>(null);
  let composer = $state<Composer>();
  let initialJumpDone = false;
  let lastAutoScrollAt = 0;
  let pinnedToBottom = true;

  const NEAR_BOTTOM_PX = 200;

  // Rooms
  let rooms = $state<Room[]>([]);
  let activeRoom = $state('sbb-lounge');
  let roomNameInput = $state('');
  let editingRoom = $state(false);

  type ReplyTarget = {
    key: string;
    name: string;
    preview: string;
  };

  let replyTo = $state<ReplyTarget | null>(null);

  function startReply(msg: ChatMessage) {
    replyTo = {
      key: messageKey(msg),
      name: senderName(msg),
      preview: replyPreviewOf(msg)
    };
    showEmojiPicker = false;
    composer?.focusInput();
  }

  function cancelReply() {
    replyTo = null;
  }

  let imageSrcs = $state<Record<string, string>>({});

  async function resolveImageSrc(info: FileInfo) {
    if (imageSrcs[info.id]) return;

    if (sentFiles[info.id]) {
      imageSrcs = { ...imageSrcs, [info.id]: convertFileSrc(sentFiles[info.id]) };
      return;
    }

    try {
      const path = await invoke<string>('get_local_image_path', { transferId: info.id, name: info.name });
      imageSrcs = { ...imageSrcs, [info.id]: convertFileSrc(path) };
    } catch {
    }
  }

  const NOTIF_PREFS_KEY = 'portsidepeer-notification-prefs';

  function loadNotificationPrefs() {
    try {
      const prefs = JSON.parse(localStorage.getItem(NOTIF_PREFS_KEY) ?? '{}');
      notificationsEnabled = typeof prefs.enabled === 'boolean' ? prefs.enabled : true;
      mutedRooms = typeof prefs.muted === 'object' && prefs.muted ? prefs.muted : {};
    } catch { }
  }

  function saveNotificationPrefs() {
    try {
      localStorage.setItem(
        NOTIF_PREFS_KEY,
        JSON.stringify({ enabled: notificationsEnabled, muted: mutedRooms })
      );
    } catch { }
  }

  function toggleNotifications() {
    notificationsEnabled = !notificationsEnabled;
    saveNotificationPrefs();
  }

  function toggleRoomMute(roomId: string) {
    mutedRooms = { ...mutedRooms, [roomId]: !mutedRooms[roomId] };
    saveNotificationPrefs();
  }

  function switchRoom(id: string) {
    activeRoom = id;
    scrollToBottom('auto');
    if (unreadByRoom[id]) {
      unreadByRoom = { ...unreadByRoom, [id]: 0 };
    }
  }

  function loadRecentEmoji() {
    try {
      const saved = JSON.parse(localStorage.getItem(RECENT_EMOJI_KEY) ?? '[]');
      recentEmoji = Array.isArray(saved)
        ? saved.filter((e) => typeof e === 'string').slice(0, RECENT_EMOJI_MAX)
        : [];
    } catch { }
  }

  function saveRecentEmoji() {
    try {
      localStorage.setItem(RECENT_EMOJI_KEY, JSON.stringify(recentEmoji));
    } catch { }
  }

  function pushRecentEmoji(emoji: string) {
    recentEmoji = [emoji, ...recentEmoji.filter((e) => e !== emoji)].slice(0, RECENT_EMOJI_MAX);
    saveRecentEmoji();
  }

  function notifyNewMessage(msg: ChatMessage) {
    const room = roomOf(msg);

    if (senderName(msg) !== userNickname && !roomIsVisible(room, activeRoom)) {
      unreadByRoom = { ...unreadByRoom, [room]: (unreadByRoom[room] ?? 0) + 1 };
    }

    if (!shouldNotify(msg, activeRoom, mutedRooms, userNickname)) return;

    try {
      sendNotification({
        title: `${senderName(msg)} • #${activeRoomNameFor(rooms, activeRoom)}`,
        body: previewFor(msg),
      });
    } catch (err) {
      console.error('[portsidepeer] notify failed', err);
    }
  }

  function activeRoomName() {
    return rooms.find((c) => c.id === activeRoom)?.name ?? activeRoom;
  }

  const AVATAR_MAX_BYTES = 32 * 1024;
  let avatars = $state<Record<string, string>>({});

  async function pickAvatar() {
    try {
      const picked = await openFileDialog({
        multiple: false,
        title: 'Choose an avatar',
        filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }]
      });
      if (!picked) return;

      const dataUrl = await resizeToAvatar(picked);
      if (!dataUrl) { alert('Image too large even after resizing — try a smaller one'); return; }

      await invoke('send_chat_message', {
        message: `${AVATAR_MARKER}${JSON.stringify({ p: userPeerId, n: userNickname, d: dataUrl.split(',')[1] })}`,
        uid: crypto.randomUUID(),
        room: activeRoom
      });

      avatars = { ...avatars, [userPeerId]: dataUrl };
    } catch (err) {
      alert(`Avatar failed: ${err}`);
    }
  }

  async function resizeToAvatar(path: string) {

    const src = convertFileSrc(path);
    const img = await new Promise<HTMLImageElement>((res, rej) => {
      const i = new Image();
      i.crossOrigin = 'anonymous';
      i.onload = () => res(i);
      i.onerror = rej;
      i.src = src;
    });
    const canvas = document.createElement('canvas');
    canvas.width = 256; canvas.height = 256;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('canvas 2d context unavailable');

    const side = Math.min(img.width, img.height);
    ctx.drawImage(img, (img.width - side) / 2, (img.height - side) / 2, side, side, 0, 0, 256, 256);

    for (const q of [0.85, 0.7, 0.5, 0.35]) {
      const url = canvas.toDataURL('image/jpeg', q);
      if (url.length < AVATAR_MAX_BYTES) return url;
    }
    return null;
  }

  function applyAvatarEvent(msg: ChatMessage) {
    const payload = avatarPayloadOf(msg);
    if (!payload) return;
    if (msg.sender_pid && payload.p !== msg.sender_pid) return;
    avatars = { ...avatars, [payload.p]: `data:image/jpeg;base64,${payload.d}` };
  }

  async function applyRoomEvent(msg: ChatMessage) {
    const payload = roomEventPayloadOf(msg);
    if (!payload) return;
    if (payload.a === 'create' && !rooms.some((c) => c.id === payload.id)) {
      try {
        const info = await invoke<Room>('create_room', { id: payload.id, name: payload.name });
        rooms = [...rooms, info];
      } catch {
        rooms = [...rooms.filter((c) => c.id !== payload.id), { id: payload.id, name: payload.name }];
      }
    } else if (payload.a === 'rename') {
      try {
        await invoke('rename_room', { id: payload.id, name: payload.name });
        rooms = rooms.map((c) => (c.id === payload.id ? { ...c, name: payload.name } : c));
      } catch (err) {
        console.error('[accord] room rename failed', err);
      }
    }
  }

  async function createRoom(id: string, name: string): Promise<boolean> {
    try {
      const info = await invoke<Room>('create_room', { id, name });
      rooms = [...rooms, info];
      await invoke('send_chat_message', {
        message: `${ROOM_MARKER}${JSON.stringify({ a: 'create', id, name })}`,
        uid: crypto.randomUUID(),
        room: activeRoom
      });
      activeRoom = id;
      return true;
    } catch (err) {
      alert(err);
      return false;
    }
  }

  async function renameRoom(e: SubmitEvent) {
    e.preventDefault();
    const name = roomNameInput.trim();
    if (!name) return;
    const id = activeRoom;
    try {
      await invoke('rename_room', { id, name });
      rooms = rooms.map((c) => (c.id === id ? { ...c, name } : c));
      await invoke('send_chat_message', {
        message: `${ROOM_MARKER}${JSON.stringify({ a: 'rename', id, name })}`,
        uid: crypto.randomUUID(),
        room: id
      });
      editingRoom = false;
    } catch (err) {
      alert(err);
    }
  }

  const visibleMessages = $derived.by(() =>
    messages.filter((m) => roomOf(m) === activeRoom)
  );

  $effect(() => {
    const count = visibleMessages.length;
    if (count === 0) return;

    if (!initialJumpDone) {
      scrollToBottom('smooth');
      initialJumpDone = true;
      return;
    }

    const lastMsg = visibleMessages[count - 1];
    const isOwnMessage = lastMsg?.sender === userNickname;
    const stillAnimating = Date.now() - lastAutoScrollAt < 900;

    if (isOwnMessage || stillAnimating || pinnedToBottom) {
      scrollToBottom('smooth');
    }
  });

  function isNearBottom() {
    if (!messageLogEl) return true;
    return (
      messageLogEl.scrollHeight - messageLogEl.scrollTop - messageLogEl.clientHeight <
      NEAR_BOTTOM_PX
    );
  }

  function scrollToBottom(behavior: ScrollBehavior = 'auto') {
    if (!messageLogEl) return;
    lastAutoScrollAt = Date.now();
    messageLogEl.scrollTo({ top: messageLogEl.scrollHeight, behavior });
  }

  function handleMessageLogScroll() {
    pinnedToBottom = isNearBottom();
  }

  $effect(() => {
    const inner = messageLogInnerEl;
    if (!inner || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => {
      if (pinnedToBottom && initialJumpDone) scrollToBottom('smooth');
    });
    observer.observe(inner);
    return () => observer.disconnect();
  });

$effect(() => {
   activeRoom;
   scrollToBottom('smooth');
 });

$effect(() => {
 for (const msg of visibleMessages) {
   const info = extractImageInfo(displayBody(msg));
   if (info && !imageSrcs[info.id] && (sentFiles[info.id] || receivedFiles[info.id])) {
     resolveImageSrc(info);
   }
 }
});

  async function attachFile() {
    try {
      const picked = await openFileDialog({ multiple: false, title: 'Share a file' });
      if (!picked) return;
      const uid = crypto.randomUUID();
      const info = await invoke('send_file', { path: picked, uid, room:activeRoom });
      sentFiles = { ...sentFiles, [uid]: picked };
      messages = [...messages, { uid, sender_pid: userPeerId, room: activeRoom, sender: userNickname, body: `${FILE_MARKER}${JSON.stringify(info)}` }];
    } catch (err) {
      alert(`File share failed: ${err}`);
    }
  }

  async function openSharedFile(info: FileInfo) {
    try {
      if (sentFiles[info.id]) {
        await invoke('open_local_file', { path: sentFiles[info.id] });
      } else if (receivedFiles[info.id]) {
        await invoke('open_shared_file', { transferId: info.id, name: info.name });
      }
    } catch (err) {
      alert(err);
    }
  }

  function me() {
    return userPeerId
      ? { p: userPeerId, n: userNickname }
      : { p: `nick:${userNickname}`, n: userNickname };
  }

  function setReactionUsers(target: string, emoji: string, users: ReactionActor[]) {
    const forMessage = { ...(reactions[target] ?? {}) };
    if (users.length > 0) forMessage[emoji] = users;
    else delete forMessage[emoji];

    const next = { ...reactions };
    if (Object.keys(forMessage).length > 0) next[target] = forMessage;
    else delete next[target];
    reactions = next;
  }

  function applyReaction(actor: ReactionActor, target: string, emoji: string, action: 'add' | 'remove') {
    const users = reactions[target]?.[emoji] ?? [];
    if (action === 'add') {
      if (!users.some((u) => u.p === actor.p)) setReactionUsers(target, emoji, [...users, actor]);
    } else if (action === 'remove') {
      setReactionUsers(target, emoji, users.filter((u) => u.p !== actor.p));
    }
  }

  function applyReactionEvent(msg: ChatMessage) {
    const payload = reactionPayloadOf(msg);
    if (payload && payload.t && payload.e) {
      applyReaction(actorOf(payload, senderName(msg)), payload.t, payload.e, payload.a ?? 'add');
    }
  }

  async function toggleReaction(target: string, emoji: string) {
    const users = reactions[target]?.[emoji] ?? [];
    const alreadyMine = users.some((u) => u.p === me().p);
    const action = alreadyMine ? 'remove' : 'add';

    applyReaction(me(), target, emoji, action);

    try {
      await invoke('send_chat_message', {
        message: `${REACT_MARKER}${JSON.stringify({ t: target, e: emoji, a: action, p: me().p, n: userNickname })}`
      });
    } catch (err) {
      console.error('[portsidepeer] reaction send failed', err);
      applyReaction(me(), target, emoji, alreadyMine ? 'add' : 'remove');
    }
  }

  let failedMediaUrls = $state<string[]>([]);

  function handleMediaError(url: string) {
    console.warn('[portsidepeer] media failed to load (CSP-blocked, offline, or dead URL):', url);
    if (!failedMediaUrls.includes(url)) failedMediaUrls = [...failedMediaUrls, url];
  }

  async function shareGif(gifUrl: string) {
    if (!gifUrl) return;
    const uid = crypto.randomUUID();
    const specializedGifPayload = `P2P_MEDIA_GIF:${gifUrl}`;
    messages = [...messages, { uid, sender_pid: userPeerId, room: activeRoom, sender: userNickname, body: specializedGifPayload }];
    await invoke('send_chat_message', { message: specializedGifPayload, uid, room: activeRoom });
  }

  async function saveRelay(peerId: string, address: string): Promise<boolean> {
    try {
      await invoke('save_relay_endpoint', { peerId, address });
      relaySaved = true;
      alert("Relay endpoint saved!\n\nPlease restart the app to connect.");
      return true;
    } catch (err) {
      alert(`Failed to save relay: ${err}`);
      return false;
    }
  }

  async function clearRelay(): Promise<boolean> {
    try {
      await invoke('clear_relay_endpoint');
      relaySaved = false;
      alert("Relay cleared.\n\nPlease restart to disable.");
      return true;
    } catch (err) {
      alert(`Failed to clear relay: ${err}`);
      return false;
    }
  }

  async function saveKlipyKey(apiKey: string): Promise<boolean> {
    try {
      await invoke('save_klipy_key', { apiKey });
      klipyApiKey = apiKey;
      klipySaved = true;
      return true;
    } catch (err) {
      alert(err);
      return false;
    }
  }

  async function clearKlipyKey() {
    try {
      await invoke('clear_klipy_key');
      klipyApiKey = '';
      klipySaved = false;
    } catch (err) {
      alert(err);
    }
  }

  onMount(() => {
    let unlisteners: Array<() => void> = [];
    let unlistenDrag: (() => void) | null = null;

    async function setupDragDrop() {
      const webview = getCurrentWebview();
      unlistenDrag = await webview.onDragDropEvent((event) => {
        if (event.payload.type === 'drop') {
          for (const path of event.payload.paths) {
            shareLocalPathAsFile(path);
          }
        }
      });
    }

    setupDragDrop()

    const pasteListener = (e: ClipboardEvent) => handlePaste(e);
    window.addEventListener('paste', pasteListener);

    const safe = async (label: string, fn: () => any) => {
      try {
        return await fn();
      } catch (err) {
        console.error(`[portsidepeer] init step failed: ${label}`, err);
        return null;
      }
    };

    const addListener = async (name: string, handler: (event: { payload: any }) => void) => {
      try {
        unlisteners.push(await listen(name, handler));
      } catch (err) {
        console.error(`[portsidepeer] listener failed: ${name}`, err);
      }
    };

    async function initializeApp() {
      loadNotificationPrefs();
      loadRecentEmoji();
      const profile = await safe('get_profile', () => invoke('get_profile'));
      if (profile) {
        userNickname = profile.nickname ?? 'Guest';
        userPeerId = profile.peer_id ?? '';
      } else {
        userNickname = 'Guest';
      }

      const list = await safe('get_allow_list', () => invoke('get_allow_list'));
      allowedPeers = Array.isArray(list) ? list : [];

      const chans = await safe('get_rooms', () => invoke('get_rooms'));
      rooms = Array.isArray(chans) && chans.length ? chans : [{ id: 'sbb-lounge', name: 'sbb-lounge' }];
      activeRoom = rooms[0].id;

      const historicalLogs = await safe('get_chat_history', () => invoke('get_chat_history'));
      const allLogs = Array.isArray(historicalLogs) ? historicalLogs : [];

      reactions = {};
      const chatOnly = [];
      for (const m of allLogs) {
        if (!m || typeof m !== 'object') continue;
        if (reactionPayloadOf(m)) { applyReactionEvent(m); continue; }
        if (roomEventPayloadOf(m)) { applyRoomEvent(m); continue; }
        if (avatarPayloadOf(m)) { applyAvatarEvent(m); continue; }
        chatOnly.push(m);
      }
      messages = chatOnly;

      const savedRelay = await safe('get_relay_endpoint', () => invoke('get_relay_endpoint'));
        if (savedRelay) {
          relaySaved = Boolean(savedRelay.peer_id && savedRelay.address);
        }

      const relayStatus = await safe('get_relay_status', () => invoke('get_relay_status'));
      if (relayStatus) {
        relayConnected = Boolean(relayStatus.connected);
        relayListening = Boolean(relayStatus.listening_via_relay);
      }

      const klipyKey = await safe('get_klipy_key', () => invoke('get_klipy_key'));
      klipyApiKey = typeof klipyKey === 'string' ? klipyKey : '';
      klipySaved = Boolean(klipyApiKey);

      await addListener('relay-status-changed', (event) => {
        relayConnected = Boolean(event.payload?.connected);
        relayListening = Boolean(event.payload?.listening_via_relay);
      });

      await addListener('relay-listening', (event) => {
        if (typeof event.payload === 'string') myCircuitAddress = event.payload;
        relayListening = true;
      });
      const received = await safe('get_received_files', () => invoke('get_received_files'));
      receivedFiles = received && typeof received === 'object' ? received : {};

      await addListener('file-progress', (event) => {
        const p = event.payload;
        if (p?.id) fileProgress = { ...fileProgress, [p.id]: p };
      });

      await addListener('file-received', (event) => {
        const p = event.payload;
        if (!p?.id) return;
        receivedFiles = { ...receivedFiles, [p.id]: p };
        const next = { ...fileProgress };
        delete next[p.id];
        fileProgress = next;

        if (imageSrcs[p.id] === undefined) {
          const msg = messages.find((m) => {
            const info = extractImageInfo(displayBody(m));
            return info?.id === p.id;
          });
          if (msg) {
            const info = extractImageInfo(displayBody(msg));
            if (info) resolveImageSrc(info);
          }
        }
      });

      await addListener('chat-msg', (event) => {
        const incoming = event.payload;
        if (incoming && typeof incoming === 'object') {
          if (incoming.uid && messages.some((m) => m.uid === incoming.uid)) return;
          notifyNewMessage(incoming);
          if (reactionPayloadOf(incoming)) { applyReactionEvent(incoming); return; }
          if (roomEventPayloadOf(incoming)) { applyRoomEvent(incoming); return; }
          if (avatarPayloadOf(incoming)) { applyAvatarEvent(incoming); return; }
          messages = [...messages, incoming];
        }
      });

      window.addEventListener('focus', () => {
        unreadByRoom = { ...unreadByRoom, [activeRoom]: 0 };
      });

      await addListener('peer-discovered', (event) => {
        const foundId = event.payload?.peer_id;
        if (foundId && foundId !== userPeerId && !allowedPeers.includes(foundId) && !discoveredPeers.includes(foundId)) {
          discoveredPeers = [...discoveredPeers, foundId];
        }
      });
    }

    initializeApp();

    return () => {
      unlisteners.forEach(fn => fn?.());
      window.removeEventListener('paste', pasteListener);
      unlistenDrag?.();
    };
  });

  async function sendMessage(text: string) {
    const raw = text.trim();
    if (raw.startsWith('file://')) {
      replyTo = null;
      showEmojiPicker = false;
      for (const line of raw.split(/\r?\n/)) {
        const path = fileUrlToPath(line.trim());
        if (path) void shareLocalPathAsFile(path);
      }
      return;
    }
    if (!raw) return;
    const uid = crypto.randomUUID();
    const rawBody = text;
    const outgoingBody = replyTo
      ? `${REPLY_MARKER}${JSON.stringify({ t: replyTo.key })}\n${rawBody}`
      : rawBody;
    messages = [...messages, { uid, sender_pid: userPeerId, room: activeRoom, sender: userNickname, body: outgoingBody }];
    await invoke('send_chat_message', { message: outgoingBody, uid, room: activeRoom });
    replyTo = null;
    showEmojiPicker = false;
  }

  async function saveNickname(newName: string): Promise<boolean> {
    try {
      const updatedProfile = await invoke<{ nickname: string }>('update_nickname', { newName });
      userNickname = updatedProfile.nickname;
      return true;
    } catch (err) {
      alert(err);
      return false;
    }
  }

  async function acceptPeer(peerId: string) {
    try {
      const updatedList = await invoke('add_to_allow_list', { targetFriendPeerId: peerId });

      allowedPeers = Array.isArray(updatedList) ? updatedList : [];
      discoveredPeers = discoveredPeers.filter(p => p !== peerId);
    } catch (err) {
      alert(err);
    }
  }

  async function revokeAccess(peerId: string) {
    try {
      const updatedList = await invoke('remove_from_allow_list', { targetPeerId: peerId });
      allowedPeers = Array.isArray(updatedList) ? updatedList : [];
    } catch (err) {
      alert(err);
    }
  }

  function handlePaste(e: ClipboardEvent) {
    const dt = e.clipboardData;
    if (!dt) return;

    const items = Array.from(dt.items ?? []);
    const imageItem = items.find((i) => i.type.startsWith('image/'));
    const files = Array.from(dt.files ?? []);

    // Image data or any clipboard File object
    const file = imageItem?.getAsFile() ?? files[0];
    if (file) {
      e.preventDefault();
      void shareBlobAsFile(file);
      return;
    }

    // On WebKitGTK neither branch below fires (getData() is broken there),
    // so no preventDefault → the file:/// URI is pasted into the input
    // as text natively, and sendMessage routes it on send.

    const uriList = dt.getData('text/uri-list');
       if (uriList) {
         const paths = uriList
           .split(/\r?\n/)
           .map((l) => l.trim())
           .filter((l) => l && !l.startsWith('#') && l.startsWith('file://'))
           .map(fileUrlToPath)
           .filter((p): p is string => p !== null);
         if (paths.length > 0) {
           e.preventDefault();
           for (const p of paths) void shareLocalPathAsFile(p);
           return;
         }
       }

       // Bare file:// path as plain text (non-WebKitGTK engines)
       const line = (dt.getData('text/plain') ?? '').trim().split(/\r?\n/)[0] ?? '';
       if (line.startsWith('file://')) {
         const path = fileUrlToPath(line);
         if (path !== null) {
           e.preventDefault();
           void shareLocalPathAsFile(path);
         }
       }
     }

  async function shareLocalPathAsFile(path: string) {
    try {
      const uid = crypto.randomUUID();
      const info = await invoke<SharedFileInfo>('send_file', { path, uid, room: activeRoom });
      sentFiles = { ...sentFiles, [uid]: path };
      messages = [...messages, {
        uid,
        sender_pid: userPeerId,
        room: activeRoom,
        sender: userNickname,
        body: buildMediaBody(info),
      }];
    } catch (err) {
      console.error('[portsidepeer] file path share failed:', err);
      alert(`Share failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  async function shareBlobAsFile(file: File) {
    if (!file) return;
    try {
      const { uid, path, info } = await shareTempFile(file, activeRoom);
      sentFiles = { ...sentFiles, [uid]: path };
      messages = [...messages, {
        uid,
        sender_pid: userPeerId,
        room: activeRoom,
        sender: userNickname,
        body: buildMediaBody(info),
      }];
    } catch (err) {
      console.error('[portsidepeer] paste share failed:', err);
      alert(`Share failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  async function handleDrop(e: DragEvent) {
    console.log('[drop-debug] drop fired, files:', e.dataTransfer?.files?.length);
    e.preventDefault();
    const files = Array.from(e.dataTransfer?.files ?? []);
    for (const file of files) {
      await shareBlobAsFile(file);
    }
  }

  function openFullReactionPicker(msg: ChatMessage) {
    reactTarget = { key: messageKey(msg), preview: previewOf(msg, messages) };
    emojiMode = 'react';
    reactionPickerFor = null;
    showEmojiPicker = true;
  }

</script>

<main class="app-layout">

    <RoomSidebar
      userPeerId={userPeerId}
      userNickname={userNickname}
      myAvatar={avatars[userPeerId] ?? null}
      relaySaved={relaySaved}
      relayConnected={relayConnected}
      relayListening={relayListening}
      myCircuitAddress={myCircuitAddress}
      rooms={rooms}
      activeRoom={activeRoom}
      unreadByRoom={unreadByRoom}
      mutedRooms={mutedRooms}
      notificationsEnabled={notificationsEnabled}
      klipySaved={klipySaved}
      discoveredPeers={discoveredPeers}
      allowedPeers={allowedPeers}
      onSwitchRoom={switchRoom}
      onToggleRoomMute={toggleRoomMute}
      onToggleNotifications={toggleNotifications}
      onDisableRelay={clearRelay}
      onClearKlipyKey={clearKlipyKey}
      onAcceptPeer={acceptPeer}
      onRevokePeer={revokeAccess}
      onPickAvatar={pickAvatar}
      onSaveRelay={saveRelay}
      onSaveKlipyKey={saveKlipyKey}
      onGrantPeer={acceptPeer}
      onRenameProfile={saveNickname}
      onCreateRoom={createRoom}
    />

  <section class="main-workspace">
    <header class="app-header">
      <div class="server-info">
          {#if editingRoom}
            <form onsubmit={renameRoom} class="room-rename-form">
              <input bind:value={roomNameInput} class="edit-input" />
              <button type="submit" class="save-btn">✓</button>
              <button type="button" onclick={() => (editingRoom = false)} class="cancel-btn">✕</button>
            </form>
          {:else}
            <button
              type="button"
              class="room-title-btn"
              onclick={() => { roomNameInput = activeRoomName(); editingRoom = true; }}
            >
              <span class="hash-tag">#</span>
              <span class="room-title-text">{activeRoomName()}</span>
              <span class="rename-hint" aria-hidden="true">✏️</span>
            </button>
          {/if}
        <span class="status-indicator">Private Mesh Active</span>
      </div>
    </header>

    <div class="chat-container">
      <div class="message-log"
          role="log"
          aria-label="Chat messages. Drop files here to share them"
          bind:this={messageLogEl}
          onscroll={handleMessageLogScroll}
          ondragover={(e) => e.preventDefault()}
          ondrop={handleDrop}
      >
     <div class="message-log-inner" bind:this={messageLogInnerEl}>
            {#if visibleMessages.length === 0}
              <div class="welcome-card">
                <h1>Welcome to #{activeRoomName()}!</h1>
              <p>The file-broker link is running. Use the GIF launcher tab to share expressions.</p>
            </div>
          {/if}
          {#each visibleMessages as msg}
              <MessageItem
                  {msg}
                  {messages}
                  {userNickname}
                  {userPeerId}
                  {avatars}
                  {reactions}
                  {imageSrcs}
                  {fileProgress}
                  {sentFiles}
                  {receivedFiles}
                  {failedMediaUrls}
                  {recentEmoji}
                  {reactionPickerFor}
                  {reactionPickerFlip}
                  {messageLogEl}
                  onPickerFor={(k) => (reactionPickerFor = k)}
                  onPickerFlip={(b) => (reactionPickerFlip = b)}
                  onReply={startReply}
                  onToggleReaction={toggleReaction}
                  onPushRecentEmoji={pushRecentEmoji}
                  onOpenSharedFile={openSharedFile}
                  onMediaError={handleMediaError}
                  onOpenFullPicker={openFullReactionPicker}
                />
          {/each}
        </div>
      </div>

      <Composer
        bind:showEmojiPicker
        bind:emojiMode
        bind:reactTarget
        {replyTo}
        {klipyApiKey}
        {recentEmoji}
        onSend={sendMessage}
        onAttach={attachFile}
        onShareGif={shareGif}
        onCancelReply={cancelReply}
        onPushRecentEmoji={pushRecentEmoji}
        onToggleReaction={toggleReaction}
        onCloseReactionPicker={() => (reactionPickerFor = null)}
        bind:this={composer}
      />
    </div>
  </section>
</main>

<style>

  .app-layout {
    display: flex;
    width: 100%;
    height: 100%;
    background: #090d16;
    color: #e2e8f0;
    overflow: hidden;
  }

  @keyframes blinker {
    50% { opacity: 0.4; }
  }

  .edit-input {
    background: #0f1115;
    border: 1px solid #1f232b;
    padding: 6px 8px;
    color: #e2e8f0;
    border-radius: 4px;
    outline: none;
    flex-grow: 1;
    font-size: 13px;
  }

  .save-btn {
    background: #10b981;
    border: none;
    color: #090d16;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-weight: bold;
  }

  .cancel-btn {
    background: #ef4444;
    border: none;
    color: white;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
  }

  .main-workspace {
    display: flex;
    flex-direction: column;
    flex-grow: 1;
    min-height: 0;
    height: 100%;
    overflow: hidden;
    background: #090d16;
  }

  .app-header {
    height: 48px;
    background: #0f1115;
    padding: 0 16px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid #1f232b;
    box-shadow: 0 1px 3px rgba(0,0,0,0.3);
    box-sizing: border-box;
  }

  .server-info {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
  }

  .hash-tag {
    color: #475569;
    font-size: 20px;
    margin-right: 8px;
  }

  .status-indicator {
    font-size: 11px;
    font-weight: 700;
    color: #10b981;
    background: rgba(16, 185, 129, 0.1);
    border: 1px solid rgba(16, 185, 129, 0.2);
    padding: 4px 8px;
    border-radius: 4px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .chat-container {
    display: flex;
    flex-direction: column;
    flex-grow: 1;
    padding: 20px;
    overflow: hidden;
    box-sizing: border-box;
    min-height: 0;
  }

  .message-log {
    flex-grow: 1;
    overflow-y: auto;
    padding-bottom: 20px;
    min-height: 0
  }

  .message-log-inner {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 100%;
  }

  .welcome-card {
    margin-top: auto;
    margin-bottom: 10px;
    padding: 20px;
  }

  .welcome-card h1 {
    font-size: 32px;
    color: #e2e8f0;
    margin: 0 0 8px 0;
    font-weight: 800;
  }

  .welcome-card p {
    font-size: 16px;
    color: #475569;
  }

  .room-rename-form { display: flex; gap: 6px; align-items: center; }

  .room-title-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 2px 6px;
    background: transparent;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 15px;
    font-weight: 600;
    color: #e2e8f0;
    text-align: left;
  }

  .room-title-btn:hover { background: #1f232b; }

  .room-title-text {
    font-size: 15px;
    font-weight: 600;
  }

  .rename-hint {
    opacity: 0;
    font-size: 11px;
    transition: opacity 0.15s ease;
  }

  .room-title-btn:hover .rename-hint,
  .room-title-btn:focus-visible .rename-hint {
    opacity: 1;
  }

  </style>
