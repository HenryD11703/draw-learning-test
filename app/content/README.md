# Contenido de Trazo Academia

Cada archivo `NN_nombre.json` es un módulo del mapa. El orden del mapa está en la lista `FILES` de
`src/content.rs`: para añadir un módulo, crea el archivo y añádelo ahí. Los archivos van dentro del
ejecutable, así que después de editarlos hay que volver a compilar (`cargo run`).

`cargo test` comprueba que todos los archivos se leen bien, que no hay identificadores repetidos, que
los pinceles existen y que cada ejercicio, con varias semillas, cabe en el lienzo (960 × 640).

## Módulo

```json
{
  "id": "perspectiva",
  "title": "Perspectiva",
  "subtitle": "Una línea",
  "lesson": "Texto de la lección. \n\n separa párrafos.",
  "accent": [150, 170, 255],
  "exercises": [ ... ]
}
```

El número del módulo ("5.") lo pone la app según el orden.

## Ejercicio

```json
{
  "id": "persp01",
  "title": "Rayos hacia el punto de fuga",
  "goal": "Qué tiene que hacer el alumno.",
  "krita_tip": "Cómo se hace o se aplica en Krita.",
  "brush": "fineliner",
  "smoothing": "Basico",
  "color": [0.1, 0.1, 0.12],
  "memory": 3,
  "vary": { "shift": 40, "scale": [0.9, 1.1], "rotate": 10 },
  "kind": { "type": "trace", ... }
}
```

- `id`: único en toda la app. Las estrellas se guardan con él: no lo cambies en un ejercicio publicado.
- `brush`: `borrador`, `basico5`, `basico2`, `aero`, `lapiz`, `fineliner`, `gpen`, `marcador`, `humedo`, `pixel`.
- `smoothing` (opcional): `Ninguno`, `Basico`, `Ponderado`, `Estabilizador`, `Pixel`.
- `color` (opcional): RGB de 0 a 1.
- `memory` (opcional): segundos que se ven las ayudas antes de ocultarse.
- `vary` (opcional): variación de cada intento. Desplazamiento máximo en px, rango de escala y giro
  máximo en grados, alrededor del centro del lienzo. No se aplica a perspectiva, que varía por sí sola.

## Tipos de ejercicio (`kind`)

| `type` | Qué ve el alumno | Campos |
|---|---|---|
| `trace` | Las guías, para repasarlas | `tol`, `guides` |
| `points` | Solo los extremos de cada guía, numerados | `tol`, `guides` (mejor líneas rectas) |
| `copy` | La referencia al lado; dibuja donde quiera | `tol`, `guides` y/o `scene` |
| `perspective` | Horizonte, PF, líneas dadas y puntos de salida | `tol`, `scene`, `hints` |
| `paint` | Un objetivo de pintura | `target`: `ValueSteps`, `Gradient`, `Sphere`, `Cylinder`, `Silhouette`, `Cleanup`, `ColorBlend` |

`tol` es la tolerancia en px (entre 8 y 18 en los ejercicios actuales). `hints: true` permite mostrar
las líneas objetivo con la casilla "Mostrar ayuda".

## Guías (`guides`)

Cada guía es una figura. Las coordenadas van en px del lienzo y los ángulos en grados.
Cualquier guía acepta `"pressure"` para pedir un perfil de presión: `"taper_out"`, `"taper_in"`,
`"spindle"`, `{ "const": 0.5 }` o `{ "wave": 2 }`.

```json
{ "shape": "line", "a": [160, 140], "b": [800, 140] }
{ "shape": "lines", "a": [160, 140], "b": [800, 140], "step": [0, 90], "count": 5 }
{ "shape": "rays", "c": [480, 320], "r0": 80, "r1": 260, "count": 8, "start": 10 }
{ "shape": "bezier", "p": [[260, 120], [80, 160], [80, 480], [260, 520]] }
{ "shape": "circle", "c": [480, 320], "r": 120 }
{ "shape": "ellipse", "c": [480, 320], "rx": 140, "ry": 60, "rot": 30, "a0": 0, "a1": 180 }
{ "shape": "poly", "pts": [[140, 180], [400, 180], [400, 440]], "closed": true }
{ "shape": "spiral", "c": [480, 320], "r0": 12, "r1": 250, "turns": 3 }
{ "shape": "wave", "x0": 140, "x1": 820, "y": 320, "amp": 120, "cycles": 2 }
{ "shape": "star", "c": [660, 320], "r_out": 200, "r_in": 80 }
{ "shape": "loops", "x0": 170, "x1": 790, "y": 320, "rx": 55, "ry": 90, "turns": 4 }
{ "shape": "petals", "c": [480, 280], "inner": 40, "outer": 150, "count": 6 }
```

`lines` y `rays` crean varias guías a la vez.

## Escenas de perspectiva (`scene`)

Cada escena se genera con un `gen`. Con cada intento se mueven un poco los puntos de fuga, las
posiciones y los tamaños.

```json
{ "gen": "converge", "vp": [480, 250], "from": [[120, 520], [300, 590]], "reach": 0.75 }
{ "gen": "grid1", "vp": [480, 200], "baseline": 600, "x0": 140, "x1": 820, "cols": 6, "rows": 4 }
{ "gen": "box1", "vp": [480, 180], "boxes": [{ "rect": [360, 340, 600, 530], "depth": 0.35 }], "back": true }
{ "gen": "box2", "horizon": 220, "vps": [60, 900],
  "boxes": [{ "x": 480, "top": 300, "bottom": 520, "left": 0.35, "right": 0.35 }], "far": true, "xray": false }
{ "gen": "box3", "horizon": 560, "vps": [40, 920], "vp3": [480, -700],
  "boxes": [{ "x": 480, "y": 470, "height": 290, "left": 0.3, "right": 0.3 }], "far": true }
```

- `converge`: rayos desde cada punto de `from` hacia el PF; `reach` es qué parte del camino se pide.
- `grid1`: suelo en cuadrícula a un punto.
- `box1`: cajas a un punto; la cara frontal está dada. `back` pide también la cara de atrás.
- `box2`: cajas a dos puntos; la arista frontal está dada. `left` y `right` son la fracción del camino
  hacia cada PF. `far` pide la cara visible de arriba o de abajo; `xray`, todas las aristas ocultas.
- `box3`: cajas a tres puntos; `y` es el extremo de la arista frontal más cercano al horizonte y la
  arista va hacia `vp3`. `far` pide la cara de ese extremo.

Una escena también sirve para un ejercicio de copia: `{ "type": "copy", "tol": 14, "scene": { ... } }`
dibuja la caja completa como referencia.
