# Kestrel — piezas procedurales v1

Mensajero de aristas: perfil inclinado, tres aletas de cresta, dos tramos de bufanda hacia -X. Sin alas ni tela simulada.

Referencia: alto 30; semiancho de tabla 9; radio hurt de referencia 10.5. Estado: `uploaded_snapshot_not_live_verified`.

Leer FORMAT.md antes de implementar. Todas las medidas son unidades de juego; rotaciones XYZ en grados; r/l son profundidad +Z/-Z, no posiciones opuestas en X. Rest pose neutra. JSON es la fuente numérica. No reescalar el modelo automáticamente.

## Huesos

| Hueso | Padre | Offset local XYZ |
|---|---|---|
| root | — | [0, 0, 0] |
| hips | root | [0, 13.7, 0] |
| spine | hips | [0, 3.15, 0] |
| chest | spine | [0, 3.675, 0] |
| neck | chest | [0, 3.675, 0] |
| head | neck | [0, 0.8, 0] |
| upper_arm_r | chest | [0, 2.94, 3.8] |
| forearm_r | upper_arm_r | [0, -5, 0] |
| hand_r | forearm_r | [0, -4.5, 0] |
| thigh_r | hips | [0, 0, 2.3] |
| shin_r | thigh_r | [0, -6.5, 0] |
| foot_r | shin_r | [0, -6, 0] |
| upper_arm_l | chest | [0, 2.94, -3.8] |
| forearm_l | upper_arm_l | [0, -5, 0] |
| hand_l | forearm_l | [0, -4.5, 0] |
| thigh_l | hips | [0, 0, -2.3] |
| shin_l | thigh_l | [0, -6.5, 0] |
| foot_l | shin_l | [0, -6, 0] |
| crest | head | [0, 2, 0] |
| scarf_a | neck | [-1.8, -0.8, 0] |
| scarf_b | scarf_a | [-3.2, -0.7, 0] |

## Piezas

| ID | Hueso | Primitiva | Parámetros | Posición XYZ | Rotación XYZ° | Slot |
|---|---|---|---|---|---|---|
| k_pelvis | hips | bevel_box | {"size": [4.5, 3, 5], "bevel": 0.45} | [0, 0, 0] | [0, 0, 0] | secondary |
| k_neck | neck | cylinder | {"r0": 1.1, "r1": 0.9, "height": 1.8, "segments": 8} | [0, 0.1, 0] | [0, 0, 0] | dark |
| k_upper_arm_r | upper_arm_r | bevel_box | {"size": [2.15, 5.2, 1.892], "bevel": 0.344} | [0, -2.5, 0] | [0, 0, 0] | secondary |
| k_forearm_r | forearm_r | bevel_box | {"size": [2.65, 4.7, 2.332], "bevel": 0.424} | [0, -2.25, 0] | [0, 0, 0] | primary |
| k_thigh_r | thigh_r | bevel_box | {"size": [2.8, 6.7, 2.464], "bevel": 0.448} | [0, -3.25, 0] | [0, 0, 0] | secondary |
| k_shin_r | shin_r | bevel_box | {"size": [2.4, 6.2, 2.112], "bevel": 0.384} | [0, -3.0, 0] | [0, 0, 0] | primary |
| k_elbow_r | forearm_r | ellipsoid | {"radii": [0.989, 0.989, 0.989], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| k_knee_r | shin_r | ellipsoid | {"radii": [1.152, 1.152, 1.152], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| k_hand_r | hand_r | bevel_box | {"size": [2.438, 1.7, 2.385], "bevel": 0.25} | [0.25, -0.65, 0] | [0, 0, 0] | light |
| k_foot_r | foot_r | bevel_box | {"size": [4.4, 1.8, 2.7], "bevel": 0.25} | [0.65, -0.1, 0] | [0, 0, 0] | dark |
| k_shin_mark_r | shin_r | bevel_box | {"size": [1.56, 1.15, 0.25], "bevel": 0.08} | [0.1, -1.8, 1.176] | [0, 0, 0] | accent |
| k_upper_arm_l | upper_arm_l | bevel_box | {"size": [2.15, 5.2, 1.892], "bevel": 0.344} | [0, -2.5, 0] | [0, 0, 0] | secondary |
| k_forearm_l | forearm_l | bevel_box | {"size": [2.65, 4.7, 2.332], "bevel": 0.424} | [0, -2.25, 0] | [0, 0, 0] | primary |
| k_thigh_l | thigh_l | bevel_box | {"size": [2.8, 6.7, 2.464], "bevel": 0.448} | [0, -3.25, 0] | [0, 0, 0] | secondary |
| k_shin_l | shin_l | bevel_box | {"size": [2.4, 6.2, 2.112], "bevel": 0.384} | [0, -3.0, 0] | [0, 0, 0] | primary |
| k_elbow_l | forearm_l | ellipsoid | {"radii": [0.989, 0.989, 0.989], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| k_knee_l | shin_l | ellipsoid | {"radii": [1.152, 1.152, 1.152], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| k_hand_l | hand_l | bevel_box | {"size": [2.438, 1.7, 2.385], "bevel": 0.25} | [0.25, -0.65, 0] | [0, 0, 0] | light |
| k_foot_l | foot_l | bevel_box | {"size": [4.4, 1.8, 2.7], "bevel": 0.25} | [0.65, -0.1, 0] | [0, 0, 0] | dark |
| k_shin_mark_l | shin_l | bevel_box | {"size": [1.56, 1.15, 0.25], "bevel": 0.08} | [0.1, -1.8, -1.176] | [0, 0, 0] | accent |
| k_abdomen | spine | plate | {"points_xy": [[-2.4, -2.7], [2.4, -2.7], [3.3, 2.5], [-2, 2.5]], "thickness": 5.5} | [0, 0, 0] | [0, 0, 0] | secondary |
| k_chest | chest | plate | {"points_xy": [[-2.6, -2.5], [2.8, -2.5], [3.6, 2.8], [-1.7, 3.5]], "thickness": 6.7} | [0, 0, 0] | [0, 0, 0] | primary |
| k_chest_stripe | chest | bevel_box | {"size": [1.1, 4.5, 0.35], "bevel": 0.1} | [1.5, 0.4, 3.5] | [0, 0, -12] | accent |
| k_head | head | ellipsoid | {"radii": [2.65, 2.4, 2.5], "lat": 4, "lon": 8} | [0.1, 0.2, 0] | [0, 0, 0] | light |
| k_face | head | plate | {"points_xy": [[1.5, -1], [3.15, -0.45], [3.2, 0.6], [1.6, 1.1]], "thickness": 3.8} | [0, 0, 0] | [0, 0, 0] | skin |
| k_eye | head | bevel_box | {"size": [1.55, 0.45, 4.35], "bevel": 0.1} | [2.0, 0.75, 0] | [0, 0, 0] | dark |
| k_crest_0 | crest | plate | {"points_xy": [[-1, 0], [1.1, 0], [-0.5, 2.8]], "thickness": 0.65} | [-1.6, 0.1, -0.9] | [0, 0, 0] | accent |
| k_crest_1 | crest | plate | {"points_xy": [[-1, 0], [1.1, 0], [-0.5, 2.8]], "thickness": 0.65} | [-0.3, 0, 0] | [0, 0, 0] | accent |
| k_crest_2 | crest | plate | {"points_xy": [[-1, 0], [1.1, 0], [-0.5, 2.8]], "thickness": 0.65} | [1, -0.3, 0.9] | [0, 0, 0] | accent |
| k_scarf_a | scarf_a | plate | {"points_xy": [[0, 0], [-3.4, -0.3], [-3.6, -1.9], [-0.3, -1.3]], "thickness": 2.1} | [0, 0, 0] | [0, 0, 0] | accent |
| k_scarf_b | scarf_b | plate | {"points_xy": [[0, 0], [-3, -0.4], [-3.2, -2.0], [-0.3, -1.3]], "thickness": 1.5} | [0, 0, 0] | [0, 0, 0] | extra |

## Retardo de extras

| Hueso | Padre | Retardo ticks | Ganancia | Límite ° | Eje |
|---|---|---:|---:|---:|---|
| crest | head | 1 | 0.25 | 2 | z |
| scarf_a | neck | 3 | 0.55 | 9 | z |
| scarf_b | scarf_a | 5 | 0.45 | 11 | z |

## Comprobación numérica de autoría

31 piezas; 21 huesos. Envolvente conservadora en reposo: mínimo [-8.2, 0.2, -4.9925], máximo [3.6, 29.9, 4.9925]. Dentro de [−semiancho,+semiancho] × [0,alto]: True. No es una ejecución del juego ni verifica poses de ataque/cápsulas animadas.

El radio real no describe una caja. Superponer cápsula de runtime y medir intersección de volumen principal en idle/crouch/hitstun; accesorios y extremidades en ataques se revisan visualmente. No cambiar daño, hitboxes ni hurtboxes para acomodar una pieza.
