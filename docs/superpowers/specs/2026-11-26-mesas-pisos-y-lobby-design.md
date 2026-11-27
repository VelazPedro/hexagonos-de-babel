# Mejoras de las mesas, variedad por piso y lobby limpio

Aprobado por el autor el 26/11/2026 («aceptar ambas propuestas», más el mismo cambio para las
otras mesas y que lo roto en el hexágono base no vuelva).

## Para qué
- La Forja no se distinguía de la Mesa de encuadernador. Cada mesa que fabrica tiene que ofrecer
  algo propio.
- Recolectar se volvía repetitivo: desde cierto piso, todos los hexágonos traían lo mismo.
- Bajar de piso tiene que pedir un logro de fabricación.
- Lo despejado en el hexágono base queda despejado. Más adelante se podrá personalizar (fuera de
  alcance ahora).

## Mejoras de mesa
En cada **objeto** de una mesa se elige **una** mejora, de nivel I a III. Sin azar. Subir o
cambiar se paga con materiales; **cambiar conserva el nivel** y cuesta lo que ese nivel. Quedan
activas siempre, se tenga lo que se tenga en la mano. Cada jugador tiene las suyas.

| Mesa | Objeto | Opciones |
|---|---|---|
| Forja | Cuchilla | quemar · robar vida · repetir el golpe |
| Forja | Martillo | más materiales · romper más rápido · más empuje |
| Forja | Lámpara | más luz · más daño en lo oscuro · recuperar vida |
| Forja | Lente | saber al leer · atraer objetos · experiencia |
| Mesa de encuadernador | Cuaderno | defensa · vida · velocidad |
| Mesa de encuadernador | Páginas | más daño · quemar · menos energía |
| Mesa de encuadernador | Antorchas | más alcance · curar cerca · quemar enemigos cerca |
| Alambique | Tónico | curar más · dar energía · fabricar más |
| Alambique | Tinta | fabricar más · inscribir más barato · envenenar |

- Las de la Forja piden la herramienta en **Hierro (I), Plata (II) y Cristal (III)**. En las otras
  mesas el límite lo ponen los materiales del precio.
- «Más materiales» del martillo **no rompe el tope de +5** por cosa rota que pidió el autor:
  permite llegar antes.
- Las piedras de afilar pasan de la Mesa de encuadernador a la Forja.
- Scriptorium (inscripciones) y Cofre (guardado) ya tienen función propia y no cambian.
- Se mejora en la mesa que se usa: su pantalla suma una pestaña.

## Variedad de materiales
- Todo hexágono (salvo el base, que no cambia) tiene estantería y piedra.
- Cada otro material desbloqueado está **seguro en un tercio** de los hexágonos, repartido de modo
  que **el propio o uno vecino siempre lo tiene**. En los demás aparece según su antigüedad: el
  recién desbloqueado 15 %, el de hace unos pisos 30 %, el viejo 45 %.
- Uno de cada ocho hexágonos es rico: dos nodos extra de un mismo material.
- Se mantienen los extras de cada galería y de las palabras del texto.

## Requisitos para bajar
Cuchilla **y** Martillo del nivel indicado. En multijugador alcanza con que uno cumpla. No afecta
lámparas, Catálogo ni subir.

| Para llegar al piso | 4 | 8 | 13 | 18 |
|---|---|---|---|---|
| Nivel | Hierro | Latón | Plata | Cristal |

Cada requisito se puede conseguir arriba de su puerta sin suerte (lo prueba el bot).

## Lobby
Lo roto en el hexágono base se guarda y no vuelve. Todo lo básico se consigue en los hexágonos
de alrededor.

## Enfoques considerados
Encantamientos al azar (descartado: el juego no depende de la suerte); puntos para repartir
(menús pesados); materiales nuevos para la variedad (pide recetas y dibujos: no ahora).

## Consecuencias
- En una partida existente, los hexágonos ya visitados pueden cambiar de recursos, también los
  agotados.
- Las dos PC de una partida necesitan la misma versión.
- El `.exe` de la raíz no se reconstruye hasta que el autor lo pida.
