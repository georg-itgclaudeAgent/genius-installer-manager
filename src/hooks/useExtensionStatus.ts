import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { fetchLatestRelease } from "../api/github";
import type { ExtensionSpec } from "../api/registry";
import { deriveState, ExtensionState, StatusInfo, RuntimeStatus, RuntimeProgress } from "./useExtensionStatus.logic";

export type { ExtensionState } from "./useExtensionStatus.logic";

export interface UseExtensionStatusResult {
  state: ExtensionState;
  busy: boolean;
  lastCheckedAt: Date | null;
  premiereWarning: boolean;
  installPath: string | null;
  runtime: RuntimeStatus | null;
  progress: RuntimeProgress | null;
  runtimeError: string | null;
  finishSetup: () => Promise<void>;
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
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [progress, setProgress] = useState<RuntimeProgress | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const hasRuntime = !!spec.runtime;

  useEffect(() => {
    if (!hasRuntime) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<{ id: string; downloaded: number; total: number | null }>("runtime-progress", (e) => {
      if (e.payload.id === spec.id) setProgress({ downloaded: e.payload.downloaded, total: e.payload.total });
    }).then((u) => { if (cancelled) u(); else unlisten = u; }).catch(() => {});
    return () => { cancelled = true; unlisten?.(); };
  }, [hasRuntime, spec.id]);

  const refresh = useCallback(async () => {
    setState({ kind: "checking" });
    try {
      const status = await invoke<StatusInfo>("get_status", { id: spec.id });
      setPremiereWarning(status.premiere_running_warning);
      setInstallPath(status.install_path);
      if (hasRuntime) {
        try { setRuntime(await invoke<RuntimeStatus>("runtime_status", { id: spec.id })); }
        catch { setRuntime(null); }
      }
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
  }, [spec.id, spec.repo, spec.tag_prefix, hasRuntime]);

  const setupRuntime = useCallback(async () => {
    setRuntimeError(null);
    setProgress({ downloaded: 0, total: null });
    try {
      await invoke<string>("ensure_runtime", { id: spec.id });
    } catch (e: any) {
      setRuntimeError(e?.message || String(e));
    } finally {
      setProgress(null);
    }
  }, [spec.id]);

  const finishSetup = useCallback(async () => {
    setBusy(true);
    try { await setupRuntime(); await refresh(); } finally { setBusy(false); }
  }, [setupRuntime, refresh]);

  const install = useCallback(async () => {
    if (state.kind !== "not-installed" && state.kind !== "update-available") return;
    setBusy(true);
    try {
      await invoke<string>("install_from_url", { id: spec.id, url: state.latest.zipUrl });
      if (hasRuntime) await setupRuntime();
      await refresh();
    } catch (e: any) {
      setState({ kind: "error", reason: e?.message || String(e) });
    } finally {
      setBusy(false);
    }
  }, [state, spec.id, refresh, hasRuntime, setupRuntime]);

  const uninstall = useCallback(async () => {
    setBusy(true);
    try {
      await invoke("uninstall_extension", { id: spec.id });
      setRuntimeError(null);
      await refresh();
    } catch (e: any) {
      setState({ kind: "error", reason: e?.message || String(e) });
    } finally {
      setBusy(false);
    }
  }, [spec.id, refresh]);

  useEffect(() => { refresh(); }, [refresh]);

  return { state, busy, lastCheckedAt, premiereWarning, installPath, runtime, progress, runtimeError, finishSetup, refresh, install, uninstall };
}
