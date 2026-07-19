// One-shot measurement harness for the deep-hide fix (perf/idle-memory).
// Launches the built app exactly like smoke.mjs, then walks the scenario:
// boot → FOCUSED baseline → hide → past the 30 s deep-hide grace → measure →
// show again (regression check) → second hide cycle → measure. Prints a table.
// Not part of any pipeline — run manually: node scripts/measure-idle.mjs

import electronPath from 'electron'
import { spawn, execSync } from 'node:child_process'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const BASE = 'http://127.0.0.1:9119'
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const rows = []

const childEnv = { ...process.env, NODE_ENV: 'development' }
delete childEnv.ELECTRON_RUN_AS_NODE
delete childEnv.ELECTRON_NO_ATTACH_CONSOLE
const child = spawn(electronPath, [join(root, 'out', 'main', 'index.js')], {
  cwd: root, env: childEnv, stdio: 'ignore',
})

const getJson = async (p) => (await fetch(BASE + p)).json()
const note = async (label) => {
  const m = await getJson('/metrics')
  const s = await getJson('/state')
  rows.push({ label, overlay: s.overlay, appMb: m.memory.totalMb })
  console.log(`[measure] ${label}: overlay=${s.overlay} appMb=${m.memory.totalMb}`)
}

try {
  let up = false
  for (let i = 0; i < 60; i++) {
    try { if ((await fetch(BASE + '/ping')).ok) { up = true; break } } catch { /* not yet */ }
    await sleep(500)
  }
  if (!up) throw new Error('app did not start')

  await sleep(20_000)
  await note('boot+20s (FOCUSED baseline)')

  await fetch(BASE + '/overlay/hide')
  await sleep(5_000)
  await note('hidden+5s (opacity phase, before deep hide)')
  await sleep(40_000)
  await note('hidden+45s (deep hide fired at 30s)')
  await sleep(60_000)
  await note('hidden+105s (settled)')

  await fetch(BASE + '/overlay/show')
  await sleep(3_000)
  await note('re-shown+3s (regression check)')

  await fetch(BASE + '/overlay/hide')
  await sleep(45_000)
  await note('cycle2 hidden+45s')

  console.log('\nRESULTS')
  for (const r of rows) console.log(`${r.label}\t${r.overlay}\t${r.appMb} MB`)
} catch (err) {
  console.error('[measure] FAILED:', err.message)
  process.exitCode = 1
} finally {
  try { execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: 'ignore' }) } catch { /* gone */ }
}
