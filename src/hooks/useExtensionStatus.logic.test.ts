import { describe, it, expect } from "vitest";
import { deriveState, StatusInfo } from "./useExtensionStatus.logic";
import type { ExtensionRelease } from "../api/github";

const notInstalled: StatusInfo = { installed: false, installed_version: null, install_path: "x", premiere_running_warning: false };
const installed = (v: string): StatusInfo => ({ ...notInstalled, installed: true, installed_version: v });
const release = (v: string): ExtensionRelease => ({
  version: v, tag: `geniuscut-v${v}`, notes: "", htmlUrl: "", publishedAt: "", zipUrl: "https://github.com/georg-itgclaudeAgent/x.zip", zipName: "x.zip", zipSize: 1,
});

describe("deriveState", () => {
  it("not installed and nothing published yet → not-released (not an error)", () => {
    expect(deriveState(notInstalled, null)).toEqual({ kind: "not-released" });
  });
  it("not installed with a release → not-installed", () => {
    expect(deriveState(notInstalled, release("0.1.0")).kind).toBe("not-installed");
  });
  it("installed, newer release → update-available", () => {
    const s = deriveState(installed("0.1.0"), release("0.2.0"));
    expect(s).toMatchObject({ kind: "update-available", installedVersion: "0.1.0" });
  });
  it("installed, same release → up-to-date", () => {
    expect(deriveState(installed("0.2.0"), release("0.2.0")).kind).toBe("up-to-date");
  });
  it("installed but no release reachable → up-to-date with installed version", () => {
    expect(deriveState(installed("1.2.0"), null)).toMatchObject({ kind: "up-to-date", installedVersion: "1.2.0" });
  });
});
