# Nombre y mascotas: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox
> (`- [ ]`) syntax for tracking.

**Goal:** nombre del personaje visible con discreción y una mascota propia por jugador, con
personalización, nombre y árbol de habilidades.

**Architecture:** los datos persistentes van en `PlayerData` (`name`, `pet`). El estado en vivo es
`Body::pet`, así que el intercambio de jugadores del cooperativo y la sincronización lo llevan
solos. La conducta vive en `core/src/game/pet.rs`; el dibujo, en `src/render/pet.rs`.

**Tech Stack:** Rust, Bevy =0.20.1, RON.

**Spec:** `docs/superpowers/specs/2026-11-26-mascotas-y-nombre-design.md`

## Global Constraints
- Sin crates nuevos; sin `unwrap` fuera de tests; los guardados viejos cargan (`serde(default)`).
- 10 habilidades de mascota, 3 ramas, 1 punto cada una, de `skill_points`.
- La mascota no recibe daño, y sin habilidades no ataca ni junta nada.
- Todo lo recibido de la otra PC se limpia o se rechaza.
- La foto de referencia no entra en el repositorio.
- **Sin commits ni push** salvo pedido del autor.

## Review Focus
1. Guardado anterior: carga con la mascota inicial y sin etiqueta de nombre.
2. Nombre vacío, de solo espacios, larguísimo, con emojis o controles: queda limpio; vacío no
   dibuja etiqueta.
3. Mascota trabada contra paredes o pozos, o tras cambiar de sitio: nunca queda perdida.
4. Datos falsificados del invitado (nombre enorme, habilidades inexistentes, posición NaN).
5. La mascota da el golpe final (también a un guardián): botín y experiencia como siempre.

---

### Task 1: Datos y progresión

**Files:** crear `core/src/progression/pet.rs`, `assets/data/pet_skills.ron`; modificar
`progression/{mod,player,sanitize,stats}.rs`, `data/{types,mod,embedded,tests}.rs`.

**Produces:**
- `NAME_LIMIT: usize = 12`; `name_char(c: char) -> bool` (letras con tilde y ñ, dígitos, espacio,
  `-`, `'`); `clean_name(&str) -> String` (filtra, junta espacios, recorta, corta en 12).
- `PetLook { fur, ears, tail, eyes, collar: u8 }`; `FURS=7, EARS=3, TAILS=3, PET_EYES=5,
  COLLARS=6`; el `Default` (todo 0) es la mascota de referencia; `sanitized()`, `random(&mut Dice)`.
- `PetData { name: String, look: PetLook, skills: BTreeSet<String> }`.
- `PlayerData { name: String, pet: PetData }`; `can_learn_pet(&GameData,&str)`, `learn_pet(..)`.
- `PetBranch { Fang, Nose, Bond }`; `PetPerk { Bite, BiteHaste, Pounce, Fetch, Dig }`;
  `PetSkillDef { id, name, description, branch, row: u8, requires, stats, perks: Vec<(PetPerk,f32)> }`;
  `GameData::pet_skills`, `pet_skill(id)`.
- `Derived::pet: PetPowers { bite, haste, pounce, fetch, dig: f32 }` (suma de `perks`).

Habilidades: Colmillo `m_mordisco` (Bite .22) → `m_agil` (BiteHaste .30) → `m_fuerte` (Bite .20) →
`m_salto` (Pounce 1). Olfato `m_buscar` (Fetch 96) → `m_escarbar` (Dig .35) → `m_sabueso`
(Yield .10). Vínculo `m_trote` (MoveSpeed .05) → `m_abrigo` (HpRegen .4) → `m_lazo`
(XpBonus .08, LightRadius .08).

- [ ] Tests (fallan primero): `old_saves_get_the_default_pet_and_no_name`
  (`ron::from_str("(level: 7, xp: 3)")` → `name == ""`, `pet == PetData::default()`);
  `names_are_cleaned` (`"  José   Ñandú  "` → `"José Ñandú"`; `"a\u{7}b🐶"` → `"ab"`; `"   "` →
  `""`; 40 `x` → 12); `pet_skills_cost_the_heros_points_and_need_the_one_before` (con 1 punto:
  `m_agil` falla, `m_mordisco` entra, quedan 0, y `learn("c_pulso")` falla);
  `pet_tree_is_complete` (10, ids únicos, requisitos de la misma rama, 4/3/3);
  `sanitized_cleans_names_and_forgets_unknown_pet_skills`.
- [ ] Implementar y correr `cargo test -p babel_core` → todo pasa.

### Task 2: Conducta

**Files:** crear `core/src/game/pet.rs`, `core/src/game/pet_tests.rs`; modificar
`game/{mod,player,travel,coop}.rs`.

**Consumes:** `Derived::pet`. **Produces:** `PetBody { pos, facing: V2, moving: bool, anim, still,
cooldown, bite, stuck: f32, digging: Option<(u32,f32)> }`; `PetBody::beside(pos)`; `Body::pet`;
`Game::update_pet(&mut self, dt)` (jugador que actúa), llamado en `step` para cada jugador después
de `update_enemies`.

Reglas: prioridad atacar > buscar > escarbar > seguir. Ataca enemigos `lit`, no ocultos, a ≤ 80 px
del dueño; daño `damage × bite` (×0,65 si blindado), cada `1.2 / (1 + haste)` s; no rompe escudos
ni ilusiones y no activa efectos del dueño; `pounce` suma alcance y empujón. Busca objetos con
`age > 0.35` a ≤ `fetch` px del dueño y los entrega con `pick_up`. Escarba el recurso que el dueño
golpeó en los últimos 2,5 s, cada 0,9 s, con potencia `dig`, sin pausa de impacto ni sacudida. Con
el dueño caído, se acerca y se sienta. A más de 150 px, o trabada 1 s, reaparece junto al dueño.

- [ ] Tests: `without_skills_it_only_follows` (enemigo y objeto al lado: vida y objeto intactos;
  tras alejar al dueño 60 px, la distancia termina < 30); `fangs_bite_only_what_is_lit`;
  `a_bite_can_finish_an_enemy_and_pays_as_usual`; `the_nose_brings_back_a_far_drop`;
  `digging_helps_with_what_the_owner_is_harvesting`; `it_never_stays_lost` (a 400 px → vuelve);
  `each_player_has_their_own_pet` (cooperativo, habilidades distintas).
- [ ] Implementar → `cargo test -p babel_core` pasa.

### Task 3: Red

**Files:** `core/src/game/{sync,sync_tests}.rs`, `src/net/mod.rs` (subir `PROTOCOL`).

**Produces:** `Action::{LearnPet(String), SetName(String), SetPet{name: String, look: PetLook}}`;
`Game::{learn_pet_skill(&str) -> bool, set_name(&str), set_pet(&str, PetLook)}` (guardan la acción
en la copia del invitado).

- [ ] Tests: `the_mirror_shows_both_pets`; `a_guests_pet_actions_change_only_the_guest`;
  `a_state_with_a_lost_pet_is_rejected` (NaN en `host_body.pet.pos` → `Err`).
- [ ] Implementar → pasan.

### Task 4: Dibujo

**Files:** crear `assets/sprites/pet.ron`, `src/render/pet.rs`; modificar `palette.ron`,
`atlas.rs`, `scene.rs`, `skill_icons.ron`.

**Produces:** `pet::build(source: &Atlas, look: PetLook) -> Atlas` con
`pet_{idle,walk,sit}_{down,up,side}`; nombres `FUR_NAMES`, `EAR_NAMES`, `TAIL_NAMES`,
`PET_EYE_NAMES`, `COLLAR_NAMES`; `pet::PetArt { look, atlas }`; 10 íconos `skill_m_*`.

- [ ] Tests: `every_pet_option_changes_what_is_seen`, `no_placeholder_is_left`,
  `every_pet_skill_has_an_icon`.
- [ ] Implementar, ordenar por `y` junto con lo demás.
- [ ] Volcar una hoja con todas las poses ampliadas, mirarla contra la referencia y corregir.

### Task 5: Pantallas

**Files:** `src/{shell,shell_coop,shell_tests,input,settings}.rs`,
`src/ui/{mod,state,look,skills,hud,menus}.rs`.

- «Tu personaje»: pestañas Personaje / Mascota, fila de nombre en cada una (teclado físico con
  tildes y mayúsculas, y teclado en pantalla para mando), vista previa de ambos.
- Habilidades: pestañas del personaje y de la mascota, puntos compartidos a la vista.
- Etiquetas sobre cada cabeza (alfa ≈ 150, ≈ 70 en combate), nombre junto al nivel, nombre del
  compañero en lugar de «Compañero». `Settings::show_names` (por defecto sí) en Opciones.
- Partida nueva y unirse sin partida usan el nombre y la mascota elegidos.

- [ ] Tests: `typing_a_name_on_the_creator_reaches_the_new_game`,
  `the_pet_tab_learns_a_pet_skill`, `names_can_be_hidden`.
- [ ] Implementar → `cargo test --workspace` pasa; capturas de cada pantalla revisadas.

### Task 6: Cierre

- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, tests, los dos
  largos en release.
- [ ] README, MULTIPLAYER, DECISIONS, PROGRESS.
- [ ] `scripts/build-release.ps1`, escaneo del `.exe` y del código por datos personales, dos
  ventanas reales conectadas, `--bench`.
- [ ] Revisión del diff.
