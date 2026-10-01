import { invoke } from "@tauri-apps/api/core";

/** Mirrors `paths::ExtensionSpec` in Rust — Rust is the source of truth. */
export interface ExtensionSpec {
  id: string;
  name: string;
  subtitle: string;
  icon: string;
  repo: string;
  tag_prefix: string;
}

export function listExtensions(): Promise<ExtensionSpec[]> {
  return invoke<ExtensionSpec[]>("list_extensions");
}

export interface RegistryStatus {
  extensions: ExtensionSpec[];
  /** "github" = fresh; "cache" = last good copy; "built-in" = shipped with this installer. */
  source: "github" | "cache" | "built-in";
  error: string | null;
}

/** Fetch the app list from GitHub (registry.json), falling back to cache then built-in. */
export function refreshRegistry(): Promise<RegistryStatus> {
  return invoke<RegistryStatus>("refresh_registry");
}
