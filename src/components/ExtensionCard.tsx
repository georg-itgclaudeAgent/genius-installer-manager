import React from "react";
import { ExtensionState } from "../hooks/useExtensionStatus";
import type { ExtensionSpec } from "../api/registry";
import { runtimeLine, runtimeNeedsSetup, RuntimeStatus, RuntimeProgress } from "../hooks/useExtensionStatus.logic";

interface ExtensionCardProps {
  spec: ExtensionSpec;
  state: ExtensionState;
  busy: boolean;
  premiereWarning: boolean;
  runtime?: RuntimeStatus | null;
  progress?: RuntimeProgress | null;
  runtimeError?: string | null;
  onFinishSetup?: () => void;
  onInstall: () => void;
  onUpdate: () => void;
  onUninstall: () => void;
  onRetry: () => void;
  /** Newly added to the app list since the last launch. */
  isNew?: boolean;
}

function summarizeNotes(notes: string): string[] {
  if (!notes) return [];
  return notes
    .split(/\r?\n/)
    .map((l) => l.trim().replace(/^[-*]\s+/, ""))
    .filter((l) => l.length > 0 && !l.startsWith("SHA256:") && !l.startsWith("##"))
    .slice(0, 3);
}

export const ExtensionCard: React.FC<ExtensionCardProps> = ({
  spec,
  state,
  busy,
  premiereWarning,
  runtime = null,
  progress = null,
  runtimeError = null,
  onFinishSetup,
  onInstall,
  onUpdate,
  onUninstall,
  onRetry,
  isNew,
}) => {
  const isUpdateAvailable = state.kind === "update-available";
  const installed = state.kind === "up-to-date" || state.kind === "update-available";
  const line = spec.runtime && installed ? runtimeLine(runtime, progress) : progress ? runtimeLine(null, progress) : null;
  const needsSetup = !!spec.runtime && installed && runtimeNeedsSetup(runtime, progress);
  const cardBorderClass = isUpdateAvailable ? "card highlight" : "card";

  return (
    <div className={cardBorderClass}>
      {isUpdateAvailable && <span className="card-new-pill">UPDATE</span>}
      {isNew && !isUpdateAvailable && <span className="card-new-pill">NEW APP</span>}
      <div className="card-row">
        <div className="card-icon">{spec.icon}</div>
        <div className="card-meta">
          <div className="card-title">{spec.name}</div>
          <div className="card-subtitle">{spec.subtitle}</div>
          <StatusLine state={state} />
          {line && <div className="card-subtitle">{line}</div>}
          {runtimeError && <div className="card-status"><span className="dot red" /> {runtimeError}</div>}
        </div>
      </div>

      {isUpdateAvailable && (
        <div className="card-notes">
          <strong>What's new in {state.latest.version}</strong>
          <ul>
            {summarizeNotes(state.latest.notes).map((b, i) => <li key={i}>{b}</li>)}
          </ul>
        </div>
      )}

      {premiereWarning && (
        <div className="card-warning">
          Premiere may be running with {spec.name} open. Close Premiere before installing or updating.
        </div>
      )}

      <div className="card-actions">
        <CardButtons
          state={state}
          busy={busy}
          onInstall={onInstall}
          onUpdate={onUpdate}
          onUninstall={onUninstall}
          onRetry={onRetry}
        />
        {needsSetup && onFinishSetup && (
          <button className="btn-primary" onClick={onFinishSetup} disabled={busy}>
            {busy ? "Setting up…" : "Finish setup"}
          </button>
        )}
      </div>
    </div>
  );
};

const StatusLine: React.FC<{ state: ExtensionState }> = ({ state }) => {
  if (state.kind === "checking") {
    return <div className="card-status"><span className="dot grey" /> Checking…</div>;
  }
  if (state.kind === "error") {
    return <div className="card-status"><span className="dot red" /> Error: {state.reason}</div>;
  }
  if (state.kind === "not-released") {
    return <div className="card-status"><span className="dot grey" /> Not released yet · check back soon</div>;
  }
  if (state.kind === "not-installed") {
    return (
      <div className="card-status">
        <span className="dot grey" /> Not installed · Latest: <strong>{state.latest.version}</strong>
      </div>
    );
  }
  if (state.kind === "up-to-date") {
    return (
      <div className="card-status">
        <span className="dot green" />
        <span style={{ color: "#aac9aa" }}>Up to date</span>
        {" · Installed: "}<strong>{state.installedVersion}</strong>
        {state.updateCheckFailed && (
          <div className="card-subtitle" title={state.updateCheckFailed}>
            Couldn't check for updates right now. Try again in a few minutes.
          </div>
        )}
      </div>
    );
  }
  // update-available
  return (
    <div className="card-status">
      <span className="dot amber" />
      <span style={{ color: "#e6c060" }}>Update available</span>
      {" · "}{state.installedVersion} → <strong>{state.latest.version}</strong>
    </div>
  );
};

const CardButtons: React.FC<{
  state: ExtensionState;
  busy: boolean;
  onInstall: () => void;
  onUpdate: () => void;
  onUninstall: () => void;
  onRetry: () => void;
}> = ({ state, busy, onInstall, onUpdate, onUninstall, onRetry }) => {
  if (state.kind === "checking") {
    return <button className="btn-primary" disabled>Checking…</button>;
  }
  if (state.kind === "error") {
    return <button className="btn-primary" onClick={onRetry} disabled={busy}>Retry</button>;
  }
  if (state.kind === "not-released") {
    return <button className="btn-primary" disabled>Not released yet</button>;
  }
  if (state.kind === "not-installed") {
    return (
      <button className="btn-primary" onClick={onInstall} disabled={busy}>
        {busy ? "Installing…" : "Install"}
      </button>
    );
  }
  if (state.kind === "up-to-date") {
    return (
      <>
        <button className="btn-primary" disabled>Up to date</button>
        <button className="btn-secondary" onClick={onUninstall} disabled={busy}>
          {busy ? "Working…" : "Uninstall"}
        </button>
      </>
    );
  }
  // update-available
  return (
    <>
      <button className="btn-primary" onClick={onUpdate} disabled={busy}>
        {busy ? "Updating…" : `Update to ${state.latest.version}`}
      </button>
      <button className="btn-secondary" onClick={onUninstall} disabled={busy}>Uninstall</button>
    </>
  );
};
