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

function parseSemver(v: string): [number, number, number] | null {
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

export async function fetchLatestRelease(repo: string, tagPrefix: string): Promise<ExtensionRelease | null> {
  // 50, not 30: three tag channels share one repo, so a quiet channel's latest
  // release could otherwise fall off the first page.
  const resp = await fetch(`https://api.github.com/repos/${REPO_OWNER}/${repo}/releases?per_page=50`, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!resp.ok) throw new Error(`GitHub releases fetch failed: HTTP ${resp.status}`);
  return pickLatestRelease(await resp.json(), tagPrefix);
}
