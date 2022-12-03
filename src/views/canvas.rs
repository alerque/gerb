/*
 * gerb
 *
 * Copyright 2022 - Manos Pitsidianakis
 *
 * This file is part of gerb.
 *
 * gerb is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * gerb is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with gerb. If not, see <http://www.gnu.org/licenses/>.
 */

mod transformation;
use crate::utils::Point;
pub use transformation::*;

use glib::{ParamFlags, ParamSpec, ParamSpecBoolean, ParamSpecDouble, ParamSpecObject, Value};

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::Cell;

const RULER_BREADTH: f64 = 13.;
#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct UnitPoint(pub Point);

#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct ViewPoint(pub Point);

#[derive(Debug, Default)]
pub struct CanvasInner {
    pub show_grid: Cell<bool>,
    pub show_guidelines: Cell<bool>,
    pub show_handles: Cell<bool>,
    pub inner_fill: Cell<bool>,
    pub transformation: Transformation,
    pub show_total_area: Cell<bool>,
    pub warp_cursor: Cell<bool>,
    view_height: Cell<f64>,
    view_width: Cell<f64>,
    mouse: Cell<ViewPoint>,
}

#[glib::object_subclass]
impl ObjectSubclass for CanvasInner {
    const NAME: &'static str = "CanvasInner";
    type Type = Canvas;
    type ParentType = gtk::DrawingArea;
}

impl ObjectImpl for CanvasInner {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        self.show_grid.set(true);
        self.show_guidelines.set(true);
        self.show_handles.set(true);
        self.inner_fill.set(false);
        self.show_total_area.set(true);
        obj.set_tooltip_text(None);
        obj.set_visible(true);
        obj.set_expand(true);
        obj.set_events(
            gtk::gdk::EventMask::BUTTON_PRESS_MASK
                | gtk::gdk::EventMask::BUTTON_RELEASE_MASK
                | gtk::gdk::EventMask::BUTTON_MOTION_MASK
                | gtk::gdk::EventMask::SCROLL_MASK
                | gtk::gdk::EventMask::SMOOTH_SCROLL_MASK
                | gtk::gdk::EventMask::POINTER_MOTION_MASK,
        );
        obj.connect_size_allocate(|self_, _rect| {
            self_.set_property::<f64>(Canvas::VIEW_HEIGHT, self_.allocated_height() as f64);
            self_.set_property::<f64>(Canvas::VIEW_WIDTH, self_.allocated_width() as f64);
        });
    }

    fn properties() -> &'static [ParamSpec] {
        static PROPERTIES: once_cell::sync::Lazy<Vec<ParamSpec>> =
            once_cell::sync::Lazy::new(|| {
                vec![
                    ParamSpecBoolean::new(
                        Canvas::SHOW_GRID,
                        Canvas::SHOW_GRID,
                        Canvas::SHOW_GRID,
                        true,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new(
                        Canvas::SHOW_GUIDELINES,
                        Canvas::SHOW_GUIDELINES,
                        Canvas::SHOW_GUIDELINES,
                        false,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new(
                        Canvas::SHOW_HANDLES,
                        Canvas::SHOW_HANDLES,
                        Canvas::SHOW_HANDLES,
                        false,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new(
                        Canvas::INNER_FILL,
                        Canvas::INNER_FILL,
                        Canvas::INNER_FILL,
                        true,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecObject::new(
                        Canvas::TRANSFORMATION,
                        Canvas::TRANSFORMATION,
                        Canvas::TRANSFORMATION,
                        Transformation::static_type(),
                        ParamFlags::READABLE,
                    ),
                    ParamSpecBoolean::new(
                        Canvas::SHOW_TOTAL_AREA,
                        Canvas::SHOW_TOTAL_AREA,
                        Canvas::SHOW_TOTAL_AREA,
                        false,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new(
                        Canvas::WARP_CURSOR,
                        Canvas::WARP_CURSOR,
                        Canvas::WARP_CURSOR,
                        true,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecDouble::new(
                        Canvas::VIEW_HEIGHT,
                        Canvas::VIEW_HEIGHT,
                        Canvas::VIEW_HEIGHT,
                        std::f64::MIN,
                        std::f64::MAX,
                        1000.0,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecDouble::new(
                        Canvas::VIEW_WIDTH,
                        Canvas::VIEW_WIDTH,
                        Canvas::VIEW_WIDTH,
                        std::f64::MIN,
                        std::f64::MAX,
                        1000.0,
                        ParamFlags::READWRITE,
                    ),
                ]
            });
        PROPERTIES.as_ref()
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> glib::Value {
        match pspec.name() {
            Canvas::SHOW_GRID => self.show_grid.get().to_value(),
            Canvas::SHOW_GUIDELINES => self.show_guidelines.get().to_value(),
            Canvas::SHOW_HANDLES => self.show_handles.get().to_value(),
            Canvas::INNER_FILL => self.inner_fill.get().to_value(),
            Canvas::TRANSFORMATION => self.transformation.to_value(),
            Canvas::SHOW_TOTAL_AREA => self.show_total_area.get().to_value(),
            Canvas::WARP_CURSOR => self.warp_cursor.get().to_value(),
            Canvas::VIEW_HEIGHT => (self.instance().allocated_height() as f64).to_value(),
            Canvas::VIEW_WIDTH => (self.instance().allocated_width() as f64).to_value(),
            _ => unimplemented!("{}", pspec.name()),
        }
    }

    fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
        match pspec.name() {
            Canvas::SHOW_GRID => {
                self.show_grid.set(value.get().unwrap());
            }
            Canvas::SHOW_GUIDELINES => {
                self.show_guidelines.set(value.get().unwrap());
            }
            Canvas::SHOW_HANDLES => {
                self.show_handles.set(value.get().unwrap());
            }
            Canvas::INNER_FILL => {
                self.inner_fill.set(value.get().unwrap());
            }
            Canvas::TRANSFORMATION => {}
            Canvas::SHOW_TOTAL_AREA => {
                self.show_total_area.set(value.get().unwrap());
            }
            Canvas::WARP_CURSOR => {
                self.warp_cursor.set(value.get().unwrap());
            }
            Canvas::VIEW_WIDTH => {
                self.view_width.set(value.get().unwrap());
            }
            Canvas::VIEW_HEIGHT => {
                self.view_height.set(value.get().unwrap());
            }
            _ => unimplemented!("{}", pspec.name()),
        }
    }
}

impl CanvasInner {}

impl DrawingAreaImpl for CanvasInner {}
impl WidgetImpl for CanvasInner {}

glib::wrapper! {
    pub struct Canvas(ObjectSubclass<CanvasInner>)
        @extends gtk::DrawingArea, gtk::Widget;
}

impl Canvas {
    pub const INNER_FILL: &str = "inner-fill";
    pub const VIEW_HEIGHT: &str = "view-height";
    pub const VIEW_WIDTH: &str = "view-width";
    pub const SHOW_GRID: &str = "show-grid";
    pub const SHOW_GUIDELINES: &str = "show-guidelines";
    pub const SHOW_HANDLES: &str = "show-handles";
    pub const SHOW_TOTAL_AREA: &str = "show-total-area";
    pub const TRANSFORMATION: &str = "transformation";
    pub const WARP_CURSOR: &str = "warp-cursor";
    pub const MOUSE: &str = "mouse";

    pub fn new() -> Self {
        let ret: Self = glib::Object::new(&[]).expect("Failed to create Canvas");
        for prop in [Self::VIEW_WIDTH, Self::VIEW_HEIGHT] {
            ret.bind_property(prop, &ret.imp().transformation, prop)
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::DEFAULT)
                .build();
        }
        ret
    }

    pub fn draw_grid(&self, cr: &gtk::cairo::Context) -> Inhibit {
        dbg!("graw_grid");
        let show_grid: bool = self.property::<bool>(Canvas::SHOW_GRID);
        let inner_fill = self.property::<bool>(Canvas::INNER_FILL);
        let scale: f64 = self
            .imp()
            .transformation
            .property::<f64>(Transformation::SCALE);
        let cw: f64 = self.property::<f64>(Canvas::VIEW_WIDTH);
        let ch: f64 = self.property::<f64>(Canvas::VIEW_HEIGHT);
        let matrix = self.imp().transformation.matrix();
        let ppu = self
            .imp()
            .transformation
            .property::<f64>(Transformation::PIXELS_PER_UNIT);

        let UnitPoint(camera) = self.imp().transformation.camera();
        let ViewPoint(view_camera) = self.unit_to_view_point(UnitPoint(camera));
        cr.save().unwrap();
        //cr.scale(scale, scale);
        let mut matrix = cairo::Matrix::identity();
        //matrix.translate(cw / 2.0, ch / 2.0);
        matrix.scale(ppu * scale, ppu * scale);
        cr.transform(matrix);
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().unwrap();
        cr.set_line_width(0.5);

        let (unit_width, unit_height) = (cw / (scale * ppu), ch / (scale * ppu));
        dbg!(unit_width, unit_height);

        if show_grid {
            //dbg!(view_camera);
            for &(color, step) in &[(0.9, 5.0), (0.8, 100.0)] {
                cr.set_source_rgb(color, color, color);
                let mut y = -camera.y % step + 0.5;
                while y < (unit_height) {
                    cr.move_to(0.5, y);
                    cr.line_to((unit_width + 0.5), y);
                    y += step;
                }
                cr.stroke().unwrap();
                let mut x = camera.x % step + 0.5;
                while x < (unit_width) {
                    cr.move_to(x, 0.5);
                    cr.line_to(x, unit_height + 0.5);
                    x += step;
                }
                cr.stroke().unwrap();
            }
        }
        let ruler_breadth = RULER_BREADTH / (scale * ppu);
        let font_size = 6.0 / (scale * ppu);
        let UnitPoint(mouse) = self.view_to_unit_point(self.get_mouse());
        /* Draw rulers */
        cr.rectangle(0., 0., unit_width, ruler_breadth);
        cr.set_source_rgb(1., 1., 1.);
        cr.fill_preserve().expect("Invalid cairo surface state");
        cr.set_source_rgb(0., 0., 0.);
        cr.stroke_preserve().unwrap();
        cr.set_source_rgb(0., 0., 0.);
        cr.move_to(mouse.x - camera.x, 0.);
        cr.line_to(mouse.x - camera.x, ruler_breadth);
        cr.stroke().unwrap();
        cr.move_to(mouse.x - camera.x + 1., 2. * ruler_breadth / 3.);
        cr.set_font_size(font_size);
        cr.show_text(&format!("{:.0}", (mouse.x - camera.x)))
            .unwrap();

        cr.rectangle(0., ruler_breadth, ruler_breadth, unit_height);
        cr.set_source_rgb(1., 1., 1.);
        cr.fill_preserve().expect("Invalid cairo surface state");
        cr.set_source_rgb(0., 0., 0.);
        cr.stroke_preserve().unwrap();
        cr.set_source_rgb(0., 0., 0.);
        cr.move_to(0., mouse.y);
        cr.line_to(ruler_breadth, mouse.y);
        cr.stroke().unwrap();
        cr.move_to(2. * ruler_breadth / 3., mouse.y - 1.);
        cr.set_font_size(font_size);
        cr.save().expect("Invalid cairo surface state");
        cr.rotate(-std::f64::consts::FRAC_PI_2);
        //cr.show_text(&format!("{:.0}", units_per_em - (mouse.y - camera.y * zoom_factor) / (f * zoom_factor))).unwrap();
        cr.restore().expect("Invalid cairo surface state");
        cr.restore().unwrap();

        Inhibit(false)
    }

    pub fn view_to_unit_point(&self, viewpoint: ViewPoint) -> UnitPoint {
        let UnitPoint(camera) = self.imp().transformation.camera();
        let scale = self
            .imp()
            .transformation
            .property::<f64>(Transformation::SCALE);
        let ppu = self
            .imp()
            .transformation
            .property::<f64>(Transformation::PIXELS_PER_UNIT);
        let ch = self.property::<f64>(Self::VIEW_HEIGHT);
        let cw = self.property::<f64>(Self::VIEW_WIDTH);
        let ViewPoint(viewpoint) = viewpoint;
        let mut retval: Point = viewpoint;
        retval.y *= -1.0;
        retval.x -= cw / 2.0;
        retval.y += ch / 2.0;
        retval /= (scale * ppu);
        retval = retval - camera;

        //println!("view_to_unit_point: {scale:?}, {ppu:?}, {view_height:?}, {retval:?}");
        UnitPoint(retval)
    }

    pub fn unit_to_view_point(&self, unitpoint: UnitPoint) -> ViewPoint {
        let UnitPoint(camera) = self.imp().transformation.camera();
        let scale = self
            .imp()
            .transformation
            .property::<f64>(Transformation::SCALE);
        let ppu = self
            .imp()
            .transformation
            .property::<f64>(Transformation::PIXELS_PER_UNIT);
        let ch = self.property::<f64>(Self::VIEW_HEIGHT);
        let cw = self.property::<f64>(Self::VIEW_WIDTH);
        let UnitPoint(unitpoint) = unitpoint;
        let mut retval: Point = (unitpoint + camera);
        retval = retval * ppu;
        retval = retval * scale;
        retval.y *= -1.0;
        retval.x += cw / 2.0;
        retval.y += ch / 2.0;

        //println!("view_to_unit_point: {scale:?}, {ppu:?}, {view_height:?}, {retval:?}");
        ViewPoint(retval)
    }

    pub fn set_mouse(&self, new_value: ViewPoint) {
        self.imp().mouse.set(new_value);
    }

    pub fn get_mouse(&self) -> ViewPoint {
        self.imp().mouse.get()
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}
