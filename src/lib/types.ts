// Wire-format types. Field names match what Rust serde sends

export type ChatMessage = {
  uid?: string;
  room?: string;
  sender: string;
  sender_pid?: string;
  channel?: string;      // legacy rows
  body: string;
  timestamp?: string;
  id?: number;           // SQLite row id
};

export type Room = {
  id: string;
  name: string;
  unread?: number;       // optional so { id, name } literals keep compiling
  muted?: boolean;
};

export type ReplyTarget = {
  key: string;
  name: string;
  preview: string
};

export type FileInfo = {
  id: string;
  name: string;
  size?: number;
};

export type ReactionActor = {
  p: string;             // peer id
  n?: string;            // nickname at reaction time
};

export type ReactionPayload = {
  e?: string;
  t?: string;
  p?: string;
  n?: string;
  a?: 'add' | 'remove';
};

export type AvatarPayload = {
  p: string;             // peer id
  d: string;             // base64 JPEG
};

export type FileProgressInfo = {
  received: number;
  total: number;
};

export type ReactTarget = {
  key: string;           // messageKey being reacted to
  preview: string;       // short preview for the picker
};

export type KlipyGif = {
  title?: string;
  file?: {
      hd?: { gif?: { url?: string } };
      sd?: { gif?: { url?: string } };
  };
};

export type PeerInfo = {
  pid: string;        // adjust to match the Rust struct's serde field
  nickname?: string;  // names exactly (snake_case unless renamed)
};

export type RoomEventPayload = {
  a: 'create' | 'rename';
  id: string;
  name: string;
};

export type ReplyInfo = {
  parentKey: string | null;  // messageKey of the message being replied to
  text: string;              // display text, reply marker stripped
};

export interface SharedFileInfo {
  id: string;
  name: string;
  size: number;
}
