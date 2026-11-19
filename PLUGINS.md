# Plugins de Visor

Visor mantiene su núcleo mínimo: las funciones extra viven en plugins que se
activan solo si las necesitas. Un plugin es una carpeta en un repositorio de
GitHub con dos archivos: `plugin.json` y `main.lua`. No hay que compilar nada ni
publicar binarios por plataforma.

Los plugins se ejecutan en **Lua 5.4 embebido** con un sandbox estricto: sin
acceso a archivos, red, procesos ni al sistema. Aun así, ejecutan código:
instala solo plugins de fuentes confiables.

## Tienda

- `Ctrl+P` o el botón de pieza de puzzle en la cabecera abre la tienda.
- Los plugins oficiales vienen incluidos en el binario y se instalan con un clic.
- Para añadir uno de terceros, pega la URL de su repositorio:
  - `https://github.com/usuario/repositorio`
  - `usuario/repositorio`
  - `https://github.com/usuario/repositorio/tree/rama/carpeta`

La URL debe apuntar a una carpeta que contenga `plugin.json` y el archivo Lua de
entrada. Si se indica solo el repositorio se usa su rama por defecto.

Los plugins se guardan en `~/.local/share/visor/plugins/<id>/` y la lista de
activos en `~/.config/visor/plugins.json`.

## Estructura

```
mi-plugin/
├── plugin.json
└── main.lua
```

`plugin.json`:

```json
{
  "id": "mi-plugin",
  "name": "Mi plugin",
  "version": "1.0.0",
  "author": "tu-usuario",
  "description": "Qué hace, en una frase.",
  "api": 1,
  "entry": "main.lua"
}
```

- `id`: solo letras, números, `-`, `_` y `.`; identifica la carpeta instalada.
- `api`: debe ser `1` para esta versión de Visor.
- `entry`: nombre del archivo `.lua` de entrada.

## API (v1)

El script declara funciones globales. Si un evento devuelve `true`, el plugin
consume el evento y el visor no aplica su comportamiento por defecto.

```lua
function on_scroll(dy) end      -- rueda: dy < 0 acerca, dy > 0 aleja
function on_navigate(dir) end   -- dir = 1 (siguiente) | -1 (anterior)
function on_frame(dt) end       -- animación; devolver true para seguir
```

Acciones disponibles en `visor`:

| Función | Descripción |
| --- | --- |
| `visor.zoom()` | Nivel de zoom actual. |
| `visor.set_zoom(z)` | Fija el zoom (0.02–32). |
| `visor.offset()` | Desplazamiento actual `(x, y)`. |
| `visor.set_offset(x, y)` | Mueve la imagen. |
| `visor.slide(p)` | Progreso del carrusel: `0` imagen anterior, `1` actual. |
| `visor.next()` / `visor.prev()` | Cambia de imagen. |
| `visor.request_frame()` | Pide empezar a recibir `on_frame`. |
| `visor.log(mensaje)` | Escribe en la salida de error / consola. |

Los frames se detienen solos: `on_frame` debe devolver `true` mientras queden
animaciones. `visor.request_frame()` es necesario para arrancar el reloj desde
un evento como `on_scroll`.

## Límites del sandbox

- Librerías disponibles: `table`, `string`, `math`, `utf8` y las funciones
  básicas de Lua. Sin `io`, `os`, `require`, `dofile`, `load`, `debug` ni
  `package`.
- Memoria máxima: 8 MB por plugin.
- Tiempo máximo: 100 ms por callback (500 ms al cargar).

## Ejemplo: zoom suave

```lua
local target = 1.0
local active = false

function on_scroll(dy)
    if dy == 0 then return false end
    if not active then target = visor.zoom() end
    target = target * (dy < 0 and 1.18 or 1 / 1.18)
    target = math.max(0.02, math.min(32.0, target))
    active = true
    visor.request_frame()
    return true
end

function on_frame(dt)
    if not active then return false end
    local current = visor.zoom()
    local next_zoom = current + (target - current) * (1 - math.exp(-dt * 14))
    if math.abs(target - current) < 0.001 then
        visor.set_zoom(target)
        active = false
        return false
    end
    visor.set_zoom(next_zoom)
    return true
end
```

El código completo de los dos plugins oficiales está en
[`plugins/`](plugins/) de este repositorio.

## Publicar un plugin

1. Crea un repositorio público con `plugin.json` y `main.lua` en la raíz (o en
   una subcarpeta).
2. Comprueba que `api` sea `1`.
3. Comparte la URL. Cualquiera puede instalarlo pegándola en la tienda.

No hace falta registro ni revisión previa. Si quieres proponer un plugin para el
índice oficial, abre una *issue* o un *pull request*.
