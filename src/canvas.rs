use gtk::gdk;
use gtk::glib;
use gtk::graphene;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};

type ZoomCallback = Box<dyn Fn(f64)>;
type ScrollCallback = Box<dyn Fn(f64) -> bool>;
type FrameCallback = Box<dyn Fn(f64) -> bool>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Canvas {
        pub texture: RefCell<Option<gdk::Texture>>,
        pub previous: RefCell<Option<gdk::Texture>>,
        pub previous_scale: Cell<f64>,
        pub previous_rotation: Cell<f64>,
        pub slide: Cell<f64>,
        pub slide_dir: Cell<f64>,
        pub zoom: Cell<f64>,
        pub fit: Cell<bool>,
        pub rotation: Cell<f64>,
        pub offset: Cell<(f64, f64)>,
        pub drag_origin: Cell<(f64, f64)>,
        pub last_percent: Cell<i32>,
        pub on_zoom: RefCell<Option<ZoomCallback>>,
        pub on_scroll: RefCell<Option<ScrollCallback>>,
        pub on_frame: RefCell<Option<FrameCallback>>,
        pub ticking: Cell<bool>,
        pub last_tick: Cell<Option<i64>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Canvas {
        const NAME: &'static str = "VisorCanvas";
        type Type = super::Canvas;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Canvas {
        fn constructed(&self) {
            self.parent_constructed();
            self.fit.set(true);
            self.zoom.set(1.0);
            self.slide.set(1.0);
            self.slide_dir.set(1.0);

            let canvas = self.obj();
            canvas.set_overflow(gtk::Overflow::Hidden);

            let drag = gtk::GestureDrag::new();
            {
                let weak = canvas.downgrade();
                drag.connect_drag_begin(move |_, _, _| {
                    if let Some(canvas) = weak.upgrade() {
                        let offset = canvas.imp().offset.get();
                        canvas.imp().drag_origin.set(offset);
                    }
                });
            }
            {
                let weak = canvas.downgrade();
                drag.connect_drag_update(move |_, dx, dy| {
                    if let Some(canvas) = weak.upgrade() {
                        let (x, y) = canvas.imp().drag_origin.get();
                        canvas.imp().offset.set((x + dx, y + dy));
                        canvas.queue_draw();
                    }
                });
            }
            canvas.add_controller(drag);

            let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
            {
                let weak = canvas.downgrade();
                scroll.connect_scroll(move |_, _dx, dy| {
                    if let Some(canvas) = weak.upgrade() {
                        let consumed = canvas
                            .imp()
                            .on_scroll
                            .borrow()
                            .as_ref()
                            .map(|callback| callback(dy))
                            .unwrap_or(false);
                        if !consumed {
                            canvas.zoom_by(if dy < 0.0 { 1.15 } else { 1.0 / 1.15 });
                        }
                    }
                    glib::Propagation::Stop
                });
            }
            canvas.add_controller(scroll);
        }
    }

    impl WidgetImpl for Canvas {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.obj().emit_zoom();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let width = gtk::prelude::WidgetExt::width(&*self.obj()) as f32;
            let height = gtk::prelude::WidgetExt::height(&*self.obj()) as f32;
            let background = gdk::RGBA::new(0.086, 0.086, 0.094, 1.0);
            snapshot.append_color(&background, &graphene::Rect::new(0.0, 0.0, width, height));

            let texture = self.texture.borrow().clone();
            let Some(current) = texture else {
                return;
            };

            let rotation = self.rotation.get();
            let image_width = current.width() as f64;
            let image_height = current.height() as f64;
            let sideways = (rotation as i32 / 90) % 2 == 1;
            let (box_width, box_height) = if sideways {
                (image_height, image_width)
            } else {
                (image_width, image_height)
            };

            let fitted = (width as f64 / box_width)
                .min(height as f64 / box_height)
                .min(1.0);
            let scale = if self.fit.get() {
                fitted
            } else {
                self.zoom.get()
            };
            if scale <= 0.0 {
                return;
            }

            let (offset_x, offset_y) = self.offset.get();
            let center_x = width as f64 / 2.0 + offset_x;
            let center_y = height as f64 / 2.0 + offset_y;

            let progress = self.slide.get().clamp(0.0, 1.0);
            let direction = self.slide_dir.get();

            if progress < 1.0
                && let Some(previous) = self.previous.borrow().clone()
            {
                let shift = -direction * width as f64 * progress;
                paint(
                    snapshot,
                    &previous,
                    self.previous_scale.get(),
                    self.previous_rotation.get(),
                    center_x + shift,
                    center_y,
                );
            }

            let shift = direction * width as f64 * (1.0 - progress);
            paint(
                snapshot,
                &current,
                scale,
                rotation,
                center_x + shift,
                center_y,
            );
        }
    }
}

fn paint(
    snapshot: &gtk::Snapshot,
    texture: &gdk::Texture,
    scale: f64,
    rotation: f64,
    center_x: f64,
    center_y: f64,
) {
    let image_width = texture.width() as f32;
    let image_height = texture.height() as f32;
    snapshot.save();
    snapshot.translate(&graphene::Point::new(center_x as f32, center_y as f32));
    snapshot.rotate(rotation as f32);
    snapshot.scale(scale as f32, scale as f32);
    snapshot.append_texture(
        texture,
        &graphene::Rect::new(
            -image_width / 2.0,
            -image_height / 2.0,
            image_width,
            image_height,
        ),
    );
    snapshot.restore();
}

glib::wrapper! {
    pub struct Canvas(ObjectSubclass<imp::Canvas>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn has_image(&self) -> bool {
        self.imp().texture.borrow().is_some()
    }

    pub fn on_zoom_changed(&self, callback: impl Fn(f64) + 'static) {
        *self.imp().on_zoom.borrow_mut() = Some(Box::new(callback));
        self.emit_zoom();
    }

    pub fn set_image(&self, texture: Option<&gdk::Texture>) {
        let imp = self.imp();
        if let Some(texture) = texture {
            let old = imp.texture.borrow().clone();
            if old.is_some() {
                imp.previous_scale.set(self.effective_zoom());
                imp.previous_rotation.set(imp.rotation.get());
            }
            imp.previous.replace(old);
            imp.texture.replace(Some(texture.clone()));
        } else {
            imp.previous.replace(None);
            imp.texture.replace(None);
        }
        imp.slide.set(1.0);
        imp.rotation.set(0.0);
        imp.offset.set((0.0, 0.0));
        imp.fit.set(true);
        self.queue_draw();
        self.emit_zoom();
    }

    /// Registra el callback consultivo de scroll. Devolver `true` consume el evento.
    pub fn set_on_scroll(&self, callback: impl Fn(f64) -> bool + 'static) {
        *self.imp().on_scroll.borrow_mut() = Some(Box::new(callback));
    }

    /// Registra el callback que se ejecuta en cada frame de animación.
    pub fn set_on_frame(&self, callback: impl Fn(f64) -> bool + 'static) {
        *self.imp().on_frame.borrow_mut() = Some(Box::new(callback));
    }

    /// Arranca el reloj de animación si no está ya en marcha.
    pub fn request_frames(&self) {
        let imp = self.imp();
        if imp.ticking.replace(true) {
            return;
        }
        imp.last_tick.set(None);

        let canvas = self.clone();
        self.add_tick_callback(move |_, clock| {
            let imp = canvas.imp();
            let now = clock.frame_time();
            let dt = match imp.last_tick.get() {
                Some(last) => ((now - last) as f64 / 1_000_000.0).clamp(0.001, 0.05),
                None => 1.0 / 60.0,
            };
            imp.last_tick.set(Some(now));

            let keep = imp
                .on_frame
                .borrow()
                .as_ref()
                .map(|callback| callback(dt))
                .unwrap_or(false);
            if keep {
                glib::ControlFlow::Continue
            } else {
                imp.ticking.set(false);
                glib::ControlFlow::Break
            }
        });
    }

    /// Progreso del carrusel: 0 muestra la imagen anterior, 1 la actual.
    pub fn set_slide(&self, progress: f64) {
        let imp = self.imp();
        let progress = progress.clamp(0.0, 1.0);
        imp.slide.set(progress);
        if progress >= 1.0 {
            imp.previous.replace(None);
        }
        self.queue_draw();
    }

    pub fn set_slide_direction(&self, direction: i32) {
        self.imp()
            .slide_dir
            .set(if direction < 0 { -1.0 } else { 1.0 });
    }

    pub fn zoom_level(&self) -> f64 {
        self.effective_zoom()
    }

    pub fn offset(&self) -> (f64, f64) {
        self.imp().offset.get()
    }

    pub fn set_offset(&self, x: f64, y: f64) {
        self.imp().offset.set((x, y));
        self.queue_draw();
    }

    pub fn set_zoom_from_plugin(&self, zoom: f64) {
        if !self.has_image() {
            return;
        }
        let imp = self.imp();
        let current = self.effective_zoom().max(0.01);
        let target = zoom.clamp(0.02, 32.0);
        let ratio = target / current;
        let (x, y) = imp.offset.get();
        imp.offset.set((x * ratio, y * ratio));
        imp.zoom.set(target);
        imp.fit.set(false);
        self.queue_draw();
        self.emit_zoom();
    }

    pub fn zoom_in(&self) {
        self.zoom_by(1.25);
    }

    pub fn zoom_out(&self) {
        self.zoom_by(0.8);
    }

    pub fn zoom_by(&self, factor: f64) {
        if !self.has_image() {
            return;
        }
        let imp = self.imp();
        let current = self.effective_zoom().max(0.01);
        let target = (current * factor).clamp(0.02, 32.0);
        let ratio = target / current;
        let (x, y) = imp.offset.get();
        imp.offset.set((x * ratio, y * ratio));
        imp.zoom.set(target);
        imp.fit.set(false);
        self.queue_draw();
        self.emit_zoom();
    }

    pub fn fit_to_window(&self) {
        let imp = self.imp();
        imp.fit.set(true);
        imp.offset.set((0.0, 0.0));
        self.queue_draw();
        self.emit_zoom();
    }

    pub fn actual_size(&self) {
        let imp = self.imp();
        imp.fit.set(false);
        imp.zoom.set(1.0);
        imp.offset.set((0.0, 0.0));
        self.queue_draw();
        self.emit_zoom();
    }

    pub fn rotate(&self, degrees: f64) {
        let imp = self.imp();
        let rotation = (imp.rotation.get() + degrees).rem_euclid(360.0);
        imp.rotation.set(rotation);
        self.queue_draw();
        self.emit_zoom();
    }

    fn effective_zoom(&self) -> f64 {
        let imp = self.imp();
        if !imp.fit.get() {
            return imp.zoom.get();
        }

        let texture = imp.texture.borrow().clone();
        let Some(texture) = texture else {
            return 1.0;
        };

        let width = gtk::prelude::WidgetExt::width(self) as f64;
        let height = gtk::prelude::WidgetExt::height(self) as f64;
        if width < 2.0 || height < 2.0 {
            return 1.0;
        }

        let (image_width, image_height) = (texture.width() as f64, texture.height() as f64);
        let sideways = (imp.rotation.get() as i32 / 90) % 2 == 1;
        let (box_width, box_height) = if sideways {
            (image_height, image_width)
        } else {
            (image_width, image_height)
        };

        (width / box_width).min(height / box_height).min(1.0)
    }

    fn emit_zoom(&self) {
        let zoom = self.effective_zoom();
        let percent = (zoom * 100.0).round().clamp(1.0, 99_999.0) as i32;
        let imp = self.imp();
        if imp.last_percent.get() == percent {
            return;
        }
        imp.last_percent.set(percent);
        if let Some(callback) = imp.on_zoom.borrow().as_ref() {
            callback(zoom);
        }
    }
}
