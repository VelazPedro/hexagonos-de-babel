# Nombre del personaje y mascotas

Diseño corto, aprobado en conversación el 2026-11-26.

## Intención
El jugador le pone nombre a su personaje y lo ve en pantalla con discreción. Desde el comienzo lo
acompaña una mascota propia, personalizable y con nombre, que crece con un árbol de habilidades
aparte pagado con los puntos del personaje. En multijugador cada uno lleva la suya.

## No es objetivo
- Que la mascota sea necesaria para avanzar. Es compañía; atacar y recolectar son extras.
- Que reciba daño, muera o haya que cuidarla.
- Varias mascotas a la vez o cambiar de especie.

## Decisiones
- **Aspecto inicial:** perro de pelaje canela corto, orejas grandes y paradas con interior
  rosado grisáceo, ojos oscuros y brillantes, hocico largo que oscurece hacia la trufa negra,
  cuerpo fino y cola levantada con la punta oscura. Es un dibujo propio en píxeles; ninguna
  foto entra en el repositorio.
- **Personalización:** nombre, color de pelaje, orejas, punta de la cola, ojos y collar.
- **Árbol:** 10 habilidades en tres ramas. *Colmillo* enseña a atacar y lo mejora. *Olfato*
  junta objetos y ayuda a recolectar. *Vínculo* da mejoras chicas de compañía. Cada una cuesta
  1 punto de los del personaje.
- **Al principio** solo sigue.
- **Nombres en pantalla:** chicos y semitransparentes sobre cada cabeza, más tenues en combate,
  con opción para ocultarlos. El del personaje también va junto al nivel. Admiten mayúsculas y
  tildes.
- **Multijugador:** cada jugador guarda su mascota y sus habilidades; ambos ven las dos; la
  simula el anfitrión.
- **Partidas guardadas:** cargan igual y reciben la mascota inicial.

## Enfoque
La mascota vive en las reglas (`babel_core`), porque ataca, recolecta y viaja por la red. Un
dibujo puesto solo en el cliente no podría hacer nada de eso. Se descartó tratarla como un
enemigo aliado: arrastraría vida, colisiones, IA hostil y botín que no le corresponden.

## Restricciones
- Todo lo que llega de la otra PC se valida con límites, como el resto del multijugador.
- Sin crates nuevos. Sin `unwrap` en el juego. Sin romper guardados.
- El rendimiento de un jugador no debe cambiar de forma medible.

## A criterio de quien implemente
Valores de daño y alcance, nombres e íconos de las habilidades, animaciones y distribución de
las pantallas.
