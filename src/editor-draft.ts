export const EDITOR_DRAFT_KEY = "studio-editor-draft";

export type SourceChannelDraft = {
  name: string;
  groupTitle: string;
  tvgId?: string | null;
  tvgLogo?: string | null;
  url: string;
};

export type DraftManaged = {
  id: string;
  name: string;
  groupTitle: string;
  tvgId: string | null;
  tvgLogo: string | null;
  notes: null;
  sortOrder: number;
  tvgShiftHours: number;
  inTuner: boolean;
  hidden: boolean;
  tunerNumber: null;
  variants: {
    id: string;
    managedChannelId: string;
    url: string;
    label: string;
    visibility: string;
    priority: number;
  }[];
  hasEpgMatch: boolean;
};

export function takeEditorDraft(storage: {
  getItem(key: string): string | null;
  removeItem(key: string): void;
}): SourceChannelDraft | null {
  const raw = storage.getItem(EDITOR_DRAFT_KEY);
  if (!raw) return null;
  storage.removeItem(EDITOR_DRAFT_KEY);
  try {
    const v = JSON.parse(raw) as Record<string, unknown>;
    const name = typeof v.name === "string" ? v.name : "";
    const url = typeof v.url === "string" ? v.url : "";
    if (!name && !url) return null;
    return {
      name,
      groupTitle: typeof v.groupTitle === "string" ? v.groupTitle : "",
      tvgId: typeof v.tvgId === "string" ? v.tvgId : null,
      tvgLogo: typeof v.tvgLogo === "string" ? v.tvgLogo : null,
      url,
    };
  } catch {
    return null;
  }
}

export function draftToManaged(
  entry: SourceChannelDraft,
  id: string,
  groupTitle?: string,
): DraftManaged {
  return {
    id,
    name: entry.name,
    groupTitle: (groupTitle ?? "").trim() || "Unassigned",
    tvgId: entry.tvgId ?? null,
    tvgLogo: entry.tvgLogo ?? null,
    notes: null,
    sortOrder: 0,
    tvgShiftHours: 0,
    inTuner: false,
    hidden: false,
    tunerNumber: null,
    variants: [
      {
        id: "draft-primary",
        managedChannelId: "",
        url: entry.url,
        label: "primary",
        visibility: "visible",
        priority: 0,
      },
    ],
    hasEpgMatch: false,
  };
}
