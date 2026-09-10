# Trama — piezas procedurales v1

Agrimensor de marcos: cuerpo vertical, cabeza hexagonal plana, faldón dividido y brazales rectangulares abiertos. Los marcos son ornamentales; no escudos ni nuevas colisiones.

Referencia: alto 32; semiancho de tabla 9.5; radio hurt de referencia 11.0. Estado: `proposed_requires_sim_table_and_tests`.

Leer FORMAT.md antes de implementar. Todas las medidas son unidades de juego; rotaciones XYZ en grados; r/l son profundidad +Z/-Z, no posiciones opuestas en X. Rest pose neutra. JSON es la fuente numérica. No reescalar el modelo automáticamente.

## Huesos

| Hueso | Padre | Offset local XYZ |
|---|---|---|
| root | — | [0, 0, 0] |
| hips | root | [0, 14.0, 0] |
| spine | hips | [0, 3.36, 0] |
| chest | spine | [0, 3.92, 0] |
| neck | chest | [0, 3.92, 0] |
| head | neck | [0, 1, 0] |
| upper_arm_r | chest | [0, 3.136, 4.25] |
| forearm_r | upper_arm_r | [0, -5, 0] |
| hand_r | forearm_r | [0, -4.5, 0] |
| thigh_r | hips | [0, 0, 2.7] |
| shin_r | thigh_r | [0, -6.5, 0] |
| foot_r | shin_r | [0, -6.3, 0] |
| upper_arm_l | chest | [0, 3.136, -4.25] |
| forearm_l | upper_arm_l | [0, -5, 0] |
| hand_l | forearm_l | [0, -4.5, 0] |
| thigh_l | hips | [0, 0, -2.7] |
| shin_l | thigh_l | [0, -6.5, 0] |
| foot_l | shin_l | [0, -6.3, 0] |
| crown | head | [-0.6, 2.6, 0] |
| apron_r | hips | [-1.25, -1, 2] |
| brace_r | forearm_r | [3, -2.25, 2.0] |
| apron_l | hips | [-1.25, -1, -2] |
| brace_l | forearm_l | [-3, -2.25, -2.0] |

## Piezas

| ID | Hueso | Primitiva | Parámetros | Posición XYZ | Rotación XYZ° | Slot |
|---|---|---|---|---|---|---|
| t_pelvis | hips | bevel_box | {"size": [4.5, 3, 5], "bevel": 0.45} | [0, 0, 0] | [0, 0, 0] | secondary |
| t_neck | neck | cylinder | {"r0": 1.1, "r1": 0.9, "height": 2, "segments": 8} | [0, 0.1, 0] | [0, 0, 0] | dark |
| t_upper_arm_r | upper_arm_r | bevel_box | {"size": [2.5, 5.2, 2.2], "bevel": 0.4} | [0, -2.5, 0] | [0, 0, 0] | secondary |
| t_forearm_r | forearm_r | bevel_box | {"size": [2.8, 4.7, 2.464], "bevel": 0.448} | [0, -2.25, 0] | [0, 0, 0] | primary |
| t_thigh_r | thigh_r | bevel_box | {"size": [3, 6.7, 2.64], "bevel": 0.45} | [0, -3.25, 0] | [0, 0, 0] | secondary |
| t_shin_r | shin_r | bevel_box | {"size": [2.55, 6.5, 2.244], "bevel": 0.408} | [0, -3.15, 0] | [0, 0, 0] | primary |
| t_elbow_r | forearm_r | ellipsoid | {"radii": [1.15, 1.15, 1.15], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| t_knee_r | shin_r | ellipsoid | {"radii": [1.224, 1.224, 1.224], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| t_hand_r | hand_r | bevel_box | {"size": [2.576, 1.7, 2.52], "bevel": 0.25} | [0.25, -0.65, 0] | [0, 0, 0] | light |
| t_foot_r | foot_r | bevel_box | {"size": [4.5, 1.9, 3], "bevel": 0.25} | [0.65, -0.1, 0] | [0, 0, 0] | dark |
| t_shin_mark_r | shin_r | bevel_box | {"size": [1.6575, 1.15, 0.25], "bevel": 0.08} | [0.1, -1.89, 1.242] | [0, 0, 0] | accent |
| t_upper_arm_l | upper_arm_l | bevel_box | {"size": [2.5, 5.2, 2.2], "bevel": 0.4} | [0, -2.5, 0] | [0, 0, 0] | secondary |
| t_forearm_l | forearm_l | bevel_box | {"size": [2.8, 4.7, 2.464], "bevel": 0.448} | [0, -2.25, 0] | [0, 0, 0] | primary |
| t_thigh_l | thigh_l | bevel_box | {"size": [3, 6.7, 2.64], "bevel": 0.45} | [0, -3.25, 0] | [0, 0, 0] | secondary |
| t_shin_l | shin_l | bevel_box | {"size": [2.55, 6.5, 2.244], "bevel": 0.408} | [0, -3.15, 0] | [0, 0, 0] | primary |
| t_elbow_l | forearm_l | ellipsoid | {"radii": [1.15, 1.15, 1.15], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| t_knee_l | shin_l | ellipsoid | {"radii": [1.224, 1.224, 1.224], "lat": 4, "lon": 8} | [0, 0, 0] | [0, 0, 0] | dark |
| t_hand_l | hand_l | bevel_box | {"size": [2.576, 1.7, 2.52], "bevel": 0.25} | [0.25, -0.65, 0] | [0, 0, 0] | light |
| t_foot_l | foot_l | bevel_box | {"size": [4.5, 1.9, 3], "bevel": 0.25} | [0.65, -0.1, 0] | [0, 0, 0] | dark |
| t_shin_mark_l | shin_l | bevel_box | {"size": [1.6575, 1.15, 0.25], "bevel": 0.08} | [0.1, -1.89, -1.242] | [0, 0, 0] | accent |
| t_abdomen | spine | bevel_box | {"size": [5.5, 6.5, 5.5], "bevel": 0.4} | [0, -0.5, 0] | [0, 0, 0] | secondary |
| t_chest | chest | plate | {"points_xy": [[-3, -2.5], [3, -2.5], [3, 3.5], [1.4, 4], [-2.8, 3.5]], "thickness": 6.7} | [0, 0, 0] | [0, 0, 0] | primary |
| t_head | head | plate | {"points_xy": [[-2.6, -1.8], [1.7, -2.2], [2.8, -0.5], [2.5, 2], [0, 2.7], [-2.6, 1.6]], "thickness": 4.8} | [0.3, 0.1, 0] | [0, 0, 0] | light |
| t_face | head | bevel_box | {"size": [1.0, 1.9, 3.6], "bevel": 0.18} | [2.55, 0.1, 0] | [0, 0, 0] | skin |
| t_eye | head | bevel_box | {"size": [0.25, 0.5, 3.1], "bevel": 0.04} | [3.1, 0.6, 0] | [0, 0, 0] | dark |
| t_crown | crown | bevel_box | {"size": [2.8, 2.6, 1.3], "bevel": 0.2} | [0, 1.1, 0] | [0, 0, 0] | accent |
| t_apron_r | apron_r | plate | {"points_xy": [[-1.4, -5.3], [1.4, -4.9], [1.6, 0], [-1.6, 0]], "thickness": 0.65} | [0, 0, 0] | [0, 0, 0] | primary |
| t_brace_r_top | brace_r | bevel_box | {"size": [7.2, 1, 1.15], "bevel": 0.15} | [0, 3, 0] | [0, 0, 0] | extra |
| t_brace_r_bottom | brace_r | bevel_box | {"size": [7.2, 1, 1.15], "bevel": 0.15} | [0, -3, 0] | [0, 0, 0] | extra |
| t_brace_r_back | brace_r | bevel_box | {"size": [1, 5, 1.15], "bevel": 0.15} | [-3.1, 0, 0] | [0, 0, 0] | extra |
| t_brace_r_front | brace_r | bevel_box | {"size": [1, 5, 1.15], "bevel": 0.15} | [3.1, 0, 0] | [0, 0, 0] | extra |
| t_brace_r_key | brace_r | bevel_box | {"size": [1.8, 0.5, 1.45], "bevel": 0.08} | [0, 3.1, 0] | [0, 0, 0] | accent |
| t_apron_l | apron_l | plate | {"points_xy": [[-1.4, -5.3], [1.4, -4.9], [1.6, 0], [-1.6, 0]], "thickness": 0.65} | [0, 0, 0] | [0, 0, 0] | primary |
| t_brace_l_top | brace_l | bevel_box | {"size": [7.2, 1, 1.15], "bevel": 0.15} | [0, 3, 0] | [0, 0, 0] | extra |
| t_brace_l_bottom | brace_l | bevel_box | {"size": [7.2, 1, 1.15], "bevel": 0.15} | [0, -3, 0] | [0, 0, 0] | extra |
| t_brace_l_back | brace_l | bevel_box | {"size": [1, 5, 1.15], "bevel": 0.15} | [-3.1, 0, 0] | [0, 0, 0] | extra |
| t_brace_l_front | brace_l | bevel_box | {"size": [1, 5, 1.15], "bevel": 0.15} | [3.1, 0, 0] | [0, 0, 0] | extra |
| t_brace_l_key | brace_l | bevel_box | {"size": [1.8, 0.5, 1.45], "bevel": 0.08} | [0, 3.1, 0] | [0, 0, 0] | accent |

## Retardo de extras

| Hueso | Padre | Retardo ticks | Ganancia | Límite ° | Eje |
|---|---|---:|---:|---:|---|
| crown | head | 0 | 0 | 0 | z |
| apron_r | hips | 3 | 0.3 | 5 | z |
| brace_r | forearm_r | 0 | 0 | 0 | z |
| apron_l | hips | 3 | 0.3 | 5 | z |
| brace_l | forearm_l | 0 | 0 | 0 | z |

## Comprobación numérica de autoría

38 piezas; 23 huesos. Envolvente conservadora en reposo: mínimo [-6.6, 0.15, -6.975], máximo [6.6, 31.2, 6.975]. Dentro de [−semiancho,+semiancho] × [0,alto]: True. No es una ejecución del juego ni verifica poses de ataque/cápsulas animadas.

El radio real no describe una caja. Superponer cápsula de runtime y medir intersección de volumen principal en idle/crouch/hitstun; accesorios y extremidades en ataques se revisan visualmente. No cambiar daño, hitboxes ni hurtboxes para acomodar una pieza.
