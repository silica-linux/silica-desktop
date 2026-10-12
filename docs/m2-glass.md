# M2 · Vetro

Come Ossido disegna il vetro di `silica-glass-v1` (`docs/protocols.md`), da dove vengono
i parametri e quanto costa. Codice in `crates/ossido-render/src/glass.rs` e negli shader
`blur.frag` e `material.frag`.

## Come funziona

Per ogni superficie con regioni di vetro, a ogni frame in cui cambia quello che le sta
dietro:

1. gli elementi dietro la superficie (e il colore del desktop) si ridisegnano in una
   texture fuori schermo grande quanto le regioni più 3σ;
2. la texture si sfoca con una gaussiana separabile (a metà o a un quarto della
   risoluzione quando σ supera 12 o 24 pixel) e si mescola con la quota nitida misurata;
3. ogni regione si disegna con lo shader del materiale: rifrazione vicino al bordo
   (campionamento spostato lungo la normale), matrice di colore sui valori sRGB, bordo
   luminoso sopra e sotto e scuro ai lati, angoli ad arco di cerchio.

Se dietro non cambia niente (stessi elementi, stessi commit, stessa geometria) le
texture si riusano. Il vetro di una finestra non vede la sua barra né la sua ombra.

## Materiali (aspetto chiaro)

| Materiale | Da | σ | Quota nitida | Colore | Rifrazione |
| --- | --- | ---: | ---: | --- | --- |
| `clear` | fondo del Dock | 2,75 pt | 0,28 | matrice libera del Dock, b = (37; 39; 37) | ~5 pt ai lati a 6 pt dal bordo, ~1 sopra, ~3 sotto |
| `thick` | menu | 9 pt | 0 | matrice dei menu, b = (142; 140; 140) | ≤ 1,5 pt ai lati |

Valori e incertezze in `plan/reference/glass.md`, `dock.md`, `menubar.md`. L'aspetto
scuro non c'è ancora.

## Livelli di qualità

| Livello | Cosa vede il vetro | Quando |
| --- | --- | --- |
| `full` | tutto quello che sta dietro | predefinito con una GPU |
| `frosted` | solo lo sfondo del desktop (layer background e bottom): si ricalcola di rado | predefinito su un rasterizzatore software (llvmpipe) |
| `opaque` | niente: il materiale sopra il colore del desktop | a richiesta |

`--glass auto|full|frosted|opaque` all'avvio, `ui.glass_quality` a sessione aperta.

## Costo (2026-10-13)

`cargo xtask bench glass`: Ossido annidato in GNOME su Ubuntu 26.04, Iris Xe (Raptor
Lake-P), output 1280×800 a scala 1. Il Dock Flutter (Impeller, vetro chiaro) sta su un
layer top; dietro scorre una finestra a ogni frame, quindi lo sfondo del vetro cambia
sempre; il puntatore passa sulle icone. 5 s per ripetizione, mediana di 3.

| Vetro | Render di Ossido: media | p95 | p99 | max | Dock: frame | p99 | lenti |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| full | 1,88 ms | 2,84 | 3,24 | 3,4 | 298 | 20,0 ms | 1 |
| frosted | 1,58 ms | 2,30 | 3,02 | 3,2 | 299 | 21,5 ms | 1 |
| opaque | 1,51 ms | 2,17 | 2,99 | 3,2 | 299 | 20,7 ms | 1 |

Il vetro pieno con lo sfondo che cambia a ogni frame costa circa 0,4 ms in più per
frame su 16,7: il Dock resta a 60 fps. La misura è il tempo del rendering sulla CPU
(costruzione degli elementi, comandi GL, submit), non il tempo della GPU: per quello
servono le query di tempo di OpenGL, non ancora in Ossido.
