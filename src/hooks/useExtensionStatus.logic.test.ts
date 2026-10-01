import { describe, it, expect } from "vitest";
import { deriveState, runtimeLine, runtimeNeedsSetup, runtimeNeededFor, StatusInfo, RuntimeStatus } from "./useExtensionStatus.logic";
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

describe("deriveState when GitHub can't be reached", () => {
  it("installed → still up-to-date, keeps installed version, flags the failed check", () => {
    expect(deriveState(installed("1.2.0"), null, "HTTP 403")).toEqual({
      kind: "up-to-date", installedVersion: "1.2.0", latest: null, updateCheckFailed: "HTTP 403",
    });
  });
  it("not installed → error (nothing to install without a release)", () => {
    expect(deriveState(notInstalled, null, "HTTP 403")).toEqual({ kind: "error", reason: "HTTP 403" });
  });
});

describe("runtimeLine", () => {
  it("says what one-time setup will download, in plain words", () => {
    expect(runtimeLine({ needed: true, installed: false, version: null, flavour: "cuda", latest: "1.0.0" }, null))
      .toBe("Needs a one-time setup (about 0.8 GB, GPU)");
    expect(runtimeLine({ needed: true, installed: false, version: null, flavour: "cpu", latest: "1.0.0" }, null))
      .toBe("Needs a one-time setup (about 0.1 GB)");
  });
  it("shows progress while downloading", () => {
    expect(runtimeLine(null, { downloaded: 300 * 2 ** 20, total: 800 * 2 ** 20 })).toBe("Setting up: 300 of 800 MB");
  });
  it("is quiet when the runtime is current", () => {
    expect(runtimeLine({ needed: true, installed: true, version: "1.0.0", flavour: "cuda", latest: "1.0.0" }, null)).toBeNull();
  });
  it("offers a runtime update when a newer one exists", () => {
    expect(runtimeLine({ needed: true, installed: true, version: "1.0.0", flavour: "cuda", latest: "1.1.0" }, null))
      .toBe("Runtime update available (1.0.0 → 1.1.0)");
  });
});

const rt = (o: Partial<RuntimeStatus>): RuntimeStatus => ({ needed: true, installed: false, version: null, flavour: "cuda", latest: "1.0.0", ...o });

describe("runtime only where the extension build uses it", () => {
  it("shows nothing for builds older than from_version (the 0.1.0 preview)", () => {
    expect(runtimeLine(rt({ needed: false }), null)).toBeNull();
    expect(runtimeLine(rt({ needed: false, installed: true, version: "1.0.0", latest: "1.1.0" }), null)).toBeNull();
    expect(runtimeNeedsSetup(rt({ needed: false }), null)).toBe(false);
  });
  it("shows no setup line or button before any runtime is released", () => {
    expect(runtimeLine(rt({ latest: null }), null)).toBeNull();
    expect(runtimeNeedsSetup(rt({ latest: null }), null)).toBe(false);
  });
  it("offers Finish setup only when needed, released, missing and idle", () => {
    expect(runtimeNeedsSetup(rt({}), null)).toBe(true);
    expect(runtimeNeedsSetup(rt({ installed: true, version: "1.0.0" }), null)).toBe(false);
    expect(runtimeNeedsSetup(rt({}), { downloaded: 0, total: null })).toBe(false);
    expect(runtimeNeedsSetup(null, null)).toBe(false);
  });
  it("decides from the version being installed whether to set the runtime up", () => {
    const spec = { tag_prefix: "runtime-v", from_version: "0.2.0" };
    expect(runtimeNeededFor(spec, "0.1.0")).toBe(false);
    expect(runtimeNeededFor(spec, "0.2.0")).toBe(true);
    expect(runtimeNeededFor(spec, "0.10.0")).toBe(true);
    expect(runtimeNeededFor(undefined, "1.0.0")).toBe(false);
  });
});

describe("runtimeLine while unpacking", () => {
  it("says Unpacking once the download has finished", () => {
    expect(runtimeLine(null, { downloaded: 800 * 2 ** 20, total: 800 * 2 ** 20, phase: "unpacking" })).toBe("Unpacking…");
  });
});
