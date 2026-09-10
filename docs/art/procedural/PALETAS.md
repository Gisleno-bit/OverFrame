# 2. Paletas definitivas — seis por personaje

Índices 0–5. Los ocho slots se entregan en sRGB hexadecimal y RGB8 en palettes.json. Alpha 255. No crear una séptima opción ni cambiar protocolo/selector. Glow es color de albedo; no añadir bloom a toda la figura.

| Personaje | Índice / nombre | primary | secondary | accent | skin | dark | glow | light | extra |
|---|---|---|---|---|---|---|---|---|---|
| kestrel | 0 Coral | #F16F54 | #20364B | #F6D889 | #D3DAD7 | #111A26 | #A7F3ED | #F3EEE0 | #914C48 |
| kestrel | 1 Escarcha | #C4E9DF | #263E51 | #E89B51 | #B5CDD0 | #121C29 | #9FFFE6 | #F7F2E4 | #427D87 |
| kestrel | 2 Azafrán | #E1B83E | #352E4B | #EDE4C8 | #C1CBCB | #181623 | #B5E7F2 | #F4EDDB | #A5743A |
| kestrel | 3 Abisal | #24384F | #B3D9DF | #F48766 | #B5C5C9 | #0D1420 | #99F0F4 | #EAF0E8 | #496A80 |
| kestrel | 4 Ciruela | #62406F | #D8CEE5 | #ECAA74 | #C9BFCE | #171322 | #D0C1FF | #F4EAF2 | #9569A0 |
| kestrel | 5 Pino | #293F35 | #B5CFAF | #E8DCA1 | #B9CBC0 | #0F1A17 | #B4EDCE | #F0EDDA | #58755B |
| boulder | 0 Ocre | #CBA559 | #343D42 | #EBE0B6 | #AFB8AC | #151C21 | #B9E7CA | #EEEAD7 | #8C7757 |
| boulder | 1 Caliza | #D9D5BF | #3E4B51 | #CE8054 | #A8B7B1 | #161D22 | #C1EADB | #F5F0DC | #949B91 |
| boulder | 2 Cobre | #DE8860 | #343846 | #EBD196 | #B9C1B4 | #171B25 | #B7DBE5 | #F0E9D8 | #985C43 |
| boulder | 3 Basalto | #303941 | #B7C2BA | #E8BF6A | #A7BDB3 | #11171C | #9DEBD7 | #E8EBDD | #697B7D |
| boulder | 4 Pizarra | #35476A | #CAD3E0 | #D7AD66 | #AEBBC6 | #111A2C | #B4DAF4 | #EDF1ED | #70839C |
| boulder | 5 Turba | #354735 | #CDD7B1 | #DBA56C | #B1C1AD | #131C13 | #C7EDB1 | #EDF0DD | #748564 |
| viper | 0 Menta | #7ED5B8 | #392D49 | #DCEACF | #B5D7C5 | #151622 | #C1FFF1 | #EEF4E7 | #507A76 |
| viper | 1 Nácar | #DADDE8 | #304051 | #A5DAB3 | #C3D0D5 | #151D2B | #C7EFFF | #F5F2EA | #879FB7 |
| viper | 2 Ámbar | #EDC26B | #3C3145 | #A4DED0 | #D5CFB5 | #1B1723 | #D9F4BA | #F6EEDC | #A57952 |
| viper | 3 Aubergine | #422F54 | #C9BDD7 | #8BE2BE | #BBAFC7 | #15111E | #D6B6FF | #EEE6F3 | #806292 |
| viper | 4 Noche | #223D49 | #BDD8D7 | #E6BD77 | #ABCBCC | #101D26 | #A9ECEE | #EAF0E5 | #477D85 |
| viper | 5 Bosque | #254A3D | #BDD7B5 | #F0D07E | #ADCAB7 | #0D2018 | #B4EBCB | #EDF2DC | #599C76 |
| trama | 0 Marfil | #E4DDC6 | #344365 | #D88A59 | #C2CACB | #171B2C | #C1E7FF | #F7F1DF | #967548 |
| trama | 1 Celeste | #A2D5E7 | #30354E | #E9CF86 | #B6CAD3 | #141B2C | #B2F1FF | #F0F2E8 | #658CA1 |
| trama | 2 Salvia | #B7D5AC | #354442 | #EFC590 | #B9CDBE | #17201F | #CBEFDB | #F1F1DE | #799271 |
| trama | 3 Índigo | #353D67 | #D8D8E8 | #E6AB77 | #B6BECC | #13172C | #C5D5FF | #F1EEDF | #7C85A4 |
| trama | 4 Óxido | #643B35 | #E2C7AE | #ACD9D3 | #CBBBAE | #231514 | #C0EFE5 | #F4E8D9 | #A77653 |
| trama | 5 Petróleo | #25464C | #BBD6D6 | #ECCC94 | #B5C6C4 | #112024 | #B9EDF0 | #EDF0DF | #5D9395 |

## Verificación de mirror matches

Se han calculado las 15 combinaciones por personaje (60 pares) con luminancia relativa sRGB linealizada. Esto mide el contraste entre los slots primary, no certifica el resultado 3D, daltonismo ni el contraste de todas las superficies iluminadas. El informe completo está en palette-contrast.json.

| Personaje | 0 / 3 | 1 / 4 | 2 / 5 | Mínimo entre los 15 pares |
|---|---:|---:|---:|---:|
| kestrel | 4.073:1 | 6.473:1 | 5.993:1 | 1.059:1 |
| boulder | 5.072:1 | 6.29:1 | 3.705:1 | 1.074:1 |
| viper | 6.849:1 | 8.456:1 | 5.895:1 | 1.036:1 |
| trama | 7.672:1 | 5.953:1 | 6.375:1 | 1.006:1 |

Los emparejamientos recomendados son 0↔3, 1↔4 y 2↔5. Objetivo de autoría para estos pares: >=3:1 entre primary de ambos. No afirmar que cualquier combinación de seis colores alcanza 3:1: varias no lo hacen. No bloquear selecciones existentes ni introducir reglas de red por arte. Si ambos eligen igual, la elección se respeta y se añade identificación redundante.

Identificación siempre visible: P1 dentro de círculo y P2 dentro de rombo, ambos blanco #F4F1E8 sobre placa #101923. Etiqueta encima de cabeza, ancho40 alto24 px a1080p, fuente16 px, separación vertical8 px sobre bounds de cabeza; nunca sobre la hitbox. Si se solapan etiquetas, P1 se desplaza24px a izquierda y P2 a derecha. Usar el índice canónico del jugador de la sesión, no «local/remoto», para que ambas pantallas coincidan. La forma también aparece en HUD y selección.

Aceptación real: renderizar cada mirror recomendado en reposo y ataque, con luz del escenario, en color y escala de grises; no perder identidad a72px de alto. Revisar pares no recomendados con marcadores visibles y simulación de deficiencias cromáticas si se dispone de ella. Esa revisión todavía no se ha ejecutado. No añadir outlines de colores intensos que tapen el contacto.

Fórmula de contraste consultada como medida numérica; las pautas de texto no son una norma de contraste de personajes 3D: https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
