# Interfaces con identidad — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que cada pantalla tenga color, marco, fondo, emblema y animación propios, con todas las opciones bordeadas.

**Architecture:** Un módulo `src/ui/theme/` define un `Place` por identidad. El shell elige el `Place` según la pantalla y las piezas compartidas de `Ui` lo leen: así casi ninguna pantalla cambia su disposición. Fondos y texturas se dibujan una vez en un `ArtCache` y se copian.

**Tech Stack:** Rust 2024, Bevy =0.20.1. Sin dependencias nuevas. Todo se dibuja en el `Canvas` propio de 480×270.

**Spec:** `docs/superpowers/specs/2026-11-27-interfaces-con-identidad-design.md` y, para las Tareas 8 y 9, `docs/superpowers/specs/2026-11-27-estadisticas-design.md` (pedido a mitad de la ejecución)

## Global Constraints
- No cambia qué hace cada pantalla, sus controles ni sus reglas. Solo se mueve lo que un marco nuevo tape.
- Texto normal y apagado: contraste ≥ 4,5:1 contra paneles y opciones. Colores de rareza ≥ 3:1.
- Ninguna pantalla pasa de 4 ms de dibujo de interfaz en versión optimizada.
- Fondos opacos salvo Título, Pausa y Al caer.
- Sin animación en Inventario, Mapa, Lector y pantalla de juego.
- La pantalla de juego no ocupa más lugar que hoy.
- No ejecutar `scripts/build-release.ps1`. Compilar con `-j 2` y prioridad baja. Sin commits.
- Textos en español rioplatense. Clippy con `-D warnings`.
- La muestra en `scratchpad/spike-ui` es descartable: sirve de referencia visual, el código se escribe con pruebas.

## Review Focus
1. PC de pocos recursos y modo compatible: el fondo y las texturas no se recalculan en cada cuadro (Tarea 2).
2. Objeto o palabra de cualquier rareza sobre cualquier panel nuevo: se sigue leyendo (Tarea 1).
3. Títulos largos («Mesa de encuadernador», «Lámparas de retorno»): la placa y sus emblemas entran en la pantalla (Tarea 2).
4. Bordes con significado (rareza, elegido, faltan materiales): el estilo nuevo no los reemplaza (Tarea 2).
5. Marco grande sobre piezas chicas (campo de texto, ficha de una línea): usa el marco simple y no tapa texto (Tarea 2).

---

### Task 1: Identidades y paletas
**Files:** Create `src/ui/theme/mod.rs`, `src/ui/theme/tests.rs`; Modify `src/ui/mod.rs` (`pub mod theme;`)

**Interfaces — Produces:**
- `enum Place { Library, Rooms, Clockwork, Forge, Binder, Alembic, Scriptorium, Build, Chest, Lamps, Inventory, Skills, Glossary, Map, Catalog, Reader, Records, Ashes, Dawn, Hud }` (`Copy, Eq, Hash, Debug`); `Place::ALL: [Place; 20]`; `Place::theme(self) -> Theme`.
- `struct Theme { place, sky, floor, panel, grain_light, grain_dark, frame_dark, frame, frame_light, cell, cell_hot, edge, accent, heading, dim, faint, well: Rgba, frame_style: FrameStyle, air: Air }` (`Copy`).
- `enum FrameStyle { Filigree, Links, Gears, Riveted, Stitched, Pipes, Parchment, Chalk, Banded, Lantern, Strapped, Starry, Gilded, Charted, Hexed, Covers, Ledger, Cracked, Rays, Slim }`
- `enum Air { None, Pages, TwoMotes, Gears, Embers, Dust, Bubbles, Letters, Chalk, Glints, Fireflies, Stars, GoldDust, Hexagons, AshLeaves, Sparks }`
- `fn place_for(screen: Screen, bench: Option<Station>) -> Place` (match exhaustivo: una pantalla nueva no compila sin estilo).
- `fn sees_through(screen: Screen) -> bool` (Title, Pause, GameOver).
- `fn contrast(a: Rgba, b: Rgba) -> f32` (fórmula WCAG).

- [ ] **Step 1: Pruebas** en `theme/tests.rs`:
  - `every_screen_has_the_place_the_design_gave_it`: tabla completa pantalla → Place; `Crafting` con `Some(Forge|Binder|Alembic)` → ese Place y con `None` → `Library`; Title, NewGame, Look, Pause → `Library`; Coop → `Rooms`; Options, Controls → `Clockwork`; Ending, Credits → `Dawn`; GameOver → `Ashes`.
  - `no_two_places_look_alike`: para cada par de `Place::ALL`, difieren `(panel, accent)` y `frame_style`.
  - `text_can_be_read_on_every_place`: para cada Place, `contrast(INK, x) >= 4.5` para `x` en panel, cell, cell_hot, well; `contrast(theme.dim, x) >= 4.5` en panel, cell, well; `contrast(theme.faint, panel) >= 2.0`; `accent` y `heading` ≥ 4.5 sobre panel.
  - `every_rarity_stands_out_on_every_place`: `contrast(rarity, panel) >= 3.0` y `contrast(rarity, cell) >= 3.0` para las 6 rarezas en todos los Place salvo `Hud`.
  - `only_the_quiet_places_have_no_animation`: `air == Air::None` exactamente en Inventory, Map, Reader, Records, Hud.
  - `only_three_screens_show_the_world_behind`.
- [ ] **Step 2:** `cargo test -j 2 -q --bin hexagonos_de_babel theme::` → falla (paletas provisorias iguales).
- [ ] **Step 3:** Paletas de las 19 identidades según la tabla del diseño (Forja y Mesa: los valores de `spike-ui/src/ui/theme.rs`).
- [ ] **Step 4:** Mismas pruebas → pasan.

### Task 2: Piezas compartidas con estilo
**Files:** Create `src/ui/theme/{frames,backdrop,air,art,cache}.rs`; Modify `src/ui/{mod,state,skills,inventory}.rs`, `src/shell.rs`, y los `ui.title` de cabecera en `src/ui/{craft,menus,book,coop,look}.rs`; Test `src/ui/theme/tests.rs`, `src/shell_tests.rs`

**Interfaces — Consumes:** Task 1.
**Produces:**
- `cache::ArtCache { pub builds: u32 }` con `ArtCache::default()`; vive en `Shell`.
- En `Ui`: `pub theme: Theme` (por defecto `Place::Library.theme()`), `pub art: Option<&'a mut ArtCache>`, `pub see_through: bool`.
- `Ui::dim_background(alpha: u8)`: mismo nombre y llamadas; ahora pinta el fondo del Place (opaco, o con `alpha` si `see_through`) y su animación detrás de los paneles.
- `Ui::heading(&mut self, text: &str, y: i32)`: placa con emblema; reemplaza los `ui.title(..., 2, GOLD)` de cabecera.
- `Ui::section(&mut self, text: &str, x: i32, end: i32, y: i32, color: Rgba)`: título con línea y rombo.
- `Ui::stripe(&mut self, r: Rect, color: Rgba)`; `Ui::medallion(&mut self, cx: i32, cy: i32, size: i32)`; `Ui::with_place<R>(&mut self, place: Place, draw: impl FnOnce(&mut Ui) -> R) -> R`.
- `Ui::tone(&self, color: Rgba) -> Rgba`: `DIM`→`theme.dim`, `FAINT`→`theme.faint`, `SKY`→`theme.heading`; lo aplican `text`, `plain`, `centered`, `right`, `paragraph`.
- `cell`: siempre con borde. `edge None` o RGB igual a `PANEL_EDGE` → `theme.edge` (con su alfa); otro color se conserva.
- `card(r, accent)`: marco del Place; el `accent` se pinta como franja solo si no es `SKY` ni `VIOLET` (`pub const VIOLET: Rgba = rgb(190,150,255)`). `panel` con `r.h < 30 || r.w < 60` usa el marco simple.

- [ ] **Step 1: Pruebas** (primitivas sobre un `Canvas` vacío):
  - `every_option_has_a_border`: `cell` sin foco → píxel del medio del borde superior == `theme.edge`; con foco → `theme.accent`.
  - `borders_that_mean_something_are_kept`: `cell` con `Some(rarity)` y con `Some(GOLD)`, con y sin foco, conserva ese color.
  - `role_colours_follow_the_place`: `tone(DIM) == theme.dim`, `tone(INK) == INK`.
  - `backgrounds_are_drawn_once`: dos `dim_background` + `card` seguidos con el mismo `ArtCache` → `builds` no cambia en la segunda pasada.
  - `each_place_paints_its_own_background`: los 19 fondos (excepto `Hud`) son distintos entre sí.
  - `a_background_hides_the_world_unless_told_otherwise`: alfa de las 4 esquinas 255, y < 255 con `see_through`.
  - `long_titles_fit_on_their_plaque`: `heading` con «Mesa de encuadernador», «Lámparas de retorno» y «El Catálogo de catálogos» no pinta en las columnas 0 y 479.
  - `small_cards_keep_their_inside_clear`: en un `card` de 18 px de alto, las filas 4..14 entre x+4 y x+w-4 quedan de un solo color base (sin esquineros).
- [ ] **Step 2:** Ver fallar.
- [ ] **Step 3:** Implementar. El shell pone `ui.theme = place_for(screen, bench).theme()`, `ui.see_through = sees_through(screen)` y `ui.art`. La animación sale de `air.rs` a partir de `ui.time`, determinista. `drifting_pages` de `menus.rs` pasa a `Air::Pages`. Marcos, fondos y emblemas de Forja y Mesa según la muestra; el resto con marco simple y degradado hasta las Tareas 3–6.
- [ ] **Step 4:** Pruebas nuevas y toda la app en verde (navegación, textos que entran).

### Task 3: Estaciones
**Files:** Modify `src/ui/theme/{frames,backdrop,air,art}.rs`, `src/ui/craft.rs`; Test `src/shell_tests.rs`

- [ ] Arte de Alambique, Scriptorium, Construir, Cofre y Lámparas (marco, fondo, emblema, animación).
- [ ] `craft.rs`: título = nombre de la mesa cuando hay `bench`; franjas por objeto o tipo (colores de la muestra); `section` en listas y explicaciones; `medallion` en la ficha; casillas de materiales con `theme.well`; cada fila de Construir dibujada con `with_place` de su estación y su emblema chico.
- [ ] Prueba `each_station_row_wears_its_own_colour`: en Construir, los píxeles de borde de las 5 filas sin foco son 5 colores distintos.
- [ ] Suite de la app en verde. Capturas de las 8 pantallas de estación revisadas.

### Task 4: Pantallas del personaje
**Files:** Modify arte en `src/ui/theme/`, `src/ui/{inventory,skills,book,state}.rs`

- [ ] Arte de Inventario, Habilidades, Glosario, Mapa y Catálogo.
- [ ] La barra de pestañas usa los colores del Place actual y cada pestaña inactiva un punto con el acento de su destino.
- [ ] `skills.rs`: los hexágonos usan `theme.cell`/`theme.cell_hot` y conservan el color de cada rama.
- [ ] Prueba `the_item_card_leaves_room_inside_its_panel`: la ficha del objeto (`ITEM_CARD`) termina al menos 4 px antes del borde derecho del panel de la mochila (`BAG_PANEL`). Ver fallar; ajustar; ver pasar.
- [ ] Suite en verde. Capturas revisadas.

### Task 5: Menús
**Files:** Modify arte en `src/ui/theme/`, `src/ui/{menus,look,coop,mod}.rs`

- [ ] Arte de Biblioteca, Multijugador y Relojería.
- [ ] Pausa: cada botón con `with_place` de su destino. Título: igual (Nueva partida → Library, Multijugador → Rooms, Opciones → Clockwork, Salir → Ashes). El título grande lleva filigrana.
- [ ] Prueba `pause_buttons_wear_where_they_lead`: los bordes sin foco de los botones de Inventario, Habilidades, Glosario y Mapa son distintos entre sí.
- [ ] Suite en verde. Capturas revisadas.

### Task 6: Lector, caída y final
**Files:** Modify arte en `src/ui/theme/`, `src/ui/{book,menus}.rs`

- [ ] Arte de Lector (tapas y señalador alrededor de la página oscura), Ceniza y Amanecer.
- [ ] En créditos, el botón va sobre una banda lisa para que el texto que pasa no se le cruce.
- [ ] Suite en verde. Capturas revisadas.

### Task 7: Pantalla de juego
**Files:** Modify `src/ui/hud.rs`, `src/ui/coop.rs` (`hud`), arte `Slim`

- [ ] `Ui` del HUD con `Place::Hud`: marcos en retrato, barras, casillas, minimapa, lista fijada, cartel de ayuda y avisos.
- [ ] Prueba `the_hud_takes_no_more_room_than_before`: antes de cambiar, guardar la máscara de píxeles pintados del HUD (partida nueva con 3 recetas fijadas) y su cantidad; después, la cantidad no supera ese número + 3 % y nada se pinta en la zona central `x 140..340, y 60..200`.
- [ ] Suite en verde. Capturas revisadas.

### Task 8: Estadísticas (reglas)
**Files:** Create `core/src/progression/records.rs`, `core/src/game/records_tests.rs`; Modify `core/src/progression/{mod,player,sanitize}.rs`, `core/src/game/{mod,combat,player,actions,travel,sync,profile}.rs`

**Interfaces — Produces:**
- `struct Tally` (`Copy, Default, PartialEq`, `serde(default)`): `play_seconds: u64`, `deepest_floor, hexagons, falls, enemies, elites, guardians: u32`, `damage_dealt, damage_taken: u64`, `books, pages: u32`, `materials: u64`, `broken, crafted, improved: u32`.
- `struct Records { begun: bool, tally: Tally, opened: BTreeMap<u64, u8> }`; `PlayerData::records: Records`.
- `Records::read_book(&mut self, book: u64, pages: u8)`: libro nuevo suma 1 libro y sus páginas; uno conocido, solo las páginas de más.
- `Profile::begin_records(&mut self)`: una sola vez toma `play_seconds`, `deepest_floor`, `visited.len()` y `guardians_defeated.len()` del mundo. Lo llaman la carga de la partida y `Game::new`.
- `State::guest_tally: Tally` viaja siempre; la comparación de `guest` ignora la `Tally`, y `opened` se aparta del anfitrión como el Glosario.

- [ ] **Pruebas** (una por regla): `a_book_counts_once_and_a_better_lens_adds_only_new_pages`; `an_older_save_starts_from_what_its_world_knew` (y la segunda llamada no cambia nada); `breaking_crafting_and_improving_are_counted`; `gathered_materials_are_counted`; `defeats_count_enemies_elites_and_damage`; `falls_and_damage_taken_are_counted`; `new_hexagons_and_depth_are_counted_once`; `time_adds_up_while_the_world_runs`; en cooperativo `enemies_count_for_both_and_crafting_only_for_who_did_it`; en red `a_blow_does_not_resend_the_guests_whole_data` (tras sumar daño, `State::guest` es `None` y `guest_tally` lleva el valor) y `the_guests_records_arrive`; `edited_records_are_clamped` (tope `opened` 20 000).
- [ ] Ver fallar, implementar, ver pasar; incluir las pruebas largas.

### Task 9: Estadísticas (pantalla)
**Files:** Create `src/ui/stats.rs`; Modify `src/ui/{mod,state,menus}.rs`, `src/ui/theme/*`, `src/shell.rs`, `assets/sprites/skill_icons.ron` (ícono `ui_stats`), `src/shell_tests.rs`

**Interfaces — Produces:** `Screen::Stats` → `Place::Records`; `stats::rows(game: &Game) -> [(&'static str, Vec<(&'static str, String)>); 4]`; `stats::play_time_text(seconds: u64) -> String` («45 min», «3 h 05 min»); `stats::grouped(n: u64) -> String` («12.345»).

- [ ] Pruebas: `numbers_are_written_for_people`; `the_four_groups_show_what_was_counted` (los 17 datos del diseño con sus valores); `statistics_open_from_the_pause_menu` (botón, navegable, Esc vuelve a Pausa); la pantalla entra en `every_view`; los rótulos y valores máximos entran en sus tarjetas.
- [ ] Ver fallar, implementar, ver pasar. Captura revisada.

### Task 10: Correcciones, medición y documentos
**Files:** `core/src/progression/upgrades.rs`, `src/shell.rs` (medición solo en pruebas), `src/shell_tests.rs`, `README.md`, `DECISIONS.md`, `PROGRESS.md`

- [ ] Prueba `explanations_have_no_doubled_full_stop`: ningún `explain` de mejora, en ningún estado, contiene `".."`. Ver fallar; corregir; ver pasar.
- [ ] Prueba ignorada `interface_drawing_times`: mediana de 120 cuadros del dibujo de interfaz por pantalla; falla si alguna pasa de 4 ms. Correrla en `--release` antes (sobre el respaldo previo a esta etapa, sin el límite) y después; anotar ambos números.
- [ ] README, DECISIONS y PROGRESS.
- [ ] `cargo fmt --check`, clippy `-D warnings`, suite completa, pruebas largas, búsqueda de datos personales, `.exe` de la raíz sin cambios.
- [ ] Revisión propia del diff contra el respaldo.
