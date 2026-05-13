import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  fetchLatestExtensionRelease,
  compareSemver,
  ExtensionRelease,
} from "../api/github";

interface StatusInfo {
  installed: boolean;
  installed_version: string | null;
  install_path: string;
  premiere_running_warning: boolean;
}

export type ExtensionState =
  | { kind: "checking" }
  | { kind: "not-installed"; latest: ExtensionRelease | null }
  | { kind: "up-to-date"; installedVersion: string; latest: ExtensionRelease }
  | {
      kind: "update-available";
      installedVersion: string;
      latest: ExtensionRelease;
    }
  | { kind: "error"; reason: string };

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

export function useExtensionStatus(): UseExtensionStatusResult {
  const [state, setState] = useState<ExtensionState>({ kind: "checking" });
  const [busy, setBusy] = useState(false);
  const [lastCheckedAt, setLastCheckedAt] = useState<Date | null>(null);
  const [premiereWarning, setPremiereWarning] = useState(false);
  const [installPath, setInstallPath] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setState({ kind: "checking" });
    try {
      const status = await invoke<StatusInfo>("get_status");
      setPremiereWarning(status.premiere_running_warning);
      setInstallPath(status.install_path);

      let latest: ExtensionRelease | null = null;
      try { latest = await fetchLatestExtensionRelease(); }
      catch (e: any) { throw new Error(e?.message || String(e)); }

      setLastCheckedAt(new Date());

      if (!status.installed) {
        setState({ kind: "not-installed", latest });
        return;
      }
      if (!latest) {
        // Installed but couldn't fetch latest -> treat as up-to-date (no upgrade available)
        setState({
          kind: "up-to-date",
          installedVersion: status.installed_version || "(unknown)",
          latest: {
            version: status.installed_version || "0.0.0",
            tag: "",
            notes: "",
            htmlUrl: "",
            publishedAt: "",
            zipUrl: "",
            zipName: "",
            zipSize: 0,
          },
        });
        return;
      }
      const installed = status.installed_version || "0.0.0";
      if (compareSemver(latest.version, installed) > 0) {
        setState({ kind: "update-available", installedVersion: installed, latest });
      } else {
        setState({ kind: "up-to-date", installedVersion: installed, latest });
      }
    } catch (e: any) {
      setState({ kind: "error", reason: e?.message || String(e) });
    }
  }, []);

  const install = useCallback(async () => {
    if (state.kind !== "not-installed" && state.kind !== "update-available") return;
    const latest = state.latest;
    if (!latest) return;
    setBusy(true);
    try {
      await invoke<string>("install_from_url", { url: latest.zipUrl });
      await refresh();
    } catch (e: any) {
      setState({ kind: "error", reason: e?.message || String(e) });
    } finally {
      setBusy(false);
    }
  }, [state, refresh]);

  const uninstall = useCallback(async () => {
    setBusy(true);
    try {
      await invoke("uninstall_extension");
      await refresh();
    } catch (e: any) {
      setState({ kind: "error", reason: e?.message || String(e) });
    } finally {
      setBusy(false);
    }
  }, [refresh]);

  useEffect(() => { refresh(); }, [refresh]);

  return { state, busy, lastCheckedAt, premiereWarning, installPath, refresh, install, uninstall };
}
