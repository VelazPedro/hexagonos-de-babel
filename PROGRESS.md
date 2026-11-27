# Progreso — Hexágonos de Babel

Para retomar: leer este archivo, `DECISIONS.md` y `git log`.

## Checklist
- [x] §1 Stack fijado (bevy =0.20.1) y perfiles
- [x] §3 Sprites .ron, atlas con padding y extrusión, tiles y variantes procedurales, fuente propia, `--atlas-dump`
- [x] §4 Target 480×270, escala entera, cámara subpíxel, 2.5D, y-sort, altura, luz WGSL con sombras, bloom, 64 luces
- [x] §5 Biblioteca: semillas, Feistel, inscripción, cuerpo
- [x] §6 Léxico (1218), Aho-Corasick, frases, 61 canónicas
- [x] §7 Rareza
- [x] §8 SiteRecipe, 11 biomas, 7 trazados, flood fill, recursos
- [x] §9 10 arquetipos, 24 modificadores, élites, drops
- [x] §10 Recursos, tiers, base, fabricación, Glosario, Scriptorium, 32 habilidades, Catálogo, 6 guardianes, reliquias, hitos, final e infinito
- [x] §11 Combate y game feel
- [x] §12 HUD y pantallas, drag and drop, teclado/mouse/gamepad, opciones y remapeo, tutorial
- [x] §13 51 efectos procedurales, música por bioma y tema de guardián
- [x] §14 Guardado atómico con respaldo y autoguardado
- [x] §16.1 build --release, clippy -D warnings, fmt
- [x] §16.2 Tests unitarios y proptest
- [x] §16.3 Estrés de 10.000 sitios (media 0,38 ms)
- [x] §16.4 Simulación de progresión (12 mundos hasta el piso 31)
- [x] §16.5 Capturas en `screenshots/`, revisadas y corregidas
- [x] §16.6 `--bench`: 257 FPS de media, p99 203 FPS (GTX 1650); overlay F3
- [x] §16.7 code-review (dos pasadas: 10 + 11 hallazgos, todos corregidos; la mayoría con test de regresión)
- [x] README.md, DECISIONS.md

## Segunda tanda de pedidos
- [x] `.exe` en la carpeta raíz (se genera con `scripts/build-release.ps1`)
- [x] Protagonista nuevo por capas y pantalla «Tu personaje» (cuerpo, piel, peinado, colores, capa, marca)
- [x] Volumen: flecha `<` y teclas ← / → (dos errores, con tests)
- [x] Sonido y música sin agudos (con tests de espectro)
- [x] Árbol de habilidades con 32 íconos
- [x] Rediseño de HUD y menús
- [x] fmt, clippy y 98 tests en verde (con los 2 largos); bench 289 FPS de media, p99 231
- [x] 35 capturas regeneradas y revisadas

Sin `code-review` formal sobre esta tanda.

## Publicación, rendimiento y equipo visible
- [x] Un solo `.exe` (recursos dentro, runtime estático, sin consola, sin rutas personales)
- [x] `.gitignore` ampliado; autor de los commits con el correo `noreply`
- [x] Repositorio público: https://github.com/VelazPedro/hexagonos-de-babel
- [x] Equipo visible en el personaje (armas, herramientas, armaduras, calzado, faroles, amuletos)
- [x] Optimización con mediciones y píxeles idénticos (ver DECISIONS.md)
- [x] fmt, clippy y 107 tests en verde (105 más los 2 largos)
- [x] Icono propio para el `.exe` y la ventana
- [x] Que abra en cualquier PC: arranque vigilado, modo compatible sin placa, aviso y
  `arranque.log`. 122 tests habituales en verde; probado con el `.exe` simulando PC sin
  controlador, Vulkan roto, cuelgues y cierres. Falta probarlo en la PC donde no abría.

## Multijugador
- [x] Salas de 2 jugadores con código; cada uno guarda su progreso; mundo del anfitrión
- [x] Un hexágono a la vez, pasan los dos; pausa individual, el mundo se detiene si pausan los dos
- [x] Caídos y reanimación; botín y experiencia compartidos; guardianes con más vida de a dos
- [x] Seguridad: autenticación mutua, mensajes firmados, validación de todo lo recibido, límites
  y puerta de entrada contra intentos repetidos
- [x] Por internet sin VPN: UPnP + dirección pública (STUN), estados explicados en pantalla
  (abierta / a mano / CG-NAT / sin internet) y `--net-check`
- [x] 84 tests de la aplicación y 102 del núcleo en verde; dos juegos reales por TCP en una PC
- [ ] Probar entre dos PC reales de redes distintas
- [x] Nuevo `.exe` en la raíz con el multijugador (51,3 MiB, sin datos personales; dos ventanas
  reales conectadas; `--bench` 312 FPS, sin cambios)

## Nombre y mascotas
- [x] Nombre del personaje y de la mascota, tenue en pantalla, con opción para ocultarlo
- [x] Mascota desde el inicio, personalizable, con árbol de 10 habilidades y puntos compartidos
- [x] Multijugador: cada uno con la suya (protocolo 2)
- [x] Dibujo revisado en una hoja de poses y en dos ventanas conectadas
- [ ] Balance jugando
- [x] Nuevo `.exe` en la raíz con las mascotas (51,5 MiB, sin datos personales; dos ventanas
  reales conectadas; `--bench` en GTX 1650: media 288–346 FPS y peor 1 % 163–202 FPS en tres
  corridas; una cuarta quedó en 60 FPS, el límite de Bevy para ventanas sin foco)

## Piedras de afilar y conexión directa
- [x] 5 piedras de afilar (+3 de daño cada una), armas base 6, enemigos con menos vida
- [x] Canal confiable sobre UDP, probado con pérdida, desorden y duplicados
- [x] Conexión directa con código de respuesta; pantallas y errores explicados
- [ ] Probar entre dos PC reales de redes distintas
- [ ] Balance jugando
- [x] Nuevo `.exe` en la raíz (51,6 MiB, sin datos personales, solo DLL de Windows; dos ventanas
  reales conectadas por la vía normal y por la directa, esta durante 90 s; `--net-check` real:
  sin UPnP, red apta para la vía directa; `--bench` en GTX 1650: media 306–323 FPS y peor 1 %
  238–251 FPS en tres corridas)

## Descripciones, martillo y recetas fijadas
- [x] «Qué hace» y «En qué te ayuda» antes de fabricar o construir
- [x] Cada cosa se recolecta con su herramienta en la mano; estanterías para el Martillo;
  +1 material por nivel del Martillo (máx. +5); cartel y tutorial actualizados
- [x] Hasta 3 recetas fijadas, con lo que falta a la vista
- [x] 147 tests del núcleo, 134 de la aplicación y las 2 pruebas largas en verde; clippy limpio
- [ ] Probar jugando

## Mesas, pisos y lobby
- [x] Lo roto en el hexágono base no vuelve
- [x] Puertas: Cuchilla y Martillo para bajar a los pisos 4, 8, 13 y 18
- [x] Materiales distintos por hexágono, con el que falta a una puerta de distancia
- [x] 27 mejoras en Forja, Mesa de encuadernador y Alambique, con pestaña propia; piedras de
  afilar en la Forja; protocolo 5
- [x] 184 tests del núcleo, 137 de la aplicación y las 2 pruebas largas en verde (el bot termina
  las 12 semillas); clippy limpio; capturas de la pantalla nueva revisadas
- [ ] Probar jugando (balance de mejoras, puertas y variedad)
- [ ] Las mejoras todavía no se pueden fijar en pantalla como las recetas

## Interfaces con identidad y estadísticas
- [x] 20 estilos, uno por lugar; todas las opciones con borde; emblemas, fondos y animaciones
- [x] Pantalla de juego enmarcada sin ocupar más lugar
- [x] Fondos y paneles guardados: la interfaz más pesada bajó de 2,08 a 0,87 ms (mediana)
- [x] Ventana de estadísticas en Pausa, 17 datos por personaje; protocolo 5
- [x] Corregidos «Vida máx..», la ficha del Inventario pegada al borde y la banda de créditos
- [x] Capturas de todas las pantallas revisadas
- [ ] Probar jugando: lectura, animaciones y estadísticas en una partida real
- [x] 2026-11-27: `.exe` de la raíz reconstruido con las tres secciones de arriba (54.546.432
  bytes, sin datos personales; 200 pruebas del núcleo y 169 de la aplicación en verde; arranca
  con la GTX 1650)
- [x] 2026-11-27: subido a GitHub todo lo hecho desde la publicación anterior, con ese `.exe`

## Pendiente de verificación humana
- Juego manual con teclado, mouse y gamepad físico.
- Escuchar el audio.
- Balance de duración (primer guardián en 20–30 min; final en 8–12 h).
- Compilación en Linux.
- Rendimiento en una PC de pocos recursos real, sobre todo con gráfica integrada (solo se
  midió en esta máquina: GTX 1650, también limitada a 2 hilos).
