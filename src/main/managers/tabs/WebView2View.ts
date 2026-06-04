/**
 * WebView2View — wraps the native webview2_addon to give TabManager an API
 * that mirrors what WebContentsView provided, using the real Edge renderer.
 *
 * Because WebView2 uses the system-installed Edge binary (not Electron's
 * Chromium), it is indistinguishable from real Edge to Cloudflare and any
 * other fingerprinting system — no tabStealth patches needed.
 *
 * The addon creates one ICoreWebView2Environment (one Edge process) shared
 * across all tabs; each tab gets its own ICoreWebView2Controller (a child
 * HWND of the overlay BrowserWindow).
 */
import path from 'node:path'
import { EventEmitter } from 'node:events'
import { app } from 'electron'
import type { BrowserWindow } from 'electron'

// ── Native addon loading ──────────────────────────────────────────────────────
// Resolve the .node file relative to the compiled main output dir.
// In dev: out/main/index.js → addon at native/webview2-addon/build/Release/
// In packaged: resources/app.asar/… — addon must be unpacked (asar unpack).

function loadAddon(): NativeAddon {
  const candidates = [
    // Dev / build output
    path.join(__dirname, '../../native/webview2-addon/build/Release/webview2_addon.node'),
    // Packaged — addon should be in resources/
    path.join(process.resourcesPath ?? '', 'webview2_addon.node'),
  ]
  for (const p of candidates) {
    try {
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      return require(p) as NativeAddon
    } catch { /* try next */ }
  }
  throw new Error('webview2_addon.node not found — run `pnpm build:addon`')
}

interface NativeAddon {
  createTab(hwndBuf: Buffer, x: number, y: number, w: number, h: number): number
  navigate(tabId: number, url: string): void
  goBack(tabId: number): void
  goForward(tabId: number): void
  reload(tabId: number): void
  stop(tabId: number): void
  setBounds(tabId: number, x: number, y: number, w: number, h: number): void
  show(tabId: number): void
  hide(tabId: number): void
  destroyTab(tabId: number): void
  executeScript(tabId: number, js: string): Promise<string>
  getURL(tabId: number): string
  canGoBack(tabId: number): boolean
  canGoForward(tabId: number): boolean
  setUserAgent(tabId: number, ua: string): void
  setZoom(tabId: number, factor: number): void
  setMuted(tabId: number, muted: boolean): void
  /** Set prefers-color-scheme for the shared Edge profile. 0=auto, 1=light, 2=dark. */
  setColorScheme(scheme: number): void
  /** Restore OS keyboard focus to the first non-WebView2 child HWND (Chromium render widget). */
  claimFocus(overlayHwnd: Buffer): void
  setEventCallback(cb: (tabId: number, type: string, dataJson: string) => void): void
}

let _addon: NativeAddon | null = null
let _callbackRegistered = false

function getAddon(): NativeAddon {
  if (!_addon) {
    _addon = loadAddon()
    // Register the single global event callback that routes to each WebView2View
    if (!_callbackRegistered) {
      _addon.setEventCallback((tabId, type, dataJson) => {
        const view = WebView2View._registry.get(tabId)
        if (!view) return
        try {
          const data = JSON.parse(dataJson) as Record<string, unknown>
          view.emit('wv2event', type, data)
        } catch { /* ignore malformed JSON */ }
      })
      _callbackRegistered = true
    }
  }
  return _addon
}

// ── WebView2View ──────────────────────────────────────────────────────────────

export interface WV2EventMap {
  'navigation_starting': { url: string; isRedirect: boolean }
  'navigation_completed': { url: string; success: boolean; canGoBack: boolean; canGoForward: boolean }
  'history_changed': { canGoBack: boolean; canGoForward: boolean }
  'title_changed': { title: string }
  'new_window': { url: string }
  'zoom_changed': { factor: number }
  'audio_changed': { playing: boolean }
  'muted_changed': { muted: boolean }
  'download': { id: string; filename: string; url: string; receivedBytes: number; totalBytes: number; state: string }
  'got_focus': Record<string, never>
}

export class WebView2View extends EventEmitter {
  /** Global registry: native tabId → WebView2View instance */
  static readonly _registry = new Map<number, WebView2View>()

  private _nativeId: number | null = null
  private _destroyed = false
  private _url = ''
  private _title = ''
  private _isLoading = false
  private _canGoBack = false
  private _canGoForward = false

  constructor(private readonly _overlay: BrowserWindow) {
    super()
  }

  // ── Lifecycle ────────────────────────────────────────────────────────────────

  /**
   * Must be called before any other method.
   * Creates the WebView2 controller embedded in the overlay window.
   */
  init(x: number, y: number, w: number, h: number): void {
    if (this._nativeId !== null) return
    const hwnd = this._overlay.getNativeWindowHandle()
    const addon = getAddon()
    this._nativeId = addon.createTab(hwnd, x, y, w, h)
    WebView2View._registry.set(this._nativeId, this)

    // Route raw addon events to typed EventEmitter events
    this.on('wv2event', (type: string, data: Record<string, unknown>) => {
      switch (type) {
        case 'navigation_starting':
          this._isLoading = true
          this._url = (data['url'] as string | undefined) ?? this._url
          this.emit('did-start-loading')
          this.emit('did-start-navigation', data['url'])
          break
        case 'navigation_completed':
          this._isLoading = false
          if (data['url']) this._url = data['url'] as string
          this.emit('did-finish-load')
          this.emit('did-navigate', data['url'])
          break
        case 'history_changed':
          // Fires after NavigationCompleted with the CORRECT canGoBack/canGoForward state.
          // (get_CanGoBack inside NavigationCompleted is stale — history is updated after.)
          this._canGoBack    = data['canGoBack']    as boolean
          this._canGoForward = data['canGoForward'] as boolean
          this.emit('history-updated')
          break
        case 'title_changed':
          this._title = (data['title'] as string | undefined) ?? ''
          this.emit('page-title-updated', this._title)
          break
        case 'favicon_changed':
          this.emit('page-favicon-updated', [data['faviconUrl']])
          break
        case 'new_window':
          this.emit('new-window', data['url'])
          break
        case 'zoom_changed':
          this.emit('zoom-updated', data['factor'])
          break
        case 'audio_changed':
          this.emit('audio-state', data['playing'])
          break
        case 'muted_changed':
          this.emit('muted-state', data['muted'])
          break
        case 'download':
          this.emit('download', data)
          break
        case 'got_focus':
          this.emit('focus')
          break
      }
    })
  }

  /** Set prefers-color-scheme for all WebView2 tabs (shared profile). 0=auto, 1=light, 2=dark. */
  static setColorScheme(scheme: number): void {
    try { getAddon().setColorScheme(scheme) } catch { /* addon not loaded yet */ }
  }

  /**
   * Restore OS keyboard focus to the Electron/Chromium render widget.
   * Calls ::SetFocus() on the first non-WebView2 child HWND of the overlay window.
   * Must be called from the main process with the overlay window HWND.
   */
  static claimFocus(overlayHwnd: Buffer): void {
    try { getAddon().claimFocus(overlayHwnd) } catch { /* addon not loaded yet */ }
  }

  destroy(): void {
    if (this._destroyed || this._nativeId === null) return
    this._destroyed = true
    WebView2View._registry.delete(this._nativeId)
    try { getAddon().destroyTab(this._nativeId) } catch { /* best effort */ }
    this._nativeId = null
    this.removeAllListeners()
  }

  // ── Navigation ────────────────────────────────────────────────────────────────

  loadURL(url: string): void {
    if (this._nativeId === null) return
    this._url = url
    getAddon().navigate(this._nativeId, url)
  }

  goBack(): void    { if (this._nativeId !== null) getAddon().goBack(this._nativeId) }
  goForward(): void { if (this._nativeId !== null) getAddon().goForward(this._nativeId) }
  reload(): void    { if (this._nativeId !== null) getAddon().reload(this._nativeId) }
  stop(): void      { if (this._nativeId !== null) getAddon().stop(this._nativeId) }

  // ── Layout ────────────────────────────────────────────────────────────────────

  setBounds(x: number, y: number, w: number, h: number): void {
    if (this._nativeId !== null) getAddon().setBounds(this._nativeId, x, y, w, h)
  }

  setVisible(visible: boolean): void {
    if (this._nativeId === null) return
    if (visible) getAddon().show(this._nativeId)
    else         getAddon().hide(this._nativeId)
  }

  // ── Zoom / audio ──────────────────────────────────────────────────────────────

  setZoom(factor: number): void {
    if (this._nativeId !== null) getAddon().setZoom(this._nativeId, factor)
  }

  setMuted(muted: boolean): void {
    if (this._nativeId !== null) getAddon().setMuted(this._nativeId, muted)
  }

  // ── Script execution ──────────────────────────────────────────────────────────

  executeJavaScript(js: string): Promise<string> {
    if (this._nativeId === null) return Promise.resolve('null')
    return getAddon().executeScript(this._nativeId, js)
  }

  // ── State accessors ───────────────────────────────────────────────────────────

  getURL(): string      { return this._nativeId !== null ? getAddon().getURL(this._nativeId) : this._url }
  getTitle(): string    { return this._title }
  isLoading(): boolean  { return this._isLoading }
  canGoBack(): boolean {
    if (this._nativeId !== null) this._canGoBack = getAddon().canGoBack(this._nativeId)
    return this._canGoBack
  }

  canGoForward(): boolean {
    if (this._nativeId !== null) this._canGoForward = getAddon().canGoForward(this._nativeId)
    return this._canGoForward
  }
  isDestroyed(): boolean  { return this._destroyed }
}

// ── Dev: log addon load path ───────────────────────────────────────────────────
if (!app.isPackaged) {
  try { getAddon(); } catch (e) { console.warn('[WebView2View] addon not loaded:', (e as Error).message) }
}
