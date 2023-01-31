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

use super::*;
use glib::{ParamSpec, Value};
use gtk::glib;
use std::cell::Cell;
use std::collections::BTreeSet;

glib::wrapper! {
    pub struct Contour(ObjectSubclass<imp::Contour>);
}

impl Default for Contour {
    fn default() -> Self {
        Self::new()
    }
}

impl Contour {
    pub const OPEN: &str = "open";
    pub const CONTINUITIES: &str = "continuities";
    pub const CONTINUITY: &str = "continuity";

    pub fn new() -> Self {
        let ret: Self = glib::Object::new::<Self>(&[]).unwrap();
        ret.imp().open.set(true);
        ret
    }

    pub fn curves(&self) -> &RefCell<Vec<Bezier>> {
        &self.imp().curves
    }

    pub fn push_curve(&self, curve: Bezier) {
        let mut curves = self.imp().curves.borrow_mut();
        let mut continuities = self.imp().continuities.borrow_mut();
        if curve.points().borrow().is_empty() {
            return;
        }
        if curves.is_empty() {
            curves.push(curve);
            return;
        }
        let prev = curves[curves.len() - 1].points().borrow();
        let curr = curve.points().borrow();
        if curve.property::<bool>(Bezier::SMOOTH) {
            continuities.push(Self::calc_smooth_continuity(
                <Vec<CurvePoint> as AsRef<[CurvePoint]>>::as_ref(&prev),
                <Vec<CurvePoint> as AsRef<[CurvePoint]>>::as_ref(&curr),
            ));
        } else {
            continuities.push(Continuity::Positional);
        }
        drop(curr);
        drop(prev);
        curves.push(curve);
    }

    pub fn close(&self) {
        if !self.imp().open.get() {
            return;
        }
        self.set_property(Contour::OPEN, false);

        let curves = self.imp().curves.borrow();
        let mut continuities = self.imp().continuities.borrow_mut();
        if curves.is_empty() {
            return;
        }
        let prev = curves[curves.len() - 1].points().borrow();
        let curr = curves[0].points().borrow();
        if curves[0].property::<bool>(Bezier::SMOOTH) {
            continuities.push(Self::calc_smooth_continuity(
                <Vec<CurvePoint> as AsRef<[CurvePoint]>>::as_ref(&prev),
                <Vec<CurvePoint> as AsRef<[CurvePoint]>>::as_ref(&curr),
            ));
        } else {
            continuities.push(Continuity::Positional);
        }
        assert_eq!(continuities.len(), curves.len());
    }

    fn calc_smooth_continuity(prev: &[CurvePoint], curr: &[CurvePoint]) -> Continuity {
        match (prev, curr) {
            (&[_, _, ref p2, ref p3_1], &[ref p3_2, ref p4, _, _])
                if p3_1.position == p3_2.position
                    && p2.position.collinear(&p3_1.position, &p4.position) =>
            {
                let beta =
                    (p4.position - p3_1.position).norm() / (p3_1.position - p2.position).norm();
                if beta == 1.0 || (beta.round() == 1.0 && (beta.fract() - 1.0).abs() < (1e-2 / 2.0))
                {
                    Continuity::Velocity
                } else {
                    debug_assert!(!beta.is_nan());
                    Continuity::Tangent { beta }
                }
            }
            (&[_, _, ref p2, ref p3_1], &[ref p3_2, ref p4, _, _])
            | (&[_, ref p2, ref p3_1], &[ref p3_2, ref p4, _, _])
            | (&[_, ref p2, ref p3_1], &[ref p3_2, ref p4, _])
            | (&[_, _, ref p2, ref p3_1], &[ref p3_2, ref p4, _])
                if p3_1.position == p3_2.position
                    && p4.position == 2.0 * p3_1.position - p2.position =>
            {
                Continuity::Velocity
            }
            _ => Continuity::Positional,
        }
    }

    pub fn reverse_direction(&self) {
        let mut curves = self.imp().curves.borrow_mut();
        curves.reverse();
        let mut continuities = self.imp().continuities.borrow_mut();
        continuities.reverse();
        for c in curves.iter_mut() {
            c.points().borrow_mut().reverse();
        }
    }

    pub fn transform_points(
        &self,
        contour_index: usize,
        idxs_slice: &[GlyphPointIndex],
        m: Matrix,
    ) -> Vec<(GlyphPointIndex, Point)> {
        let uuids = idxs_slice
            .iter()
            .filter(|i| i.contour_index == contour_index)
            .map(|i| (i.curve_index, i.uuid))
            .collect::<BTreeSet<_>>();
        let mut extra_uuids = uuids.clone();
        let curves_idxs = idxs_slice
            .iter()
            .filter(|i| i.contour_index == contour_index)
            .map(|i| i.curve_index)
            .collect::<BTreeSet<_>>();
        let mut updated_points = vec![];
        macro_rules! updated {
            ($b:expr, $point:expr) => {
                updated_points.push((
                    GlyphPointIndex {
                        contour_index,
                        curve_index: $b,
                        uuid: $point.uuid,
                    },
                    $point.position,
                ));
                extra_uuids.insert(($b, $point.uuid));
            };
        }
        let closed: bool = !self.imp().open.get();
        let continuities = self.imp().continuities.borrow();
        let curves = self.imp().curves.borrow();
        let prev_iter = curves
            .iter()
            .enumerate()
            .cycle()
            .skip(curves.len().saturating_sub(1));
        let curr_iter = curves.iter().enumerate().cycle();
        let next_iter = curves.iter().enumerate().cycle().skip(1);
        for (((prev_idx, prev), (curr_idx, curr)), (next_idx, next)) in prev_iter
            .zip(curr_iter)
            .zip(next_iter)
            .take(curves.len())
            .filter(|((_, (curr_idx, _)), _)| curves_idxs.contains(curr_idx))
        {
            let mut pts = curr.points().borrow_mut();
            let pts_len = pts.len();
            let pts_to_transform = pts
                .iter()
                .enumerate()
                .filter(|(_, p)| uuids.contains(&(curr_idx, p.uuid)))
                .map(|(i, p)| (i, p.uuid))
                .collect::<Vec<(usize, Uuid)>>();
            for (i, _uuid) in pts_to_transform {
                macro_rules! points_mut {
                    (prev) => {{
                        Some(prev.points().borrow_mut())
                        //if prev_idx == curr_idx {
                        //    None
                        //} else {
                        //    Some(prev.points().borrow_mut())
                        //}
                    }};
                    (next) => {{
                        Some(next.points().borrow_mut())
                        //if next_idx == curr_idx {
                        //    None
                        //} else {
                        //    Some(next.points().borrow_mut())
                        //}
                    }};
                }
                pts[i].position *= m;
                updated!(curr_idx, pts[i]);
                if i == 0 {
                    // Point is first oncurve point.
                    // also transform prev last oncurve point and its handle

                    /* Points handle if it's not quadratic */
                    {
                        if pts_len > 2 && !extra_uuids.contains(&(curr_idx, pts[1].uuid)) {
                            pts[1].position *= m;
                            updated!(curr_idx, pts[1]);
                        }
                    }
                    if closed || curr_idx != 0 {
                        if let Some(mut prev_points) = points_mut!(prev) {
                            let pts_len = prev_points.len();
                            assert!(!prev_points.is_empty());
                            /* previous curve's last oncurve point */
                            if !extra_uuids.contains(&(prev_idx, prev_points[pts_len - 1].uuid)) {
                                prev_points[pts_len - 1].position *= m;
                                updated!(prev_idx, prev_points[pts_len - 1]);
                            }
                            /* previous curve's last handle if it's not quadratic */
                            if pts_len > 2
                                && !extra_uuids.contains(&(prev_idx, prev_points[pts_len - 2].uuid))
                            {
                                prev_points[pts_len - 2].position *= m;
                                updated!(prev_idx, prev_points[pts_len - 2]);
                            }
                        }
                    }
                } else if i + 1 == pts_len {
                    // Point is last oncurve point.
                    // also transform next first oncurve point and its handle

                    /* Points handle if it's not quadratic */
                    {
                        if pts_len > 2 && !extra_uuids.contains(&(curr_idx, pts[i - 1].uuid)) {
                            pts[i - 1].position *= m;
                            updated!(curr_idx, pts[i - 1]);
                        }
                    }
                    if closed || curr_idx + 1 != curves.len() {
                        if let Some(mut next_points) = points_mut!(next) {
                            let pts_len = next_points.len();
                            assert!(!next_points.is_empty());
                            /* next first oncurve point */
                            if !extra_uuids.contains(&(next_idx, next_points[0].uuid)) {
                                next_points[0].position *= m;
                                updated!(next_idx, next_points[0]);
                            }
                            /* next curve's first handle if it's not quadratic */
                            if pts_len > 2
                                && !extra_uuids.contains(&(next_idx, next_points[1].uuid))
                            {
                                next_points[1].position *= m;
                                updated!(next_idx, next_points[1]);
                            }
                        }
                    }
                } else if closed || (next_idx + 1 != curves.len() && curr_idx != 0) {
                    // Point is handle.
                    // also transform neighbored handle if continuity constraints demand so
                    macro_rules! cont {
                        (between ($idx:expr) and next) => {{
                            if $idx + 1 == continuities.len() {
                                debug_assert!(closed);
                                debug_assert_eq!(continuities.len(), curves.len());
                                continuities[$idx - 1]
                            } else if $idx == 0 {
                                debug_assert!(closed);
                                continuities[curves.len() - 1]
                            } else {
                                continuities[$idx - 1]
                            }
                        }};
                    }
                    if i == 1 {
                        if let Some(mut prev_points) = points_mut!(prev) {
                            let pts_len = prev_points.len();
                            assert!(!prev_points.is_empty());
                            if pts_len > 2
                                && !extra_uuids.contains(&(prev_idx, prev_points[pts_len - 2].uuid))
                            {
                                match cont!(between (curr_idx) and next) {
                                    Continuity::Positional => {}
                                    Continuity::Velocity => {
                                        prev_points[pts_len - 2].position =
                                            pts[1].position.mirror(pts[0].position);
                                        updated!(prev_idx, prev_points[pts_len - 2]);
                                    }
                                    Continuity::Tangent { beta } => {
                                        let center = prev_points[pts_len - 1].position;
                                        let b_ = pts[1].position;
                                        assert_eq!(pts[0].position, center);
                                        let m_ = b_ - center;
                                        let n_ = 2.0 * center - (m_ / beta + center);
                                        prev_points[pts_len - 2].position = n_;
                                        updated!(prev_idx, prev_points[pts_len - 2]);
                                    }
                                }
                            }
                        }
                    } else if let Some(mut next_points) = points_mut!(next) {
                        let pts_len = next_points.len();
                        assert!(!next_points.is_empty());
                        if pts_len > 2 && !extra_uuids.contains(&(next_idx, next_points[1].uuid)) {
                            match cont!(between (next_idx) and next) {
                                Continuity::Positional => {}
                                Continuity::Velocity => {
                                    next_points[1].position =
                                        pts[i].position.mirror(pts[i + 1].position);
                                    updated!(next_idx, next_points[1]);
                                }
                                Continuity::Tangent { beta } => {
                                    let center = next_points[0].position;
                                    let n_ = pts[i].position - pts[i + 1].position;
                                    let m_ = 2.0 * center - (beta * n_ + center);
                                    next_points[1].position = m_;
                                    updated!(next_idx, next_points[1]);
                                }
                            }
                        }
                    }
                } else {
                    // Point is handle.
                    // Transform nothing else
                }
            }
        }
        updated_points
    }
}

mod imp {
    use super::*;
    #[derive(Default)]
    pub struct Contour {
        pub open: Cell<bool>,
        pub curves: RefCell<Vec<Bezier>>,
        pub continuities: RefCell<Vec<Continuity>>,
    }

    impl std::fmt::Debug for Contour {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            fmt.debug_struct("Contour")
                .field("open", &self.open.get())
                .field(
                    "curves",
                    &self
                        .curves
                        .borrow()
                        .iter()
                        .map(Bezier::imp)
                        .collect::<Vec<_>>(),
                )
                .field("continuities", &self.continuities.borrow())
                .finish()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Contour {
        const NAME: &'static str = "Contour";
        type Type = super::Contour;
        type ParentType = glib::Object;
        type Interfaces = ();
    }

    impl ObjectImpl for Contour {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: once_cell::sync::Lazy<Vec<ParamSpec>> =
                once_cell::sync::Lazy::new(|| {
                    vec![
                        glib::ParamSpecValueArray::new(
                            super::Contour::CONTINUITIES,
                            super::Contour::CONTINUITIES,
                            super::Contour::CONTINUITIES,
                            &glib::ParamSpecBoxed::new(
                                super::Contour::CONTINUITY,
                                super::Contour::CONTINUITY,
                                super::Contour::CONTINUITY,
                                Continuity::static_type(),
                                glib::ParamFlags::READWRITE,
                            ),
                            glib::ParamFlags::READWRITE,
                        ),
                        glib::ParamSpecBoolean::new(
                            super::Contour::OPEN,
                            super::Contour::OPEN,
                            super::Contour::OPEN,
                            true,
                            glib::ParamFlags::READWRITE | crate::UI_EDITABLE,
                        ),
                    ]
                });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> glib::Value {
            match pspec.name() {
                super::Contour::CONTINUITIES => {
                    let continuities = self.continuities.borrow();
                    let mut ret = glib::ValueArray::new(continuities.len() as u32);
                    for c in continuities.iter() {
                        ret.append(&c.to_value());
                    }
                    ret.to_value()
                }
                super::Contour::OPEN => self.open.get().to_value(),
                _ => unimplemented!("{}", pspec.name()),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                super::Contour::CONTINUITIES => {
                    let arr: glib::ValueArray = value.get().unwrap();
                    let mut continuities = self.continuities.borrow_mut();
                    continuities.clear();
                    for c in arr.iter() {
                        continuities.push(c.get().unwrap());
                    }
                }
                super::Contour::OPEN => {
                    self.open.set(value.get().unwrap());
                }
                _ => unimplemented!("{}", pspec.name()),
            }
        }
    }
}
