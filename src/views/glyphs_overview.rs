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

use glib::clone;
use gtk::cairo::{Context, FontSlant, FontWeight};
use gtk::glib;
use gtk::glib::subclass::Signal;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use once_cell::{sync::Lazy, unsync::OnceCell};
use std::cell::Cell;
use std::sync::{Arc, Mutex};

use crate::project::{Glyph, Project};

#[derive(Debug, Default)]
pub struct GlyphsArea {
    app: OnceCell<gtk::Application>,
    project: OnceCell<Arc<Mutex<Option<Project>>>>,
    grid: OnceCell<gtk::Grid>,
    hide_empty: Cell<bool>,
    widgets: OnceCell<Vec<GlyphBoxItem>>,
}

#[glib::object_subclass]
impl ObjectSubclass for GlyphsArea {
    const NAME: &'static str = "GlyphsArea";
    type Type = GlyphsOverview;
    type ParentType = gtk::EventBox;
}

impl ObjectImpl for GlyphsArea {
    // Here we are overriding the glib::Object::contructed
    // method. Its what gets called when we create our Object
    // and where we can initialize things.
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        let grid = gtk::Grid::builder()
            .expand(true)
            .visible(true)
            .can_focus(true)
            .column_spacing(5)
            .row_spacing(5)
            .build();

        let scrolled_window = gtk::ScrolledWindow::builder()
            .expand(true)
            .visible(true)
            .can_focus(true)
            .margin_start(5)
            .build();
        scrolled_window.set_child(Some(&grid));

        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 0);
        box_.set_spacing(5);
        let tool_palette = gtk::ToolPalette::new();
        tool_palette.set_hexpand(true);
        tool_palette.set_vexpand(false);
        tool_palette.set_height_request(40);
        tool_palette.set_orientation(gtk::Orientation::Horizontal);
        let glyph_overview_tools = gtk::ToolItemGroup::new("");
        let hide_empty_button = gtk::ToggleToolButton::builder()
            .label("Hide empty glyphs")
            .valign(gtk::Align::Center)
            .build();
        hide_empty_button.connect_clicked(clone!(@weak obj => move |_| {
            let imp = obj.imp();
            let hide_empty = !imp.hide_empty.get();
            imp.hide_empty.set(hide_empty);
            obj.update_grid(hide_empty);
            imp.grid.get().unwrap().queue_draw();
        }));
        glyph_overview_tools.add(&hide_empty_button);
        tool_palette.add(&glyph_overview_tools);
        tool_palette
            .style_context()
            .add_class("glyphs_area_toolbar");
        box_.add(&tool_palette);
        box_.add(&scrolled_window);
        obj.set_child(Some(&box_));
        self.hide_empty.set(false);

        self.grid
            .set(grid)
            .expect("Failed to initialize window state");
    }
}

impl WidgetImpl for GlyphsArea {}
impl ContainerImpl for GlyphsArea {}
impl BinImpl for GlyphsArea {}
impl EventBoxImpl for GlyphsArea {}

glib::wrapper! {
    pub struct GlyphsOverview(ObjectSubclass<GlyphsArea>)
        @extends gtk::Widget, gtk::Container, gtk::EventBox;
}

impl GlyphsOverview {
    pub fn new(app: gtk::Application, project: Arc<Mutex<Option<Project>>>) -> Self {
        let ret: Self = glib::Object::new(&[]).expect("Failed to create Main Window");
        let (mut col, mut row) = (0, 0);
        let grid = ret.imp().grid.get().unwrap();
        let project_copy = project.clone();
        let mut widgets = vec![];
        for c in crate::utils::CODEPOINTS.chars() {
            if let Some(glyph) = project
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|p| p.glyphs.get(&(c as u32)))
            {
                let glyph_box = GlyphBoxItem::new(app.clone(), project_copy.clone(), glyph.clone());
                grid.attach(&glyph_box, col, row, 1, 1);
                widgets.push(glyph_box);
                col += 1;
                if col == 4 {
                    col = 0;
                    row += 1;
                }
            }
        }
        ret.imp().app.set(app).unwrap();
        ret.imp().widgets.set(widgets).unwrap();
        ret.imp().project.set(project).unwrap();
        ret
    }

    fn update_grid(&self, hide_empty: bool) {
        let (mut col, mut row) = (0, 0);
        let grid = self.imp().grid.get().unwrap();
        let children = grid.children();
        for c in children {
            grid.remove(&c);
        }
        for c in self.imp().widgets.get().unwrap() {
            if hide_empty && c.imp().glyph.get().unwrap().is_empty() {
                continue;
            }
            grid.attach(c, col, row, 1, 1);
            col += 1;
            if col == 4 {
                col = 0;
                row += 1;
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct GlyphBox {
    pub app: OnceCell<gtk::Application>,
    pub project: OnceCell<Arc<Mutex<Option<Project>>>>,
    pub glyph: OnceCell<Glyph>,
    pub focused: Cell<bool>,
    pub drawing_area: OnceCell<gtk::DrawingArea>,
}

unsafe impl Send for GlyphBox {}
unsafe impl Sync for GlyphBox {}

unsafe impl Send for GlyphBoxItem {}
unsafe impl Sync for GlyphBoxItem {}
#[glib::object_subclass]
impl ObjectSubclass for GlyphBox {
    const NAME: &'static str = "GlyphBox";
    type Type = GlyphBoxItem;
    type ParentType = gtk::EventBox;
}

impl ObjectImpl for GlyphBox {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        obj.set_height_request(140);
        obj.set_width_request(110);
        obj.set_can_focus(true);
        obj.set_expand(false);

        obj.connect(
            "button-press-event",
            false,
            clone!(@weak obj => @default-return Some(false.to_value()), move |_| {
                obj.imp().app.get().unwrap().downcast_ref::<crate::GerbApp>().unwrap().imp().window.get().unwrap().emit_by_name::<()>("open-glyph-edit", &[&obj]);
                println!("open-glyph-edit emitted!");


                Some(true.to_value())
            }),
        );
        let drawing_area = gtk::DrawingArea::builder()
            .expand(true)
            .visible(true)
            .can_focus(true)
            .build();
        drawing_area.connect_draw(clone!(@weak obj => @default-return Inhibit(false), move |_drar: &gtk::DrawingArea, cr: &Context| {
            let is_focused:bool = obj.imp().focused.get();
            //cr.scale(500f64, 500f64);
            //let (r, g, b) = crate::utils::hex_color_to_rgb("#c4c4c4").unwrap();
            //cr.set_source_rgb(r, g, b);
            cr.set_source_rgb(1., 1., 1.);
            cr.paint().expect("Invalid cairo surface state");

            const GLYPH_BOX_WIDTH: f64 = 110.;
            const GLYPH_BOX_HEIGHT: f64 = 140.;
            let (x, y) = (0.01, 0.01);
            let c = obj.imp().glyph.get().unwrap().char;
            let label = c.to_string();
            cr.set_line_width(1.5);
            let (point, (width, height)) = crate::utils::draw_round_rectangle(cr, (x, y), (GLYPH_BOX_WIDTH, GLYPH_BOX_HEIGHT), 1.0, 1.5);
            if is_focused {
                cr.set_source_rgb(1., 250./255., 141./255.);
            } else {
                cr.set_source_rgb(1., 1., 1.);
            }
            cr.fill_preserve().expect("Invalid cairo surface state");
            cr.set_source_rgba(0., 0., 0., 0.5);
            cr.stroke_preserve().expect("Invalid cairo surface state");
            cr.clip();
            cr.new_path();
            cr.set_source_rgba(0., 0., 0., 0.4);
            cr.move_to(x+width/2., point.1+ 2.* (height / 3.));
            cr.set_font_size(62.);
            let sextents = cr
                .text_extents(&label)
                .expect("Invalid cairo surface state");
            cr.move_to(point.0 + width/2. - sextents.width/2., point.1+(height / 3.)+20.);
            let glyph = obj.imp().glyph.get().unwrap();
            if glyph.curves.is_empty() {
                cr.show_text(&label).expect("Invalid cairo surface state");
            } else {
                glyph.draw(_drar, cr, (point.0, point.1+20.), (width*0.8, height*0.8));
            }


            cr.set_line_width(2.);
            cr.set_source_rgb(0., 0., 0.);
            cr.move_to(x, point.1+ 2.* (height / 3.));
            cr.line_to(x+width*1.2, point.1+ 2.* (height / 3.));
            cr.stroke().expect("Invalid cairo surface state");
            cr.set_source_rgb(196./255., 196./255., 196./255.);
            cr.new_path();
            cr.rectangle(x, point.1+2.*(height/3.), width*1.2, 1.2*height/3.);
            cr.fill().expect("Invalid cairo surface state");
            cr.reset_clip();

            cr.set_source_rgb(0., 0., 0.);
            cr.select_font_face("Monospace", FontSlant::Normal, FontWeight::Normal);
            cr.set_font_size(12.);
            let sextents = cr
                .text_extents(&label)
                .expect("Invalid cairo surface state");
            cr.move_to(point.0 + width/2. - sextents.width/2., point.1+ 2.* (height / 3.)+20.);
            cr.show_text(&label).expect("Invalid cairo surface state");


            let label = format!("U+{:04X}", c as u32);
            let extents = cr
                .text_extents(&label)
                .expect("Invalid cairo surface state");
            cr.move_to(point.0 + width/2. - extents.width/2., point.1+ 2.* (height / 3.)+22.0 + sextents.height);
            cr.show_text(&label).expect("Invalid cairo surface state");

            Inhibit(false)
        }
        ));
        obj.set_child(Some(&drawing_area));

        obj.set_events(
            gtk::gdk::EventMask::POINTER_MOTION_MASK
                | gtk::gdk::EventMask::ENTER_NOTIFY_MASK
                | gtk::gdk::EventMask::LEAVE_NOTIFY_MASK,
        );
        obj.connect_enter_notify_event(|_self, _event| -> Inhibit {
            //println!("obj has window {}", _self.has_window());
            if let Some(screen) = _self.window() {
                let display = screen.display();
                screen.set_cursor(Some(
                    &gtk::gdk::Cursor::from_name(&display, "pointer").unwrap(),
                ));
            }
            _self.imp().focused.set(true);
            _self.imp().drawing_area.get().unwrap().queue_draw();
            //println!("focus in {:?}", _self.imp().glyph.get().unwrap());
            Inhibit(false)
        });

        obj.connect_leave_notify_event(|_self, _event| -> Inhibit {
            //println!("focus out {:?}", _self.imp().glyph.get().unwrap());
            if let Some(screen) = _self.window() {
                let display = screen.display();
                screen.set_cursor(Some(
                    &gtk::gdk::Cursor::from_name(&display, "default").unwrap(),
                ));
            }
            _self.imp().focused.set(false);
            _self.imp().drawing_area.get().unwrap().queue_draw();

            Inhibit(false)
        });

        self.drawing_area
            .set(drawing_area)
            .expect("Failed to initialize window state");
        self.focused.set(false);
    }
}

impl WidgetImpl for GlyphBox {}
impl ContainerImpl for GlyphBox {}
impl BinImpl for GlyphBox {}
impl EventBoxImpl for GlyphBox {}

glib::wrapper! {
    pub struct GlyphBoxItem(ObjectSubclass<GlyphBox>)
        @extends gtk::Widget, gtk::Container, gtk::Bin, gtk::EventBox;
}

impl GlyphBoxItem {
    pub fn new(app: gtk::Application, project: Arc<Mutex<Option<Project>>>, glyph: Glyph) -> Self {
        let ret: Self = glib::Object::new(&[]).expect("Failed to create Main Window");
        ret.imp().app.set(app).unwrap();
        ret.imp().project.set(project).unwrap();
        ret.imp().glyph.set(glyph).unwrap();
        ret
    }
}
