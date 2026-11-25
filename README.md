# Hexágonos de Babel

Un action-RPG de exploración y recolección en pixel art 2.5D, ambientado en una Biblioteca de
Babel de galerías hexagonales infinitas. Cada hexágono tiene un libro procedural, y las palabras
reales que aparecen en él deciden el escenario, los enemigos, el botín y su rareza. La
progresión no depende del azar. Todo es local y offline.

## Compilar y ejecutar

Hace falta Rust estable (probado con 1.100.0). Windows 10/11 es la plataforma principal.

```powershell
cargo run --release
```

El resultado es un único ejecutable, `target/release/hexagonos_de_babel(.exe)`, más la carpeta
`assets/`. Para distribuirlo, copiá los dos juntos. El juego busca `assets/` junto al
ejecutable, o en la ruta de `HEXBABEL_ASSETS`.

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

En los menús se navega con flechas, WASD, stick o cruceta. Enter, E o A eligen, y Escape o B
vuelven. Q/Tab o L1/R1 cambian de pestaña, y X recicla un objeto. El inventario admite
arrastrar y soltar con el mouse; con teclado o gamepad, se toma un objeto y se suelta en otro
lugar. Las teclas se pueden remapear en **Opciones → Remapear controles**.

## Cómo se juega

1. **Tu hexágono** (piso 1, centro) es la base. Juntá papel y piedra y construí la **Mesa de
   encuadernador** en una baldosa de obra. Después vienen la Forja, el Alambique, el
   Scriptorium y el Cofre.
2. **Herramientas**: Cuchilla, Martillo, Lámpara y Lente, en seis tiers (Rústico, Hierro,
   Latón, Plata, Cristal, Áureo). Los nodos de tier superior muestran un candado, y al
   acercarte se ve qué herramienta y qué material hacen falta. El latón aparece en todos los
   sitios desde el piso 3, la plata desde el 8, el cristal desde el 13 y el vitral desde el 18.
3. **Lámparas de retorno**: se encienden con aceite en cualquier sitio. Sirven de viaje rápido
   y de punto de reaparición. Al morir, el 20 % de tus materiales queda en una bolsa que se
   puede recuperar.
4. **Escaleras**: en cada piso hay una cada pocos hexágonos, y siempre hay una puerta que
   acerca a la más próxima.
5. **Guardianes**: cada 5 pisos, uno sella las escaleras. Cada uno deja una reliquia fija. El
   del piso 10 abre el **Catálogo**.
6. **Piso 30**: el Catálogo de catálogos. Después del final se abre el **Modo infinito**.

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

## Estructura

- `core/` (`babel_core`, sin Bevy): Biblioteca, generación, simulación del juego, progresión,
  guardado y bot de progresión.
- `src/`: capa de Bevy (dibujo, luz en WGSL, interfaz, audio, entrada y herramientas).
- `assets/data/*.ron` (balance y contenido), `assets/sprites/*.ron` (pixel art como filas de
  caracteres) y `assets/shaders/*.wgsl`.
