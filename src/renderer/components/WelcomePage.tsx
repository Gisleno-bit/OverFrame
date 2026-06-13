import { useState, useEffect, useRef } from 'react'
import { Home, Newspaper, Trophy, Globe, RefreshCw, Loader2, CheckCircle2, AlertCircle, Mail, Plus, X } from 'lucide-react'
import { DiscordIcon } from './icons/DiscordIcon'

import { useAppStore } from '../store/appStore'
import { useDiscordUrl } from '../hooks/useDiscordUrl'
import { cn } from '../lib/cn'
import { MissionsPanel } from './MissionsPanel'

// ── IG partner banner data (extracted from instant-gaming.com/api/banner/partner/loader.js) ──
const IG_PROMO: Record<string, { text: string; cta: string }> = {
  fr: { text: 'Tous vos jeux vidéos, moins cher', cta: 'Découvrir' },
  en: { text: 'All your Video Games, cheaper',    cta: 'Discover' },
  es: { text: 'Todos tus videojuegos más baratos', cta: 'Descúbrelos' },
  it: { text: 'Tutti i tuoi videogiochi, scontati', cta: 'Scoprili' },
  de: { text: 'All deine Games, günstiger',        cta: 'Jetzt entdecken' },
  pt: { text: 'Todos os teus videojogos, mais baratos.', cta: 'Descobrir' },
  nl: { text: 'All your Video Games, cheaper',    cta: 'Discover' },
  pl: { text: 'Wszystkie Twoje gry wideo, taniej', cta: 'Sprawdź oferty!' },
}
const IG_BANNER_IMG_V = '1780387156'

// ── Seen-news persistence ────────────────────────────────────────────────────
const SEEN_KEY = 'overframe:seenAnnouncements'
function loadSeenIds(): Set<string> {
  try {
    const raw = localStorage.getItem(SEEN_KEY)
    return raw ? new Set<string>(JSON.parse(raw) as string[]) : new Set()
  } catch { return new Set() }
}

// ── Quick links ────────────────────────────────────────────────────────────────
interface QuickLink { name: string; url: string; description: string }
const STATIC_LINKS: QuickLink[] = [
  { name: 'YouTube', url: 'https://www.youtube.com', description: 'Videos'      },
  { name: 'Twitch',  url: 'https://www.twitch.tv',   description: 'Live streams' },
  { name: 'Reddit',  url: 'https://www.reddit.com',  description: 'Communities'  },
]

const HOMEPAGE_LABELS: Record<string, string> = {
  'https://www.google.com':   'Google',
  'https://duckduckgo.com':   'DuckDuckGo',
  'https://www.bing.com':     'Bing',
  'https://search.brave.com': 'Brave',
}

function homepageLink(url: string): QuickLink {
  const name = HOMEPAGE_LABELS[url] ?? (() => {
    try { return new URL(url).hostname.replace(/^www\./, '') } catch { return 'Home' }
  })()
  return { name, url, description: 'Search' }
}

// â”€â”€ News / patch-notes feed â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
interface GHRelease {
  tag_name: string
  name: string
  published_at: string
  body: string
  html_url: string
  prerelease: boolean
}

const GH_RELEASES_URL = 'https://api.github.com/repos/overframeApp-arch/Overframe/releases'

function stripMd(md: string): string {
  return md
    .replace(/#{1,6}\s+/g, '')
    .replace(/\*\*(.*?)\*\*/g, '$1')
    .replace(/\*(.*?)\*/g, '$1')
    .replace(/`{1,3}[^`]*`{1,3}/g, '')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/^[-*+]\s+/gm, '')
    .replace(/\r?\n+/g, ' ')
    .trim()
}

function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })
}

function useGitHubReleases(): { releases: GHRelease[] | null; error: boolean } {
  const [releases, setReleases] = useState<GHRelease[] | null>(null)
  const [error, setError] = useState(false)
  useEffect(() => {
    let cancelled = false
    fetch(GH_RELEASES_URL, { headers: { Accept: 'application/vnd.github+json' } })
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error(r.statusText))))
      .then((data: GHRelease[]) => { if (!cancelled) setReleases(data) })
      .catch(() => { if (!cancelled) setError(true) })
    return () => { cancelled = true }
  }, [])
  return { releases, error }
}

type HomeTab = 'home' | 'missions' | 'news'

type UpdateStatus =
  | null
  | { status: 'checking' }
  | { status: 'up-to-date' }
  | { status: 'available'; version: string }
  | { status: 'downloaded'; version: string }
  | { status: 'error'; message: string }
  | { status: 'dev' }

export function WelcomePage(): JSX.Element | null {
  const { activeTabId, settings } = useAppStore()
  const discordUrl = useDiscordUrl()
  const [tab, setTab] = useState<HomeTab>('home')
  const [addingLink, setAddingLink] = useState(false)
  const [newLinkDraft, setNewLinkDraft] = useState('')
  const [newLinkError, setNewLinkError] = useState('')
  const addInputRef = useRef<HTMLInputElement>(null)

  const igLang = (() => {
    const l = navigator.language.split('-')[0].toLowerCase()
    return l in IG_PROMO ? l : 'en'
  })()
  const [seenIds, setSeenIds] = useState<Set<string>>(loadSeenIds)
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>(null)
  const [version, setVersion] = useState('')
  const checkTimeoutRef = useRef<NodeJS.Timeout | null>(null)
  const { releases, error: releasesError } = useGitHubReleases()

  const bannerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const link = document.createElement('link')
    link.rel = 'stylesheet'
    link.href = 'https://www.instant-gaming.com/api/banner/partner/style.css'
    document.head.appendChild(link)
    // IG's ::before/::after don't set left/right — on a flex div the static-position
    // can drift. Pin them explicitly so the gradient always spans the full width.
    const fix = document.createElement('style')
    fix.textContent = '.ig-dynamic-banner::before,.ig-dynamic-banner::after{left:0!important;right:0!important}'
    document.head.appendChild(fix)
    return () => {
      try { document.head.removeChild(link) } catch { /* already removed */ }
      try { document.head.removeChild(fix) } catch { /* already removed */ }
    }
  }, [])

  // Replicate IG's own JS: apply ig-size-XXX class based on rendered banner width
  // so their CSS responsive breakpoints (column layout, gradients, etc.) kick in correctly.
  useEffect(() => {
    const el = bannerRef.current
    if (!el) return
    const SIZE_CLASSES = ['ig-size-500','ig-size-650','ig-size-800','ig-size-900','ig-size-1000','ig-size-1100'] as const
    // Thresholds use offsetWidth (border-box) to stay IG-CSS-load-order independent.
    // Small (column/Y): 500, 650, 800. Large (row/X): 900, 1000, 1100.
    const getSizeClass = (w: number): typeof SIZE_CLASSES[number] => {
      if (w >= 1090) return 'ig-size-1100'
      if (w >= 990)  return 'ig-size-1000'
      if (w >= 890)  return 'ig-size-900'
      if (w >= 765)  return 'ig-size-800'
      if (w >= 615)  return 'ig-size-650'
      return 'ig-size-500'  // never ig-size-400 (it hides the CTA button)
    }
    const apply = (w: number) => {
      const cls = getSizeClass(w)
      SIZE_CLASSES.forEach((c) => el.classList.remove(c))
      el.classList.add(cls)
    }
    // Apply immediately so the class is set before the async ResizeObserver fires.
    apply(el.offsetWidth)
    const ro = new ResizeObserver(([entry]) => {
      // Use borderBoxSize when available (always border-box, independent of CSS padding).
      const w = entry.borderBoxSize?.[0]?.inlineSize ?? el.offsetWidth
      apply(w)
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  useEffect(() => {
    void window.aether.system.getVersion().then(setVersion).catch(() => { /* non-critical */ })
  }, [])
  useEffect(() =>
    window.aether.on.updateStatus((s) => {
      if (checkTimeoutRef.current) { clearTimeout(checkTimeoutRef.current); checkTimeoutRef.current = null }
      const status = s as UpdateStatus
      setUpdateStatus(status)
      if (status?.status === 'up-to-date') scheduleUpToDateDismiss()
    })
  , [])

  const dismissTimer = useRef<NodeJS.Timeout | null>(null)
  const scheduleUpToDateDismiss = (): void => {
    if (dismissTimer.current) clearTimeout(dismissTimer.current)
    dismissTimer.current = setTimeout(() => {
      dismissTimer.current = null
      setUpdateStatus((prev) => prev?.status === 'up-to-date' ? null : prev)
    }, 3_000)
  }

  const handleCheckForUpdates = (): void => {
    setUpdateStatus({ status: 'checking' })
    void window.aether.system.checkForUpdates()
    // Fallback: if no status event arrives within 8s, assume up-to-date
    checkTimeoutRef.current = setTimeout(() => {
      checkTimeoutRef.current = null
      setUpdateStatus((prev) => {
        if (prev?.status === 'checking') { scheduleUpToDateDismiss(); return { status: 'up-to-date' } }
        return prev
      })
    }, 8_000)
  }

  if (activeTabId !== null) return null
  if (!settings) return null

  const customLinks = settings.quickLinks ?? []

  const commitAddLink = async (): Promise<void> => {
    let url = newLinkDraft.trim()
    if (!url) return
    if (!url.startsWith('http://') && !url.startsWith('https://')) url = 'https://' + url
    try {
      const hostname = new URL(url).hostname
      if (!hostname) { setNewLinkError('Invalid URL'); return }
      const name = hostname.replace(/^www\./, '')
      await window.aether.settings.set('quickLinks', [...customLinks, { name, url }])
      setAddingLink(false)
      setNewLinkDraft('')
      setNewLinkError('')
    } catch {
      setNewLinkError('Invalid URL')
    }
  }

  const removeCustomLink = async (url: string): Promise<void> => {
    await window.aether.settings.set('quickLinks', customLinks.filter((l) => l.url !== url))
  }

  const hasNew = releases?.some((r) => !seenIds.has(r.tag_name)) ?? false

  function switchTab(next: HomeTab): void {
    setTab(next)
    if (next === 'news' && releases) {
      const freshIds = releases.filter((r) => !seenIds.has(r.tag_name)).map((r) => r.tag_name)
      if (freshIds.length > 0) {
        const updated = new Set([...seenIds, ...freshIds])
        localStorage.setItem(SEEN_KEY, JSON.stringify([...updated]))
        setSeenIds(updated)
      }
    }
  }

  const tabBtn = (id: HomeTab, icon: React.ReactNode, label: React.ReactNode): JSX.Element => (
    <button
      role="tab"
      aria-selected={tab === id}
      type="button"
      onClick={() => switchTab(id)}
      className={cn(
        'relative flex items-center gap-1.5 px-3 py-2.5 text-[11px] font-medium -mb-px border-b-2 transition-colors outline-none focus-visible:ring-1 focus-visible:ring-ring',
        tab === id
          ? 'border-primary text-foreground'
          : 'border-transparent text-muted-foreground hover:text-foreground',
      )}
    >
      {icon}
      {label}
    </button>
  )

  return (
    <div className="absolute inset-0 flex flex-col bg-background">
      {/* Tab bar */}
      <div role="tablist" className="flex items-center gap-1 px-3 border-b border-border shrink-0">
        {tabBtn('home', <Home size={11} />, 'Home')}
        {tabBtn('missions', <Trophy size={11} />, 'Missions')}
        {tabBtn('news', <Newspaper size={11} />,
          <>
            News
            {hasNew && (
              <span className="absolute top-1.5 right-0.5 h-1.5 w-1.5 rounded-full bg-red-500" />
            )}
          </>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 min-h-0 overflow-y-auto">

        {/* Home tab — always in DOM so the partner banner div is never unmounted */}
        <div className={cn('flex flex-col gap-5 py-5', tab !== 'home' && 'hidden')} aria-hidden={tab !== 'home'}>

          {/* ── IG partner banner ────────────────────────────────────────────── */}
          <section aria-label="Instant Gaming — affiliate partner" className="flex justify-center px-5">
            <div
              ref={bannerRef}
              role="button"
              tabIndex={0}
              onClick={() => void window.aether.tabs.create(
                `https://www.instant-gaming.com/${igLang}/?igr=overframe&utm_source=dynamic_banner&utm_campaign=19535FORZA`
              )}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') void window.aether.tabs.create(
                  `https://www.instant-gaming.com/${igLang}/?igr=overframe&utm_source=dynamic_banner&utm_campaign=19535FORZA`
                )
              }}
              className="ig-dynamic-banner w-full cursor-pointer focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-white/50 transition-opacity hover:opacity-90 active:opacity-80"
              style={{
                backgroundImage: `url('https://www.instant-gaming.com/images/bp/16/16-${igLang}.jpg?v=${IG_BANNER_IMG_V}')`,
                backgroundSize: 'cover',
                backgroundPosition: 'center center',
                height: '220px',
                maxWidth: '1200px',
                minHeight: 'unset',
              }}
              aria-label={`Instant Gaming — ${IG_PROMO[igLang]?.text ?? ''}`}
            >
              <div className="ig-dynamic-banner-logo" />
              <div className="ig-dynamic-banner-text">
                <div className="ig-dynamic-banner-title">{IG_PROMO[igLang]?.text}</div>
                <div className="ig-dynamic-banner-cta">{IG_PROMO[igLang]?.cta}</div>
              </div>
            </div>
          </section>

          {/* ── Contenu centré à 1200px max ──────────────────────────────────── */}
          <div className="w-full max-w-[1200px] mx-auto px-5 flex flex-col gap-5">

          {/* Brand header */}
          <div className="flex flex-col gap-1">
            <p className="text-[14px] font-semibold tracking-tight text-foreground">Good to have you.</p>
            <p className="text-xs text-muted-foreground leading-relaxed">
              Remember: set your game to <span className="text-foreground/70">borderless windowed</span> mode for the overlay to appear on top.
            </p>
          </div>

          <div className="h-px bg-border/50" />

          {/* Quick access */}
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <p className="text-[11px] uppercase tracking-[0.08em] font-semibold text-muted-foreground">Quick access</p>
              {!addingLink && (
                <button
                  type="button"
                  onClick={() => { setAddingLink(true); setNewLinkDraft(''); setNewLinkError(''); setTimeout(() => addInputRef.current?.focus(), 0) }}
                  className="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                  aria-label="Add quick link"
                >
                  <Plus size={11} /> Add
                </button>
              )}
            </div>
            <div className="grid grid-cols-2 gap-1.5">
              {[homepageLink(settings.homepageUrl ?? 'https://www.google.com'), ...STATIC_LINKS].map(({ name, url, description }) => {
                const hostname = (() => { try { return new URL(url).hostname } catch { return '' } })()
                const favicon = hostname ? `https://www.google.com/s2/favicons?domain=${hostname}&sz=32` : null
                return (
                  <button
                    key={url}
                    type="button"
                    onClick={() => void window.aether.tabs.create(url)}
                    className="flex items-center gap-2.5 px-3 py-2.5 rounded-md bg-muted border border-border hover:border-primary/40 hover:bg-muted/80 text-left transition-colors group"
                  >
                    {favicon && <img src={favicon} alt="" width={16} height={16} className="rounded-sm shrink-0 opacity-80 group-hover:opacity-100 transition-opacity" onError={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none' }} />}
                    <div className="flex flex-col min-w-0">
                      <span className="text-[11px] font-medium text-foreground truncate">{name}</span>
                      <span className="text-[11px] text-muted-foreground truncate">{description}</span>
                    </div>
                  </button>
                )
              })}

              {/* Custom links */}
              {customLinks.map(({ name, url }) => {
                const hostname = (() => { try { return new URL(url).hostname } catch { return '' } })()
                const favicon = hostname ? `https://www.google.com/s2/favicons?domain=${hostname}&sz=32` : null
                return (
                  <div key={url} className="relative group/custom">
                    <button
                      type="button"
                      onClick={() => void window.aether.tabs.create(url)}
                      className="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-md bg-muted border border-border hover:border-primary/40 hover:bg-muted/80 text-left transition-colors"
                    >
                      {favicon && <img src={favicon} alt="" width={16} height={16} className="rounded-sm shrink-0 opacity-80" onError={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none' }} />}
                      <span className="text-[11px] font-medium text-foreground truncate pr-4">{name}</span>
                    </button>
                    <button
                      type="button"
                      onClick={() => void removeCustomLink(url)}
                      aria-label={`Remove ${name}`}
                      className="absolute top-1.5 right-1.5 opacity-0 group-hover/custom:opacity-100 p-0.5 rounded text-muted-foreground hover:text-destructive transition-all"
                    >
                      <X size={10} />
                    </button>
                  </div>
                )
              })}

              {/* Add form */}
              {addingLink && (
                <div className="col-span-2 flex items-center gap-1.5">
                  <input
                    ref={addInputRef}
                    type="text"
                    value={newLinkDraft}
                    onChange={(e) => { setNewLinkDraft(e.target.value); setNewLinkError('') }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') void commitAddLink()
                      if (e.key === 'Escape') { setAddingLink(false); setNewLinkDraft('') }
                    }}
                    placeholder="example.com"
                    className={cn(
                      'flex-1 min-w-0 rounded border bg-input px-2.5 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1',
                      newLinkError ? 'border-destructive focus:ring-destructive' : 'border-border focus:ring-ring',
                    )}
                  />
                  <button type="button" onClick={() => void commitAddLink()} className="px-2.5 py-1.5 rounded border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-muted/40 transition-colors">Add</button>
                  <button type="button" onClick={() => { setAddingLink(false); setNewLinkDraft('') }} className="px-2 py-1.5 rounded text-xs text-muted-foreground hover:text-foreground transition-colors">Cancel</button>
                </div>
              )}
              {newLinkError && <p className="col-span-2 text-[11px] text-destructive">{newLinkError}</p>}
            </div>
          </div>

          {/* Support section */}
          <div className="flex flex-col gap-2">
            <p className="text-[11px] uppercase tracking-[0.08em] font-semibold text-muted-foreground">Support Overframe</p>
            <div className="grid grid-cols-2 gap-1.5">
              {([
                {
                  url: `https://www.instant-gaming.com/${igLang}/?igr=overframe`,
                  favicon: 'https://www.google.com/s2/favicons?domain=instant-gaming.com&sz=32',
                  name: 'Instant Gaming',
                  desc: 'Buy games at a discount — same price for you, small commission for us.',
                },
                {
                  url: 'https://ko-fi.com/overframe',
                  favicon: 'https://www.google.com/s2/favicons?domain=ko-fi.com&sz=32',
                  name: 'Ko-fi',
                  desc: 'One-time donation or monthly membership. Directly supports development.',
                },
              ] as const).map(({ url, favicon, name, desc }) => (
                <button
                  key={url}
                  type="button"
                  onClick={() => void window.aether.tabs.create(url)}
                  className="flex items-start gap-2.5 px-3 py-2.5 rounded-md bg-muted border border-border hover:border-primary/40 hover:bg-muted/80 text-left transition-colors group"
                >
                  <img src={favicon} alt="" width={16} height={16} className="rounded-sm shrink-0 mt-0.5 opacity-80 group-hover:opacity-100 transition-opacity" onError={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none' }} />
                  <div className="flex flex-col gap-0.5 min-w-0">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] font-medium text-foreground">{name}</span>
                      {name === 'Instant Gaming' && (
                        <span className="text-[11px] px-1.5 py-px rounded border leading-none border-orange-400/40 bg-orange-400/10 text-orange-200 font-medium">
                          affiliate
                        </span>
                      )}
                    </div>
                    <span className="text-[11px] text-muted-foreground leading-snug">{desc}</span>
                  </div>
                </button>
              ))}
            </div>
          </div>

          </div>{/* end max-w-[1200px] wrapper */}
        </div>

        {tab === 'missions' && (
          <div className="w-full max-w-[1200px] mx-auto px-5 py-4">
            <MissionsPanel />
          </div>
        )}

        {tab === 'news' && (
          <div className="w-full max-w-[1200px] mx-auto flex flex-col gap-3 px-5 py-4">
            {releases === null && !releasesError && (
              <div className="flex items-center justify-center py-16">
                <Loader2 size={18} className="animate-spin text-muted-foreground" />
              </div>
            )}
            {releasesError && (
              <div className="flex flex-col items-center justify-center py-16 gap-2 text-center">
                <AlertCircle size={28} className="text-muted-foreground" />
                <p className="text-[11px] text-muted-foreground">Could not load releases — check your connection.</p>
              </div>
            )}
            {releases?.length === 0 && (
              <div className="flex flex-col items-center justify-center py-16 gap-2 text-center">
                <Newspaper size={28} className="text-muted-foreground" />
                <p className="text-[11px] text-muted-foreground">No releases yet — check back soon.</p>
              </div>
            )}
            {releases?.map(({ tag_name, name, published_at, body, html_url, prerelease }) => {
              const isNew = !seenIds.has(tag_name)
              const trimmed = stripMd(body)
              const excerpt = trimmed.length > 220 ? trimmed.slice(0, 220).trimEnd() + '…' : trimmed
              return (
                <div
                  key={tag_name}
                  className="flex flex-col gap-2 p-3 rounded-md border bg-muted border-border"
                >
                  <div className="flex items-center gap-2">
                    {prerelease && (
                      <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-semibold uppercase tracking-wide bg-amber-500/15 text-amber-400 border border-amber-500/20 leading-none">
                        Early Access
                      </span>
                    )}
                    {isNew && (
                      <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-semibold uppercase tracking-wide bg-primary/10 text-primary border border-primary/20 leading-none">
                        new
                      </span>
                    )}
                  </div>
                  <span className="text-[11px] font-semibold text-foreground">{name || tag_name}</span>
                  {excerpt && <p className="text-[11px] text-muted-foreground leading-relaxed">{excerpt}</p>}
                  <div className="flex items-center gap-3">
                    <span className="text-[11px] text-muted-foreground">{fmtDate(published_at)}</span>
                    <button
                      type="button"
                      onClick={() => void window.aether.tabs.create(html_url)}
                      className="text-[11px] text-primary hover:underline"
                    >
                      Read more →
                    </button>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="flex items-center gap-3 px-5 py-2.5 border-t border-border shrink-0">
        <button
          type="button"
          className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-indigo-400 transition-colors"
          onClick={() => void window.aether.tabs.create(discordUrl)}
        >
          <DiscordIcon size={11} /> Community
        </button>
        <button
          type="button"
          className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
          onClick={() => void window.aether.tabs.create('https://overframe.app')}
        >
          <Globe size={11} /> Website
        </button>
        <button
          type="button"
          className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
          onClick={() => void window.aether.system.openExternal('mailto:contact@overframe.app')}
        >
          <Mail size={11} /> Contact
        </button>

        <span className="flex-1" />

        {/* Update status feedback */}
        {updateStatus && updateStatus.status !== 'checking' && (
          <>
            {updateStatus.status === 'up-to-date' && (
              <span className="inline-flex items-center gap-1 text-[11px] text-muted-foreground">
                <CheckCircle2 size={11} className="text-green-500" aria-hidden="true" />Up to date
              </span>
            )}
            {updateStatus.status === 'available' && (
              <span className="inline-flex items-center gap-1 text-[11px] text-muted-foreground">
                <Loader2 size={11} className="animate-spin text-blue-400" aria-hidden="true" />Downloading update…
              </span>
            )}
            {updateStatus.status === 'downloaded' && (
              <button
                type="button"
                onClick={() => void window.aether.system.restartToUpdate()}
                className="inline-flex items-center gap-1 text-[11px] text-green-400 hover:text-green-300 transition-colors font-medium"
              >
                <CheckCircle2 size={11} aria-hidden="true" />Restart to update
              </button>
            )}
            {updateStatus.status === 'error' && (
              <span className="inline-flex items-center gap-1 text-[11px] text-destructive" title={updateStatus.message}>
                <AlertCircle size={11} aria-hidden="true" />Update failed
              </span>
            )}
          </>
        )}

        {version && (
          <span className="text-[11px] text-muted-foreground" aria-label={`Version ${version}`}>
            {version}
          </span>
        )}

        <button
          type="button"
          disabled={updateStatus?.status === 'checking'}
          onClick={handleCheckForUpdates}
          className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {updateStatus?.status === 'checking'
            ? <Loader2 size={11} className="animate-spin" aria-hidden="true" />
            : <RefreshCw size={11} aria-hidden="true" />}
          {updateStatus?.status === 'checking' ? 'Checking…' : 'Check for updates'}
        </button>
      </div>
    </div>
  )
}
