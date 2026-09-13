# PortsidePeer

Decentralized peer to peer chat app built for the user and not for data centers. It was built with local first in mind, you can use it on your local network without being connected to the world wide web. To use it over the www all you need is a relay!

### Features (so far)

- Automatically discover other PortsidePeer clients on the local network.
- All messages are encrypted by default with the NOISE protocol.
- Allow list based friend lists.
- Send gifs and paste gif urls. 
- React to messages with emojis, emoji search, etc.
- Send Files to everyone in the chatroom.
- Message history stored locally.

### Running from source

After installing the prerequisites for Tauri clone the repo and run `npm run tauri dev`.

### Building from source

`npm run tauri build`

For some reason no Arch based distros this is needed.

`NO_STRIP=true npm run tauri build`

### Releases (soon)
