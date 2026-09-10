# Viper — piezas procedurales v1

Perfil curvo por tres cuñas convexas de capucha; torso estrecho y cola baja de tres segmentos. No túnica ni cabello de alta frecuencia.

Referencia: alto 27; semiancho de tabla 7.5; radio hurt de referencia 9.0. Estado: `uploaded_snapshot_not_live_verified`.

Leer FORMAT.md antes de implementar. Todas las medidas son unidades de juego; rotaciones XYZ en grados; r/l son profundidad +Z/-Z, no posiciones opuestas en X. Rest pose neutra. JSON es la fuente numérica. No reescalar el modelo automáticamente.

## Huesos

| Hueso | Padre | Offset local XYZ |
|---|---|---|
| root | — | [0, 0, 0] |
| hips | root | [0, 12.8, 0] |
| spine | hips | [0, 2.79, 0] |
| chest | spine | [0, 3.255, 0] |
| neck | chest | [0, 3.255, 0] |
| head | neck | [0, 0.7, 0] |
| upper_arm_r | chest | [0, 2.604, 3.15] |
| forearm_r | upper_arm_r | [0, -4.5, 0] |
| hand_r | forearm_r | [0, -4, 0] |
| thigh_r | hips | [0, 0, 1.8] |
| shin_r | thigh_r | [0, -6, 0] |
| foot_r | shin_r | [0, -5.8, 0] |
| upper_arm_l | chest | [0, 2.604, -3.15] |
| forearm_l | upper_arm_l | [0, -4.5, 0] |
| hand_l | forearm_l | [0, -4, 0] |
| thigh_l | hips | [0, 0, -1.8] |
| shin_l | thigh_l | [0, -6, 0] |
| foot_l | shin_l | [0, -5.8, 0] |
| hood | head | [-0.3, 0.1, 0] |
| tail_a | hips | [-1.6, -1.5, 0] |
| tail_b | tail_a | [-2.4, -1, 0] |
| tail_c | tail_b | [-1.9, -1.7, 0] |

## Piezas

| ID | Hueso | Primitiva | Parámetros | Posición XYZ | Rotación XYZ° | Slot |
|---|---|---|---|---|---|---|
| v_pelvis | hips | bevel_box | {"size": [4.5, 3, 5], "bevel": 0.45} | [0, 0, 0] | [0, 0, 0] | secondary |
| v_neck | neck | cylinder | {"r0": 1.1, "r1": 0.9, "height": 1.7, "segments": 8} | [0, 0.1, 0] | [0, 0, 0] | dark |
| v_upper_arm_r | upper_arm_r | bevel_box | {"size": [1.9, 4.7, 1.672], "bevel": 0.304} | [0, -2.25, 0] | [0, 0, 0] | secondary |
| v_forearm_r | forearm_r | bevel_box | {"size": [2.3, 4.2, 2.024], "bevel": 0.368} | [0, -2.0, 0] | [0, 0, 0] | primary |
| v_thigh_r | thigh_r | bevel_box | {"size": [2.5, 6.2, 2.2], "bevel": 0.4} | [0, -3.0, 0] | [0, 0, 0] | secondary |
| v_shin_r | shin_r | bevel_box | {"size": [2.05, 6.0, 1.804], "bevel": 0.328} | [0, -2.9, 0] | [0, 0, 0] | primary |
| v_elbow_r | forearm_r | ellipsoid | {"radii": [0.874, 0.874, 0.874], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| v_knee_r | shin_r | ellipsoid | {"radii": [0.984, 0.984, 0.984], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| v_hand_r | hand_r | bevel_box | {"size": [2.116, 1.7, 2.07], "bevel": 0.25} | [0.25, -0.65, 0] | [0, 0, 0] | light |
| v_foot_r | foot_r | bevel_box | {"size": [3.8, 1.55, 2.35], "bevel": 0.25} | [0.65, -0.1, 0] | [0, 0, 0] | dark |
| v_shin_mark_r | shin_r | bevel_box | {"size": [1.3325, 1.15, 0.25], "bevel": 0.08} | [0.1, -1.74, 1.022] | [0, 0, 0] | accent |
| v_upper_arm_l | upper_arm_l | bevel_box | {"size": [1.9, 4.7, 1.672], "bevel": 0.304} | [0, -2.25, 0] | [0, 0, 0] | secondary |
| v_forearm_l | forearm_l | bevel_box | {"size": [2.3, 4.2, 2.024], "bevel": 0.368} | [0, -2.0, 0] | [0, 0, 0] | primary |
| v_thigh_l | thigh_l | bevel_box | {"size": [2.5, 6.2, 2.2], "bevel": 0.4} | [0, -3.0, 0] | [0, 0, 0] | secondary |
| v_shin_l | shin_l | bevel_box | {"size": [2.05, 6.0, 1.804], "bevel": 0.328} | [0, -2.9, 0] | [0, 0, 0] | primary |
| v_elbow_l | forearm_l | ellipsoid | {"radii": [0.874, 0.874, 0.874], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| v_knee_l | shin_l | ellipsoid | {"radii": [0.984, 0.984, 0.984], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| v_hand_l | hand_l | bevel_box | {"size": [2.116, 1.7, 2.07], "bevel": 0.25} | [0.25, -0.65, 0] | [0, 0, 0] | light |
| v_foot_l | foot_l | bevel_box | {"size": [3.8, 1.55, 2.35], "bevel": 0.25} | [0.65, -0.1, 0] | [0, 0, 0] | dark |
| v_shin_mark_l | shin_l | bevel_box | {"size": [1.3325, 1.15, 0.25], "bevel": 0.08} | [0.1, -1.74, -1.022] | [0, 0, 0] | accent |
| v_abdomen | spine | plate | {"points_xy": [[-1.8, -2.8], [1.8, -2.8], [2.45, 2.3], [-2.1, 2.3]], "thickness": 4.4} | [0, 0, 0] | [0, 0, 0] | secondary |
| v_chest | chest | plate | {"points_xy": [[-2.4, -2.3], [2.2, -2.3], [2.7, 2.5], [0, 3.3], [-2.6, 2]], "thickness": 5.6} | [0, 0, 0] | [0, 0, 0] | primary |
| v_head | head | ellipsoid | {"radii": [2.2, 2.1, 2.2], "lat": 4, "lon": 8} | [0.3, 0.2, 0] | [0, 0, 0] | skin |
| v_hood_back | hood | plate | {"points_xy": [[-2.8, -2.6], [-1.8, -2.5], [-1.5, 2.6], [-2.9, 1.5]], "thickness": 5.6} | [0, 0, 0] | [0, 0, 0] | primary |
| v_hood_top | hood | plate | {"points_xy": [[-2.9, 1.5], [2.6, 1.6], [1.8, 3.7], [-1.6, 3.9]], "thickness": 5.6} | [0, 0, 0] | [0, 0, 0] | primary |
| v_hood_rim_r | hood | plate | {"points_xy": [[-1.9, -2.5], [-0.6, -2.7], [1.8, 2.4], [-1.5, 2.6]], "thickness": 0.55} | [0, 0, 2.65] | [0, 0, 0] | accent |
| v_hood_rim_l | hood | plate | {"points_xy": [[-1.9, -2.5], [-0.6, -2.7], [1.8, 2.4], [-1.5, 2.6]], "thickness": 0.55} | [0, 0, -2.65] | [0, 0, 0] | accent |
| v_eye | head | bevel_box | {"size": [2, 0.4, 4.3], "bevel": 0.08} | [1.25, 0.7, 0] | [0, 0, 0] | dark |
| v_tail_a | tail_a | plate | {"points_xy": [[0, 0.7], [-2.5, -0.6], [-2.7, -2], [-0.1, -0.6]], "thickness": 1.9} | [0, 0, 0] | [0, 0, 0] | primary |
| v_tail_b | tail_b | plate | {"points_xy": [[0, 0.5], [-2.1, -1.2], [-2.2, -2.5], [-0.1, -0.6]], "thickness": 1.45} | [0, 0, 0] | [0, 0, 0] | accent |
| v_tail_c | tail_c | plate | {"points_xy": [[0, 0.4], [-0.7, -1.6], [0.4, -3.4], [0.7, -1.5]], "thickness": 0.95} | [0, 0, 0] | [0, 0, 0] | extra |

## Retardo de extras

| Hueso | Padre | Retardo ticks | Ganancia | Límite ° | Eje |
|---|---|---:|---:|---:|---|
| hood | head | 1 | 0.15 | 2 | z |
| tail_a | hips | 3 | 0.4 | 7 | z |
| tail_b | tail_a | 5 | 0.35 | 9 | z |
| tail_c | tail_b | 7 | 0.3 | 10 | z |

## Comprobación numérica de autoría

31 piezas; 22 huesos. Envolvente conservadora en reposo: mínimo [-6.6, 0.125, -4.185], máximo [2.7, 26.8, 4.185]. Dentro de [−semiancho,+semiancho] × [0,alto]: True. No es una ejecución del juego ni verifica poses de ataque/cápsulas animadas.

El radio real no describe una caja. Superponer cápsula de runtime y medir intersección de volumen principal en idle/crouch/hitstun; accesorios y extremidades en ataques se revisan visualmente. No cambiar daño, hitboxes ni hurtboxes para acomodar una pieza.
