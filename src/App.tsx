import React, { useCallback, useEffect, useState } from "react";
import { useExtensionStatus } from "./hooks/useExtensionStatus";
import { useManagerUpdate } from "./hooks/useManagerUpdate";
import { ExtensionCard } from "./components/ExtensionCard";
import { ManagerUpdateBanner } from "./components/ManagerUpdateBanner";
import { listExtensions, ExtensionSpec } from "./api/registry";

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
}> = ({ spec, refreshSignal, onChecked }) => {
  const ext = useExtensionStatus(spec);
  useEffect(() => { if (refreshSignal > 0) ext.refresh(); }, [refreshSignal]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (ext.lastCheckedAt) onChecked(ext.lastCheckedAt); }, [ext.lastCheckedAt, onChecked]);
  return (
    <ExtensionCard
      spec={spec}
      state={ext.state}
      busy={ext.busy}
      premiereWarning={ext.premiereWarning}
      onInstall={ext.install}
      onUpdate={ext.install}
      onUninstall={ext.uninstall}
      onRetry={ext.refresh}
    />
  );
};

export const App: React.FC = () => {
  const mgr = useManagerUpdate();
  const [specs, setSpecs] = useState<ExtensionSpec[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [refreshSignal, setRefreshSignal] = useState(0);
  const [lastCheckedAt, setLastCheckedAt] = useState<Date | null>(null);

  useEffect(() => {
    listExtensions().then(setSpecs).catch((e) => setLoadError(e?.message || String(e)));
  }, []);

  const onChecked = useCallback((d: Date) => {
    setLastCheckedAt((prev) => (!prev || d > prev ? d : prev));
  }, []);

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
        <div className="app-logo">PR</div>
        <div className="app-title">PR Extension Manager</div>
        <div className="app-version">v{MANAGER_VERSION}</div>
      </header>

      <main className="app-main">
        {loadError && <div className="card-warning">Couldn't load the extension list: {loadError}</div>}
        {specs?.map((spec) => (
          <ManagedExtension key={spec.id} spec={spec} refreshSignal={refreshSignal} onChecked={onChecked} />
        ))}
      </main>

      <footer className="app-footer">
        <span>Last checked: {relativeTime(lastCheckedAt)}</span>
        <a
          onClick={(e) => { e.preventDefault(); setRefreshSignal((n) => n + 1); }}
          href="#"
          className="footer-link"
        >
          Check again
        </a>
      </footer>
    </div>
  );
};
