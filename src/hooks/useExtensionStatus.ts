import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { fetchLatestRelease } from "../api/github";
import type { ExtensionSpec } from "../api/registry";
import { deriveState, ExtensionState, StatusInfo } from "./useExtensionStatus.logic";

export type { ExtensionState } from "./useExtensionStatus.logic";

export interface UseExtensionStatusResult {
  state: ExtensionState;
  busy: boolean;
  lastCheckedAt: Date | null;
  premiereWarning: boolean;
  installPath: string | null;
  refresh: () => Promise<void>;
  install: () => Promise<void>;
  uninstall: () => Promise<void>;
}

export function useExtensionStatus(spec: ExtensionSpec): UseExtensionStatusResult {
  const [state, setState] = useState<ExtensionState>({ kind: "checking" });
  const [busy, setBusy] = useState(false);
  const [lastCheckedAt, setLastCheckedAt] = useState<Date | null>(null);
  const [premiereWarning, setPremiereWarning] = useState(false);
  const [installPath, setInstallPath] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setState({ kind: "checking" });
    try {
      const status = await invoke<StatusInfo>("get_status", { id: spec.id });
      setPremiereWarning(status.premiere_running_warning);
      setInstallPath(status.install_path);
      let latest = null;
      let updateCheckError: string | undefined;
      try {
        latest = await fetchLatestRelease(spec.repo, spec.tag_prefix);
        setLastCheckedAt(new Date());
      } catch (e: any) {
        updateCheckError = e?.message || String(e);
      }
      setState(deriveState(status, latest, updateCheckError));
    } catch (e: any) {
      setState({ kind: "error", reason: e?.message || String(e) });
    }
  }, [spec.id, spec.repo, spec.tag_prefix]);

  const install = useCallback(async () => {
    if (state.kind !== "not-installed" && state.kind !== "update-available") return;
    setBusy(true);
    try {
      await invoke<string>("install_from_url", { id: spec.id, url: state.latest.zipUrl });
      await refresh();
    } catch (e: any) {
      setState({ kind: "error", reason: e?.message || String(e) });
    } finally {
      setBusy(false);
    }
  }, [state, spec.id, refresh]);

  const uninstall = useCallback(async () => {
    setBusy(true);
    try {
      await invoke("uninstall_extension", { id: spec.id });
      await refresh();
    } catch (e: any) {
      setState({ kind: "error", reason: e?.message || String(e) });
    } finally {
      setBusy(false);
    }
  }, [spec.id, refresh]);

  useEffect(() => { refresh(); }, [refresh]);

  return { state, busy, lastCheckedAt, premiereWarning, installPath, refresh, install, uninstall };
}
