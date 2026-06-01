/**
 * Browser-identity helpers: make Overframe's tab traffic indistinguishable from
 * stock desktop Chrome on the wire.
 *
 * Electron otherwise leaks two inconsistent signals that Google sign-in and
 * Cloudflare flag instantly:
 *   - "Electron/x" inside the User-Agent string, and
 *   - "Electron";v="33" inside the Sec-CH-UA client-hint header.
 * A UA that claims Chrome while the client hints claim Electron is a textbook
 * "tampered / automated browser" tell.
 *
 * Everything here is pure and derived from the Chromium version string, so a
 * single source of truth (process.versions.chrome) keeps the UA string and the
 * client hints in lockstep across Electron upgrades. Unit-tested in
 * userAgent.test.ts — the only part that needs a real site (Google login,
 * Cloudflare) is validated manually per WORKFLOW.md §4.
 */

/** Extract the major version ("130") from a full Chromium version ("130.0.6723.59"). */
export function chromeMajor(chromeVersion: string): string {
  const major = chromeVersion.split('.')[0]
  return major && /^\d+$/.test(major) ? major : '0'
}

/** A stock desktop-Chrome (Windows) User-Agent string for the given Chromium version. */
export function buildUserAgent(chromeVersion: string): string {
  return `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${chromeVersion} Safari/537.36`
}

/**
 * A current desktop Firefox/Windows UA. Google's "secure browser" gate flags
 * Chrome-family embedded clients (Electron) on its account sign-in flow; a Firefox
 * identity (no navigator.userAgentData, different UA) is treated differently and
 * commonly gets through. Applied ONLY to Google sign-in hosts (see isGoogleSignInHost).
 */
export const FIREFOX_UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:131.0) Gecko/20100101 Firefox/131.0'

/** Hosts behind Google's account sign-in / "this browser may not be secure" gate. */
export function isGoogleSignInHost(host: string): boolean {
  return /(^|\.)accounts\.google\.com$/i.test(host) || /(^|\.)accounts\.youtube\.com$/i.test(host)
}

export interface UaBrand {
  brand: string
  version: string
}

/**
 * The UA-brand list stock Chrome advertises. Single source of truth shared by
 * the Sec-CH-UA HTTP header AND the navigator.userAgentData JS object so the two
 * never disagree (a mismatch is itself a fingerprinting tell). The "Not?A_Brand"
 * entry is Chrome's intentional GREASE brand.
 */
export function buildUaBrands(chromeVersion: string): UaBrand[] {
  const major = chromeMajor(chromeVersion)
  return [
    { brand: 'Chromium', version: major },
    { brand: 'Google Chrome', version: major },
    { brand: 'Not?A_Brand', version: '99' },
  ]
}

export interface ClientHints {
  'Sec-CH-UA': string
  'Sec-CH-UA-Mobile': string
  'Sec-CH-UA-Platform': string
}

/**
 * Low-entropy UA client hints matching stock desktop Chrome on Windows. These
 * are the three hints Chrome sends by default; keeping them consistent with the
 * UA string is what defeats the "UA says Chrome, hints say Electron" mismatch.
 */
export function buildClientHints(chromeVersion: string): ClientHints {
  const list = buildUaBrands(chromeVersion)
    .map((b) => `"${b.brand}";v="${b.version}"`)
    .join(', ')
  return {
    'Sec-CH-UA': list,
    'Sec-CH-UA-Mobile': '?0',
    'Sec-CH-UA-Platform': '"Windows"',
  }
}

/**
 * Merge client hints into outgoing request headers, replacing any existing
 * variant case-insensitively (Electron may already have set "Sec-CH-UA" with an
 * Electron brand under a different casing). Returns a new object — does not
 * mutate the input.
 */
export function mergeClientHintHeaders(
  existing: Record<string, string>,
  hints: ClientHints,
): Record<string, string> {
  const hintKeys = new Set(Object.keys(hints).map((k) => k.toLowerCase()))
  const merged: Record<string, string> = {}
  for (const [key, value] of Object.entries(existing)) {
    if (!hintKeys.has(key.toLowerCase())) merged[key] = value
  }
  return { ...merged, ...hints }
}
