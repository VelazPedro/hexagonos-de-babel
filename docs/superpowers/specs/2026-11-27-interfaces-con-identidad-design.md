# Interfaces con identidad propia

Estado: **aprobado por el autor el 27/11/2026** («aprobalo»), después de aprobar la muestra de
la Forja y la Mesa de encuadernador («así me gusta»).

## Para qué
El pedido: las interfaces se ven sosas y todas iguales. Tienen que tener más color, más variedad,
detalles adornados y opciones con líneas de borde.

La causa: casi todas se dibujan con las mismas piezas (panel violeta liso, borde de un píxel y
franja celeste o lila) y muchas opciones no tienen borde.

## Objetivos
- Cada lugar se reconoce de un vistazo por su color, su marco, su fondo y su emblema.
- Toda opción tiene borde; la señalada se ilumina; la ya elegida queda marcada en dorado.
- Más color con sentido: una franja o un tono dice a qué objeto, tipo o destino pertenece algo.

## Fuera de alcance
- Cambiar qué hace cada pantalla, sus controles o sus reglas.
- Reorganizar contenidos. Se mueve algo solo si un marco nuevo lo tapa.
- Tipografía nueva, dependencias nuevas o personalización del lobby.
- Reconstruir el `.exe` antes de que el autor lo pida.

## Lo que sería un fracaso
- Que cueste leer: adorno detrás del texto, texto apagado que no contrasta, colores de rareza que
  se confundan con el fondo.
- Que el juego vaya más lento. Ya se pidió 60 FPS en PC de pocos recursos.
- Que la pantalla de juego quede recargada y tape lo que pasa.

## Restricciones
- Se dibuja en 480×270 con la fuente propia. Mouse, teclado y gamepad siguen funcionando.
- Partidas y multijugador no cambian: esto no toca reglas ni red.
- El proyecto no tiene commits nuevos sin pedido del autor.

## Enfoques
**Qué variedad** (eligió el autor):
- **A. Cada lugar con su identidad.** La que más varía y la que más trabajo lleva. **Elegida.**
- B. Un solo estilo más adornado. Más rápido, pero las pantallas seguirían pareciéndose.
- C. Color según el contenido. Ayuda a encontrar cosas, pero adorna poco.

**Cómo construirla:**
1. Todo dibujado con código. Se ajusta a cualquier tamaño; las figuras quedan simples.
2. Marcos en pixel art por lugar. Más artesanal; caro de cambiar y de adaptar a cada panel.
3. **Mezcla:** marcos, texturas y animaciones con código, y dibujos chicos para esquinas y
   emblemas. **Elegida** (propuesta mía, mostrada en la muestra y aprobada con ella).

## Piezas que comparten todas las pantallas
Como en la muestra:
- Título en una placa con el emblema del lugar a los costados.
- Paneles con el marco y la textura del lugar.
- Opciones con borde y esquinas recortadas; brillo al señalarlas; borde dorado si ya está
  elegida; franja de color cuando distingue algo.
- Títulos de sección con una línea y un rombo.
- Ícono destacado en un marco hexagonal.
- Pestañas, barra de teclas, casillas de materiales y textos apagados en los tonos del lugar.
- Fondo propio y una animación suave que se ve sobre todo en los márgenes.

## Identidad de cada pantalla

| Pantalla | Idea | Colores | Animación |
|---|---|---|---|
| Forja ✔ | Hierro y remaches sobre ladrillo | carbón y naranja | brasas que suben |
| Mesa de encuadernador ✔ | Cuero cosido y latón | marrón, dorado y verde agua | polvo y papelitos |
| Alambique | Vidrio y caños de cobre | verde botella y cobre | burbujas |
| Scriptorium | Escritorio manchado de tinta, con pluma | nogal, azul tinta y crema | letras que flotan |
| Construir | Plano de obra con trazos de tiza | azul plano y blanco | trazos que titilan |
| Cofre | Tablas con flejes de hierro | madera rojiza y oro | destellos |
| Lámparas de retorno | Noche con faroles | verde azulado y ámbar | luciérnagas |
| Inventario | Morral de viaje con correas y hebillas | lona verde oliva y cuero | ninguna |
| Habilidades | Cielo con constelaciones | índigo; cada rama con su color | estrellas que titilan |
| Glosario | Fichero de biblioteca con cantos dorados | bordó y oro | polvo dorado |
| Mapa | Mesa de cartógrafo con rosa de los vientos | sepia y tinta de mar | ninguna |
| Catálogo | Archivo infinito | violeta profundo y oro | hexágonos que giran |
| Lector | Libro abierto con tapas y señalador | cuero y tinta | ninguna, para leer tranquilo |
| Título, Tu mundo, Tu personaje y Pausa | La Biblioteca: filigrana y rejilla de hexágonos | índigo, oro y celeste | páginas que flotan |
| Multijugador | Dos hexágonos enlazados | turquesa y rosa | motas de dos colores |
| Opciones y Controles | Relojería | grafito y latón | engranajes lentos |
| Al caer | Ceniza fría | gris y violeta apagado | hojas que caen |
| Final y créditos | Amanecer sobre los estantes | noche y oro | destellos que suben |

**Fondos:** son opacos, como en la muestra. **Título, Pausa y Al caer** son la excepción: siguen
dejando ver el mundo detrás (en el Título se mueve), con la filigrana o la ceniza encima.

Además, para sumar color donde hay muchas opciones:
- **Pausa y Título:** cada botón lleva el tono del lugar al que lleva.
- **Construir:** cada estación lleva su color y su emblema.
- **Fabricación abierta sin mesa** (no pasa al jugar): usa el estilo de la Biblioteca.

## Pantalla de juego
Retoque liviano: marcos en el retrato, las barras, las casillas de herramientas y objetos, el
minimapa, la lista de recetas fijadas, el cartel de ayuda y los avisos. Sin fondo ni
animaciones, y sin ocupar más lugar que hoy.

## Decisiones técnicas
- **Un lugar central** dice qué estilo usa cada pantalla; las piezas compartidas lo leen. Una
  pantalla nueva sin estilo falla en las pruebas.
- **Fondos y texturas se dibujan una vez y se guardan.** En cada cuadro se copian; solo se
  dibujan las animaciones y lo que cambia. La muestra los dibuja en cada cuadro y no debe quedar
  así.
- La muestra es descartable: el código final se escribe de nuevo, con pruebas.

## Cómo se comprueba
- Cada estilo tiene contraste suficiente: texto normal y apagado de al menos 4,5 a 1 contra
  paneles y opciones; colores de rareza de al menos 3 a 1 donde se usan.
- Las pruebas de siempre: todo se alcanza sin mouse, nada se superpone, las explicaciones entran.
- Se mide el tiempo de dibujo de cada pantalla antes y después, en versión optimizada. Ninguna
  puede pasar de 4 ms.
- Se revisan capturas de todas las pantallas.

## De paso se corrige
- «Vida máx..» con dos puntos en la explicación de una mejora (error de la entrega anterior).
- La ficha del objeto en el Inventario queda pegada al borde de su panel.

## Queda a criterio de quien lo construya
Tonos exactos, dibujo de cada emblema y esquina, y cantidad y velocidad de las animaciones.

## Orden de trabajo
Piezas compartidas → mesas → otras estaciones → pantallas del personaje → menús → lector, caída y
final → pantalla de juego.
