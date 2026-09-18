<script>

  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';

  let messages = $state([]);
  let inputMessage = $state('');
  let userNickname = $state('Loading...');
  let userPeerId = $state('');
  let editMode = $state(false);
  let nicknameInput = $state('');

  let allowedPeers = $state([]);
  let targetFriendId = $state('');
  let discoveredPeers = $state([]);

  let showGifPanel = $state(false);
  let gifSearchQuery = $state('');
  let searchResults = $state([]);
  let isSearchingGifs = $state(false);

  let showEmojiPicker = $state(false);

  let relayPeerId = $state('');
  let relayAddress = $state('');
  let relaySaved = $state(false);
  let relayConnected = $state(false);
  let relayListening = $state(false);
  let myCircuitAddress = $state('');
  let dialAddress = $state('');
  let showRelayPanel = $state(false);
  let reactions = $state({});
  let reactionPickerFor = $state(null);
  let emojiMode = $state('compose');
  let reactTarget = $state(null);
  let emojiSearchQuery = $state('');

  let sentFiles = $state({});
  let receivedFiles = $state({});
  let fileProgress = $state({});

  let klipyApiKey = $state('');
  let klipyKeyInput = $state('');
  let klipySaved = $state(false);

  let messageLogEl = $state(null);
  let messageLogInnerEl = $state(null);
  let composerInputEl;
  let initialJumpDone = false;
  let lastAutoScrollAt = 0;
  let pinnedToBottom = true;

  const NEAR_BOTTOM_PX = 200;

  // Channels
  let channels = $state([]);
  let activeChannel = $state('sbb-lounge');
  let channelNameInput = $state('');
  let editingChannel = $state(false);
  let showNewChannel = $state(false);
  let newChannelInput = $state('');

  const CHANNEL_MARKER = 'P2P_CHANNEL:';

  let replyTo = $state(null);

  const REPLY_MARKER = 'P2P_REPLY:';

  function replyPreviewOf(msg) {
    const body = displayBody(msg);
    return extractGifUrl(body) ? 'a GIF' : extractFileInfo(body) ? 'a file' : body.slice(0, 80);
  }

  function startReply(msg) {
    replyTo = {
      key: messageKey(msg),
      name: senderName(msg),
      preview: replyPreviewOf(msg)
    };
    showEmojiPicker = false;
    composerInputEl?.focus();
  }

  function cancelReply() {
    replyTo = null;
  }

  function messageByKey(key) {
    if (typeof key !== 'string') return null;
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

  function replyParentOf(body) {
    if (typeof body !== 'string' || !body.startsWith(REPLY_MARKER)) return null;
    const newline = body.indexOf('\n');
    const jsonPart = newline === -1 ? body.slice(REPLY_MARKER.length) : body.slice(REPLY_MARKER.length, newline);
    try {
      const payload = JSON.parse(jsonPart);
      if (typeof payload?.t === 'string') return payload.t;
    } catch { }
    return null;
  }

  function slugify(name) {
    const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40);
    return slug || 'channel';
  }

  function channelOf(msg) {
    return typeof msg?.channel === 'string' && msg.channel ? msg.channel : 'sbb-lounge';
  }

  function activeChannelName() {
    return channels.find((c) => c.id === activeChannel)?.name ?? activeChannel;
  }

  function channelEventPayloadOf(msg) {
    const body = displayBody(msg);
    if (typeof body !== 'string' || !body.startsWith(CHANNEL_MARKER)) return null;
    try {
      const payload = JSON.parse(body.slice(CHANNEL_MARKER.length));
      if (typeof payload?.id === 'string' && typeof payload?.name === 'string' && (payload.a === 'create' || payload.a === 'rename')) return payload;
    } catch { }
    return null;
  }

  async function applyChannelEvent(msg) {
    const payload = channelEventPayloadOf(msg);
    if (!payload) return;
    if (payload.a === 'create' && !channels.some((c) => c.id === payload.id)) {
      try {
        const info = await invoke('create_channel', { id: payload.id, name: payload.name });
        channels = [...channels, info];
      } catch {
        channels = [...channels.filter((c) => c.id !== payload.id), { id: payload.id, name: payload.name }];
      }
    } else if (payload.a === 'rename') {
      try {
        await invoke('rename_channel', { id: payload.id, name: payload.name });
        channels = channels.map((c) => (c.id === payload.id ? { ...c, name: payload.name } : c));
      } catch (err) {
        console.error('[accord] channel rename failed', err);
      }
    }
  }

  async function createChannel(e) {
    e.preventDefault();
    const name = newChannelInput.trim();
    if (!name) return;
    let id = slugify(name);
    if (channels.some((c) => c.id === id)) id = `${id}-${Math.random().toString(36).slice(2, 6)}`;
    try {
      const info = await invoke('create_channel', { id, name });
      channels = [...channels, info];
      await invoke('send_chat_message', {
        message: `${CHANNEL_MARKER}${JSON.stringify({ a: 'create', id, name })}`,
        uid: crypto.randomUUID(),
        channel: activeChannel
      });
      activeChannel = id;
      showNewChannel = false;
      newChannelInput = '';
    } catch (err) {
      alert(err);
    }
  }

  async function renameChannel(e) {
    e.preventDefault();
    const name = channelNameInput.trim();
    if (!name) return;
    const id = activeChannel;
    try {
      await invoke('rename_channel', { id, name });
      channels = channels.map((c) => (c.id === id ? { ...c, name } : c));
      await invoke('send_chat_message', {
        message: `${CHANNEL_MARKER}${JSON.stringify({ a: 'rename', id, name })}`,
        uid: crypto.randomUUID(),
        channel: id
      });
      editingChannel = false;
    } catch (err) {
      alert(err);
    }
  }

  const visibleMessages = $derived.by(() =>
    messages.filter((m) => channelOf(m) === activeChannel)
  );

  function isNearBottom() {
    if (!messageLogEl) return true;
    return (
      messageLogEl.scrollHeight - messageLogEl.scrollTop - messageLogEl.clientHeight <
      NEAR_BOTTOM_PX
    );
  }

  function scrollToBottom(behavior = 'auto') {
    if (!messageLogEl) return;
    lastAutoScrollAt = Date.now();
    messageLogEl.scrollTo({ top: messageLogEl.scrollHeight, behavior });
  }

  function handleMessageLogScroll() {
    pinnedToBottom = isNearBottom();
  }

  $effect(() => {
    const count = visibleMessages.length;
    if (count === 0) return;

    if (!initialJumpDone) {
      scrollToBottom('auto');
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
   activeChannel;
   scrollToBottom('auto');
 });

  function escapeRegExp(str) {
    return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  function displayBody(msg) {
    const raw = typeof msg?.body === 'string' ? msg.body : '';
    const sender = typeof msg?.sender === 'string' ? msg.sender.trim() : '';
    if (sender) {
      const prefix = new RegExp(`^${escapeRegExp(sender)}\\s*:\\s*`);
      if (prefix.test(raw)) return raw.replace(prefix, '');
    }
    return raw;
  }

  const FILE_MARKER = 'P2P_MEDIA_FILE:';

  function extractFileInfo(body) {
    if (typeof body !== 'string' || !body.startsWith(FILE_MARKER)) return null;
    try {
      const info = JSON.parse(body.slice(FILE_MARKER.length));
      if (typeof info?.id === 'string' && typeof info?.name === 'string' && typeof info?.size === 'number') return info;
    } catch {}
    return null;
  }

  function formatBytes(n) {
    if (n < 1024) return `${n} B`;
    if (n < 1048576) return `${(n / 1024).toFixed(1)} KB`;
    if (n < 1073741824) return `${(n / 1048576).toFixed(1)} MB`;
    return `${(n / 1073741824).toFixed(2)} GB`;
  }

  async function attachFile() {
    try {
      const picked = await openFileDialog({ multiple: false, title: 'Share a file' });
      if (!picked) return;
      const uid = crypto.randomUUID();
      const info = await invoke('send_file', { path: picked, uid });
      sentFiles = { ...sentFiles, [uid]: picked };
      messages = [...messages, { uid, sender: userNickname, channel: 'sbb-lounge', body: `${FILE_MARKER}${JSON.stringify(info)}` }];
    } catch (err) {
      alert(`File share failed: ${err}`);
    }
  }

  async function openSharedFile(info) {
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

  const GIF_MARKER = 'P2P_MEDIA_GIF:';

  function extractGifUrl(rawBody) {
    const body = typeof rawBody === 'string' ? rawBody : '';
    const idx = body.indexOf(GIF_MARKER);
    if (idx === -1) return null;
    const url = body.slice(idx + GIF_MARKER.length).trim();
    return url || null;
  }

  // Reactions
  const REACT_MARKER = 'P2P_REACT:';
  const QUICK_REACTIONS = ['👍', '❤️', '😂', '😮', '😢', '🎉', '🍻', '💩'];

  function messageKey(msg) {
    if (typeof msg?.uid === 'string' && msg.uid) return `uid:${msg.uid}`;
    return `legacy:${senderName(msg)}::${displayBody(msg)}`;
  }

  function reactionPayloadOf(msg) {
    const body = displayBody(msg);
    if (typeof body !== 'string' || !body.startsWith(REACT_MARKER)) return null;
    try {
      const payload = JSON.parse(body.slice(REACT_MARKER.length));
      if (typeof payload?.t === 'string' && typeof payload?.e === 'string') return payload;
    } catch {}
    return null;
  }

  function me() {
    return userPeerId
      ? { p: userPeerId, n: userNickname }
      : { p: `nick:${userNickname}`, n: userNickname };
  }

  function actorOf(payload, fallbackName) {
    if (typeof payload?.p === 'string' && payload.p) {
      return { p: payload.p, n: typeof payload.n === 'string' && payload.n ? payload.n : fallbackName };
    }
    return { p: `nick:${fallbackName}`, n: fallbackName };
  }

  function setReactionUsers(target, emoji, users) {
    const forMessage = { ...(reactions[target] ?? {}) };
    if (users.length > 0) forMessage[emoji] = users;
    else delete forMessage[emoji];

    const next = { ...reactions };
    if (Object.keys(forMessage).length > 0) next[target] = forMessage;
    else delete next[target];
    reactions = next;
  }

  function applyReaction(actor, target, emoji, action) {
    const users = reactions[target]?.[emoji] ?? [];
    if (action === 'add') {
      if (!users.some((u) => u.p === actor.p)) setReactionUsers(target, emoji, [...users, actor]);
    } else if (action === 'remove') {
      setReactionUsers(target, emoji, users.filter((u) => u.p !== actor.p));
    }
  }

  function applyReactionEvent(msg) {
    const payload = reactionPayloadOf(msg);
    if (payload) applyReaction(actorOf(payload, senderName(msg)), payload.t, payload.e, payload.a);
  }

  async function toggleReaction(target, emoji) {
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

  function senderName(msg) {
    const s = msg?.sender;
    return typeof s === 'string' && s.trim() ? s : 'Unknown';
  }

  function senderInitials(msg) {
    return (senderName(msg).trim().slice(0, 2) || '?').toUpperCase();
  }

  function shortPeer(peer, len = 12) {
    return typeof peer === 'string' ? `${peer.slice(0, len)}...` : '';
  }

  function gifTileUrl(gif) {
    return gif?.file?.hd?.gif?.url || gif?.file?.sd?.gif?.url || '';
  }

  let failedMediaUrls = $state([]);

  function handleMediaError(url) {
    console.warn('[portsidepeer] media failed to load (CSP-blocked, offline, or dead URL):', url);
    if (!failedMediaUrls.includes(url)) failedMediaUrls = [...failedMediaUrls, url];
  }

  function clickOutside(node, callback) {
    const onClick = (event) => {
      if (!node.contains(event.target)) callback?.();
    };
    document.addEventListener('click', onClick);
    return {
      destroy() {
        document.removeEventListener('click', onClick);
      }
    };
  }

  async function searchGifs() {
    if (!klipyApiKey) return;
      if (!gifSearchQuery.trim()) {
        searchResults = [];
        return;
    }
    isSearchingGifs = true;
    try {
      const queryText = encodeURIComponent(gifSearchQuery.trim());
      const response = await fetch(
        `https://api.klipy.com/api/v1/${klipyApiKey}/gifs/search?q=${queryText}`,
        {
          method: "GET",
          headers: {
            "Accept": "application/json",
            "Content-Type": "application/json"
          }
        }
      );

      if (!response.ok) throw new Error(`HTTP Error: ${response.status}`);

      const payload = await response.json();
      searchResults = payload?.data?.data || [];
    } catch (err) {
      console.error("GIF search failed:", err);
      searchResults = [];
    } finally {
      isSearchingGifs = false;
    }
  }

  async function shareGif(gifUrl) {
    if (!gifUrl) return;
    const uid = crypto.randomUUID();
    const specializedGifPayload = `P2P_MEDIA_GIF:${gifUrl}`;
    messages = [...messages, { uid, sender: userNickname, channel: activeChannel, body: specializedGifPayload }];
    await invoke('send_chat_message', { message: specializedGifPayload, uid, channel: activeChannel });
    showGifPanel = false;
    gifSearchQuery = '';
    searchResults = [];
  }

  let activeEmojiCategory = $state(0);

  const EMOJI_CATEGORIES = [
    {
      name: 'Smileys & Faces',
      icon: '😀',
      emojis: [
        '😀','😃','😄','😁','😆','😅','🤣','😂','🙂','🙃',
        '😉','😊','😇','🥰','😍','🤩','😘','😗','😚','😙',
        '🥲','😋','😛','😜','🤪','😝','🤑','🤗','🤭','🤫',
        '🤔','🤐','🤨','😐','😑','😶','😏','😒','🙄','😬',
        '🤥','😌','😔','😪','🤤','😴','😷','🤒','🤕','🤢',
        '🤮','🥵','🥶','😵','🤯','🤠','🥳','🥸','😎','🤓',
        '🧐','😕','😟','🙁','😮','😯','😲','😳','🥺','😦',
        '😧','😨','😰','😥','😢','😭','😱','😖','😣','😞',
        '😓','😩','😫','🥱','😤','😡','😠','🤬','😈','👿',
        '💀','☠️','💩','🤡','👹','👻','👽','🤖','😺','😹',
        '😻','😼','😽','😾','😿','🙀'
      ]
    },
    {
      name: 'People & Gestures',
      icon: '👍',
      emojis: [
        '👍','👎','👌','🤌','🤏','✌️','🤞','🤟','🤘','🤙',
        '👈','👉','👆','👇','☝️','👋','🤚','🖐️','✋','🖖',
        '👏','🙌','🤲','🤝','🙏','💪','🦾','✍️','💅','🤳',
        '👀','👁️','👄','🫦','🧠','🦷','🦴','👶','🧒','👦',
        '👧','🧑','👨','🧔','👩','🧓','👴','👵','🙍','🙎',
        '🙅','🙆','💁','🙋','🧏','🙇','🤦','🤷','💃','🕺',
        '👯','🧘','🏃','🚶'
      ]
    },
    {
      name: 'Celebration & Activity',
      icon: '🎉',
      emojis: [
        '🎉','🎊','🎈','🎁','🎀','🏅','🏆','🥇','🥈','🥉',
        '⚽','⚾','🏀','🏈','🏐','🏉','🎾','🎳','🏏','🏑',
        '🏒','🏓','🏸','🥊','🥋','⛳','🏹','🎣','🥅','🎮',
        '🕹️','🎲','🧩','🎯','🎰','🪀','🪁','🎤','🎧','🎼',
        '🎹','🥁','🎷','🎺','🎸','🎻','🎬','🎨','🎭','🎪',
        '🎟️','🎫', '💩'
      ]
    },
    {
      name: 'Animals & Nature',
      icon: '🐶',
      emojis: [
        '🐶','🐱','🐭','🐹','🐰','🦊','🐻','🐼','🐨','🐯',
        '🦁','🐮','🐷','🐸','🐵','🙈','🙉','🙊','🐔','🐧',
        '🐦','🐤','🦆','🦅','🦉','🦇','🐺','🐗','🐴','🦄',
        '🐝','🐛','🦋','🐌','🐞','🐜','🦂','🐢','🐍','🦎',
        '🐙','🦑','🦐','🦀','🐡','🐠','🐟','🐬','🐳','🐋',
        '🦈','🐊','🐘','🦒','🦓','🦍','🦏','🐪','🐫','🦘',
        '🐃','🐄','🐎','🐖','🐏','🐑','🐐','🦌','🐕','🐩',
        '🐈','🐓','🦃','🦚','🦜','🦢','🕊️','🐇','🦝','🦔',
        '🐾','🌵','🌲','🌳','🌴','🌱','🌿','☘️','🍀','🎍',
        '🎋','🍃','🍂','🍁','🌾','🌺','🌻','🌹','🥀','🌷',
        '🌼','🌸','💐','🍄','🌰','🌍','🌎','🌏','🌕','⭐',
        '🌟','✨','⚡','🔥','🌈','☀️','⛅','☁️','🌧️','⛈️',
        '❄️','☃️','⛄','🌪️','🌫️','🌊','💧'
      ]
    },
    {
      name: 'Food & Drink',
      icon: '🍕',
      emojis: [
        '🍏','🍎','🍐','🍊','🍋','🍌','🍉','🍇','🍓','🫐',
        '🍒','🍑','🥭','🍍','🥥','🥝','🍅','🍆','🥑','🥦',
        '🥬','🥒','🌽','🥕','🧄','🧅','🥔','🍠','🍞','🥐',
        '🥖','🥨','🧇','🥞','🧈','🍳','🥚','🧀','🥓','🍗',
        '🍖','🌭','🍔','🍟','🍕','🥪','🥙','🌮','🌯','🥗',
        '🥘','🍝','🍜','🍲','🍛','🍣','🍱','🥟','🍤','🍙',
        '🍚','🍘','🍥','🥠','🍢','🍡','🍧','🍨','🍦','🥧',
        '🧁','🍰','🎂','🍮','🍭','🍬','🍫','🍿','🍩','🍪',
        '☕','🍵','🧃','🥤','🧋','🍺','🍻','🥂','🍷','🥃',
        '🍸','🍹','🍾','🥄','🍴','🍽️'
      ]
    },
    {
      name: 'Travel & Places',
      icon: '🚗',
      emojis: [
        '🚗','🚕','🚌','🚎','🏎️','🚓','🚑','🚒','🚐','🛻',
        '🚚','🚛','🚜','🛵','🏍️','🚲','🛴','🚨','🚦','🚧',
        '⚓','⛵','🚤','🛳️','⛴️','🚢','✈️','🛫','🛬','🚀',
        '🛸','🚁','🗺️','🏔️','🏕️','🏖️','🏝️','🏟️','🏛️','🏗️',
        '🏠','🏡','🏢','🏣','🏥','🏦','🏨','🏫','🏭','🏰',
        '🏯','🗼','🗽','⛲','🌁','🌃','🏙️','🌄','🌅','🌇','🌉'
      ]
    },
    {
      name: 'Objects & Tech',
      icon: '💡',
      emojis: [
        '⌚','📱','💻','⌨️','🖥️','🖨️','🖱️','💽','💾','💿',
        '📀','📷','📸','📹','🎥','📞','☎️','📟','📠','📺',
        '📻','🎙️','⏰','⏱️','⏳','📡','🔋','🔌','💡','🔦',
        '🕯️','🧯','🪓','🔪','🗡️','🛡️','💣','🔮','📿','🧿',
        '💈','🔭','🔬','💊','💉','🩹','🩺','🚪','🛏️','🛋️',
        '🚽','🚿','🛁','🧴','🧹','🧺','🧻','🪣','🧼','🪥',
        '🧽','🛒','📦','📫','📮','📪','📬','📭','💭','💬',
        '🗯️','📄','📃','📑','📊','📈','📉','🗒️','🗓️','📅',
        '📇','🗃️','🗳️','🗄️','📋','📁','📂','🗞️','📰','📓',
        '📔','📒','📚','📖','🔖','🔗','📎','🖇️','📐','📏',
        '🧮','📌','📍','✂️','🖊️','🖋️','✒️','🖌️','🖍️','📝',
        '✏️','🔍','🔎','🔏','🔐','🔒','🔓','🔑','🗝️'
      ]
    },
    {
      name: 'Hearts & Symbols',
      icon: '❤️',
      emojis: [
        '❤️','🧡','💛','💚','💙','💜','🖤','🤍','🤎','💔',
        '❣️','💕','💞','💓','💗','💖','💘','💝','☮️','✝️',
        '☪️','🕉️','☸️','✡️','🔯','🕎','☯️','☦️','⛎','♈',
        '♉','♊','♋','♌','♍','♎','♏','♐','♑','♒',
        '♓','🆔','⚛️','☢️','☣️','📴','📳','🈶','🈚','🈸',
        '🈺','🈷️','✴️','🆚','🆕','🆙','🆗','🆒','🆓','🆖',
        'ℹ️','Ⓜ️','🅾️','🅿️','♻️','⚠️','🚸','🔞','🚫','⛔',
        '✅','❌','❓','❗','‼️','⁉️','💯','🔅','🔆','〽️',
        '🔱','⚜️','🔰','〰️','➰','➿','✔️','☑️','🔘','🔴',
        '🟠','🟡','🟢','🔵','🟣','⚫','⚪','🟤','🔺','🔻',
        '🔸','🔹','🔶','🔷','♠️','♣️','♥️','♦️','🃏','🎴',
        '🀄','🕐','🕑','🕒','🕓','🕔','🕕','🕖','🕗','🕘',
        '🕙','🕚','🕛','⌛'
      ]
    }
  ];

  const EMOJI_KEYWORDS = {
    // Smileys & Faces
    '😀': 'grin happy smile cheerful',
    '😂': 'lol laughing tears joy cry',
    '🤣': 'rofl rolling floor laughing lol',
    '😊': 'blush smile happy',
    '😍': 'heart eyes love',
    '🤩': 'star struck excited',
    '🥳': 'party celebrate birthday',
    '😎': 'cool sunglasses',
    '🤓': 'nerd glasses',
    '🤔': 'think thinking hmm',
    '🙄': 'eyeroll annoyed',
    '😴': 'sleep tired zzz bored',
    '😭': 'cry crying loudly tears sad',
    '😱': 'scream shocked scared',
    '😡': 'angry mad rage',
    '💀': 'skull dead death',
    '🤖': 'robot bot',
    '👻': 'ghost spooky halloween',
    // People & Gestures
    '👍': 'thumbs up yes ok approve like',
    '👎': 'thumbs down no disapprove',
    '👌': 'ok perfect',
    '✌️': 'peace victory',
    '🤞': 'fingers crossed luck hope',
    '🙏': 'pray please thanks namaste',
    '👏': 'clap applause bravo',
    '🙌': 'praise hands hooray',
    '💪': 'muscle strong flex arm',
    '👋': 'wave hello hi bye',
    '👀': 'eyes look watching',
    '🤦': 'facepalm',
    '🤷': 'shrug idk whatever',
    '💃': 'dance dancing party',
    '🕺': 'dance dancing man',
    '🏃': 'run running',
    '🚶': 'walk walking',
    // Celebration & Activity
    '🎉': 'party tada celebrate congrats confetti',
    '🎊': 'confetti celebrate party',
    '🎈': 'balloon party birthday',
    '🎁': 'gift present birthday',
    '🏆': 'trophy win winner champion',
    '🥇': 'gold medal first winner',
    '🎮': 'game controller video games',
    '🎲': 'dice game random',
    '🎯': 'dart target goal bullseye',
    '🎤': 'mic microphone sing karaoke',
    '🎸': 'guitar music rock',
    '🎬': 'movie film',
    '💩': 'poop poo',
    // Animals & Nature
    '🐶': 'dog puppy',
    '🐱': 'cat kitten',
    '🐰': 'rabbit bunny',
    '🦊': 'fox',
    '🐻': 'bear',
    '🐼': 'panda',
    '🦁': 'lion',
    '🐸': 'frog',
    '⭐': 'star',
    '✨': 'sparkles shine magic',
    '🔥': 'fire lit hot flame',
    '🌈': 'rainbow pride',
    '☀️': 'sun sunny weather',
    '🌊': 'wave water ocean sea',
    // Food & Drink
    '🍕': 'pizza',
    '🍔': 'burger hamburger',
    '🍟': 'fries chips',
    '🌮': 'taco mexican',
    '🌭': 'hotdog sausage',
    '🍦': 'ice cream',
    '🎂': 'cake birthday',
    '🍩': 'donut doughnut',
    '☕': 'coffee tea cafe',
    '🍺': 'beer brew cheers',
    '🍻': 'beers cheers toast',
    '🍷': 'wine drink glass',
    // Travel & Places
    '🚗': 'car',
    '🚕': 'taxi cab',
    '🚓': 'police cop car',
    '🚑': 'ambulance',
    '🚒': 'fire truck engine',
    '🚀': 'rocket ship launch space',
    '✈️': 'plane flight travel airplane',
    '🏠': 'house home',
    '🏰': 'castle',
    // Objects & Tech
    '⌚': 'watch time clock',
    '📱': 'phone mobile smartphone',
    '💻': 'laptop computer pc',
    '⌨️': 'keyboard typing',
    '🖥️': 'desktop monitor computer',
    '📷': 'camera photo',
    '📺': 'tv television',
    '💡': 'idea light bulb',
    '🔋': 'battery charge power',
    '🔑': 'key password access',
    '🔒': 'lock secure private locked',
    '💬': 'speech chat message talk',
    '🔍': 'search find magnify',
    '📝': 'note write memo',
    '📚': 'books reading study',
    // Hearts & Symbols
    '❤️': 'heart love red',
    '🧡': 'heart orange',
    '💛': 'heart yellow',
    '💚': 'heart green',
    '💙': 'heart blue',
    '💜': 'heart purple',
    '🖤': 'heart black',
    '💔': 'broken heart sad breakup',
    '💕': 'hearts love',
    '💯': 'hundred 100 perfect',
    '✅': 'check done yes complete',
    '❌': 'x no wrong cancel cross',
    '❓': 'question confused what',
    '⚠️': 'warning caution alert',
    '♻️': 'recycle reuse',
    '☑️': 'checkbox checked done'
  };

  const emojiSearchResults = $derived.by(() => {
    const terms = emojiSearchQuery.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (terms.length === 0) return null;
    return FLAT_EMOJIS.filter(({ emoji }) => {
      const tokens = EMOJI_TOKENS.get(emoji) ?? [];
      return terms.every((t) => tokens.some((tok) => tok.startsWith(t)));
    }).slice(0, 80);
  });

  const stripVariation = (e) => e.replace(/\uFE0F/g, '');
  const KEYWORD_INDEX = new Map(
    Object.entries(EMOJI_KEYWORDS).map(([e, k]) => [stripVariation(e), k.toLowerCase()])
  );

  const CATEGORY_TERMS = EMOJI_CATEGORIES.map((cat) =>
    cat.name.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean)
  );

  const FLAT_EMOJIS = EMOJI_CATEGORIES.flatMap((cat, catIndex) =>
    cat.emojis.map((emoji) => ({ emoji, catIndex }))
  );

  const EMOJI_TOKENS = new Map(
    FLAT_EMOJIS.map(({ emoji, catIndex }) => [
      emoji,
      [
        ...CATEGORY_TERMS[catIndex],
        ...(KEYWORD_INDEX.get(stripVariation(emoji)) ?? '').split(/\s+/).filter(Boolean)
      ]
    ])
  );

  function previewOf(msg) {
    const body = displayBody(msg);
    return extractGifUrl(body) ? 'a GIF' : body;
  }

  function openFullReactionPicker(msg) {
    reactTarget = { key: messageKey(msg), preview: previewOf(msg) };
    emojiMode = 'react';
    emojiSearchQuery = '';
    activeEmojiCategory = 0;
    reactionPickerFor = null;
    showEmojiPicker = true;
  }

  function closeEmojiPicker() {
    showEmojiPicker = false;
    emojiMode = 'compose';
    reactTarget = null;
    emojiSearchQuery = '';
  }

  function pickEmoji(emoji) {
    if (emojiMode === 'react' && reactTarget) {
      toggleReaction(reactTarget.key, emoji);
      closeEmojiPicker();
      return;
    }
    inputMessage += emoji;
    composerInputEl?.focus();
  }

  async function saveRelay(e) {
    e.preventDefault();
    if (!relayPeerId.trim() || !relayAddress.trim()) return;
    try {
      await invoke('save_relay_endpoint', { peerId: relayPeerId, address: relayAddress });
      relaySaved = true;
      alert("Relay endpoint saved!\n\nPlease restart the app to connect.");
    } catch (err) {
      alert(`Failed to save relay: ${err}`);
    }
  }

  async function clearRelay() {
    try {
      await invoke('clear_relay_endpoint');
      relayPeerId = '';
      relayAddress = '';
      relaySaved = false;
      alert("Relay cleared.\n\nPlease restart to disable.");
    } catch (err) {
      alert(`Failed to clear relay: ${err}`);
    }
  }

  async function saveKlipyKey(e) {
    e.preventDefault();
    if (!klipyKeyInput.trim()) return;
    try {
      await invoke('save_klipy_key', { apiKey: klipyKeyInput.trim() });
      klipyApiKey = klipyKeyInput.trim();
      klipySaved = true;
      klipyKeyInput = '';
    } catch (err) {
      alert(err);
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

  async function dialPeerViaRelay(e) {
    e.preventDefault();
    if (!dialAddress.trim()) return;
    try {
      await invoke('dial_peer', { address: dialAddress.trim() });
      alert("Dialing peer...");
      dialAddress = '';
    } catch (err) {
      alert(`Dial failed: ${err}`);
    }
  }

  function copyCircuitAddress() {
    if (myCircuitAddress) {
      navigator.clipboard.writeText(myCircuitAddress);
      alert("Circuit address copied to clipboard!");
    }
  }

  onMount(() => {
    let unlisteners = [];

    const safe = async (label, fn) => {
      try {
        return await fn();
      } catch (err) {
        console.error(`[portsidepeer] init step failed: ${label}`, err);
        return null;
      }
    };

    const addListener = async (name, handler) => {
      try {
        unlisteners.push(await listen(name, handler));
      } catch (err) {
        console.error(`[portsidepeer] listener failed: ${name}`, err);
      }
    };

    async function initializeApp() {
      const profile = await safe('get_profile', () => invoke('get_profile'));
      if (profile) {
        userNickname = profile.nickname ?? 'Guest';
        userPeerId = profile.peer_id ?? '';
        nicknameInput = profile.nickname ?? '';
      } else {
        userNickname = 'Guest';
      }

      const list = await safe('get_allow_list', () => invoke('get_allow_list'));
      allowedPeers = Array.isArray(list) ? list : [];

      const chans = await safe('get_channels', () => invoke('get_channels'));
      channels = Array.isArray(chans) && chans.length ? chans : [{ id: 'sbb-lounge', name: 'sbb-lounge' }];
      activeChannel = channels[0].id;

      const historicalLogs = await safe('get_chat_history', () => invoke('get_chat_history'));
      const allLogs = Array.isArray(historicalLogs) ? historicalLogs : [];

      reactions = {};
      const chatOnly = [];
      for (const m of allLogs) {
        if (!m || typeof m !== 'object') continue;
        if (reactionPayloadOf(m)) { applyReactionEvent(m); continue; }
        if (channelEventPayloadOf(m)) { applyChannelEvent(m); continue; }
        chatOnly.push(m);
      }
      messages = chatOnly;

      const savedRelay = await safe('get_relay_endpoint', () => invoke('get_relay_endpoint'));
      if (savedRelay) {
        relayPeerId = savedRelay.peer_id ?? '';
        relayAddress = savedRelay.address ?? '';
        relaySaved = Boolean(relayPeerId && relayAddress);
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
      });
      await addListener('chat-msg', (event) => {
        const incoming = event.payload;
        if (incoming && typeof incoming === 'object') {
          if (incoming.uid && messages.some((m) => m.uid === incoming.uid)) return;
          if (reactionPayloadOf(incoming)) { applyReactionEvent(incoming); return; }
          if (channelEventPayloadOf(incoming)) { applyChannelEvent(incoming); return; }
          messages = [...messages, incoming];
        }
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
    };
  });

  async function sendMessage(e) {
    e.preventDefault();
    if (!inputMessage.trim()) return;
    const uid = crypto.randomUUID();

    const rawBody = inputMessage;
      const outgoingBody = replyTo
        ? `${REPLY_MARKER}${JSON.stringify({ t: replyTo.key })}\n${rawBody}`
        : rawBody;

    messages = [...messages, { uid, channel: activeChannel, sender: userNickname, body: outgoingBody }];
    await invoke('send_chat_message', { message: outgoingBody, uid, channel: activeChannel });

    inputMessage = '';
    replyTo = null;
    showEmojiPicker = false;
  }

  async function saveNickname(e) {
    e.preventDefault();
    try {
      const updatedProfile = await invoke('update_nickname', { newName: nicknameInput });
      userNickname = updatedProfile.nickname;
      editMode = false;
    } catch (err) {
      alert(err);
    }
  }

  async function acceptPeer(peerId) {
    try {
      const updatedList = await invoke('add_to_allow_list', { targetFriendPeerId: peerId });

      allowedPeers = Array.isArray(updatedList) ? updatedList : [];
      discoveredPeers = discoveredPeers.filter(p => p !== peerId);
    } catch (err) {
      alert(err);
    }
  }

  async function revokeAccess(peerId) {
    try {
      const updatedList = await invoke('remove_from_allow_list', { targetPeerId: peerId });
      allowedPeers = Array.isArray(updatedList) ? updatedList : [];
    } catch (err) {
      alert(err);
    }
  }

  function handleManualSubmit(e) {
    e.preventDefault();
    if (!targetFriendId.trim()) return;
    acceptPeer(targetFriendId.trim());
    targetFriendId = '';
  }

  function selectCircuitAddr(e) {
    e.target.select();
  }

  function displayReply(body) {
    const parentKey = replyParentOf(body);
    if (!parentKey) return { parentKey: null, text: body };
    const rest = body.slice(body.indexOf('\n') + 1);
    return { parentKey, text: rest };
  }

</script>

<main class="app-layout">
  <aside class="sidebar">
    <div class="sidebar-header">
      <h2>PortsidePeer settings</h2>
    </div>

    <div class="sidebar-content">
      <div class="panel-section">
        <h4>My Identity Key</h4>
        <p>Give this to friends so they can whitelist you:</p>
        <textarea readonly class="id-share-box" value={userPeerId} onclick={(e) => e.target.select()}></textarea>
      </div>

      <div class="panel-section">
        <h4>Circuit Relay</h4>

        {#if relaySaved}
          <div class="relay-status-box" class:active={relayConnected && relayListening}>
            <span class="relay-indicator"></span>
            <span>{relayConnected && relayListening ? 'Relay Connected' : 'Connecting...'}</span>
            <button onclick={clearRelay} class="clear-relay-btn">Disable</button>
          </div>

          {#if myCircuitAddress}
            <textarea readonly class="id-share-box" style="height: 35px; margin-top: 8px; font-size: 10px; color: #94a3b8;" onclick={selectCircuitAddr}>{myCircuitAddress}</textarea>
            <p class="relay-notice">Click address to select • Share with peers to connect</p>
          {:else if !relayConnected}
            <p class="relay-notice">⚠️ Restart required to connect</p>
          {/if}

          <form onsubmit={dialPeerViaRelay} class="sidebar-form" style="margin-top: 8px;">
            <input bind:value={dialAddress} placeholder="Dial: /p2p/<relay>/p2p-circuit/p2p/<peer>" autocomplete="off" />
            <button type="submit">Dial Peer</button>
          </form>
        {:else}
          <form onsubmit={saveRelay} class="sidebar-form">
            <input bind:value={relayPeerId} placeholder="Relay Peer ID..." autocomplete="off" />
            <input bind:value={relayAddress} placeholder="/ip4/1.2.3.4/tcp/4001" autocomplete="off" />
            <button type="submit">Save & Enable Relay</button>
          </form>
        {/if}
      </div>

      <div class="panel-section">
        <h4>Channels ({channels.length})</h4>
        <div class="channel-list">
          {#each channels as channel (channel.id)}
            <button
              type="button"
              class="channel-row"
              class:active={channel.id === activeChannel}
              onclick={() => (activeChannel = channel.id)}
            >
              <span class="hash-tag">#</span>
              <span class="channel-name">{channel.name}</span>
            </button>
          {/each}
        </div>
        {#if showNewChannel}
          <form onsubmit={createChannel} class="sidebar-form" style="margin-top: 8px;">
            <input bind:value={newChannelInput} placeholder="Add new channel!" autocomplete="off" />
            <button type="submit">Create</button>
          </form>
        {:else}
          <button type="button" class="add-channel-btn" onclick={() => (showNewChannel = true)}>+ Add Channel</button>
        {/if}
      </div>

      <div class="panel-section">
        <h4>GIF Search</h4>
        {#if klipySaved}
          <div class="relay-status-box active">
            <span class="relay-indicator"></span>
            <span>Klipy key configured</span>
            <button onclick={clearKlipyKey} class="clear-relay-btn">Clear</button>
          </div>
        {:else}
          <form onsubmit={saveKlipyKey} class="sidebar-form">
            <input bind:value={klipyKeyInput} placeholder="Klipy API key..." autocomplete="off" />
            <button type="submit">Save Key</button>
          </form>
        {/if}
      </div>

      {#if discoveredPeers.length > 0}
        <div class="panel-section discovery-alert-zone">
          <h4 class="pulse-text">⚡ Nearby Peer Found!</h4>
          {#each discoveredPeers as peer}
            <div class="discovery-card">
              <span class="disc-id">ID: {shortPeer(peer, 10)}</span>
              <button onclick={() => acceptPeer(peer)} class="accept-invite-btn">Connect</button>
            </div>
          {/each}
        </div>
      {/if}

      <div class="panel-section">
        <h4>Manual Authorization</h4>
        <form onsubmit={handleManualSubmit} class="sidebar-form">
          <input bind:value={targetFriendId} placeholder="Paste friend's PeerID..." autocomplete="off" />
          <button type="submit">Grant Entry</button>
        </form>
      </div>

      <div class="panel-section allowed-list-box">
        <h4>Allowed Friends ({allowedPeers.length})</h4>
        <div class="allowed-scroll">
          {#if allowedPeers.length === 0}
            <span class="empty-notice">Waiting for automatic detection...</span>
          {/if}
          {#each allowedPeers as peer}
            <div class="peer-row" title={peer}>
              <span class="key-icon">🔑</span>
              <span class="peer-id-text">{shortPeer(peer, 12)}</span>
              <button onclick={() => revokeAccess(peer)} class="revoke-btn" title="Revoke access and block peer">✕</button>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <div class="profile-drawer">
      {#if !editMode}
        <div class="profile-view">
          <span class="profile-name" title={userPeerId}>🌐 {userNickname}</span>
          <button onclick={() => editMode = true} class="edit-btn">Edit</button>
        </div>
      {:else}
        <form onsubmit={saveNickname} class="profile-edit">
          <input bind:value={nicknameInput} placeholder="Change name..." class="edit-input" />
          <div class="edit-actions">
            <button type="submit" class="save-btn">✓</button>
            <button type="button" onclick={() => editMode = false} class="cancel-btn">✕</button>
          </div>
        </form>
      {/if}
    </div>
  </aside>

  <section class="main-workspace">
    <header class="app-header">
      <div class="server-info">
          {#if editingChannel}
            <form onsubmit={renameChannel} class="channel-rename-form">
              <input bind:value={channelNameInput} class="edit-input" />
              <button type="submit" class="save-btn">✓</button>
              <button type="button" onclick={() => (editingChannel = false)} class="cancel-btn">✕</button>
            </form>
          {:else}
            <button
              type="button"
              class="channel-title-btn"
              onclick={() => { channelNameInput = activeChannelName(); editingChannel = true; }}
            >
              <span class="hash-tag">#</span>
              <span class="channel-title-text">{activeChannelName()}</span>
              <span class="rename-hint" aria-hidden="true">✏️</span>
            </button>
          {/if}
        <span class="status-indicator">Private Mesh Active</span>
      </div>
    </header>

    <div class="chat-container">
      <div class="message-log" bind:this={messageLogEl} onscroll={handleMessageLogScroll}>
        <div class="message-log-inner" bind:this={messageLogInnerEl}>
            {#if visibleMessages.length === 0}
              <div class="welcome-card">
                <h1>Welcome to #{activeChannelName()}!</h1>
              <p>The file-broker link is running. Use the GIF launcher tab to share expressions.</p>
            </div>
          {/if}
          {#each visibleMessages as msg}
            {@const body = displayBody(msg)}
            {@const gifUrl = extractGifUrl(body)}
            {@const fileInfo = extractFileInfo(body)}
            {@const isMine = senderName(msg) === userNickname}
            {@const mKey = messageKey(msg)}
            {@const replyInfo = displayReply(body)}
            <div class="message-card" id={`msg-${mKey}`}>
              <div class="avatar-mock">{senderInitials(msg)}</div>
              <div class="message-content">
                <span class="user-badge" class:self-user={isMine}>{senderName(msg)}</span>

                {#if gifUrl}
                    {#if failedMediaUrls.includes(gifUrl)}
                        <a class="gif-fallback-link" href={gifUrl} target="_blank" rel="noopener noreferrer" title={gifUrl}>
                          GIF unavailable in-app — click to open in browser
                        </a>
                      {:else}
                        <div class="gif-media-frame">
                          <img
                            src={gifUrl}
                            alt="Media shared over libp2p"
                            loading="lazy"
                            onerror={() => handleMediaError(gifUrl)}
                          />
                        </div>
                      {/if}
                  {:else if fileInfo}
                    <div class="file-card">
                      <span class="file-icon">📄</span>
                      <div class="file-details">
                        <span class="file-name" title={fileInfo.name}>{fileInfo.name}</span>
                        <span class="file-size">{formatBytes(fileInfo.size)}</span>
                      </div>
                      {#if sentFiles[fileInfo.id] || receivedFiles[fileInfo.id]}
                        <button type="button" class="file-open-btn" onclick={() => openSharedFile(fileInfo)}>Open</button>
                      {:else if fileProgress[fileInfo.id]}
                        <span class="file-progress-label">
                          {Math.round((fileProgress[fileInfo.id].received / Math.max(fileProgress[fileInfo.id].total, 1)) * 100)}%
                        </span>
                      {:else}
                        <span class="file-progress-label">Not downloaded</span>
                      {/if}
                    </div>
                  {:else}
                    {#if replyInfo.parentKey}
                      {@const parent = messageByKey(replyInfo.parentKey)}
                      {#if parent}
                        <button
                          type="button"
                          class="reply-quote"
                          onclick={() => {
                            document.getElementById(`msg-${replyInfo.parentKey}`)?.scrollIntoView({
                              behavior: 'smooth', block: 'center'
                            });
                          }}
                        >
                          <span class="reply-quote-bar" aria-hidden="true"></span>
                          <span class="reply-quote-sender">{senderName(parent)}</span>
                          <span class="reply-quote-text">{replyPreviewOf(parent)}</span>
                        </button>
                      {:else}
                        <span class="reply-quote reply-quote-ghost">
                          <span class="reply-quote-bar" aria-hidden="true"></span>
                          <span class="reply-quote-sender">Original message</span>
                          <span class="reply-quote-text">not available on this device</span>
                        </span>
                      {/if}
                    {/if}
                    <p class="msg-body">{replyInfo.text}</p>
                    {/if}
                {#if reactions[mKey]}
                  <div class="reaction-row">
                    {#each Object.entries(reactions[mKey]) as [emoji, users] (emoji)}
                      <button
                        type="button"
                        class="reaction-pill"
                        class:mine={users.some((u) => u.p === me().p)}
                        title={users.map((u) => u.n).join(', ')}
                        onclick={() => toggleReaction(mKey, emoji)}
                      >
                        <span>{emoji}</span><span class="reaction-count">{users.length}</span>
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>

              <div class="reaction-add-wrap">
                <button
                  type="button"
                  class="reaction-add-btn"
                  aria-label="Add reaction"
                  onclick={(e) => {
                    e.stopPropagation();
                    reactionPickerFor = reactionPickerFor === mKey ? null : mKey;
                  }}
                >🙂</button>
                <button
                    type="button"
                    class="reaction-add-btn"
                    aria-label="Reply to message"
                    onclick={() => startReply(msg)}
                  >↩️</button>
                </div>

                {#if reactionPickerFor === mKey}
                  <div class="reaction-quick-bar" use:clickOutside={() => (reactionPickerFor = null)}>
                    {#each QUICK_REACTIONS as emoji}
                      <button
                        type="button"
                        class="reaction-quick-emoji"
                        onclick={() => { toggleReaction(mKey, emoji); reactionPickerFor = null; }}
                      >{emoji}</button>
                    {/each}
                    <button
                      type="button"
                      class="reaction-quick-emoji reaction-more-btn"
                      title="More reactions"
                      onclick={(e) => { e.stopPropagation(); openFullReactionPicker(msg); }}
                    >⋯</button>
                  </div>
                {/if}
            </div>
          {/each}
        </div>
      </div>

      {#if showGifPanel}
        <div class="gif-search-drawer">
          <div class="drawer-header">
            <input
              bind:value={gifSearchQuery}
              oninput={searchGifs}
              placeholder="Search Giphy for an expression..."
              autocomplete="off"
            />
            <button onclick={() => showGifPanel = false} class="close-drawer-btn">✕</button>
          </div>

          <div class="gif-results-grid">
            {#if !klipyApiKey}
                <span class="gif-notice">GIF search needs a Klipy API key — add one under GIF Search in the sidebar</span>
              {:else if isSearchingGifs}
                <span class="gif-notice">Querying decentralized channels...</span>
              {:else if searchResults.length === 0}
                <span class="gif-notice">Type something to load matching expressions...</span>
            {/if}
            {#each searchResults as gif}
              {@const tileUrl = gifTileUrl(gif)}
              {#if tileUrl}
                <button onclick={() => shareGif(tileUrl)} class="gif-tile-btn">
                  <img src={tileUrl} alt={gif.title || "Klipy GIF"} loading="lazy" />
                </button>
              {/if}
            {/each}
          </div>
        </div>
      {/if}

      <div class="portsidepeer-composer-container">
          {#if showEmojiPicker}
            <div class="emoji-picker-popover" use:clickOutside={closeEmojiPicker}>
              <div class="emoji-search-bar">
                <input bind:value={emojiSearchQuery} placeholder="Search emoji…" autocomplete="off" />

                {#if emojiSearchQuery}
                  <button type="button" class="emoji-search-clear" onclick={() => (emojiSearchQuery = '')}>✕</button>
                {/if}
              </div>

              {#if emojiMode === 'react'}
                <div class="emoji-react-banner">
                  <strong>Reacting</strong>
                  <span class="emoji-react-preview" title={reactTarget?.preview}>
                    {reactTarget?.preview?.slice(0, 60) ?? ''}
                  </span>
                </div>
              {/if}

              {#if emojiSearchResults}
                <div class="emoji-grid">
                  {#if emojiSearchResults.length === 0}
                    <span class="emoji-empty">No emoji match “{emojiSearchQuery}”</span>
                  {/if}
                  {#each emojiSearchResults as { emoji } (emoji)}
                    <button type="button" class="emoji-cell" onclick={() => pickEmoji(emoji)}>{emoji}</button>
                  {/each}
                </div>
              {:else}
                <div class="emoji-category-tabs">
                  {#each EMOJI_CATEGORIES as cat, idx}
                    <button
                      type="button"
                      class="emoji-tab"
                      class:active={activeEmojiCategory === idx}
                      title={cat.name}
                      onclick={() => (activeEmojiCategory = idx)}
                    >{cat.icon}</button>
                  {/each}
                </div>
                <div class="emoji-grid">
                  {#each (EMOJI_CATEGORIES[activeEmojiCategory] ?? EMOJI_CATEGORIES[0]).emojis as emoji}
                    <button type="button" class="emoji-cell" onclick={() => pickEmoji(emoji)}>{emoji}</button>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}

          {#if replyTo}
            <div class="reply-composer-bar">
              <span class="reply-quote-bar" aria-hidden="true"></span>
              <div class="reply-composer-info">
                <span class="reply-composer-label">Replying to <strong>{replyTo.name}</strong></span>
                <span class="reply-composer-preview">{replyTo.preview}</span>
              </div>
              <button type="button" class="reply-cancel-btn" onclick={cancelReply} aria-label="Cancel reply">✕</button>
            </div>
          {/if}

        <form onsubmit={sendMessage} class="portsidepeer-input-capsule">

            <button
              type="button"
              class="portsidepeer-action-btn attach-btn"
              aria-label="Upload file"
              onclick={attachFile}
            >
              ➕
            </button>

          <input
            bind:this={composerInputEl}
            bind:value={inputMessage}
            placeholder="Message #sbb-lounge"
            autocomplete="off"
            class="portsidepeer-text-field"
          />

          <div class="portsidepeer-actions-group">
            <button
              type="button"
              class="portsidepeer-action-btn gif-btn"
              onclick={() => showGifPanel = !showGifPanel}
              aria-label="Toggle GIF panel"
            >
              GIF
            </button>

            <button
              type="button"
              class="portsidepeer-action-btn emoji-btn"
              onclick={(e) => {
                e.stopPropagation();
                emojiMode = 'compose';
                reactTarget = null;
                showEmojiPicker = !showEmojiPicker;
              }}
              aria-label="Toggle emoji picker"
            >
              😀
            </button>
          </div>

          <button type="submit" class="portsidepeer-send-btn">Send</button>
        </form>
      </div>
    </div>
  </section>
</main>

<style>

  .app-layout {
    display: flex;
    width: 100vw;
    height: 100vh;
    background: #090d16;
    color: #e2e8f0;
    overflow: hidden;
  }

  .sidebar {
    width: 280px;
    min-width: 280px;
    background: #0f1115;
    display: flex;
    flex-direction: column;
    height: 100%;
    box-sizing: border-box;
    border-right: 1px solid #1f232b;
  }

  .sidebar-header {
    height: 48px;
    padding: 0 16px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid #1f232b;
    box-shadow: 0 1px 3px rgba(0,0,0,0.4);
  }

  .sidebar-header h2 {
    margin: 0;
    font-size: 14px;
    color: #e2e8f0;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .sidebar-content {
    flex-grow: 1;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    overflow-y: auto;
  }

  .panel-section h4 {
    margin: 0 0 8px 0;
    font-size: 11px;
    text-transform: uppercase;
    color: #475569;
    letter-spacing: 0.5px;
  }

  .id-share-box {
    width: 100%;
    height: 50px;
    background: #090d16;
    border: 1px solid #1f232b;
    border-radius: 4px;
    color: #10b981;
    font-family: monospace;
    font-size: 11px;
    padding: 8px;
    resize: none;
    box-sizing: border-box;
    outline: none;
  }

  .discovery-alert-zone {
    background: rgba(16, 185, 129, 0.05);
    border: 1px dashed #10b981;
    padding: 12px;
    border-radius: 4px;
  }

  .pulse-text {
    color: #10b981 !important;
    font-weight: 700;
    animation: blinker 2s linear infinite;
  }

  @keyframes blinker {
    50% { opacity: 0.4; }
  }

  .discovery-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 8px;
    background: #090d16;
    padding: 8px;
    border-radius: 4px;
    border: 1px solid #1f232b;
  }

  .disc-id {
    font-family: monospace;
    font-size: 11px;
    color: #e2e8f0;
  }

  .accept-invite-btn {
    background: #10b981;
    border: none;
    color: #090d16;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: 4px;
    font-weight: 700;
    cursor: pointer;
    transition: background-color 0.15s ease;
  }

  .accept-invite-btn:hover {
    background: #059669;
  }

  .sidebar-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sidebar-form input {
    background: #090d16;
    border: 1px solid #1f232b;
    padding: 10px;
    color: #e2e8f0;
    border-radius: 4px;
    outline: none;
    font-size: 13px;
  }

  .sidebar-form input::placeholder {
    color: #475569;
  }

  .sidebar-form button {
    background: #1f232b;
    border: 1px solid #334155;
    color: #e2e8f0;
    padding: 10px;
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
    font-size: 13px;
    transition: all 0.15s ease;
  }

  .sidebar-form button:hover {
    background: #334155;
    border-color: #10b981;
  }

  .allowed-list-box {
    flex-grow: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-height: 150px;
  }

  .allowed-scroll {
    flex-grow: 1;
    overflow-y: auto;
    background: #090d16;
    border: 1px solid #1f232b;
    border-radius: 4px;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .peer-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px;
  }

  .peer-id-text {
    flex-grow: 1;
    font-family: monospace;
    font-size: 12px;
    color: #94a3b8;
  }

  .revoke-btn {
    background: transparent;
    border: none;
    color: #475569;
    font-size: 11px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
    font-weight: bold;
    transition: all 0.15s ease;
  }

  .revoke-btn:hover {
    color: #ffffff;
    background-color: #ef4444;
  }

  .key-icon {
    margin-right: 6px;
    filter: sepia(100%) hue-rotate(90deg) saturate(300%);
  }

  .empty-notice {
    font-size: 11px;
    color: #475569;
    text-align: center;
    font-style: italic;
    margin-top: 15px;
    line-height: 1.4;
  }

  .profile-drawer {
    background: #090d16;
    padding: 12px 16px;
    height: 52px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    border-top: 1px solid #1f232b;
  }

  .profile-view {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
  }

  .profile-name {
    font-weight: 600;
    color: #e2e8f0;
    font-size: 14px;
  }

  .edit-btn {
    background: #1f232b;
    border: 1px solid #334155;
    color: #e2e8f0;
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
    transition: border-color 0.15s ease;
  }

  .edit-btn:hover {
    border-color: #10b981;
  }

  .profile-edit {
    display: flex;
    gap: 6px;
    width: 100%;
    align-items: center;
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

  .edit-actions {
    display: flex;
    gap: 4px;
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

  .server-info h3 {
    margin: 0;
    font-size: 15px;
    color: #e2e8f0;
    font-weight: 600;
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
  }

  .message-log {
    flex-grow: 1;
    overflow-y: auto;
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

  .message-card {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    padding: 4px 0;
    position: relative;
  }

  .avatar-mock {
    width: 40px;
    height: 40px;
    background: #1f232b;
    border: 1px solid #334155;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #10b981;
    font-weight: bold;
    font-size: 14px;
    flex-shrink: 0;
  }

  .message-content {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .user-badge {
    color: #e2e8f0;
    font-weight: 600;
    font-size: 15px;
  }

  .user-badge.self-user {
    color: #10b981;
  }

  .msg-body {
    margin: 0;
    color: #94a3b8;
    word-break: break-word;
    font-size: 15px;
    line-height: 1.5;
  }

  .gif-search-drawer {
    background: #2b2d31;
    border: 1px solid #1f2023;
    border-radius: 8px;
    margin-bottom: 12px;
    display: flex;
    flex-direction: column;
    height: 320px;
    box-shadow: 0 4px 12px rgba(0,0,0,0.3);
    z-index: 100;
    flex-shrink: 0;
  }
  .drawer-header {
    display: flex;
    padding: 10px;
    gap: 10px;
    border-bottom: 1px solid #1f2023;
  }
  .drawer-header input {
    flex-grow: 1;
    background: #1e1f22;
    border: none;
    padding: 10px;
    color: white;
    border-radius: 4px;
    outline: none;
    font-size: 14px;
  }
  .close-drawer-btn {
    background: transparent;
    border: none;
    color: #949ba4;
    font-size: 16px;
    cursor: pointer;
  }
  .close-drawer-btn:hover { color: #f2f3f5; }

  .gif-results-grid {
    flex-grow: 1;
    overflow-y: auto;
    padding: 10px;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    align-content: start;
  }
  .gif-tile-btn {
    background: #1e1f22;
    border: none;
    padding: 0;
    margin: 0;
    cursor: pointer;
    border-radius: 4px;
    overflow: hidden;
    height: 90px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .gif-tile-btn img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.2s ease;
  }
  .gif-tile-btn:hover img { transform: scale(1.05); }
  .gif-notice {
    grid-column: span 3;
    text-align: center;
    color: #949ba4;
    font-size: 13px;
    margin-top: 40px;
    font-style: italic;
  }

  .gif-media-frame {
    margin-top: 4px;
    width: 280px;
    height: 210px;
    border-radius: 6px;
    overflow: hidden;
    background-color: #1e1f22;
    display: flex;
    flex-shrink: 0;
  }
  .gif-media-frame img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .gif-fallback-link {
    display: inline-block;
    margin-top: 4px;
    max-width: 280px;
    color: #10b981;
    font-size: 13px;
    text-decoration: underline;
    word-break: break-all;
  }

  .gif-fallback-link:hover {
    color: #34d399;
  }

  .portsidepeer-composer-container {
    position: relative;
    width: 100%;
    padding: 0 16px 24px 16px;
    box-sizing: border-box;
  }

  .portsidepeer-input-capsule {
    display: flex;
    align-items: center;
    background-color: #0f1115;
    border: 1px solid #1f232b;
    border-radius: 4px;
    padding: 10px 16px;
    gap: 12px;
    width: 100%;
    box-sizing: border-box;
  }

  .portsidepeer-text-field {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #e2e8f0;
    font-size: 1rem;
    font-family: inherit;
    padding: 0;
  }

  .portsidepeer-text-field::placeholder {
    color: #475569;
  }

  .portsidepeer-actions-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .portsidepeer-action-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 4px;
    font-size: 1.3rem;
    display: flex;
    align-items: center;
    justify-content: center;
    filter: grayscale(100%) brightness(0.7);
    transition: filter 0.15s ease, transform 0.1s ease;
  }

  .portsidepeer-action-btn:hover {
    filter: grayscale(0%) brightness(1);
    transform: scale(1.05);
  }

  .portsidepeer-action-btn.gif-btn {
    background-color: #1f232b;
    color: #94a3b8;
    font-size: 0.75rem;
    font-weight: 700;
    padding: 3px 6px;
    border-radius: 4px;
    filter: none;
  }

  .portsidepeer-action-btn.gif-btn:hover {
    background-color: #334155;
    color: #10b981;
  }

  .portsidepeer-send-btn {
    background-color: #10b981;
    color: #090d16;
    border: none;
    padding: 6px 14px;
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
    font-size: 0.85rem;
    transition: background-color 0.15s ease, transform 0.1s ease;
  }

  .portsidepeer-send-btn:hover {
    background-color: #059669;
  }

  .emoji-picker-popover {
    position: absolute;
    bottom: 100%;
    right: 16px;
    z-index: 1000;
    margin-bottom: 8px;
    width: 328px;
    height: 340px;
    background: #0f1115;
    border: 1px solid #1f232b;
    border-radius: 4px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .emoji-category-tabs {
    display: flex;
    background: #090d16;
    border-bottom: 1px solid #1f232b;
    flex-shrink: 0;
  }

  .emoji-tab {
    flex: 1;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 7px 0 5px 0;
    font-size: 15px;
    cursor: pointer;
    filter: grayscale(100%) brightness(0.8);
    transition: filter 0.15s ease, border-color 0.15s ease, background-color 0.15s ease;
  }

  .emoji-tab:hover {
    filter: none;
  }

  .emoji-tab.active {
    filter: none;
    border-bottom-color: #10b981;
    background: #0f1115;
  }

  .emoji-grid {
    flex-grow: 1;
    overflow-y: auto;
    padding: 8px;
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 2px;
    align-content: start;
  }

  .emoji-cell {
    background: transparent;
    border: none;
    border-radius: 4px;
    font-size: 20px;
    line-height: 1.4;
    padding: 3px 0;
    cursor: pointer;
    transition: background-color 0.1s ease, transform 0.1s ease;
  }

  .emoji-cell:hover {
    background: #1f232b;
    transform: scale(1.2);
  }

  .relay-status-box {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #090d16;
    border: 1px solid #1f232b;
    padding: 8px 10px;
    border-radius: 4px;
    font-size: 12px;
    color: #94a3b8;
    font-weight: 600;
  }

  .relay-status-box.active {
    border-color: #10b981;
    color: #10b981;
  }

  .relay-indicator {
    width: 8px;
    height: 8px;
    background-color: #10b981;
    border-radius: 50%;
    box-shadow: 0 0 6px #10b981;
    animation: blinker 2s linear infinite;
  }

  .clear-relay-btn {
    margin-left: auto;
    background: transparent;
    border: 1px solid #475569;
    color: #94a3b8;
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .clear-relay-btn:hover {
    border-color: #ef4444;
    color: #ef4444;
  }

  .relay-notice {
    margin: 8px 0 0 0;
    font-size: 10px;
    color: #f59e0b;
    font-style: italic;
    text-align: center;
  }

  .reaction-row {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
  }

  .reaction-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: #0f1115;
    border: 1px solid #1f232b;
    border-radius: 6px;
    padding: 1px 8px;
    cursor: pointer;
    font-size: 13px;
    color: #94a3b8;
    transition: border-color 0.15s ease, background-color 0.15s ease;
  }

  .reaction-pill:hover {
    border-color: #334155;
    background: #1f232b;
  }

  .reaction-pill.mine {
    background: rgba(16, 185, 129, 0.12);
    border-color: #10b981;
    color: #10b981;
  }

  .reaction-count {
    font-size: 11px;
    font-weight: 700;
  }

  .reaction-add-wrap {
    position: absolute;
    top: 2px;
    right: 4px;
    z-index: 20;
  }

  .reaction-add-btn {
    background: #0f1115;
    border: 1px solid #1f232b;
    border-radius: 6px;
    color: #94a3b8;
    font-size: 13px;
    padding: 2px 7px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s ease, border-color 0.15s ease;
  }

  .message-card:hover .reaction-add-btn {
    opacity: 1;
  }

  .reaction-add-btn:hover {
    border-color: #10b981;
    color: #10b981;
  }

  .reaction-quick-bar {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 30;
    display: flex;
    gap: 2px;
    padding: 4px;
    background: #0f1115;
    border: 1px solid #1f232b;
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  }

  .reaction-quick-emoji {
    background: transparent;
    border: none;
    border-radius: 4px;
    font-size: 18px;
    padding: 2px 4px;
    cursor: pointer;
    transition: background-color 0.1s ease, transform 0.1s ease;
  }

  .reaction-quick-emoji:hover {
    background: #1f232b;
    transform: scale(1.15);
  }

  .reaction-more-btn {
    color: #94a3b8;
    font-weight: 700;
  }

  .reaction-more-btn:hover {
    color: #10b981;
  }

  .emoji-search-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 10px;
    background: #090d16;
    border-bottom: 1px solid #1f232b;
    flex-shrink: 0;
  }

  .emoji-search-bar input {
    flex: 1;
    background: #0f1115;
    border: 1px solid #1f232b;
    border-radius: 4px;
    padding: 6px 8px;
    color: #e2e8f0;
    font-size: 13px;
    outline: none;
  }

  .emoji-search-bar input:focus {
    border-color: #10b981;
  }

  .emoji-search-clear {
    background: transparent;
    border: none;
    color: #475569;
    cursor: pointer;
    font-size: 12px;
  }

  .emoji-search-clear:hover {
    color: #e2e8f0;
  }

  .emoji-react-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    background: rgba(16, 185, 129, 0.08);
    border-bottom: 1px solid #1f232b;
    font-size: 12px;
    color: #94a3b8;
    flex-shrink: 0;
  }

  .emoji-react-banner strong {
    color: #10b981;
  }

  .emoji-react-preview {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .emoji-empty {
    grid-column: 1 / -1;
    text-align: center;
    color: #475569;
    font-size: 12px;
    font-style: italic;
    padding: 32px 0;
  }

  @media (pointer: coarse) {
    .reaction-add-btn {
      opacity: 1;
    }
  }

  .file-card {
    display: flex;
    align-items: center;
    gap: 10px;
    background: #0f1115;
    border: 1px solid #1f232b;
    border-radius: 6px;
    padding: 8px 12px;
    max-width: 320px;
  }
  .file-icon { font-size: 20px; }

  .file-details { display: flex; flex-direction: column; min-width: 0; flex: 1; }

  .file-name {
    color: #e2e8f0; font-size: 13px; font-weight: 600;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  .file-size { color: #475569; font-size: 11px; }

  .file-open-btn {
    background: #10b981; border: none; color: #090d16;
    padding: 4px 10px; border-radius: 4px; font-weight: 700;
    font-size: 11px; cursor: pointer;
  }

  .file-open-btn:hover { background: #059669; }

  .file-progress-label { color: #10b981; font-size: 11px; font-weight: 700; flex-shrink: 0; }

  .channel-list { display: flex; flex-direction: column; gap: 2px; }

  .channel-row {
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    color: #94a3b8;
    padding: 6px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
    text-align: left;
    transition: background-color 0.15s ease, color 0.15s ease;
  }

  .channel-row:hover { background: #1f232b; color: #e2e8f0; }
  .channel-row.active { background: #1f232b; color: #10b981; font-weight: 600; }

  .channel-row .hash-tag { font-size: 14px; }
  .channel-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .add-channel-btn {
    width: 100%;
    margin-top: 6px;
    background: transparent;
    border: 1px dashed #1f232b;
    color: #475569;
    padding: 6px;
    border-radius: 4px;
    font-size: 11px;
    cursor: pointer;
    transition: border-color 0.15s ease, color 0.15s ease;
  }
  .add-channel-btn:hover { border-color: #10b981; color: #10b981; }

  .channel-rename-form { display: flex; gap: 6px; align-items: center; }

  .rename-hint { opacity: 0; font-size: 11px; transition: opacity 0.15s ease; }
  .channel-title-btn {
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

  .channel-title-btn:hover { background: #1f232b; }

  .channel-title-text {
    font-size: 15px;
    font-weight: 600;
  }

  .rename-hint {
    opacity: 0;
    font-size: 11px;
    transition: opacity 0.15s ease;
  }

  .channel-title-btn:hover .rename-hint,
  .channel-title-btn:focus-visible .rename-hint {
    opacity: 1;
  }

  .reply-quote {
    display: flex;
    align-items: center;
    gap: 6px;
    background: #0f1115;
    border: 1px solid #1f232b;
    border-radius: 4px;
    padding: 3px 8px;
    max-width: 400px;
    cursor: pointer;
    font-size: 12px;
    color: #475569;
    text-align: left;
    transition: border-color 0.15s ease, color 0.15s ease;
  }

  .reply-quote:hover { border-color: #10b981; color: #94a3b8; }

  .reply-quote-ghost { cursor: default; }
  .reply-quote-ghost:hover { border-color: #1f232b; color: #475569; }

  .reply-quote-bar {
    width: 2px;
    align-self: stretch;
    background: #10b981;
    border-radius: 1px;
    flex-shrink: 0;
  }

  .reply-quote-sender { color: #10b981; font-weight: 600; flex-shrink: 0; }

  .reply-quote-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .reply-composer-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #0f1115;
    border: 1px solid #1f232b;
    border-left: 2px solid #10b981;
    border-radius: 4px;
    padding: 6px 10px;
    margin-bottom: 8px;
  }

  .reply-composer-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .reply-composer-label { font-size: 12px; color: #94a3b8; }
  .reply-composer-preview {
    font-size: 12px;
    color: #475569;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .reply-cancel-btn {
    background: transparent;
    border: none;
    color: #475569;
    font-size: 12px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .reply-cancel-btn:hover { color: #ef4444; background: #1f232b; }

  </style>
