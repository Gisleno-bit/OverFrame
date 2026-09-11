# Roles y recursos de referencia

Quién decide qué en OVERFRAME, y qué material se estudia para animaciones
y movimientos. Las instrucciones explícitas actuales del usuario tienen prioridad;
este documento conserva el reparto de trabajo acordado para el juego.

## Entrega activa: Alpha 0.7

La petición del usuario del 2026-09-11 fija [Alpha 0.7](ALPHA_0_7.md) como objetivo actual.
ChatGPT dirige técnica, gameplay y arte; Claude implementa. Ese documento actualiza
el orden de fases del piloto anterior sin descartar su trabajo ya realizado.

## Inicio de cada entrega

Confirmar repositorio, rama, SHA y cambios existentes siguiendo [WORKFLOW.md](WORKFLOW.md).
El juego vive en `Gisleno-bit/OverFrame`; `Gisleno-bit/Overframe-1` es otro
producto, un navegador overlay. Leer [REPOSITORY_MAP.md](REPOSITORY_MAP.md) para
conservar el trabajo de ambos y evitar mezclar sus carpetas, comandos o políticas.

## Roles

| Quién | Se encarga de | No hace |
|---|---|---|
| **Espartaco** (Gisleno-bit) | Producción y decisiones: prioridades, fases (build → itch.io → Steam), presupuesto, jugadores y feedback, aprobar o rechazar entregas | — |
| **ChatGPT — dirección técnica, gameplay y visual** | **Dirección técnica y de gameplay, diagnóstico y criterios de aceptación; todo el tema visual**: diseño de personajes y escenario como especificaciones procedurales (`docs/art/procedural/*.json`, FORMAT.md), paletas, identidad (logo, portada, HUD), **dirección de animación y poses de los movimientos**, revisión de cada tanda de evidencia (`docs/art/reviews/<sha>.md`) con observaciones por `piece_id` | Escribir código Rust; cambiar daño, hitboxes, frame data o cápsulas para acomodar una pieza |
| **Claude — código** | Simulación, renderer, loader de specs, captura de evidencia, exportación de frame data, CI, tests, empaquetado; implementar las specs y las poses que ChatGPT define, y avisar cuando algo no sea implementable tal cual | Decidir la dirección de arte por su cuenta; inventar números "a ojo" en lugar de exportarlos del runtime |

Bucle: ChatGPT entrega spec → Claude implementa y commitea → CI (o Claude en
local) publica evidencia con cámaras fijas → ChatGPT revisa contra imágenes
concretas → Claude corrige lo indicado. Formato y rutas: `docs/art/EXCHANGE.md`.

## Recursos que ambos estudian para animaciones y movimientos

Referencias de **cómo se mueven y cuándo golpean** los personajes de un
platform fighter. Se usan para el ritmo (anticipación, golpe, recuperación),
las poses de contacto y los rangos habituales de startup/activo/endlag por
clase de movimiento. Se toman *como orientación* ("más o menos"): los
números de OVERFRAME siguen siendo originales y se calibran con
`docs/GAME_FEEL.md`; nunca se copian assets, modelos, sprites ni tablas
enteras (regla clean-room de `docs/LEGAL.md` y `CONTRIBUTING.md`).

| Recurso | Qué aporta | Cómo usarlo aquí |
|---|---|---|
| [FightCore — Super Smash Bros. Melee Frame Data](https://www.fightcore.gg/) | Frame data de los 26 personajes de Melee con **animación de cada movimiento (webm) y visualización de hitboxes** por frame; calculadora de crouch cancel | Referencia principal de *feel*: qué frame sale el golpe respecto a la pose, cuánto dura el active, cómo se recupera; poses de contacto para `anim.rs` y para las hojas `contact-<acción>.png` |
| [Dragdown — RoA2 Frame Data](https://dragdown.wiki/wiki/RoA2/Frame_Data) | Frame data completo de *Rivals of Aether II*: un platform fighter moderno de estudio pequeño, con roster reducido y personajes muy diferenciados | Referencia de **cómo se lee un roster pequeño**: identidad por silueta y movimiento, ventanas de aéreos, dirección de animación con presupuesto contenido |
| [SSBWiki](https://www.ssbwiki.com/) | Mecánicas universales y frame data por personaje en texto | Mirror que sí es accesible desde el entorno de Claude; se usó para calibrar `GAME_FEEL.md` |

Nota de acceso: desde el entorno de Claude las páginas de personaje de
FightCore devuelven un bucle de redirecciones y Dragdown responde 403, así
que Claude no puede leerlas directamente. Cuando una revisión necesite un
dato concreto de esas páginas, ChatGPT (que sí puede navegarlas) o
Espartaco lo pegan en el hilo o en la spec; Claude no lo rellena de memoria.

## Cómo pasan las referencias de animación al juego

1. ChatGPT estudia el movimiento equivalente en FightCore / RoA2 y escribe
   la **pose de referencia** para OVERFRAME: miembro que golpea, arco
   (de dónde a dónde), inclinación del torso, qué hace la otra mano, y en qué
   fracción del startup se alcanza la anticipación. Sin números de daño ni
   ventanas: eso sale de `runtime/frame-data.csv`.
2. Claude la implementa en `src/model/anim.rs` sobre el evaluador actual
   (anticipación → golpe → recuperación guiado por el frame data real) y
   publica `contact-<acción>.png` con la hitbox del motor superpuesta.
3. ChatGPT revisa que la extremidad de contacto coincide con la hitbox en
   "primer activo" y que la silueta lee; corrige por `piece_id` o por pose.

Formato vigente: [contrato de animación](procedural/anim/FORMAT.md) y
`docs/art/procedural/anim/<personaje>.json`, una entrada por `action_id` real
y evidencia de todas sus variantes. Kestrel ya tiene su dirección de 22 acciones;
la existencia del JSON no equivale a implementación ni aprobación visual.
