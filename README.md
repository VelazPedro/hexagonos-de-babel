# Hexágonos de Babel

Un action-RPG de exploración y recolección en pixel art 2.5D, ambientado en una Biblioteca de
Babel de galerías hexagonales infinitas. Cada hexágono tiene un libro procedural, y las palabras
reales que aparecen en él deciden el escenario, los enemigos, el botín y su rareza. La
progresión no depende del azar. Todo es local y offline.

## Compilar y ejecutar

El juego es **un solo ejecutable**: los datos, los sprites y los shaders van dentro, y en
Windows el runtime de C se enlaza de forma estática. Para jugar alcanza con
`hexagonos_de_babel.exe`.

La última versión compilada está en la raíz del repositorio:
[descargar `hexagonos_de_babel.exe`](https://github.com/VelazPedro/hexagonos-de-babel/raw/main/hexagonos_de_babel.exe).
Requiere Windows 10/11 de 64 bits. Usa la placa o gráfica integrada con DirectX 12 o Vulkan; si
no puede (placa vieja, sin controlador de video o máquina virtual), pasa solo al **modo
compatible**, que dibuja todo con el procesador. Para forzarlo: `hexagonos_de_babel.exe --compat`.
El ejecutable no está firmado, así que Windows SmartScreen puede pedir confirmación: «Más
información» → «Ejecutar de todas formas».

Para compilarlo hace falta Rust estable (probado con 1.100.0). Windows 10/11 es la plataforma
principal.

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-release.ps1   # deja el .exe en la raíz
cargo run --release                                                 # o compilar y jugar
```

El script además borra del binario las rutas de la máquina que compila, y falla si encuentra el
nombre de usuario dentro. Para probar cambios en `assets/` sin recompilar, definí
`HEXBABEL_ASSETS` con la ruta de esa carpeta. Si el juego no puede abrir o se cierra por un
error, lo avisa en una ventana y deja los detalles en `arranque.log`, junto a la partida. Ese archivo
registra cada intento de arranque y la placa detectada.

El icono se genera con los sprites del juego. Si cambia el protagonista, un test lo avisa; se
regenera con `cargo run -- --icon-dump`.

En Linux se necesitan además los paquetes de desarrollo de ALSA, udev y X11 (en Debian/Ubuntu:
`libasound2-dev libudev-dev libx11-dev pkg-config`). La compilación en Linux no se verificó en
esta máquina.

La partida y las opciones se guardan en `%APPDATA%\HexagonosDeBabel` (Linux:
`~/.local/share/HexagonosDeBabel`). `HEXBABEL_SAVE_DIR` cambia esa carpeta.

## Controles

| Acción | Teclado y mouse | Gamepad |
|---|---|---|
| Moverse | WASD o flechas | stick izquierdo |
| Atacar / recolectar (mantener: golpe cargado) | clic izquierdo o J | X |
| Esquivar | Espacio | B |
| Interactuar (puertas, escaleras, estaciones, cofres) | E | A |
| Lanzar páginas | clic derecho o K | R2 |
| Cambiar herramienta | rueda o F | R1 / L1 |
| Leer el libro | R (o atacar con la Lente) | Y |
| Beber tónico | Q | L2 |
| Plantar antorcha | T | cruceta ↓ |
| Lámpara de retorno | L | cruceta ↑ |
| Inventario / Mapa | I / M | cruceta ← / → |
| Habilidades / Glosario / Catálogo | H / G / C | desde la pausa |
| Pausa | Escape | Start |

En los menús se navega con flechas, WASD, stick o cruceta. En las filas con `<` y `>`,
izquierda y derecha cambian el valor; el volumen también se ajusta con clic o arrastre sobre la
barra. Enter, E o A eligen, y Escape o B
vuelven. Q/Tab o L1/R1 cambian de pestaña, y X recicla un objeto. El inventario admite
arrastrar y soltar con el mouse; con teclado o gamepad, se toma un objeto y se suelta en otro
lugar. Las teclas se pueden remapear en **Opciones → Remapear controles**.

## Tu personaje

Al empezar una partida elegís **cuerpo** (Hombre, Mujer o No binario), **piel**, **peinado**
(7), **color de pelo** (9), **ojos** (6), **abrigo** (8), **capa** y **marca en la frente**.
«Al azar» propone una combinación. Se puede cambiar cuando quieras desde **Pausa → Personaje**,
y es solo estético: no cambia las estadísticas.

**Nombre:** el creador tiene un campo de nombre (hasta 12 caracteres, con mayúsculas y tildes;
con gamepad o mouse, el botón «Teclado»). Se ve en letra tenue sobre el personaje y junto al
nivel, y más tenue todavía en combate. Se oculta en **Opciones → Nombres en pantalla**.

### Tu mascota

Te acompaña desde el primer paso. En la pestaña **Mascota** del creador se eligen **nombre**,
**pelaje** (7), **orejas** (paradas, caídas o una y una), **cola** (punta oscura, blanca o lisa),
**ojos** (5) y **collar**. La inicial es un cachorro canela de orejas grandes y paradas.

No recibe daño ni puede morir. Al principio solo te sigue. Tiene su propio árbol, en
**Habilidades**, que se paga con **los mismos puntos** que el tuyo:

| Rama | Habilidades |
|---|---|
| Colmillo | Ataca a los enemigos cercanos que estén en la luz; luego muerde más seguido, más fuerte y con un salto que empuja. |
| Olfato | Trae lo que quedó tirado; luego ayuda con el recurso que estás recolectando y sube el rinde. |
| Vínculo | Mejoras chicas para vos: velocidad, vida que se recupera, experiencia y luz. |

Sus mordiscos no rompen escudos ni reflejos, y no activan tus efectos (quemar, robar vida, etc.).

### El equipo se ve

Lo que llevás puesto se dibuja en el personaje, en el inventario y en el retrato:

- **Armas:** lanza, tridente, espada, daga, estoque, sable, hacha, guadaña, maza, bastón, arco,
  látigo y pluma. La herramienta en uso va en la mano y el arma pasa a la espalda.
- **Armaduras:** coraza, capa, túnica, chaleco, escudo, yelmo, capucha, gorro, corona y máscara.
- **Calzado, faroles y amuletos:** botas, sandalias, grebas y espuelas; linterna, vela, tea y
  pebetero; colgante, anillo, frasco, bolsa y libro.

La forma depende de la palabra del objeto y el color, de su rareza o del tier.

## Cómo se juega

1. **Tu hexágono** (piso 1, centro) es la base. Juntá papel y piedra y construí la **Mesa de
   encuadernador** en una baldosa de obra. Después vienen la Forja, el Alambique, el
   Scriptorium y el Cofre.
2. **Herramientas**: Cuchilla, Martillo, Lámpara y Lente, en seis tiers (Rústico, Hierro,
   Latón, Plata, Cristal, Áureo). Los nodos de tier superior muestran un candado, y al
   acercarte se ve qué herramienta y qué material hacen falta. **Cada cosa se recolecta con su
   herramienta en la mano** (se cambia con F, la rueda o RB/LB): el **Martillo** rompe
   estanterías, piedras, metales y vetas; la **Cuchilla**, enredaderas, códices y tinteros. Cada
   mejora del Martillo rompe más rápido y suma 1 material por cosa rota, hasta +5.
   **No todos los hexágonos traen lo mismo.** Estanterías y piedras hay en todos. Cada otro
   material está seguro en el hexágono o en uno vecino, y en los demás aparece según su
   antigüedad: el recién desbloqueado es el más raro. A veces un hexágono es rico en uno. El
   latón aparece desde el piso 3, la plata desde el 8, el cristal (espejos) desde el 13 y el
   vitral desde el 18. Lo que rompés en tu hexágono **no vuelve**.
   **Piedras de afilar**: en la Forja, pestaña **Herramientas**. Son cinco (de hierro a áurea),
   se hacen en orden y cada una suma **+3 de daño por golpe para siempre**.
   Antes de fabricar o construir, la pantalla dice **qué hace** cada cosa y **en qué te ayuda**,
   con tus números, y avisa si todavía te falta algo para usarla. Con **X** (clic derecho o
   botón X del gamepad) se **fija** hasta 3 recetas: mientras jugás se ve cuánto falta de cada
   material. Las que se hacen una sola vez se sueltan solas al fabricarlas.
3. **Lámparas de retorno**: se encienden con aceite en cualquier sitio. Sirven de viaje rápido
   y de punto de reaparición. Al morir, el 20 % de tus materiales queda en una bolsa que se
   puede recuperar.
4. **Escaleras**: en cada piso hay una cada pocos hexágonos, y siempre hay una puerta que
   acerca a la más próxima. Para bajar a ciertos pisos hacen falta **Cuchilla y Martillo** de un
   nivel mínimo (en multijugador alcanza con que uno los tenga). Subir no pide nada:

   | Para llegar al piso | 4 | 8 | 13 | 18 |
   |---|---|---|---|---|
   | Nivel | Hierro | Latón | Plata | Cristal |
5. **Guardianes**: cada 5 pisos, uno sella las escaleras. Cada uno deja una reliquia fija. El
   del piso 10 abre el **Catálogo**.
6. **Piso 30**: el Catálogo de catálogos. Después del final se abre el **Modo infinito**.

### Mejoras de las mesas

Además de fabricar, tres mesas **mejoran** tus cosas. Se hace en la mesa, en su pestaña propia.
Para cada objeto elegís **una** de tres mejoras, de nivel I a III. No hay azar y quedan siempre
activas. **Cambiar de mejora conserva el nivel** y cuesta lo que ese nivel.

| Mesa (pestaña) | Objeto | Para elegir |
|---|---|---|
| Forja (**Templar**) | Cuchilla | quemar · robar vida · golpe doble |
| | Martillo | más materiales · romper más rápido · más empuje |
| | Lámpara | más luz · más daño en lo oscuro · curarte |
| | Lente | aprender más al leer · atraer objetos · más experiencia |
| Mesa de encuadernador (**Encuadernar**) | Cuaderno | defensa · vida · velocidad |
| | Páginas | más daño · quemar · gastar menos energía |
| | Antorchas | más alcance · curar cerca · quemar enemigos cercanos |
| Alambique (**Destilar**) | Tónico | curar más · dar energía · más por tanda |
| | Tinta | más por tanda · inscribir más barato · envenenar |

En la Forja, el nivel I pide esa herramienta en Hierro, el II en Plata y el III en Cristal. «Más
materiales» no supera el tope de +5 del Martillo: sirve para llegar antes. El efecto de una
antorcha mejorada sirve a los dos jugadores.

### Interfaces y estadísticas

Cada pantalla tiene su estilo: color, marco, fondo, emblema y, en la mayoría, algo que se mueve
detrás. La Forja es de hierro con brasas, la Mesa de encuadernador de cuero cosido, el
Alambique de vidrio y cobre, el Mapa una mesa de cartógrafo, y así con todas. Toda opción tiene
borde; la señalada se ilumina.

En **Pausa → Estadísticas** se ve lo hecho por tu personaje: tiempo jugado, piso más profundo,
hexágonos visitados y caídas; enemigos, élites, guardianes y daño; libros, páginas, palabras y
frases canónicas; y materiales, cosas rotas, objetos fabricados y mejoras. En una partida
anterior, el tiempo, el piso, los hexágonos y los guardianes salen de lo ya guardado, las
palabras del Glosario y el resto empieza en cero. Releer un libro no suma.

## Multijugador (2 jugadores)

Desde el menú principal: **Multijugador**.

**Alcanza con el `.exe`**: la otra PC no necesita ningún otro archivo. Las dos tienen que usar
la **misma versión**.

1. El anfitrión elige **Crear una sala** y le pasa el código de 24 caracteres a su compañero.
2. El compañero lo pega en **Multijugador** y pulsa **Unirme**. Entra con su propio personaje.
3. Si por la vía normal no puede entrar, a los pocos segundos le aparece un **código de
   respuesta**. Se lo manda al anfitrión, que lo pega en la pantalla de la sala (**Pegar**). Se
   conectan solos. El invitado no tiene que cerrar esa pantalla.
4. Cada uno guarda su progreso en su PC; el mundo es el del anfitrión.

Reglas: se pasa de hexágono cuando pasan los dos (puerta: los dos en la misma; escalera o
portal: los dos pulsan interactuar). La pausa es individual y el mundo solo se detiene cuando los
dos pausan. Si uno cae, el otro lo levanta quedándose a su lado.

**Por internet, sin VPN y sin tocar el router:** si el router del anfitrión abre el puerto solo
(UPnP), se entra directo. Si no, el código de respuesta permite que las dos PC se encuentren.
Algunas redes lo bloquean: en ese caso conviene probar que aloje la otra persona. `--net-check`
dice qué esperar desde una PC. Detalles, límites y seguridad en [MULTIPLAYER.md](MULTIPLAYER.md).

La primera vez, Windows puede avisar dos cosas:
- **«Windows protegió tu PC»** (el `.exe` no está firmado): **Más información → Ejecutar de todas
  formas**.
- **Permiso de red:** hay que aceptarlo. Si se canceló, se habilita en **Seguridad de Windows →
  Firewall → Permitir una aplicación**.

## La Biblioteca

### Semillas

Cada partida tiene una `world_seed` de 64 bits: al azar o escrita por vos. Cualquier texto vale
como semilla. Cada sitio se genera en el momento a partir de
`blake3(world_seed ‖ piso ‖ q ‖ r)`, con sub-semillas separadas para texto, trazado,
enemigos, botín y decoración. La misma semilla y coordenada dan **siempre** el mismo sitio.
Tus cambios (recursos agotados, cofres, antorchas, lámparas, puertas abiertas) se guardan
aparte.

### El libro

- **Inscripción** (80 caracteres de un alfabeto de 30: a–z, ñ, espacio, coma y punto). Es una
  permutación invertible de la dirección del sitio: una red de Feistel de 10 rondas sobre 394
  bits con *cycle-walking*. Cada texto de 80 caracteres tiene exactamente una dirección.
- **Cuerpo** (3 páginas de 40 × 80): galimatías con palabras reales intercaladas. La proporción
  de palabras reales crece con la profundidad: unas 6 por sitio en el piso 1 y unas 26 en el
  piso 30.

### Palabras

El léxico tiene 1218 palabras en 8 categorías. Una palabra solo cuenta si está entera (entre
espacios, puntuación o bordes).

| Categoría | Efecto en el sitio |
|---|---|
| Bioma | paleta, peligros y música (11 galerías) |
| Luz | nivel y temperatura de la luz ambiente |
| Orientación | trazado (laberinto, espiral, arena, islas, pozo), escaleras y puertas |
| Criatura | qué enemigos aparecen (10 arquetipos) |
| Cualidad | junto a una criatura, la modifica (24 modificadores) |
| Material | deja más nodos de ese material |
| Objeto | botín con nombre |
| Acción | efectos para el equipo |

**Frases**: «lobo sombrío» (criatura u objeto + cualidad, pegadas) crea un **élite** con nombre
propio o un cofre. Las **61 frases canónicas** dejan un cofre con un objeto Canónico. Un sitio
sin palabras descriptivas ni de orientación es tranquilo.

### Rareza

La decide la palabra, no la suerte:

`rareza base + longitud (≥7: +1; ≥10: +2) + frase (+1) + inscripción (+1) + piso/10`

| Puntos | Rareza |
|---|---|
| ≤2 | Común |
| 3 | Poco común |
| 4 | Raro |
| 5–6 | Épico |
| ≥7 | Legendario |
| frase canónica | Canónico |

La rareza escala las estadísticas y decide cuántos efectos de palabra lleva un objeto (de 0 a
3).

### Leer e inscribir

Con la **Lente** se abre el libro. Las palabras encontradas entran al **Glosario**, dan
experiencia y quedan disponibles en el **Scriptorium**. Ahí podés **inscribirlas** sobre tu
equipo gastando tinta, para sumarle su efecto.

### El Catálogo

Escribí una frase de hasta 80 caracteres y gastá **Índices**, según la rareza de las palabras
buscadas. Viajás al sitio cuya inscripción es exactamente esa frase, así que sus palabras le dan
forma. Los sitios remotos tienen dificultad igual al piso actual + 2, sus palabras nunca superan
lo Épico y tienen un portal de regreso.

## Depuración

| Opción | Qué hace |
|---|---|
| `F3` | overlay con FPS, actores, entidades, luces y sitio |
| `--atlas-dump [archivo.png]` | exporta el atlas de sprites |
| `--screenshot <seed> <piso> <q> <r> [--out f.png] [--seconds n] [--screen nombre]` | renderiza un sitio, o una pantalla de la interfaz, a PNG |
| `--find <seed> <criterio>` | busca un sitio: `elite`, `canon`, `dark`, `bright`, `guardian:<piso>`, `biome:<id>`, `layout:<tipo>` |
| `--bench [segundos]` | 40 enemigos, 64 luces y partículas, sin vsync |
| `--mute` | sin audio |
| `--compat` | modo sin placa de video |
| `--net-check` | dice cómo se podrá entrar a una sala creada en esta PC (abre y cierra el puerto un instante) |
| `--coop-host <archivo>` / `--coop-join <código>` | pruebas de multijugador en una sola PC |
| `--coop-direct <archivo>` | junto con las anteriores: prueba la conexión directa sin red |
| `--help` | ayuda |

## Pruebas

```powershell
cargo test --workspace                                      # unitarias, proptest e integración
cargo test --workspace --release -- --include-ignored       # + estrés (10.000 sitios) y progresión completa
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

## Limitaciones conocidas

- El **balance de duración** (primer guardián en 20–30 minutos, final en 8–12 horas) está
  estimado a partir de las curvas. El bot de progresión verifica materiales y accesos, no el
  ritmo ni la dificultad del combate: falta probarlo con jugadores.
- No se verificó con un **gamepad físico** ni escuchando el **audio**. La navegación sin mouse
  se prueba con entradas simuladas, y los sonidos, con tests de señal.
- La compilación en **Linux** no se probó.
- El **balance de la mascota** (daño, alcance, costo en puntos) no se probó jugando. Con sus 10
  habilidades, completar los dos árboles pide 42 puntos en lugar de 32.
- El **multijugador** se probó con dos juegos en una misma PC, con un router simulado y con
  redes con pérdida simuladas; **no entre dos PC reales de redes distintas**. La conexión directa
  puede fallar si alguna red cambia de puerto según el destino (común en datos móviles). La
  conexión está firmada pero no cifrada.
- El **balance nuevo** (piedras de afilar y enemigos con menos vida) está calculado en golpes,
  no probado jugando. Tampoco las mejoras de mesa, las puertas entre pisos ni la variedad de
  materiales.

## Estructura

- `core/` (`babel_core`, sin Bevy): Biblioteca, generación, simulación del juego, progresión,
  guardado y bot de progresión.
- `src/`: capa de Bevy (dibujo, luz en WGSL, interfaz, audio, entrada y herramientas).
- `assets/data/*.ron` (balance y contenido; `pet_skills.ron` es el árbol de la mascota),
  `assets/sprites/*.ron` (pixel art como filas de caracteres; `hero.ron` son las capas del
  protagonista y `pet.ron` las piezas de la mascota) y `assets/shaders/*.wgsl`.
