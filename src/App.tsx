import React, { useCallback, useEffect, useState } from "react";
import { useExtensionStatus } from "./hooks/useExtensionStatus";
import { useManagerUpdate } from "./hooks/useManagerUpdate";
import { ExtensionCard } from "./components/ExtensionCard";
import { ManagerUpdateBanner } from "./components/ManagerUpdateBanner";
import { refreshRegistry, ExtensionSpec, RegistryStatus } from "./api/registry";
import { loadSeen, newApps, saveSeen } from "./api/newApps";

const MANAGER_VERSION = "0.2.0";

function relativeTime(d: Date | null): string {
  if (!d) return "never";
  const sec = Math.round((Date.now() - d.getTime()) / 1000);
  if (sec < 5) return "just now";
  if (sec < 60) return `${sec}s ago`;
  if (sec < 3600) return `${Math.round(sec / 60)}m ago`;
  return `${Math.round(sec / 3600)}h ago`;
}

/** One hook per card — hooks can't be called in a loop, so each card owns its own. */
const ManagedExtension: React.FC<{
  spec: ExtensionSpec;
  refreshSignal: number;
  onChecked: (d: Date) => void;
  isNew: boolean;
}> = ({ spec, refreshSignal, onChecked, isNew }) => {
  const ext = useExtensionStatus(spec);
  useEffect(() => { if (refreshSignal > 0) ext.refresh(); }, [refreshSignal]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (ext.lastCheckedAt) onChecked(ext.lastCheckedAt); }, [ext.lastCheckedAt, onChecked]);
  return (
    <ExtensionCard
      spec={spec}
      state={ext.state}
      busy={ext.busy}
      premiereWarning={ext.premiereWarning}
      runtime={ext.runtime}
      progress={ext.progress}
      runtimeError={ext.runtimeError}
      onFinishSetup={ext.finishSetup}
      onInstall={ext.install}
      onUpdate={ext.install}
      onUninstall={ext.uninstall}
      onRetry={ext.refresh}
      isNew={isNew}
    />
  );
};

export const App: React.FC = () => {
  const mgr = useManagerUpdate();
  const [registry, setRegistry] = useState<RegistryStatus | null>(null);
  const [newIds, setNewIds] = useState<string[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [refreshSignal, setRefreshSignal] = useState(0);
  const [lastCheckedAt, setLastCheckedAt] = useState<Date | null>(null);
  const [checking, setChecking] = useState(false);

  const loadRegistry = useCallback(async () => {
    try {
      const r = await refreshRegistry();
      const fresh = newApps(r.extensions.map((e) => e.id), loadSeen());
      saveSeen(fresh.seen);
      setNewIds((prev) => Array.from(new Set([...prev, ...fresh.newIds])));
      setRegistry(r);
      setLoadError(null);
    } catch (e: any) {
      setLoadError(e?.message || String(e));
    }
  }, []);

  useEffect(() => { loadRegistry(); }, [loadRegistry]);

  const onChecked = useCallback((d: Date) => {
    setLastCheckedAt((prev) => (!prev || d > prev ? d : prev));
  }, []);

  /** One button: look for newly added apps on GitHub, then re-check every app for updates. */
  async function checkForUpdates() {
    setChecking(true);
    await loadRegistry();
    setRefreshSignal((n) => n + 1);
    setChecking(false);
  }

  return (
    <div className="app">
      {mgr.available && mgr.newVersion && (
        <ManagerUpdateBanner
          newVersion={mgr.newVersion}
          onApply={mgr.applyAndRestart}
          applying={mgr.applying}
        />
      )}

      <header className="app-header">
        <div className="app-logo">GI</div>
        <div className="app-title">Genius Installer Manager</div>
        <div className="app-version">v{MANAGER_VERSION}</div>
      </header>

      <main className="app-main">
        {loadError && <div className="card-warning">Couldn't load the app list: {loadError}</div>}
        {registry && registry.source !== "github" && (
          <div className="card-warning">
            Couldn't check GitHub for new apps{registry.error ? ` (${registry.error})` : ""}. Showing the
            {registry.source === "cache" ? " last list we fetched" : " apps built into this installer"}.
          </div>
        )}
        {registry?.extensions.map((spec) => (
          <ManagedExtension key={spec.id} spec={spec} refreshSignal={refreshSignal} onChecked={onChecked}
            isNew={newIds.includes(spec.id)} />
        ))}
      </main>

      <footer className="app-footer">
        <span>Last checked: {relativeTime(lastCheckedAt)}</span>
        <a
          onClick={(e) => { e.preventDefault(); if (!checking) checkForUpdates(); }}
          href="#"
          className="footer-link"
        >
          {checking ? "Checking…" : "Check for updates"}
        </a>
      </footer>
    </div>
  );
};
