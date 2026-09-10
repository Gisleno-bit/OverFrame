# OVERFRAME — entrega procedural v1

Entrega de dirección y especificaciones para que Claude implemente en Rust. Orden de trabajo: 6 → 1 (Kestrel, Boulder, Viper, Trama) → 2 → 3 → 4 → 5. No contiene cambios al juego ni afirma que los modelos estén implementados. No rehace el brief humano de Kestrel ni B/C/D/E.

## 6. Primero, el intercambio

Leer 06-INTERCAMBIO.md. Estado CI propuesto: rama visual-evidence, PROJECT_STATUS.json; evidencia por SHA completo y ejecución; specs en docs/art/procedural. La plantilla incluida usa null/not_run, no datos de una build imaginaria.

GitHub devolvió404 al intentar leer el repositorio y su PROJECT_STATUS.md. Esta entrega se basa en los archivos adjuntos; no certifica el estado actual de main. En esa copia, el exportador headless produce2D, mientras el frontend dispone de screenshot/record: Claude debe demostrar la ruta de captura del renderer3D real. Un GIF2D no valida mallas3D.

## 1. Personajes, en orden

| Personaje | Tabla legible / JSON | Piezas | Alto de tabla / semiancho | Extensión Y del modelo en reposo | Identidad en negro |
|---|---|---:|---|---|---|
| Kestrel | docs/art/procedural/kestrel.md / kestrel.json |31|30 /9|0,20–29,90|Tres cuñas de cresta; dos tramos de bufanda hacia−X; pecho inclinado |
| Boulder | docs/art/procedural/boulder.md / boulder.json |31|36 /12|0,25–35,80|Antebrazos de6,8 de ancho; torso de13,8 de extensiónX; tres bloques elevados |
| Viper | docs/art/procedural/viper.md / viper.json |31|27 /7,5|0,125–26,80|Capucha poligonal sobre torso estrecho; cola baja de tres tramos |
| Trama | docs/art/procedural/trama.md / trama.json |38|32 /9,5 PROPUESTOS|0,15–31,20|Dos marcos abiertos de7,2×7; corona rectangular; faldón partido |

Todas las piezas están enumeradas, incluidas izquierda/derecha: hueso, primitiva, dimensiones/radios, posición, rotación y slot. Huesos adicionales tienen padre, offset y retardo explícitos. FORMAT.md fija ejes y orden de rotación. No cambiar las cápsulas existentes para acomodar accesorios. El radio hurt real de la copia añade1,5 al half_width; Trama necesita tabla y tests nuevos.

131 piezas, construidas con bevel_box, plate, ellipsoid y cylinder. Sin skinning, texturas externas ni normales de textura. El renderer actual aporta las capacidades básicas; la primera vuelta puede mostrar todos los extras rígidos con retardo cero. Contacto y animación base conservan el frame data actual, que sigue sin exportarse en esta entrega.

## 2. Paletas

PALETAS.md + palettes.json: exactamente6 por personaje,8 slots, RGB8 sRGB. palette-contrast.json contiene las60 combinaciones de mirror. Parejas recomendadas0↔3,1↔4,2↔5; mínimo entre ellas3,705:1 de luminancia de primary. No todos los demás pares cumplen3:1. Identificación redundante P1=círculo/P2=rombo, sin cambiar protocolo ni selector.

## 3. The Lattice

lattice.md + lattice.json:41 piezas, posiciones mundo y colores. Tapas coincidentes con los cuatro colliders reales; decoración por debajo de las superficies. Fondo distante estático y sin glow. No nuevos colliders.

## 4. Identidad

IDENTIDAD.md contiene tamaños y reglas. assets/brand incluye wordmark y símbolo originales en SVG; portada SVG/PNG1260×1000 (315:250 exacto); maqueta HUD SVG/PNG1920×1080. El concept generado en conversación es dirección asistida por IA, no captura ni plano técnico; las tablas mandan ante discrepancias.

Las capturas/GIF reales quedan especificadas, no fabricadas. La portada es una composición de identidad con símbolos abstractos y puede usarse sin afirmar que muestra gameplay.

## 5. Limitaciones y gasto

Leer05-LIMITES-Y-GASTO.md. La vía procedural puede servir para el piloto; la aprobación depende del renderer real, no de esta documentación. Se mantiene la prueba humana para cuando haya retorno de jugadores, limitación visual demostrada y soporte técnico del brief existente.

## Verificación realizada

JSON parseables; referencias de huesos ordenadas; IDs únicos; placas convexas; conteos de paletas/slots; componentesRGB válidos; envolventes conservadoras en reposo dentro del alto y semiancho de referencia. Ver validation-report.json y bounds-report.json. Se rasterizaron e inspeccionaron portada y HUD.

No se ejecutaron fmt, clippy ni tests del juego, porque no se modificó código Rust. No se midieron FPS ni se vio una build con estos modelos. Los checks de autoría no sustituyen la validación de Claude contra el repositorio.

## Primera vuelta concreta para Claude

Crear el estado y suite de captura. Implementar únicamente kestrel.json + sus seis paletas sobre el renderer existente, extras inicialmente rígidos. Publicar turnaround, silueta, mirror0/3 y tres ticks de un ataque con el frame data exportado. Si faltan capturas3D, resolver esa ruta antes de ampliar al roster: así las correcciones son sobre imágenes y piezas concretas. Después, Boulder → Viper → Trama; la especificación completa ya está entregada.
