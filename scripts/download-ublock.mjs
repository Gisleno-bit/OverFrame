/**
 * Download uBlock Origin MV2 (Chromium build) from GitHub releases.
 * Run once: pnpm download:ublock
 * Output: public/extensions/ublock/
 */

import { execSync } from 'node:child_process'
import { existsSync, mkdirSync, rmSync, readdirSync, renameSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const ROOT      = join(__dirname, '..')
const EXT_DIR   = join(ROOT, 'public', 'extensions')
const DEST      = join(EXT_DIR, 'ublock')
const TMP_ZIP   = join(EXT_DIR, 'ublock_tmp.zip')
const TMP_DIR   = join(EXT_DIR, 'ublock_extracted')

// Pin the version — MV2 (1.x) required for WebView2 extension support
const VERSION = '1.71.0'
const ZIP_URL = `https://github.com/gorhill/uBlock/releases/download/${VERSION}/uBlock0_${VERSION}.chromium.zip`

mkdirSync(EXT_DIR, { recursive: true })

console.log(`Downloading uBlock Origin ${VERSION}...`)
execSync(
  `powershell -Command "Invoke-WebRequest -Uri '${ZIP_URL}' -OutFile '${TMP_ZIP}'"`,
  { stdio: 'inherit' }
)

console.log('Extracting...')
if (existsSync(TMP_DIR)) rmSync(TMP_DIR, { recursive: true })
execSync(
  `powershell -Command "Expand-Archive -Path '${TMP_ZIP}' -DestinationPath '${TMP_DIR}' -Force"`,
  { stdio: 'inherit' }
)

// The zip contains a single directory — move its contents to DEST
const entries = readdirSync(TMP_DIR)
const srcDir  = entries.length === 1 ? join(TMP_DIR, entries[0]) : TMP_DIR

if (existsSync(DEST)) rmSync(DEST, { recursive: true })
renameSync(srcDir, DEST)

// Cleanup
if (existsSync(TMP_DIR)) rmSync(TMP_DIR, { recursive: true })
if (existsSync(TMP_ZIP)) rmSync(TMP_ZIP)

console.log(`Done — uBlock Origin ${VERSION} at ${DEST}`)
