# Decisiones y por qué

## Entorno
- **Bevy `=0.20.1`**, la última estable en crates.io al empezar. Las demás versiones están
  fijadas con `=` en `Cargo.toml`.
- **context7 no estaba autorizado** en la sesión. Con la aprobación del usuario, la API se
  verificó contra el **código fuente de la versión exacta** (`~/.cargo/registry`: módulos,
  doc comments y ejemplos de `bevy-0.20.1`).
- Bevy se compila sin features por defecto, solo con lo que se usa (2D, estados, winit, gilrs,
  audio wav, png, x11).
- No se usan `rand`, `noise` ni `dirs`. El muestreo está implementado sobre
  `ChaCha8Rng::next_u32/u64`, para que un cambio de distribuciones en una biblioteca nunca
  altere mundos ya generados. La carpeta de datos sale de variables de entorno.

## Arquitectura
- **Dos crates.** `babel_core` contiene toda la lógica, sin Bevy: no solo `babel/` y la
  generación, también la simulación de juego (combate, IA, botín, progresión). Bevy es la capa
  de entrada, dibujo, luz, audio y ventana. Así las reglas reales se prueban headless, el bot
  de progresión usa el mismo código que el jugador y la superficie expuesta a cambios de API de
  Bevy es mínima. El árbol de §15 se mantiene, repartido entre ambos crates.
- La simulación usa estructuras propias en lugar de entidades ECS. Hay un único mundo activo y
  pequeño (menos de 100 actores), y así se evitan conflictos de préstamos entre sistemas y el
  orden queda explícito y determinista.

## Render
- El **pixel art se compone en CPU** (framebuffer 482×272) y la **luz y el post-proceso en
  WGSL**. Así no puede haber bleeding ni costuras, el y-sort es el orden de dibujo y la fuente
  y la interfaz son exactas al píxel. El atlas igual se arma con padding de 1 px y extrusión.
- **Luz sin render graph propio** (más estable entre versiones de Bevy). La CPU calcula el
  polígono de visibilidad de cada luz sobre la grilla, y una malla con un abanico por luz se
  rasteriza con mezcla aditiva en un light map `Rgba16Float`. Una pasada de composición aplica
  blur de 9 muestras, `escena × (ambiente + luz)`, emisivos y bloom.
- La capa de la escena tiene 1 px de margen. La fracción de la cámara se aplica solo al
  escalar (recorte `rect` del sprite final), nunca al mundo.
- La luz ambiente usa una curva perceptual, para que un nivel de 0,15 se vea oscuro pero
  legible. Las luces puntuales pesan menos en galerías ya muy iluminadas.
- La interfaz se dibuja en la resolución del juego con la fuente propia, no con `bevy_ui`.
- La fuente es proporcional (hasta 5×7). Con ancho fijo, los renglones de 80 caracteres del
  lector no entrarían en 480 px.

## Biblioteca y generación
- La dirección caminable se empaqueta como `piso:32 | zigzag(q):32 | zigzag(r):32`. La
  probabilidad de que una frase buscada caiga ahí es despreciable, y si pasa, se va al sitio
  caminable, como pide el diseño.
- **λ (tasa de palabras reales).** El ejemplo del diseño (0,04–0,22) daría cientos de palabras
  por sitio, porque un cuerpo tiene unos 1500 tokens. Pesan más los objetivos medidos (4–8 en
  el piso 1 y 20–35 en el 30), así que λ va de 0,0031 a 0,0156. Un test lo verifica.
- El generador a veces emite **pares y frases canónicas** a propósito. Dos palabras reales casi
  nunca quedarían pegadas por azar, y no habría élites ni objetos canónicos. En 10.000 sitios
  hay unos 0,4 élites por sitio y un 3 % con frase canónica.
- **Puertas.** Cada sitio abre una puerta que acerca a la escalera más próxima y una de
  «deriva» (NE, SE o S). Esas tres direcciones siempre aumentan `2q+r`, así que ningún grupo
  de hexágonos puede quedar cerrado. La deriva se agregó porque el bot de progresión
  **encontró un softlock real** en el piso 22 con puertas puramente aleatorias.
- Una puerta que usaste queda abierta (parte del delta del sitio). Así volver siempre es
  posible sin depender de los vecinos.
- Las **escaleras** principales forman una grilla (una cada pocos hexágonos), además de las que
  agregan las palabras «escalera», «arriba» y «abajo».
- Los nodos sólidos solo se colocan donde las 8 casillas vecinas son transitables, así que no
  pueden cortar un camino. Si un trazado no conecta las 6 puertas, se reintenta con una
  sub-semilla y, como último recurso, se abren pasillos.

## Progresión y balance
- **Garantías por profundidad**: cada material de tier aparece en todos los sitios desde cierto
  piso. La simulación completa en 12 mundos alcanza los tiers en los pisos 1, 3, 8, ≤13 y 18.
- La Lente decide cuántas páginas se leen (1, 2 o 3), y «Página de más» suma una.
- Los nodos del hexágono propio vuelven a aparecer, para que la base nunca se quede sin
  materiales básicos. Los demás quedan agotados.
- Los sitios despejados no repueblan enemigos, así que no se puede farmear sin fin el mismo
  lugar.
- Con el inventario lleno, el objeto más débil se recicla solo en tinta, para no perder botín.
- Al morir se pierde el 20 % de cada material, redondeado. Si morís otra vez, la bolsa anterior
  se suma a la nueva.
- El Catálogo no permite escapar de un Guardián.
- Los guardianes de pisos superiores a 30 dan «Fragmentos infinitos» acumulables.

## Juego
- El daño solo sale de ataques telegrafiados, nunca del contacto. Los enemigos tienen un tiempo
  de gracia al aparecer y al entrar vos.
- La galería de espejos crea **reflejos** inofensivos de los enemigos, que se rompen de un golpe.
- Cada palabra de criatura fija el arquetipo, una rotación de color y un adorno.
- La cruceta del gamepad tiene doble uso: atajos al jugar y navegación en los menús.

## Guardado
- RON con campo `version`. La escritura es temporal + `fsync` + rename. El respaldo solo se
  reemplaza si el archivo actual se puede leer. Ante un archivo dañado o de otra versión, se
  carga el respaldo o se muestra un aviso.

## Pruebas
- Además de lo pedido, hay tests de integración que manejan todo el shell **sin ventana** y solo
  con entradas de teclado. Comprueban que todos los controles de cada pantalla son alcanzables
  sin mouse.

## Segunda tanda: personaje, sonido, habilidades e interfaz

### Protagonista
- El pedido fue un héroe más épico basado en Kaladin Stormblessed. Es un **diseño original
  inspirado** en esa figura: lancero de pelo largo y oscuro, abrigo azul, capa, marca en la
  frente y volutas de luz. El juego no usa nombres, emblemas ni textos de la saga. Si el juego se
  publica, conviene mantenerlo así.
- El sprite pasó de 16×20 a 20×28 y se arma **por capas** (`assets/sprites/hero.ron`): cabeza,
  torso según el cuerpo, faldones, piernas, peinado delante y detrás, capa, lanza, cara y marca.
  Los dígitos y `a`/`A`/`x` de la paleta son marcadores que se recolorean según la elección.
- Se compone un atlas chico al cambiar la apariencia, así que dibujar cuesta lo mismo que antes.
- «Género» se ofrece como **Cuerpo**: Hombre, Mujer o No binario. Cambia silueta y rasgos, no
  las estadísticas, y todos los peinados y colores sirven para los tres.
- La apariencia vive en la partida (`player.look`) con valor por defecto, así que las partidas
  anteriores cargan sin cambiar `SAVE_VERSION`. Los índices fuera de rango se ajustan.
- La lanza es parte de la figura. Las herramientas del juego no cambian: en los golpes cargados y
  giratorios la lanza barre el arco, como efecto visual.

### Volumen
- Eran dos errores. La zona que restaba no coincidía con la flecha `<` dibujada, y ← / → además
  movían la selección, porque las filas están alineadas en vertical. Ahora lo que se dibuja y lo
  que responde al mouse salen de la misma función (`adjuster_zones`), y un control que usa
  ← / → consume esa dirección. Hay un test para cada error.

### Sonido
- Las ondas cuadradas y de sierra sin limitar en banda generaban aliasing. Ahora hay ondas con
  pocos armónicos, notas más graves, un ataque mínimo de 6 ms y un paso bajo de 2,3 kHz (2 kHz
  en la música).
- «Sin agudos» se mide: un test limita la parte de la señal por encima de 3 kHz. Antes lo
  superaban 21 de los 51 efectos y el tema de guardián.

### Interfaz
- Criterio: **mostrar cuando hace falta**. El HUD enseña números, nombre de herramienta y de
  galería solo al cambiar. El cinturón oculta lo que no llevás. El HUD no se dibuja detrás de
  los menús. Los detalles aparecen para el elemento señalado.
- Los pies de texto se reemplazaron por dos o tres teclas con su verbo, según el dispositivo.
- El teclado en pantalla está plegado en «Tu mundo» y en el Catálogo. Escribir con el teclado
  físico funciona igual.
- La pausa es una grilla de íconos. Controles tiene tres pestañas. El inventario separa mochila,
  materiales y herramientas. El cofre lista solo lo que existe.
- Las habilidades son hexágonos con un ícono propio cada una (un test lo exige) y una sola
  tarjeta con el detalle.

## Equipo visible, rendimiento y publicación

### Equipo en el personaje
- La **forma** de cada pieza sale de la primera palabra del objeto (hacha, yelmo, capucha,
  sandalia, anillo…). Un test exige que toda palabra de arma, armadura, calzado y farol tenga
  forma. Los amuletos sin forma propia se cuelgan al cuello.
- El **color** sale de la rareza. En las armas y herramientas, el metal sale del tier de la
  cuchilla o de la herramienta. Desde Épico (o tier Cristal) emiten un brillo suave.
- La mano muestra el arma con la Cuchilla activa, o la herramienta que uses. En ese caso el arma
  va a la espalda. Sin arma, la Cuchilla va montada en una lanza.
- Durante un golpe, lo que está en la mano gira por el arco.
- El héroe solo se vuelve a componer cuando cambia la apariencia o el equipo (1,3 ms).
- La pantalla «Tu personaje» lo muestra sin equipo, para que un yelmo no tape lo que elegís.
- Las reliquias son pasivas y no se dibujan.

### Rendimiento
- Se midió antes de tocar. El cuadro ya cumplía (3,5 ms de CPU); lo que bajaba de 60 FPS eran
  los **tirones**: música generada en el hilo principal (88 ms) y guardado con `fsync` en cada
  cambio de sala (hasta 55 ms). Los dos pasaron a hilos aparte. La cola de guardado se vacía
  antes de cargar, de empezar otra partida y de cerrar.
- El dibujo de la escena pasó de 2,7 a 1,35 ms (fondo y suelo en una pasada, copias por filas,
  camino directo para sprites sin escala). Una huella de todos los píxeles de 7 escenas durante
  90 cuadros dio **idéntica** antes y después.
- No se bajó resolución, luces, partículas ni efectos.
- Compilación de release con `lto = "fat"`, una sola unidad de código y sin símbolos.
- `--bench` del `.exe` final (40 enemigos, 64 luces, 900 partículas; GTX 1650): con Vulkan,
  385 FPS de media y p99 303; con DirectX 12 y el proceso limitado a 2 hilos, 269 y 206.
  No se probó en una gráfica integrada.

### Un solo ejecutable
- Datos, sprites y shaders van dentro del programa. `HEXBABEL_ASSETS` permite reemplazar datos
  y sprites al desarrollar (los shaders requieren recompilar).
- El runtime de C se enlaza estático y no hay ventana de consola; un error queda en `arranque.log`.

### Que abra en cualquier PC
- El usuario probó el `.exe` en una PC sin nada instalado y no abrió. No hubo registro de esa PC.
  Se reprodujeron dos causas posibles: sin controlador de video, Windows ofrece uno emulado en el
  que el dispositivo gráfico se suspende antes del primer segundo (le pasa también a una app de
  Bevy 0.20.1 con solo un sprite; solo lo evitaban los shaders sin optimizar, a unos 4 FPS); y
  sin una placa compatible con DirectX 12 o Vulkan el programa terminaba al arrancar. En los dos
  casos no se veía ningún mensaje, porque no hay consola.
- **Modo compatible:** `render/software.rs` repite en el procesador `light.wgsl`,
  `composite.wgsl` y el escalado, y `compat.rs` muestra el resultado con GDI. Bevy arranca con
  `backends: None`, sin tocar ningún controlador. Comparado con la placa en cuatro cuadros
  idénticos, ningún píxel difiere en más de 2 niveles de 255. Si cambia un shader, hay que
  cambiar también `software.rs`. Se fuerza con `--compat`. `--bench` en este modo (Ryzen 7 5700G, ventana de 1280×720):
  104 FPS de media y p99 85 con todo el procesador; 91 y 74 con un solo hilo. En un procesador
  más viejo dará menos.
- **Arranque vigilado:** el programa ejecuta una copia de sí mismo y la observa (`startup.rs`).
  Si muere o no
  responde en 30 s antes de andar bien, prueba solo DirectX 12 y después el modo compatible; si Windows solo
  ofrece el controlador emulado, pasa directo al compatible. Si todo falla, o si se cae mientras
  se juega, muestra un mensaje que indica dónde está `arranque.log`. Hace falta un segundo proceso
  porque un controlador puede cerrar el programa sin dar aviso. Las herramientas (`--bench`,
  `--screenshot`) no se vigilan.

### Publicación
- Icono: lo compone `src/render/icon.rs` con el mismo código que dibuja al protagonista (aspecto
  y equipo iniciales; no cambia con la personalización). Fondo de galería y marco hexagonal; 16,
  24 y 32 px se componen aparte y el resto se amplía por píxeles enteros. `build.rs` convierte el
  `.ico` en un `.res` que enlaza MSVC, sin compilador de recursos ni crates nuevos.
- La ventana usa ese mismo icono. Para eso `winit` figura en `Cargo.toml`: es una excepción a la
  lista de crates de `PROMPT.md`, pero no agrega código (es la versión que Bevy ya compila).
- Los commits usan el correo `noreply` de GitHub, para no publicar un correo personal. El
  historial local se reescribió antes del primer `push`.
- `scripts/build-release.ps1` reescribe las rutas de la máquina y falla si el binario todavía
  contiene el nombre de usuario o la carpeta del proyecto.
- El `.exe` se versiona en la raíz del repositorio, sin *release*, por pedido del autor. Es la
  única excepción a `*.exe` en `.gitignore`. Cada versión nueva suma su peso al historial, así
  que conviene subirlo solo cuando cambie el juego. GitHub rechaza archivos de más de 100 MiB.
- No se agregó licencia: es una decisión del autor. Sin licencia, el código se puede ver pero
  nadie tiene permiso para reutilizarlo.

## Multijugador
Diseño completo, reglas y seguridad en [MULTIPLAYER.md](MULTIPLAYER.md). Lo que conviene recordar:

- **Autoridad del anfitrión.** El invitado envía controles y acciones y dibuja un espejo; no
  simula. Así no hay dos mundos que se desincronicen y un invitado no puede inventar resultados.
- **Dos jugadores, un `Game`.** El compañero se guarda aparte (`Companion`) y se intercambia con
  `body/player/derived` mientras actúa (`as_player`), de modo que la lógica de un jugador no cambió.
- **Internet sin servidor propio:** UPnP + STUN y la dirección pública dentro del código. Se
  descartó un servidor de retransmisión (resolvería el CG-NAT, pero alguien tiene que alojarlo) y
  la perforación de NAT (necesita un punto de encuentro y falla con muchos routers). Se descartó
  Steam/EOS (cuentas y publicación). Se prefirió ser claros cuando no se puede (CG-NAT) a
  prometer lo que no es seguro.
- **Sin cifrado, con firma.** BLAKE3 con clave derivada del secreto de la sala y de dos números
  aleatorios, por dirección y con contador. Evita suplantar, alterar, repetir y reordenar; no
  oculta la partida. Cifrar pedía un primitivo más (ChaCha) y el riesgo que cubre es el de ver, no
  el de manipular.
- **Seguridad antes que comodidad:** todo lo recibido se valida con límites (hubo hallazgos reales:
  un vector gigante en un ataque congelaba el dibujo; listas de efectos y de pantallas crecían sin
  tope).
- **Crates fuera de la lista de `PROMPT.md`:** `postcard` (formato binario compacto de serde) y
  `winit` (icono de ventana). `blake3` y `serde` ya estaban permitidos. UPnP y STUN se escribieron
  con la biblioteca estándar en lugar de sumar un crate.
- **Dos PC reales sin probar.** Las pruebas son dos juegos reales por TCP en una PC, un router
  UPnP simulado (incluidos puerto ocupado, solo concesiones permanentes y respuestas por partes) y
  un STUN simulado. En la red de esta máquina el router no ofrece UPnP, y `--net-check` lo dijo.

## Nombre y mascotas
Diseño en [docs/superpowers/specs](docs/superpowers/specs/2026-11-26-mascotas-y-nombre-design.md).

- **En las reglas, no en el dibujo.** El estado en vivo es `Body::pet`: el intercambio de
  jugadores del cooperativo y la sincronización lo llevan sin código aparte. Tratarla como un
  enemigo aliado habría arrastrado vida, colisiones e IA hostil.
- **No recibe daño** y sin habilidades no ataca ni junta nada: es compañía, no un requisito.
- **Muerde con una fracción del daño del dueño**, así crece con el equipo sin un balance propio.
  No pasa por `strike_enemy`, para no disparar robo de vida, quemaduras ni ecos del jugador.
- **Árbol aparte (`pet_skills.ron`) y puntos compartidos**, como se pidió. El árbol del héroe y
  sus tests no cambiaron.
- **Se ubica al costado, nunca detrás.** Con la primera versión quedaba tapada por el personaje;
  se vio en una captura de dos ventanas y quedó fijado con un test. En cooperativo cada una toma
  un lado distinto.
- **Dibujo propio por piezas** (`pet.ron` + `render/pet.rs`): cada opción cambia solo su pieza.
- **Nombres:** hasta 12 caracteres de un juego cerrado (letras con tilde, dígitos, espacio, `-`,
  `'`), que la fuente sabe dibujar. Lo que llega de la otra PC se limpia igual.
- **Protocolo 2:** las dos PC necesitan la versión nueva.

## Piedras de afilar y conexión directa
Diseño en [docs/superpowers/specs](docs/superpowers/specs/2026-11-26-dano-y-conexion-directa-design.md).

- **Piedras fabricables y no objetos al azar:** el pedido era poder pegar más fuerte; una receta
  no depende de la suerte ni cambia qué objeto sale de cada palabra. Piden solo materiales que
  las herramientas de ese nivel ya piden (hay un test).
- **Menos vida:** 20 % menos en enemigos y guardianes, y 11 % por piso en vez de 13 %. El daño
  base de las armas pasó de 4 a 6.
- **Perforación UDP con código de respuesta** en lugar de un servidor: nada que mantener. El
  costo es un pegado más.
- **Canal confiable propio** en lugar de una biblioteca (KCP, QUIC): sin dependencias nuevas y
  con las reglas separadas del socket, así se prueban redes con pérdida sin red. Encima viajan
  el saludo y los mensajes firmados de siempre.
- **Ventana mínima alta (24) mientras el otro lado responde:** en redes hogareñas se pierden
  paquetes sin que haya congestión; achicando a la mitad no llegaba a 200 kB/s con 3 % de
  pérdida. Si el otro lado queda mudo, baja a 4 y espera cada vez más. Una actualización normal
  de la partida pesa alrededor de 1 kB.
- **El anfitrión confirma antes de dejar de golpear:** sin eso, según el orden de llegada, el
  invitado podía quedarse sin prueba de haber sido oído. Apareció como un test que fallaba a
  veces y quedó fijado con uno que falla siempre sin el arreglo.
- **Protocolo 3** y códigos nuevos: las dos PC necesitan la versión nueva.

## Descripciones, martillo y recetas fijadas
- **Las descripciones se calculan con las reglas** (`progression/guide.rs`): los números salen de
  las mismas constantes y fórmulas que usa el juego. Si algo no sirve todavía, o no suma nada
  (las lentes de cristal y áurea), lo dice.
- **El Martillo no tenía utilidad propia** porque al golpear se usaba el nivel de la herramienta
  correcta aunque no estuviera en la mano. Ahora hay que tenerla en la mano. Las estanterías
  pasaron al Martillo; la Cuchilla conserva enredaderas, códices y tinteros. La mascota ayuda
  con lo que su dueño esté rompiendo.
- **Material extra del Martillo:** +1 por nivel (máximo +5) por cosa rota, repartido entre lo que
  da. Es dato (`hammer_extra`).
- **Recetas fijadas:** hasta 3, guardadas por jugador. Cuentan solo lo que llevás encima, que es
  lo que gasta la fabricación; lo del cofre se menciona aparte. Se fijan y sueltan desde
  Fabricación y Construir.
- **Protocolo 4:** las dos PC necesitan la misma versión.

## Mesas, pisos y lobby
Diseño en [docs/superpowers/specs](docs/superpowers/specs/2026-11-26-mesas-pisos-y-lobby-design.md).

- **Mejoras elegidas y no encantamientos al azar:** el juego no depende de la suerte. Una por
  objeto, para que haya que elegir; cambiar conserva el nivel, para que probar no se castigue.
- **Son datos** (`assets/data/upgrades.ron`): 3 mesas, 9 objetos y 27 mejoras. Lo que no es una
  estadística ya existente es un `Perk` que el juego consulta donde corresponde.
- **Siempre activas**, se tenga lo que se tenga en la mano. Cada jugador tiene las suyas.
- **Antorchas:** quedan en la galería, así que vale la mejor mejora entre los jugadores. El fuego
  hace daño de a segundos enteros, para que sea exactamente el que dice.
- **Sin aviso emergente al mejorar:** tapaba la lista; la fila muestra el nivel.
- **Piedras de afilar en la Forja**, y la Forja templa las herramientas: es lo que la distingue.
- **Materiales por tercios:** `(q − r) mod 3` da a un hexágono y sus vecinos los tres tercios,
  así lo que falta está siempre a una puerta. La primera versión de la variedad no lo garantizaba
  sin recorrer mucho.
- **Puertas entre pisos** con Cuchilla y Martillo, las dos herramientas de juntar. Cada una se
  puede conseguir arriba de su puerta (hay un test, y el bot termina las 12 semillas).
- **Lobby:** lo roto en el hexágono base se guarda como en cualquier otro. Lo básico está en los
  vecinos.
- **Protocolo 5:** las dos PC necesitan la misma versión.

## Interfaces con identidad y estadísticas
Diseños en [docs/superpowers/specs](docs/superpowers/specs/2026-11-27-interfaces-con-identidad-design.md)
y [estadísticas](docs/superpowers/specs/2026-11-27-estadisticas-design.md).

- **Un `Place` por identidad** (`src/ui/theme/`). El shell elige el de cada pantalla y las piezas
  compartidas lo leen: casi ninguna pantalla cambió su disposición. El `match` es exhaustivo, así
  una pantalla nueva no compila sin estilo.
- **Marcos, texturas y animaciones dibujados con código**, con emblemas chicos en pixel art. Sin
  imágenes nuevas, salvo el ícono de Estadísticas.
- **Fondos y paneles se dibujan una vez** (`ArtCache`) y se copian. En la medición optimizada, la
  pantalla más pesada pasó de 2,08 ms a 0,87 ms por cuadro (mediana). El primer cuadro de cada
  pantalla, que dibuja su arte, no pasa de 3,8 ms en esta PC.
- **Contraste probado:** texto normal y apagado ≥ 4,5:1, bloqueado ≥ 3:1 y rarezas ≥ 3:1 en
  todas las paletas.
- **Los colores de rol** (apagado, bloqueado, encabezado) toman el tono del lugar al dibujar el
  texto. Los bordes con significado (rareza, elegido) se respetan.
- **El Lector conserva la página oscura** para que las rarezas se distingan.
- **Pantalla de juego:** solo marcos. Un test limita el área que ocupa a la de antes (+3 %).
- **Estadísticas por personaje** (`PlayerData::records`). Las cuentas (`Tally`) viajan aparte en
  el multijugador: si no, cada golpe reenviaría inventario y Glosario. El tiempo se anota de a
  5 segundos.
- **En cooperativo** cuentan para los dos los enemigos, hexágonos y materiales, que ya se
  compartían. El daño por quemadura continua se anota a quien esté actuando al aplicarse, no
  siempre a quien prendió el fuego.

