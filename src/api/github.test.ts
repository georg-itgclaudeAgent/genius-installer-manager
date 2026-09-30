import { describe, it, expect } from "vitest";
import { pickLatestRelease, createReleaseFetcher, RawRelease } from "./github";

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

describe("createReleaseFetcher", () => {
  function fakeFetch(body: unknown, status = 200) {
    let calls = 0;
    const fn = (async () => {
      calls++;
      return { ok: status === 200, status, json: async () => body } as Response;
    }) as unknown as typeof fetch;
    return { fn, calls: () => calls };
  }

  it("shares one GitHub call across extensions in the same repo", async () => {
    const f = fakeFetch([rel("extension-v1.2.0"), rel("geniuscut-v0.1.0")]);
    const { fetchLatestRelease } = createReleaseFetcher(f.fn, () => 1000);
    const [pr, gc] = await Promise.all([
      fetchLatestRelease("pr-extension", "extension-v"),
      fetchLatestRelease("pr-extension", "geniuscut-v"),
    ]);
    expect(pr?.version).toBe("1.2.0");
    expect(gc?.version).toBe("0.1.0");
    expect(f.calls()).toBe(1);
  });

  it("refetches after the cache window", async () => {
    let t = 0;
    const f = fakeFetch([rel("extension-v1.2.0")]);
    const { fetchLatestRelease } = createReleaseFetcher(f.fn, () => t);
    await fetchLatestRelease("pr-extension", "extension-v");
    t = 61_000;
    await fetchLatestRelease("pr-extension", "extension-v");
    expect(f.calls()).toBe(2);
  });

  it("does not cache a failure", async () => {
    const f = fakeFetch({}, 403);
    const { fetchLatestRelease } = createReleaseFetcher(f.fn, () => 1000);
    await expect(fetchLatestRelease("pr-extension", "extension-v")).rejects.toThrow(/403/);
    await expect(fetchLatestRelease("pr-extension", "extension-v")).rejects.toThrow(/403/);
    expect(f.calls()).toBe(2);
  });
});
