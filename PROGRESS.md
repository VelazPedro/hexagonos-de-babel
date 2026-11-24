# Progreso — Hexágonos de Babel

Para retomar: leer este archivo, `DECISIONS.md` y `git log`.

## Estructura
- `core/` (`babel_core`, sin Bevy): babel, data, world, game (simulación), progression, save, sim (bot).
- `src/`: capa Bevy (render, iluminación WGSL, UI, audio, debug).
- `assets/data/*.ron`, `assets/sprites/*.ron`, `assets/shaders/*.wgsl`.

## Checklist
- [x] §1 Stack fijado (bevy =0.20.1), perfiles de compilación
- [x] §5 Alfabeto, semillas blake3, Feistel + cycle-walking, inscripción biyectiva, cuerpo con λ(profundidad)
- [x] §6 Léxico (1218 palabras), Aho-Corasick, frases, 61 frases canónicas
- [x] §7 Rareza
- [x] §8 SiteRecipe, 11 biomas, 7 layouts, validación por flood fill, recursos
- [x] §9 10 arquetipos de IA, 24 modificadores, élites, drops (simulación)
- [x] §10 Recursos, herramientas por tier, base, fabricación, Glosario, Scriptorium, 32 habilidades, Catálogo, 6 guardianes, reliquias, hitos, final e infinito (simulación)
- [x] §11 Combate, estados, muerte y bolsa (simulación)
- [x] §14 Guardado RON atómico con respaldo y versión (core)
- [x] §16.2 Tests unitarios y proptest (63 verdes)
- [x] §16.3 Estrés de 10.000 sitios (media 0,36 ms)
- [x] §16.4 Simulación de progresión (12 mundos hasta el piso 31)
- [ ] §3 Sprites .ron, atlas, variantes procedurales, fuente 5x7, `--atlas-dump`
- [ ] §4 Pixel target 480x270, cámara subpíxel, 2.5D, iluminación WGSL, bloom
- [ ] §11 Presentación del game feel (flash, shake, números, partículas)
- [ ] §12 HUD y todas las pantallas, teclado/mouse/gamepad, opciones, remapeo
- [ ] §13 Audio procedural (SFX + música por bioma + guardián)
- [ ] §14 Autoguardado conectado en la app
- [ ] §16.1 build --release / clippy -D warnings / fmt
- [ ] §16.5 `--screenshot` y revisión visual de ≥8 capturas
- [ ] §16.6 Rendimiento 60 FPS (`--bench`, overlay F3)
- [ ] §16.7 code-review
- [ ] README.md, DECISIONS.md
