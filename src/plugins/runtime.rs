use crate::canvas::Canvas;
use crate::plugins::manifest::Manifest;
use crate::plugins::store::{self, Installed};
use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Value, VmState};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Tope de memoria por plugin (incluye el propio estado de Lua).
const MEMORY_LIMIT: usize = 8 * 1024 * 1024;
/// Tiempo máximo que puede durar un callback.
const CALLBACK_BUDGET: Duration = Duration::from_millis(100);
/// Tiempo máximo para cargar el script al activar el plugin.
const LOAD_BUDGET: Duration = Duration::from_millis(500);

type NavigateCallback = Rc<dyn Fn(i32)>;

/// Acciones que un plugin puede pedir al visor.
///
/// Se comparte entre todos los plugins activos; `navigate` se rellena cuando la
/// ventana ya existe (un plugin solo puede navegar sobre una ventana montada).
pub struct Host {
    pub canvas: Canvas,
    navigate: RefCell<Option<NavigateCallback>>,
}

impl Host {
    fn new(canvas: Canvas) -> Rc<Self> {
        Rc::new(Self {
            canvas,
            navigate: RefCell::new(None),
        })
    }

    fn navigate(&self, delta: i32) {
        let callback = self.navigate.borrow().clone();
        if let Some(callback) = callback {
            callback(delta);
        }
    }
}

struct Plugin {
    name: String,
    /// Los handles lua se liberan antes que la VM (orden de declaración).
    on_scroll: Option<Function>,
    on_navigate: Option<Function>,
    on_frame: Option<Function>,
    deadline: Rc<Cell<Instant>>,
    _lua: Lua,
}

impl Plugin {
    fn load(installed: &Installed, host: Rc<Host>) -> Result<Self, String> {
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            LuaOptions::default(),
        )
        .map_err(|err| format!("no se pudo iniciar Lua: {err}"))?;

        lua.set_memory_limit(MEMORY_LIMIT)
            .map_err(|err| format!("no se pudo limitar la memoria: {err}"))?;

        let deadline = Rc::new(Cell::new(Instant::now() + LOAD_BUDGET));
        let hook_deadline = deadline.clone();
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(10_000),
            move |_, _: &mlua::debug::Debug| {
                if Instant::now() > hook_deadline.get() {
                    Err(mlua::Error::RuntimeError(
                        "tiempo de ejecución excedido".into(),
                    ))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )
        .map_err(|err| format!("no se pudo instalar el límite de tiempo: {err}"))?;

        install_api(&lua, host, &installed.manifest.id)
            .map_err(|err| format!("API no disponible: {err}"))?;
        remove_dangerous_globals(&lua).map_err(|err| format!("sandbox: {err}"))?;

        lua.load(&installed.script)
            .set_name(&installed.manifest.id)
            .exec()
            .map_err(|err| format!("error al cargar {}: {err}", installed.manifest.name))?;

        let globals = lua.globals();
        let grab = |name: &str| -> Option<Function> {
            match globals.get::<Value>(name) {
                Ok(Value::Function(function)) => Some(function),
                _ => None,
            }
        };

        Ok(Self {
            name: installed.manifest.id.clone(),
            on_scroll: grab("on_scroll"),
            on_navigate: grab("on_navigate"),
            on_frame: grab("on_frame"),
            deadline,
            _lua: lua,
        })
    }

    fn begin(&self, budget: Duration) {
        self.deadline.set(Instant::now() + budget);
    }

    fn scroll(&self, dy: f64) -> bool {
        let Some(function) = &self.on_scroll else {
            return false;
        };
        self.begin(CALLBACK_BUDGET);
        function.call::<bool>(dy).unwrap_or_else(|err| {
            self.report("on_scroll", err);
            false
        })
    }

    fn navigate(&self, dir: i32) -> bool {
        let Some(function) = &self.on_navigate else {
            return false;
        };
        self.begin(CALLBACK_BUDGET);
        function.call::<bool>(dir).unwrap_or_else(|err| {
            self.report("on_navigate", err);
            false
        })
    }

    fn frame(&self, dt: f64) -> bool {
        let Some(function) = &self.on_frame else {
            return false;
        };
        self.begin(CALLBACK_BUDGET);
        function.call::<bool>(dt).unwrap_or_else(|err| {
            self.report("on_frame", err);
            false
        })
    }

    fn report(&self, event: &str, err: mlua::Error) {
        eprintln!("[visor:{}] {event}: {err}", self.name);
    }
}

fn remove_dangerous_globals(lua: &Lua) -> mlua::Result<()> {
    let globals = lua.globals();
    for name in [
        "dofile",
        "loadfile",
        "load",
        "require",
        "package",
        "collectgarbage",
        "debug",
        "print",
        "coroutine",
    ] {
        globals.raw_set(name, Value::Nil)?;
    }
    Ok(())
}

fn install_api(lua: &Lua, host: Rc<Host>, id: &str) -> mlua::Result<()> {
    let visor = lua.create_table()?;

    {
        let host = host.clone();
        visor.set(
            "zoom",
            lua.create_function(move |_, ()| Ok(host.canvas.zoom_level()))?,
        )?;
    }
    {
        let host = host.clone();
        visor.set(
            "set_zoom",
            lua.create_function(move |_, zoom: f64| {
                host.canvas.set_zoom_from_plugin(zoom);
                Ok(())
            })?,
        )?;
    }
    {
        let host = host.clone();
        visor.set(
            "offset",
            lua.create_function(move |_, ()| {
                let (x, y) = host.canvas.offset();
                Ok((x, y))
            })?,
        )?;
    }
    {
        let host = host.clone();
        visor.set(
            "set_offset",
            lua.create_function(move |_, (x, y): (f64, f64)| {
                host.canvas.set_offset(x, y);
                Ok(())
            })?,
        )?;
    }
    {
        let host = host.clone();
        visor.set(
            "slide",
            lua.create_function(move |_, progress: f64| {
                host.canvas.set_slide(progress);
                Ok(())
            })?,
        )?;
    }
    {
        let host = host.clone();
        visor.set(
            "next",
            lua.create_function(move |_, ()| {
                host.navigate(1);
                Ok(())
            })?,
        )?;
    }
    {
        let host = host.clone();
        visor.set(
            "prev",
            lua.create_function(move |_, ()| {
                host.navigate(-1);
                Ok(())
            })?,
        )?;
    }
    {
        let host = host.clone();
        visor.set(
            "request_frame",
            lua.create_function(move |_, ()| {
                host.canvas.request_frames();
                Ok(())
            })?,
        )?;
    }
    {
        let id = id.to_string();
        visor.set(
            "log",
            lua.create_function(move |_, message: String| {
                eprintln!("[visor:{id}] {message}");
                Ok(())
            })?,
        )?;
    }

    lua.globals().set("visor", visor)?;
    Ok(())
}

pub struct PluginManager {
    host: Rc<Host>,
    plugins: RefCell<Vec<Rc<Plugin>>>,
    enabled: RefCell<Vec<String>>,
}

impl PluginManager {
    pub fn new(canvas: Canvas) -> Self {
        let manager = Self {
            host: Host::new(canvas),
            plugins: RefCell::new(Vec::new()),
            enabled: RefCell::new(Vec::new()),
        };
        manager.reload();
        manager
    }

    /// Conecta la navegación del host con la ventana.
    pub fn set_navigate(&self, callback: impl Fn(i32) + 'static) {
        *self.host.navigate.borrow_mut() = Some(Rc::new(callback));
    }

    /// Relee los plugins activos del disco y reinicia sus VM.
    pub fn reload(&self) {
        let enabled = store::load_enabled();
        let mut plugins = Vec::new();

        for id in &enabled {
            let Some(installed) = store::find(id) else {
                continue;
            };
            match Plugin::load(&installed, self.host.clone()) {
                Ok(plugin) => plugins.push(Rc::new(plugin)),
                Err(error) => eprintln!("[visor] {error}"),
            }
        }

        self.enabled.replace(enabled);
        self.plugins.replace(plugins);
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.borrow().iter().any(|item| item == id)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        {
            let mut list = self.enabled.borrow_mut();
            list.retain(|item| item != id);
            if enabled {
                list.push(id.to_string());
            }
        }
        let list = self.enabled.borrow().clone();
        store::save_enabled(&list)?;
        self.reload();
        Ok(())
    }

    /// Ejecuta `on_scroll`; devuelve `true` si algún plugin consume el evento.
    pub fn on_scroll(&self, dy: f64) -> bool {
        for plugin in self.plugins.borrow().clone() {
            if plugin.scroll(dy) {
                return true;
            }
        }
        false
    }

    /// Ejecuta `on_navigate`; devuelve `true` si algún plugin controla el cambio.
    pub fn on_navigate(&self, dir: i32) -> bool {
        for plugin in self.plugins.borrow().clone() {
            if plugin.navigate(dir) {
                return true;
            }
        }
        false
    }

    /// Ejecuta `on_frame`; devuelve `true` mientras quede animación.
    pub fn on_frame(&self, dt: f64) -> bool {
        let mut keep = false;
        for plugin in self.plugins.borrow().clone() {
            keep |= plugin.frame(dt);
        }
        keep
    }
}

/// Manifiesto de un plugin oficial embebido en el binario.
pub struct Official {
    pub manifest: Manifest,
    pub script: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Table;

    fn lua_sandbox() -> Lua {
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            LuaOptions::default(),
        )
        .unwrap();
        remove_dangerous_globals(&lua).unwrap();
        lua
    }

    #[test]
    fn elimina_globals_peligrosos() {
        let lua = lua_sandbox();
        let globals = lua.globals();
        for name in ["io", "os", "require", "dofile", "load", "loadfile", "debug"] {
            let value: Value = globals.get(name).unwrap();
            assert!(matches!(value, Value::Nil), "{name} sigue disponible");
        }
        let safe: Value = globals.get("math").unwrap();
        assert!(matches!(safe, Value::Table(_)));
    }

    #[test]
    fn corta_bucles_infinitos() {
        let lua = lua_sandbox();
        let deadline = Rc::new(Cell::new(Instant::now() + Duration::from_millis(20)));
        let hook = deadline.clone();
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(1_000),
            move |_, _: &mlua::debug::Debug| {
                if Instant::now() > hook.get() {
                    Err(mlua::Error::RuntimeError("tiempo".into()))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )
        .unwrap();

        let result: mlua::Result<()> = lua.load("while true do end").exec();
        assert!(result.is_err());
    }

    #[test]
    fn api_visor_expone_acciones() {
        let lua = lua_sandbox();
        let visor: Table = lua.create_table().unwrap();
        visor
            .set("zoom", lua.create_function(|_, ()| Ok(1.0)).unwrap())
            .unwrap();
        lua.globals().set("visor", visor).unwrap();

        let zoom: f64 = lua.load("return visor.zoom()").eval().unwrap();
        assert_eq!(zoom, 1.0);
    }

    fn texture() -> gtk::gdk::Texture {
        use gtk::prelude::Cast;
        let bytes = gtk::glib::Bytes::from_static(&[255, 0, 0, 255]);
        gtk::gdk::MemoryTexture::new(1, 1, gtk::gdk::MemoryFormat::R8g8b8a8, &bytes, 4).upcast()
    }

    fn official(id: &str) -> Installed {
        let (manifest, script) = match id {
            "smooth-zoom" => (
                include_str!("../../plugins/smooth-zoom/plugin.json"),
                include_str!("../../plugins/smooth-zoom/main.lua"),
            ),
            _ => (
                include_str!("../../plugins/carousel/plugin.json"),
                include_str!("../../plugins/carousel/main.lua"),
            ),
        };
        Installed {
            manifest: Manifest::parse(manifest).unwrap(),
            script: script.to_string(),
        }
    }

    #[test]
    fn plugins_oficiales_funcionan() {
        if gtk::init().is_err() {
            return;
        }

        let canvas = Canvas::new();
        canvas.set_image(Some(&texture()));
        let plugin = Plugin::load(&official("smooth-zoom"), Host::new(canvas.clone())).unwrap();

        assert!(!plugin.frame(0.016));
        assert!(plugin.scroll(-1.0));
        for _ in 0..120 {
            plugin.frame(1.0 / 60.0);
        }
        assert!(
            canvas.zoom_level() > 1.1,
            "el zoom suave no avanzó: {}",
            canvas.zoom_level()
        );

        let canvas = Canvas::new();
        let host = Host::new(canvas);
        let navigated = Rc::new(Cell::new(0));
        {
            let navigated = navigated.clone();
            *host.navigate.borrow_mut() = Some(Rc::new(move |dir| navigated.set(dir)));
        }
        let plugin = Plugin::load(&official("carousel"), host).unwrap();

        assert!(plugin.navigate(1));
        assert_eq!(navigated.get(), 1);
        assert!(plugin.frame(0.016));
        for _ in 0..120 {
            plugin.frame(1.0 / 60.0);
        }
        assert!(!plugin.frame(0.016));
    }
}
