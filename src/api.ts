import { invoke } from "@tauri-apps/api/core";

export type Source = {
  id: string;
  name: string;
  kind: string;
  location: string;
  channelCount: number;
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

export const api = {
  listSources: () => invoke<Source[]>("list_sources"),
  listGroups: (sourceId: string) => invoke<Group[]>("list_groups", { sourceId }),
  listChannels: (sourceId: string, groupTitle: string) =>
    invoke<Channel[]>("list_channels", { sourceId, groupTitle }),
  searchSources: (query: string) => invoke<Channel[]>("search_sources", { query }),
  removeSource: (sourceId: string) => invoke<void>("remove_source", { sourceId }),
  pickSourceFile: () => invoke<Source | null>("pick_source_file"),
  addSourceUrl: (url: string, name?: string, headers?: Record<string, string>) =>
    invoke<Source>("add_source_url", { args: { url, name, headers } }),
  playUrl: (url: string, sourceId?: string) => invoke<void>("play_url", { url, sourceId }),
};
