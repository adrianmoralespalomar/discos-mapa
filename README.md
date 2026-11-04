# Discos Mapa

Mapa visual de lo que ocupa cada carpeta de tus discos (al estilo de WinDirStat), con ayuda para decidir qué se puede borrar y qué no.

- **Mapa de parcelas (treemap)**: cada rectángulo es una carpeta o un fichero, de tamaño proporcional a lo que ocupa. Se puede colorear por tipo de fichero o por riesgo de borrarlo.
- **Liberar espacio**: lista de sitios que se pueden limpiar, con el porqué y, cuando existe, la forma oficial de hacerlo (`npm cache clean`, Archivos temporales de Windows, `DISM`…).
- **Más grandes**: los 200 ficheros que más ocupan, con filtro para los que llevan más de un año sin tocarse.
- **Tipos**: cuánto ocupa cada tipo de fichero y cada extensión.
- **Proyectos**: proyectos de código (Node, Rust, .NET, Gradle, Python, Maven) sin tocar desde hace 30, 90, 180 o 365 días, con sus carpetas regenerables (`node_modules`, `dist`, `target`, `bin/obj`…) y un botón para apartarlas. La actividad se mide sin contar esas carpetas.
- **Duplicados**: ficheros de más de 1 MB con exactamente el mismo contenido, con la opción **Dejar solo la más antigua**.
- **Cambios**: qué carpetas han crecido más de 50 MB desde el análisis anterior de la misma carpeta, y cuánto ocupaba en cada análisis.
- **Carpetas de red**: se puede analizar una ruta como `\\servidor\carpeta` (por ejemplo, un NAS). Ahí solo se mira: la cuarentena no funciona en red.
- **Cuarentena**: lo que marcas para limpiar se aparta a `X:\DiscosMapa-Cuarentena` en el mismo disco. Se puede restaurar en cualquier momento, y solo se borra de verdad cuando vacías un lote.

## Cómo funciona la limpieza

1. En **Liberar espacio** marca lo que quieras (o "Marcar todo lo seguro") y pulsa **Mover a la cuarentena**. En **Más grandes**, cada fichero tiene un botón **Apartar**.
2. Se confirma con la lista exacta y el tamaño. Lo que esté en uso o sin permiso se salta y se explica por qué.
3. En **Cuarentena** cada limpieza es un lote, con **Restaurar** (vuelve todo a su sitio) y **Borrar para siempre** (pide una segunda confirmación). Mientras está en la cuarentena sigue ocupando espacio.

Por qué una cuarentena y no la Papelera: mover dentro del mismo disco es instantáneo, aunque sea un `node_modules` con miles de ficheros. Además, la Papelera de Windows borra para siempre, sin avisar, lo que no le cabe.

Qué nunca se toca:

- Nada de nivel **No tocar** ni lo que haya dentro (el backend lo vuelve a comprobar aunque la interfaz lo pida).
- Discos extraíbles, de red (también las unidades de red con letra, aunque digan ser NTFS) o que no sean NTFS/ReFS (como la unidad de Google Drive), y la raíz de un disco.
- Carpetas de OneDrive y ficheros que están solo en la nube: apartarlos los borraría en todos tus equipos.
- Carpetas que Windows o un programa necesitan (como `Temp`): de esas solo se aparta lo de dentro.
- Lo que tiene una herramienta oficial (`Windows.old`, la Papelera, emuladores de Android, el disco de Docker): solo se indica la herramienta, y no se puede apartar nada de dentro, ni siquiera un fichero suelto.

## Descargar

En [Releases](https://github.com/adrianmoralespalomar/discos-mapa/releases) está el instalador de Windows (`*-setup.exe` o `.msi`).

El instalador no está firmado, así que Windows SmartScreen avisará la primera vez: pulsa **Más información** y luego **Ejecutar de todas formas**.

## Cómo se decide qué se puede borrar

Cada carpeta recibe uno de estos niveles (en `src-tauri/src/rules.rs`):

| Nivel | Significado | Ejemplos |
|---|---|---|
| Seguro | Se regenera solo o es basura | `%TEMP%`, `Windows\Temp`, descargas de Windows Update, cachés de miniaturas y de sombreadores, `.angular`, `__pycache__`, Papelera |
| Probable | Se puede regenerar, con algo de coste | `node_modules` y `dist` junto a un `package.json`, `target` de Rust, `bin/obj` de .NET, cachés de npm, Gradle, NuGet, cachés de navegadores |
| Revisar | Depende de ti | Descargas, imágenes de emulador Android, disco de Docker, `hiberfil.sys` |
| No tocar | Necesario para el sistema o irrecuperable | `Windows`, `Program Files`, `ProgramData`, `WinSxS`, `.git`, `.ssh`, `pagefile.sys` |

Reglas generales:

- Una carpeta sin regla propia hereda la de su antepasado más cercano.
- Las reglas de proyecto miran el contexto: `node_modules` solo se sugiere si al lado hay un `package.json`.
- Dentro de una zona protegida (como `Windows`) solo valen reglas explícitas de ruta, nunca las genéricas por nombre.

## Cómo cuenta el espacio

- Solo lee metadatos, nunca el contenido de los ficheros, y no necesita permisos de administrador. Las carpetas sin permiso se cuentan aparte.
- No entra en enlaces simbólicos ni uniones (*junctions*), para no contar dos veces ni entrar en bucles.
- Los ficheros de OneDrive que están solo en la nube cuentan 0 bytes.
- Pendiente: los enlaces duros (frecuentes en `WinSxS`) se cuentan una vez por enlace, así que esa carpeta aparece más grande de lo que es.
## Historial

- Cada análisis completo se guarda en `%APPDATA%\com.adrianmoralespalomar.discosmapa\historial`: la fecha y el tamaño total de los últimos 50 análisis de cada carpeta, y el tamaño de cada subcarpeta del último (solo las de más de 10 MB y hasta 6 niveles).
- Un análisis detenido a medias no se guarda.
- La cuarentena no cuenta como crecimiento.
- La pantalla inicial lista las carpetas analizadas antes, para repetir el análisis con un clic (útil para las carpetas de red).

## Cómo se buscan los duplicados

- Solo en tus carpetas: no entra en programas, `AppData`, carpetas que empiezan por punto, zonas con herramienta oficial ni carpetas regenerables o seguras de borrar (ahí los repetidos son normales).
- Primero agrupa por tamaño, luego compara el principio y el final de cada fichero y, solo si coinciden, el contenido entero (BLAKE3).
- Los enlaces duros al mismo fichero no cuentan como copias.
- Dentro de cada grupo, la primera es la más antigua.

## Desarrollo

Requisitos: Node 24, Rust estable y Visual Studio Build Tools con C++ (lo pide Tauri).

```bash
npm install
npm run tauri dev
```

Instalable local (queda en `src-tauri/target/release/bundle/`):

```bash
npm run tauri build
```

Escaneo desde la terminal, sin ventana (útil para probar reglas):

```bash
cd src-tauri && cargo run --release --example escanear -- "C:\Users"
```

## Publicar una versión

1. Sube el número de versión en `package.json`, `src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json`.
2. Crea y sube la etiqueta: `git tag v0.3.0 && git push origin v0.3.0`.
3. La acción de GitHub `Publicar versión` compila el instalador y lo publica en Releases.

## Estructura

- `src-tauri/src/scanner.rs`: recorrido paralelo del disco.
- `src-tauri/src/rules.rs`: reglas de lo que se puede borrar.
- `src-tauri/src/tree.rs`: árbol en memoria y vistas reducidas que se envían a la interfaz (nunca el árbol entero).
- `src-tauri/src/quarantine.rs`: apartar, restaurar y vaciar la cuarentena.
- `src-tauri/src/projects.rs`: proyectos de código y sus carpetas regenerables.
- `src-tauri/src/duplicates.rs`: búsqueda de ficheros repetidos.
- `src-tauri/src/history.rs`: historial de análisis y comparación con el anterior.
- `src/app`: interfaz en Angular. El mapa se dibuja en un `canvas` con `d3-hierarchy`.

## Próximas fases

- Alertas cuando un disco se llena o una carpeta crece de golpe.
- Contar una sola vez los enlaces duros.
