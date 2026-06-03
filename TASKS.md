# TASKS — Overframe

> **Pour Claude :** Lis ce fichier en début de session pour savoir où en est le projet et quoi faire ensuite.
> Mets-le à jour en fin de session : déplace les tâches terminées dans "Done", ajuste les priorités.
> Ne modifie pas la section "Contraintes" sans accord explicite.

---

## État actuel du projet

**Version en cours :** v0.6 → v1.0 (Beta Polish → Release)
**Branche active :** `dev` (setup d'autonomie mergé) ; feature en cours sur `feat/oauth-popup` (onglets WebView2)
**Dernière session :** 2026-06-03 — [FEAT] Bascule des onglets sur addon natif **WebView2** (vrai Edge → passe Google/Cloudflare nativement). Zoom/mute/audio/downloads réimplémentés côté natif, garde de navigation, build-from-source (SDK vendored). Pile stealth + OAuth GGG/PoE supprimés. Pipelines vertes (typecheck/lint/test:coverage 100%/build/smoke).

Le cœur du produit est fonctionnel : overlay, tabs, profils, collections, sessions, raccourcis globaux, tray, auto-update, onboarding. L'objectif immédiat est de solidifier pour la release publique v1.0.

---

## En cours

_(vide — à remplir par Claude au début d'une session de travail)_

---

## Priorité haute — Chemin vers v1.0

### Qualité & robustesse
- [ ] **[FIX] Smoke flaky sur `/overlay/show`** — `scripts/smoke.mjs` échoue ~1 run sur 2 (`overlay/show leaves HIDDEN`). Cause probable : le `sleep(500)` unique court contre le polling de détection de jeu / la séquence first-show. Remplacer par un poll-until (retry jusqu'à ~3 s) plutôt qu'un sleep fixe. Observé le 2026-06-01.
- [ ] **[VALID HUMAIN] [FEAT] WebView2 — test réel Google + Cloudflare** — vérifier un vrai login Google et un site Cloudflare-protégé (Turnstile inclus) dans un onglet Overframe. Les onglets sont désormais rendus par Edge WebView2 (vrai navigateur), donc plus de spoofing `navigator.userAgentData` : l'ancien résiduel Electron est levé. Vérifier aussi `pnpm make` (addon packagé en `extraResource`).
- [ ] **[PERF] RAM au boot ~310 MB > budget 300** — détecté par `pnpm smoke` le 2026-06-01 (overlay FOCUSED / welcome au lancement). Lancer le subagent `perf-auditor`, isoler la cause (welcome page ? WebContentsView retenue ?), consigner avant/après. NB : la RAM observée varie fortement run-à-run (122–310 MB) — mesurer plusieurs fois.
- [ ] **[PERF] Audit performance** : `curl http://127.0.0.1:9119/metrics` idle + 3 onglets. Corriger si hors budget (< 150 MB idle, < 300 MB actif). Consigner avant/après chiffrés dans DEVLOG. Guide : `.claude/guides/PERFORMANCE.md`
- [ ] **[FIX] Multi-monitor** : vérifier que la fenêtre se souvient du bon écran après un changement de configuration moniteurs.
- [ ] **[FIX] Gestion d'erreur page load** : affiner l'état "failed to load" dans les WebContentsViews (réseau coupé, SSL invalide). Ajouter test de régression.
- [ ] **[SEC] Valider inputs IPC collections + profiles** : vérifier que chaque handler valide le type et la plage de ses arguments. Guide : `.claude/guides/SECURITY.md`

### UX & polish
- [ ] **Context menu** dans les WebContentsViews : right-click → copier, coller, ouvrir dans un nouvel onglet, inspecter.
- [ ] **Raccourci Ctrl+L** : focus address bar depuis n'importe quel état.
- [ ] **Vérifier l'onboarding flow** : parcourir le OnboardingOverlay complet, valider chaque étape, tester sur une installation fraîche (devStoreReset).

### Release
- [ ] **README** : ajouter captures d'écran + GIF de démonstration (enregistrer l'overlay en action sur un jeu).
- [ ] **Build packagé** : valider `pnpm make` produit un `.exe` installable sans droits admin, SmartScreen bypass documenté.
- [ ] **FAQ README** : SmartScreen workaround, disclaimer anti-cheat, modes jeu supportés (borderless windowed only).

---

## Priorité normale — Backlog post-v1.0

| Priorité | Feature | Version cible |
|---|---|---|
| ★★★ | Ad blocker intégré (uBlock-style filter lists) | v1.1 |
| ★★★ | Collection sharing via lien (infrastructure serveur) | v1.1 |
| ★★☆ | Ctrl+F recherche dans la page courante | v1.2 |
| ★★☆ | Picture-in-Picture mode (vue compacte) | v1.2 |
| ★☆☆ | Cloud sync collections + profils (tier payant) | v1.3 |
| ★☆☆ | CSS custom par site (nettoyage wikis) | v1.4 |
| ★☆☆ | Code signing certificate | v1.5 |

---

## Done — Récent

- [x] **[FEAT] Compatibilité navigateur standard (Google, Cloudflare)** (2026-06-01) — `feat/browser-compat`
  - Google login : UA Firefox + Sec-Fetch-* cohérents + identité JS complète (userAgent, productSub, oscpu, buildID, plugins:0, chrome:undefined) + auto-retry sur /rejected (clear cookies AEC + redir /signin/identifier)
  - Cloudflare navigation générale : UA Chrome propre + Sec-CH-UA alignés + userAgentData "Google Chrome" + webdriver:false + chrome.loadTimes/csi/runtime corrects + Function.prototype.toString native
  - `src/shared/userAgent.ts` + `src/preload/tabStealth.ts` + `TabManager` onBeforeSendHeaders + `index.ts` disable-blink-features
  - 162 tests, 100% coverage ; typecheck + lint verts
  - **Cloudflare Turnstile OAuth** (poe.ninja, filterblade) : incompatibilité plateforme WebContentsView documentée dans SECURITY.md — solution future : shell.openExternal
- [x] **Durcissement méthode — chaque axe ≥9/10** (2026-06-01)
  - Garde-fous : egress hors-localhost + `node -e` bloqués, permissions scopées
  - `SessionStart` hook (boot avec branche+TASKS+DEVLOG) ; routage auto des guides métier
  - 4 subagents (`security-reviewer`/`qa-tester`/`perf-auditor`/`a11y-reviewer`) + commands `/review-security` `/cover` `/ship`
  - **Smoke produit** `pnpm smoke` (lance la vraie app, vérifie overlay/screenshot/RAM) — ALL PASS
  - `postinstall` installe le pre-commit ; job CI `build-windows` (natif)
- [x] **Audit méthode + couverture logique métier à 100%** (2026-06-01)
  - Hooks réparés : feedback ESLint réellement remonté à Claude (`additionalContext`), `--max-warnings 0`
  - Garde-fou `PreToolUse` : bloque push main / force-push / reset --hard / clean -f / --no-verify / npm add
  - Setup d'autonomie enfin commité **et poussé** sur `origin/chore/claude-setup`
  - 147 tests, **100% stmts/branches/funcs/lines** sur l'allowlist logique (`vitest.config.ts`) + gate 100% en CI
  - Couvre : `[TEST] CollectionsManager` (CRUD + export/import Base64) et `[TEST] SessionManager` (save/restore/autosave)
- [x] Setup autonomie Claude : CLAUDE.md, devLogger, devServer, hooks, DEVLOG, TASKS (2026-05-29)
- [x] Core overlay + états (HIDDEN / FOCUSED / CLICK_THROUGH)
- [x] TabManager + WebContentsViews multi-onglets
- [x] ProfileManager + détection de jeu par polling processus
- [x] CollectionsManager + SessionManager
- [x] ShortcutManager + uiohook (WH_KEYBOARD_LL)
- [x] TrayManager
- [x] Settings panel
- [x] Onboarding + WelcomePage
- [x] MissionsTracker / Achievements
- [x] Auto-updater (GitHub Releases)
- [x] Crash logger

---

## Contraintes permanentes

- **Windows only** — pas de code macOS/Linux tant que la v1.0 n'est pas sortie
- **Zéro dépendance npm** sans accord explicite — auditer avec `pnpm check:deps`
- **Tout code dev-only** doit être gardé par `!app.isPackaged`
- **Tout canal IPC** doit être déclaré dans `src/shared/ipc.ts` avant usage
- **Pas de push direct sur `main`** — toujours passer par une PR depuis `dev`
- **`pnpm typecheck && pnpm lint`** doit passer avant tout commit

---

## Comment utiliser ce fichier (pour Claude)

1. **En début de session** : lis "État actuel" + "En cours" + "Priorité haute" pour choisir ta tâche.
2. **Pendant la session** : déplace la tâche choisie dans "En cours".
3. **En fin de session** : déplace les tâches terminées dans "Done", mets à jour "État actuel", note la date.
4. **Si tu trouves un bug** : ajoute-le en haut de "Priorité haute" avec le tag `[BUG]`.
5. **Si une tâche bloque** : note le blocage en commentaire sur la ligne, ne la supprime pas.
