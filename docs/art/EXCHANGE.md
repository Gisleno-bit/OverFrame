# 6. Contrato de intercambio — OVERFRAME procedural v1

> Coordinación actual: [repositorio y trabajo conservado](REPOSITORY_MAP.md),
> [flujo de entrega](WORKFLOW.md) y [roles](ROLES.md). Los párrafos de estado
> de la entrega inicial que siguen son históricos; el acceso y los resultados
> actuales se verifican en Git y en el estado CI enlazado desde
> [PROJECT_STATUS.md](../../PROJECT_STATUS.md). Las cámaras y fixtures vigentes
> están en [capture-suite.json](capture-suite.json), y la animación se valida
> con su [contrato específico](procedural/anim/FORMAT.md). No reutilizar un
> antiguo número de tests, error de acceso o tabla de cámaras como estado actual.

Estado de esta entrega: especificación propuesta; no implementación ni aprobación visual de una build. El acceso GitHub al repositorio y a PROJECT_STATUS.md devolvió 404. No determina si el repositorio es privado, aún no está publicado o falta acceso. Referencia consultada: copia adjunta del proyecto, identificada por el usuario como v0.5.0/db251b0. No se han ejecutado sus tests en esta entrega. El número 92 es el baseline comunicado, no un resultado nuevo.

## Autoridad y orden de lectura

1. Leer `https://raw.githubusercontent.com/Gisleno-bit/OverFrame/visual-evidence/PROJECT_STATUS.json`.
2. Comparar `source_sha` con el SHA actual de `main`. Si difieren: revisión histórica, nunca «estado actual».
3. Leer manifiesto, specs y fuente por SHA completo. Las imágenes se leen desde `evidence_commit`, no desde una URL móvil de main.
4. Si falta estado, manifiesto, imágenes o coincidencia de SHA: se puede especificar; no aprobar la implementación.

No existe todavía ese contrato en el repositorio accesible: Claude debe crearlo. La plantilla adjunta usa null y not_run deliberadamente.

## Rutas estables

| Rama | Ruta | Autor / función |
|---|---|---|
| main | PROJECT_STATUS.md | Claude: enlace al estado CI y explicación del esquema; no escribir aquí un SHA autorreferente |
| main | docs/art/procedural/FORMAT.md | Contrato de unidades, geometría y animación visual |
| main | docs/art/procedural/{kestrel,boulder,viper,trama}.json | Fuente numérica de las piezas, huesos y extras |
| main | docs/art/procedural/{kestrel,boulder,viper,trama}.md | Tablas legibles, generadas de los mismos JSON |
| main | docs/art/procedural/palettes.json | 4 × 6 × 8 colores RGB enteros; índices estables 0–5 |
| main | docs/art/procedural/lattice.json | Geometría visual del escenario; sin modificar colisiones |
| main | docs/art/procedural/IDENTIDAD.md | Logo, HUD, portada y lista de capturas |
| main | docs/art/capture-suite.json | Cámaras, poses, entradas y versión de la batería de capturas |
| visual-evidence | PROJECT_STATUS.json | CI: último intento y última build completa, distinguibles |
| visual-evidence | builds/<source_sha>/attempt-<run_id>-<run_attempt>/manifest.json | Evidencia inmutable por ejecución |
| visual-evidence | builds/.../characters/<id>/ | turnaround, silhouette, poses y contactos |
| visual-evidence | builds/.../scenes/ | combate y escenario |
| visual-evidence | builds/.../ui/ | selección y HUD reales |
| visual-evidence | builds/.../runtime/ | frame-data.csv, characters.json, stages.json |
| main | docs/art/reviews/<source_sha>.md | Correcciones contra evidencia concreta |
| main | docs/art/ROLES.md | Roles (ChatGPT = dirección visual, Claude = código) y referencias de animación/movimientos |

No modificar el brief de Kestrel ni B/C/D/E. Esta especificación es un documento nuevo para la vía procedural. Los JSON son de autoría; no se exige un lector JSON dentro del ejecutable. Claude puede traducirlos a constructores Rust.

## Publicación CI

Se conserva la CI existente de compilación y Releases. Añadir captura después de fmt, clippy -D warnings y todos los tests. El baseline de 92 no es un techo: los nuevos tests deben aumentar el total cuando corresponda.

La CI debe publicar también el estado fallido mediante una tarea final que se ejecute aunque fallen las comprobaciones. Al iniciar, registrar intento pendiente; al terminar, publicar juntos manifiesto y archivos antes de cambiar el puntero a evidencia completa. Nunca atribuir imágenes de una ejecución anterior a la nueva. `latest_pass` puede seguir apuntando a la anterior, explícitamente.

Publicación en rama separada con permiso `contents: write` exclusivamente en trabajo de publicación de código confiable de main. PR de forks: artefacto CI, sin escritura ni secretos. Disparadores limitados a main/tags; la rama de imágenes no vuelve a compilar. Serializar publicación y no dejar que una ejecución antigua sobrescriba el estado de un SHA más reciente. Consultar el HEAD remoto antes de actualizar el puntero. No publicar tokens, nombres de red ni datos personales de testers.

Cada manifest incluye: SHA fuente completo; versión; SHA de specs; suite de captura; run_id y run_attempt; fecha UTC; plataforma; renderer real empleado; GPU/driver si disponibles; checks con comando, resultado y total de tests; archivos con SHA256, dimensiones, tick y cámara; capacidades implementadas y pendientes. Una capacidad solo es `verified` si identifica una prueba/evidencia. `not_run` nunca equivale a verde.

Las imágenes en Git acumulan historial incluso si se borran del árbol. Presupuesto propuesto: PNG <=2 MiB y GIF <=8 MiB; máximo 24 MiB por ejecución. Guardar comparación reciente en rama y usar artefactos de Releases para archivo de largo plazo; no reescribir historia automáticamente. Si el presupuesto se supera, falla la publicación visual con causa explícita; no bajar resolución silenciosamente.

Documentación de disparadores: https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow

## Captura: requisito real, no supuesto

En la copia adjunta, `src/bin/replay.rs` + `src/headless.rs` dibujan `viz::draw_scene`, una referencia 2D. Eso no demuestra el modelo 3D. El frontend tiene opciones de screenshot/record: reutilizar su renderer real para las capturas 3D, mediante contexto gráfico virtual si es necesario. El soporte de captura de menús y cámaras fijas debe implementarse/verificarse. No llamar «captura del juego» a un concept o a un render que evita el renderer de producción.

Etiquetas obligatorias: `game3d` (renderer real), `sim2d` (diagnóstico) o `concept` (dirección, nunca evidencia de implementación). Mesa/software es aceptable para validar forma y color, no para demostrar 60 FPS en una GTX 1050. Una comprobación Windows real con mando y rendimiento sigue siendo independiente.

## Tomas idénticas entre iteraciones

Convención del modelo: +X frente del personaje, +Y arriba, +Z hacia cámara lateral. Unidades de mundo, proyección ortográfica. Cero autoencuadre y cero reescala por personaje.

| Archivo | Resolución | Cámara / entrada fija |
|---|---:|---|
| characters/<id>/turnaround.png | 1024×1024 | 4 celdas 512²; ojo (0,22,120), objetivo (0,22,0), up +Y, alto ortográfico 44; yaw raíz 0/90/180/270°; rest pose sin lag |
| characters/<id>/silhouette.png | 1024×1024 | Mismo montaje; objeto negro, fondo blanco, sin suelo, luz ni contorno |
| characters/<id>/combat-size.png | 640×180 | Mismo modelo y pose, alturas proyectadas de referencia 72 y 120 px; declarar escalas por celda; fondo jugable real |
| scenes/lattice-fixed.png | 1920×1080 | Ojo (0,52.5,500), objetivo (0,52.5,0), ancho ortográfico 400; alto 225; sin jugadores |
| scenes/combat-fixed.png | 1920×1080 | Cámara anterior; P1 raíz (-45,0,0), P2 (45,0,0), mirando al centro, idle tick de fixture 120; HUD real |
| scenes/combat-depth.png | 1920×1080 | Ojo (0,156.5,489), objetivo (0,52.5,0), ancho 400; misma escena; segunda toma para leer profundidad |
| characters/<id>/palettes.png | 1536×1024 | 3×2 celdas de 512², misma cámara y pose; índices 0–5 |
| characters/<id>/contact-<action>.png | 1536×512 | 3 celdas: último tick inactivo, primer activo, primer tick posterior al último activo; hitboxes superpuestas; valores del exportador |
| scenes/combat.gif | 960×540 | Fixture de 180 ticks a 60 Hz; capturar cada 3 ticks y reproducir a 20 fps, duración 3 s; misma cámara fija |
| ui/selection.png | 1920×1080 | Menú real, dos slots ocupados, selecciones e índices de paleta explícitos en manifest |

El fixture es un fichero versionado de inputs, seed local si existe y estado inicial. Su hash va en manifest. Calentar efectos visuales 64 ticks antes de secuencias animadas; resetearlos al comenzar cada fixture. Capturar rest pose aparte de idle para no confundir geometría con animación. Para Trama pendiente: archivo ausente con motivo, nunca otro personaje renombrado.

## Exportación real de frame data

Claude exporta desde el runtime. Esta entrega NO contiene filas de frame data inventadas. Cabecera acordada:

`source_sha,character_id,action_id,variant_id,sample_phase,tick_index,state_frame,hitbox_id,hitbox_active,center_x,center_y,radius,hitlag_remaining,contact_marker`

Una fila por hitbox y tick observado. `sample_phase=after_step` fija el posible desfase de un tick. Acciones sin hitboxes: hitbox_id y geometría vacíos. `variant_id` distingue carga, orientación y otras condiciones que cambien el ataque; el manifest contiene los inputs y estado inicial de cada caso. Las cápsulas y plataformas van en characters.json/stages.json desde runtime. Si hay un dato que el runtime no expone, no rellenarlo por intuición.

## Bucle y responsabilidad

1. Yo entrego spec versionada, identificadores de pieza estables y criterio numérico.
2. Claude implementa, comprueba números contra el runtime, añade los tests necesarios y commitea. Cambios de parámetros propuestos se anotan en la spec; no se esconden en un factor global de escala.
3. CI publica evidencia y estado. Tú me pasas el enlace o pides revisión. No hay vigilancia autónoma de GitHub implícita.
4. Leo estado y evidencia del SHA indicado; doy correcciones concretas. Si no puedo abrir una imagen, lo digo y no la apruebo.
5. Claude cambia las piezas indicadas. Nueva ejecución, misma cámara y fixture. Se cierra una corrección cuando la evidencia nueva cumple el criterio.

Formato de cada observación:

```yaml
source_sha: <40 caracteres>
evidence_commit: <40 caracteres>
run_id: <id real>
image: builds/<sha>/attempt-<id>-<attempt>/characters/kestrel/turnaround.png
spec: docs/art/procedural/kestrel.json
code: src/model/characters.rs
piece_id: k_crest_2
expected: <medida o apariencia observable concreta>
observed: <lo realmente visible; desconocido si falta evidencia>
change: <campo exacto, valor anterior y propuesto>
acceptance: <qué captura o medida demuestra la corrección>
verdict: pass | changes_required | blocked_missing_evidence
```

Una observación estética sin medida puede señalar el problema; no autoriza a inventar un número medido a ojo. En ese caso pedir bounds/overlay en la siguiente captura y separar medida de estimación.
