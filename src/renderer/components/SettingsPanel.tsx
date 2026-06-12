import { useEffect, useMemo, useState, type KeyboardEvent } from 'react'
import { Heart, Palette, Keyboard, Gamepad2, Globe, Cpu, Info, ExternalLink, Mail, Trash2, FolderOpen } from 'lucide-react'
import { DiscordIcon } from './icons/DiscordIcon'
import type { Settings } from '@shared/types'
import { DEFAULT_SHORTCUTS, MIN_OPACITY, DEFAULT_HOMEPAGE, DEFAULT_PROTECTED_DOMAINS, HOMEPAGE_PRESETS, SEARCH_ENGINES } from '@shared/types'
import type { SearchEngineId } from '@shared/types'
import type { ShortcutId, Shortcuts } from '@shared/types'
import { useAppStore } from '../store/appStore'
import { useDiscordUrl } from '../hooks/useDiscordUrl'
import { Button } from './ui/Button'
import { Slider } from './ui/Slider'
import { Check, Field, Section, StringListEditor } from './settings/Layout'
import { ShortcutsSection } from './settings/ShortcutsSection'
import { GameDetectionSection } from './settings/GameDetectionSection'
import { cn } from '../lib/cn'

function isValidDomain(domain: string): boolean {
  return /^[^\s/]+\.[a-z]{2,}/i.test(domain.trim())
}

type TabId = 'appearance' | 'browser' | 'shortcuts' | 'detection' | 'system' | 'about'

const HOMEPAGE_TO_ENGINE: Partial<Record<string, SearchEngineId>> = {
  'https://www.google.com':   'google',
  'https://duckduckgo.com':   'duckduckgo',
  'https://www.bing.com':     'bing',
  'https://search.brave.com': 'brave',
}

interface TabDef {
  id: TabId
  label: string
  Icon: typeof Palette
}

const TABS: readonly TabDef[] = [
  { id: 'appearance', label: 'Appearance',     Icon: Palette },
  { id: 'browser',    label: 'Browser',        Icon: Globe },
  { id: 'shortcuts',  label: 'Shortcuts',      Icon: Keyboard },
  { id: 'detection',  label: 'Game detection', Icon: Gamepad2 },
  { id: 'system',     label: 'System',         Icon: Cpu },
  { id: 'about',      label: 'About',          Icon: Info },
] as const

export function SettingsPanel(): JSX.Element {
  const { settings, setSettings, activeProfile, setActiveProfile } = useAppStore()
  const discordUrl = useDiscordUrl()
  const [active, setActive] = useState<TabId>('appearance')
  const [version, setVersion] = useState('')
  const [liveOpacity, setLiveOpacity] = useState(activeProfile?.opacity ?? 1)
  const [homepageInput, setHomepageInput] = useState(settings?.homepageUrl ?? DEFAULT_HOMEPAGE)
  const [homepageError, setHomepageError] = useState(false)

  useEffect(() => {
    void window.aether.system.getVersion().then(setVersion).catch(() => { /* non-critical */ })
  }, [])
  useEffect(() => {
    setHomepageInput(settings?.homepageUrl ?? DEFAULT_HOMEPAGE)
  }, [settings?.homepageUrl])
  useEffect(() => {
    setLiveOpacity(activeProfile?.opacity ?? 1)
  }, [activeProfile?.opacity])
  useEffect(() => window.aether.on.opacityChanged(setLiveOpacity), [])

  const updateSetting = useMemo(
    () =>
      async <K extends keyof Settings>(key: K, value: Settings[K]): Promise<void> => {
        const next = await window.aether.settings.set(key, value)
        if (next) setSettings(next as Settings)
      },
    [setSettings],
  )

  if (!settings) return <div className="p-4 text-xs text-muted-foreground">Loading…</div>

  const saveHomepage = async (url: string): Promise<void> => {
    const domain = url.replace(/^https?:\/\//i, '').trim()
    if (!domain) { setHomepageInput(settings?.homepageUrl ?? DEFAULT_HOMEPAGE); setHomepageError(false); return }
    if (!isValidDomain(domain)) { setHomepageError(true); return }
    setHomepageError(false)
    const next = await window.aether.settings.set('homepageUrl', 'https://' + domain)
    if (next) { setSettings(next as Settings); setHomepageInput('https://' + domain) }
    else setHomepageInput(settings?.homepageUrl ?? DEFAULT_HOMEPAGE)
  }

  const selectHomepagePreset = async (url: string): Promise<void> => {
    setHomepageInput(url)
    await saveHomepage(url)
    const engineId = HOMEPAGE_TO_ENGINE[url]
    if (engineId && SEARCH_ENGINES[engineId]) {
      const next = await window.aether.settings.set('searchEngine', engineId)
      if (next) setSettings(next as Settings)
    }
  }

  const setOpacity = async (val: number): Promise<void> => {
    if (!activeProfile) return
    setLiveOpacity(val)
    await window.aether.overlay.setOpacity(val)
    setActiveProfile({ ...activeProfile, opacity: val })
  }

  const shortcuts: Shortcuts = settings.shortcuts ?? DEFAULT_SHORTCUTS

  const handleShortcutChange = async (id: ShortcutId, value: string | null): Promise<void> => {
    const next: Shortcuts = { ...DEFAULT_SHORTCUTS, ...shortcuts, [id]: value }
    await updateSetting('shortcuts', next)
  }

  const resetAllShortcuts = async (): Promise<void> => {
    await updateSetting('shortcuts', DEFAULT_SHORTCUTS)
  }

  // Roving tabindex / arrow-key nav for the sidebar (WAI-ARIA tabs pattern).
  const onTabKeyDown = (e: KeyboardEvent<HTMLDivElement>): void => {
    const idx = TABS.findIndex((t) => t.id === active)
    if (idx < 0) return
    let next: number
    if (e.key === 'ArrowDown') next = (idx + 1) % TABS.length
    else if (e.key === 'ArrowUp') next = (idx - 1 + TABS.length) % TABS.length
    else if (e.key === 'Home') next = 0
    else if (e.key === 'End') next = TABS.length - 1
    else return
    e.preventDefault()
    setActive(TABS[next].id)
  }

  return (
    <div className="flex h-full bg-background text-foreground" role="dialog" aria-label="Settings">
      {/* ── Sidebar ──────────────────────────────────────────────────── */}
      <nav
        role="tablist"
        aria-orientation="vertical"
        aria-label="Settings categories"
        onKeyDown={onTabKeyDown}
        className="shrink-0 w-[150px] border-r border-border bg-muted/30 py-2 px-1.5 flex flex-col gap-0.5 overflow-y-auto"
      >
        {TABS.map(({ id, label, Icon }) => {
          const isActive = active === id
          return (
            <button
              key={id}
              role="tab"
              type="button"
              id={`tab-${id}`}
              aria-selected={isActive}
              aria-controls={`panel-${id}`}
              tabIndex={isActive ? 0 : -1}
              onClick={() => setActive(id)}
              className={cn(
                'flex items-center gap-2 px-2 py-1.5 rounded text-[11px] text-left transition-colors',
                'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
                isActive
                  ? 'bg-primary/15 text-foreground'
                  : 'text-muted-foreground hover:text-foreground hover:bg-background/60',
              )}
            >
              <Icon size={13} className="shrink-0" />
              <span className="truncate">{label}</span>
            </button>
          )
        })}
      </nav>

      {/* ── Panels ───────────────────────────────────────────────────── */}
      <div className="flex-1 min-w-0 overflow-y-auto">
        <div
          role="tabpanel"
          id={`panel-${active}`}
          aria-labelledby={`tab-${active}`}
          className="p-4 space-y-4"
        >
          {active === 'appearance' && (
            <Section
              title="Appearance"
              description="How the overlay looks."
            >
              <Field
                label={`Opacity — ${Math.round(liveOpacity * 100)}%`}
                hint="How see-through the overlay is. You can also use Ctrl+Shift+↑ and Ctrl+Shift+↓ while in-game."
              >
                <Slider
                  value={[liveOpacity]}
                  min={MIN_OPACITY}
                  max={1}
                  step={0.05}
                  onValueChange={(v) => void setOpacity(v[0])}
                />
              </Field>
              <Check
                label="Show memory usage in the tab bar"
                hint="Click it for a breakdown per tab."
              >
                <input
                  type="checkbox"
                  checked={settings.showMemoryUsage ?? false}
                  onChange={(e) => void updateSetting('showMemoryUsage', e.target.checked)}
                />
              </Check>
            </Section>
          )}

          {active === 'browser' && (
            <>
              {/* ── Homepage ──────────────────────────────────── */}
              <Section
                title="Homepage"
                description="The page that opens when you create a new tab."
              >
                <div className="flex flex-col gap-1.5">
                  <div className="grid grid-cols-3 gap-1.5">
                    {HOMEPAGE_PRESETS.map(({ label, url }) => {
                      const checked = (settings.homepageUrl ?? DEFAULT_HOMEPAGE) === url
                      return (
                        <button
                          key={url}
                          type="button"
                          onClick={() => void selectHomepagePreset(url)}
                          className={cn(
                            'flex items-center justify-center h-8 rounded-lg border text-xs font-medium transition-colors',
                            checked
                              ? 'border-primary/60 bg-primary/10 text-primary'
                              : 'border-border text-muted-foreground hover:border-border/80 hover:text-foreground',
                          )}
                        >
                          {label}
                        </button>
                      )
                    })}
                  </div>
                  <Field label="Custom URL">
                    <div className="flex gap-2 items-start">
                      <div className="flex flex-col gap-1 flex-1 min-w-0">
                        <div className={cn(
                          'flex h-8 rounded border bg-input text-xs overflow-hidden focus-within:ring-1',
                          homepageError
                            ? 'border-destructive focus-within:ring-destructive'
                            : 'border-border focus-within:ring-ring',
                        )}>
                          <span className="flex items-center px-2 text-muted-foreground bg-muted/40 border-r border-border/60 select-none shrink-0">
                            https://
                          </span>
                          <input
                            type="text"
                            value={homepageInput.replace(/^https?:\/\//i, '')}
                            onChange={(e) => { setHomepageError(false); setHomepageInput('https://' + e.target.value) }}
                            onBlur={() => void saveHomepage(homepageInput)}
                            onKeyDown={(e) => { if (e.key === 'Enter') e.currentTarget.blur() }}
                            placeholder="example.com"
                            spellCheck={false}
                            aria-invalid={homepageError}
                            aria-describedby={homepageError ? 'homepage-url-error' : undefined}
                            className="flex-1 min-w-0 bg-transparent px-2 text-foreground placeholder:text-muted-foreground focus:outline-none"
                          />
                        </div>
                        {homepageError && (
                          <p id="homepage-url-error" role="alert" className="text-[11px] text-destructive">
                            Enter a valid domain — e.g. example.com
                          </p>
                        )}
                      </div>
                      {homepageInput !== (settings.homepageUrl ?? DEFAULT_HOMEPAGE) && (
                        <button
                          type="button"
                          onClick={() => void saveHomepage(homepageInput)}
                          className="h-8 px-3 rounded border border-primary/60 text-xs font-medium text-primary hover:bg-primary/10 transition-colors"
                        >
                          Save
                        </button>
                      )}
                    </div>
                  </Field>
                </div>
              </Section>

              {/* ── Dark mode ────────────────────────────────── */}
              <Section
                title="Dark mode"
                description="Makes websites use dark mode when they support it."
              >
                <Check
                  label="Enable dark mode for websites"
                >
                  <input
                    type="checkbox"
                    checked={settings.applyDarkMode ?? true}
                    onChange={(e) => void updateSetting('applyDarkMode', e.target.checked)}
                  />
                </Check>
              </Section>

              {/* ── Instant Gaming promo ─────────────────────── */}
              <Section
                title="Instant Gaming"
                description="Overframe is affiliated with Instant Gaming. A small deal card may appear occasionally while browsing."
              >
                <Check
                  label="Show Instant Gaming deal cards"
                >
                  <input
                    type="checkbox"
                    checked={settings.showIGPromo ?? true}
                    onChange={(e) => void updateSetting('showIGPromo', e.target.checked)}
                  />
                </Check>
              </Section>

              {/* ── Privacy — ad & cookie blocking ──────────────── */}
              <Section
                title="Privacy"
                description="Block ads and cookie consent banners using uBlock Origin. Disable this if a site breaks or if you want to support ad-supported partners."
              >
                <Check
                  label="Block ads & cookie banners (uBlock Origin)"
                >
                  <input
                    type="checkbox"
                    checked={settings.adBlockEnabled ?? false}
                    onChange={(e) => void updateSetting('adBlockEnabled', e.target.checked)}
                  />
                </Check>
              </Section>

              {/* ── Protected tabs ───────────────────────────────── */}
              <Section
                title="Protected tabs"
                description="These sites stay open when you switch profiles or hide the overlay — so your calls are never interrupted."
              >
                <StringListEditor
                  label="Protected domains"
                  hint="e.g. 'zoom.us', 'meet.google.com'."
                  values={settings.protectedDomains ?? DEFAULT_PROTECTED_DOMAINS}
                  placeholder="example.com"
                  normalize={(v) => v.trim().toLowerCase().replace(/^https?:\/\//, '').replace(/\/.*$/, '')}
                  validate={(v) => v.includes('.') ? null : 'Enter a valid domain (e.g. discord.com)'}
                  emptyText="No protected tabs — all tabs close on profile switch."
                  onReset={() => void updateSetting('protectedDomains', DEFAULT_PROTECTED_DOMAINS)}
                  onChange={(next) => void updateSetting('protectedDomains', next)}
                />
              </Section>

            </>
          )}

          {active === 'shortcuts' && (
            <ShortcutsSection
              shortcuts={shortcuts}
              onChange={handleShortcutChange}
              onReset={resetAllShortcuts}
            />
          )}

          {active === 'detection' && <GameDetectionSection />}

          {active === 'system' && (
            <>
              <Section
                title="System"
                description="System and performance options."
              >
                <Check
                  label="Launch at Windows startup"
                  hint="Starts Overframe in the tray when Windows boots."
                >
                  <input
                    type="checkbox"
                    checked={settings.startWithWindows}
                    onChange={(e) => void updateSetting('startWithWindows', e.target.checked)}
                  />
                </Check>
                <Check
                  label="Free up memory when the overlay is hidden"
                  hint="Tabs reload when you reopen the overlay."
                >
                  <input
                    type="checkbox"
                    checked={settings.performanceMode ?? false}
                    onChange={(e) => void updateSetting('performanceMode', e.target.checked)}
                  />
                </Check>
              </Section>

              <Section
                title="Folders"
                description="Open Overframe's folders in Explorer."
              >
                <div className="flex flex-col gap-1.5">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void window.aether.system.openFolder('userData')}
                    className="justify-start gap-2"
                  >
                    <FolderOpen size={11} /> User data — profiles, collections, settings
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void window.aether.system.openFolder('app')}
                    className="justify-start gap-2"
                  >
                    <FolderOpen size={11} /> Installation folder
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void window.aether.system.openFolder('logs')}
                    className="justify-start gap-2"
                  >
                    <FolderOpen size={11} /> Crash logs
                  </Button>
                </div>
              </Section>

              <Section
                title="Reset"
                description="Delete all your data and start fresh."
              >
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => void window.aether.system.resetData()}
                  className="justify-start gap-2"
                >
                  <Trash2 size={11} /> Reset all data &amp; relaunch
                </Button>
              </Section>

              <Section
                title="Uninstall"
                description="Permanently remove Overframe from your system."
              >
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => void window.aether.system.uninstall()}
                  className="justify-start gap-2"
                >
                  <Trash2 size={11} /> Uninstall Overframe
                </Button>
              </Section>
            </>
          )}

          {active === 'about' && (
            <div className="space-y-4">
              <Section title="Overframe">
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  A lightweight overlay browser for gamers. Browse guides, wikis and streams without leaving your game.
                </p>
                <p className="text-[11px] text-muted-foreground">
                  Version <span className="text-foreground font-medium">{version || '\u2014'}</span>
                  <span className="text-muted-foreground"> · </span>
                  MIT License
                </p>
              </Section>

              <Section title="Community &amp; support">
                <div className="flex flex-col gap-1.5">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void window.aether.tabs.create(discordUrl)}
                    className="justify-start gap-2 hover:border-indigo-500/50 hover:text-indigo-400"
                  >
                    <DiscordIcon size={11} /> Discord &mdash; bugs, ideas &amp; chat
                    <ExternalLink size={10} className="ml-auto text-muted-foreground" />
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void window.aether.tabs.create('https://overframe.app')}
                    className="justify-start gap-2"
                  >
                    <Globe size={11} /> overframe.app
                    <ExternalLink size={10} className="ml-auto text-muted-foreground" />
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void window.aether.system.openExternal('mailto:contact@overframe.app')}
                    className="justify-start gap-2"
                  >
                    <Mail size={11} /> contact@overframe.app
                    <ExternalLink size={10} className="ml-auto text-muted-foreground" />
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void window.aether.tabs.create('https://ko-fi.com/overframe')}
                    className="justify-start gap-2 hover:border-pink-500/50 hover:text-pink-400"
                  >
                    <Heart size={11} /> Support development on Ko-fi
                    <ExternalLink size={10} className="ml-auto text-muted-foreground" />
                  </Button>
                </div>
              </Section>

              <Section title="Legal">
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  No analytics, no telemetry, no account required. All data stays on your machine.
                  Provided as-is under the MIT license.
                </p>
                <div className="flex gap-1">
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => void window.aether.tabs.create('https://overframe.app/privacy')}
                    className="justify-start gap-1.5 text-muted-foreground"
                  >
                    <ExternalLink size={10} /> Privacy policy
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => void window.aether.tabs.create('https://overframe.app/terms')}
                    className="justify-start gap-1.5 text-muted-foreground"
                  >
                    <ExternalLink size={10} /> Terms of use
                  </Button>
                </div>
              </Section>

              {import.meta.env.DEV && (
                <Section title="Developer">
                  <Button
                    size="sm"
                    variant="destructive"
                    onClick={() => void window.aether.system.devStoreReset()}
                  >
                    Reset store &amp; relaunch
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void window.aether.settings.set('hasCompletedOnboarding', false).then((s) => { if (s) useAppStore.getState().setSettings(s) })}
                  >
                    Show onboarding
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void window.aether.system.simulateCrash()}
                  >
                    Write test crash
                  </Button>
                </Section>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
