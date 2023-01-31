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

use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::rc::{Rc, Weak};

use crate::ufo;
use crate::unicode::names::CharName;
use crate::utils::{curves::*, *};

use gtk::cairo::{Context, Matrix};
use gtk::{glib::prelude::*, subclass::prelude::*};
use uuid::Uuid;

mod guidelines;
pub use guidelines::*;

mod glif;

mod contours;
pub use contours::*;

#[derive(Debug, Clone)]
pub struct Component {
    base_name: String,
    base: Weak<RefCell<Glyph>>,
    x_offset: f64,
    y_offset: f64,
    x_scale: f64,
    xy_scale: f64,
    yx_scale: f64,
    y_scale: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlyphKind {
    Char(char),
    Component,
}

#[derive(Debug, Clone)]
pub struct Glyph {
    pub name: Cow<'static, str>,
    pub name2: Option<crate::unicode::names::Name>,
    pub kind: GlyphKind,
    pub width: Option<f64>,
    pub contours: Vec<Contour>,
    pub components: Vec<Component>,
    pub guidelines: Vec<Guideline>,
    pub glif_source: String,
}

impl Ord for Glyph {
    fn cmp(&self, other: &Self) -> Ordering {
        use GlyphKind::*;
        match (&self.kind, &other.kind) {
            (Char(s), Char(o)) => s.cmp(o),
            (Char(_), _) => Ordering::Less,
            (Component, Component) => self.name.cmp(&other.name),
            (Component, Char(_)) => Ordering::Greater,
        }
    }
}

impl PartialOrd for Glyph {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Glyph {
    fn eq(&self, other: &Self) -> bool {
        use GlyphKind::*;
        match (&self.kind, &other.kind) {
            (Char(s), Char(o)) => s == o,
            (Char(_), Component) | (Component, Char(_)) => false,
            (Component, Component) => self.name == other.name,
        }
    }
}

impl Eq for Glyph {}

impl Default for Glyph {
    fn default() -> Self {
        Glyph::new_empty("space", ' ')
    }
}

#[derive(Clone, Copy)]
pub struct GlyphDrawingOptions {
    pub outline: Color,
    pub inner_fill: Option<Color>,
    pub highlight: Option<(usize, usize)>,
    pub matrix: Matrix,
    pub units_per_em: f64,
    pub line_width: f64,
    pub handle_size: Option<f64>,
}

impl Default for GlyphDrawingOptions {
    fn default() -> Self {
        Self {
            outline: Color::WHITE,
            inner_fill: None,
            highlight: None,
            matrix: Matrix::identity(),
            units_per_em: 1000.,
            line_width: 4.0,
            handle_size: None,
        }
    }
}

impl Glyph {
    #[allow(clippy::type_complexity)]
    pub fn from_ufo(
        path: &Path,
        contents: &ufo::Contents,
    ) -> Result<HashMap<String, Rc<RefCell<Glyph>>>, Box<dyn std::error::Error>> {
        let mut ret: HashMap<String, Rc<RefCell<Glyph>>> = HashMap::default();
        let mut glyphs_with_refs: Vec<Rc<_>> = vec![];
        let mut path = path.to_path_buf();
        path.push("glyphs");

        for (name, filename) in contents.glyphs.iter() {
            path.push(filename);
            use std::fs::File;
            use std::io::prelude::*;
            let mut file = match File::open(&path) {
                Err(err) => return Err(format!("Couldn't open {}: {}", path.display(), err).into()),
                Ok(file) => file,
            };

            let mut s = String::new();
            if let Err(err) = file.read_to_string(&mut s) {
                return Err(format!("Couldn't read {}: {}", path.display(), err).into());
            }
            let g: Result<glif::Glif, _> = glif::Glif::from_str(&s);
            match g {
                Err(err) => {
                    eprintln!("couldn't parse {}: {}", path.display(), err);
                }
                Ok(g) => {
                    for mut g in g.into_iter() {
                        g.glif_source = s.clone();
                        let has_components = !g.components.is_empty();
                        let g = Rc::new(RefCell::new(g));
                        if has_components {
                            glyphs_with_refs.push(g.clone());
                        }
                        ret.insert(name.into(), g);
                    }
                }
            }
            path.pop();
        }

        for g in glyphs_with_refs {
            let mut deref = g.borrow_mut();
            for c in deref.components.iter_mut() {
                if let Some(o) = ret.get(&c.base_name) {
                    c.base = Rc::downgrade(o);
                }
            }
        }
        Ok(ret)
    }

    pub fn new(name: &'static str, char: char, curves: Vec<Bezier>) -> Self {
        let contour = Contour::new();
        *contour.imp().curves.borrow_mut() = curves;
        Glyph {
            name: name.into(),
            name2: char.char_name(),
            kind: GlyphKind::Char(char),
            contours: vec![contour],
            components: vec![],
            guidelines: vec![],
            width: None,
            glif_source: String::new(),
        }
    }

    pub fn new_empty(name: &'static str, char: char) -> Self {
        Glyph::new(name, char, vec![])
    }

    pub fn draw(&self, cr: &Context, options: GlyphDrawingOptions) {
        if self.is_empty() {
            return;
        }
        let GlyphDrawingOptions {
            outline,
            inner_fill,
            highlight,
            matrix,
            units_per_em: _,
            line_width,
            handle_size,
        } = options;

        cr.save().expect("Invalid cairo surface state");
        cr.set_line_width(line_width);
        cr.transform(matrix);
        //cr.transform(Matrix::new(1.0, 0., 0., -1.0, 0., units_per_em.abs()));
        cr.set_source_color_alpha(outline);
        let mut pen_position: Option<Point> = None;
        for (_ic, contour) in self.contours.iter().enumerate() {
            let curves = contour.imp().curves.borrow();
            if !contour.property::<bool>(Contour::OPEN) {
                if let Some(point) = curves
                    .last()
                    .and_then(|b| b.points().borrow().last().cloned())
                {
                    cr.move_to(point.x, point.y);
                    pen_position = Some(point.position);
                }
            } else if let Some(point) = curves
                .first()
                .and_then(|b| b.points().borrow().first().cloned())
            {
                cr.move_to(point.x, point.y);
            }

            for (_jc, curv) in curves.iter().enumerate() {
                let degree = curv.degree();
                let degree = if let Some(v) = degree {
                    v
                } else {
                    continue;
                };
                let curv_points = curv.points().borrow();
                match degree {
                    0 => { /* Single point */ }
                    1 => {
                        /* Line. */
                        let new_point = curv_points[1].position;
                        cr.line_to(new_point.x, new_point.y);
                        pen_position = Some(new_point);
                    }
                    2 => {
                        /* Quadratic. */
                        let a = if let Some(v) = pen_position.take() {
                            v
                        } else {
                            curv_points[0].position
                        };
                        let b = curv_points[1].position;
                        let c = curv_points[2].position;
                        cr.curve_to(
                            2.0 / 3.0 * b.x + 1.0 / 3.0 * a.x,
                            2.0 / 3.0 * b.y + 1.0 / 3.0 * a.y,
                            2.0 / 3.0 * b.x + 1.0 / 3.0 * c.x,
                            2.0 / 3.0 * b.y + 1.0 / 3.0 * c.y,
                            c.x,
                            c.y,
                        );
                        pen_position = Some(c);
                    }
                    3 => {
                        /* Cubic */
                        let _a = if let Some(v) = pen_position.take() {
                            v
                        } else {
                            curv_points[0].position
                        };
                        let b = curv_points[1].position;
                        let c = curv_points[2].position;
                        let d = curv_points[3].position;
                        cr.curve_to(b.x, b.y, c.x, c.y, d.x, d.y);
                        pen_position = Some(d);
                    }
                    d => {
                        eprintln!("Something's wrong. Bezier of degree {}: {:?}", d, curv);
                        pen_position = Some(curv_points.last().unwrap().position);
                        continue;
                    }
                }
            }
        }

        if let Some(inner_fill) = inner_fill {
            cr.save().unwrap();
            cr.close_path();
            cr.set_source_color_alpha(inner_fill);
            cr.fill_preserve().expect("Invalid cairo surface state");
            cr.restore().expect("Invalid cairo surface state");
        }

        cr.stroke().expect("Invalid cairo surface state");

        if let Some((degree, curv)) = highlight.and_then(|(contour_idx, curve_idx)| {
            self.contours
                .get(contour_idx)
                .and_then(|contour| {
                    contour
                        .curves()
                        .clone()
                        .borrow()
                        .get(curve_idx)
                        .map(Clone::clone)
                })
                .and_then(|curv| Some((curv.degree()?, curv)))
        }) {
            let curv_points = curv.points().borrow();
            cr.set_source_color(Color::RED);
            let point = curv_points[0].position;
            cr.move_to(point.x, point.y);
            match degree {
                0 => { /* Single point */ }
                1 => {
                    /* Line. */
                    let new_point = curv_points[1].position;
                    cr.line_to(new_point.x, new_point.y);
                }
                2 => {
                    /* Quadratic. */
                    let a = if let Some(v) = pen_position.take() {
                        v
                    } else {
                        curv_points[0].position
                    };
                    let b = curv_points[1].position;
                    let c = curv_points[2].position;
                    cr.curve_to(
                        2.0 / 3.0 * b.x + 1.0 / 3.0 * a.x,
                        2.0 / 3.0 * b.y + 1.0 / 3.0 * a.y,
                        2.0 / 3.0 * b.x + 1.0 / 3.0 * c.x,
                        2.0 / 3.0 * b.y + 1.0 / 3.0 * c.y,
                        c.x,
                        c.y,
                    );
                }
                3 => {
                    /* Cubic */
                    let _a = { curv_points[0].position };
                    cr.move_to(_a.x, _a.y);
                    let b = curv_points[1].position;
                    let c = curv_points[2].position;
                    let d = curv_points[3].position;
                    cr.curve_to(b.x, b.y, c.x, c.y, d.x, d.y);
                }
                d => {
                    eprintln!("Something's wrong. Bezier of degree {}: {:?}", d, curv);
                }
            }
            cr.stroke().expect("Invalid cairo surface state");
        }
        if let Some(handle_size) = handle_size {
            let draw_oncurve = |p: Point| {
                cr.set_line_width(line_width);
                cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
                cr.rectangle(
                    p.x - handle_size / 2.0,
                    p.y - handle_size / 2.0,
                    handle_size,
                    handle_size,
                );
                cr.stroke().unwrap();
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
                cr.rectangle(
                    p.x - handle_size / 2.0,
                    p.y - handle_size / 2.0,
                    handle_size,
                    handle_size + 1.0,
                );
                cr.stroke().unwrap();
            };
            let draw_handle = |p: Point| {
                if inner_fill.is_some() {
                    cr.set_source_rgba(0.9, 0.9, 0.9, 1.0);
                } else {
                    cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
                }
                cr.arc(p.x, p.y, handle_size / 2.0, 0.0, 2.0 * std::f64::consts::PI);
                cr.fill().unwrap();
                cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
                cr.arc(
                    p.x,
                    p.y,
                    handle_size / 2.0 + 1.0,
                    0.0,
                    2.0 * std::f64::consts::PI,
                );
                cr.stroke().unwrap();
            };
            let draw_handle_connection = |h: Point, ep: Point| {
                cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
                cr.move_to(h.x - 2.5, h.y - 2.5);
                cr.line_to(ep.x, ep.y);
                cr.stroke().unwrap();
            };
            for contour in self.contours.iter() {
                let curves = contour.imp().curves.borrow();
                for curv in curves.iter() {
                    let degree = curv.degree();
                    let degree = if let Some(v) = degree {
                        v
                    } else {
                        continue;
                    };
                    let curv_points = curv.points().borrow();
                    match degree {
                        0 => {
                            /* Single point */
                            draw_oncurve(curv_points[0].position);
                        }
                        1 => {
                            /* Line. */
                            draw_oncurve(curv_points[0].position);
                            draw_oncurve(curv_points[1].position);
                        }
                        2 => {
                            /* Quadratic. */
                            let handle = curv_points[1].position;
                            let ep1 = curv_points[0].position;
                            let ep2 = curv_points[2].position;
                            draw_handle_connection(handle, ep1);
                            draw_handle_connection(handle, ep2);
                            draw_handle(handle);
                            draw_oncurve(ep1);
                            draw_oncurve(ep2);
                        }
                        3 => {
                            /* Cubic */
                            let handle1 = curv_points[1].position;
                            let handle2 = curv_points[2].position;
                            let ep1 = curv_points[0].position;
                            let ep2 = curv_points[3].position;
                            draw_handle_connection(handle1, ep1);
                            draw_handle_connection(handle2, ep2);
                            draw_handle(handle1);
                            draw_handle(handle2);
                            draw_oncurve(ep1);
                            draw_oncurve(ep2);
                        }
                        d => {
                            eprintln!("Something's wrong. Bezier of degree {}: {:?}", d, curv);
                            continue;
                        }
                    }
                }
            }
        }
        cr.restore().expect("Invalid cairo surface state");
        for component in self.components.iter() {
            if let Some(rc) = component.base.upgrade() {
                let glyph = rc.borrow();
                cr.save().unwrap();
                cr.transform(matrix);
                let matrix = Matrix::new(
                    component.x_scale,
                    component.xy_scale,
                    component.yx_scale,
                    component.y_scale,
                    component.x_offset,
                    component.y_offset,
                );
                glyph.draw(
                    cr,
                    GlyphDrawingOptions {
                        matrix,
                        handle_size: None,
                        ..options
                    },
                );
                cr.restore().expect("Invalid cairo surface state");
            }
        }
    }

    pub fn into_cubic(&mut self) {
        if self.is_empty() {
            return;
        }
        for contour in self.contours.iter_mut() {
            let mut pen_position: Option<Point> = None;
            let mut curves = contour.imp().curves.borrow_mut();
            if !contour.property::<bool>(Contour::OPEN) {
                if let Some(point) = curves
                    .last()
                    .and_then(|b| b.points().borrow().last().cloned())
                {
                    pen_position = Some(point.position);
                }
            }

            for curv in curves.iter_mut() {
                let curv_points = curv.points().borrow();
                if curv_points.len() == 3 {
                    let a = if let Some(v) = pen_position.take() {
                        v
                    } else {
                        curv_points[0].position
                    };
                    let b = curv_points[1].position;
                    let c = curv_points[2].position;
                    let new_points = vec![
                        a,
                        (
                            2.0 / 3.0 * b.x + 1.0 / 3.0 * a.x,
                            2.0 / 3.0 * b.y + 1.0 / 3.0 * a.y,
                        )
                            .into(),
                        (
                            2.0 / 3.0 * b.x + 1.0 / 3.0 * c.x,
                            2.0 / 3.0 * b.y + 1.0 / 3.0 * c.y,
                        )
                            .into(),
                        c,
                    ];
                    drop(curv_points);
                    *curv = Bezier::new(new_points);
                    pen_position = Some(c);
                } else if let Some(last_p) = curv_points.last() {
                    pen_position = Some(last_p.position);
                }
            }
        }
    }

    #[cfg(feature = "svg")]
    pub fn save_to_svg<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let surface = gtk::cairo::SvgSurface::new(self.width.unwrap_or(500.0), 1000., Some(path))?;
        let ctx = gtk::cairo::Context::new(&surface)?;

        let options = GlyphDrawingOptions {
            outline: Color::BLACK,
            inner_fill: None,
            highlight: None,
            matrix: Matrix::new(1.0, 0.0, 0.0, -1.0, 0.0, 0.0),
            ..Default::default()
        };
        self.draw(&ctx, options);
        surface.flush();
        surface.finish();
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        (self.contours.is_empty()
            || self
                .contours
                .iter()
                .all(|c| c.imp().curves.borrow().is_empty()))
            && self.components.is_empty()
    }

    pub fn name_markup(&self) -> gtk::glib::GString {
        match self.kind {
            GlyphKind::Char(c) => {
                let mut b = [0; 4];
                gtk::glib::markup_escape_text(c.encode_utf8(&mut b).replace('\0', "").trim())
            }
            GlyphKind::Component => {
                gtk::glib::markup_escape_text(self.name.as_ref().replace('\0', "").trim())
            }
        }
    }

    /*
    pub fn points(&self) -> Vec<Point> {
        self.contours
            .clone()
            .into_iter()
            .map(|v| v.curves.into_iter().map(|b| b.points.into_iter()).flatten())
            .flatten()
            .collect::<Vec<Point>>()
    }
    */

    pub fn on_curve_query(
        &self,
        position: Point,
        pts: &[(GlyphPointIndex, IPoint)],
    ) -> Option<((usize, usize), Bezier)> {
        for (ic, contour) in self.contours.iter().enumerate() {
            for (jc, curve) in contour.curves().borrow().iter().enumerate() {
                if curve.on_curve_query(position, None) {
                    return Some(((ic, jc), curve.clone()));
                }
                for (
                    GlyphPointIndex {
                        contour_index,
                        curve_index,
                        uuid,
                    },
                    _p,
                ) in pts
                {
                    if (*contour_index, *curve_index) != (ic, jc) {
                        continue;
                    }
                    if curve.points().borrow().iter().any(|cp| cp.uuid == *uuid) {
                        return Some(((ic, jc), curve.clone()));
                    }
                }
            }
        }
        None
    }
}

#[derive(Clone, Hash, Eq, PartialEq, Debug, Default, Copy)]
pub struct GlyphPointIndex {
    pub contour_index: usize,
    pub curve_index: usize,
    pub uuid: Uuid,
}
