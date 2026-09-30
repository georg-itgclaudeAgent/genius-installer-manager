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
