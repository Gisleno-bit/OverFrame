import { randomUUID } from 'node:crypto'
import { store } from '../store'
import type {
  Collection,
  CollectionAuthor,
  CollectionExport,
  CollectionSource,
  Link,
  NewCollection,
  NewLink
} from '@shared/types'
import { MAX_PINNED_LINKS } from '@shared/types'

// ── Sanitization ────────────────────────────────────────────────────────────────
// Shared by create() and decode(). Import payloads come from untrusted sources
// (Discord/Reddit/landing pages), so everything is validated and length-capped.

const MAX_HANDLE_LEN = 30
const MAX_DESCRIPTION_LEN = 280
const MAX_NOTE_LEN = 500
const MAX_NAME_LEN = 200
const MAX_TITLE_LEN = 500
const MAX_ICON_URL_LEN = 65536
const HEX_COLOR_RE = /^#[0-9a-fA-F]{6}$/

/** Strip control chars, collapse whitespace, trim, cap length. Returns undefined when empty. */
function sanitizeText(raw: unknown, max: number): string | undefined {
  if (typeof raw !== 'string') return undefined
  let out = ''
  for (const ch of raw) {
    // Replace C0 control chars + DEL with a space (collapsed below); keep everything else.
    const code = ch.charCodeAt(0)
    out += code < 0x20 || code === 0x7f ? ' ' : ch
  }
  const cleaned = out.replace(/\s+/g, ' ').trim().slice(0, max)
  return cleaned.length > 0 ? cleaned : undefined
}

/** Accept only a #rrggbb hex colour — prevents CSS injection via the accent colour. */
function sanitizeColor(raw: unknown): string | undefined {
  return typeof raw === 'string' && HEX_COLOR_RE.test(raw) ? raw.toLowerCase() : undefined
}

/** A creator signature is valid only with a non-empty handle; the colour is optional. */
function sanitizeAuthor(raw: unknown): CollectionAuthor | undefined {
  if (!raw || typeof raw !== 'object') return undefined
  const handle = sanitizeText((raw as { handle?: unknown }).handle, MAX_HANDLE_LEN)
  if (!handle) return undefined
  const color = sanitizeColor((raw as { color?: unknown }).color)
  return color ? { handle, color } : { handle }
}

/** Keep only http(s) URLs (used for favicons). */
function sanitizeHttpUrl(raw: unknown): string | undefined {
  if (typeof raw !== 'string') return undefined
  try {
    const proto = new URL(raw).protocol
    return proto === 'http:' || proto === 'https:' ? raw : undefined
  } catch {
    return undefined
  }
}

/** Accept a data:image URL or an http(s) URL within the size cap; reject anything else. */
function sanitizeIconUrl(raw: unknown): string | undefined {
  if (typeof raw === 'string' && raw.length > 0 && raw.length <= MAX_ICON_URL_LEN) {
    if (/^data:image\/[a-z+.-]+;base64,/.test(raw)) return raw
    try {
      const proto = new URL(raw).protocol
      if (proto === 'http:' || proto === 'https:') return raw
    } catch {
      /* ignore */
    }
  }
  return undefined
}

export class CollectionsManager {
  getAll(): Collection[] {
    return store.get('collections')
  }

  getById(id: string): Collection | undefined {
    return this.getAll().find((c) => c.id === id)
  }

  getForProfile(profileId: string): Collection[] {
    return this.getAll().filter(
      (c) => c.profileId === profileId || c.profileId === 'shared'
    )
  }

  /** All pinned links across all collections visible in the active profile. */
  getPinnedForProfile(profileId: string): Array<{ collectionId: string; link: Link }> {
    const visible = this.getForProfile(profileId)
    const out: Array<{ collectionId: string; link: Link }> = []
    for (const c of visible) {
      for (const l of c.links) {
        if (l.pinned) out.push({ collectionId: c.id, link: l })
      }
    }
    return out.slice(0, MAX_PINNED_LINKS)
  }

  create(input: NewCollection): Collection {
    const now = Date.now()
    const description = sanitizeText(input.description, MAX_DESCRIPTION_LEN)
    const author = sanitizeAuthor(input.author)
    const collection: Collection = {
      id: randomUUID(),
      name: input.name,
      profileId: input.profileId,
      source: input.source ?? 'user',
      ...(input.iconUrl ? { iconUrl: input.iconUrl } : {}),
      ...(description ? { description } : {}),
      ...(author ? { author } : {}),
      links: [],
      createdAt: now,
      updatedAt: now
    }
    this.persist([...this.getAll(), collection])
    return collection
  }

  remove(id: string): void {
    this.persist(this.getAll().filter((c) => c.id !== id))
  }

  rename(id: string, name: string): Collection | null {
    return this.mutate(id, (c) => ({ ...c, name }))
  }

  setDescription(id: string, description: string | null): Collection | null {
    return this.mutate(id, (c) => {
      const updated = { ...c }
      const clean = description ? sanitizeText(description, MAX_DESCRIPTION_LEN) : undefined
      if (clean) updated.description = clean
      else delete updated.description
      return updated
    })
  }

  /** Stamp (or clear) the creator signature that travels with the collection when shared. */
  setAuthor(id: string, author: CollectionAuthor | null): Collection | null {
    return this.mutate(id, (c) => {
      const updated = { ...c }
      const clean = author ? sanitizeAuthor(author) : undefined
      if (clean) updated.author = clean
      else delete updated.author
      return updated
    })
  }

  setIconUrl(id: string, iconUrl: string | null): Collection | null {
    return this.mutate(id, (c) => {
      const updated = { ...c }
      if (iconUrl) updated.iconUrl = iconUrl
      else delete updated.iconUrl
      return updated
    })
  }

  addLink(collectionId: string, input: NewLink): Collection | null {
    return this.mutate(collectionId, (c) => {
      const link: Link = {
        id: randomUUID(),
        title: input.title,
        url: input.url,
        note: input.note,
        favicon: input.favicon,
        pinned: input.pinned ?? false,
        order: c.links.length
      }
      return { ...c, links: [...c.links, link] }
    })
  }

  removeLink(collectionId: string, linkId: string): Collection | null {
    return this.mutate(collectionId, (c) => ({
      ...c,
      links: c.links
        .filter((l) => l.id !== linkId)
        .map((l, i) => ({ ...l, order: i }))
    }))
  }

  updateLink(
    collectionId: string,
    linkId: string,
    patch: Partial<Pick<Link, 'title' | 'url' | 'note' | 'pinned' | 'favicon' | 'order'>>
  ): Collection | null {
    return this.mutate(collectionId, (c) => ({
      ...c,
      links: c.links.map((l) => (l.id === linkId ? { ...l, ...patch } : l))
    }))
  }

  togglePin(collectionId: string, linkId: string): Collection | null {
    return this.mutate(collectionId, (c) => ({
      ...c,
      links: c.links.map((l) => (l.id === linkId ? { ...l, pinned: !l.pinned } : l))
    }))
  }

  /** Export a collection as a Base64-encoded JSON string. */
  export(id: string): string | null {
    const c = this.getById(id)
    if (!c) return null
    const payload: CollectionExport = {
      version: 1,
      name: c.name,
      source: c.source,
      ...(c.description ? { description: c.description } : {}),
      ...(c.author ? { author: c.author } : {}),
      ...(c.iconUrl ? { iconUrl: c.iconUrl } : {}),
      links: c.links.map((l) => ({
        title: l.title,
        url: l.url,
        note: l.note,
        pinned: l.pinned,
        // Only export http/https favicons — data: URLs are non-portable and large
        ...(l.favicon && /^https?:\/\//.test(l.favicon) ? { favicon: l.favicon } : {})
      }))
    }
    return Buffer.from(JSON.stringify(payload), 'utf8').toString('base64')
  }

  /**
   * Decode + validate + sanitize a Base64 export payload, WITHOUT persisting.
   * Returns the cleaned CollectionExport, or null when the payload is invalid.
   * Shared by previewImport() (preview UI) and import() (persist).
   */
  private decode(base64: string): CollectionExport | null {
    let json: string
    try {
      json = Buffer.from(base64, 'base64').toString('utf8')
    } catch {
      return null
    }
    let parsed: CollectionExport
    try {
      parsed = JSON.parse(json) as CollectionExport
    } catch {
      return null
    }
    if (parsed.version !== 1 || !Array.isArray(parsed.links)) return null

    // Whitelist source field to prevent arbitrary strings from persisting in the store
    const VALID_SOURCES: CollectionSource[] = ['user', 'publisher', 'community']
    const source: CollectionSource = VALID_SOURCES.includes(parsed.source as CollectionSource)
      ? (parsed.source as CollectionSource)
      : 'user'

    const iconUrl = sanitizeIconUrl(parsed.iconUrl)
    const description = sanitizeText(parsed.description, MAX_DESCRIPTION_LEN)
    const author = sanitizeAuthor(parsed.author)

    const links = parsed.links
      .map((l) => {
        const url = String(l.url ?? '')
        // Reject any non-http(s) URL to prevent javascript:/file:// injection
        try {
          const proto = new URL(url).protocol
          if (proto !== 'http:' && proto !== 'https:') return null
        } catch {
          return null
        }
        const favicon = sanitizeHttpUrl(l.favicon)
        const link: Pick<Link, 'title' | 'url' | 'note' | 'pinned' | 'favicon'> = {
          title: String(l.title ?? '').slice(0, MAX_TITLE_LEN),
          url,
          note: sanitizeText(l.note, MAX_NOTE_LEN),
          pinned: Boolean(l.pinned),
          ...(favicon ? { favicon } : {})
        }
        return link
      })
      .filter((l): l is NonNullable<typeof l> => l !== null)

    return {
      version: 1,
      name: String(parsed.name ?? '').slice(0, MAX_NAME_LEN),
      source,
      ...(description ? { description } : {}),
      ...(author ? { author } : {}),
      ...(iconUrl ? { iconUrl } : {}),
      links
    }
  }

  /** Decode + sanitize a shared payload for preview, WITHOUT importing it. */
  previewImport(base64: string): CollectionExport | null {
    return this.decode(base64)
  }

  /** Import a Base64 string into a new collection assigned to the given profile. */
  import(base64: string, profileId: string | 'shared'): Collection | null {
    const parsed = this.decode(base64)
    if (!parsed) return null

    const now = Date.now()
    const collection: Collection = {
      id: randomUUID(),
      name: parsed.name,
      profileId,
      source: parsed.source,
      ...(parsed.description ? { description: parsed.description } : {}),
      ...(parsed.author ? { author: parsed.author } : {}),
      ...(parsed.iconUrl ? { iconUrl: parsed.iconUrl } : {}),
      links: parsed.links.map((l, i) => ({
        id: randomUUID(),
        title: l.title,
        url: l.url,
        note: l.note,
        favicon: l.favicon,
        pinned: l.pinned,
        order: i
      })),
      createdAt: now,
      updatedAt: now
    }
    this.persist([...this.getAll(), collection])
    return collection
  }

  // ─────────────────────────────────────────────────────────────────

  /** Reorder links within a collection by providing the ordered array of link IDs. */
  reorderLinks(collectionId: string, linkIds: string[]): Collection | null {
    return this.mutate(collectionId, (c) => {
      const linkMap = new Map(c.links.map((l) => [l.id, l]))
      const reordered = linkIds
        .filter((id) => linkMap.has(id))
        .map((id, i) => ({ ...linkMap.get(id)!, order: i }))
      // Preserve any links not in linkIds (shouldn't happen, but safety)
      const included = new Set(linkIds)
      const extras = c.links
        .filter((l) => !included.has(l.id))
        .map((l, i) => ({ ...l, order: reordered.length + i }))
      return { ...c, links: [...reordered, ...extras] }
    })
  }

  /** Reorder collections for a given profile by providing the ordered array of collection IDs. */
  reorder(collectionIds: string[]): void {
    const all = this.getAll()
    const idxMap = new Map(collectionIds.map((id, i) => [id, i]))
    const reordered = [...all].sort((a, b) => {
      const ia = idxMap.get(a.id) ?? Infinity
      const ib = idxMap.get(b.id) ?? Infinity
      return ia - ib
    })
    this.persist(reordered)
  }

  // ─────────────────────────────────────────────────────────────────

  private mutate(id: string, fn: (c: Collection) => Collection): Collection | null {
    const all = this.getAll()
    const idx = all.findIndex((c) => c.id === id)
    if (idx === -1) return null
    const updated = { ...fn(all[idx]), id: all[idx].id, updatedAt: Date.now() }
    all[idx] = updated
    this.persist(all)
    return updated
  }

  private persist(collections: Collection[]): void {
    store.set('collections', collections)
  }
}
