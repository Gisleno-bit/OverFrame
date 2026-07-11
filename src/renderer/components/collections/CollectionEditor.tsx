import { useMemo, useRef, useState } from 'react'
import {
  Check, Download, FolderPlus, Globe, GripVertical, LayoutList,
  Pencil, Plus, Trash2, X,
} from 'lucide-react'
import type { Collection, CollectionAuthor, CreatorLink, CreatorPlatform, Link } from '@shared/types'
import { cn } from '../../lib/cn'
import { Button } from '../ui/Button'
import { Input } from '../ui/Input'
import { Tooltip } from '../ui/Tooltip'
import { Favicon } from './atoms'

// ── Types ─────────────────────────────────────────────────────────────────────

type Tab = { id: string; title: string; url: string; favicon: string | null }

interface CollectionEditorProps {
  collection: Collection
  tabs: Tab[]
  onAddLink: (link: { title: string; url: string; favicon?: string; section?: string }) => void
  onEditLink: (lid: string, title: string, url: string, note: string) => void
  onRemoveLink: (lid: string) => void
  onReorderLinks: (ids: string[]) => void
  onSetSections: (sections: string[]) => void
  onRenameSection: (oldName: string, newName: string) => void
  onDeleteSection: (name: string) => void
  onMoveLink: (lid: string, targetSection: string | null, insertBeforeLinkId: string | null) => void
  onSetName: (name: string) => void
  onSetDescription: (desc: string | null) => void
  onSetIconUrl: (url: string | null) => void
  onSetBannerUrl: (url: string | null) => void
  onSetAuthor: (author: CollectionAuthor | null) => void
  onExport?: () => void
}

type DropTarget =
  | { kind: 'before'; linkId: string; section: string | null }
  | { kind: 'append'; section: string | null }

const DEFAULT_SECTIONS = ['My Content', 'Resources', 'Community']
const UNSORTED = '__unsorted__'

// ── Platform auto-detection ───────────────────────────────────────────────────

function detectPlatform(url: string): CreatorPlatform {
  try {
    const h = new URL(url).hostname.toLowerCase()
    if (h.includes('twitch.tv')) return 'twitch'
    if (h.includes('youtube.com') || h.includes('youtu.be')) return 'youtube'
    if (h.includes('kick.com')) return 'kick'
    if (h.includes('twitter.com') || h.includes('x.com')) return 'twitter'
    if (h.includes('tiktok.com')) return 'tiktok'
    if (h.includes('discord.gg') || h.includes('discord.com')) return 'discord'
    if (h.includes('ko-fi.com')) return 'kofi'
    if (h.includes('patreon.com')) return 'patreon'
  } catch { /* invalid URL */ }
  return 'website'
}

// ── Creator link chip (favicon only) ─────────────────────────────────────────

function CreatorLinkChip({ link, onRemove }: { link: CreatorLink; onRemove: () => void }): JSX.Element {
  let label = link.url
  try { label = new URL(link.url).hostname.replace(/^www\./, '') } catch { /* */ }
  return (
    <Tooltip label={label}>
      <div className="relative group/p">
        <a
          href={link.url}
          target="_blank"
          rel="noreferrer"
          aria-label={label}
          className="inline-flex items-center justify-center h-7 w-7 rounded-lg border border-border/40 bg-muted/30 hover:bg-muted/60 transition-colors"
          onClick={(e) => e.preventDefault()}
        >
          <Favicon url={link.url} className="w-[18px] h-[18px]" />
        </a>
        <button
          type="button"
          onClick={onRemove}
          aria-label={`Remove ${label}`}
          className="absolute -top-1.5 -right-1.5 h-3.5 w-3.5 rounded-full bg-muted border border-border flex items-center justify-center opacity-0 group-hover/p:opacity-100 transition-opacity hover:bg-destructive hover:border-destructive hover:text-white"
        >
          <X size={7} />
        </button>
      </div>
    </Tooltip>
  )
}

// ── Creator links row ─────────────────────────────────────────────────────────

function CreatorLinks({
  links,
  onAdd,
  onRemove,
}: {
  links: CreatorLink[]
  onAdd: (platform: CreatorPlatform, url: string) => void
  onRemove: (platform: CreatorPlatform) => void
}): JSX.Element {
  const [adding, setAdding] = useState(false)
  const [urlDraft, setUrlDraft] = useState('')

  const commit = (): void => {
    let u = urlDraft.trim()
    if (!u) return
    if (!/^https?:\/\//i.test(u)) u = `https://${u}`
    try { new URL(u) } catch { return }
    onAdd(detectPlatform(u), u)
    setUrlDraft('')
    setAdding(false)
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {links.map((l) => (
        <CreatorLinkChip key={l.platform} link={l} onRemove={() => onRemove(l.platform)} />
      ))}
      {adding ? (
        <div className="flex items-center gap-1">
          <Input
            autoFocus
            value={urlDraft}
            onChange={(e) => setUrlDraft(e.target.value)}
            placeholder="https://…"
            className="h-7 w-36 text-[11px]"
            onKeyDown={(e) => {
              if (e.key === 'Enter') commit()
              if (e.key === 'Escape') { setAdding(false); setUrlDraft('') }
            }}
          />
          <button type="button" onClick={commit}
            className="h-7 px-2 rounded text-[11px] bg-primary/15 text-primary hover:bg-primary/25 transition-colors shrink-0">
            Add
          </button>
          <button type="button" onClick={() => { setAdding(false); setUrlDraft('') }}
            className="h-7 w-7 rounded flex items-center justify-center text-muted-foreground hover:bg-muted/50 shrink-0">
            <X size={11} />
          </button>
        </div>
      ) : (
        <Tooltip label="Add link">
          <button
            type="button"
            aria-label="Add link"
            onClick={() => setAdding(true)}
            className="inline-flex items-center justify-center h-7 w-7 rounded-lg border border-dashed border-border/50 text-muted-foreground/50 hover:text-muted-foreground hover:border-border transition-colors"
          >
            <Plus size={13} />
          </button>
        </Tooltip>
      )}
    </div>
  )
}

// ── Inline link edit form ─────────────────────────────────────────────────────

function LinkEditForm({
  link, onSave, onCancel,
}: {
  link: Link
  onSave: (title: string, url: string, note: string) => void
  onCancel: () => void
}): JSX.Element {
  const [title, setTitle] = useState(link.title)
  const [url, setUrl] = useState(link.url)
  const [note, setNote] = useState(link.note ?? '')

  const commit = (): void => {
    const u = url.trim(); if (!u) return
    onSave(title.trim() || u, u, note.trim())
  }

  return (
    <div className="px-4 py-3 flex flex-col gap-2 bg-muted/20 border-y border-border/40">
      <Input autoFocus aria-label="Title" value={title} onChange={(e) => setTitle(e.target.value)}
        placeholder="Title…" className="h-7 text-[12px]"
        onKeyDown={(e) => { if (e.key === 'Enter') commit(); if (e.key === 'Escape') onCancel() }} />
      <Input aria-label="URL" value={url} onChange={(e) => setUrl(e.target.value)}
        placeholder="URL…" className="h-7 text-[11px]"
        onKeyDown={(e) => { if (e.key === 'Enter') commit(); if (e.key === 'Escape') onCancel() }} />
      <textarea aria-label="Note" value={note} onChange={(e) => setNote(e.target.value)}
        placeholder="Note (optional)…" rows={2}
        className="w-full rounded border bg-input px-2.5 py-1.5 text-[11px] text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 border-border focus:ring-ring resize-none"
        onKeyDown={(e) => { if (e.key === 'Escape') onCancel() }} />
      <div className="flex gap-2">
        <button type="button" onClick={commit}
          className="flex-1 h-7 rounded text-[12px] font-medium bg-primary/15 text-primary hover:bg-primary/25 transition-colors">Save</button>
        <button type="button" onClick={onCancel}
          className="flex-1 h-7 rounded text-[12px] text-muted-foreground hover:bg-muted/50 transition-colors">Cancel</button>
      </div>
    </div>
  )
}

// ── Add-link mini form ────────────────────────────────────────────────────────

function AddLinkForm({
  section, tabs, onAdd, onCancel,
}: {
  section: string | null
  tabs: Tab[]
  onAdd: (link: { title: string; url: string; favicon?: string; section?: string }) => void
  onCancel: () => void
}): JSX.Element {
  const [url, setUrl] = useState('')
  const [title, setTitle] = useState('')
  const [showTabs, setShowTabs] = useState(false)

  const submit = (): void => {
    let u = url.trim(); if (!u) return
    if (!/^https?:\/\//i.test(u)) u = `https://${u}`
    try { new URL(u) } catch { return }
    onAdd({ title: title.trim() || u, url: u, ...(section ? { section } : {}) })
    onCancel()
  }

  if (showTabs) return (
    <div className="border-t border-border/30 bg-muted/10">
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border/20">
        <span className="text-[11px] font-medium flex-1">Open tabs</span>
        <button type="button" onClick={() => setShowTabs(false)} className="text-muted-foreground hover:text-foreground"><X size={11} /></button>
      </div>
      {tabs.length === 0
        ? <p className="px-4 py-3 text-[11px] text-muted-foreground">No open tabs</p>
        : tabs.map((tab) => (
          <button key={tab.id} type="button"
            onClick={() => { onAdd({ title: tab.title || tab.url, url: tab.url, favicon: tab.favicon ?? undefined, ...(section ? { section } : {}) }); onCancel() }}
            className="w-full flex items-center gap-2.5 px-4 py-2 hover:bg-muted/40 text-left border-b border-border/20 last:border-b-0 transition-colors">
            <Favicon url={tab.url} favicon={tab.favicon} className="w-4 h-4 shrink-0" />
            <div className="min-w-0 flex-1">
              <div className="text-[11px] font-medium truncate">{tab.title || tab.url}</div>
              <div className="text-[10px] text-muted-foreground truncate">{tab.url}</div>
            </div>
          </button>
        ))
      }
    </div>
  )

  return (
    <div className="px-4 py-2.5 flex flex-col gap-1.5 border-t border-border/30 bg-muted/10">
      <Input autoFocus aria-label="URL" value={url} onChange={(e) => setUrl(e.target.value)}
        placeholder="Paste a URL…" className="h-7 text-[12px]"
        onKeyDown={(e) => { if (e.key === 'Enter') submit(); if (e.key === 'Escape') onCancel() }} />
      <Input aria-label="Title" value={title} onChange={(e) => setTitle(e.target.value)}
        placeholder="Title (optional)" className="h-6 text-[11px]"
        onKeyDown={(e) => { if (e.key === 'Enter') submit(); if (e.key === 'Escape') onCancel() }} />
      <div className="flex gap-1.5">
        <button type="button" onClick={submit}
          className="flex-1 h-7 rounded text-[12px] font-medium bg-primary/15 text-primary hover:bg-primary/25 transition-colors">Add link</button>
        {tabs.length > 0 && (
          <button type="button" onClick={() => setShowTabs(true)}
            className="h-7 px-2.5 rounded text-[11px] text-muted-foreground hover:bg-muted/50 transition-colors flex items-center gap-1">
            <LayoutList size={10} /> From tabs
          </button>
        )}
        <button type="button" onClick={onCancel}
          className="h-7 w-7 rounded flex items-center justify-center text-muted-foreground hover:bg-muted/50"><X size={11} /></button>
      </div>
    </div>
  )
}

// ── Editable link card ────────────────────────────────────────────────────────

function LinkCard({
  link, section, sections, isDragging, isDropTarget,
  onDragStart, onDragOver, onDragLeave, onDrop,
  onEdit, onRemove, onMoveSection,
}: {
  link: Link; section: string | null; sections: string[]
  isDragging: boolean; isDropTarget: boolean
  onDragStart: () => void
  onDragOver: (e: React.DragEvent) => void
  onDragLeave: () => void
  onDrop: (e: React.DragEvent) => void
  onEdit: (lid: string, title: string, url: string, note: string) => void
  onRemove: (lid: string) => void
  onMoveSection: (lid: string, s: string | null) => void
}): JSX.Element {
  const [editing, setEditing] = useState(false)
  const [showPicker, setShowPicker] = useState(false)
  const [confirmDel, setConfirmDel] = useState(false)

  if (editing) return (
    <LinkEditForm link={link}
      onSave={(t, u, n) => { onEdit(link.id, t, u, n); setEditing(false) }}
      onCancel={() => setEditing(false)} />
  )

  if (confirmDel) return (
    <div className="flex items-center gap-3 px-4 py-2.5 bg-destructive/5 border-b border-border/20">
      <Favicon url={link.url} favicon={link.favicon} className="w-4 h-4 opacity-40 shrink-0" />
      <span className="flex-1 text-[11px] text-muted-foreground truncate">Delete &ldquo;{link.title}&rdquo;?</span>
      <button type="button" onClick={() => onRemove(link.id)}
        className="h-5 px-2 rounded text-[11px] bg-destructive/80 text-destructive-foreground hover:bg-destructive shrink-0">Delete</button>
      <button type="button" onClick={() => setConfirmDel(false)}
        className="h-5 px-2 rounded text-[11px] text-muted-foreground hover:bg-muted/50 shrink-0">Cancel</button>
    </div>
  )

  return (
    <li
      draggable
      onDragStart={(e) => { e.dataTransfer.effectAllowed = 'move'; onDragStart() }}
      onDragOver={onDragOver} onDragLeave={onDragLeave} onDrop={onDrop}
      className={cn('relative group/link transition-colors hover:bg-muted/20', isDragging && 'opacity-30')}
    >
      {isDropTarget && <div className="absolute top-0 inset-x-0 h-0.5 bg-primary z-10 pointer-events-none" />}
      <div className="flex items-center gap-3 pl-3 pr-4 py-3">
        <GripVertical size={14} className="text-muted-foreground/20 group-hover/link:text-muted-foreground/50 cursor-grab shrink-0 transition-colors" />
        <Favicon url={link.url} favicon={link.favicon} className="w-[18px] h-[18px] shrink-0" />
        <button type="button" onClick={() => void window.aether.tabs.create(link.url)}
          className="flex-1 min-w-0 text-left">
          <p className="text-[12px] font-medium text-foreground leading-snug truncate">{link.title || link.url}</p>
          {link.note
            ? <p className="text-[11px] text-muted-foreground/70 mt-0.5 leading-relaxed line-clamp-2">{link.note}</p>
            : <p className="text-[10px] text-muted-foreground/40 truncate">{link.url}</p>
          }
        </button>
        <div className="flex items-center gap-0.5 opacity-0 group-hover/link:opacity-100 transition-opacity shrink-0" onClick={(e) => e.stopPropagation()}>
          {sections.length > 0 && (
            <div className="relative">
              <Tooltip label="Move to section">
                <Button size="icon" variant="ghost" className="h-6 w-6 text-muted-foreground/50 hover:text-muted-foreground"
                  onClick={() => setShowPicker((v) => !v)}>
                  <LayoutList size={11} />
                </Button>
              </Tooltip>
              {showPicker && (
                <>
                  <div className="fixed inset-0 z-20" onClick={() => setShowPicker(false)} />
                  <div className="absolute right-0 top-full mt-1 z-30 w-40 rounded-xl border border-border bg-popover shadow-xl py-1">
                    {[{ label: 'Unsorted', value: null }, ...sections.map((s) => ({ label: s, value: s }))].map((opt) => (
                      <button key={opt.value ?? '__u'} type="button"
                        className={cn('w-full flex items-center gap-2 px-3 py-1.5 text-[11px] text-left hover:bg-muted/60 transition-colors',
                          opt.value === section ? 'text-primary font-medium' : 'text-foreground')}
                        onClick={() => { onMoveSection(link.id, opt.value); setShowPicker(false) }}>
                        {opt.value === section && <Check size={9} className="shrink-0" />}
                        {opt.label}
                      </button>
                    ))}
                  </div>
                </>
              )}
            </div>
          )}
          <Tooltip label="Edit">
            <Button size="icon" variant="ghost" className="h-6 w-6 text-muted-foreground/50 hover:text-muted-foreground"
              onClick={() => setEditing(true)}>
              <Pencil size={11} />
            </Button>
          </Tooltip>
          <Tooltip label="Remove">
            <Button size="icon" variant="ghost" className="h-6 w-6 text-muted-foreground/50 hover:text-destructive"
              onClick={() => setConfirmDel(true)}>
              <Trash2 size={11} />
            </Button>
          </Tooltip>
        </div>
      </div>
    </li>
  )
}

// ── URL input popover ─────────────────────────────────────────────────────────

function UrlPopover({
  label, current, onSave, onClear, onClose,
}: {
  label: string; current?: string
  onSave: (url: string) => void; onClear: () => void; onClose: () => void
}): JSX.Element {
  const [val, setVal] = useState(current ?? '')
  return (
    <div className="absolute left-1/2 -translate-x-1/2 top-full mt-2 z-30 w-64 rounded-xl border border-border bg-popover shadow-xl p-3 flex flex-col gap-2">
      <p className="text-[11px] font-medium text-foreground">{label}</p>
      <Input autoFocus value={val} onChange={(e) => setVal(e.target.value)} placeholder="https://…"
        className="h-7 text-[11px]"
        onKeyDown={(e) => { if (e.key === 'Enter') { onSave(val); onClose() } if (e.key === 'Escape') onClose() }} />
      <div className="flex gap-1.5">
        <button type="button" onClick={() => { onSave(val); onClose() }}
          className="flex-1 h-6 rounded text-[11px] bg-primary/15 text-primary hover:bg-primary/25 transition-colors">Save</button>
        {current && (
          <button type="button" onClick={() => { onClear(); onClose() }}
            className="h-6 px-2 rounded text-[11px] text-destructive/80 hover:bg-destructive/10 transition-colors">Remove</button>
        )}
        <button type="button" onClick={onClose}
          className="h-6 px-2 rounded text-[11px] text-muted-foreground hover:bg-muted/50 transition-colors">Cancel</button>
      </div>
    </div>
  )
}

// ── Main component ────────────────────────────────────────────────────────────

export function CollectionEditor({
  collection, tabs,
  onAddLink, onEditLink, onRemoveLink, onReorderLinks,
  onSetSections, onRenameSection, onDeleteSection, onMoveLink,
  onSetName, onSetDescription, onSetIconUrl, onSetBannerUrl, onSetAuthor,
  onExport,
}: CollectionEditorProps): JSX.Element {

  const [editingName, setEditingName] = useState(false)
  const [nameDraft, setNameDraft] = useState(collection.name)
  const [descDraft, setDescDraft] = useState(collection.description ?? '')
  const [editingBanner, setEditingBanner] = useState(false)
  const [editingIcon, setEditingIcon] = useState(false)

  const [renamingSection, setRenamingSection] = useState<string | null>(null)
  const [renameDraft, setRenameDraft] = useState('')
  const [confirmDeleteSection, setConfirmDeleteSection] = useState<string | null>(null)
  const [addingToSection, setAddingToSection] = useState<string | null>(null)
  const [showNewSectionForm, setShowNewSectionForm] = useState(false)
  const [newSectionName, setNewSectionName] = useState('')

  const [flatDraggedId, setFlatDraggedId] = useState<string | null>(null)
  const [flatOverId, setFlatOverId] = useState<string | null>(null)
  const [draggedLinkId, setDraggedLinkId] = useState<string | null>(null)
  const [dropTarget, setDropTarget] = useState<DropTarget | null>(null)

  const nameInputRef = useRef<HTMLInputElement>(null)

  const accentColor = collection.author?.color ?? '#6366f1'
  const sections = useMemo(() => collection.sections ?? [], [collection.sections])
  const sectionsActive = collection.sections !== undefined
  const authorLinks: CreatorLink[] = useMemo(() => collection.author?.links ?? [], [collection.author?.links])

  const sortedLinks = useMemo(() => [...collection.links].sort((a, b) => a.order - b.order), [collection.links])
  const { bySection, unsorted } = useMemo(() => {
    const map = new Map<string, Link[]>()
    for (const s of sections) map.set(s, [])
    const uns: Link[] = []
    for (const l of sortedLinks) {
      if (l.section && map.has(l.section)) map.get(l.section)!.push(l)
      else uns.push(l)
    }
    return { bySection: map, unsorted: uns }
  }, [sections, sortedLinks])

  const addAuthorLink = (platform: CreatorPlatform, url: string): void => {
    const current = collection.author ?? { handle: '' }
    const newLinks = [...(current.links ?? []).filter((l) => l.platform !== platform), { platform, url }]
    onSetAuthor({ ...current, links: newLinks })
  }

  const removeAuthorLink = (platform: CreatorPlatform): void => {
    const current = collection.author
    if (!current) return
    const newLinks = (current.links ?? []).filter((l) => l.platform !== platform)
    onSetAuthor({ ...current, links: newLinks })
  }

  const resetDrag = (): void => { setDraggedLinkId(null); setDropTarget(null) }
  const handleDrop = (): void => {
    if (!draggedLinkId || !dropTarget) { resetDrag(); return }
    if (dropTarget.kind === 'before') onMoveLink(draggedLinkId, dropTarget.section, dropTarget.linkId)
    else onMoveLink(draggedLinkId, dropTarget.section, null)
    resetDrag()
  }

  const linkDragHandlers = (link: Link, renderedSection: string | null) => ({
    onDragStart: () => setDraggedLinkId(link.id),
    onDragOver: (e: React.DragEvent) => {
      e.preventDefault(); e.stopPropagation()
      if (draggedLinkId && draggedLinkId !== link.id)
        setDropTarget({ kind: 'before', linkId: link.id, section: renderedSection })
    },
    onDragLeave: () => setDropTarget(null),
    onDrop: (e: React.DragEvent) => { e.preventDefault(); e.stopPropagation(); handleDrop() },
  })

  const headerDragHandlers = (section: string | null): React.HTMLAttributes<HTMLDivElement> => ({
    onDragOver: (e) => { e.preventDefault(); if (draggedLinkId) setDropTarget({ kind: 'append', section }) },
    onDragLeave: () => setDropTarget(null),
    onDrop: (e) => { e.preventDefault(); handleDrop() },
  })

  const commitRename = (): void => {
    const draft = renameDraft.trim()
    if (draft && renamingSection) onRenameSection(renamingSection, draft)
    setRenamingSection(null); setRenameDraft('')
  }

  const commitNewSection = (): void => {
    const name = newSectionName.trim(); if (!name) return
    onSetSections([...sections, name])
    setNewSectionName(''); setShowNewSectionForm(false)
  }

  return (
    <div className="flex flex-col w-full" onDragEnd={resetDrag}>

      {/* ── Hero: banner + icon ───────────────────────────────────────────── */}
      <div className="relative shrink-0">
        <div className="relative h-24 overflow-hidden cursor-pointer group/banner"
          onClick={() => setEditingBanner(true)}>
          {collection.bannerUrl
            ? <img src={collection.bannerUrl} alt="" className="w-full h-full object-cover"
                onError={(e) => { e.currentTarget.style.display = 'none' }} />
            : <div className="w-full h-full"
                style={{ background: `linear-gradient(135deg, ${accentColor}50 0%, ${accentColor}28 60%, ${accentColor}08 100%)` }} />
          }
          <div className="absolute inset-0 flex items-center justify-center opacity-0 group-hover/banner:opacity-100 bg-black/25 transition-opacity">
            <span className="flex items-center gap-1.5 text-[11px] font-medium text-white bg-black/40 px-3 py-1.5 rounded-full backdrop-blur-sm">
              <Pencil size={11} /> Edit banner
            </span>
          </div>
          {onExport && (
            <button type="button" onClick={(e) => { e.stopPropagation(); onExport() }}
              aria-label="Export collection"
              className="absolute top-2 right-2 flex items-center gap-1 h-6 px-2 rounded-lg text-[11px] font-medium text-white/80 bg-black/30 hover:bg-black/50 backdrop-blur-sm transition-colors">
              <Download size={11} /> Export
            </button>
          )}
          {editingBanner && (
            <div onClick={(e) => e.stopPropagation()}>
              <UrlPopover label="Banner image URL"
                current={collection.bannerUrl}
                onSave={(u) => onSetBannerUrl(u || null)}
                onClear={() => onSetBannerUrl(null)}
                onClose={() => setEditingBanner(false)}
              />
            </div>
          )}
        </div>

        <div className="absolute -bottom-6 left-4 z-10">
          <div className="relative group/icon cursor-pointer" onClick={() => setEditingIcon(true)}>
            {collection.iconUrl
              ? <img src={collection.iconUrl} alt={collection.name}
                  className="h-14 w-14 rounded-2xl object-contain ring-4 ring-background shadow-lg"
                  onError={(e) => { e.currentTarget.style.display = 'none' }} />
              : <div className="h-14 w-14 rounded-2xl ring-4 ring-background shadow-lg flex items-center justify-center text-2xl font-bold text-white/80"
                  style={{ background: `linear-gradient(135deg, ${accentColor}70, ${accentColor}40)` }}>
                  {collection.name.charAt(0).toUpperCase()}
                </div>
            }
            <div className="absolute inset-0 rounded-2xl bg-black/40 opacity-0 group-hover/icon:opacity-100 flex items-center justify-center transition-opacity">
              <Pencil size={14} className="text-white" />
            </div>
            {editingIcon && (
              <div onClick={(e) => e.stopPropagation()}>
                <UrlPopover label="Icon URL"
                  current={collection.iconUrl}
                  onSave={(u) => onSetIconUrl(u || null)}
                  onClear={() => onSetIconUrl(null)}
                  onClose={() => setEditingIcon(false)}
                />
              </div>
            )}
          </div>
        </div>
      </div>

      {/* ── Identity ─────────────────────────────────────────────────────── */}
      <div className="pt-9 pb-3 px-4 border-b border-border/40">
        {editingName
          ? <div className="flex items-center gap-1.5 mb-1">
              <input ref={nameInputRef} autoFocus value={nameDraft}
                onChange={(e) => setNameDraft(e.target.value)}
                className="flex-1 bg-transparent text-[17px] font-bold text-foreground outline-none border-b border-primary focus:border-primary/80"
                onKeyDown={(e) => {
                  if (e.key === 'Enter') { if (nameDraft.trim()) onSetName(nameDraft.trim()); setEditingName(false) }
                  if (e.key === 'Escape') setEditingName(false)
                }}
                onBlur={() => { if (nameDraft.trim()) onSetName(nameDraft.trim()); setEditingName(false) }}
              />
            </div>
          : <div className="flex items-center gap-2 group/name mb-0.5">
              <h2 className="text-[17px] font-bold text-foreground leading-tight">{collection.name}</h2>
              <button type="button" onClick={() => { setNameDraft(collection.name); setEditingName(true) }}
                className="opacity-0 group-hover/name:opacity-100 transition-opacity text-muted-foreground hover:text-foreground">
                <Pencil size={12} />
              </button>
            </div>
        }
        {collection.author?.handle && (
          <p className="text-[12px] font-medium mb-2" style={{ color: accentColor }}>
            @{collection.author.handle}
          </p>
        )}
        <textarea
          aria-label="Description"
          value={descDraft}
          onChange={(e) => setDescDraft(e.target.value)}
          onBlur={() => onSetDescription(descDraft.trim() || null)}
          placeholder="Add a description for this collection…"
          rows={2}
          className="w-full bg-transparent text-[12px] text-foreground placeholder:text-muted-foreground/40 focus:outline-none resize-none leading-relaxed"
        />
      </div>

      {/* ── Creator links ─────────────────────────────────────────────────── */}
      <div className="px-4 py-3 border-b border-border/40">
        <p className="text-[10px] uppercase tracking-[0.1em] font-semibold text-muted-foreground mb-2">Links</p>
        <CreatorLinks links={authorLinks} onAdd={addAuthorLink} onRemove={removeAuthorLink} />
      </div>

      {/* ── Links (flat or sectioned) ─────────────────────────────────────── */}
      <div className="flex-1 flex flex-col py-2">

        {!sectionsActive && (
          <>
            <ul role="list">
              {sortedLinks.map((l) => {
                const isOver = flatOverId === l.id && flatDraggedId !== l.id
                return (
                  <li key={l.id}
                    draggable
                    className={cn('relative', isOver && 'border-t-2 border-primary')}
                    onDragStart={(e) => { e.dataTransfer.effectAllowed = 'move'; setFlatDraggedId(l.id) }}
                    onDragOver={(e) => { e.preventDefault(); if (flatDraggedId && flatDraggedId !== l.id) setFlatOverId(l.id) }}
                    onDragLeave={() => setFlatOverId(null)}
                    onDrop={(e) => {
                      e.preventDefault()
                      if (flatDraggedId && flatDraggedId !== l.id) {
                        const ids = sortedLinks.map((s) => s.id)
                        const from = ids.indexOf(flatDraggedId), to = ids.indexOf(l.id)
                        if (from !== -1 && to !== -1) {
                          const r = [...ids]; r.splice(from, 1); r.splice(to, 0, flatDraggedId)
                          onReorderLinks(r)
                        }
                      }
                      setFlatDraggedId(null); setFlatOverId(null)
                    }}
                    onDragEnd={() => { setFlatDraggedId(null); setFlatOverId(null) }}
                  >
                    <LinkCard link={l} section={null} sections={[]}
                      isDragging={flatDraggedId === l.id} isDropTarget={false}
                      onDragStart={() => {}} onDragOver={() => {}} onDragLeave={() => {}} onDrop={() => {}}
                      onEdit={onEditLink} onRemove={onRemoveLink} onMoveSection={() => {}} />
                  </li>
                )
              })}
            </ul>

            {sortedLinks.length === 0 && addingToSection !== UNSORTED && (
              <div className="flex flex-col items-center py-8 gap-1.5 text-muted-foreground/40">
                <Globe size={20} />
                <p className="text-[11px]">No links yet</p>
              </div>
            )}

            <div className="border-t border-border/30 mt-1">
              {addingToSection === UNSORTED
                ? <AddLinkForm section={null} tabs={tabs} onAdd={onAddLink} onCancel={() => setAddingToSection(null)} />
                : <div className="flex items-center justify-between px-4 py-2">
                    <button type="button" onClick={() => setAddingToSection(UNSORTED)}
                      className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors">
                      <Plus size={12} /> Add link
                    </button>
                    <div className="flex items-center gap-1.5">
                      <span className="text-[10px] text-muted-foreground/40">Organise with sections</span>
                      <button type="button" onClick={() => onSetSections([...DEFAULT_SECTIONS])}
                        className="h-5 px-2 rounded text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors">Quick setup</button>
                      <button type="button" onClick={() => onSetSections([])}
                        className="h-5 px-2 rounded text-[10px] text-muted-foreground/60 hover:text-muted-foreground hover:bg-muted/40 transition-colors">Blank</button>
                    </div>
                  </div>
              }
            </div>
          </>
        )}

        {sectionsActive && sections.length === 0 && (
          <div className="flex flex-col items-center gap-4 px-6 py-8 text-center">
            <p className="text-[12px] font-semibold text-foreground">Create your first section</p>
            <p className="text-[11px] text-muted-foreground">Quick setup adds <span className="text-foreground">{DEFAULT_SECTIONS.join(', ')}</span></p>
            <div className="flex gap-2">
              <button type="button" onClick={() => onSetSections([...DEFAULT_SECTIONS])}
                className="h-8 px-4 rounded-lg text-[12px] font-medium bg-primary/15 text-primary hover:bg-primary/25 transition-colors">Quick setup</button>
              <button type="button" onClick={() => setShowNewSectionForm(true)}
                className="h-8 px-4 rounded-lg text-[12px] text-muted-foreground hover:bg-muted/50 transition-colors">Create manually</button>
            </div>
            {showNewSectionForm && (
              <div className="flex items-center gap-1.5 w-full max-w-[240px]">
                <Input autoFocus value={newSectionName} onChange={(e) => setNewSectionName(e.target.value)}
                  placeholder="Section name…" className="h-7 text-[11px] flex-1"
                  onKeyDown={(e) => { if (e.key === 'Enter') commitNewSection(); if (e.key === 'Escape') { setShowNewSectionForm(false); setNewSectionName('') } }} />
                <button type="button" onClick={commitNewSection}
                  className="h-7 px-2.5 rounded text-[11px] bg-primary/15 text-primary hover:bg-primary/25 shrink-0">Add</button>
              </div>
            )}
            {unsorted.length > 0 && (
              <ul role="list" className="w-full text-left border border-border/40 rounded-xl overflow-hidden mt-2">
                {unsorted.map((l) => (
                  <LinkCard key={l.id} link={l} section={null} sections={[]}
                    isDragging={false} isDropTarget={false}
                    onDragStart={() => {}} onDragOver={() => {}} onDragLeave={() => {}} onDrop={() => {}}
                    onEdit={onEditLink} onRemove={onRemoveLink} onMoveSection={() => {}} />
                ))}
              </ul>
            )}
          </div>
        )}

        {sectionsActive && sections.length > 0 && (
          <>
            {sections.map((sectionName) => {
              const links = bySection.get(sectionName) ?? []
              const isAppend = dropTarget?.kind === 'append' && dropTarget.section === sectionName

              return (
                <div key={sectionName} className="mb-4 px-4">
                  <div {...headerDragHandlers(sectionName)}
                    className={cn('flex items-center gap-2 group/section py-2 transition-colors rounded-lg px-1',
                      isAppend ? 'bg-primary/8' : '')}>
                    {renamingSection === sectionName ? (
                      <div className="flex items-center gap-1.5 flex-1">
                        <Input autoFocus value={renameDraft} onChange={(e) => setRenameDraft(e.target.value)}
                          className="h-6 text-[11px] flex-1"
                          onKeyDown={(e) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') { setRenamingSection(null); setRenameDraft('') } }} />
                        <button type="button" onClick={commitRename}
                          className="h-6 px-2 rounded text-[11px] bg-primary/15 text-primary hover:bg-primary/25 shrink-0">Save</button>
                        <button type="button" onClick={() => { setRenamingSection(null); setRenameDraft('') }}
                          className="h-6 w-6 rounded flex items-center justify-center text-muted-foreground hover:bg-muted/50 shrink-0"><X size={10} /></button>
                      </div>
                    ) : confirmDeleteSection === sectionName ? (
                      <div className="flex items-center gap-2 flex-1 flex-wrap">
                        <span className="text-[11px] text-destructive flex-1">Delete &ldquo;{sectionName}&rdquo;?</span>
                        <button type="button" onClick={() => { onDeleteSection(sectionName); setConfirmDeleteSection(null) }}
                          className="h-5 px-2 rounded text-[11px] bg-destructive/15 text-destructive hover:bg-destructive/25 shrink-0">Delete</button>
                        <button type="button" onClick={() => setConfirmDeleteSection(null)}
                          className="h-5 px-2 rounded text-[11px] text-muted-foreground hover:bg-muted/50 shrink-0">Cancel</button>
                      </div>
                    ) : (
                      <>
                        <h3 className="text-[10px] uppercase tracking-[0.12em] font-bold flex-1 select-none"
                          style={{ color: `${accentColor}cc` }}>
                          {sectionName}
                          {links.length > 0 && <span className="ml-1.5 opacity-50 font-normal">· {links.length}</span>}
                        </h3>
                        {isAppend && <span className="text-[9px] text-primary font-medium">Drop here</span>}
                        <div className="flex items-center gap-0.5 opacity-0 group-hover/section:opacity-100 transition-opacity">
                          <Tooltip label="Add link">
                            <Button size="icon" variant="ghost" className="h-5 w-5 text-muted-foreground/50 hover:text-muted-foreground"
                              onClick={() => setAddingToSection(sectionName)}>
                              <Plus size={10} />
                            </Button>
                          </Tooltip>
                          <Tooltip label="Rename">
                            <Button size="icon" variant="ghost" className="h-5 w-5 text-muted-foreground/50 hover:text-muted-foreground"
                              onClick={() => { setRenamingSection(sectionName); setRenameDraft(sectionName) }}>
                              <Pencil size={9} />
                            </Button>
                          </Tooltip>
                          <Tooltip label="Delete section">
                            <Button size="icon" variant="ghost" className="h-5 w-5 text-muted-foreground/50 hover:text-destructive"
                              onClick={() => setConfirmDeleteSection(sectionName)}>
                              <Trash2 size={9} />
                            </Button>
                          </Tooltip>
                        </div>
                      </>
                    )}
                  </div>

                  <div className="rounded-xl border border-border/40 overflow-hidden bg-card/40">
                    <ul role="list">
                      {links.map((l) => (
                        <LinkCard key={l.id} link={l} section={sectionName} sections={sections}
                          isDragging={draggedLinkId === l.id}
                          isDropTarget={dropTarget?.kind === 'before' && dropTarget.linkId === l.id}
                          {...linkDragHandlers(l, sectionName)}
                          onEdit={onEditLink} onRemove={onRemoveLink}
                          onMoveSection={(lid, s) => onMoveLink(lid, s, null)} />
                      ))}
                    </ul>
                    {addingToSection === sectionName && (
                      <AddLinkForm section={sectionName} tabs={tabs} onAdd={onAddLink} onCancel={() => setAddingToSection(null)} />
                    )}
                    {links.length === 0 && addingToSection !== sectionName && (
                      <div className="px-4 py-3 text-[11px] text-muted-foreground/40 italic flex items-center gap-2">
                        <Globe size={13} /> Drag links here or click +
                      </div>
                    )}
                  </div>

                  {addingToSection !== sectionName && (
                    <button type="button" onClick={() => setAddingToSection(sectionName)}
                      className="flex items-center gap-1.5 mt-2 text-[11px] text-muted-foreground/50 hover:text-muted-foreground transition-colors px-1">
                      <Plus size={11} /> Add link
                    </button>
                  )}
                </div>
              )
            })}

            {unsorted.length > 0 && (
              <div className="mb-4 px-4">
                <div {...headerDragHandlers(null)}
                  className={cn('flex items-center gap-2 py-2 px-1 rounded-lg transition-colors',
                    dropTarget?.kind === 'append' && dropTarget.section === null ? 'bg-primary/8' : '')}>
                  <h3 className="text-[10px] uppercase tracking-[0.12em] font-bold text-muted-foreground/50 flex-1 select-none">
                    Unsorted · {unsorted.length}
                  </h3>
                  {dropTarget?.kind === 'append' && dropTarget.section === null && (
                    <span className="text-[9px] text-primary font-medium">Drop here</span>
                  )}
                </div>
                <div className="rounded-xl border border-border/40 overflow-hidden bg-card/40">
                  <ul role="list">
                    {unsorted.map((l) => (
                      <LinkCard key={l.id} link={l} section={null} sections={sections}
                        isDragging={draggedLinkId === l.id}
                        isDropTarget={dropTarget?.kind === 'before' && dropTarget.linkId === l.id}
                        {...linkDragHandlers(l, null)}
                        onEdit={onEditLink} onRemove={onRemoveLink}
                        onMoveSection={(lid, s) => onMoveLink(lid, s, null)} />
                    ))}
                  </ul>
                </div>
              </div>
            )}

            <div className="px-4 pb-4">
              {showNewSectionForm ? (
                <div className="flex items-center gap-1.5">
                  <Input autoFocus value={newSectionName} onChange={(e) => setNewSectionName(e.target.value)}
                    placeholder="Section name…" className="h-7 text-[11px] flex-1"
                    onKeyDown={(e) => { if (e.key === 'Enter') commitNewSection(); if (e.key === 'Escape') { setShowNewSectionForm(false); setNewSectionName('') } }} />
                  <button type="button" onClick={commitNewSection}
                    className="h-7 px-2.5 rounded text-[11px] bg-primary/15 text-primary hover:bg-primary/25 shrink-0">Add</button>
                  <button type="button" onClick={() => { setShowNewSectionForm(false); setNewSectionName('') }}
                    className="h-7 w-7 rounded flex items-center justify-center text-muted-foreground hover:bg-muted/50 shrink-0"><X size={11} /></button>
                </div>
              ) : (
                <button type="button" onClick={() => setShowNewSectionForm(true)}
                  className="flex items-center gap-1.5 text-[11px] text-muted-foreground/50 hover:text-muted-foreground transition-colors">
                  <FolderPlus size={12} /> New section
                </button>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  )
}
