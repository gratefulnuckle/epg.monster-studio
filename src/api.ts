import { invoke } from "@tauri-apps/api/core";

export type Source = {
  id: string;
  name: string;
  kind: string;
  location: string;
  headersJson?: string;
  channelCount: number;
  expiresAt?: number | null;
};

export type Group = { title: string; count: number };

export type Channel = {
  id: string;
  sourceId: string;
  groupTitle: string;
  name: string;
  tvgId?: string | null;
  tvgLogo?: string | null;
  url: string;
};

export type ChannelAudit = {
  ok: boolean;
  grade: string;
  error?: string | null;
  latencyMs?: number | null;
  width?: number | null;
  height?: number | null;
  fps?: number | null;
  aspectRatio?: string | null;
  videoCodec?: string | null;
  audioCodec?: string | null;
};

export const api = {
  listSources: () => invoke<Source[]>("list_sources"),
  listGroups: (sourceId: string) => invoke<Group[]>("list_groups", { sourceId }),
  listChannels: (sourceId: string, groupTitle: string, limit = 5000) =>
    invoke<Channel[]>("list_channels", { sourceId, groupTitle, limit }),
  searchSources: (query: string) => invoke<Channel[]>("search_sources", { query }),
  removeSource: (sourceId: string) => invoke<void>("remove_source", { sourceId }),
  pickSourceFile: () => invoke<string | null>("pick_source_file"),
  addSourceUrl: (url: string, name?: string, headers?: Record<string, string>) =>
    invoke<Source>("add_source_url", { args: { url, name, headers } }),
  probeXtreamExpiry: (sourceId: string) =>
    invoke<number | null>("probe_xtream_expiry", { sourceId }),
  addSourceXtream: (args: {
    server: string;
    username: string;
    password: string;
    output?: string;
    name?: string;
    headers?: Record<string, string>;
  }) => invoke<Source>("add_source_xtream", { args }),
  updateSource: (args: {
    id: string;
    name: string;
    kind: string;
    location: string;
    headers?: Record<string, string>;
    refetch?: boolean;
  }) => invoke<Source>("update_source", { args }),
  playUrl: (url: string, sourceId?: string) => invoke<void>("play_url", { url, sourceId }),
  auditSourceChannel: (url: string, sourceId?: string) =>
    invoke<ChannelAudit>("audit_source_channel", { url, sourceId }),
  refreshSource: (sourceId: string) => invoke<Source>("refresh_source", { sourceId }),
  managedCount: () => invoke<number>("managed_count"),
  addBackupFromSource: (managedId: string, entryId: string) =>
    invoke<string>("add_backup_from_source", { managedId, entryId }),
  listManaged: (group?: string) =>
    invoke<{ id: string; name: string; groupTitle: string }[]>("list_managed", { group }),
};
