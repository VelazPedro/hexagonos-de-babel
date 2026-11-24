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
- [x] §16.6 `--bench`: 259 FPS de media, p99 199 FPS (GTX 1650); overlay F3
- [ ] §16.7 code-review y correcciones
- [x] README.md, DECISIONS.md

## Pendiente de verificación humana
- Juego manual con teclado, mouse y gamepad físico.
- Escuchar el audio.
- Balance de duración (primer guardián en 20–30 min; final en 8–12 h).
- Compilación en Linux.
