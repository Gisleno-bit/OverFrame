# Boulder — piezas procedurales v1

Mampostería articulada: abdomen y antebrazos de bloques; cabeza pequeña hundida, tres piedras superiores. Silueta rectangular escalonada.

Referencia: alto 36; semiancho de tabla 12; radio hurt de referencia 13.5. Estado: `uploaded_snapshot_not_live_verified`.

Leer FORMAT.md antes de implementar. Todas las medidas son unidades de juego; rotaciones XYZ en grados; r/l son profundidad +Z/-Z, no posiciones opuestas en X. Rest pose neutra. JSON es la fuente numérica. No reescalar el modelo automáticamente.

## Huesos

| Hueso | Padre | Offset local XYZ |
|---|---|---|
| root | — | [0, 0, 0] |
| hips | root | [0, 14.6, 0] |
| spine | hips | [0, 4.2, 0] |
| chest | spine | [0, 4.9, 0] |
| neck | chest | [0, 4.9, 0] |
| head | neck | [0, 0.4, 0] |
| upper_arm_r | chest | [0, 3.92, 5.4] |
| forearm_r | upper_arm_r | [0, -6, 0] |
| hand_r | forearm_r | [0, -5, 0] |
| thigh_r | hips | [0, 0, 3.4] |
| shin_r | thigh_r | [0, -7, 0] |
| foot_r | shin_r | [0, -6, 0] |
| upper_arm_l | chest | [0, 3.92, -5.4] |
| forearm_l | upper_arm_l | [0, -6, 0] |
| hand_l | forearm_l | [0, -5, 0] |
| thigh_l | hips | [0, 0, -3.4] |
| shin_l | thigh_l | [0, -7, 0] |
| foot_l | shin_l | [0, -6, 0] |
| stone_0 | chest | [-6, 8, 0] |
| stone_1 | chest | [0, 9.2, -2.2] |
| stone_2 | chest | [6, 7.5, 0] |

## Piezas

| ID | Hueso | Primitiva | Parámetros | Posición XYZ | Rotación XYZ° | Slot |
|---|---|---|---|---|---|---|
| b_pelvis | hips | bevel_box | {"size": [7, 3, 7], "bevel": 0.45} | [0, 0, 0] | [0, 0, 0] | secondary |
| b_neck | neck | cylinder | {"r0": 1.1, "r1": 0.9, "height": 1.4, "segments": 8} | [0, 0.1, 0] | [0, 0, 0] | dark |
| b_upper_arm_r | upper_arm_r | bevel_box | {"size": [4.8, 6.2, 4.224], "bevel": 0.45} | [0, -3.0, 0] | [0, 0, 0] | secondary |
| b_forearm_r | forearm_r | bevel_box | {"size": [6.8, 5.2, 5.984], "bevel": 0.45} | [0, -2.5, 0] | [0, 0, 0] | primary |
| b_thigh_r | thigh_r | bevel_box | {"size": [4.4, 7.2, 3.872], "bevel": 0.45} | [0, -3.5, 0] | [0, 0, 0] | secondary |
| b_shin_r | shin_r | bevel_box | {"size": [4.2, 6.2, 3.696], "bevel": 0.45} | [0, -3.0, 0] | [0, 0, 0] | primary |
| b_elbow_r | forearm_r | ellipsoid | {"radii": [2.208, 2.208, 2.208], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| b_knee_r | shin_r | ellipsoid | {"radii": [2.016, 2.016, 2.016], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| b_hand_r | hand_r | bevel_box | {"size": [6.256, 1.7, 6.12], "bevel": 0.25} | [0.25, -0.65, 0] | [0, 0, 0] | light |
| b_foot_r | foot_r | bevel_box | {"size": [6, 2.5, 4.4], "bevel": 0.25} | [0.65, -0.1, 0] | [0, 0, 0] | dark |
| b_shin_mark_r | shin_r | bevel_box | {"size": [2.73, 1.15, 0.25], "bevel": 0.08} | [0.1, -1.8, 1.968] | [0, 0, 0] | accent |
| b_upper_arm_l | upper_arm_l | bevel_box | {"size": [4.8, 6.2, 4.224], "bevel": 0.45} | [0, -3.0, 0] | [0, 0, 0] | secondary |
| b_forearm_l | forearm_l | bevel_box | {"size": [6.8, 5.2, 5.984], "bevel": 0.45} | [0, -2.5, 0] | [0, 0, 0] | primary |
| b_thigh_l | thigh_l | bevel_box | {"size": [4.4, 7.2, 3.872], "bevel": 0.45} | [0, -3.5, 0] | [0, 0, 0] | secondary |
| b_shin_l | shin_l | bevel_box | {"size": [4.2, 6.2, 3.696], "bevel": 0.45} | [0, -3.0, 0] | [0, 0, 0] | primary |
| b_elbow_l | forearm_l | ellipsoid | {"radii": [2.208, 2.208, 2.208], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| b_knee_l | shin_l | ellipsoid | {"radii": [2.016, 2.016, 2.016], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| b_hand_l | hand_l | bevel_box | {"size": [6.256, 1.7, 6.12], "bevel": 0.25} | [0.25, -0.65, 0] | [0, 0, 0] | light |
| b_foot_l | foot_l | bevel_box | {"size": [6, 2.5, 4.4], "bevel": 0.25} | [0.65, -0.1, 0] | [0, 0, 0] | dark |
| b_shin_mark_l | shin_l | bevel_box | {"size": [2.73, 1.15, 0.25], "bevel": 0.08} | [0.1, -1.8, -1.968] | [0, 0, 0] | accent |
| b_abdomen | spine | bevel_box | {"size": [10.5, 8, 9.3], "bevel": 1} | [0, 0, 0] | [0, 0, 0] | secondary |
| b_chest | chest | plate | {"points_xy": [[-6, -3], [5.8, -3], [7, 2.3], [4.8, 5], [-4.8, 5], [-6.8, 2.2]], "thickness": 11} | [0, 0, 0] | [0, 0, 0] | primary |
| b_seam | chest | bevel_box | {"size": [1.15, 6.2, 0.35], "bevel": 0.12} | [0.5, 0.2, 5.7] | [0, 0, 0] | accent |
| b_head | head | bevel_box | {"size": [4.8, 4.3, 4.4], "bevel": 0.75} | [1, -0.1, 0] | [0, 0, 0] | light |
| b_face | head | bevel_box | {"size": [1.0, 1.5, 3.7], "bevel": 0.25} | [3.2, -0.1, 0] | [0, 0, 0] | skin |
| b_eye | head | bevel_box | {"size": [0.3, 0.48, 3.3], "bevel": 0.06} | [3.75, 0.3, 0] | [0, 0, 0] | dark |
| b_stone_0 | stone_0 | bevel_box | {"size": [4.3, 4.5, 4.5], "bevel": 0.65} | [0, 0, 0] | [0, 0, -8] | primary |
| b_stone_1 | stone_1 | bevel_box | {"size": [3.8, 5.8, 3.8], "bevel": 0.65} | [0, 0, 0] | [0, 0, 0] | extra |
| b_stone_2 | stone_2 | bevel_box | {"size": [4, 4.6, 4.2], "bevel": 0.65} | [0, 0, 0] | [0, 0, 8] | primary |
| b_fist_cap_r | forearm_r | bevel_box | {"size": [6.2, 1.2, 6.5], "bevel": 0.35} | [0, -3.7, 0] | [0, 0, 0] | accent |
| b_fist_cap_l | forearm_l | bevel_box | {"size": [6.2, 1.2, 6.5], "bevel": 0.35} | [0, -3.7, 0] | [0, 0, 0] | accent |

## Retardo de extras

| Hueso | Padre | Retardo ticks | Ganancia | Límite ° | Eje |
|---|---|---:|---:|---:|---|
| stone_0 | chest | 2 | 0.25 | 3 | z |
| stone_1 | chest | 3 | 0.25 | 3 | z |
| stone_2 | chest | 4 | 0.25 | 3 | z |

## Comprobación numérica de autoría

31 piezas; 21 huesos. Envolvente conservadora en reposo: mínimo [-8.44222, 0.25, -8.65], máximo [8.30063, 35.8, 8.65]. Dentro de [−semiancho,+semiancho] × [0,alto]: True. No es una ejecución del juego ni verifica poses de ataque/cápsulas animadas.

El radio real no describe una caja. Superponer cápsula de runtime y medir intersección de volumen principal en idle/crouch/hitstun; accesorios y extremidades en ataques se revisan visualmente. No cambiar daño, hitboxes ni hurtboxes para acomodar una pieza.
