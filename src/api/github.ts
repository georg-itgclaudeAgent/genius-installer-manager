const REPO_OWNER = "georg-itgclaudeAgent";
const VERSION_REGEX = /^(\d+)\.(\d+)\.(\d+)$/;

export interface ExtensionRelease {
  version: string;
  tag: string;
  notes: string;
  htmlUrl: string;
  publishedAt: string;
  zipUrl: string;       // direct download URL for the zip asset
  zipName: string;
  zipSize: number;
}

interface RawAsset {
  name: string;
  browser_download_url: string;
  size: number;
}

export interface RawRelease {
  tag_name: string;
  body: string | null;
  html_url: string;
  published_at: string;
  draft: boolean;
  prerelease: boolean;
  assets: RawAsset[];
}

export function parseSemver(v: string): [number, number, number] | null {
  const m = v.match(VERSION_REGEX);
  if (!m) return null;
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

export function compareSemver(a: string, b: string): number {
  const pa = parseSemver(a);
  const pb = parseSemver(b);
  if (!pa || !pb) return 0;
  for (let i = 0; i < 3; i++) {
    if (pa[i] > pb[i]) return 1;
    if (pa[i] < pb[i]) return -1;
  }
  return 0;
}

/** Highest stable semver release on one tag channel, or null if that channel has none. */
export function pickLatestRelease(releases: RawRelease[], tagPrefix: string): ExtensionRelease | null {
  const candidates = releases
    .filter((r) => !r.draft && !r.prerelease && r.tag_name.startsWith(tagPrefix))
    .map((r): ExtensionRelease | null => {
      const version = r.tag_name.slice(tagPrefix.length);
      if (!parseSemver(version)) return null;
      const zip = r.assets.find((a) => a.name.endsWith(".zip"));
      if (!zip) return null;
      return {
        version,
        tag: r.tag_name,
        notes: r.body || "",
        htmlUrl: r.html_url,
        publishedAt: r.published_at,
        zipUrl: zip.browser_download_url,
        zipName: zip.name,
        zipSize: zip.size,
      };
    })
    .filter((r): r is ExtensionRelease => r !== null)
    .sort((a, b) => compareSemver(b.version, a.version));
  return candidates[0] || null;
}

const CACHE_MS = 60_000;

/**
 * Every card in the Manager asks about the same repo, and unauthenticated
 * GitHub allows 60 calls an hour per IP (shared across an office). So one
 * in-flight or recent response per repo is shared by all cards; failures
 * are never cached, so "Check again" really does retry.
 */
export function createReleaseFetcher(fetchFn: typeof fetch, now: () => number) {
  const cache = new Map<string, { at: number; releases: Promise<RawRelease[]> }>();

  function releasesFor(repo: string): Promise<RawRelease[]> {
    const hit = cache.get(repo);
    if (hit && now() - hit.at < CACHE_MS) return hit.releases;
    // 50, not 30: three tag channels share one repo, so a quiet channel's latest
    // release could otherwise fall off the first page.
    const releases = fetchFn(`https://api.github.com/repos/${REPO_OWNER}/${repo}/releases?per_page=50`, {
      headers: { Accept: "application/vnd.github+json" },
    }).then(async (resp) => {
      if (!resp.ok) throw new Error(`GitHub releases fetch failed: HTTP ${resp.status}`);
      return (await resp.json()) as RawRelease[];
    });
    releases.catch(() => cache.delete(repo));
    cache.set(repo, { at: now(), releases });
    return releases;
  }

  return {
    async fetchLatestRelease(repo: string, tagPrefix: string): Promise<ExtensionRelease | null> {
      return pickLatestRelease(await releasesFor(repo), tagPrefix);
    },
  };
}

export const { fetchLatestRelease } = createReleaseFetcher((...a) => fetch(...a), () => Date.now());
