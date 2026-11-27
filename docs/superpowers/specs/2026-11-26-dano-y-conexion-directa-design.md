# Piedras de afilar, enemigos con menos vida y conexión directa

Aprobado por el autor el 26/11/2026 sobre la propuesta en el chat («proseguir con la opción A»).

## Para qué
- Los enemigos aguantan demasiado. Hay que poder subir el daño por golpe a voluntad, sin depender
  de lo que caiga.
- Dos PC en redes distintas tienen que poder jugar con **solo el `.exe`**: sin configurar el
  router, sin instalar nada y sin servidor propio.

## Objetivos
1. Cinco piedras de afilar fabricables, +3 de daño por golpe cada una (máximo +15), permanentes.
2. Arma base 6 en lugar de 4. Enemigos y guardianes con 20 % menos de vida; la vida crece 11 % por
   piso en lugar de 13 %.
3. Conexión directa con código de respuesta cuando la conexión normal falla.
4. Mensajes claros cuando no se puede conectar.

## Fuera de alcance
Servidor intermediario, IPv6, más de dos jugadores, ranuras de equipo nuevas, cambiar el daño de
los enemigos.

## Restricciones
- Un solo `.exe`; solo bibliotecas de Windows. Sin dependencias nuevas.
- Las partidas guardadas viejas cargan igual.
- Todo lo que llega por la red sigue validándose; la seguridad no puede bajar.
- 60 FPS en PC modestas: la red no bloquea el dibujo.

## Enfoques

### Daño
| Enfoque | A favor | En contra |
|---|---|---|
| **Piedras fabricables (elegido)** | Sin suerte; no toca el inventario ni el formato de los objetos | Poca variedad |
| Ranura nueva (anillos o guantes) | Más botín | Depende de la suerte; cambia qué objeto sale de cada palabra |

### Redes distintas
| Enfoque | A favor | En contra |
|---|---|---|
| **A. Perforación UDP con código de respuesta (elegido)** | Sin router ni servidor | Un pegado más; falla si las dos redes cambian de puerto según el destino |
| B. Abrir el puerto a mano | Sin código nuevo | Hay que tocar el router; no sirve con CG-NAT |
| C. Servidor intermediario | Casi siempre funciona | Alguien tiene que mantenerlo |

La prueba del 26/11 desde la red del autor: tres servidores STUN vieron el mismo puerto, igual al
local. De este lado la opción A es viable. No dice nada de la red de la otra persona.

## Cómo funciona la opción A
1. El anfitrión crea la sala. Además del puerto TCP, abre un socket UDP, pregunta por STUN qué
   dirección y puerto ve internet y lo mantiene vivo. El código de sala lleva dirección, puerto
   TCP, puerto UDP y el secreto.
2. El invitado pega el código. El juego intenta TCP. Si entra (misma red, UPnP, puerto abierto a
   mano o red virtual), no cambia nada.
3. Si TCP falla y el código trae puerto UDP: el invitado averigua su propia dirección por STUN
   desde el socket que va a usar, muestra el **código de respuesta** y empieza a mandar paquetes al
   anfitrión.
4. El anfitrión pega la respuesta. Los dos mandan paquetes hasta que ambos reciben del otro.
5. Sobre UDP corre un flujo confiable y ordenado propio. Encima van el mismo saludo y los mismos
   mensajes firmados que sobre TCP.

El código de respuesta lleva la dirección del invitado y una marca calculada con el secreto de la
sala: uno de otra sala se rechaza con un mensaje propio.

## Seguridad
- Cada paquete UDP va firmado con una clave derivada del secreto **con las mismas 200 000 rondas**
  que el saludo, distinta en cada sentido. Lo que no verifica se descarta en silencio.
- Cada intento del invitado lleva un número de sesión al azar, para descartar paquetes de un
  intento anterior.
- Quien tenga la firma correcta puede llegar desde otro puerto (cubre redes que cambian de
  puerto de un solo lado). Después igual tiene que pasar el saludo.
- El anfitrión solo manda paquetes a una dirección que él mismo pegó, por tiempo limitado.
- Búferes y ventana acotados.

## Qué ve el jugador
- **Anfitrión, sala abierta:** su código, y debajo «si no logra entrar, pegá acá su código de
  respuesta», con un botón para pegar.
- **Invitado:** si hace falta, el código de respuesta en grande, un botón para copiarlo y la
  indicación de no cerrar la pantalla.
- **Si no se puede:** que alguna de las dos redes lo bloquea y que pruebe alojar la otra persona.
- `--net-check` dice además si esta red es apta para la conexión directa.

## Queda a criterio de quien lo construye
Formato exacto de paquetes y códigos, tiempos de reintento, ventana y retransmisión, disposición
de las pantallas, costos de las piedras.

## Cómo se comprueba
- Flujo confiable: tests deterministas con pérdida, desorden y duplicados.
- Conexión: tests por UDP en la propia PC con un STUN simulado y el TCP inaccesible.
- Dos ventanas reales del `.exe` conectadas por el camino directo.
- **Queda pendiente y no se puede hacer desde acá:** dos PC reales en redes distintas.
