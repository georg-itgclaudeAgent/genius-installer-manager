import { describe, it, expect } from "vitest";
import { pickLatestRelease, RawRelease } from "./github";

function rel(tag: string, opts: Partial<RawRelease> = {}): RawRelease {
  return {
    tag_name: tag,
    body: `notes for ${tag}`,
    html_url: `https://github.com/x/${tag}`,
    published_at: "2026-09-30T00:00:00Z",
    draft: false,
    prerelease: false,
    assets: [{ name: `${tag}.zip`, browser_download_url: `https://github.com/georg-itgclaudeAgent/pr-extension/releases/download/${tag}/${tag}.zip`, size: 10 }],
    ...opts,
  };
}

describe("pickLatestRelease", () => {
  const releases = [
    rel("extension-v1.2.0"),
    rel("geniuscut-v0.3.0"),
    rel("extension-v1.10.0"),
    rel("manager-v0.2.0"),
    rel("geniuscut-v0.1.0"),
  ];

  it("picks the highest semver for the prefix, numerically not lexically", () => {
    expect(pickLatestRelease(releases, "extension-v")?.version).toBe("1.10.0");
  });

  it("never offers another channel's release", () => {
    expect(pickLatestRelease(releases, "geniuscut-v")?.version).toBe("0.3.0");
    expect(pickLatestRelease([rel("geniuscut-v9.0.0")], "extension-v")).toBeNull();
  });

  it("returns null when the channel has no releases yet", () => {
    expect(pickLatestRelease([rel("extension-v1.2.0")], "geniuscut-v")).toBeNull();
  });

  it("skips drafts, prereleases, zip-less and non-semver releases", () => {
    const r = pickLatestRelease([
      rel("geniuscut-v0.9.0", { draft: true }),
      rel("geniuscut-v0.8.0", { prerelease: true }),
      rel("geniuscut-v0.7.0", { assets: [] }),
      rel("geniuscut-vbeta"),
      rel("geniuscut-v0.2.0"),
    ], "geniuscut-v");
    expect(r?.version).toBe("0.2.0");
  });
});
