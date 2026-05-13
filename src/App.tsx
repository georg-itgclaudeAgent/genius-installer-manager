import React from "react";
import { useExtensionStatus } from "./hooks/useExtensionStatus";
import { useManagerUpdate } from "./hooks/useManagerUpdate";
import { ExtensionCard } from "./components/ExtensionCard";
import { ManagerUpdateBanner } from "./components/ManagerUpdateBanner";

const MANAGER_VERSION = "0.1.0";

function relativeTime(d: Date | null): string {
  if (!d) return "never";
  const sec = Math.round((Date.now() - d.getTime()) / 1000);
  if (sec < 5) return "just now";
  if (sec < 60) return `${sec}s ago`;
  if (sec < 3600) return `${Math.round(sec / 60)}m ago`;
  return `${Math.round(sec / 3600)}h ago`;
}

export const App: React.FC = () => {
  const ext = useExtensionStatus();
  const mgr = useManagerUpdate();

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
        <ExtensionCard
          state={ext.state}
          busy={ext.busy}
          premiereWarning={ext.premiereWarning}
          onInstall={ext.install}
          onUpdate={ext.install}
          onUninstall={ext.uninstall}
        />
      </main>

      <footer className="app-footer">
        <span>Last checked: {relativeTime(ext.lastCheckedAt)}</span>
        <a
          onClick={(e) => { e.preventDefault(); ext.refresh(); }}
          href="#"
          className="footer-link"
        >
          Check again
        </a>
      </footer>
    </div>
  );
};
