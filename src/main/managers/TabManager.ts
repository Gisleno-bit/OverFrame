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

  constructor(private overlay: OverlayWindow) {
    this.overlay.win.on('resize', () => this.relayoutActive())
    this.overlay.onLayoutChange = () => this.relayoutActive()
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

    view.on('did-start-loading', () => update({ isLoading: true }))

    view.on('did-finish-load', () => {
      update({ isLoading: false, canGoBack: view.canGoBack(), canGoForward: view.canGoForward() })
      const url = view.getURL()
      if (!url.startsWith('http://') && !url.startsWith('https://')) return
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

    view.on('new-window', (url: unknown) => {
      if (typeof url === 'string') this.handlePopup(url)
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
  }


  // ── Close ────────────────────────────────────────────────────────────────────

  close(id: string): void {
    const tab = this.tabs.get(id)
    if (!tab) return

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

    // Position + show the active tab
    const { x, y, width, height } = this.overlay.getTabContentBounds()
    tab.view.setBounds(x, y, width, height)
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
    const tab = this.activeTabId ? this.tabs.get(this.activeTabId) : null
    if (!tab || tab.view.isDestroyed()) return
    const { x, y, width, height } = this.overlay.getTabContentBounds()
    tab.view.setBounds(x, y, width, height)
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

  unloadAll(): void {
    this.stoppedDuringHide.clear()
    this.unloadedUrls.clear()
    for (const tab of this.tabs.values()) {
      try {
        const url = tab.state.url
        if (url && url !== 'about:blank') {
          this.unloadedUrls.set(tab.id, url)
          tab.view.loadURL('about:blank')
        }
      } catch { /* destroyed */ }
    }
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

  private handlePopup(url: string): void {
    // Security: only open http(s) popups as tabs — drop javascript:/data:/custom schemes.
    try {
      const proto = new URL(url).protocol
      if (proto !== 'http:' && proto !== 'https:') return
    } catch {
      return
    }
    const now = Date.now()
    const last = this.recentPopups.get(url) ?? 0
    if (now - last <= POPUP_DEDUP_WINDOW_MS) return
    this.recentPopups.set(url, now)
    if (this.recentPopups.size > POPUP_DEDUP_EVICT_THRESHOLD) {
      const cutoff = now - POPUP_DEDUP_MAX_AGE_MS
      for (const [u, t] of this.recentPopups) {
        if (t < cutoff) this.recentPopups.delete(u)
      }
    }
    this.create(url)
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
    this.closeAll()
  }
}
