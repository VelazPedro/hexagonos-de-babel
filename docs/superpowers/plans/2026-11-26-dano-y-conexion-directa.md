# Piedras de afilar y conexión directa — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Subir el daño con piedras fabricables, acortar las peleas y permitir jugar entre dos redes distintas con solo el `.exe`.

**Architecture:** El daño es dato (`recipes.ron`, `items.ron`, `enemies.ron`) más un contador en `PlayerData`. La conexión directa es UDP perforado más un flujo confiable propio (`net/stream.rs`, puro; `net/pipe.rs`, socket e hilos) que se ve como `Read + Write`, así el saludo y los mensajes firmados de hoy no cambian.

**Tech Stack:** Rust 2024, Bevy =0.20.1, blake3, postcard. Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-11-26-dano-y-conexion-directa-design.md`

## Global Constraints
- Un solo `.exe`, solo bibliotecas de Windows, sin dependencias nuevas.
- Sin `unwrap`/`expect` fuera de tests; clippy con `-D warnings`.
- Las partidas viejas cargan igual (`serde(default)`).
- Todo lo recibido por red se valida; búferes acotados; nada bloquea el hilo del juego.
- +3 de daño por piedra, 5 piedras; arma base 6; vida de enemigos y guardianes ×0,8; `hp_per_floor` 0.11.
- Textos en español rioplatense.
- Sin commits salvo pedido del autor.

## Review Focus
1. En Windows, un «puerto inalcanzable» (ICMP) hace fallar `recv_from` con `ConnectionReset`: la conexión tiene que seguir (Tarea 4).
2. El invitado cancela y reintenta en la misma sala: el anfitrión acepta el intento nuevo (Tarea 4).
3. Un mensaje grande (1 MiB) con pérdida llega entero y en orden (Tarea 2).
4. El aviso de despedida enviado justo antes de cerrar llega (Tareas 2 y 4).
5. Pegar en el campo de respuesta el código de sala, texto cualquiera o la respuesta de otra sala da un mensaje claro y la sala sigue abierta (Tarea 5).

---

### Task 1: Piedras de afilar y balance

**Files:** `core/src/data/types.rs`, `assets/data/{recipes,items,enemies}.ron`, `assets/sprites/skill_icons.ron`, `core/src/progression/{player,stats,crafting,sanitize,tests}.rs`, `src/ui/craft.rs`

**Interfaces — Produces:** `RecipeOutput::Whetstone(u8)` (nivel 1..=5); `PlayerData::whetstones: u8`; `ItemsFile::whetstone_damage: f32`; recetas `whet_1`..`whet_5` en la mesa de encuadernador; ícono `ui_whetstone`.

Costos: 1 `[(Stone,8),(Iron,3)]` xp 20 · 2 `[(Stone,10),(Brass,4)]` 40 · 3 `[(Stone,12),(Silver,4),(Ash,4)]` 70 · 4 `[(Stone,12),(Crystal,4),(Salt,6)]` 110 · 5 `[(Vitral,5),(Crystal,4),(Bone,8)]` 170.

- [ ] **Step 1: Tests que fallan** en `core/src/progression/tests.rs`:
  - `whetstones_add_three_damage_each_and_go_in_order`: con materiales de sobra y la mesa construida, `whet_2` da `Err(NeedPreviousTier)`; `whet_1` suma exactamente 3.0 a `derive(..).damage`; repetirla da `Err(AlreadyOwned)`; las cinco suman 15.0.
  - `whetstones_need_nothing_their_level_cannot_reach`: cada material de `whet_L` aparece en alguna receta de herramienta de tier ≤ L.
  - `old_saves_have_no_whetstones_and_edited_ones_are_clamped`: `(level: 7)` → 0; 200 → 5 tras `sanitized`.
  - `fights_are_shorter`: `hp_per_floor == 0.11`; arma base `(Damage, 6.0)`; guardianes `[336,720,1200,1840,2880,4480]`; el libro de piso 1 (hp 19.2) cae con los dos primeros golpes de un combo con daño 10.
- [ ] **Step 2:** `cargo test -p babel_core whetstone fights_are` → fallan.
- [ ] **Step 3:** Implementar. La orden y el «ya la tenés» siguen la regla de las herramientas. En Fabricación las piedras van en «Herramientas», solo la siguiente; el texto de lista vacía pasa a «Ya fabricaste todo lo de esta pestaña.».
- [ ] **Step 4:** `cargo test --workspace` → verde; ajustar los tests que dependan de la vida anterior solo si el cambio de vida es la causa.

### Task 2: Flujo confiable (puro)

**Files:** Create `src/net/stream.rs`; Modify `src/net/mod.rs` (`mod stream;`).

**Interfaces — Produces:**
```rust
pub const SEGMENT: usize = 1200;
#[derive(Clone, Serialize, Deserialize)]
pub enum Frame { Punch { seen: bool }, Data { seq: u32, bytes: Vec<u8> }, Ack { next: u32, ahead: u64 }, Ping, Close }
pub struct Stream;
impl Stream {
    pub fn new(now: Instant) -> Stream;
    pub fn write(&mut self, bytes: &[u8]) -> usize;      // 0 = búfer lleno
    pub fn read(&mut self, out: &mut [u8]) -> usize;      // 0 = nada todavía
    pub fn accept(&mut self, frame: Frame, now: Instant);
    pub fn transmit(&mut self, now: Instant) -> Option<Frame>; // llamar hasta None
    pub fn delivered(&self) -> bool;                      // todo lo escrito fue confirmado
    pub fn peer_closed(&self) -> bool;                    // llegó Close
    pub fn silent_for(&self, now: Instant) -> Duration;
}
```
Búfer sin enviar 32 KiB; ventana hasta 256 segmentos con arranque en 16, mitad ante pérdida, mínimo 4; RTO suavizado entre 100 ms y 2 s; retransmisión rápida con 3 posteriores confirmados; `Ping` cada 1 s sin nada que mandar; el receptor guarda hasta 256 segmentos adelantados y 512 KiB sin leer, y lo que excede no se confirma. `Punch { seen: false }` se contesta con `Punch { seen: true }`.

- [ ] **Step 1: Tests que fallan** con un canal simulado determinista (`Dice`): retardo, pérdida, desorden y duplicados, con reloj ficticio.
  - `bytes_arrive_whole_and_in_order_on_a_perfect_link`
  - `a_megabyte_survives_loss_reordering_and_duplicates` (20 % de pérdida, 10 % duplicados, retardo 20–120 ms, en ambos sentidos a la vez; menos de 120 s simulados)
  - `a_full_buffer_refuses_more_until_it_drains` (sin respuesta del otro lado, `write` termina devolviendo 0 y lo pendiente queda acotado)
  - `a_dead_link_is_retried_calmly` (sin respuestas durante 10 s simulados se transmiten menos de 200 tramas)
  - `an_idle_link_keeps_talking` (`Ping` cada ~1 s)
  - `nonsense_acks_and_far_sequences_are_ignored`
  - `delivered_turns_true_only_when_everything_was_confirmed`
- [ ] **Step 2:** `cargo test -p hexagonos_de_babel net::stream` → fallan.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** mismo comando → verde.

### Task 3: Códigos, claves y STUN con puerto

**Files:** `src/net/{code,secure,stun,reach}.rs`

**Interfaces — Produces:**
```rust
pub struct RoomCode { pub address: SocketAddrV4 /* TCP */, pub udp: u16 /* 0 = sin vía directa */, pub secret: [u8; 6] }
pub struct ReplyCode; // new(address, &PunchKeys), parse(&str) -> Option<Self>, text(), address(), belongs_to(&PunchKeys) -> bool
pub struct PunchKeys { pub from_host: [u8; 32], pub from_guest: [u8; 32], mark: [u8; 32] } // derive(&secret), con STRETCH rondas
pub fn seal_datagram(key: &[u8;32], session: u64, frame: &Frame) -> Vec<u8>;   // sesión + postcard + firma de 16
pub fn open_datagram(key: &[u8;32], bytes: &[u8]) -> Option<(u64, Frame)>;
pub fn stun::mapped_address(socket: &UdpSocket, servers: &[&str]) -> Option<SocketAddrV4>;
pub enum Reach { Searching, Open(RoomCode), Direct(RoomCode), Offline }
pub fn reach::find(probe: &Probe, room: &RoomCode, socket: &UdpSocket) -> (Reach, Option<Mapping>);
// Probe gana `loopback: bool`: sin red, la dirección «pública» es 127.0.0.1 y el puerto local (tests y modo de desarrollo).
```
Código de sala: 14 bytes + 8 bits de control = 24 caracteres en grupos de 6. Respuesta: 6 bytes + 2 de marca + 11 bits de control = 15 caracteres en grupos de 5. `Open` si el router abrió el puerto y su dirección es pública y coincide; si no, y STUN contestó, `Direct`; si nada contestó, `Offline`. El código lleva en `udp` el puerto que vio STUN, o 0.

- [ ] **Step 1: Tests que fallan:** `codes_of_both_kinds_read_back_however_they_are_typed`, `a_room_code_is_not_a_reply_and_a_reply_is_not_a_room`, `a_reply_from_another_room_is_told_apart`, `datagrams_with_another_key_or_altered_are_refused` (cada byte alterado), `punch_keys_differ_by_direction_and_secret`, `stun_reports_the_port_too` (XOR y plano), `the_answer_is_read_on_the_socket_that_asked`, y los de `reach` adaptados: sin UPnP → `Direct` con el puerto UDP visto; CG-NAT o dos routers → `Direct`; STUN mudo → `Offline`.
- [ ] **Step 2–4:** fallar, implementar, `cargo test -p hexagonos_de_babel net::` verde (junto con la Tarea 4 si el cambio de `RoomCode` lo exige).

### Task 4: Perforación, tubería y sala

**Files:** Create `src/net/pipe.rs`, `src/net/direct.rs`; Modify `src/net/mod.rs`, `src/main.rs`.

**Interfaces — Consumes:** Tareas 2 y 3. **Produces:**
```rust
pub(crate) trait Wire: Read + Write + Send + 'static {
    fn another(&self) -> io::Result<Self> where Self: Sized;
    fn limits(&self, read: Duration, write: Duration) -> io::Result<()>;
    fn closer(&self) -> io::Result<Box<dyn Fn() + Send + Sync>>;
}                                   // para TcpStream y Pipe; welcome, knock y Link::start pasan a ser genéricos
pub struct Pipe;                    // Pipe::start(socket, peer, session, send_key, receive_key) -> io::Result<Pipe>
pub struct Punched { pub peer: SocketAddr, pub session: u64 }
pub enum Punching { Idle, Trying, Blocked }
impl Room    { pub fn answer(&self, reply: ReplyCode) -> Result<(), AnswerError /* OtherRoom, NotReady */>; pub fn punching(&self) -> Punching; }
impl Joining { pub fn start(room: RoomCode, probe: Probe) -> Joining; pub fn reply(&self) -> Option<ReplyCode>; } // soltarlo cancela
pub const PROTOCOL: u16 = 3;
```
Perforación: el invitado manda `Punch` cada 300 ms hasta 5 min; el anfitrión, tras `answer`, cada 200 ms hasta 45 s y después `Blocked`; una respuesta nueva reinicia. Una dirección queda establecida al recibir `Punch { seen: true }` o cualquier otra trama de la sesión. Se acepta el origen del paquete firmado aunque no coincida con la dirección anunciada; una vez establecida la conexión, el par ya no cambia. En espera, el anfitrión repite la consulta STUN cada 20 s. `Pipe`: al cerrar, entrega lo pendiente (hasta 3 s) y manda `Close` tres veces; 20 s de silencio lo dan por muerto; `ConnectionReset`, `WouldBlock` y `TimedOut` del socket se ignoran. Si el invitado no consigue su dirección por STUN: `Failure::Unreachable`; si vence el plazo: `Failure::DirectBlocked` («Alguna de las dos redes bloquea la conexión directa. Prueben que aloje la otra persona.»). `--net-check` consulta dos servidores desde el mismo socket y dice si la red conserva el puerto.

- [ ] **Step 1: Tests que fallan** en `net/mod.rs`, con `Probe { loopback: true }` y el puerto TCP del código cambiado por uno cerrado:
  - `a_closed_front_door_is_passed_by_with_a_reply_code` (se conectan y viajan mensajes en ambos sentidos; uno de 600 KiB llega entero)
  - `a_farewell_sent_just_before_leaving_still_arrives`
  - `a_reply_from_another_room_is_refused_and_the_room_stays`
  - `a_guest_who_gives_up_and_tries_again_gets_in`
  - `strangers_datagrams_are_ignored` (basura y firmas falsas al puerto UDP no impiden entrar)
  - `the_link_survives_a_port_unreachable` (mandar a un puerto UDP cerrado desde el socket de la tubería y seguir hablando)
  - `closing_the_room_frees_both_ports`
  - `a_code_without_a_direct_way_fails_as_before` (`udp: 0` → `Unreachable`)
- [ ] **Step 2–4:** fallar, implementar, `cargo test -p hexagonos_de_babel net::` verde.

### Task 5: Pantallas, juego y modo de prueba

**Files:** `src/ui/{coop,mod,state}.rs`, `src/shell.rs`, `src/shell_coop.rs`, `src/main.rs`, `src/shell_tests.rs`

**Interfaces — Consumes:** Tarea 4. **Produces:** `CoopView::Waiting { code, lan, internet, punching }`; `CoopView::Connecting { reply: Option<&str> }`; `UiData::reply: String`; `Cmd::{PasteReply, SendReply, CopyReply}`; `Shell::{open_direct_room() -> Option<String>, join_direct(&str), reply_code() -> Option<String>, answer_with(&str)}`; bandera `--coop-direct <archivo>` junto a `--coop-host`/`--coop-join` (sin red: todo por 127.0.0.1; la respuesta viaja por `<archivo>.reply`).

Textos: anfitrión «¿No logra entrar? Pegá acá su código de respuesta:», botones «Pegar» y «Conectar», estados «Conectando directo…» y el de `DirectBlocked`; invitado «Mandale este código de respuesta a quien creó la sala:», «Copiar», «No cierres esta pantalla.». Errores: «Ese no es un código de respuesta.», «Ese es un código de sala: acá va el de respuesta.», «Esa respuesta es de otra sala.».

- [ ] **Step 1: Tests que fallan** en `src/shell_tests.rs` con dos `Rig`:
  - `two_homes_play_through_the_reply_code`: sala directa, el invitado muestra su respuesta en pantalla, el anfitrión la carga por los botones, ambos terminan jugando y el invitado ve moverse al anfitrión.
  - `wrong_things_in_the_reply_field_are_explained`: los tres errores de arriba, y la sala sigue esperando.
  - `leaving_while_waiting_for_the_reply_cancels_cleanly`.
- [ ] **Step 2–4:** fallar, implementar, `cargo test --workspace` verde. Revisar las dos pantallas en las imágenes de `HEXBABEL_PICTURES`: nada se superpone ni se sale de 480×270.

### Task 6: Documentos, revisión y `.exe`

**Files:** `README.md`, `MULTIPLAYER.md`, `DECISIONS.md`, `PROGRESS.md`, `hexagonos_de_babel.exe`

- [ ] **Step 1:** Documentar piedras, balance, conexión directa, límites y que ambas PC necesitan esta versión.
- [ ] **Step 2:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo test -p babel_core --release -- --ignored`.
- [ ] **Step 3:** Revisar el diff propio buscando errores en la red (bloqueos, límites, datos sin validar).
- [ ] **Step 4:** `scripts\build-release.ps1`; buscar datos personales en el `.exe` y en los archivos del repo; comprobar que solo importa DLL de Windows.
- [ ] **Step 5:** Dos ventanas del `.exe` conectadas por la vía directa (`--coop-direct`) y por la normal; `--net-check` real; `--bench 8`.
