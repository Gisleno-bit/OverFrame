# 4. Identidad, HUD y material de página

## Logo

Archivos vectoriales originales, sin fuente externa incrustada: `assets/brand/overframe-wordmark.svg` (claro), `overframe-wordmark-dark.svg` y `overframe-mark.svg`. El wordmark dibuja OVERFRAME con trazos geométricos de 9 unidades sobre altura de letra80 y avance78. Caja total732×120. El símbolo usa dos corchetes y un bloque central; caja64×64. No registrar ni prometer disponibilidad jurídica de marca a partir de este diseño.

Wordmark: mostrar a >=180 px de ancho. Por debajo, usar el símbolo a24/32/48 px. Área libre mínima: 12% de la altura del logo. Versión monocroma para fondos variables; no glow, perspectiva, sombras ni texturas. Colores de identidad: fondo #111F2B, tinta clara #F4F1E8, acento #F16F54; el logo nunca cambia con la paleta del jugador.

## Tipografía

Para coste cero, conservar la fuente que ya usa el juego, sin incorporar una familia nueva ni asumir una licencia. Texto de interfaz con caja normal; mayúsculas solo en personajes, pestañas y wordmark. Tamaños a1080p: cuerpo22px, ayuda18px, título32px, personaje22px, porcentaje64px, temporizador48px. Interlineado1,25. Las cifras de porcentaje y tiempo deben ocupar celdas de avance fijo: centrar cada glifo en una celda de ancho0,62×tamaño si la fuente no trae cifras tabulares. No rellenar porcentajes con ceros a la izquierda. Fuente genérica en la maqueta SVG: referencia de distribución, no compromiso con una familia concreta.

Escala global de UI=min(ancho/1920,alto/1080), centrada en canvas1920×1080 con bandas si cambia el aspecto. No escalar X/Y por separado. Ayuda nunca por debajo de12px de salida: si falta espacio, dividir en dos líneas.

## HUD exacto

Origen de pantalla arriba izquierda; unidades de diseño en px a1920×1080. El archivo hud-mockup-1920x1080.svg es una maqueta etiquetada, no una captura.

| Elemento | Rectángulo x,y,w,h | Regla |
|---|---|---|
| Placa P1 | 48,872,376,160 | Fondo #101923, radio8, opacidad1 |
| Placa P2 | 1496,872,376,160 | Igual; orden de lectura sin espejo tipográfico |
| Acento jugador | x,872,376,5 | primary de su paleta; nunca fondo del texto |
| Nombre | x+24,890,328,30 | Texto22, #F4F1E8; truncar alias, conservar personaje |
| Porcentaje | x+24,926,240,66 | Texto64, #F4F1E8; dígitos de ancho fijo, símbolo % a48 si hace falta |
| Stocks | x+280,994,80,20 | Hasta3 símbolos de14px, paso24; >3 mostrar un símbolo y ×N, sin recorte |
| Temporizador | 842,40,236,76 | Fondo #101923; texto48 centrado; formato del modo, sin inventar límite |
| Mensaje de pausa/fin | 560,420,800,160 | Solo cuando el estado del juego lo exige; no ocultar acción durante partida |

Porcentaje se obtiene del dato real y conserva las reglas de redondeo existentes. Mostrar el valor, no interpolarlo durante hitlag ni suavizarlo durante varios ticks. No hacer crecer el texto con el daño. Si ya hay flashes, limitar al marco, no hacer ilegible la cifra. Datos del mockup son ejemplos de diseño.

P1=círculo y P2=rombo, ambos claros sobre fondo oscuro. Usar el mismo distintivo en selector, HUD y etiqueta sobre luchador. Stock muestra cuenta real, no tres vidas decorativas fijas. Color de daño opcional solo en un indicador secundario; la cifra permanece clara. Distinción por forma evita depender exclusivamente de paletas.

Objetivo de contraste del texto >=4,5:1 sobre su placa opaca. Fórmula sRGB y referencia: https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html . No confundir esta verificación del HUD con accesibilidad total de la partida.

## Portada itch

Se entrega `itch-cover-1260x1000.svg`, exactamente315:250, con logo y cuatro símbolos abstractos del roster. Es material de identidad, no una captura ni prueba de que Trama esté implementado. Puede rasterizarse a1260×1000 o630×500 sin reencuadrar. La versión PNG adjunta es la rasterización del mismo original vectorial. Verificar lectura de OVERFRAME a315×250 antes de subirla. No poner texto de controles ni promesas de online en la miniatura.

La guía oficial pide relación315:250 y recomienda3–5 capturas: https://itch.io/docs/creators/getting-started . No se modifica aquí el documento C de publicación.

## Capturas y GIF que deben salir de la build

| Orden | Material | Qué tiene que demostrar | Condición para publicarlo |
|---|---|---|---|
| 1 | Combate local en The Lattice,1920×1080 | Dos siluetas distinguibles, bordes legibles, HUD | Captura game3d de la misma versión descargable |
| 2 | Selección,1920×1080 | Los personajes y paletas realmente disponibles | No mostrar Trama como disponible hasta implementarlo |
| 3 | Training con hitboxes,1920×1080 | Qué puede practicar el jugador | Funciones visibles realmente operativas |
| 4 | Mirror Kestrel0/3,1920×1080 | Identificación por color y P1/P2 | Captura del renderer con luz final |
| 5 | Lobby,1280×720 o1920×1080 | Método de conexión existente | Sin datos personales; sin simular Steam o matchmaking inexistente |
| GIF 1 | 3 segundos,960×540,20fps | Movimiento, salto y un contacto legible |180 ticks reales a60Hz, muestreo cada3; sin acelerar |
| GIF 2 | 3,6 segundos,512²,10fps | Giro360° de Kestrel,36 vistas | Renderer real, pose fija,10° entre vistas; identificarlo como turnaround |

No se entregan aquí falsas capturas ni GIF de una build que no he podido revisar. Los SVG de identidad sí están listos para usar. La imagen generada en la conversación es concept de dirección, no sustituye las tablas: puede variar el número de piezas, las proporciones, la curvatura de cola o los detalles. Implementar JSON, no reconstruir esa imagen a ojo.

Declaración para créditos de concept: «Dirección conceptual asistida por IA. Modelos finales procedurales implementados y revisados a partir de especificaciones propias». Adaptar la segunda frase a lo que efectivamente esté terminado. Esta entrega no contiene assets extraídos de juegos ni fuentes externas.
