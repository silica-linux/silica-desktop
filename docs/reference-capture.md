# Raccolta dei riferimenti su macOS 27

Procedura per le sessioni sul Mac che osservano macOS 27 e producono i riferimenti per la
parità. Vale per persone e agenti. Le regole clean-room sono in `AGENTS.md`: qui si
applicano.

## Dove vanno i riferimenti

Screenshot e note di macOS non entrano mai nel repo di Silica (`plan/` è gitignored).
Vivono nel repo privato `silica-linux/silica-reference`, clonato dentro
`plan/reference/`:

```bash
gh repo clone silica-linux/silica-reference plan/reference
```

Struttura:

```
plan/reference/
  setup.md              Mac, build di macOS, display, impostazioni usate
  <area>.md             misure e osservazioni di un'area (dock.md, glass.md, …)
  shots/<area>/*.png    screenshot citati dalle note
  video/<area>/*.mov    registrazioni per le animazioni
```

## Condizioni fisse

Le misure si confrontano con gli screenshot di Ossido: le condizioni devono essere
ripetibili e scritte in `setup.md`.

- Display: modello, risoluzione nativa, scala (es. 2×), refresh.
- Risoluzione "Predefinita" in Impostazioni › Display.
- Sfondo: colore pieno (Impostazioni › Sfondo › Colori), lo stesso grigio per tutte le
  sessioni; poi una ripresa con uno sfondo a strisce generato in codice, per vedere
  sfocatura e rifrazione.
- Aspetto chiaro e scuro, colore d'accento predefinito, "Riduci trasparenza" e "Riduci
  movimento" disattivati.
- Dock: dimensione e ingrandimento annotati (cursori in Impostazioni › Desktop e Dock).

## Cosa è consentito

- Usare la UI, fare screenshot (`screencapture -x`, `-R x,y,w,h`, `-l <windowid>`) e
  registrazioni dello schermo (`screencapture -v` o QuickTime).
- Misurare sui pixel degli screenshot con script propri (Python + Pillow).
- Leggere geometria e gerarchia degli elementi visibili con l'API di accessibilità (es.
  posizione e dimensione delle icone del Dock): è osservazione della UI in esecuzione.

## Cosa è vietato

- Leggere, copiare o disassemblare file del sistema: `/System`, `/Library`, bundle in
  `/Applications`, framework, risorse, plist di Apple, `defaults read` dei domini Apple.
- Estrarre icone, immagini, suoni, font o testi di Apple per usarli in Silica.
- Mettere screenshot di macOS nel repo di Silica o in issue e PR.

## Formato delle note

Ogni misura ha valore, unità, metodo e screenshot di origine. Dimensioni in punti
(pixel / scala), durate in ms, colori in sRGB esadecimale con alfa.

```markdown
## Dock · icone a riposo

- Lato icona: 48 pt (96 px a 2×), metodo: bordo alfa su shots/dock/rest-light.png
- Spaziatura fra icone: 8 pt
- macOS 27.0 (build …), 2026-10-13
```

Quando una cosa non si può misurare (es. la curva esatta di un'animazione), si scrive
cosa si è osservato e con quale incertezza, senza stimare numeri.

## Sessioni

In ordine di priorità, secondo la roadmap.

1. **Dock** (serve a M1.5): dimensioni e spaziature a riposo, ingrandimento massimo,
   scala di ogni icona in funzione della distanza dal puntatore (screenshot con il
   puntatore in punti noti, o accessibilità), raggio d'influenza, ritorno a riposo
   (video a 60 fps: durata e rimbalzo), etichetta (font, dimensioni, posizione), sfondo
   (raggio, colore, bordo, ombra), indicatori delle app aperte.
2. **Vetro** (M2): sfocatura, tinta, bordo e luci del materiale su sfondo pieno e a
   strisce, chiaro e scuro; rifrazione ai bordi.
3. **Finestre** (M2): raggi degli angoli, ombre attive e inattive, barra del titolo,
   pulsanti della finestra (dimensioni, spaziature, colori, stati), animazioni di
   apertura, chiusura e riduzione.
4. **Barra dei menu** (M3): altezza, spaziature, font, area di stato, menu aperti.
5. **Centro di controllo e notifiche** (M4), **Mission Control e spazi** (M5).
