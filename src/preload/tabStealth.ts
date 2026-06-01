/**
 * Tab stealth preload — runs in the MAIN world of every browser tab before any
 * page script (requires `contextIsolation: false` on the WebContentsView).
 *
 * Why this exists: Google sign-in and Cloudflare read the browser identity in
 * JavaScript (navigator.userAgentData / navigator.webdriver), not just the HTTP
 * headers. Electron reports a Chromium-only userAgentData (no "Google Chrome"
 * brand), which contradicts the UA string + Sec-CH-UA header and gets flagged as
 * a tampered/embedded browser. The session-level UA + client-hint headers
 * (TabManager) cannot reach these JS APIs — only main-world code can.
 *
 * SECURITY: this is the ONLY reason tabs run with contextIsolation:false. To keep
 * that safe, this preload:
 *   - runs with `sandbox: true` (no Node access — see TabManager webPreferences),
 *   - exposes NOTHING to the page (no ipcRenderer, no contextBridge, no globals),
 *   - is a self-contained IIFE that only redefines a few navigator getters.
 * See .claude/guides/SECURITY.md (§ "Exception contextIsolation").
 */
import { buildUaBrands, isGoogleSignInHost, FIREFOX_UA } from '@shared/userAgent'

type AnyFn = (...args: unknown[]) => unknown

interface UAData {
  brands?: { brand: string; version: string }[]
  mobile?: boolean
  platform?: string
  getHighEntropyValues?: (hints: string[]) => Promise<Record<string, unknown>>
}

;((): void => {
  // ── Stealth helpers ───────────────────────────────────────────────────────────

  const nativeToString = Function.prototype.toString

  // Make fn.toString() return "[native code]" so detectors that call
  // Function.prototype.toString.call(getter) see a native-looking string.
  // Symbol used to mark functions patched by makeNative, invisible to for..in.
  const NATIVE_MARK = Symbol('nativeMark')

  const makeNative = <T extends AnyFn>(fn: T, name?: string): T => {
    const n = name ?? fn.name ?? ''
    try {
      Object.defineProperty(fn, 'name', { value: n, configurable: true })
      // Mark so the patched Function.prototype.toString can recognise this fn.
      ;(fn as unknown as Record<symbol, boolean>)[NATIVE_MARK] = true
      Object.defineProperty(fn, 'toString', {
        value: (): string => `function ${n}() { [native code] }`,
        configurable: true,
      })
    } catch { /* best effort */ }
    return fn
  }

  // Patch Function.prototype.toString so both direct calls (fn.toString()) AND
  // explicit calls (Function.prototype.toString.call(fn)) return "[native code]"
  // for functions we patched. Turnstile uses both forms.
  try {
    const patchedToString = function toString(this: unknown): string {
      if (typeof this === 'function' && (this as unknown as Record<symbol, boolean>)[NATIVE_MARK]) {
        const n = (this as { name?: string }).name ?? ''
        return `function ${n}() { [native code] }`
      }
      return nativeToString.call(this)
    }
    makeNative(patchedToString, 'toString')
    Object.defineProperty(Function.prototype, 'toString', {
      value: patchedToString,
      configurable: true,
      writable: true,
    })
  } catch { /* best effort */ }

  const def = (obj: object, prop: string, get: AnyFn): void => {
    try {
      Object.defineProperty(obj, prop, {
        get: makeNative(get, prop),
        configurable: true,
        enumerable: true,
      })
    } catch { /* best effort */ }
  }

  // ── navigator.webdriver ───────────────────────────────────────────────────────
  def(navigator, 'webdriver', makeNative((): false => false, 'get webdriver'))

  // ── Google sign-in: Firefox identity ─────────────────────────────────────────
  // Google rejects Chrome-family embedded clients. Present as Firefox on sign-in
  // hosts so the JS identity matches the Firefox UA + no Sec-CH-UA from headers.
  const loc = (globalThis as Record<string, unknown>)['location'] as { hostname?: string } | undefined
  if (loc?.hostname && isGoogleSignInHost(loc.hostname)) {
    def(navigator, 'userAgent', makeNative((): string => FIREFOX_UA, 'get userAgent'))
    def(navigator, 'appVersion', makeNative((): string => '5.0 (Windows)', 'get appVersion'))
    def(navigator, 'vendor', makeNative((): string => '', 'get vendor'))
    def(navigator, 'userAgentData', makeNative((): undefined => undefined, 'get userAgentData'))

    // Firefox has no window.chrome — an empty {} object is a stronger tell than no object at all.
    ;(globalThis as Record<string, unknown>)['chrome'] = undefined

    // Firefox-specific navigator properties absent or different in Chrome.
    // Google checks these as consistency signals against the UA string.
    def(navigator, 'productSub', makeNative((): string => '20100101', 'get productSub'))  // Chrome: 20030107
    def(navigator, 'oscpu', makeNative((): string => 'Windows NT 10.0; Win64; x64', 'get oscpu'))  // Chrome: undefined
    def(navigator, 'buildID', makeNative((): string => '20100101', 'get buildID'))  // Chrome: undefined

    // Firefox modern (110+) returns empty PluginArray — NPAPI plugins are deprecated.
    const emptyPlugins = { length: 0, item: (): null => null, namedItem: (): null => null, refresh: (): void => {}, [Symbol.iterator]: makeNative(function* () {}, '[Symbol.iterator]') }
    const emptyMimes = { length: 0, item: (): null => null, namedItem: (): null => null, [Symbol.iterator]: makeNative(function* () {}, '[Symbol.iterator]') }
    def(navigator, 'plugins', makeNative((): unknown => emptyPlugins, 'get plugins'))
    def(navigator, 'mimeTypes', makeNative((): unknown => emptyMimes, 'get mimeTypes'))
    return
  }

  // ── window.chrome augmentation ────────────────────────────────────────────────
  // Electron exposes a bare empty chrome object. Turnstile checks loadTimes(),
  // csi(), and runtime structure — all must return real-looking data.
  const g = globalThis as Record<string, unknown>
  if (!g['chrome']) g['chrome'] = {}
  const chrome = g['chrome'] as Record<string, unknown>

  const noopEv = {
    addListener: makeNative((): void => {}, 'addListener'),
    removeListener: makeNative((): void => {}, 'removeListener'),
    hasListeners: makeNative((): false => false, 'hasListeners'),
  }

  chrome['app'] = {
    isInstalled: false,
    getDetails: makeNative((): null => null, 'getDetails'),
    getIsInstalled: makeNative((): false => false, 'getIsInstalled'),
    runningState: makeNative((): string => 'cannot_run', 'runningState'),
  }

  chrome['runtime'] = {
    id: undefined,
    lastError: undefined,
    connect: makeNative((): unknown => ({ disconnect: (): void => {}, postMessage: (): void => {}, onDisconnect: noopEv, onMessage: noopEv }), 'connect'),
    sendMessage: makeNative((): void => {}, 'sendMessage'),
    onMessage: noopEv,
    onConnect: noopEv,
    onStartup: noopEv,
    onInstalled: noopEv,
  }

  chrome['loadTimes'] = makeNative((): unknown => {
    const t = performance.now() / 1000
    return {
      requestTime: t - 0.5,
      startLoadTime: t - 0.45,
      commitLoadTime: t - 0.3,
      finishDocumentLoadTime: t - 0.1,
      finishLoadTime: t - 0.05,
      firstPaintTime: 0,
      firstPaintAfterLoadTime: 0,
      navigationType: 'Other',
      wasFetchedViaSpdy: false,
      wasNpnNegotiated: true,
      npnNegotiatedProtocol: 'h2',
      wasAlternateProtocolAvailable: false,
      connectionInfo: 'h2',
    }
  }, 'loadTimes')

  chrome['csi'] = makeNative((): unknown => ({
    startE: Date.now() - 500,
    onloadT: Date.now() - 100,
    pageT: performance.now(),
    tran: 15,
  }), 'csi')

  // ── navigator.userAgentData — add "Google Chrome" brand ───────────────────────
  // Chromium-only brands contradict the UA + Sec-CH-UA headers → detection tell.
  const origUaData = (navigator as unknown as { userAgentData?: UAData }).userAgentData
  if (origUaData && Array.isArray(origUaData.brands)) {
    const chromium = origUaData.brands.find((b) => /chromium/i.test(b.brand))
    const brands = buildUaBrands(chromium?.version ?? '130')
    const platform = origUaData.platform ?? 'Windows'
    const patched: UAData & { toJSON?: () => unknown } = {
      brands,
      mobile: origUaData.mobile ?? false,
      platform,
      getHighEntropyValues: (hints: string[]) =>
        (origUaData.getHighEntropyValues?.(hints) ?? Promise.resolve({} as Record<string, unknown>))
          .then((v) => ({
            ...(v as Record<string, unknown>),
            brands,
            fullVersionList: brands.map((b) => ({
              brand: b.brand,
              version: b.brand === 'Not?A_Brand' ? '99.0.0.0' : ((v as Record<string, unknown>)['uaFullVersion'] as string | undefined) ?? '',
            })),
          }))
          .catch((): Record<string, unknown> => ({ brands, mobile: false, platform })),
    }
    patched.toJSON = (): unknown => ({ brands, mobile: patched.mobile, platform })
    def(navigator, 'userAgentData', (): UAData => patched)
  }
})()
