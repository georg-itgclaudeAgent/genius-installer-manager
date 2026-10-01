import { compareSemver, ExtensionRelease } from "../api/github";

export interface StatusInfo {
  installed: boolean;
  installed_version: string | null;
  install_path: string;
  premiere_running_warning: boolean;
}

export type ExtensionState =
  | { kind: "checking" }
  | { kind: "not-released" }
  | { kind: "not-installed"; latest: ExtensionRelease }
  | { kind: "up-to-date"; installedVersion: string; latest: ExtensionRelease | null; updateCheckFailed?: string }
  | { kind: "update-available"; installedVersion: string; latest: ExtensionRelease }
  | { kind: "error"; reason: string };

export function deriveState(
  status: StatusInfo,
  latest: ExtensionRelease | null,
  updateCheckError?: string,
): ExtensionState {
  if (updateCheckError) {
    // An installed extension still works offline — keep its version and
    // Uninstall button visible rather than turning the whole card into an error.
    if (!status.installed) return { kind: "error", reason: updateCheckError };
    return {
      kind: "up-to-date",
      installedVersion: status.installed_version || "0.0.0",
      latest: null,
      updateCheckFailed: updateCheckError,
    };
  }
  if (!status.installed) {
    return latest ? { kind: "not-installed", latest } : { kind: "not-released" };
  }
  const installedVersion = status.installed_version || "0.0.0";
  if (latest && compareSemver(latest.version, installedVersion) > 0) {
    return { kind: "update-available", installedVersion, latest };
  }
  return { kind: "up-to-date", installedVersion, latest };
}

export interface RuntimeStatus { installed: boolean; version: string | null; flavour: "cuda" | "cpu"; latest: string | null }
export interface RuntimeProgress { downloaded: number; total: number | null }

export function runtimeLine(rt: RuntimeStatus | null, progress: RuntimeProgress | null): string | null {
  const mb = (n: number) => Math.round(n / 2 ** 20);
  if (progress) return progress.total ? `Setting up: ${mb(progress.downloaded)} of ${mb(progress.total)} MB` : `Setting up: ${mb(progress.downloaded)} MB`;
  if (!rt) return null;
  if (!rt.installed) return `Needs a one-time setup (about ${rt.flavour === "cuda" ? "0.8 GB, GPU" : "0.1 GB"})`;
  if (rt.latest && rt.version && compareSemver(rt.latest, rt.version) > 0) return `Runtime update available (${rt.version} → ${rt.latest})`;
  return null;
}
