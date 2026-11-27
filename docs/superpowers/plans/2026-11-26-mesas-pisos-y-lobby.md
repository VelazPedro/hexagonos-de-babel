# Mesas, pisos y lobby — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dar a cada mesa mejoras propias, variar los materiales por hexágono, pedir herramientas para bajar y dejar limpio lo roto en el lobby.

**Architecture:** Todo en `babel_core`: las mejoras son datos (`assets/data/upgrades.ron`) con reglas en `progression/upgrades.rs`; la generación cambia en `world/site.rs`; los requisitos en `world/floors.rs`. La interfaz suma una pestaña en Fabricación.

**Tech Stack:** Rust 2024, Bevy =0.20.1. Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-11-26-mesas-pisos-y-lobby-design.md`

## Global Constraints
- Determinista: la misma semilla y coordenada dan el mismo sitio. Nada depende de la suerte.
- Partidas viejas cargan igual (`serde(default)`). Todo lo recibido por red se valida.
- Tope de +5 materiales extra del martillo. Niveles I–III. Puertas 4/8/13/18 = tier 1/2/3/4.
- No ejecutar `scripts/build-release.ps1`. Compilar con `-j 2` y prioridad baja.
- Textos en español rioplatense. Clippy con `-D warnings`. Sin commits.

## Review Focus
1. Partida vieja en un piso más profundo sin las herramientas: puede quedarse y subir; solo se le pide al cruzar la siguiente puerta (Tarea 2).
2. Guardar editado o red con mejoras inventadas, de otro objeto o de nivel 200: se descartan o recortan (Tarea 4).
3. Un invitado mejora algo: cambia solo lo suyo y el anfitrión lo aplica (Tarea 4).
4. Mejora «más materiales» con martillo Áureo: sigue en +5 y la pantalla lo avisa (Tarea 4).
5. Antorchas con efectos y un jugador caído o sin antorchas puestas: no cura caídos ni falla (Tarea 5).

---

### Task 1: Lobby limpio
**Files:** `core/src/game/player.rs`, `core/src/game/tests.rs`
- [ ] Test `what_is_broken_at_home_stays_broken`: romper un nodo del base, irse a otro sitio, volver con `travel_home` y también recargar `Game::new` con el perfil: el nodo no está. Ver fallar; quitar la excepción del hogar; ver pasar.

### Task 2: Requisitos para bajar
**Files:** `core/src/world/floors.rs`, `core/src/game/{travel,player}.rs`, `core/src/progression/guide.rs`
**Produces:** `floors::gate(floor: u32) -> Option<u8>`; `Game::missing_for_stairs(&self) -> Option<(u32, u8)>` (piso de destino y tier que falta).
- [ ] Tests: `some_floors_ask_for_better_tools` (4→1, 8→2, 13→3, 18→4, otros nada); `stairs_stay_shut_without_both_tools` (solo cuchilla no alcanza, mensaje con «Hierro»; con ambas baja); `deeper_saves_only_meet_the_next_gate`; en cooperativo `either_player_opens_the_way`; `every_gate_can_be_met_above_it` (los materiales de cada herramienta del tier salen de nodos disponibles en pisos anteriores a la puerta); la descripción de `hammer_1` menciona «piso 4». El cartel de la escalera dice qué falta.

### Task 3: Variedad por piso
**Files:** `core/src/world/{site,tests}.rs`, `core/src/game/tests.rs`, `README.md`
- [ ] Tests en `world/tests.rs`: `every_material_is_here_or_next_door` (pisos 1..=30, muchas coordenadas sin guardián); `sites_of_one_floor_differ` (≥ 8 combinaciones en 40 sitios del piso 5 y ningún material de tier en todos); `newer_materials_are_rarer` (piso 8: plata en menos sitios que latón); `some_sites_are_rich`; `the_home_hexagon_is_as_it_was`. Ver fallar, implementar `site_nodes(depth, colour, dice)`, ver pasar junto con estrés y bot en release.

### Task 4: Mejoras de mesa (reglas)
**Files:** Create `assets/data/upgrades.ron`, `core/src/progression/upgrades.rs`; Modify `data/{types,mod,embedded,tests}.rs`, `progression/{player,stats,crafting,sanitize,guide,mod}.rs`, `game/{actions,sync}.rs`, `assets/data/recipes.ron`
**Produces:** `PlayerData::upgrades: BTreeMap<String, Fitted>` (`Fitted { id: String, level: u8 }`); `upgrades::{offer, check, apply, explain}`; `Derived::perk(Perk) -> f32`; `Game::upgrade(&str) -> Result<(), UpgradeError>`; `Action::Upgrade(String)`.
- [ ] Tests: datos completos (9 objetos, 3 opciones cada uno, 3 valores crecientes o iguales); nuevo/subir/cambiar conserva nivel y precio del nivel; tope III; Forja pide Hierro/Plata/Cristal; falta mesa, fuera de casa, sin materiales no gasta; las de estadística aparecen en `derive`; descuento de fabricación; saneado; invitado; `explain` con valor actual y siguiente, aviso de habilidad y del tope del martillo; piedras en la Forja.

### Task 5: Mejoras de mesa (efectos)
**Files:** `core/src/game/{player,mod,combat}.rs`, tests
- [ ] Un test por efecto especial: material extra del martillo con tope, fuerza del martillo, daño/quemadura/energía de páginas, alcance/calor/fuego de antorchas, curación/energía/cantidad de tónico, cantidad de tinta.

### Task 6: Pantalla de mejoras
**Files:** `core/src/game/{events,player}.rs`, `src/{shell,ui/craft,ui/state}.rs`, `src/shell_tests.rs`
**Produces:** `ScreenRequest::Crafting` lleva la mesa usada; `UiData::bench: Option<Station>`; pestaña «Templar» / «Encuadernar» / «Destilar».
- [ ] Tests: interactuar con la Forja abre con su pestaña; mejorar desde la pantalla; navegable sin mouse en las tres mesas; las explicaciones entran en el panel. Capturas revisadas.

### Task 7: Documentos y verificación
- [ ] README, DECISIONS, PROGRESS, MULTIPLAYER; protocolo 5; fmt, clippy, suite completa, pruebas largas en release; búsqueda de datos personales; confirmar que el `.exe` de la raíz no cambió.
