# Multijugador cooperativo

Estado: **implementado** (2 jugadores). Probado en una sola PC (dos juegos reales conectados por
la vía normal y por la directa), contra un router y un servidor STUN simulados y con redes con
pérdida simuladas. **No se probó todavía entre dos PC de redes distintas.**

## Pedido del autor
- Salas, y cada persona guarda su propio progreso en su PC.
- Pausa individual: el juego solo se detiene si los dos pausan.
- Un solo hexágono a la vez; como en *The Binding of Isaac*, se pasa al siguiente cuando pasan
  los dos.
- Con seguridad.
- Jugar desde dos PC en redes distintas, sin VPN: uno aloja la sala y el otro entra.

## Cómo se juega
Cada PC necesita **solo el `.exe`**, y las dos la misma versión.

1. El anfitrión elige **Multijugador → Crear una sala**. Aparece un código de 24 caracteres.
2. Se lo pasa a su compañero (por chat). El compañero elige **Multijugador**, pega el código y
   pulsa **Unirme**. Entra con su propio personaje.
3. Si no puede entrar por la vía normal, le aparece un **código de respuesta** de 15 caracteres.
   Se lo manda al anfitrión, que lo pega en la pantalla de la sala. Se conectan solos.
4. Cada uno guarda su progreso en su PC. El mundo es el del anfitrión.

El anfitrión puede seguir jugando mientras espera: para pegar la respuesta vuelve con
**Pausa → De a dos**.

## Reglas de juego
| Tema | Regla |
|---|---|
| Puertas | Se cruza cuando los dos están en la misma puerta. |
| Escaleras, portal, lámpara de retorno | Cada uno pulsa interactuar ahí; al confirmar los dos, viajan. |
| Viajes desde un menú (lámparas, Catálogo) | El compañero tiene que estar al lado. |
| Pisos 4, 8, 13 y 18 | Para bajar alcanza con que **uno** tenga la Cuchilla y el Martillo que se piden. |
| Estadísticas | Cada uno tiene las suyas y las guarda en su PC. Enemigos, hexágonos nuevos y materiales cuentan para los dos. |
| Mejoras de las mesas | Cada uno tiene las suyas y las paga con sus materiales. Las de antorchas sirven a los dos. |
| Pausa | Pausar no detiene el mundo; se detiene cuando los dos están en «Pausa». |
| Otros menús | No detienen el mundo. Quien tiene un menú abierto queda quieto y vulnerable. |
| Botín y experiencia | Cada objeto, material y punto de experiencia lo reciben los dos. |
| Caída | El caído se levanta si el compañero se queda a su lado. Si caen los dos, reaparecen en la lámpara. |
| Enemigos | Persiguen al jugador en pie más cercano. Los guardianes tienen más vida de a dos. |
| Mascotas | Cada uno lleva la suya, con su aspecto, nombre y habilidades. Se ven las dos; las mueve el anfitrión. Esperan a lados opuestos. |

## Quién guarda qué
- **Anfitrión:** su mundo y su personaje, como siempre.
- **Invitado:** su personaje (nivel, habilidades, inventario, materiales, reliquias, glosario,
  aspecto, nombre) y su mascota (nombre, aspecto y habilidades). Su propio mundo no se toca: durante la visita el juego no puede escribir el mundo del
  anfitrión en el disco del invitado.

## Arquitectura
- El anfitrión simula todo. El invitado envía sus controles y sus acciones de menú, y recibe el
  estado del hexágono para dibujarlo.
- La lógica del jugador actual (`body`, `profile.player`, `derived`) se mantiene; el compañero se
  guarda aparte y se intercambia mientras actúa. Con un solo jugador no cambia nada.
- Transporte: TCP con mensajes con longitud, en hilos aparte (`src/net/`). Formato binario con
  `postcard`.

## Conexión entre redes distintas, sin VPN
No hace falta servidor, cuenta, instalar nada ni configurar el router.

Al crear la sala, en segundo plano (`src/net/reach.rs`):
1. **UPnP** (`upnp.rs`): busca el router y le pide «el puerto TCP N va a esta PC». El puerto se
   cierra cuando se cierra la sala, o cuando termina la conexión si entró alguien. Si el juego se
   cierra de golpe, el router lo olvida solo en 2 h.
2. **STUN** (`stun.rs`): desde el socket UDP de la sala pregunta a un servidor público (Google,
   Cloudflare, Mozilla) «¿qué dirección y puerto ve internet?». Solo viaja esa pregunta.

El código lleva la dirección, el puerto TCP, el puerto UDP y el secreto.

**Vía normal (TCP).** El invitado la prueba primero. Funciona en la misma red, si el router abrió
el puerto, si se abrió a mano (TCP 47612) o por una red virtual (`CÓDIGO@dirección`).

**Vía directa (UDP), si la normal falla.** Un router deja entrar paquetes desde un lugar solo
después de que algo salió hacia ese lugar. Por eso las dos PC se envían paquetes a la vez
(`direct.rs`). El anfitrión necesita saber dónde está el invitado: eso es el código de respuesta.
Cuando las dos se oyen, encima corre un canal confiable y ordenado propio (`stream.rs`,
`pipe.rs`) con el mismo saludo y los mismos mensajes firmados.

| Estado de la sala | Qué pasó | Qué ve el anfitrión |
|---|---|---|
| Abierta | El router abrió el puerto y su dirección es pública | Código para internet; lo normal es que entre directo |
| Directa | El router no lo abrió, o hay otro equipo en el medio | Código para internet y el campo para la respuesta |
| Sin internet | Nada respondió | Solo sirve la misma red |

El código de la misma red se muestra aparte («Desde tu misma red»), porque muchos routers no dejan
volver a entrar por su propia dirección pública. **Dos PC de la misma casa tienen que usar ese.**

`hexagonos_de_babel.exe --net-check` hace la prueba de verdad desde esa PC y lo cuenta, incluido
si la red es apta para la vía directa.

### Límites que conviene saber
- La vía directa **falla si las redes cambian de puerto según el destino** (común en datos
  móviles y en algunos proveedores). Conviene probar que aloje la otra persona; si tampoco, queda
  una red virtual.
- El invitado espera la respuesta hasta 5 minutos. Una vez pegada, el anfitrión intenta 45 s.
- **Windows:** si alguna vez se canceló el permiso de red, queda una regla que bloquea el juego,
  incluida la vía directa. Se habilita en **Seguridad de Windows → Firewall → Permitir una
  aplicación**.
- Solo IPv4.
- **No implementado:** un servidor de retransmisión resolvería también las redes que bloquean la
  vía directa, pero necesita alguien que lo aloje todo el tiempo.

## Seguridad
- Código de sala con dirección, puerto y un secreto aleatorio del sistema operativo (48 bits).
  Cambia en cada sala.
- Autenticación mutua por desafío: el secreto nunca viaja. Cada intento de adivinarlo le cuesta
  200 000 rondas de BLAKE3 al anfitrión y se responde 0,7 s tarde, así que adivinarlo es
  inviable.
- En la vía directa, cada paquete UDP va firmado con una clave derivada del secreto con las
  mismas 200 000 rondas, distinta en cada sentido; lo que no verifica se descarta sin responder.
  El anfitrión solo envía paquetes a la dirección de una respuesta que él mismo pegó (o al origen
  de un paquete firmado que llegó mientras tanto), durante 45 s. La respuesta lleva una marca que
  solo puede calcular quien conoce el secreto de esa sala.
- Cada mensaje lleva una firma (BLAKE3 con clave de sesión por dirección) y un número de orden. No
  hay cifrado: quien escuche la red puede ver la partida, pero no modificarla, repetirla ni
  hacerse pasar por nadie.
- Con el puerto expuesto a internet:
  - cada dirección tiene un saludo a la vez y, tras 5 fallos, se la ignora;
  - hay a lo sumo 4 saludos simultáneos y la sala se cierra tras 200 fallos en total;
  - los saludos corren en hilos aparte con plazo de 10 s, así que quien se quede callado no bloquea
    a un amigo;
  - el puerto solo se abre mientras hay una sala; entra un único invitado.
- Límite de tamaño por mensaje (256 B en el saludo, 1 MiB hacia el anfitrión, 4 MiB hacia el
  invitado), plazos de espera y cola de entrada acotada.
- Todo lo recibido se valida antes de usarlo (`State::check`, `PlayerData::sanitized`,
  `tidy_input`, `tidy_events`): tamaños, finitud, límites de coordenadas, radios, velocidades y
  largos de listas. Nada que llegue de la red puede colgar o cerrar el juego.
- El router solo se obedece a sí mismo: de lo que contesta se siguen únicamente direcciones del
  propio router y lo que se copia a un pedido se filtra antes.
- Las dos PC deben tener la misma versión del juego (se comprueba en el saludo).
- Trampas: cada uno guarda su progreso, así que puede editarlo. Está pensado para jugar entre
  personas conocidas.
- Las direcciones públicas van dentro de los códigos (el de sala y el de respuesta):
  compartirlos solo con quien se invita.
