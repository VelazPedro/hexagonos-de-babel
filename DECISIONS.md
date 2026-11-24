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
