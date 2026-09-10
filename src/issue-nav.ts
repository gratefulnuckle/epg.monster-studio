/** Stay on the current group after an issue is fixed, instead of jumping to the first one. */
export function keepGroup(visibleGroups: string[], current: string): string {
  return visibleGroups.includes(current) ? current : (visibleGroups[0] ?? "");
}

/**
 * After the applied row drops out of the issues list, land on the issue that
 * slid into its place (or the new last row). Does not rewind to the top.
 */
export function nextIssueId(visibleIds: string[], removedId: string): string | null {
  const idx = visibleIds.indexOf(removedId);
  const remaining = visibleIds.filter((id) => id !== removedId);
  if (remaining.length === 0) return null;
  if (idx < 0) return remaining[0];
  return remaining[Math.min(idx, remaining.length - 1)];
}

export type LogoIssueRow = {
  managedChannelId: string;
  channelName: string;
  groupTitle: string;
  tvgId?: string | null;
  currentLogo?: string | null;
  issue: string;
  reason: string;
};

/** Local patch after a successful logo_set — does not re-scan other channels. */
export function patchLogoAfterSet<T extends LogoIssueRow>(row: T, url: string | null): T {
  const trimmed = url?.trim() ?? "";
  if (!trimmed) {
    return {
      ...row,
      currentLogo: null,
      issue: "missing",
      reason: "No logo URL.",
    };
  }
  return {
    ...row,
    currentLogo: trimmed,
    issue: "",
    reason: "",
  };
}
