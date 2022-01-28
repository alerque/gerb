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
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use once_cell::unsync::OnceCell;
use std::cell::Cell;
use std::sync::{Arc, Mutex};

use crate::glyphs::Glyph;
use crate::project::Project;

#[derive(Debug, Default)]
pub struct GlyphEditArea {
    app: OnceCell<gtk::Application>,
    glyph: OnceCell<Glyph>,
    drawing_area: OnceCell<gtk::DrawingArea>,
    overlay: OnceCell<gtk::Overlay>,
    pub toolbar_box: OnceCell<gtk::Box>,
    zoom_percent_label: OnceCell<gtk::Label>,
    camera: Cell<(f64, f64)>,
    mouse: Cell<(f64, f64)>,
    zoom: Cell<f64>,
    button: Cell<Option<u32>>,
    project: OnceCell<Arc<Mutex<Option<Project>>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for GlyphEditArea {
    const NAME: &'static str = "GlyphEditArea";
    type Type = GlyphEditView;
    type ParentType = gtk::Bin;
}

impl ObjectImpl for GlyphEditArea {
    // Here we are overriding the glib::Objcet::contructed
    // method. Its what gets called when we create our Object
    // and where we can initialize things.
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        self.camera.set((0., 0.));
        self.mouse.set((0., 0.));
        self.zoom.set(1.);

        let drawing_area = gtk::DrawingArea::builder()
            .expand(true)
            .visible(true)
            .build();
        drawing_area.set_events(
            gtk::gdk::EventMask::BUTTON_PRESS_MASK
                | gtk::gdk::EventMask::BUTTON_RELEASE_MASK
                | gtk::gdk::EventMask::POINTER_MOTION_MASK,
        );
        drawing_area.connect_button_press_event(
            clone!(@weak obj => @default-return Inhibit(false), move |_self, event| {

                obj.imp().mouse.set(event.position());
                obj.imp().button.set(Some(event.button()));

                Inhibit(false)
            }),
        );
        drawing_area.connect_button_release_event(
            clone!(@weak obj => @default-return Inhibit(false), move |_self, _event| {
                //obj.imp().mouse.set((0., 0.));
                obj.imp().button.set(None);
                    if let Some(screen) = _self.window() {
                        let display = screen.display();
                        screen.set_cursor(Some(
                                &gtk::gdk::Cursor::from_name(&display, "default").unwrap(),
                        ));
                    }

                Inhibit(false)
            }),
        );
        drawing_area.connect_motion_notify_event(
            clone!(@weak obj => @default-return Inhibit(false), move |_self, event| {
                if let Some(gtk::gdk::BUTTON_SECONDARY) = obj.imp().button.get(){
                    let mut camera = obj.imp().camera.get();
                    let mouse = obj.imp().mouse.get();
                    camera.0 += event.position().0 - mouse.0;
                    camera.1 += event.position().1 - mouse.1;
                    obj.imp().camera.set(camera);
                    if let Some(screen) = _self.window() {
                        let display = screen.display();
                        screen.set_cursor(Some(
                                &gtk::gdk::Cursor::from_name(&display, "grab").unwrap(),
                        ));
                    }
                }
                obj.imp().mouse.set(event.position());
                _self.queue_draw();

                Inhibit(false)
            }),
        );

        drawing_area.connect_draw(clone!(@weak obj => @default-return Inhibit(false), move |drar: &gtk::DrawingArea, cr: &gtk::cairo::Context| {
            let zoom_factor = obj.imp().zoom.get();
            cr.save().unwrap();
            cr.scale(zoom_factor, zoom_factor);
            let (units_per_em, x_height, cap_height, _ascender, _descender) = {
                let mutex = obj.imp().project.get().unwrap();
                let lck = mutex.lock().unwrap();
                if lck.is_none() {
                    return Inhibit(false);
                }
                let p = lck.as_ref().unwrap();
                (p.units_per_em, p.x_height, p.cap_height, p.ascender, p.descender)
            };
            let width = drar.allocated_width() as f64;
            let height = drar.allocated_height() as f64;
            cr.set_source_rgb(1., 1., 1.);
            cr.paint().expect("Invalid cairo surface state");

            cr.set_line_width(1.0);

            let camera = obj.imp().camera.get();
            let mouse = obj.imp().mouse.get();

            for &(color, step) in &[(0.9, 5.0), (0.8, 100.0)] {
                cr.set_source_rgb(color, color, color);
                let mut y = (camera.1 % step).floor() + 0.5;
                while y < height {
                    cr.move_to(0., y);
                    cr.line_to(width, y);
                    y += step;
                }
                cr.stroke().unwrap();
                let mut x = (camera.0 % step).floor() + 0.5;
                while x < width {
                    cr.move_to(x, 0.);
                    cr.line_to(x, height);
                    x += step;
                }
                cr.stroke().unwrap();
            }

            /* Draw em square of 1000 units: */

            cr.save().unwrap();
            cr.translate(camera.0, camera.1);
            cr.set_source_rgba(210./255., 227./255., 252./255., 0.6);
            cr.rectangle(0., 0., 200., 200.);
            cr.fill().unwrap();

            /* Draw x-height */
            cr.set_source_rgba(1.0, 0., 0., 0.6);
            cr.set_line_width(2.0);
            cr.move_to(0., x_height*0.2);
            cr.line_to(200., x_height*0.2);
            cr.stroke().unwrap();
            cr.move_to(200., x_height*0.2);
            cr.show_text("x-height").unwrap();

            /* Draw baseline */
            cr.move_to(0., units_per_em*0.2);
            cr.line_to(200., units_per_em*0.2);
            cr.stroke().unwrap();
            cr.move_to(200., units_per_em*0.2);
            cr.show_text("baseline").unwrap();

            /* Draw cap height */
            cr.move_to(0., -cap_height*0.2);
            cr.line_to(200., -cap_height*0.2);
            cr.stroke().unwrap();
            cr.move_to(200., -cap_height*0.2);
            cr.show_text("cap height").unwrap();

            /* Draw the glyph */

            if let Some(glyph) = obj.imp().glyph.get() {
                //println!("cairo drawing glyph {}", glyph.name);
                glyph.draw(drar, cr, (0.0, 0.0), (200., 200.));
                cr.set_source_rgb(1.0, 0.0, 0.5);
                cr.set_line_width(1.5);
                if let Some(width) = glyph.width {
                    cr.move_to(0., 0.);
                    cr.line_to(0., 200.);
                    cr.stroke().unwrap();
                    cr.move_to(width as f64 *0.2, 0.);
                    cr.line_to(width as f64 *0.2, 200.);
                    cr.stroke().unwrap();
                }
                /*for c in &glyph.curves {
                    for &(x, y) in &c.points {
                        cr.rectangle(x as f64, y as f64, 5., 5.);
                        cr.stroke_preserve().expect("Invalid cairo surface state");
                    }
                }
                */
            } else {
                //println!("cairo drawing without glyph");
            }
            cr.restore().unwrap();
            cr.restore().unwrap();

            /* Draw rulers */
            cr.rectangle(0., 0., width, 13.);
            cr.set_source_rgb(1., 1., 1.);
            cr.fill_preserve().expect("Invalid cairo surface state");
            cr.set_source_rgb(0., 0., 0.);
            cr.stroke_preserve().unwrap();
            cr.set_source_rgb(0., 0., 0.);
            cr.move_to(mouse.0, 0.);
            cr.line_to(mouse.0, 13.);
            cr.stroke().unwrap();
            cr.move_to(mouse.0, 0.);
            cr.set_font_size(6.);
            cr.show_text(&format!("{:.0}", mouse.0-camera.0)).unwrap();


            cr.rectangle(0., 0., 15., height);
            cr.set_source_rgb(1., 1., 1.);
            cr.fill_preserve().expect("Invalid cairo surface state");
            cr.set_source_rgb(0., 0., 0.);
            cr.stroke_preserve().unwrap();
            cr.set_source_rgb(0., 0., 0.);
            cr.move_to(0., mouse.1);
            cr.line_to(13., mouse.1);
            cr.stroke().unwrap();
            cr.move_to(0., mouse.1);
            cr.set_font_size(6.);
            cr.show_text(&format!("{:.0}", (mouse.1-camera.1)*5.)).unwrap();


           Inhibit(false)
        }));
        let toolbar_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .expand(false)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Start)
            .spacing(5)
            .visible(true)
            .can_focus(true)
            .build();
        let toolbar = gtk::Toolbar::builder()
            .orientation(gtk::Orientation::Horizontal)
            .expand(false)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Start)
            //.toolbar_style(gtk::ToolbarStyle::Both)
            .visible(true)
            .can_focus(true)
            .build();

        let edit_pixbuf =
            gtk::gdk_pixbuf::Pixbuf::from_read(crate::resources::GRAB_ICON_SVG.as_bytes()).unwrap();
        let edit_pixbuf = edit_pixbuf
            .scale_simple(26, 26, gtk::gdk_pixbuf::InterpType::Bilinear)
            .unwrap();
        let edit_image = gtk::Image::from_pixbuf(Some(&edit_pixbuf));
        edit_image.set_visible(true);
        let edit_button = gtk::ToolButton::new(Some(&edit_image), Some("Edit"));
        //let edit_button = gtk::ToolButton::new(gtk::ToolButton::NONE, Some("Edit"));
        edit_button.set_visible(true);

        let pen_pixbuf =
            gtk::gdk_pixbuf::Pixbuf::from_read(crate::resources::PEN_ICON_SVG.as_bytes()).unwrap();
        let pen_pixbuf = pen_pixbuf
            .scale_simple(26, 26, gtk::gdk_pixbuf::InterpType::Bilinear)
            .unwrap();
        let pen_image = gtk::Image::from_pixbuf(Some(&pen_pixbuf));
        pen_image.set_visible(true);

        let pen_button = gtk::ToolButton::new(Some(&pen_image), Some("Pen"));
        //let pen_button = gtk::ToolButton::new(gtk::ToolButton::NONE, Some("Pen"));
        pen_button.set_visible(true);
        let zoom_in_button = gtk::ToolButton::new(gtk::ToolButton::NONE, Some("Zoom in"));
        zoom_in_button.set_visible(true);
        zoom_in_button.connect_clicked(clone!(@weak obj => move |_| {
            let imp = obj.imp();
            let zoom_factor = imp.zoom.get() + 0.25;
            if zoom_factor < 4.25 {
                imp.zoom.set(zoom_factor);
                imp.zoom_percent_label.get().unwrap().set_text(&format!("{}%", zoom_factor * 100.));
                imp.overlay.get().unwrap().queue_draw();
            }
        }));
        let zoom_out_button = gtk::ToolButton::new(gtk::ToolButton::NONE, Some("Zoom out"));
        zoom_out_button.set_visible(true);
        zoom_out_button.connect_clicked(clone!(@weak obj => move |_| {
            let imp = obj.imp();
            let zoom_factor = imp.zoom.get() - 0.25;
            if zoom_factor > 0. {
                imp.zoom.set(zoom_factor);
                imp.zoom_percent_label.get().unwrap().set_text(&format!("{}%", zoom_factor * 100.));
                imp.overlay.get().unwrap().queue_draw();
            }
        }));
        let zoom_percent_label = gtk::Label::new(Some("100%"));
        zoom_percent_label.set_visible(true);
        toolbar.add(&edit_button);
        toolbar.set_item_homogeneous(&edit_button, false);
        toolbar.add(&pen_button);
        toolbar.set_item_homogeneous(&pen_button, false);
        toolbar.add(&zoom_in_button);
        toolbar.set_item_homogeneous(&zoom_in_button, false);
        toolbar.add(&zoom_out_button);
        toolbar.set_item_homogeneous(&zoom_out_button, false);
        toolbar_box.pack_start(&toolbar, false, false, 0);
        toolbar_box.pack_start(&zoom_percent_label, false, false, 0);
        toolbar_box.style_context().add_class("glyph-edit-toolbox");
        let overlay = gtk::Overlay::builder()
            .expand(true)
            .visible(true)
            .can_focus(true)
            .build();
        overlay.add_overlay(&drawing_area);
        overlay.add_overlay(&toolbar_box);
        obj.add(&overlay);
        obj.set_visible(true);
        obj.set_expand(true);
        obj.set_can_focus(true);

        self.zoom_percent_label
            .set(zoom_percent_label)
            .expect("Failed to initialize window state");
        self.overlay
            .set(overlay)
            .expect("Failed to initialize window state");
        self.toolbar_box
            .set(toolbar_box)
            .expect("Failed to initialize window state");
        self.drawing_area
            .set(drawing_area)
            .expect("Failed to initialize window state");
    }
}

impl WidgetImpl for GlyphEditArea {}
impl ContainerImpl for GlyphEditArea {}
impl BinImpl for GlyphEditArea {}

glib::wrapper! {
    pub struct GlyphEditView(ObjectSubclass<GlyphEditArea>)
        @extends gtk::Widget, gtk::Container, gtk::Bin;
}

impl GlyphEditView {
    pub fn new(app: gtk::Application, project: Arc<Mutex<Option<Project>>>, glyph: Glyph) -> Self {
        let ret: Self = glib::Object::new(&[]).expect("Failed to create Main Window");
        ret.imp().glyph.set(glyph).unwrap();
        ret.imp().app.set(app).unwrap();
        ret.imp().project.set(project).unwrap();
        ret
    }
}
