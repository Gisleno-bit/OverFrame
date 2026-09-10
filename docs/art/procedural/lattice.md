# 3. The Lattice — especificación procedural

Todas las piezas son decorativas; ninguna crea colisiones. Usar exactamente los colliders existentes. Dimensiones completas XYZ, posiciones mundo XYZ, grados Euler según FORMAT.md. Sin texturas, partículas ni animación de fondo.

| ID | Forma | Tamaño XYZ | Posición XYZ | Rotación XYZ° | Bisel | sRGB | Capa |
|---|---|---|---|---|---:|---|---|
| main_cap | cuboid | [310, 1, 32] | [0, -0.5, 0] | [0, 0, 0] | 0 | #73878F | playable |
| main_lip | cuboid | [310, 0.5, 0.6] | [0, -0.25, 16.3] | [0, 0, 0] | 0 | #A6B5B7 | playable |
| main_base | bevel_box | [309, 7, 31] | [0, -4.5, 0] | [0, 0, 0] | 0.6 | #344B59 | playable |
| main_end_l | bevel_box | [2, 2, 33] | [-154.0, -2, 0] | [0, 0, 0] | 0.15 | #B7A479 | playable |
| main_end_r | bevel_box | [2, 2, 33] | [154.0, -2, 0] | [0, 0, 0] | 0.15 | #B7A479 | playable |
| left_cap | cuboid | [82, 1, 18] | [-84, 58.5, 0] | [0, 0, 0] | 0 | #73878F | playable |
| left_lip | cuboid | [82, 0.5, 0.6] | [-84, 58.75, 9.3] | [0, 0, 0] | 0 | #A6B5B7 | playable |
| left_base | bevel_box | [81, 4, 17] | [-84, 56, 0] | [0, 0, 0] | 0.6 | #344B59 | playable |
| left_end_l | bevel_box | [2, 2, 19] | [-124.0, 57, 0] | [0, 0, 0] | 0.15 | #B7A479 | playable |
| left_end_r | bevel_box | [2, 2, 19] | [-44.0, 57, 0] | [0, 0, 0] | 0.15 | #B7A479 | playable |
| right_cap | cuboid | [82, 1, 18] | [84, 58.5, 0] | [0, 0, 0] | 0 | #73878F | playable |
| right_lip | cuboid | [82, 0.5, 0.6] | [84, 58.75, 9.3] | [0, 0, 0] | 0 | #A6B5B7 | playable |
| right_base | bevel_box | [81, 4, 17] | [84, 56, 0] | [0, 0, 0] | 0.6 | #344B59 | playable |
| right_end_l | bevel_box | [2, 2, 19] | [44.0, 57, 0] | [0, 0, 0] | 0.15 | #B7A479 | playable |
| right_end_r | bevel_box | [2, 2, 19] | [124.0, 57, 0] | [0, 0, 0] | 0.15 | #B7A479 | playable |
| top_cap | cuboid | [82, 1, 18] | [0, 116.5, 0] | [0, 0, 0] | 0 | #73878F | playable |
| top_lip | cuboid | [82, 0.5, 0.6] | [0, 116.75, 9.3] | [0, 0, 0] | 0 | #A6B5B7 | playable |
| top_base | bevel_box | [81, 4, 17] | [0, 114, 0] | [0, 0, 0] | 0.6 | #344B59 | playable |
| top_end_l | bevel_box | [2, 2, 19] | [-40.0, 115, 0] | [0, 0, 0] | 0.15 | #B7A479 | playable |
| top_end_r | bevel_box | [2, 2, 19] | [40.0, 115, 0] | [0, 0, 0] | 0.15 | #B7A479 | playable |
| main_truss_0 | bevel_box | [38, 2, 7] | [-120, -13, 0] | [0, 0, 20] | 0.2 | #263C49 | playable |
| main_truss_1 | bevel_box | [38, 2, 7] | [-60, -13, 0] | [0, 0, -20] | 0.2 | #263C49 | playable |
| main_truss_2 | bevel_box | [38, 2, 7] | [0, -13, 0] | [0, 0, 20] | 0.2 | #263C49 | playable |
| main_truss_3 | bevel_box | [38, 2, 7] | [60, -13, 0] | [0, 0, -20] | 0.2 | #263C49 | playable |
| main_truss_4 | bevel_box | [38, 2, 7] | [120, -13, 0] | [0, 0, 20] | 0.2 | #263C49 | playable |
| main_keel | bevel_box | [190, 3, 9] | [0, -20, 0] | [0, 0, 0] | 0.4 | #203540 | playable |
| bg_post_0 | bevel_box | [5, 220, 5] | [-180, 65, -150] | [0, 0, 0] | 0.4 | #172C39 | background |
| bg_post_1 | bevel_box | [5, 220, 5] | [-120, 65, -150] | [0, 0, 0] | 0.4 | #172C39 | background |
| bg_post_2 | bevel_box | [5, 220, 5] | [-60, 65, -150] | [0, 0, 0] | 0.4 | #172C39 | background |
| bg_post_3 | bevel_box | [5, 220, 5] | [0, 65, -150] | [0, 0, 0] | 0.4 | #172C39 | background |
| bg_post_4 | bevel_box | [5, 220, 5] | [60, 65, -150] | [0, 0, 0] | 0.4 | #172C39 | background |
| bg_post_5 | bevel_box | [5, 220, 5] | [120, 65, -150] | [0, 0, 0] | 0.4 | #172C39 | background |
| bg_post_6 | bevel_box | [5, 220, 5] | [180, 65, -150] | [0, 0, 0] | 0.4 | #172C39 | background |
| bg_brace_0 | bevel_box | [75, 3, 3] | [-150, 20, -148] | [0, 0, 35] | 0.2 | #1B303D | background |
| bg_brace_1 | bevel_box | [75, 3, 3] | [-90, 85, -148] | [0, 0, -35] | 0.2 | #1B303D | background |
| bg_brace_2 | bevel_box | [75, 3, 3] | [-30, 20, -148] | [0, 0, 35] | 0.2 | #1B303D | background |
| bg_brace_3 | bevel_box | [75, 3, 3] | [30, 85, -148] | [0, 0, -35] | 0.2 | #1B303D | background |
| bg_brace_4 | bevel_box | [75, 3, 3] | [90, 20, -148] | [0, 0, 35] | 0.2 | #1B303D | background |
| bg_brace_5 | bevel_box | [75, 3, 3] | [150, 85, -148] | [0, 0, -35] | 0.2 | #1B303D | background |
| bg_beam_low | bevel_box | [420, 5, 8] | [0, -40, -155] | [0, 0, 0] | 0.5 | #203442 | background |
| bg_beam_high | bevel_box | [420, 4, 8] | [0, 180, -155] | [0, 0, 0] | 0.5 | #203442 | background |

## Lectura y límites

Losa principal: x=[−155,155], cara superior y=0. Laterales: [−125,−43] y [43,125], caras superiores y=59. Superior: [−41,41], cara superior y=117. Las tapas sin bisel conservan toda la superficie visual hasta el borde; el bisel se aplica a la masa inferior. El borde luminoso ocupa y∈[y−0.5,y], nunca por encima del suelo. Los testigos ocres quedan dentro de extremos y por debajo del plano de apoyo.

Fondo: entramado estático en z≈−150, albedo #172C39–#203442 sobre #111F2B. No estrellas, cables brillantes, símbolos flotantes ni movimiento detrás de los luchadores. No elementos que parezcan plataformas adicionales. Las vigas distantes se dibujan sin contorno y sin glow; exposición de fondo fija, sin rim. Su escaso contraste está deliberadamente separado de las tapas #73878F y labios #A6B5B7.

El objetivo es reconocer dónde termina cada superficie de apoyo en una captura en escala de grises. Captura frontal fija primero; toma inclinada después. No mover cámara ni plataformas para hacer caber la decoración. Si un detalle tapa una extremidad, quitar primero el detalle. No ocultar hitboxes de entrenamiento con malla: overlay al final del pase jugable.

Presupuesto: 41 piezas decorativas, más fondo plano; contar triángulos en runtime. No sombras dinámicas nuevas requeridas. La calidad vendrá de proporciones, planos grandes, paleta y composición. Aceptación pendiente de captura del renderer real.
