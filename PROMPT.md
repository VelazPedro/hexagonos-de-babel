# PROMPT — "Hexágonos de Babel" (título provisional)

## 0. Rol, objetivo y definición de terminado

Sos un desarrollador senior de videojuegos en Rust. Vas a construir **un juego completo, pulido y jugable de principio a fin**, no un prototipo ni un MVP. No entregues por fases: planificá todo una vez (en modo plan) y después implementá hasta cumplir la **Definición de terminado** (sección 17). No me pidas confirmación entre pasos. Si una decisión es ambigua, tomá la opción más robusta, anotala en `DECISIONS.md` y seguí.

**Concepto:** un action-RPG de exploración y recolección en pixel art 2.5D, con dirección de arte inspirada en *Forager*. El mundo es la Biblioteca de Babel: galerías hexagonales infinitas cuyos libros se generan de forma procedural y determinista. Cada sitio (hexágono) tiene un libro/inscripción. Las palabras reales que aparecen en ese texto deciden el escenario, los enemigos, los drops y la rareza de los objetos. La progresión del jugador **no** depende del azar: sale de recolectar recursos, fabricar herramientas por tiers, subir habilidades y obtener objetos especiales garantizados al avanzar.

Todo es local y offline. No hay red, cuentas ni telemetría. La seguridad se revisa después, no la priorices ahora.

Idioma: **código, identificadores y comentarios en inglés; todo el texto del juego en español.**

---

## 1. Stack técnico (obligatorio)

- **Rust estable** con **Bevy**. Usá la última versión estable que figure en crates.io y **fijala exacta** en `Cargo.toml` (`bevy = "=X.Y.Z"`).
- **Antes de escribir cualquier código que use la API de Bevy, consultá su documentación con context7** para esa versión exacta. No escribas la API de memoria: cambia entre versiones (sistemas, schedules, render, assets, UI, audio, input). Hacé lo mismo con cada crate que agregues.
- Shaders en **WGSL**: es lo único fuera de Rust, y solo para iluminación y post-proceso.
- Crates permitidos (verificá compatibilidad con la versión de Bevy antes de agregarlos):
  - `rand` + `rand_chacha` (`ChaCha8Rng`: salida reproducible entre plataformas)
  - `blake3` (hash estable para semillas). **Nunca** uses `std::collections::hash_map::DefaultHasher` ni hashes no estables para generación.
  - `num-bigint` + `num-traits` (biyección de la inscripción)
  - `aho-corasick` (detección de palabras)
  - `serde` + `ron` (datos y guardado)
  - `noise` (opcional, decoración)
  - `proptest` (solo para tests)
- Un crate de terceros para iluminación o físicas solo si soporta la versión fijada de Bevy **y** lo verificaste con context7 y docs.rs. Si no, implementalo propio (preferido para la iluminación).
- `Cargo.toml`: `[profile.dev] opt-level = 1`, `[profile.dev.package."*"] opt-level = 3`, `[profile.release] lto = "thin"`.
- Plataforma objetivo: Windows 10/11 (también tiene que compilar en Linux). Un solo ejecutable más la carpeta `assets/`.

## 2. Herramientas y plugins del entorno

- **superpowers:** usá sus skills de planificación, TDD y verificación antes de declarar algo terminado.
- **game-dev / gamedev:** consultá sus skills para game feel, game loop, ECS y balance cuando aplique.
- **context7:** obligatorio para Bevy y cualquier crate (sección 1).
- **code-review:** pasalo sobre todo el código al final y corregí lo que encuentre.
- **rust-analyzer (LSP), si está disponible:** usalo para chequear tipos y errores.
- Ignorá `pixijs-skills`, `vercel` y `frontend-design`: este proyecto no es web.

---

## 3. Dirección de arte

Inspirado en el **lenguaje visual** de *Forager*. **No copies** sprites, personajes, íconos, UI ni nombres de Forager: todos los assets son originales.

- Paleta saturada y alegre con sombras frías: verdes vivos, turquesas, naranjas cálidos, violetas de sombra. Nunca negro puro (usá violeta/azul muy oscuro, por ejemplo `#1a1028`).
- Formas redondeadas y chunky. Contorno oscuro de 1 px solo en la silueta exterior de personajes, enemigos y drops, no en los tiles.
- Sombras ovaladas suaves bajo toda entidad.
- Tiles de 16×16. Personaje de unos 16×20. Animaciones de 4 a 6 frames.
- Las cosas se sienten vivas: los recursos tiemblan al golpearlos y se rompen en partículas. Los drops saltan con arco y rebote, brillan según su rareza y vuelan hacia el jugador con imán. Las plantas y lámparas se mecen.
- Tema de Babel, reinterpretado con ese color: estanterías de lomos multicolores, lámparas colgantes, barandas, pozos de ventilación, escaleras en espiral, espejos. Cada bioma recolorea la galería (sección 8).

**Assets 100 % procedurales o definidos en código.** No descargues imágenes.

- Sprites como mapas de píxeles indexados por paleta, en archivos de datos legibles (`assets/sprites/*.ron`, con filas de caracteres donde cada carácter es un índice de paleta). Se compilan en un atlas al iniciar.
- Variantes procedurales (lomos de libros, grietas, musgo, recolores por bioma) generadas por código desde la misma semilla.
- **Fuente bitmap propia** de 5×7 (o 6×8) definida en código, con `ñ`, tildes, `¿`, `¡`, dígitos y signos.
- Agregá un modo debug `--atlas-dump` que exporte el atlas a PNG, para que puedas revisarlo visualmente.

## 4. Render 2.5D e iluminación dinámica

**Pipeline pixel-perfect**
- El mundo se renderiza a un render target de baja resolución fija (**480×270**) y se escala con nearest neighbor por un entero, con letterbox.
- Cámara con suavizado y subpíxel: el desplazamiento fraccional se aplica al escalar, no al mundo, para que no haya jitter.
- **No puede haber** bleeding entre tiles del atlas (padding de 1 px y extrusión), costuras ni parpadeos.

**2.5D**
- Vista top-down con profundidad: las paredes y estanterías tienen cara superior y frontal.
- Orden por Y (y-sort) entre todas las entidades y objetos altos.
- Altura (`z`) separada de la posición en el suelo: saltos, rebotes de drops y proyectiles con arco. La sombra queda en el suelo y se achica con la altura.

**Iluminación dinámica (propia, en WGSL)**
- Luz ambiente por sitio: nivel y temperatura de color dependen de las palabras (sección 8).
- Luces puntuales: farol del jugador (su radio depende del tier de la lámpara), lámparas del sitio, antorchas que el jugador coloca, proyectiles de fuego, drops raros (brillo del color de su rareza), enemigos luminosos.
- Parpadeo suave (flicker) con ruido.
- **Sombras 2D:** las paredes y estanterías bloquean la luz. Calculá polígonos de visibilidad por luz sobre la grilla de tiles, rasterizá la máscara y suavizá los bordes con un blur barato.
- Light map en textura aparte con composición multiplicativa sobre la escena más un leve bloom aditivo para lo emisivo.
- Límite: 64 luces activas con culling por pantalla, manteniendo 60 FPS.
- La oscuridad es mecánica de juego: los enemigos "sombríos" solo se ven y pueden ser dañados dentro de la luz.

---

## 5. La Biblioteca: estructura y generación

**Estructura**
- El mundo son **pisos** (profundidad 1, 2, 3…) unidos por escaleras.
- Cada piso es una grilla hexagonal infinita con coordenadas axiales `(q, r)`.
- Cada celda es un **sitio**: un escenario jugable con forma de hexágono, de unos 40×34 tiles, con hasta 6 salidas a los hexágonos vecinos y quizás escaleras.

**Semillas**
- `world_seed: u64` aleatoria en cada partida nueva. También se puede escribir una.
- `site_seed = blake3(world_seed ‖ floor ‖ q ‖ r)`. De ahí se derivan sub-semillas con etiquetas (`"text"`, `"layout"`, `"enemies"`, `"loot"`, `"decor"`), para que cambiar un subsistema no altere a los demás.
- Todo se genera **en tiempo real** al entrar al sitio. No hay nada pregenerado. Objetivo: **< 30 ms** por sitio en release.
- La misma semilla y coordenada producen siempre el mismo sitio. Los cambios del jugador (recursos agotados, cofres abiertos, antorchas) se guardan aparte como un delta por sitio.

**Alfabeto canónico:** 30 símbolos: `a–z`, `ñ`, espacio, coma y punto. Sin tildes en el texto generado.

**El libro del sitio** tiene dos partes:

1. **Inscripción (80 caracteres, biyectiva).**
   - Es una permutación invertible entre "dirección" y texto.
   - La dirección es un entero grande `A`. Para un sitio caminable, `A = pack(floor, zigzag(q), zigzag(r))`.
   - La inscripción es `decode_base30(F⁻¹(A))`, donde `F` es una **red de Feistel** de al menos 8 rondas sobre 394 bits. La función de ronda es `blake3` con clave derivada de `world_seed`. Usá **cycle-walking** para quedarte dentro de `[0, 30^80)`.
   - Esto habilita **el Catálogo** (sección 10.6): buscar una frase de hasta 80 caracteres devuelve la dirección del sitio cuya inscripción la contiene.
2. **Cuerpo (3 páginas visibles de 40 líneas × 80 caracteres).**
   - Mezcla a nivel token. Para cada token: con probabilidad `λ(profundidad)` se emite una palabra del léxico (muestreada con peso inverso a su rareza base); si no, se emite un token de galimatías (longitud 1–9, caracteres casi uniformes con un leve sesgo de bigramas del español).
   - Puntuación y espacios insertados como en Babel.
   - `λ` crece con la profundidad (por ejemplo, de 0,04 en el piso 1 a 0,22 en el piso 30): **cuanto más profundo, más legible el texto**, y eso es progresión.
   - Ajustá `λ` con un test que mida la distribución: unas 4 a 8 palabras detectadas por sitio en el piso 1 y unas 20 a 35 en el piso 30.

## 6. Léxico y detección de palabras

- `assets/data/lexicon.ron`: **al menos 900 palabras en español** sin tildes, curadas por vos, cada una con:
  - `word`
  - `category`: `Bioma | Luz | Orientacion | Criatura | Material | Objeto | Cualidad | Accion`
  - `tags` (por ejemplo `fuego`, `veneno`, `velocidad`, `gigante`, `vertical`)
  - `base_rarity` de 1 a 5 (las palabras comunes y cortas son 1; las raras y evocadoras, 4–5)
  - `palette_hint` opcional
- Mínimos por categoría: Bioma 60, Luz 40, Orientación 50, Criatura 120, Material 100, Objeto 150, Cualidad 200, Acción 120.
- Detección con **Aho-Corasick** sobre inscripción y cuerpo. Solo cuenta como palabra si está entre límites (espacio, puntuación o borde).
- **Frases:** detectá pares adyacentes con forma válida en español (`Criatura|Objeto + Cualidad`, por ejemplo "lobo sombrio", o `Cualidad + Criatura|Objeto`). Una frase se vuelve una entidad u objeto compuesto con rareza mayor.
- `assets/data/canon.ron`: unas 60 **frases canónicas originales** (escritas por vos, no citas de libros) de 2 a 4 palabras del léxico. Si una aparece completa, produce un objeto de rareza Canónica.

## 7. Rareza (depende de la palabra, no del azar)

```
score = base_rarity(word)
      + bonus_longitud     (≥7 letras: +1; ≥10: +2)
      + bonus_frase        (parte de una frase válida: +1; frase canónica: rareza Canónica directa)
      + bonus_origen       (palabra en la inscripción: +1)
      + floor(profundidad / 10)
```

| Score | Rareza | Color del brillo |
|---|---|---|
| ≤2 | Común | gris claro |
| 3 | Poco común | verde |
| 4 | Raro | azul |
| 5–6 | Épico | violeta |
| ≥7 | Legendario | dorado |
| canónica | **Canónico** | blanco iridiscente animado |

- La rareza escala los stats con una tabla fija, decide cuántos efectos de palabra lleva el objeto (de 0 a 3) y el color de su brillo y luz.
- La misma palabra en el mismo contexto da siempre la misma rareza. El azar decide **qué** aparece en el texto; la **palabra** decide cuánto vale.

## 8. Generación de sitios a partir de las palabras

Analizá todas las palabras detectadas del sitio y armá una `SiteRecipe`:

- **Bioma** (palabras `Bioma`, gana la de mayor peso total): paleta, tileset recoloreado, decoración y peligros. Mínimo 10 biomas, por ejemplo:
  - Galería canónica (sin palabras de bioma)
  - Ceniza (brasas que dañan)
  - Agua (charcos que ralentizan y reflejan)
  - Hielo (suelo resbaladizo)
  - Jardín (enredaderas que se cortan para obtener fibra)
  - Sal
  - Arena (dunas de papel)
  - Óxido
  - Espejo (ilusiones)
  - Noche
  - Hueso
- **Luz** (palabras `Luz`): nivel de luz ambiente (de 0,15 a 1,0), temperatura de color y densidad de lámparas. Sin palabras de luz se usa un default según la profundidad.
- **Orientación** (palabras `Orientacion`) elige el **algoritmo de layout** y las salidas:
  - laberinto / pasillo → maze (recursive backtracker)
  - espiral / caracol → espiral
  - centro / circulo / plaza → arena central
  - puente / isla → islas unidas por puentes
  - pozo / abismo → pozo central infranqueable
  - escalera / arriba / abajo → aparece una escalera
  - norte / sur / este / oeste, etc. → fuerzan salidas en esa dirección hexagonal
  - **Sin palabras de orientación:** hexágono canónico de Borges (estanterías en 4 paredes, 2 salidas, pozo de ventilación central con baranda).
- Si el sitio no tiene **ninguna** palabra descriptiva ni de orientación, es un sitio canónico tranquilo: pocos enemigos y recursos básicos. Sirve de respiro.
- **Garantías validadas con flood fill:** todas las salidas abiertas y la escalera son alcanzables desde la entrada, no hay enemigos ni recursos dentro de paredes y nadie aparece encima del jugador. Si la validación falla, se regenera con una sub-semilla de reintento (determinista).
- **Recursos del sitio** según bioma, materiales mencionados y profundidad: estanterías (papel, cuero), barandas (hierro), lámparas (aceite), piedra hexagonal, espejos (cristal), más los nodos de cada bioma.

## 9. Enemigos y drops

**Enemigos compuestos.**
- Arquetipo según la palabra `Criatura`. Al menos 10 arquetipos de comportamiento:
  - Polilla (errática)
  - Libro hambriento (embiste)
  - Lector extraviado (huye y lanza páginas)
  - Purificador (antorcha, área de fuego)
  - Araña de estante (emboscada)
  - Golem de tomos (lento, tanque)
  - Espectro (atraviesa paredes, visible solo con luz)
  - Enjambre de letras
  - Autómata (patrón en línea recta)
  - Bestia del bioma
- Cada palabra `Criatura` del léxico se asigna a un arquetipo más un recolor y una variante de sprite.
- Modificadores por palabras `Cualidad` cercanas en el texto: ardiente (estela de fuego, quemadura), sombrío (invisible fuera de la luz), veloz, gigante (×1,6 tamaño y vida, más lento), venenoso, antiguo (más vida, más XP), hueco (se divide al morir), etc. Mínimo 20 modificadores, cada uno con efecto jugable y visual.
- Las frases detectadas ("lobo sombrío") se vuelven un **élite** con nombre propio en pantalla.
- Sin palabras `Criatura`: arquetipos default del piso.
- Stats = base del arquetipo × curva de profundidad × modificadores. Los ataques se telegrafían siempre (wind-up visible) y no hay daño de contacto injusto al aparecer.

**Drops.**
- Material ligado al arquetipo y al bioma.
- Con chance fija por arquetipo, un **objeto de palabra**: equipo cuyo nombre sale de la palabra (por ejemplo "Daga ardiente"), con efecto según tags y rareza según la sección 7.
- Los élites dropean siempre un objeto de palabra.

---

## 10. Progresión (determinista)

### 10.1 Recursos
Unos 14 recursos, por ejemplo: papel, cuero, fibra, tinta, aceite, piedra, hierro, latón, plata, cristal, ceniza, sal, hueso y vitral. Cada uno tiene nodo, ícono, color y uso claro en recetas.

### 10.2 Herramientas por tier
- Herramientas: **Martillo** (piedra y metal), **Cuchilla** (papel, cuero y fibra, y además arma básica), **Lámpara** (radio de luz) y **Lente** (lectura, sección 10.4).
- 6 tiers: Rústico → Hierro → Latón → Plata → Cristal → Áureo.
- El tier decide qué nodos se pueden recolectar, la velocidad y el radio de luz.
- Los nodos de tier superior muestran un candado claro y el material que hace falta.

### 10.3 Base y fabricación
- El **Hexágono propio** (el sitio inicial) es la base. Ahí se construyen estaciones en una grilla: Mesa de encuadernador, Forja, Alambique de tinta, Scriptorium y Cofres.
- Recetas en `assets/data/recipes.ron`, con una UI que muestra qué falta.
- **Lámparas de retorno:** se encienden con aceite en cualquier sitio, funcionan como viaje rápido a la base y como punto de reaparición.

### 10.4 Lectura y Glosario
- Con la Lente se abre el libro del sitio: vista de página con galimatías y las palabras detectadas resaltadas con el color de su rareza.
- Leer registra palabras nuevas en el **Glosario** (codex), que da XP y muestra categoría y efecto.
- En el **Scriptorium** se pueden **inscribir** palabras del Glosario sobre el equipo para sumarle su efecto. Gasta tinta, y el costo escala con la rareza. Máximo de inscripciones según la rareza del objeto. Este es el corazón del sistema: las palabras son poder.

### 10.5 Habilidades (sin azar)
- XP por recolectar, fabricar, leer, matar y despejar sitios. Cada nivel da 1 punto de habilidad.
- Árbol de **unos 32 nodos en 4 ramas**:
  - Lector: más palabras detectadas en la lectura, inscripción más barata
  - Explorador: velocidad, dash, radio de luz, lámparas más baratas
  - Artesano: recolección, rendimiento, recetas
  - Combatiente: daño, vida, i-frames, combos
- Hay nodos que desbloquean mecánicas, por ejemplo: dash, golpe cargado, colocar antorchas, imán ampliado o inscripción doble.
- Todo es visible y planificable desde el principio.

### 10.6 El Catálogo
- Se desbloquea al vencer al guardián del piso 10.
- El jugador escribe una frase (≤80 caracteres, alfabeto canónico) y gasta **Índices**, un objeto fabricable caro.
- El juego calcula la dirección con `F(frase)` y lo lleva a un **sitio remoto** cuya inscripción es exactamente esa frase, así que sus palabras moldean el sitio. Si la dirección cae en un sitio caminable, se va ahí.
- El costo en Índices escala con la suma de las rarezas de las palabras buscadas. Las palabras de sitios remotos tienen la rareza **tope en Épico**, para que no se pueda farmear Legendarios.
- Los sitios remotos tienen dificultad = piso actual + 2 y una salida de regreso.

### 10.7 Objetos especiales garantizados
- Cada 5 pisos hay un **Guardián** (jefe con 2–3 fases y patrones propios). Se arma con las palabras más raras del sitio de la escalera.
- Cada guardián dropea siempre una **reliquia fija**, distinta para cada uno: por ejemplo botas de dash doble, lámpara eterna, lente profunda o el desbloqueo del Catálogo.
- Hitos fijos de descubrimiento (por ejemplo, el primer sitio de cada bioma) también dan objetos garantizados.

### 10.8 Final
- **Piso 30:** el sitio del **Catálogo de catálogos**, con jefe final, una escena de cierre breve y original, y créditos.
- Después se desbloquea el **Modo infinito**: pisos sin tope, con escalado continuo.
- Duración objetivo: primer guardián en 20–30 minutos; final en unas 8–12 horas.

---

## 11. Combate y game feel

- Movimiento de 8 direcciones con aceleración y desaceleración cortas. Dash con i-frames. Ataque básico con combo de 3 golpes. Golpe cargado si se desbloquea. Arma a distancia opcional (páginas afiladas o tinta).
- Al golpear: hitstop (unos 50 ms), knockback, flash blanco en el enemigo, screen shake proporcional (y desactivable), números de daño y partículas.
- Vida, energía y estados (quemado, envenenado, lento) con íconos claros.
- Muerte: reaparecés en la última lámpara encendida y perdés el 20 % de los recursos no equipados, que quedan en una bolsa recuperable.
- Feedback en todo: sonido y partículas al recolectar, al fabricar, al subir de nivel y al encontrar una palabra legendaria (destello y fanfarria).

## 12. UI / UX

- HUD: vida, energía, XP, herramienta activa, minimapa hexagonal, piso actual y bioma.
- Pantallas: inventario con drag and drop, fabricación, árbol de habilidades, Glosario, lector de libro, mapa de hexágonos visitados, Catálogo, opciones y pausa.
- Menú principal: nueva partida (con semilla opcional), continuar, opciones y salir.
- Controles: teclado y mouse (WASD, clic, rueda para herramientas) y **gamepad** completo. Todo es navegable sin mouse.
- Opciones: volumen general, música y efectos, escala de píxel, pantalla completa, screen shake on/off, remapeo básico.
- Tutorial integrado en el primer sitio con carteles contextuales, sin muros de texto.

## 13. Audio

- Efectos **procedurales** generados al iniciar (sintetizador tipo sfxr propio: onda, envolvente, barrido de pitch, ruido). Más de 25 efectos: pasos, golpes, recolección por material, drops por rareza, UI, dash, daño y muerte.
- Música ambiental procedural simple por bioma (pads con escala pentatónica que cambia según el bioma) y un tema de guardián.
- Sin archivos de audio externos.

## 14. Guardado

- Archivo `RON` en la carpeta de datos del usuario. Guarda `world_seed`, el estado del jugador, el Glosario, la base, los deltas de sitios visitados y las lámparas.
- Autoguardado al cambiar de sitio y al cerrar. Escritura atómica (archivo temporal y rename) más una copia de respaldo.
- Si el archivo está corrupto o es de otra versión, se carga el respaldo o aparece un aviso claro, nunca un crash. El formato tiene campo `version`.

---

## 15. Arquitectura

```
src/
  main.rs, app.rs            (plugins, estados: Boot, MainMenu, InGame, Paused, GameOver, Credits)
  babel/      (alphabet, seeds, feistel, inscription, body_gen, lexicon, detect, rarity)
  world/      (floors, hex coords, site_recipe, layout/*, validation, deltas)
  render/     (pixel_target, camera, ysort, atlas, procedural_sprites, font, lighting/*.wgsl)
  gameplay/   (player, combat, enemies/*, ai, loot, items, inscription, tools, harvesting)
  progression/(xp, skills, recipes, crafting, base, relics, catalog, bosses)
  ui/, audio/, save/, debug/
assets/data/  (lexicon.ron, canon.ron, recipes.ron, items.ron, skills.ron, enemies.ron, biomes.ron)
assets/sprites/*.ron
```

- **El módulo `babel/` y la generación de sitios son lógica pura, sin dependencias de Bevy**, para poder testearlos headless.
- Todos los datos de balance van en `.ron`, nunca hardcodeados.
- Nada de `unwrap()` ni `expect()` en rutas de runtime. Los errores se manejan con fallback. El único lugar con `expect` permitido es la carga inicial de datos propios, con un mensaje claro.

---

## 16. Calidad y verificación

Obligatorio, y lo corrés vos:

1. `cargo build --release`, `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` sin errores ni warnings.
2. Tests unitarios y de propiedades (`proptest`):
   - Feistel: `F⁻¹(F(x)) == x`.
   - Inscripción: frase → dirección → frase.
   - Determinismo: el mismo sitio dos veces da el mismo resultado, byte a byte.
   - Detección de palabras con casos de borde.
   - Tabla de rareza.
   - Léxico sin duplicados y con los mínimos por categoría.
   - Recetas sin ingredientes inexistentes.
   - Árbol de habilidades sin nodos huérfanos.
3. **Test de estrés headless:** generar 10.000 sitios en pisos 1–60 sin pánicos, con todas las validaciones de alcanzabilidad cumplidas, tiempo medio < 30 ms y la distribución de palabras por piso dentro del rango de la sección 5.
4. **Simulación de progresión:** un bot headless simple recorre sitios, recolecta y fabrica, y verifica que siempre se puede llegar al tier siguiente sin depender de la suerte (no hay softlocks de recursos).
5. **Verificación visual:** modo `--screenshot <seed> <floor> <q> <r>`, que renderiza un sitio y guarda un PNG. Generá capturas de al menos 8 sitios variados (biomas, luz alta y baja, élite, guardián) y **miralas vos mismo**. Corregí bleeding, costuras, y-sort incorrecto, sombras rotas, contraste o legibilidad del texto.
6. **Rendimiento:** 60 FPS estables en release con 40 enemigos, 64 luces y partículas. Agregá un overlay de debug (F3) con FPS, entidades y luces.
7. Al final, `code-review` sobre todo el proyecto, y corregí lo que encuentre.

## 17. Definición de terminado

- [ ] Todo lo de las secciones 3 a 14 implementado y conectado. Nada de "TODO" en el juego.
- [ ] Partida completa posible: menú → tutorial → 6 guardianes → final del piso 30 → créditos → modo infinito.
- [ ] Todos los checks de la sección 16 en verde.
- [ ] `README.md` con cómo compilar, controles, cómo funciona la Biblioteca (semillas, palabras, rareza, Catálogo) y flags de debug.
- [ ] `DECISIONS.md` con las decisiones tomadas y por qué.

## 18. Forma de trabajo

- Llevá un `PROGRESS.md` con la checklist completa y actualizala a medida que avanzás. Si la sesión se corta, la siguiente retoma desde ahí sin rehacer trabajo.
- Implementá en el orden que te parezca más sólido: lo lógico es empezar por la base pura (babel y world), seguir con render e iluminación, gameplay, progresión, UI/audio/guardado y terminar con pulido. Pero no te detengas a entregar entre etapas.
- Compilá y corré los tests de forma continua. No acumules código sin compilar.
- Cuando termines, dame un resumen corto: qué hay, cómo jugarlo, capturas generadas y cualquier limitación real que haya quedado.
