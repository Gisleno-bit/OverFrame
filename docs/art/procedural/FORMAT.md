# Formato y contrato de implementación v1

## Geometría

JSON manda; las tablas Markdown son su copia legible. Identificadores de pieza estables. Prohibido fusionar IDs semánticos aunque se agrupen sus triángulos en el mismo mesh. Conservar un mapa ID → rango de vértices para diagnóstico.

Unidades: las mismas que src/sim, sin centímetros ni factor de importación. +X hacia delante del personaje, +Y arriba, +Z hacia la cámara lateral. `r` corresponde a +Z y `l` a −Z en reposo. No reflejar X para crear el brazo izquierdo. Facing lo aplica el transform raíz existente; corregir winding/normales al reflejar, como hace el rig actual.

Transformación de pieza: `T(position) * Rx(rx) * Ry(ry) * Rz(rz) * vertex`. Euler en grados; sobre un vértice se aplica Z, luego Y, luego X. Después se aplica la transformación mundial del hueso. Huesos con offset local y rotación de reposo identidad. Escala de todas las piezas: [1,1,1]. No usar auto-fit ni reajustar altura en runtime.

| Primitiva | Parámetros exactos | Origen |
|---|---|---|
| cuboid | size=[ancho X,alto Y,fondo Z], dimensiones completas; escenario usa campo size | Centro |
| bevel_box | size=[ancho X,alto Y,fondo Z], dimensiones completas; bevel=distancia de corte | Centro |
| ellipsoid | radii=[radio X,radio Y,radio Z], lat=4, lon=8 | Centro; radios, NO diámetros |
| cylinder | r0=radio inferior, r1=superior, height=altura completa, segments | Eje Y; extremos ±height/2 |
| plate | points_xy convexos CCW; thickness=espesor completo | Puntos XY explícitos; Z=±thickness/2 |

Una cuña o prisma se construye con plate; no hace falta una primitiva nueva. No pasar un contorno cóncavo al abanico de triángulos actual. Los marcos de Trama son cuatro cajas, no una placa con agujero. Comprobar área positiva y convexidad. Al facetar elipsoides, eliminar triángulos degenerados de los polos antes de normalizar sus normales. Los radios son bajos deliberadamente: juntas pequeñas, masas principales de caras planas.

Toda pieza incluye `id,bone,primitive,parameters,position,rotation_deg,slot`. Campos no reconocidos: error de autoría. Todos los números finitos. El padre debe existir antes del hijo. Slots permitidos, en orden: primary, secondary, accent, skin, dark, glow, light, extra. No nuevos slots ni séptima paleta. No se requiere añadir parser al ejecutable: las tablas pueden traducirse a Rust.

Los modelos entregados usan 31/31/31/38 piezas. Los huesos root son anclas sin malla; una pieza por hueso no es una obligación. No añadir dientes, remaches, dedos independientes ni patrones menores de 0,4 unidades.

## Cápsulas y proporciones

Kestrel height=30, half_width=9; Boulder 36/12; Viper 27/7.5. Trama 32/9.5 es PROPUESTA nueva: Claude debe definirla en la tabla de personaje, exportarla y probarla antes de validarla. No se han propuesto valores de daño, velocidad o frame data de Trama aquí.

En la copia de referencia, hurt_radius=half_width+1.5. La cápsula tiene extremos del segmento en r y max(height−r,r+0.5), y height efectivo cambia con estados como crouch/shield/knockdown. Por eso un AABB dentro de height/half_width no demuestra coincidencia con la cápsula curva. El informe adjunto solo verifica la envolvente del modelo en reposo; las posiciones de combate necesitan overlays reales. No modificar cápsulas para cubrir accesorios ni convertir prendas en hurtboxes.

Criterios de implementación: ningún vértice en reposo bajo y=0; ningún vértice por encima de height o fuera de ±half_width en X; root en pies. En poses de ataque las extremidades pueden salir de esa envolvente; deben corresponder al contacto de las hitboxes existentes. Las prendas nunca cuentan como alcance de ataque. Los pies están 0,125–0,25 unidades sobre y=0 en reposo; el apoyo visual se resuelve con pose/shadow, no moviendo el suelo.

## Animación principal y contacto

Se conserva el evaluador procedural actual derivado del frame data. No introducir clips, skinning, físicas de tela, PBR, normal maps ni importaciones nuevas para esta entrega.

Separar capas en este orden: pose del combate existente → transform raíz existente → extras visuales → iluminación. La capa de extras solo escribe sus huesos nuevos. No escribe head, hand, forearm, foot ni otros huesos que definan contacto. No mezclar la pose de ataque con idle dentro de ticks activos. Si ya existe una transición, conservar su condición y comprobar con export runtime que no desplaza el miembro de contacto.

La referencia de pose es `state_frame` con su semántica actual, no una curva de tiempo en segundos. No sumar 1 ni restar 1 hasta verificar captura after_step y primer tick activo. Los estados de hitlag conservan la pose de contacto conforme al evaluador actual. Colisiones, ventanas, hitlag, checksum, save/load y rollback no consultan render ni extras.

## Retardo de extras, algoritmo cerrado

Solo adorno. Sin física de resortes ni acumulación dependiente de dt. Cada extra e declara delay_ticks d, gain g, max_angle_deg L y axis=z. Los valores están en cada JSON; no hay ruido ni RNG. Cada historial es privado del renderer.

1. Al evaluar las poses de cada tick simulado, fuera de sim::step, registrar la orientación del padre de e SIN la capa de retardo del propio e. Usar quaternions normalizados. En cadenas, evaluar padres antes de hijos.
2. Reloj visual por luchador: un índice por tick corregido que avanza solo cuando no está congelada su pose por hitlag; conservar la asociación con el tick de simulación. No usar wall clock. Para rollback, invalidar las muestras desde el tick restaurado y regenerarlas con los snapshots de la resincronización; ese registro es caché visual, nunca parte de la lógica de combate.
3. Obtener q_now y q_old, separados d muestras del reloj no congelado. Calcular delta=q_now.inverse()*q_old. Extraer el twist firmado alrededor de Z, convertir a grados, escoger intervalo [−180,180] y aplicar theta=clamp(g*twist,−L,+L).
4. Rotación local del hueso extra = su rotación de reposo (identidad) seguida de Rz(theta). Su offset se conserva: se ancla al padre actual, no a la posición antigua. No retardo de traslación. No oscilación autónoma.
5. En hitlag, congelar reloj y theta de extras; el transform raíz sigue la posición corregida del luchador. Al cambiar facing, resetear historial para impedir que el giro de 180° haga cruzar la bufanda por el cuerpo.
6. Al cargar replay, saltar de tick, cambiar personaje o faltar historial: rellenar d muestras con q_now y theta=0; no reutilizar memoria de una partida anterior. Mantener hasta 32 muestras y sus ticks, suficientes para el mayor retardo de 7 y margen de rollback local; si el rollback excede la historia disponible, usar el mismo reset, no inventar pasado.

Si registrar poses durante rollback requiere una modificación invasiva, la versión inicial usa theta=0 para todos los extras. Esta es una degradación visual explícita, segura y aceptable; no introducir cachés incorrectas para simular un retardo. La CI debe comparar reset/seek/repetición de fixture. Los brazales de Trama, su corona y cualquier elemento de contacto llevan d=g=L=0.

## Renderer: orden y límite de alcance

1. Mallas nuevas y paletas sobre el renderer actual; normales finitas y winding correcto. Prueba sin luz de silueta y albedo.
2. Plano de combate y layout del escenario, masas de color sin texturas. Conservar las texturas procedurales existentes solo si no reducen legibilidad; esta dirección no depende de ellas.
3. Ajustar luz toon CPU y contorno. Perfil inicial propuesto, sujeto a capturas: key direction normalizada (−0.35,0.8,0.6); key RGB (0.48,0.47,0.45); sky (0.32,0.34,0.36); ground (0.18,0.20,0.23); fill=0.16; rim=0.03; bands=4. Son coeficientes de iluminación, no colores sRGB. No cambiar la fórmula del shader sin prueba visual comparativa. El código actual no es un pipeline PBR lineal; no prometer respuesta física de metal/tela.
4. Contorno exterior oscuro: objetivo 1,5 px a 1080p y 1 px a 720p. Calcular grosor de mundo a partir de proyección local si se conserva inverted hull; limitar 0,08–0,28 unidades. Suprimir contorno en piezas interiores y extras <1 unidad, no dibujar cada triángulo. En cámara fija de ancho400 a1920px, 1,5px equivale a0,3125u y se limita a0,28u. Registrar el grosor final en evidencia.
5. Sombra de contacto opcional: elipse sobre superficie de soporte real, sin alterar colisión; radio X entre2 y6 según personaje, radioZ=2, opacidad máxima0,20. Se desvanece linealmente hasta altura40 y no proyecta a través de plataformas. Si no se puede determinar soporte sin ambigüedad, omitirla. Esto no equivale a sombras dinámicas reales.
6. Retardo visual de extras y HUD. Mantener un solo LOD inicialmente; presupuesto objetivo <3000 triángulos/personaje y <6000 del escenario visible. Contar en runtime antes de prometer rendimiento. No añadir LOD ni materiales avanzados hasta medir un cuello de botella.

Pruebas necesarias por riesgo: cantidades de paletas exactas; bones/slots válidos; bounds; normales finitas y no degeneradas; determinismo/save-load al incorporar Trama; hashes de sim iguales con renderer/extras activados y desactivados. Conservar todos los tests existentes. Capturas no sustituyen tests y un check de forma no acredita 60 FPS.
