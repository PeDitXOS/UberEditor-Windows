# UberEditor

Editor de video de escritorio multiplataforma (Tauri 2 + Rust + React) pensado para creadores de contenido, con superpoderes de IA: **edición basada en texto** (Whisper palabra a palabra), **silencios fuera con un click**, **verticales automáticos**, **avatar reactivo por emociones**, **subtítulos karaoke** y un **servidor MCP embebido** para que un agente (Claude, etc.) edite tu proyecto por ti.

**El plan maestro está en [PLAN.md](PLAN.md)** — arquitectura, las 16 features al detalle, mapeo del Youtubers-toolkit y roadmap.

- UI 100 % en español · tema carbón cálido con acento ámbar
- Todo deshacible: cada operación (incluidas las de IA y las del MCP) es **una** entrada de undo
- Mismo motor de render en preview y export (cadenas ffmpeg compartidas): lo que ves es lo que sale
- 105 tests (unitarios, de píxeles sobre exports reales y 13 pasos visuales con Playwright)

---

## Requisitos

| Qué | Versión | Notas |
|---|---|---|
| **FFmpeg + FFprobe** | ≥ 6 en el `PATH` | El corazón del render. `brew install ffmpeg` / `apt install ffmpeg`. Se puede apuntar a binarios concretos con `UE_FFMPEG` y `UE_FFPROBE` |
| **Rust** | estable | Solo para compilar |
| **Node** | ≥ 20 | Solo para compilar / desarrollo |

```bash
npm install
npx tauri dev        # la app de escritorio completa
```

Sin instalar nada más: los modelos de Whisper se descargan solos la primera vez que transcribes (a la carpeta de datos de la app).

---

## La interfaz

```
┌────────────┬──────────────────────────────┬─────────────┐
│ Medios /   │                              │  Inspector  │
│ Texto      │        Preview               │  (del clip  │
│ (pool +    │  (frames reales + overlays)  │  seleccio-  │
│ transcript)│                              │  nado)      │
├────────────┴──────────────────────────────┴─────────────┤
│  Línea de tiempo (pistas V/A, waveforms y thumbs reales) │
├──────────────────────────────────────────────────────────┤
│  Barra de estado (guardado, selección, última acción)    │
└──────────────────────────────────────────────────────────┘
```

- **Medios**: importa con `+ Importar` o arrastrando archivos desde el Finder/Explorador; **doble click** sobre un medio lo añade al timeline.
- **Texto**: aparece cuando hay transcripciones; desde ahí se edita el video borrando o moviendo palabras.
- **Preview**: frames reales del motor (usa un proxy ligero si el archivo es grande). En pausa, el frame es exacto al export.
- **Inspector**: todas las propiedades del clip seleccionado; sin selección muestra los ajustes de IA (modelo/idioma de Whisper).

---

## Atajos de teclado

| Tecla | Acción |
|---|---|
| `Espacio` | Reproducir / pausar |
| `J` / `K` / `L` | Shuttle: atrás / pausa / adelante (repetir duplica: 1→2→4→8×) |
| `S` (o `⌘K`) | Dividir el clip bajo el playhead (los enlazados se dividen juntos) |
| `Supr` / `Retroceso` | Eliminar selección · con `⇧` elimina **y cierra el hueco** (ripple) |
| `←` / `→` | Un frame atrás/adelante · con `⇧` diez frames |
| `Inicio` | Ir a 0 |
| `I` / `O` | Marcar entrada / salida del **rango de trabajo** (banda ámbar en la regla) |
| `⇧X` | Limpiar el rango I–O |
| `⌘Z` / `⌘⇧Z` | Deshacer / rehacer |
| `⌘S` / `⌘O` | Guardar / abrir proyecto |
| `Alt` (durante un arrastre) | Desactiva el imán (snapping) |

---

## Edición en el timeline

- **Mover**: arrastra un clip; el **imán** lo pega a bordes de otros clips, al playhead, al 0, al rango I–O y a los marcadores (guía punteada al engancharse; `Alt` lo desactiva).
- **Recortar**: arrastra los **bordes** de un clip (asas visibles al seleccionar).
- **Selección múltiple**: arrastra un **rectángulo** sobre área vacía (marquee); `⇧` acumula. La barra de estado muestra cuántos clips llevas.
- **Clips enlazados 🔗**: al añadir un video con audio se crean dos clips (video en `V*`, audio en `A*`) que se comportan como uno: dividir, mover, recortar, cambiar velocidad o borrar afecta a ambos. `Inspector → Enlace → Desenlazar` los separa.
- **Pistas**: `+V` / `+A` añaden pistas; en la cabecera: `M` silencia, `S` solo, 🔒 bloquea, **doble click en el nombre** renombra, ✕ elimina (deshacible), y en pistas de audio el **dB se arrastra** verticalmente (doble click → 0 dB).
- **Multicapa real**: la pista de video más baja es la base; los clips de pistas superiores se componen encima (con su posición, escala y opacidad) también en el export.
- **Velocidad**: presets 0.25×–4× en el Inspector. En el export **el tono de la voz se conserva** (atempo); en la reproducción en vivo el pitch cambia por ahora.

## Transformación y keyframes

Cada clip tiene **Posición X/Y, Opacidad, Escala y Rotación** (y el audio, **Ganancia**). Junto a cada slider:

- `◇` — la propiedad no anima; click = **crear keyframe** en el playhead (la propiedad pasa a animada).
- `◆` — hay keyframe justo en el playhead; click = quitarlo.
- Con la propiedad animada, **mover el slider escribe un keyframe** en el playhead (como Premiere/Resolve) y el valor mostrado es el del playhead.
- Debajo aparece el **editor de curvas**: arrastra los rombos (tiempo y valor), **doble click** añade o borra keys, y al seleccionar un key eliges su interpolación (**lineal / escalón / suave**).
- Los rombos también se dibujan sobre el clip seleccionado en el timeline.

La animación se ve **igual en pausa, reproduciendo y en el export** (misma matemática de curvas en los tres caminos). El crop existe pero aún no es animable desde la UI.

## Efectos (packs modulares)

`Inspector → Efectos → + Añadir efecto`. Incluidos: **Chroma Key**, Corrección de color, Desenfoque gaussiano y Relleno vertical. Cada efecto sale de un `manifest.json` (parámetros + plantilla ffmpeg), así que preview y export usan exactamente la misma cadena.

**Packs propios**: crea una carpeta en `«config de la app»/effects/<mi-efecto>/manifest.json` y pulsa `↻ packs`. Un manifest inválido no rompe nada (se reporta) y un pack con el mismo `id` que uno core lo reemplaza.

## Generadores (formas y fondos)

Botón **▦ Forma** en el timeline → añade un clip generado:

- **Rectángulo sólido**: color, ancho y alto.
- **Degradado**: dos colores (diagonal).

Se cambia el tipo y los parámetros en `Inspector → Generador`. Como son clips normales, la transformación completa les aplica: puedes animar con keyframes un panel que entra deslizándose, ponerlo semitransparente detrás de un título, etc. Mismo sistema de manifests que los efectos (carpeta `generators/`).

## Texto, títulos y plantillas

- **+ Título** añade un texto en el playhead (en una pista libre).
- `Inspector → Texto`: contenido, **fuente del sistema** (todas las instaladas), tamaño, color, alineación izquierda/centro/derecha y posición X/Y.
- **Plantillas**: guarda un estilo con nombre y aplícalo a cualquier título después.
- Todo se quema en el export con la misma fuente y colocación que ves en el preview.

## Transiciones

`Inspector → Transición` (en el clip de la derecha del corte): **11 tipos** (crossfade, wipes, slides, círculo, dissolve, pixelize, radial) con duración configurable. Los handles se extienden hacia ambos lados limitados por el material disponible, y funcionan también entre clips con velocidades distintas.

---

## IA

### Transcripción (Whisper)

- Botón **T** sobre un medio del pool → transcribe palabra a palabra (el modelo se descarga solo). `T✓` = ya transcrito.
- **Modelo e idioma** se eligen en el Inspector con nada seleccionado (`IA · Whisper`): tiny/base/small/medium/large-v3-turbo, idioma auto/es/en/…

### Edición por texto

Pestaña **Texto**: la transcripción completa, con la palabra actual resaltada al reproducir (click en una palabra = seek).

- Marca palabras y **✂ Cortar** — elimina esos trozos del video **en todas las pistas** y cierra los huecos (1 undo).
- **⇢ Mover** — reordena un rango de material a otro punto del timeline (reordena frases habladas sin tocar cuchillas).

### Silencios

`Inspector → Silencios` (clip con audio):

- **🔇 Eliminar** — corta los silencios y cierra los huecos (todas las pistas, 1 undo).
- **⏩ Acelerar 4×** — en vez de borrar, acelera los tramos silenciosos.
- Sliders de **umbral (dB)**, **duración mínima** y **margen** alrededor del habla.

### Subtítulos automáticos

Botón **💬** en un clip transcrito. Tres modos en `Inspector → Subtítulos`:

- **Por frases** — una línea por segmento.
- **Palabra a palabra** — una palabra grande cada vez (estilo shorts).
- **Karaoke** — la frase completa visible y **cada palabra se enciende al sonar** (color de resaltado configurable).

### Vertical automático (Shorts/Reels)

Botón **📱 Vertical** → genera una secuencia 1080×1920 con fondo desenfocado y el video centrado. El selector de secuencias (junto a los botones del timeline) permite volver a la horizontal. Cada secuencia se exporta por separado.

### Avatar reactivo

Botón **🧑‍🎤** en un clip transcrito → elige el `config.json` de avatares (formato compatible con el Youtubers-toolkit: un video en loop por emoción). El avatar aparece en la esquina, **cambia de emoción según lo que dices** (clasificador de energía/ritmo offline, u OpenAI-compatible si defines `OPENAI_API_KEY`) y en el export tiembla al ritmo del volumen. Visible en pausa, reproducción y export.

---

## Audio

- **Ganancia** (animable con keyframes), **Pan** (ley de balance), **fades** de entrada/salida por clip.
- Volumen por **pista** (arrastra el dB de la cabecera).
- **Medidores RMS L/R** en la barra de transporte durante la reproducción.
- El audio es el **reloj maestro**: la posición viene de los frames servidos al dispositivo (sin drift).

## Export

Botón **Exportar…**:

- **Presets**: YouTube 1080p, YouTube 4K, Máxima calidad, Borrador rápido, **Solo audio (M4A)** y **GIF**.
- Ajustables: resolución máxima, calidad CRF, velocidad del códec, bitrate de audio.
- **Normalización R128** opcional (−14 LUFS, estilo YouTube).
- **Rango I–O**: exporta solo el rango marcado con `I`/`O`.
- Progreso en vivo sobre el botón y **Cancelar** que limpia el archivo a medias.

## Proyectos

- Formato **`.uep`** (JSON legible), **portable**: las rutas de los medios se guardan relativas al proyecto — mueve la carpeta entera a otro disco/máquina y ábrela.
- Medios que no aparecen quedan **offline** (en rojo) con botón **Relocalizar…**.
- **Autoguardado**: cada minuto (si hay cambios) se escribe una copia `.uep.autosave`; si la app muere, al arrancar ofrece **recuperar**. Guardar de verdad la invalida.
- Los cachés (audio conformado, proxies, waveforms, miniaturas) viven fuera del proyecto, indexados por hash del contenido: se regeneran solos en otra máquina.

---

## Servidor MCP (edición por agentes)

Al arrancar, la app levanta un servidor MCP en `http://127.0.0.1:4599/mcp` (solo loopback) **protegido con token** (se genera al arrancar; míralo en el pill «MCP» del header, que incluye el comando de conexión listo para copiar):

```bash
claude mcp add --transport http ubereditor http://127.0.0.1:4599/mcp \
  --header "Authorization: Bearer <token>"
```

14 herramientas: `get_project_summary`, `get_timeline`, `get_media_pool`, `get_effects_catalog`, `get_transcript`, `add_clip`, `split_clip`, `delete_clips`, `set_clip_transition`, `remove_silences` (con modo y parámetros), `move_range`, `generate_vertical`, `undo`, `redo`. Todo lo que hace un agente es deshacible desde la UI.

---

## Desarrollo

```bash
cargo test                    # toda la suite Rust (unitaria + píxeles sobre ffmpeg real)
cargo clippy --workspace --all-targets

npm run dev                   # UI en el navegador con motor MOCK (http://localhost:5175)
npm run typecheck

npx tauri dev                 # la app real

npm run screenshot            # pruebas visuales: 13 pasos con aserciones funcionales
                              #   → screenshots/<fecha>/*.png (arranca vite solo si hace falta)
```

### Arquitectura (crates)

| Crate | Qué hace |
|---|---|
| `ue-core` | Modelo puro, acciones con inversas mecánicas, historial transaccional, curvas de keyframes |
| `ue-media` | ffprobe, hashing, frames de preview (MJPEG), proxies, miniaturas, conformado de audio |
| `ue-audio` | WAV por mmap, mezclador puro (testeable), salida cpal (reloj maestro), picos |
| `ue-render` | Packs de efectos y generadores (manifest → cadena ffmpeg), transform y expresiones de animación |
| `ue-export` | EDL, grafo ffmpeg (multicapa, transiciones, texto/karaoke, avatar), progreso y cancelación |
| `ue-ai` | Detección de silencios (histéresis + padding), clasificación de emociones |
| `ue-whisper` | whisper-rs (Metal/CUDA), timestamps por palabra, descarga de modelos |
| `src-tauri` | Comandos IPC, FrameService, servidor MCP, autosave |

Convención de tiempo: microsegundos (`i64`) en todo el modelo; fps racional; ids ULID. El frontend espeja los tipos de serde a mano (`src/engine/types.ts`) y tiene dos motores intercambiables: `TauriEngine` (IPC real) y `MockEngine` (demo del navegador).
