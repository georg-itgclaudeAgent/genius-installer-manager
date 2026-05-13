const REPO_OWNER = "georg-itgclaudeAgent";
const REPO_NAME = "pr-extension";
const API_BASE = `https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}`;
const EXTENSION_TAG_PREFIX = "extension-v";
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

interface RawRelease {
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

export async function fetchLatestExtensionRelease(): Promise<ExtensionRelease | null> {
  const resp = await fetch(`${API_BASE}/releases?per_page=30`, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!resp.ok) throw new Error(`GitHub releases fetch failed: HTTP ${resp.status}`);
  const releases: RawRelease[] = await resp.json();
  const candidates = releases
    .filter((r) => !r.draft && !r.prerelease && r.tag_name.startsWith(EXTENSION_TAG_PREFIX))
    .map((r) => {
      const version = r.tag_name.slice(EXTENSION_TAG_PREFIX.length);
      const zipAsset = r.assets.find((a) => a.name.endsWith(".zip"));
      if (!zipAsset) return null;
      return {
        version,
        tag: r.tag_name,
        notes: r.body || "",
        htmlUrl: r.html_url,
        publishedAt: r.published_at,
        zipUrl: zipAsset.browser_download_url,
        zipName: zipAsset.name,
        zipSize: zipAsset.size,
      } as ExtensionRelease;
    })
    .filter((r): r is ExtensionRelease => r !== null && parseSemver(r.version) !== null)
    .sort((a, b) => compareSemver(b.version, a.version));
  return candidates[0] || null;
}
