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

## Pendiente de verificación humana
- Juego manual con teclado, mouse y gamepad físico.
- Escuchar el audio.
- Balance de duración (primer guardián en 20–30 min; final en 8–12 h).
- Compilación en Linux.
- Rendimiento en una PC de pocos recursos real, sobre todo con gráfica integrada (solo se
  midió en esta máquina: GTX 1650, también limitada a 2 hilos).
