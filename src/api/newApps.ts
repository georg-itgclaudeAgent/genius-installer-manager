/** Every installer before Genius Installer Manager (the old PR Extension Manager) had this one. */
const ALWAYS_KNOWN = ["com.attract.pr-extension"];

/**
 * Which extensions to badge as NEW: ones not seen on a previous launch. Returns the ids to
 * badge now and the list to store for next time (so each app is new exactly once).
 */
export function newApps(currentIds: string[], seen: string[] | null): { newIds: string[]; seen: string[] } {
  const known = Array.isArray(seen) ? seen : ALWAYS_KNOWN;
  const newIds = currentIds.filter((id) => !known.includes(id));
  return { newIds, seen: Array.from(new Set([...known, ...currentIds])) };
}

const KEY = "gim.seenExtensions";

export function loadSeen(): string[] | null {
  try { const v = JSON.parse(localStorage.getItem(KEY) || "null"); return Array.isArray(v) ? v : null; }
  catch { return null; }
}

export function saveSeen(ids: string[]): void {
  try { localStorage.setItem(KEY, JSON.stringify(ids)); } catch { /* private mode: badges just repeat */ }
}
