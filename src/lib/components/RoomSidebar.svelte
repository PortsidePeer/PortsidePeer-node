<script lang="ts">
  import type { Room } from '$lib/types';
  import { shortPeer, slugify } from '$lib/markers';

  interface Props {
    /** Identity + profile (shared, page-owned). */
    userPeerId: string;
    userNickname: string;
    myAvatar: string | null;
    /** Relay panel state (page-owned: relay-status events write it). */
    relaySaved: boolean;
    relayConnected: boolean;
    relayListening: boolean;
    myCircuitAddress: string;
    /** Rooms. */
    rooms: Room[];
    activeRoom: string;
    unreadByRoom: Record<string, number>;
    mutedRooms: Record<string, boolean>;
    /** Notifications + GIF key. */
    notificationsEnabled: boolean;
    klipySaved: boolean;
    /** Peers. */
    discoveredPeers: string[];
    allowedPeers: string[];
    /** Mutations out. Boolean-returning callbacks resolve true on success;
    the component resets its local form state only then (parity with the
    original handlers, which reset inside try after the invoke resolved). */
    onSwitchRoom: (id: string) => void;
    onToggleRoomMute: (id: string) => void;
    onToggleNotifications: () => void;
    onDisableRelay: () => Promise<boolean>;
    onClearKlipyKey: () => void;
    onAcceptPeer: (peerId: string) => void;
    onRevokePeer: (peerId: string) => void;
    onPickAvatar: () => void;
    onSaveRelay: (peerId: string, address: string) => Promise<boolean>;
    onSaveKlipyKey: (apiKey: string) => Promise<boolean>;
    onGrantPeer: (peerId: string) => void;
    onRenameProfile: (nickname: string) => Promise<boolean>;
    onCreateRoom: (id: string, name: string) => Promise<boolean>;
  }

  let {
    userPeerId,
    userNickname,
    myAvatar = null,
    relaySaved,
    relayConnected,
    relayListening,
    myCircuitAddress,
    rooms,
    activeRoom,
    unreadByRoom,
    mutedRooms,
    notificationsEnabled,
    klipySaved,
    discoveredPeers,
    allowedPeers,
    onSwitchRoom,
    onToggleRoomMute,
    onToggleNotifications,
    onDisableRelay: onDisableRelayAsync,
    onClearKlipyKey,
    onAcceptPeer,
    onRevokePeer,
    onPickAvatar,
    onSaveRelay,
    onSaveKlipyKey,
    onGrantPeer,
    onRenameProfile,
    onCreateRoom
  }: Props = $props();

  // sidebar-local form state
  let showNewRoom = $state(false);
  let newRoomInput = $state('');
  let relayPeerId = $state('');
  let relayAddress = $state('');
  let klipyKeyInput = $state('');
  let targetFriendId = $state('');
  let editMode = $state(false);
  let nicknameInput = $state('');

  // prime the rename form whenever the profile loads/changes upstream
  $effect(() => {
    nicknameInput = userNickname === 'Loading...' ? '' : userNickname;
  });

  // form submits: validate, delegate, reset local state on success
  async function submitRelay(e: SubmitEvent) {
    e.preventDefault();
    if (!relayPeerId.trim() || !relayAddress.trim()) return;
    if (await onSaveRelay(relayPeerId, relayAddress)) {
      relayPeerId = '';
      relayAddress = '';
    }
  }

  async function submitNewRoom(e: SubmitEvent) {
    e.preventDefault();
    const name = newRoomInput.trim();
    if (!name) return;
    let id = slugify(name);
    if (rooms.some((c) => c.id === id)) id = `${id}-${Math.random().toString(36).slice(2, 6)}`;
    if (await onCreateRoom(id, name)) {
      newRoomInput = '';
      showNewRoom = false;
    }
  }

  async function submitKlipyKey(e: SubmitEvent) {
    e.preventDefault();
    if (!klipyKeyInput.trim()) return;
    if (await onSaveKlipyKey(klipyKeyInput.trim())) {
      klipyKeyInput = '';
    }
  }

  function submitManualGrant(e: SubmitEvent) {
    e.preventDefault();
    if (!targetFriendId.trim()) return;
    onGrantPeer(targetFriendId.trim());
    targetFriendId = '';
  }

  async function submitNickname(e: SubmitEvent) {
    e.preventDefault();
    if (await onRenameProfile(nicknameInput)) {
      editMode = false;
    }
  }

  // the Disable button also clears the form fields the page used to reset
  // in clearRelay() — they live here now, so it takes a callback that
  // signals completion, and this wrapper wipes the local inputs.
  let disableInFlight = $state(false);
  async function disableRelay() {
    if (disableInFlight) return;
    disableInFlight = true;
    try {
      await onDisableRelayAsync();
      relayPeerId = '';
      relayAddress = '';
    } finally {
      disableInFlight = false;
    }
  }
</script>

  <aside class="sidebar">
    <div class="sidebar-header">
      <h2>PortsidePeer settings</h2>
    </div>

    <div class="sidebar-content">
      <div class="panel-section">
        <h4>My Identity Key</h4>
        <p>Give this to friends so they can whitelist you:</p>
        <textarea readonly class="id-share-box" value={userPeerId} onclick={(e) => e.currentTarget.select()}></textarea>
      </div>

      <div class="panel-section">
        <h4>Circuit Relay</h4>

        {#if relaySaved}
          <div class="relay-status-box" class:active={relayConnected && relayListening}>
            <span class="relay-indicator"></span>
            <span>{relayConnected && relayListening ? 'Relay Connected' : 'Connecting...'}</span>
            <button onclick={disableRelay} class="clear-relay-btn">Disable</button>
          </div>

          {#if myCircuitAddress}
            <textarea readonly class="id-share-box" style="height: 35px; margin-top: 8px; font-size: 10px; color: #94a3b8;" onclick={(e) => e.currentTarget.select()}>{myCircuitAddress}</textarea>
            <p class="relay-notice">Click address to select • Share with peers to connect</p>
          {:else if !relayConnected}
            <p class="relay-notice">⚠️ Restart required to connect</p>
          {/if}

        {:else}
          <form onsubmit={submitRelay} class="sidebar-form">
            <input bind:value={relayPeerId} placeholder="Relay Peer ID..." autocomplete="off" />
            <input bind:value={relayAddress} placeholder="/ip4/1.2.3.4/tcp/4001" autocomplete="off" />
            <button type="submit">Save & Enable Relay</button>
          </form>
        {/if}
      </div>

      <div class="panel-section">
        <h4>Rooms ({rooms.length})</h4>
        <div class="room-list">
          {#each rooms as room (room.id)}
              <div class="room-row-wrap">
              <button
              type="button"
              class="room-row"
              class:active={room.id === activeRoom}
              onclick={() => onSwitchRoom(room.id)}
            >
              <span class="hash-tag">#</span>
              <span class="room-name">{room.name}</span>
              {#if unreadByRoom[room.id]}
                <span class="room-badge">{unreadByRoom[room.id]}</span>
              {/if}
            </button>
            <button
                type="button"
                class="room-mute-btn"
                class:muted={mutedRooms[room.id]}
                title={mutedRooms[room.id] ? 'Unmute room' : 'Mute room'}
                aria-label={mutedRooms[room.id]
                    ? `Unmute ${room.name}`
                    : `Mute ${room.name}`}
                onclick={() => onToggleRoomMute(room.id)}
                >{mutedRooms[room.id] ? '🔕' : '🔔'}</button>
              </div>
          {/each}
        </div>
        {#if showNewRoom}
          <form onsubmit={submitNewRoom} class="sidebar-form" style="margin-top: 8px;">
            <input bind:value={newRoomInput} placeholder="Add new room!" autocomplete="off" />
            <button type="submit">Create</button>
          </form>
        {:else}
          <button type="button" class="add-room-btn" onclick={() => (showNewRoom = true)}>+ Add Room</button>
        {/if}
      </div>

      <div class="panel-section">
        <h4>Notifications</h4>
        <button
          type="button"
          class="notif-toggle-row"
          onclick={onToggleNotifications}
        >
          <span class="notif-toggle-label">{notificationsEnabled ? '🔔 Enabled' : '🔕 Muted'}</span>
          <span class="notif-toggle-track" class:on={notificationsEnabled}>
            <span class="notif-toggle-thumb"></span>
          </span>
        </button>
      </div>

      <div class="panel-section">
        <h4>GIF Search</h4>
        {#if klipySaved}
          <div class="relay-status-box active">
            <span class="relay-indicator"></span>
            <span>Klipy key configured</span>
            <button onclick={onClearKlipyKey} class="clear-relay-btn">Clear</button>
          </div>
        {:else}
          <form onsubmit={submitKlipyKey} class="sidebar-form">
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
              <button onclick={() => onAcceptPeer(peer)} class="accept-invite-btn">Connect</button>
            </div>
          {/each}
        </div>
      {/if}

      <div class="panel-section">
        <h4>Manual Authorization</h4>
        <form onsubmit={submitManualGrant} class="sidebar-form">
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
              <button onclick={() => onRevokePeer(peer)} class="revoke-btn" title="Revoke access and block peer">✕</button>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <div class="profile-drawer">
      {#if !editMode}
        <div class="profile-view">
            {#if myAvatar}
                <img class="avatar-img" src={myAvatar} alt="" />
            {:else}
            <span class="profile-name" title={userPeerId}>🌐 {userNickname}</span>
            {/if}
           <button onclick={onPickAvatar} class="edit-btn">Avatar</button>
           <button onclick={() => (editMode = true)} class="edit-btn">Edit</button>
        </div>
      {:else}
        <form onsubmit={submitNickname} class="profile-edit">
          <input bind:value={nicknameInput} placeholder="Change name..." class="edit-input" />
          <div class="edit-actions">
            <button type="submit" class="save-btn">✓</button>
            <button type="button" onclick={() => editMode = false} class="cancel-btn">✕</button>
          </div>
        </form>
      {/if}
    </div>
  </aside>

<style>
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

.hash-tag {
color: #475569;
font-size: 20px;
margin-right: 8px;
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

.room-list { display: flex; flex-direction: column; gap: 2px; }

.room-row {
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

.room-row:hover { background: #1f232b; color: #e2e8f0; }

.room-row.active { background: #1f232b; color: #10b981; font-weight: 600; }

.room-row .hash-tag { font-size: 14px; }

.room-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.add-room-btn {
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

.add-room-btn:hover { border-color: #10b981; color: #10b981; }

.notif-toggle-row {
display: flex;
align-items: center;
justify-content: space-between;
width: 100%;
background: transparent;
border: none;
color: #94a3b8;
font-size: 13px;
cursor: pointer;
padding: 4px 0;
}

.notif-toggle-label { color: #e2e8f0; }

.notif-toggle-track {
width: 34px;
height: 18px;
border-radius: 9px;
background: #1f232b;
border: 1px solid #334155;
position: relative;
transition: background-color 0.15s ease, border-color 0.15s ease;
flex-shrink: 0;
}

.notif-toggle-track.on {
background: #10b981;
border-color: #10b981;
}

.notif-toggle-thumb {
position: absolute;
top: 1px;
left: 1px;
width: 14px;
height: 14px;
border-radius: 50%;
background: #e2e8f0;
transition: transform 0.15s ease;
}

.notif-toggle-track.on .notif-toggle-thumb {
transform: translateX(16px);
}

.room-row-wrap {
display: flex;
align-items: center;
gap: 2px;
}

.room-row { flex: 1; min-width: 0; }

.room-badge {
margin-left: auto;
background: #10b981;
color: #090d16;
font-size: 10px;
font-weight: 700;
border-radius: 8px;
padding: 1px 6px;
min-width: 16px;
text-align: center;
flex-shrink: 0;
}

.room-mute-btn {
background: transparent;
border: none;
font-size: 11px;
cursor: pointer;
padding: 2px 4px;
border-radius: 4px;
opacity: 0;
transition: opacity 0.15s ease;
flex-shrink: 0;
}

.room-row-wrap:hover .room-mute-btn { opacity: 0.6; }

.room-mute-btn:hover { opacity: 1 !important; background: #1f232b; }

.room-mute-btn.muted { opacity: 0.8; }

.avatar-img {
width: 52px;
height: 52px;
border-radius: 50%;
object-fit: cover;
flex-shrink: 0;
background: #1f232b;
}
</style>
