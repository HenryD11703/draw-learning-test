# Cambios

## 0.2.0 (14/11/2026)

De una app de ejercicios a un recorrido completo para aprender a dibujar y entrenar el ojo crítico.

### Añadido
- **Recorrido de 48 lecciones** en 12 bloques sin fecha, basadas en libros de dibujo (Drawabox, Betty
  Edwards, Scott Robertson, Bert Dodson, James Gurney, Kimon Nicolaïdes, Andrew Loomis, Michael Hampton,
  George Bridgman y otros). Pantalla **Hoy** con la práctica del día: calentamiento con un ejercicio de la
  lección y otro de repaso, gesto diario, estudio, racha y revisión al cerrar cada lección.
- **Estudio con referencia:** foto al lado del lienzo (zoom, girar, voltear, ocultar), comparación con
  superposición, alternancia, espejo, grises, rejilla y medidas en unidades, y autocrítica guiada por tema.
  Primero se responde de memoria y después se comprueba; la app mide el **ojo crítico** con esas dos
  respuestas.
- **Diario de estudios** con la referencia, el dibujo, las respuestas y las notas, filtro por tema y los
  **dibujos de control** (las mismas fotos cada cuatro semanas) en fila.
- **Biblioteca de referencias** con una carpeta por tema; sesiones de gesto con fotos propias; copia de
  maestro semanal y recordatorio para pedir una opinión de fuera.
- **Contenido como datos (JSON):** 17 módulos y 169 ejercicios. Nuevos módulos de perspectiva, volumen,
  sombreado, figura, cabeza, manos, entintado, observación y memoria, valor, luz y forma, bordes, color, y
  color y luz. Variaciones en cada intento, gestos con tiempo y marcas de error en el lienzo.
- **Pintura:** escenas definidas en datos y calificación de valor, tono y saturación.
- **Lienzo libre:** capas, imagen de referencia, rejilla y tercios, vista girada y en espejo, galería de
  dibujos y sesiones de gesto.
- Nuevo diseño de la interfaz: tema propio, pantalla de inicio y mapa de niveles.

### Cambiado
- Cada modo (ejercicios, lienzo libre y estudio) tiene su propio lienzo.
- Las fechas del diario se muestran en la hora local.

### Proyecto
- Licencia **GPL-3.0-or-later**, README con créditos a Krita y aviso de no afiliación.
- La configuración local de compilación ya no está en el repositorio (ver
  `app/.cargo/config.toml.example`).

## 0.1.0 (12/11/2026)

- 7 módulos y 32 ejercicios progresivos.
- Suavizado compatible con Krita (básico, ponderado, estabilizador y píxel).
- Curvas de presión, calibración con la mano y simulación con ratón.
- Preajustes de pinceles basados en los de Krita y calificación automática.
