import { useState, useEffect, useCallback } from "react";
import { check, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UseManagerUpdateResult {
  available: boolean;
  newVersion: string | null;
  applyAndRestart: () => Promise<void>;
  applying: boolean;
  error: string | null;
}

export function useManagerUpdate(): UseManagerUpdateResult {
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const u = await check();
        if (!cancelled && u) setPendingUpdate(u);
      } catch (e: any) {
        if (!cancelled) setError(e?.message || String(e));
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const applyAndRestart = useCallback(async () => {
    if (!pendingUpdate) return;
    setApplying(true);
    setError(null);
    try {
      await pendingUpdate.downloadAndInstall();
      await relaunch();
    } catch (e: any) {
      setError(e?.message || String(e));
      setApplying(false);
    }
  }, [pendingUpdate]);

  return {
    available: pendingUpdate !== null,
    newVersion: pendingUpdate?.version || null,
    applyAndRestart,
    applying,
    error,
  };
}
