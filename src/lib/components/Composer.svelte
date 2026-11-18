<script lang="ts">
  import type { KlipyGif, ReactTarget } from '$lib/types';
  import { gifTileUrl, stripVariation } from '$lib/markers';

  type ReplyTarget = {
    key: string;
    name: string;
    preview: string;
  };

  interface Props {
    showEmojiPicker?: boolean;
    emojiMode?: 'compose' | 'react';
    reactTarget?: ReactTarget | null;
    replyTo: ReplyTarget | null;
    klipyApiKey: string;
    recentEmoji: string[];
    onSend: (text: string) => void;
    onAttach: () => void;
    onShareGif: (url: string) => void;
    onCancelReply: () => void;
    onPushRecentEmoji: (emoji: string) => void;
    onToggleReaction: (key: string, emoji: string) => void;
    onCloseReactionPicker: () => void;
  }

  let {
    showEmojiPicker = $bindable(false),
    emojiMode = $bindable('compose'),
    reactTarget = $bindable(null),
    replyTo,
    klipyApiKey,
    recentEmoji,
    onSend,
    onAttach,
    onShareGif,
    onCancelReply,
    onPushRecentEmoji,
    onToggleReaction,
    onCloseReactionPicker
  }: Props = $props();

  let inputMessage = $state('');
  let showGifPanel = $state(false);
  let gifSearchQuery = $state('');
  let searchResults = $state<KlipyGif[]>([]);
  let isSearchingGifs = $state(false);
  let emojiSearchQuery = $state('');
  let composerInputEl: HTMLInputElement;

  export function focusInput() {
    composerInputEl?.focus();
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
        '🎟️','🎫'
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
    '💩': 'poo poop',
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

  function closeEmojiPicker() {
    showEmojiPicker = false;
    emojiMode = 'compose';
    reactTarget = null;
    emojiSearchQuery = '';
  }

  function pickEmoji(emoji: string) {
    onPushRecentEmoji(emoji);
    if (emojiMode === 'react' && reactTarget) {
      onToggleReaction(reactTarget.key, emoji);
      closeEmojiPicker();
      return;
    }
    inputMessage += emoji;
    composerInputEl?.focus();
  }

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

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!inputMessage.trim()) return;
    onSend(inputMessage);
    inputMessage = '';
    showEmojiPicker = false;
  }

  // parity with the old page behavior: opening the react picker resets
  // search + category even though those states now live here
  $effect(() => {
    if (emojiMode === 'react' && showEmojiPicker) {
      emojiSearchQuery = '';
      activeEmojiCategory = 0;
    }
  });
</script>

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
                <span class="gif-notice">Querying decentralized rooms...</span>
              {:else if searchResults.length === 0}
                <span class="gif-notice">Type something to load matching expressions...</span>
            {/if}
            {#each searchResults as gif}
              {@const tileUrl = gifTileUrl(gif)}
              {#if tileUrl}
                <button onclick={() => { onShareGif(tileUrl); showGifPanel = false; gifSearchQuery = ''; searchResults = []; }} class="gif-tile-btn">
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

              {#if !emojiSearchResults && recentEmoji.length > 0}
                <div class="emoji-recents-row">
                    <span class="emoji-recents-label">Recent</span>
                    <div class="emoji-recents-grid">
                        {#each recentEmoji as emoji (emoji)}
                          <button
                            type="button"
                            class="emoji-cell emoji-recent-cell"
                            onclick={() => pickEmoji(emoji)}
                          >{emoji}</button>
                        {/each}
                    </div>
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
              <button type="button" class="reply-cancel-btn" onclick={onCancelReply} aria-label="Cancel reply">✕</button>
            </div>
          {/if}

        <form onsubmit={submit} class="portsidepeer-input-capsule">

            <button
              type="button"
              class="portsidepeer-action-btn attach-btn"
              aria-label="Upload file"
              onclick={onAttach}
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
                onCloseReactionPicker();
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

<style>
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
    right: 32px;
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

  .emoji-recents-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px 6px 12px;
    background: #090d16;
    border-bottom: 1px solid #1f232b;
    flex-shrink: 0;
  }

  .emoji-recents-label {
    font-size: 10px;
    text-transform: uppercase;
    color: #475569;
    letter-spacing: 0.5px;
    flex-shrink: 0;
  }

  .emoji-recents-grid {
    display: flex;
    gap: 2px;
    overflow-x: auto;
    flex: 1;
  }

  .emoji-recent-cell {
    font-size: 17px;
    padding: 2px 0;
  }

  .emoji-recent-cell:hover {
    background: #1f232b;
    transform: scale(1.2);
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

  .reply-quote-bar {
    width: 2px;
    align-self: stretch;
    background: #10b981;
    border-radius: 1px;
    flex-shrink: 0;
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
