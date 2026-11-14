# Trazo Academia — hoja de mecánicas

Valores leídos del código de Krita (libkritaui.dll, Krita x64 instalado) con Ghidra.
Las funciones citadas son direcciones dentro de `libkritaui.dll`. Lo que no viene del
código está marcado **estimado** o **diseño propio**.

Krita es software libre (GNU GPL) y su código fuente es público en
[invent.kde.org/graphics/krita](https://invent.kde.org/graphics/krita). Las clases citadas aquí están en
estos archivos, que sirven para comprobar cada valor sin necesidad del binario:

| Clase | Archivo en el código fuente de Krita |
|---|---|
| `KisToolFreehandHelper` | `libs/ui/tool/kis_tool_freehand_helper.cpp` |
| `KisSmoothingOptions` | `libs/ui/tool/kis_smoothing_options.cpp` |
| `KisSpeedSmoother` | `libs/ui/tool/kis_speed_smoother.cpp` |
| `KisPaintingInformationBuilder` | `libs/ui/tool/kis_painting_information_builder.cpp` |
| `KisConfig` | `libs/ui/kis_config.cc` |
| `KisCubicCurve` | `libs/image/kis_cubic_curve.cpp` |

Este proyecto reimplementa ese comportamiento en Rust y, por eso, se distribuye con la misma familia de
licencias (GPL-3.0-or-later; ver `LICENSE` en la raíz del repositorio).

## 1. Suavizado del trazo (KisSmoothingOptions / KisToolFreehandHelper)

| Valor | Krita | Fuente |
|---|---|---|
| Tipos de suavizado | 0 Ninguno, 1 Básico, 2 Ponderado, 3 Estabilizador, 4 Pixel | `KisToolFreehandHelper::paint` 0x180340180 (ramas `type == 1/2/3/4`) |
| Tipo por defecto | 1 (Básico) | `KisConfig::lineSmoothingType` 0x1801bf9a0 |
| Distancia mínima | 50.0 | `lineSmoothingDistanceMin` 0x1801bfa40 (0x4049000000000000) |
| Distancia máxima | 50.0 | `lineSmoothingDistanceMax` 0x1801bfaf0 |
| Agresividad de cola | 0.15 | `lineSmoothingTailAggressiveness` 0x1801bfc40 (0x3fc3333333333333) |
| Suavizar presión | no | `lineSmoothingSmoothPressure` 0x1801bfcf0 |
| Distancia escalable con el zoom | sí | `lineSmoothingScalableDistance` 0x1801bfd90 |
| Distancia de retardo (zona muerta) | 50.0, activada | `lineSmoothingDelayDistance` 0x1801bfe30, `UseDelayDistance` 0x1801bfee0 |
| Terminar curva estabilizada | sí | `lineSmoothingFinishStabilizedCurve` 0x1801bff80 |
| Estabilizar sensores | sí | `lineSmoothingStabilizeSensors` 0x1801c0020 |
| Intervalo de muestreo del estabilizador | 50 ms | `KisConfig::stabilizerSampleSize` 0x1801c1b80 (0x32) |
| Pintado retardado del estabilizador | sí | `stabilizerDelayedPaint` 0x1801c1c20 |

### Ponderado (tipo 2) — `paint` 0x180340180, constantes en 0x180783cb0
- Se guarda un historial de puntos y la distancia entre cada uno. Solo actúa con más de 3 puntos.
- `sigma = escala * ((1 - velocidad) * distMax + distMin * velocidad) / 3.0`
  (3.0 en 0x180783cb8; `escala = 1/zoom` si la distancia es escalable).
- Peso gaussiano: `w = exp(-D² / (2·sigma²)) / (sqrt(2π)·sigma)` (2.5066 en 0x180783cc0), donde `D`
  es la distancia acumulada desde el punto más nuevo hacia atrás.
- Cola: si la presión baja entre un punto y el siguiente, `D += Δp · (agresividad·40) · (1 - p) · 3 · sigma`
  (40.0 en 0x180783cc8).
- Corte: deja de mirar atrás cuando `peso_más_nuevo / w > 100` (100.0 en 0x180783ce0).
- Posición = media ponderada; la presión también si "suavizar presión" está activo.
- Después de ponderar, el punto pasa por el Básico (curva Bézier).

### Básico (tipo 1) — `paint` + `paintBezierSegment` 0x18033f850, constantes en 0x180783c40
- Tangente = (pos nueva − pos anterior-anterior) / max(1 ms, Δt).
- Se pinta el tramo entre el punto "anterior-anterior" y el "anterior" como Bézier cúbica.
- Si la línea entre los objetivos de control corta al tramo, cada control va a 0.5·|tramo| a lo largo
  de su tangente (0.5 en 0x180783c60); si no, al cruce de las tangentes (si |x|+|y| ≤ 1e6, 0x180783c40)
  o al punto medio (0.5, 0x180783c50).
- Proporción de velocidades `r = min(v1/v2, v2/v1)` (mín 1e-6); `k = (1 − max(0, r − 0.8)) · 0.8`
  (−0.8 en 0x180783c88, 0.8 en 0x180783c98). El lado más rápido usa `k`, el más lento `r·k`.
- Si una tangente es cero se pinta línea recta. Sin movimiento durante 100 ms se cierra el tramo
  (`QTimer::start(100)` en `paint`).

### Estabilizador (tipo 3) — `stabilizerStart` 0x18033f630, `stabilizerPollAndPaint` 0x1803416c0, `getStabilizedPaintInfo` 0x180341430
- Cola de `max(3, round((1−vel)·distMax + distMin·vel))` copias del primer punto (sin escala de zoom).
- Cada 50 ms se procesan los eventos: el punto pintado es la media de la cola con el evento
  actual (mezcla `i/(i+1)`), luego sale el primero de la cola y entra el evento.
- Zona muerta: si el evento está a menos de `distanciaRetardo/zoom` del último punto pintado,
  no se avanza.
- Al soltar, con "terminar curva" se sigue promediando hasta alcanzar el último punto.

### Pixel (tipo 4)
- Posición redondeada al centro del píxel (`floor + 0.5`, 0.5 en 0x180783cf0); se evita el píxel de
  esquina cuando el siguiente queda a 1 píxel en ambos ejes.

## 2. Presión

| Valor | Krita | Fuente |
|---|---|---|
| Curva global de presión de la tableta | "0,0;1,1;" (lineal) | `KisConfig::pressureTabletCurve` 0x1801bd1d0, cadena en 0x18075f2ac |
| Muestras de la curva | 1025, con interpolación lineal | `KisPaintingInformationBuilder::updateSettings` 0x18033be30 (`floatTransfer(0x401)`), `pressureToCurve` 0x18033c530 |
| Velocidad de dibujo | `min(1, velocidad / maxAllowedSpeedValue)` | `createPaintingInformation` 0x18033c250 |
| maxAllowedSpeedValue | 30 | `updateSettings` 0x18033be30 |
| Suavizado de velocidad | 3 muestras | `KisSpeedSmoother::updateSettings` 0x18033b3e0 |
| Unidad de velocidad | px/ms, con escala ×10 antes de dividir por 30 para que la mano llegue al rango 0..1 | **estimado** |
| Interpolación entre puntos de la curva | spline cúbica por tramos `a·x³+b·x²+c·x+d`, x limitada al primer/último punto, resultado limitado a [0,1] | `KisCubicCurve::Data::value` 0x180207ff0 en libkritaimage.dll; condición de borde "natural" **estimada** |

### Adaptación propia (diseño propio)
- **Calibración**: el alumno hace trazos suave, normal y fuerte; con la presión media de cada uno se arma
  una curva (0,0) (suave, 0.2) (normal, 0.5) (fuerte, 0.9) (1,1). Así el rango natural de su mano cubre todo.
- **Presión simulada con ratón**: constante, o por velocidad (`p = 1 − 0.7·velocidad`), para quien no tiene tableta.

## 3. Pinceles

Nombres y orden de categorías del paquete "Krita 4 Default Resources" (a) Borradores, b) Básicos,
c) Lápices, d) Tintas, e) Rotuladores, i) Húmedos, u) Pixel art).

Curvas leídas de los archivos .kpp (XML comprimido dentro del PNG). "Dureza" = `hfade` de Krita.

| Pincel | Krita | Fuente |
|---|---|---|
| a) Eraser Circle | diámetro 50, hfade 0.87, tamaño = presión "0,0;0.414,0.333;0.675,0.772;1,1" | `a)_Eraser_Circle.kpp` |
| b) Basic-5 Size | diámetro 5, hfade 0.5, espaciado 0.1, tamaño = presión "0,0;0.35,0.1;1,1" | `b)_Basic-5_Size_default.kpp` |
| b) Basic-2 Opacity | hfade 0.5, tamaño fijo, opacidad = presión lineal | `b)_Basic-2_Opacity.kpp` |
| b) Airbrush Soft | diámetro 600, hfade 0, opacidad = presión "0,0;0.675,0.579;0.859,1" | `b)_Airbrush_Soft.kpp` |
| c) Pencil-2 | diámetro 5, tamaño "0,0.486;1,1", opacidad "0,0;0.128,0.1;0.474,0.711;1,1" | `c)_Pencil-2.kpp` |
| d) Ink-2 Fineliner | diámetro 5, espaciado 0.1, tamaño = presión (0→0.754, 1→1) × velocidad (0→1, 1→0) | `d)_Ink-2_Fineliner.kpp` |
| d) Ink-3 Gpen | diámetro 5.97, flujo 0.6, tamaño "0,0.07;0.317,0.235;0.651,0.838;1,1" | `d)_Ink-3_Gpen.kpp` |
| e) Marker Details | diámetro 20, espaciado 0.2, hfade 0.66, curvas de tamaño, opacidad y flujo | `e)_Marker_Details.kpp` |
| i) Wet Paint | colorsmudge, diámetro 46, espaciado 0.07, curvas de tamaño, opacidad y arrastre | `i)_Wet_Paint.kpp` |
| Tamaños en la app | escalados al lienzo de 960×640 (p. ej. Basic-5 a 20 px, Airbrush a 90 px) | **diseño propio** |
| Flujo del aerógrafo (0.12) y grano del lápiz | acumulación y textura de papel | **estimado** |
| Mezcla del pincel húmedo | `color = pintura + (lienzo − pintura) · 0.8 · curva_arrastre(p)` | **estimado** (Krita usa el motor colorsmudge) |
| Pincel Difuminar (k) Blender Basic) | sin color propio: `color = lienzo` (arrastre 1), tamaño 36, dureza 0, flujo 0.5, opacidad = presión | **estimado** (no leído de su .kpp) |
| Espaciado entre manchas | `max(1, diámetro × espaciado)` | **estimado** |
| Opacidad del trazo | el trazo se acumula con el flujo hasta el tope de opacidad ("lavado") | **estimado** |

## 4. Calificación (diseño propio, no existe en Krita)
- Contención: `pintado_fuera` = celdas pintadas fuera de la zona / celdas de la zona.
- Precisión = `1 − distancia_media / (2·tolerancia)`. La distancia se promedia por celda de tinta de 4 px,
  no por mancha: repasar un tramo o pintar fino no le da más peso.
- Cobertura = parte de la guía que queda a menos de la tolerancia de algún trazo.
- Pulso (suavidad) = `1 − (desvío_medio − 0.15) / 1.5`. El desvío es cuánto cambia la flecha (distancia con
  signo del punto central a la cuerda de sus vecinos) entre muestras consecutivas, con muestreo cada 6 px
  de arco, tope de 2 px por muestra y factor 0.85. Una curva limpia (círculo, espiral) no penaliza.
- Presión = `1 − error_medio / 0.35` frente al perfil objetivo de la guía, también por celda de tinta.
- Valor (pintura) = `1 − error_medio_de_luminancia / 0.3` en celdas de 16 px; contención = `1 − 3·pintado_fuera`.
- Transición (pintura) = `1 − diferencia_media_de_saltos / 0.08`, comparando cada celda con la de su derecha
  y la de abajo.
- Total trazo: 40 % precisión, 30 % cobertura, 30 % pulso; con presión 30/25/15/30.
- Completitud: precisión, pulso y presión se multiplican por `min(1, cobertura / 0.5)`; con menos de media
  guía cubierta cuentan en proporción.
- Total pintura: 60 % valor, 25 % contención, 15 % transición.
- La guía magnética resta hasta un 25 % (`total × (1 − 0.25·fuerza)`). Solo existe en los ejercicios de calcar.
- Punto a punto: cada guía se empareja con el trazo cuyos extremos quedan más cerca de los suyos (en
  cualquier sentido, a menos de 4·tolerancia). Puntería = `1 − error_medio_de_extremos / (2.5·tolerancia)`;
  precisión = `1 − distancia_media_a_la_guía / (2·tolerancia)`; completadas = pares unidos / pares.
  Total = `completadas^1.5 × (40 % puntería + 35 % precisión + 25 % pulso)`.
- Copia: se busca la escala y el desplazamiento que mejor llevan el dibujo sobre la referencia (ICP
  simétrico, 25 iteraciones, sin giro) y luego se compara. Forma = `1 − (d(dibujo→ref) + d(ref→dibujo)) / 2 /
  (2·tolerancia)`; cobertura = parte de la referencia a menos de 1.5·tolerancia del dibujo; proporción =
  `1 − |ln(aspecto_dibujo / aspecto_ref)| / 0.4`. Total = `30 % cobertura + completitud × (50 % forma +
  10 % proporción + 10 % pulso)`, con la misma completitud que en calcar.
- Perspectiva: cada trazo de más de 20 px se ajusta a una recta. Convergencia = `1 − error_angular_medio / 8°`,
  donde el error de cada recta es el ángulo con la dirección válida más cercana desde su centro (hacia un PF,
  vertical u horizontal), ponderado por longitud y con tope de 20°. Cada tarea se empareja con la recta que
  sale de su punto (a menos de 2.5·tolerancia) en su dirección (menos de 15°). Puntería = `1 − distancia_media_al_punto
  / (2·tolerancia)`; completadas = media de `min(1, largo_cubierto / (0.8·largo_pedido))`.
  Total = `completadas^1.5 × (45 % convergencia + 30 % puntería + 25 % pulso)`.
- Rayado: cada trazo de más de 8 px se ajusta a una recta y pertenece a la zona que contiene su centro.
  En cada zona, las rectas se reparten en familias (una, o dos si es trama cruzada) según la dirección
  pedida más cercana. Dirección = `1 − error_angular_medio / 12°`; regularidad = `1 − (cv − 0.2) / 0.6`,
  con `cv` el coeficiente de variación de la separación entre rectas vecinas (dos rectas a menos de 2 px
  cuentan como una); densidad = `1 − |ln(separación_mediana / separación_pedida)| / 0.6`; cobertura =
  parte de una rejilla de 8 px dentro de la zona que tiene tinta a menos de `max(6, 0.75·separación)`;
  contención = `1 − (parte de la tinta fuera de toda zona) / 0.3`. Las zonas pesan según su área.
  Total = `25 % cobertura + min(1, cobertura / 0.6) × (25 % dirección + 20 % regularidad + 20 % densidad
  + 10 % contención)`.
- Ejercicios con tiempo límite (gesto): la cuenta empieza al abrir el ejercicio; al llegar a cero se
  califica lo dibujado con el calificador de su tipo y no se aceptan trazos nuevos hasta Repetir.
- Pintura en color: además del valor (luminancia), por cada celda de 16 px se compara el tono (HSV,
  diferencia circular en grados, pesada por la saturación del objetivo, `peso = clamp((s − 0.08) / 0.3)`)
  y la saturación. Tono = `1 − error_medio / 40°`; saturación = `1 − error_medio / 0.3`.
  Total = `30 % valor + 25 % tono + 15 % saturación + 15 % contención + 15 % transición`.
- Escenas de pintura: la esfera y el cilindro usan `t = 0.3 + 0.7·(n·l)` en el lado iluminado y
  `t = reflejada·(1 − n_z)·√(−n·l)` en la sombra, entre el color de sombra y el de luz (diseño propio,
  siguiendo las familias de luz y sombra de Gurney). La luz se toma como `(lx, ly, 0.6)` normalizado.
- Guías de contexto y ocultas (calcar): las de contexto no se califican; las ocultas sí, aunque no se vean.
- Estrellas: 60 → 1, 75 → 2, 90 → 3. Un módulo se abre al conseguir al menos una estrella en la mitad
  (redondeada hacia arriba) de los ejercicios del anterior; dentro de un módulo abierto, todos los ejercicios
  están disponibles.
- Variaciones: cada intento construye el ejercicio con una semilla nueva (desplazamiento, escala y giro de
  las guías; posición de los puntos de fuga y tamaño de las cajas en perspectiva). Las variaciones que se
  salen del lienzo se descartan. El modo demostración usa siempre la versión de referencia.

## 5. Estudio con referencia y diario (diseño propio, sin nota)

El trabajo de observación (figura, cabeza, manos, bodegón, paisaje, copias de maestros, color) no se
califica: se dibuja a partir de una foto real y se compara con herramientas, y la autocrítica la hace el
alumno. Solo los fundamentos (líneas, elipses, perspectiva, rayado, valor y color de las escenas) siguen
con nota automática.

- Fases: preparar (soltar la foto en la ventana, elegir tema y tiempo) → dibujar (referencia a la
  izquierda, lienzo propio a la derecha; ocultar, girar 90°, voltear, zoom con la rueda y mover arrastrando)
  → comparar. Con tiempo elegido, al acabarse pasa sola a comparar.
- Comparar: la referencia se coloca sobre el lienzo encajada en él (`ajuste = min(W/ancho, H/alto)`) y se
  puede superponer con opacidad, alternar cada 0,5 s («Alternar»), mover y escalar («Alinear») y cubrir
  con la misma rejilla de 4 × 4 que la foto. «Espejo» y «Grises» de la barra afectan a la foto y al dibujo.
- Medir (Betty Edwards, «visión de proporciones»): la primera medida de cada lado es la unidad; cada medida
  siguiente se expresa en unidades en la foto y en el dibujo. Diferencia = `(dibujo / foto − 1) × 100 %`;
  verde hasta ±5 %, ámbar hasta ±12 %, rojo por encima. Las medidas de la foto se guardan en px de la
  imagen y las del dibujo en px del lienzo, así que no cambian con el zoom.
- Primera impresión: al terminar de dibujar, las preguntas se responden primero sin superponer ni medir
  (se pueden usar espejo y grises). Después se comprueba con las herramientas y se dan las respuestas
  finales. Ojo crítico = parte de las preguntas respondidas en ambas veces en que la primera respuesta ya
  era la final; se muestra para los 10 últimos estudios y, si hay más de 10, para los 10 primeros.
- Autocrítica: cuatro preguntas por tema, sacadas del método que cita cada tema (Nicolaïdes, Edwards,
  Gurney y Dodson, Loomis, Drawabox, copia de maestros), con «Sí / A medias / No» y dos notas: lo mejor y
  qué mejorar en el siguiente.
- Diario: cada estudio guardado crea `Imágenes/TrazoAcademia/estudios/estudio-<segundos>/` con
  `dibujo.png`, `referencia.png` (máx. 1600 px) y `estudio.json` (tema, minutos, respuestas y notas).
  El diario los muestra del más nuevo al más antiguo, con filtro por tema y horas acumuladas.
- Demostración: `TRAZO_PANTALLA=estudio` (con `TRAZO_FASE=comparar` para la comparación) usa una escena de
  pintura como referencia; `TRAZO_DIARIO=<carpeta>` cambia la carpeta del diario y, en la comparación,
  guarda el estudio de demostración. `TRAZO_PANTALLA=diario` abre el diario.

## 6. Recorrido de 48 lecciones, hilos continuos y biblioteca (diseño propio)

Por qué está hecho así (resumen de la revisión de la investigación, noviembre de 2026):
- La mayoría de los errores al dibujar del natural vienen de percibir mal, no de la mano (Cohen y Bennett,
  1997; Ostrofsky y colaboradores, 2015; Chamberlain y Wagemans, 2016). Por eso el centro es el estudio con
  referencia y las herramientas que no se dejan engañar: superponer y medir.
- Los principiantes calcan líneas simples casi tan bien como los expertos (Tchalenko); los ejercicios con
  nota quedan como calentamiento corto y su nota es orientativa.
- La autoevaluación de los principiantes es poco fiable. De ahí la primera impresión que luego se comprueba
  (y que mide el ojo crítico) y la opinión de fuera periódica.
- La práctica repartida y mezclada retiene mejor que la práctica en bloques (metaanálisis de Donovan y
  Radosevich, 1999; Brunmair y Richter, 2019). Ningún método serio va un tema por mes, así que las lecciones
  no tienen fecha y lo aprendido se repasa siempre.

- `content/programa.json`: 12 bloques de 4 lecciones. Cada lección tiene título, fuente (el libro o método
  en el que se basa), texto, módulos de calentamiento (ids de `content/*.json`), tema del estudio y minutos
  sugeridos. Un test comprueba que son 48 lecciones y que todos los módulos existen.
- Un día queda completo con 2 ejercicios distintos calificados (cualquier nota), una sesión de gesto y un
  estudio (o dibujo de control) guardado en el diario. Se recomiendan 4 días completos por lección, pero la
  lección se cierra cuando el alumno quiere, con una revisión (qué mejoró, qué error se repite, qué dijo
  alguien de fuera y el foco para la siguiente; el foco se muestra durante la lección siguiente).
- Calentamiento: un ejercicio de un módulo de la lección y otro de repaso de un módulo de lecciones
  anteriores. En ambos casos se elige el módulo con la práctica más antigua (sin contar hoy, para que la
  sugerencia no cambie mientras se hace), y dentro de él el ejercicio hecho hoy, el primero sin tres
  estrellas o el menos repetido.
- Gesto: 5 poses de 60 s con fotos al azar de la carpeta «gesto» (los dibujos no se guardan). Si la carpeta
  está vacía se usa la sesión de poses esquemáticas de la app.
- Copia de maestro: una por semana (últimos 7 días), 30 minutos, con fotos de la carpeta «maestros».
- Opinión de fuera: se recuerda si pasaron 35 días desde la última revisión con ese campo relleno (o desde
  el primer día de práctica).
- Dibujos de control: de 1 a 3 fotos fijas en la carpeta «control», 15 minutos cada una. Se abre una ronda
  la primera vez (punto de partida), cada 28 días y en la última lección de un bloque si no hubo ronda
  desde que empezó el bloque. La ronda se cierra cuando todas las fotos tienen dibujo; cada dibujo cuenta
  como el estudio del día. El diario los muestra en fila, del más antiguo al más reciente.
- Racha: días seguidos con algo de práctica, contando hasta hoy o, si hoy aún no se practicó, hasta ayer.
  Las fechas son locales (AAAA-MM-DD).
- El estado se guarda en `progreso.json` (`program`: lección, inicio de la lección y del bloque, días,
  revisiones, dibujos y rondas de control, copias de maestro). Los archivos guardados con la versión
  anterior (por semanas) se siguen leyendo.
- Biblioteca: `Imágenes/TrazoAcademia/referencias/<tema>/` (gesto, proporcion, figura, valor, cabeza, manos,
  objetos, paisaje, maestros, color) y `control/`. Los estudios empiezan con una foto PNG o JPG al azar de la
  carpeta del tema, sin repetir la última.
- Demostración: `TRAZO_PANTALLA=hoy` (lección 6, o `TRAZO_LECCION=<n>`), con `TRAZO_FASE=revision` (lección
  lista para cerrar) o `TRAZO_FASE=estudio` (abre el estudio de la lección); `TRAZO_REFERENCIAS=<carpeta>`
  cambia la biblioteca y `TRAZO_DESPLAZAR=<px>` baja la pantalla. En el estudio de demostración,
  `TRAZO_IMPRESION=1` deja la comparación en la primera impresión y `TRAZO_CONTROL=<nombre>` guarda el
  estudio como dibujo de control.
