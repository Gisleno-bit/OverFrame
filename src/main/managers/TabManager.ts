import { app } from 'electron'
import { randomUUID } from 'node:crypto'
import type { TabState, MemorySnapshot, DownloadEvent } from '@shared/types'
import { DEFAULT_HOMEPAGE } from '@shared/types'
import { IPC } from '@shared/ipc'
import type { OverlayWindow } from '../windows/OverlayWindow'
import { WebView2View } from './tabs/WebView2View'
import {
  POPUP_DEDUP_EVICT_THRESHOLD,
  POPUP_DEDUP_MAX_AGE_MS,
  POPUP_DEDUP_WINDOW_MS,
  type ManagedTab,
  type TabEvent,
} from './tabs/types'

export type { TabEvent } from './tabs/types'

function isProtectedUrl(url: string, protectedDomains: string[]): boolean {
  if (!protectedDomains.length) return false
  try {
    const { hostname } = new URL(url)
    return protectedDomains.some((d) => hostname === d || hostname.endsWith('.' + d))
  } catch {
    return false
  }
}

/** Google's favicon service — returns a 32×32 PNG for any public hostname. */
function faviconUrl(url: string): string | null {
  try {
    const { hostname, protocol } = new URL(url)
    if (!hostname || protocol === 'about:') return null
    return `https://www.google.com/s2/favicons?domain=${encodeURIComponent(hostname)}&sz=32`
  } catch {
    return null
  }
}

export class TabManager {
  private tabs = new Map<string, ManagedTab>()
  private displayOrder: string[] = []
  private activeTabId: string | null = null
  private listeners = new Set<(e: TabEvent) => void>()
  private recentPopups = new Map<string, number>()
  private stoppedDuringHide = new Set<string>()
  private unloadedUrls = new Map<string, string>()
  private _dark = true

  private viewBoundsCache: { x: number; y: number; w: number; h: number } | null = null
  private _fullscreenTabId: string | null = null
  private _unsubBeforeHide: (() => void) | null = null

  constructor(private overlay: OverlayWindow) {
    this.overlay.onLayoutChange = () => this.relayoutActive()
    this._unsubBeforeHide = this.overlay.onBeforeHide(() => {
      if (this._fullscreenTabId) this._exitFullscreen()
    })
  }

  /** Called from renderer with exact CSS-computed bounds of the WebView2 host div. */
  setActiveViewBounds(x: number, y: number, w: number, h: number): void {
    this.viewBoundsCache = { x, y, w, h }
    if (this._fullscreenTabId) return // fullscreen manages its own bounds
    const tab = this.activeTabId ? this.tabs.get(this.activeTabId) : null
    if (!tab || tab.view.isDestroyed()) return
    tab.view.setBounds(x, y, w, h)
  }

  // ── Listeners ────────────────────────────────────────────────────────────────

  on(cb: (e: TabEvent) => void): () => void {
    this.listeners.add(cb)
    return () => { this.listeners.delete(cb) }
  }

  private emit(e: TabEvent): void {
    for (const cb of this.listeners) cb(e)
  }

  // ── Accessors ─────────────────────────────────────────────────────────────────

  getAll(): TabState[] {
    return this.getOrderedIds().map((id) => this.tabs.get(id)!.state)
  }

  getOrderedIds(): string[] {
    if (this.displayOrder.length > 0) {
      return this.displayOrder.filter((id) => this.tabs.has(id))
    }
    return Array.from(this.tabs.keys())
  }

  getActiveId(): string | null { return this.activeTabId }

  reorder(ids: string[]): void {
    this.displayOrder = ids.filter((id) => this.tabs.has(id))
  }

  getMemorySnapshot(): MemorySnapshot {
    // WebView2 tabs run in Edge's own process — not visible via app.getAppMetrics().
    // Still report Electron-process memory (main + overlay renderer) so the smoke
    // test and memory widget show a meaningful non-zero value.
    const metrics = app.getAppMetrics()
    const appKb = metrics.reduce(
      (sum, m) => sum + (m.memory.privateBytes ?? m.memory.workingSetSize ?? 0), 0
    )
    const tabs = Array.from(this.tabs.keys()).map((id) => ({ tabId: id, privateKb: 0 }))
    return { tabs, appKb }
  }

  // ── Create ───────────────────────────────────────────────────────────────────

  create(url: string): TabState {
    const id = randomUUID()

    const bounds = this.overlay.getTabContentBounds()
    const view = new WebView2View(this.overlay.win)
    view.init(bounds.x, bounds.y, bounds.width, bounds.height)
    view.setVisible(false) // hidden until setActive()

    const isHomepage = url === DEFAULT_HOMEPAGE || url === DEFAULT_HOMEPAGE + '/'
    const initialState: TabState = {
      id,
      url,
      title: isHomepage ? 'New tab' : '',
      favicon: null,
      isLoading: true,
      canGoBack: false,
      canGoForward: false,
      zoomFactor: 1,
      isAudioPlaying: false,
      isMuted: false,
    }
    const tab: ManagedTab = { id, view, state: initialState }
    this.tabs.set(id, tab)
    this.displayOrder.push(id)

    this.wireWebView2(tab)
    view.loadURL(url)
    this.setActive(id)

    return initialState
  }

  // ── Event wiring ──────────────────────────────────────────────────────────────

  private wireWebView2(tab: ManagedTab): void {
    const view = tab.view

    const update = (patch: Partial<TabState>): void => {
      if (!this.tabs.has(tab.id)) return
      tab.state = { ...tab.state, ...patch }
      this.emit({ type: 'updated', tab: tab.state })
    }

    // Poll for SPA URL changes (pushState/replaceState don't fire WebView2 navigation events)
    let spaPoller: ReturnType<typeof setInterval> | null = null
    const clearSpaPoller = (): void => {
      if (spaPoller !== null) { clearInterval(spaPoller); spaPoller = null }
    }

    view.on('did-start-loading', () => {
      clearSpaPoller()
      update({ isLoading: true })
    })

    view.on('did-finish-load', () => {
      update({ isLoading: false, canGoBack: view.canGoBack(), canGoForward: view.canGoForward() })
      const url = view.getURL()
      if (!url.startsWith('http://') && !url.startsWith('https://')) return
      // Google GSI uses this page to postMessage the OAuth token back to window.opener.
      // Once it has finished loading the JS has already run — close the popup tab.
      // Defer via setImmediate: closing a WebView2 tab synchronously from inside
      // its own did-finish-load callback can leave stale HWND messages in flight.
      if (url.includes('accounts.google.com/gsi/transform')) {
        const tabId = tab.id
        setImmediate(() => { if (this.tabs.has(tabId)) this.close(tabId) })
        return
      }
      const d = this._dark
      // Activate framework-based dark/light themes (Docusaurus, Tailwind, etc.)
      // data-theme covers Docusaurus/VitePress; .dark class covers Tailwind/Next.js.
      // Sites without these patterns are unaffected.
      void view.executeJavaScript(
        `(function(d){` +
        `document.documentElement.setAttribute('data-theme',d?'dark':'light');` +
        `document.documentElement.classList[d?'add':'remove']('dark')` +
        `})(${d})`
      )

      // On Discord invite/onboarding pages, auto-click "Continue in browser" so the
      // "Open App" dialog doesn't block the user. A MutationObserver handles the case
      // where the dialog is injected after initial paint.
      if (/^https:\/\/discord\.com\/(invite\/|app\/invite)/.test(url)) {
        void view.executeJavaScript(
          `(function(){` +
          `function go(){` +
          `for(var e of document.querySelectorAll('button,[role=button],a')){` +
          `var t=(e.textContent||'').toLowerCase();` +
          `if(t.includes('navigateur')||t.includes('browser')){e.click();return true}` +
          `}return false}` +
          `if(!go()){var o=new MutationObserver(function(){if(go())o.disconnect()});` +
          `o.observe(document.documentElement,{childList:true,subtree:true});` +
          `setTimeout(function(){o.disconnect()},15000)}` +
          `})()`
        )
      }

      // Start SPA poller after each full page load.
      // Also checks document.fullscreenElement to drive the overlay fullscreen mode,
      // since ContainsFullScreenElementChanged can be unreliable across Edge versions.
      clearSpaPoller()
      let pollPending = false
      spaPoller = setInterval(() => {
        if (view.isDestroyed()) { clearSpaPoller(); return }
        if (pollPending) return
        pollPending = true
        void view.executeJavaScript('[window.location.href, !!document.fullscreenElement]').then((raw) => {
          pollPending = false
          const [current, isFullscreen] = JSON.parse(raw) as [string, boolean]
          if (current && current !== tab.state.url) {
            update({ url: current, favicon: faviconUrl(current) })
          }
          if (isFullscreen && this._fullscreenTabId !== tab.id) {
            this._enterFullscreen(tab.id)
          } else if (!isFullscreen && this._fullscreenTabId === tab.id) {
            this._exitFullscreen()
          }
        }).catch(() => { pollPending = false })
      }, 300)
    })

    view.on('focus', () => {
      this.overlay.win.webContents.send(IPC.EventWebviewFocused)
    })

    view.on('did-navigate', (url: unknown) => {
      if (typeof url !== 'string') return
      update({ url, favicon: faviconUrl(url) })
    })

    view.on('page-title-updated', (title: unknown) => {
      if (typeof title !== 'string') return
      const currentUrl = tab.state.url
      const isHomepage = currentUrl === DEFAULT_HOMEPAGE || currentUrl === DEFAULT_HOMEPAGE + '/'
      update({ title: isHomepage ? 'New tab' : title })
    })

    view.on('page-favicon-updated', (favicons: unknown) => {
      const list = favicons as string[]
      update({ favicon: list?.[0] ?? null })
    })

    view.on('history-updated', () => {
      update({ canGoBack: view.canGoBack(), canGoForward: view.canGoForward() })
    })

    view.on('new-window', (url: unknown, reqId: unknown) => {
      if (typeof url === 'string') {
        this.handlePopup(url, typeof reqId === 'number' ? reqId : undefined)
      }
    })

    view.on('zoom-updated', (factor: unknown) => {
      if (typeof factor === 'number' && Number.isFinite(factor)) update({ zoomFactor: factor })
    })

    view.on('audio-state', (playing: unknown) => {
      update({ isAudioPlaying: playing === true })
    })

    view.on('muted-state', (muted: unknown) => {
      update({ isMuted: muted === true })
    })

    view.on('download', (event: unknown) => {
      this.emit({ type: 'download', event: event as DownloadEvent })
    })

    view.on('fullscreen-changed', (active: unknown) => {
      if (active === true) {
        this._enterFullscreen(tab.id)
      } else if (this._fullscreenTabId === tab.id) {
        this._exitFullscreen()
      }
    })
  }


  // ── Close ────────────────────────────────────────────────────────────────────

  close(id: string): void {
    const tab = this.tabs.get(id)
    if (!tab) return

    if (this._fullscreenTabId === id) this._exitFullscreen()
    tab.view.setVisible(false)
    tab.view.destroy()

    let successor: string | null = null
    if (this.activeTabId === id) {
      const order = this.displayOrder
      const idx = order.indexOf(id)
      successor = order[idx + 1] ?? order[idx - 1] ?? null
    }

    this.tabs.delete(id)
    this.displayOrder = this.displayOrder.filter((x) => x !== id)
    this.emit({ type: 'removed', id })

    if (this.activeTabId === id) {
      if (successor && this.tabs.has(successor)) {
        this.setActive(successor)
      } else {
        this.activeTabId = null
        this.emit({ type: 'activeChanged', id: null })
      }
    }
  }

  closeAll(): void {
    for (const tab of this.tabs.values()) {
      tab.view.setVisible(false)
      tab.view.destroy()
      this.emit({ type: 'removed', id: tab.id })
    }
    this.tabs.clear()
    this.displayOrder = []
    this.activeTabId = null
    this.stoppedDuringHide.clear()
    this.unloadedUrls.clear()
    this.emit({ type: 'activeChanged', id: null })
  }

  // ── Active tab ────────────────────────────────────────────────────────────────

  setActive(id: string): void {
    const tab = this.tabs.get(id)
    if (!tab) return

    // Hide every other tab
    for (const other of this.tabs.values()) {
      if (other.id !== id) other.view.setVisible(false)
    }

    // Position + show the active tab — prefer the renderer-provided bounds (accurate)
    // and fall back to the formula for the very first activation before any DOM update.
    if (this.viewBoundsCache) {
      const { x, y, w, h } = this.viewBoundsCache
      tab.view.setBounds(x, y, w, h)
    } else {
      const { x, y, width, height } = this.overlay.getTabContentBounds()
      tab.view.setBounds(x, y, width, height)
    }
    tab.view.setVisible(true)

    this.activeTabId = id
    this.emit({ type: 'activeChanged', id })
  }

  deactivate(): void {
    if (this.activeTabId === null) return
    const tab = this.tabs.get(this.activeTabId)
    if (tab) tab.view.setVisible(false)
    this.activeTabId = null
    this.emit({ type: 'activeChanged', id: null })
  }

  relayoutActive(): void {
    if (this._fullscreenTabId) return // fullscreen manages its own bounds
    const tab = this.activeTabId ? this.tabs.get(this.activeTabId) : null
    if (!tab || tab.view.isDestroyed()) return
    const { x, y, width, height } = this.overlay.getTabContentBounds()
    tab.view.setBounds(x, y, width, height)
  }

  /**
   * Width (px) of the active tab's vertical scrollbar — 0 when there is none
   * (or when the page uses overlay scrollbars). Used to position the IG promo
   * with an equal visual gap to the visible content edge on the right and bottom.
   */
  async measureActiveScrollbarWidth(): Promise<number> {
    const tab = this.activeTabId ? this.tabs.get(this.activeTabId) : null
    if (!tab || tab.view.isDestroyed()) return 0
    try {
      const raw = await tab.view.executeJavaScript('(window.innerWidth - document.documentElement.clientWidth)')
      const n = Number(JSON.parse(raw))
      return Number.isFinite(n) && n >= 0 && n <= 40 ? Math.round(n) : 0
    } catch {
      return 0
    }
  }

  // ── Navigation ────────────────────────────────────────────────────────────────

  navigate(id: string, url: string): void {
    this.tabs.get(id)?.view.loadURL(url)
  }

  goBack(id: string): void {
    const tab = this.tabs.get(id)
    if (tab?.state.canGoBack) tab.view.goBack()
  }

  goForward(id: string): void {
    const tab = this.tabs.get(id)
    if (tab?.state.canGoForward) tab.view.goForward()
  }

  reload(id: string): void {
    this.tabs.get(id)?.view.reload()
  }

  stop(id: string): void {
    this.tabs.get(id)?.view.stop()
  }

  setDarkMode(dark: boolean): void {
    this._dark = dark
  }

  // ── Suspend / unload / resume (performance mode) ──────────────────────────────

  suspendAll(): void {
    this.stoppedDuringHide.clear()
    for (const tab of this.tabs.values()) {
      try {
        if (tab.view.isLoading()) {
          tab.view.stop()
          this.stoppedDuringHide.add(tab.id)
        }
      } catch { /* destroyed */ }
    }
  }

  unloadAll(protectedDomains: string[] = []): void {
    this.stoppedDuringHide.clear()
    this.unloadedUrls.clear()
    for (const tab of this.tabs.values()) {
      try {
        const url = tab.state.url
        if (url && url !== 'about:blank' && !isProtectedUrl(url, protectedDomains)) {
          this.unloadedUrls.set(tab.id, url)
          tab.view.loadURL('about:blank')
        }
      } catch { /* destroyed */ }
    }
  }

  closeUnprotected(protectedDomains: string[]): void {
    const toClose = [...this.displayOrder].filter((id) => {
      const tab = this.tabs.get(id)
      return tab && !isProtectedUrl(tab.state.url, protectedDomains)
    })
    for (const id of toClose) this.close(id)
  }

  resumeAll(): void {
    for (const tab of this.tabs.values()) {
      try {
        const unloadedUrl = this.unloadedUrls.get(tab.id)
        if (unloadedUrl) {
          tab.state = { ...tab.state, url: unloadedUrl }
          this.emit({ type: 'updated', tab: tab.state })
          tab.view.loadURL(unloadedUrl)
          this.unloadedUrls.delete(tab.id)
        } else if (this.stoppedDuringHide.has(tab.id)) {
          tab.view.reload()
        }
      } catch { /* destroyed */ }
    }
    this.stoppedDuringHide.clear()
  }

  // ── Zoom / audio ──────────────────────────────────────────────────────────────

  setZoom(id: string, factor: number): void {
    const tab = this.tabs.get(id)
    if (!tab) return
    const clamped = Math.max(0.25, Math.min(5, factor))
    // The native ZoomFactorChanged event confirms the value (zoom-updated → state),
    // but we update optimistically so the UI reflects the change immediately.
    tab.view.setZoom(clamped)
    tab.state = { ...tab.state, zoomFactor: clamped }
    this.emit({ type: 'updated', tab: tab.state })
  }

  setMuted(id: string, muted: boolean): void {
    const tab = this.tabs.get(id)
    if (!tab) return
    tab.view.setMuted(muted)
    tab.state = { ...tab.state, isMuted: muted }
    this.emit({ type: 'updated', tab: tab.state })
  }

  // ── Popup deduplication ───────────────────────────────────────────────────────

  private handlePopup(url: string, reqId?: number): void {
    const blockPopup = (): void => {
      // Complete the deferral without a NewWindow → window.open() returns null.
      if (reqId !== undefined) WebView2View.completeNewWindow(reqId, -1)
    }

    // Security: only open http(s) popups as tabs — drop javascript:/data:/custom schemes.
    try {
      const proto = new URL(url).protocol
      if (proto !== 'http:' && proto !== 'https:') { blockPopup(); return }
    } catch {
      blockPopup(); return
    }

    const now = Date.now()
    const last = this.recentPopups.get(url) ?? 0
    if (now - last <= POPUP_DEDUP_WINDOW_MS) { blockPopup(); return }

    this.recentPopups.set(url, now)
    if (this.recentPopups.size > POPUP_DEDUP_EVICT_THRESHOLD) {
      const cutoff = now - POPUP_DEDUP_MAX_AGE_MS
      for (const [u, t] of this.recentPopups) {
        if (t < cutoff) this.recentPopups.delete(u)
      }
    }

    const newTabState = this.create(url)

    // Provide the new controller as NewWindow so the popup page has window.opener.
    // This is required for OAuth flows (e.g. Google Sign-In) that postMessage
    // the token back to window.opener after authentication completes.
    if (reqId !== undefined) {
      const nativeId = this.tabs.get(newTabState.id)?.view.nativeId ?? -1
      WebView2View.completeNewWindow(reqId, nativeId)
    }
  }

  // ── Fullscreen ────────────────────────────────────────────────────────────────

  private _enterFullscreen(tabId: string): void {
    if (this._fullscreenTabId) return
    const tab = this.tabs.get(tabId)
    if (!tab || tab.view.isDestroyed()) return
    this._fullscreenTabId = tabId
    const { width, height } = this.overlay.win.getContentBounds()
    tab.view.setBounds(0, 0, width, height)
  }

  private _exitFullscreen(): void {
    if (!this._fullscreenTabId) return
    this._fullscreenTabId = null
    this.relayoutActive()
  }

  // ── Dev eval ──────────────────────────────────────────────────────────────────

  async devEval(js: string): Promise<unknown> {
    const tab = this.activeTabId ? this.tabs.get(this.activeTabId) : null
    if (!tab || tab.view.isDestroyed()) throw new Error('no active tab')
    return tab.view.executeJavaScript(js)
  }

  // ── Dispose ───────────────────────────────────────────────────────────────────

  dispose(): void {
    this.overlay.onLayoutChange = null
    this._unsubBeforeHide?.()
    this._unsubBeforeHide = null
    this.closeAll()
  }
}
