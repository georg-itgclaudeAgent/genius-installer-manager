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
  | { kind: "up-to-date"; installedVersion: string; latest: ExtensionRelease | null }
  | { kind: "update-available"; installedVersion: string; latest: ExtensionRelease }
  | { kind: "error"; reason: string };

export function deriveState(status: StatusInfo, latest: ExtensionRelease | null): ExtensionState {
  if (!status.installed) {
    return latest ? { kind: "not-installed", latest } : { kind: "not-released" };
  }
  const installedVersion = status.installed_version || "0.0.0";
  if (latest && compareSemver(latest.version, installedVersion) > 0) {
    return { kind: "update-available", installedVersion, latest };
  }
  return { kind: "up-to-date", installedVersion, latest };
}
