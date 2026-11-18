<script lang="ts">
  import type { ChatMessage, FileInfo, ReactionActor, FileProgressInfo } from '$lib/types';
  import {
    displayBody, displayReply, displayText, extractGifUrl, extractImageUrl,
    extractFileInfo, extractImageInfo, formatBytes, messageByKey, messageKey,
    senderName, senderInitials
  } from '$lib/markers';

  interface Props {
    msg: ChatMessage;
    messages: ChatMessage[];
    userNickname: string;
    userPeerId: string;
    avatars: Record<string, string>;
    reactions: Record<string, Record<string, ReactionActor[]>>;
    imageSrcs: Record<string, string>;
    fileProgress: Record<string, FileProgressInfo>;
    sentFiles: Record<string, string>;
    receivedFiles: Record<string, string>;
    failedMediaUrls: string[];
    recentEmoji: string[];
    reactionPickerFor: string | null;
    reactionPickerFlip: boolean;
    messageLogEl: HTMLDivElement | null;
    onPickerFor: (key: string | null) => void;
    onPickerFlip: (flip: boolean) => void;
    onReply: (msg: ChatMessage) => void;
    onToggleReaction: (key: string, emoji: string) => void;
    onPushRecentEmoji: (emoji: string) => void;
    onOpenSharedFile: (info: FileInfo) => void;
    onMediaError: (url: string) => void;
    onOpenFullPicker: (msg: ChatMessage) => void;
  }

  let {
    msg, messages, userNickname, userPeerId, avatars, reactions, imageSrcs,
    fileProgress, sentFiles, receivedFiles, failedMediaUrls, recentEmoji,
    reactionPickerFor, reactionPickerFlip, messageLogEl,
    onPickerFor, onPickerFlip, onReply, onToggleReaction, onPushRecentEmoji,
    onOpenSharedFile, onMediaError, onOpenFullPicker
  }: Props = $props();

  const QUICK_REACTIONS = ['👍', '❤️', '😂', '😮', '😢', '🎉', '🍻', '💩'];

  // these were the {#each}-level {/@const} lines — now derived from the msg prop
  const body = $derived(displayBody(msg));
  const gifUrl = $derived(extractGifUrl(body));
  const imgUrl = $derived(extractImageUrl(body));
  const fileInfo = $derived(extractFileInfo(body));
  const isMine = $derived(senderName(msg) === userNickname);
  const mKey = $derived(messageKey(msg));
  const replyInfo = $derived(displayReply(body));
  const imageInfo = $derived(extractImageInfo(body));

  // copied from page.svelte, unchanged
  function avatarFor(m: ChatMessage): string | null {
    const pid = m.sender_pid ?? `nick:${senderName(m)}`;
    return avatars[pid] ?? null;
  }

  function me() {
    return userPeerId
      ? { p: userPeerId, n: userNickname }
      : { p: `nick:${userNickname}`, n: userNickname };
  }

  // copied from page.svelte — the page keeps its own copy for the emoji popover
  function clickOutside(node: HTMLElement, callback?: () => void) {
    const onClick = (event: MouseEvent) => {
      if (!node.contains(event.target as Node | null)) callback?.();
    };
    document.addEventListener('click', onClick);
    return {
      destroy() {
        document.removeEventListener('click', onClick);
      }
    };
  }
</script>

<div class="message-card" id={`msg-${mKey}`}>
  {#if avatarFor(msg)}
    <img class="avatar-img" src={avatarFor(msg)} alt="" />
  {:else}
    <div class="avatar-mock">{senderInitials(msg)}</div>
  {/if}
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
            alt={`GIF from ${senderName(msg)}`}
            loading="lazy"
            onerror={() => onMediaError(gifUrl)}
          />
        </div>
      {/if}
    {:else if imgUrl}
      {#if failedMediaUrls.includes(imgUrl)}
        <a class="gif-fallback-link" href={imgUrl} target="_blank" rel="noopener noreferrer" title={imgUrl}>
          Image unavailable in-app — click to open in browser
        </a>
      {:else}
        <div class="gif-media-frame">
          <img
            src={imgUrl}
            alt={`Image from ${senderName(msg)}`}
            loading="lazy"
            onerror={() => onMediaError(imgUrl)}
          />
        </div>
      {/if}
    {:else if imageInfo}
      {@const src = imageSrcs[imageInfo.id]}
      {#if src}
        <div class="gif-media-frame">
          <img
            src={src}
            alt={`Image from ${senderName(msg)}`}
            onerror={() => onMediaError(src)}
          />
        </div>
      {:else if fileProgress[imageInfo.id]}
        <div class="file-card">
          <span class="file-icon">🖼️</span>
          <div class="file-details">
            <span class="file-name" title={imageInfo.name}>{imageInfo.name}</span>
            <span class="file-size">
              {Math.round((fileProgress[imageInfo.id].received / Math.max(fileProgress[imageInfo.id].total, 1)) * 100)}%
            </span>
          </div>
        </div>
      {:else if sentFiles[imageInfo.id] || receivedFiles[imageInfo.id]}
        <span class="file-progress-label">Loading image…</span>
      {:else}
        <div class="file-card">
          <span class="file-icon">🖼️</span>
          <div class="file-details">
            <span class="file-name" title={imageInfo.name}>{imageInfo.name}</span>
            <span class="file-size">{imageInfo.size ? formatBytes(imageInfo.size) : ''}</span>
          </div>
          <span class="file-progress-label">Not downloaded</span>
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
          <button type="button" class="file-open-btn" onclick={() => onOpenSharedFile(fileInfo)}>Open</button>
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
        {@const parent = messageByKey(replyInfo.parentKey, messages)}
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
            <span class="reply-quote-text">{displayText(parent)}</span>
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
            onclick={() => {onPushRecentEmoji(emoji); onToggleReaction(mKey, emoji)}}
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
        if (reactionPickerFor === mKey) {
          onPickerFor(null);
        } else {
          const card = document.getElementById(`msg-${mKey}`);
          const log = messageLogEl;
          if (card && log) {
            const cardTop = card.getBoundingClientRect().top;
            const logBottom = log.getBoundingClientRect().bottom;
            onPickerFlip(cardTop > logBottom - 140);
          }
          onPickerFor(mKey);
        }
      }}
    >🙂</button>
    <button
      type="button"
      class="reaction-add-btn"
      aria-label="Reply to message"
      onclick={() => onReply(msg)}
    >↩️</button>
  </div>

  {#if reactionPickerFor === mKey}
    <div class="reaction-quick-bar"
      class:flip-up={reactionPickerFlip}
      use:clickOutside={() => onPickerFor(null)}
    >
      {#if recentEmoji.length > 0}
        <div class="quick-bar-recent">
          {#each recentEmoji.slice(0, 5) as emoji (emoji)}
            <button
              type="button"
              class="reaction-quick-emoji"
              onclick={() => { onPushRecentEmoji(emoji); onToggleReaction(mKey, emoji); onPickerFor(null); }}
            >{emoji}</button>
          {/each}
        </div>
        <span class="quick-bar-divider" aria-hidden="true"></span>
      {/if}
      {#each QUICK_REACTIONS as emoji}
        <button
          type="button"
          class="reaction-quick-emoji"
          onclick={() => { onPushRecentEmoji(emoji); onToggleReaction(mKey, emoji); onPickerFor(null); }}
        >{emoji}</button>
      {/each}
      <button
        type="button"
        class="reaction-quick-emoji reaction-more-btn"
        title="More reactions"
        onclick={(e) => { e.stopPropagation(); onOpenFullPicker(msg); }}
      >⋯</button>
    </div>
  {/if}
</div>

<style>
  .message-card {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    padding: 4px 0;
    position: relative;
  }

  .avatar-mock {
    width: 52px;
    height: 52px;
    border-radius: 50%;
    background: #1f232b;
    border: 1px solid #334155;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #10b981;
    font-weight: bold;
    font-size: 16px;
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

  .quick-bar-recent {
    display: flex;
    gap: 2px;
  }

  .quick-bar-divider {
    width: 1px;
    align-self: stretch;
    background: #1f232b;
    margin: 2px 3px;
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
    right: 28px;
    z-index: 1100;
    display: flex;
    gap: 2px;
    padding: 4px;
    background: #0f1115;
    border: 1px solid #1f232b;
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  }

  .reaction-quick-bar.flip-up {
    top: auto;
    bottom: calc(100% + 4px);
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
    min-width: 0;
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .avatar-img {
    width: 52px;
    height: 52px;
    border-radius: 50%;
    object-fit: cover;
    flex-shrink: 0;
    background: #1f232b;
  }
</style>
