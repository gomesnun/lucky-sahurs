//! Port of pygame-ce 2.5.8 src_c/draw.c (the primitives the game uses).
//! Pixels are written raw (no blending), exactly like pygame.draw.

#![allow(clippy::too_many_arguments)]

use super::rect::Rect;
use super::surface::{Color, Surface};

struct Ctx<'a> {
    s: &'a mut Surface,
    clip: Rect,
    color: u32,
}

impl Ctx<'_> {
    #[inline]
    fn set_at(&mut self, x: i32, y: i32) {
        let c = self.clip;
        if x < c.x || x >= c.x + c.w || y < c.y || y >= c.y + c.h {
            return;
        }
        let w = self.s.w;
        self.s.px[(y * w + x) as usize] = self.color;
    }
    #[inline]
    fn unsafe_set_at(&mut self, x: i32, y: i32) {
        let w = self.s.w;
        self.s.px[(y * w + x) as usize] = self.color;
    }
    #[inline]
    fn horz(&mut self, x1: i32, y1: i32, x2: i32) {
        let w = self.s.w;
        let row = (y1 * w) as usize;
        let color = self.color;
        self.s.px[row + x1 as usize..=row + x2 as usize].fill(color);
    }
    #[inline]
    fn vert(&mut self, y1: i32, x1: i32, y2: i32) {
        let w = self.s.w;
        let color = self.color;
        for y in y1..=y2 {
            self.s.px[(y * w + x1) as usize] = color;
        }
    }
    fn horz_clip(&mut self, x1: i32, y1: i32, x2: i32) {
        let c = self.clip;
        if y1 < c.y || y1 >= c.y + c.h {
            return;
        }
        let (mut x1, mut x2) = if x2 < x1 { (x2, x1) } else { (x1, x2) };
        x1 = x1.max(c.x);
        x2 = x2.min(c.x + c.w - 1);
        if x2 < c.x || x1 >= c.x + c.w {
            return;
        }
        if x1 == x2 {
            self.set_at(x1, y1);
            return;
        }
        self.horz(x1, y1, x2);
    }
    // the "bounding" variants only differ by the drawn-area bookkeeping, which the game never reads
    #[inline]
    fn horz_bound(&mut self, x1: i32, y1: i32, x2: i32) {
        self.horz_clip(x1, y1, x2);
    }
}

fn draw_line_thin(ctx: &mut Ctx, mut x1: i32, mut y1: i32, x2: i32, y2: i32) {
    if x1 == x2 && y1 == y2 {
        ctx.set_at(x1, y1);
        return;
    }
    if y1 == y2 {
        let dx = if x1 < x2 { 1 } else { -1 };
        for sx in 0..=(x1 - x2).abs() {
            ctx.set_at(x1 + dx * sx, y1);
        }
        return;
    }
    if x1 == x2 {
        let dy = if y1 < y2 { 1 } else { -1 };
        for sy in 0..=(y1 - y2).abs() {
            ctx.set_at(x1, y1 + dy * sy);
        }
        return;
    }
    let dx = (x2 - x1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let dy = (y2 - y1).abs();
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = (if dx > dy { dx } else { -dy }) / 2;
    while x1 != x2 || y1 != y2 {
        ctx.set_at(x1, y1);
        let e2 = err;
        if e2 > -dx {
            err -= dy;
            x1 += sx;
        }
        if e2 < dy {
            err += dx;
            y1 += sy;
        }
    }
    ctx.set_at(x2, y2);
}

fn clip_line(clip: Rect, x1: i32, y1: i32, x2: i32, y2: i32, width: i32, xinc: bool) -> bool {
    let (left, right, top, bottom) = if xinc {
        (x1.min(x2) - width, x1.max(x2) + width, y1.min(y2), y1.max(y2))
    } else {
        (x1.min(x2), x1.max(x2), y1.min(y2) - width, y1.max(y2) + width)
    };
    !(clip.x > right || clip.y > bottom || clip.x + clip.w <= left || clip.y + clip.h <= top)
}

fn draw_line_width(ctx: &mut Ctx, mut x1: i32, mut y1: i32, x2: i32, y2: i32, width: i32) {
    let clip = ctx.clip;
    let end_x = clip.x + clip.w - 1;
    let end_y = clip.y + clip.h - 1;
    let extra_width = 1 - (width % 2);
    if width < 1 {
        return;
    }
    if width == 1 {
        draw_line_thin(ctx, x1, y1, x2, y2);
        return;
    }
    let width = width / 2;
    let xinc = (x1 - x2).abs() <= (y1 - y2).abs();
    if !clip_line(clip, x1, y1, x2, y2, width, xinc) {
        return;
    }
    if x1 == x2 && y1 == y2 {
        let start = ((x1 - width) + extra_width).max(clip.x);
        let end = end_x.min(x1 + width);
        if start <= end {
            ctx.horz(start, y1, end);
        }
        return;
    }
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    let sx = if x2 > x1 { 1 } else { -1 };
    let sy = if y2 > y1 { 1 } else { -1 };
    let mut err = (if dx > dy { dx } else { -dy }) / 2;
    if xinc {
        while y1 != y2 + sy {
            if clip.y <= y1 && y1 <= end_y {
                let start = ((x1 - width) + extra_width).max(clip.x);
                let end = end_x.min(x1 + width);
                if start <= end {
                    ctx.horz(start, y1, end);
                }
            }
            let e2 = err;
            if e2 > -dx {
                err -= dy;
                x1 += sx;
            }
            if e2 < dy {
                err += dx;
                y1 += sy;
            }
        }
    } else {
        while x1 != x2 + sx {
            if clip.x <= x1 && x1 <= end_x {
                let start = ((y1 - width) + extra_width).max(clip.y);
                let end = end_y.min(y1 + width);
                if start <= end {
                    ctx.vert(start, x1, end);
                }
            }
            let e2 = err;
            if e2 > -dx {
                err -= dy;
                x1 += sx;
            }
            if e2 < dy {
                err += dx;
                y1 += sy;
            }
        }
    }
}

fn draw_filltri(ctx: &mut Ctx, xs: &[i32], ys: &[i32]) {
    let (mut p0x, mut p1x, mut p2x) = (xs[0], xs[1], xs[2]);
    let (mut p0y, mut p1y, mut p2y) = (ys[0], ys[1], ys[2]);
    fn swap(ax: &mut i32, ay: &mut i32, bx: &mut i32, by: &mut i32) {
        std::mem::swap(ax, bx);
        std::mem::swap(ay, by);
    }
    if p1y < p0y {
        swap(&mut p1x, &mut p1y, &mut p0x, &mut p0y);
    }
    if p2y < p1y {
        swap(&mut p1x, &mut p1y, &mut p2x, &mut p2y);
        if p1y < p0y {
            swap(&mut p1x, &mut p1y, &mut p0x, &mut p0y);
        }
    }
    if p0y == p1y && p1y == p2y && p0x == p1x && p1x != p2x {
        swap(&mut p1x, &mut p1y, &mut p2x, &mut p2y);
    }
    let d1 = ((p2x - p0x) as f64 / ((p2y - p0y) as f64 + 1e-17)) as f32;
    let d2 = ((p1x - p0x) as f64 / ((p1y - p0y) as f64 + 1e-17)) as f32;
    let d3 = ((p2x - p1x) as f64 / ((p2y - p1y) as f64 + 1e-17)) as f32;
    for y in p0y..=p2y {
        let x1 = p0x + ((y - p0y) as f32 * d1) as i32;
        let x2 = if y < p1y { p0x + ((y - p0y) as f32 * d2) as i32 } else { p1x + ((y - p1y) as f32 * d3) as i32 };
        ctx.horz_bound(x1, y, x2);
    }
}

fn draw_fillpoly(ctx: &mut Ctx, px: &[i32], py: &[i32]) {
    let n = px.len();
    let mut miny = py[0];
    let mut maxy = py[0];
    for &y in &py[1..] {
        miny = miny.min(y);
        maxy = maxy.max(y);
    }
    if miny == maxy {
        let mut minx = px[0];
        let mut maxx = px[0];
        for &x in &px[1..] {
            minx = minx.min(x);
            maxx = maxx.max(x);
        }
        ctx.horz_bound(minx, miny, maxx);
        return;
    }
    let mut xi: Vec<i32> = Vec::with_capacity(n);
    for y in miny..=maxy {
        xi.clear();
        for i in 0..n {
            let ip = if i > 0 { i - 1 } else { n - 1 };
            let (mut y1, mut y2) = (py[ip], py[i]);
            let (x1, x2);
            if y1 < y2 {
                x1 = px[ip];
                x2 = px[i];
            } else if y1 > y2 {
                y2 = py[ip];
                y1 = py[i];
                x2 = px[ip];
                x1 = px[i];
            } else {
                continue;
            }
            if (y >= y1 && y < y2) || (y == maxy && y2 == maxy) {
                let mut inter = ((y - y1) * (x2 - x1)) as f32 / (y2 - y1) as f32;
                inter = if xi.len() % 2 == 0 { inter.floor() } else { inter.ceil() };
                xi.push(inter as i32 + x1);
            }
        }
        xi.sort_unstable();
        let mut i = 0;
        while i + 1 < xi.len() {
            ctx.horz_bound(xi[i], y, xi[i + 1]);
            i += 2;
        }
    }
    for i in 0..n {
        let ip = if i > 0 { i - 1 } else { n - 1 };
        let y = py[i];
        if miny < y && py[ip] == y && y < maxy {
            ctx.horz_bound(px[i], y, px[ip]);
        }
    }
}

fn draw_circle_filled(ctx: &mut Ctx, x0: i32, y0: i32, radius: i32) {
    let mut f = 1 - radius;
    let mut ddf_x = 0;
    let mut ddf_y = -2 * radius;
    let mut x = 0;
    let mut y = radius;
    let xmax = if x0 < 0 { x0.wrapping_add(i32::MAX).wrapping_add(1) } else { i32::MAX - x0 };
    while x < y {
        if f >= 0 {
            y -= 1;
            ddf_y += 2;
            f += ddf_y;
        }
        x += 1;
        ddf_x += 2;
        f += ddf_x + 1;
        if f >= 0 {
            ctx.horz_bound(x0 - x, y0 + y - 1, x0 + (x - 1).min(xmax));
            ctx.horz_bound(x0 - x, y0 - y, x0 + (x - 1).min(xmax));
        }
        ctx.horz_bound(x0 - y, y0 + x - 1, x0 + (y - 1).min(xmax));
        ctx.horz_bound(x0 - y, y0 - x, x0 + (y - 1).min(xmax));
    }
}

fn draw_circle_bresenham_thin(ctx: &mut Ctx, x0: i32, y0: i32, radius: i32) {
    let mut f = 1 - radius;
    let mut ddf_x = 0;
    let mut ddf_y = -2 * radius;
    let mut x = 0;
    let mut y = radius;
    while x < y {
        if f >= 0 {
            y -= 1;
            ddf_y += 2;
            f += ddf_y;
        }
        x += 1;
        ddf_x += 2;
        f += ddf_x + 1;
        ctx.set_at(x0 + x - 1, y0 + y - 1);
        ctx.set_at(x0 - x, y0 + y - 1);
        ctx.set_at(x0 + x - 1, y0 - y);
        ctx.set_at(x0 - x, y0 - y);
        ctx.set_at(x0 + y - 1, y0 + x - 1);
        ctx.set_at(x0 + y - 1, y0 - x);
        ctx.set_at(x0 - y, y0 + x - 1);
        ctx.set_at(x0 - y, y0 - x);
    }
}

fn draw_circle_bresenham(ctx: &mut Ctx, x0: i32, y0: i32, radius: i32, thickness: i32) {
    let mut x: i64 = 0;
    let mut y: i64 = radius as i64;
    let radius_squared: i64 = (radius * radius) as i64;
    let double_radius_squared = 2 * radius_squared;
    let mut d1: f64 = radius_squared as f64 * (1.25 - radius as f64);
    let mut dx: i64 = 0;
    let mut dy: i64 = double_radius_squared * y;

    let mut line = true;
    let radius_inner: i64 = (radius - thickness + 1) as i64;
    let mut x_inner: i64 = 0;
    let mut y_inner: i64 = radius_inner;
    let radius_inner_squared = radius_inner * radius_inner;
    let double_radius_inner_squared = 2 * radius_inner_squared;
    let mut d1_inner: f64 = radius_inner_squared as f64 * (1.25 - radius_inner as f64);
    let mut d2_inner: f64 = 0.0;
    let mut dx_inner: i64 = 0;
    let mut dy_inner: i64 = double_radius_inner_squared * y_inner;

    let emit = |ctx: &mut Ctx, line: bool, x: i64, y: i64, x_inner: i64| {
        let (x, y, xi) = (x as i32, y as i32, x_inner as i32);
        if line {
            ctx.horz_bound(x0 - x, y0 - y, x0 + x - 1);
            ctx.horz_bound(x0 - x, y0 + y - 1, x0 + x - 1);
        } else {
            ctx.horz_bound(x0 - x, y0 - y, x0 - xi);
            ctx.horz_bound(x0 - x, y0 + y - 1, x0 - xi);
            ctx.horz_bound(x0 + xi - 1, y0 - y, x0 + x - 1);
            ctx.horz_bound(x0 + xi - 1, y0 + y - 1, x0 + x - 1);
        }
    };

    while dx < dy {
        while d1 < 0.0 {
            x += 1;
            dx += double_radius_squared;
            d1 += (dx + radius_squared) as f64;
        }
        emit(ctx, line, x, y, x_inner);
        x += 1;
        y -= 1;
        dx += double_radius_squared;
        dy -= double_radius_squared;
        d1 += (dx - dy + radius_squared) as f64;
        if line && y < radius_inner {
            line = false;
        }
        if !line {
            while d1_inner < 0.0 {
                x_inner += 1;
                dx_inner += double_radius_inner_squared;
                d1_inner += (dx_inner + radius_inner_squared) as f64;
            }
            x_inner += 1;
            y_inner -= 1;
            dx_inner += double_radius_inner_squared;
            dy_inner -= double_radius_inner_squared;
            d1_inner += (dx_inner - dy_inner + radius_inner_squared) as f64;
        }
    }
    d1 = radius_squared as f64
        * ((x as f64 + 0.5) * (x as f64 + 0.5) + ((y - 1) * (y - 1)) as f64 - radius_squared as f64);
    while y >= 0 {
        emit(ctx, line, x, y, x_inner);
        if d1 > 0.0 {
            y -= 1;
            dy -= double_radius_squared;
            d1 += (radius_squared - dy) as f64;
        } else {
            y -= 1;
            x += 1;
            dx += double_radius_squared;
            dy -= double_radius_squared;
            d1 += (dx - dy + radius_squared) as f64;
        }
        if line && y < radius_inner {
            line = false;
        }
        if !line {
            if dx_inner < dy_inner {
                while d1_inner < 0.0 {
                    x_inner += 1;
                    dx_inner += double_radius_inner_squared;
                    d1_inner += (dx_inner + radius_inner_squared) as f64;
                }
                x_inner += 1;
                y_inner -= 1;
                dx_inner += double_radius_inner_squared;
                dy_inner -= double_radius_inner_squared;
                d1_inner += (dx_inner - dy_inner + radius_inner_squared) as f64;
            } else {
                if d2_inner == 0.0 {
                    d2_inner = radius_inner_squared as f64
                        * ((x_inner as f64 + 0.5) * (x_inner as f64 + 0.5)
                            + ((y_inner - 1) * (y_inner - 1)) as f64
                            - radius_inner_squared as f64);
                }
                if d2_inner > 0.0 {
                    y_inner -= 1;
                    dy_inner -= double_radius_inner_squared;
                    d2_inner += (radius_inner_squared - dy_inner) as f64;
                } else {
                    y_inner -= 1;
                    x_inner += 1;
                    dx_inner += double_radius_inner_squared;
                    dy_inner -= double_radius_inner_squared;
                    d2_inner += (dx_inner - dy_inner + radius_inner_squared) as f64;
                }
            }
        }
    }
}

fn draw_circle_quadrant(
    ctx: &mut Ctx,
    x0: i32,
    y0: i32,
    radius: i32,
    mut thickness: i32,
    top_right: bool,
    top_left: bool,
    bottom_left: bool,
    bottom_right: bool,
) {
    let mut f = 1 - radius;
    let mut ddf_x = 0;
    let mut ddf_y = -2 * radius;
    let mut x = 0;
    let mut y = radius;
    let mut i_y = radius - thickness;
    let mut i_f = 1 - i_y;
    let mut i_ddf_x = 0;
    let mut i_ddf_y = -2 * i_y;
    if radius == 1 {
        if top_right {
            ctx.set_at(x0, y0 - 1);
        }
        if top_left {
            ctx.set_at(x0 - 1, y0 - 1);
        }
        if bottom_left {
            ctx.set_at(x0 - 1, y0);
        }
        if bottom_right {
            ctx.set_at(x0, y0);
        }
        return;
    }
    if thickness != 0 {
        while x < y {
            if f >= 0 {
                y -= 1;
                ddf_y += 2;
                f += ddf_y;
            }
            if i_f >= 0 {
                i_y -= 1;
                i_ddf_y += 2;
                i_f += i_ddf_y;
            }
            x += 1;
            ddf_x += 2;
            f += ddf_x + 1;
            i_ddf_x += 2;
            i_f += i_ddf_x + 1;
            if thickness > 1 {
                thickness = y - i_y;
            }
            if top_right {
                for i in 0..thickness {
                    let y1 = y - i;
                    if (y0 - y1) < (y0 - x) {
                        ctx.set_at(x0 + x - 1, y0 - y1);
                    }
                    if (x0 + y1 - 1) >= (x0 + x - 1) {
                        ctx.set_at(x0 + y1 - 1, y0 - x);
                    }
                }
            }
            if top_left {
                for i in 0..thickness {
                    let y1 = y - i;
                    if (y0 - y1) <= (y0 - x) {
                        ctx.set_at(x0 - x, y0 - y1);
                    }
                    if (x0 - y1) < (x0 - x) {
                        ctx.set_at(x0 - y1, y0 - x);
                    }
                }
            }
            if bottom_left {
                for i in 0..thickness {
                    let y1 = y - i;
                    if (x0 - y1) <= (x0 - x) {
                        ctx.set_at(x0 - y1, y0 + x - 1);
                    }
                    if (y0 + y1 - 1) > (y0 + x - 1) {
                        ctx.set_at(x0 - x, y0 + y1 - 1);
                    }
                }
            }
            if bottom_right {
                for i in 0..thickness {
                    let y1 = y - i;
                    if (y0 + y1 - 1) >= (y0 + x - 1) {
                        ctx.set_at(x0 + x - 1, y0 + y1 - 1);
                    }
                    if (x0 + y1 - 1) > (x0 + x - 1) {
                        ctx.set_at(x0 + y1 - 1, y0 + x - 1);
                    }
                }
            }
        }
    } else {
        while x < y {
            if f >= 0 {
                y -= 1;
                ddf_y += 2;
                f += ddf_y;
            }
            x += 1;
            ddf_x += 2;
            f += ddf_x + 1;
            if top_right {
                for y1 in (y0 - x)..=y0 {
                    ctx.set_at(x0 + y - 1, y1);
                }
                for y1 in (y0 - y)..=y0 {
                    ctx.set_at(x0 + x - 1, y1);
                }
            }
            if top_left {
                for y1 in (y0 - x)..=y0 {
                    ctx.set_at(x0 - y, y1);
                }
                for y1 in (y0 - y)..=y0 {
                    ctx.set_at(x0 - x, y1);
                }
            }
            if bottom_left {
                for y1 in y0..(y0 + x) {
                    ctx.set_at(x0 - y, y1);
                }
                for y1 in y0..(y0 + y) {
                    ctx.set_at(x0 - x, y1);
                }
            }
            if bottom_right {
                for y1 in y0..(y0 + x) {
                    ctx.set_at(x0 + y - 1, y1);
                }
                for y1 in y0..(y0 + y) {
                    ctx.set_at(x0 + x - 1, y1);
                }
            }
        }
    }
}

fn draw_rect_outline(ctx: &mut Ctx, x1: i32, y1: i32, x2: i32, y2: i32, width: i32) {
    for i in 0..width {
        ctx.horz_clip(x1, y1 + i, x2);
        ctx.horz_clip(x1, y2 - i, x2);
    }
    for i in 0..((y2 - y1) - 2 * width + 1) {
        ctx.horz_clip(x1, y1 + width + i, x1 + width - 1);
        ctx.horz_clip(x2 - width + 1, y1 + width + i, x2);
    }
}

fn draw_round_rect(ctx: &mut Ctx, x1: i32, y1: i32, x2: i32, y2: i32, radius: i32, width: i32) {
    let (mut tl, mut tr, mut bl, mut br) = (radius, radius, radius, radius);
    if (tl + tr) > (x2 - x1 + 1) || (bl + br) > (x2 - x1 + 1) || (tl + bl) > (y2 - y1 + 1) || (tr + br) > (y2 - y1 + 1) {
        let q_top = (x2 - x1 + 1) as f32 / (tl + tr) as f32;
        let q_left = (y2 - y1 + 1) as f32 / (tl + bl) as f32;
        let q_bottom = (x2 - x1 + 1) as f32 / (bl + br) as f32;
        let q_right = (y2 - y1 + 1) as f32 / (tr + br) as f32;
        let f = q_top.min(q_left).min(q_bottom).min(q_right);
        tl = (tl as f32 * f) as i32;
        tr = (tr as f32 * f) as i32;
        bl = (bl as f32 * f) as i32;
        br = (br as f32 * f) as i32;
    }
    if width == 0 {
        let xs = [x1, x1 + tl, x2 - tr, x2, x2, x2 - br, x1 + bl, x1];
        let ys = [y1 + tl, y1, y1, y1 + tr, y2 - br, y2, y2, y2 - bl];
        draw_fillpoly(ctx, &xs, &ys);
        draw_circle_quadrant(ctx, x2 - tr + 1, y1 + tr, tr, 0, true, false, false, false);
        draw_circle_quadrant(ctx, x1 + tl, y1 + tl, tl, 0, false, true, false, false);
        draw_circle_quadrant(ctx, x1 + bl, y2 - bl + 1, bl, 0, false, false, true, false);
        draw_circle_quadrant(ctx, x2 - br + 1, y2 - br + 1, br, 0, false, false, false, true);
    } else {
        let half = width / 2;
        if x2 - tr == x1 + tl {
            for i in 0..width {
                ctx.set_at(x1 + tl, y1 + i);
            }
        } else {
            draw_line_width(ctx, x1 + tl, y1 + half - 1 + width % 2, x2 - tr, y1 + half - 1 + width % 2, width);
        }
        if y2 - bl == y1 + tl {
            for i in 0..width {
                ctx.set_at(x1 + i, y1 + tl);
            }
        } else {
            draw_line_width(ctx, x1 + half - 1 + width % 2, y1 + tl, x1 + half - 1 + width % 2, y2 - bl, width);
        }
        if x2 - br == x1 + bl {
            for i in 0..width {
                ctx.set_at(x1 + bl, y2 - i);
            }
        } else {
            draw_line_width(ctx, x1 + bl, y2 - half, x2 - br, y2 - half, width);
        }
        if y2 - br == y1 + tr {
            for i in 0..width {
                ctx.set_at(x2 - i, y1 + tr);
            }
        } else {
            draw_line_width(ctx, x2 - half, y1 + tr, x2 - half, y2 - br, width);
        }
        draw_circle_quadrant(ctx, x2 - tr + 1, y1 + tr, tr, width, true, false, false, false);
        draw_circle_quadrant(ctx, x1 + tl, y1 + tl, tl, width, false, true, false, false);
        draw_circle_quadrant(ctx, x1 + bl, y2 - bl + 1, bl, width, false, false, true, false);
        draw_circle_quadrant(ctx, x2 - br + 1, y2 - br + 1, br, width, false, false, false, true);
    }
}

fn draw_ellipse_filled(ctx: &mut Ctx, x0: i32, y0: i32, width: i32, height: i32) {
    if width == 1 {
        draw_line_thin(ctx, x0, y0, x0, y0 + height - 1);
        return;
    }
    if height == 1 {
        ctx.horz_bound(x0, y0, x0 + width - 1);
        return;
    }
    let x0 = x0 + width / 2;
    let y0 = y0 + height / 2;
    let x_offset = (width + 1) % 2;
    let y_offset = (height + 1) % 2;
    let width = (width / 2) as i64;
    let height = (height / 2) as i64;
    let mut x: i64 = 0;
    let mut y: i64 = height;
    let mut d1: f64 = ((height * height) - (width * width * height)) as f64 + (0.25 * (width * width) as f64);
    let mut dx: i64 = 2 * height * height * x;
    let mut dy: i64 = 2 * width * width * y;
    while dx < dy {
        ctx.horz_bound(x0 - x as i32, y0 - y as i32, x0 + x as i32 - x_offset);
        ctx.horz_bound(x0 - x as i32, y0 + y as i32 - y_offset, x0 + x as i32 - x_offset);
        if d1 < 0.0 {
            x += 1;
            dx += 2 * height * height;
            d1 += (dx + height * height) as f64;
        } else {
            x += 1;
            y -= 1;
            dx += 2 * height * height;
            dy -= 2 * width * width;
            d1 += (dx - dy + height * height) as f64;
        }
    }
    let mut d2: f64 = ((height * height) as f64 * ((x as f64 + 0.5) * (x as f64 + 0.5)))
        + ((width * width) as f64 * ((y - 1) * (y - 1)) as f64)
        - (width * width * height * height) as f64;
    while y >= 0 {
        ctx.horz_bound(x0 - x as i32, y0 - y as i32, x0 + x as i32 - x_offset);
        ctx.horz_bound(x0 - x as i32, y0 + y as i32 - y_offset, x0 + x as i32 - x_offset);
        if d2 > 0.0 {
            y -= 1;
            dy -= 2 * width * width;
            d2 += (width * width - dy) as f64;
        } else {
            y -= 1;
            x += 1;
            dx += 2 * height * height;
            dy -= 2 * width * width;
            d2 += (dx - dy + width * width) as f64;
        }
    }
}

fn draw_ellipse_thickness(ctx: &mut Ctx, x0: i32, y0: i32, width: i32, height: i32, thickness: i32) {
    let x0 = x0 + width / 2;
    let y0 = y0 + height / 2;
    let x_offset = (width + 1) % 2;
    let y_offset = (height + 1) % 2;
    let width = (width / 2) as i64;
    let height = (height / 2) as i64;
    let t = thickness as i64;
    let mut line = true;
    let mut x: i64 = 0;
    let mut y: i64 = height;
    let mut x_inner: i64 = 0;
    let mut y_inner: i64 = height - t;
    let mut d1: f64 = ((height * height) - (width * width * height)) as f64 + (0.25 * (width * width) as f64);
    let hi = height - t;
    let wi = width - t;
    let mut d1_inner: f64 = ((hi * hi) - (wi * wi * hi)) as f64 + (0.25 * (wi * wi) as f64);
    let mut dx: i64 = 2 * height * height * x;
    let mut dy: i64 = 2 * width * width * y;
    let mut dx_inner: i64 = 2 * hi * hi * x_inner;
    let mut dy_inner: i64 = 2 * wi * wi * y_inner;
    let mut d2_inner: f64 = 0.0;

    let emit = |ctx: &mut Ctx, line: bool, x: i64, y: i64, x_inner: i64| {
        let (x, y, xi) = (x as i32, y as i32, x_inner as i32);
        if line {
            ctx.horz_bound(x0 - x, y0 - y, x0 + x - x_offset);
            ctx.horz_bound(x0 - x, y0 + y - y_offset, x0 + x - x_offset);
        } else {
            ctx.horz_bound(x0 - x, y0 - y, x0 - xi);
            ctx.horz_bound(x0 - x, y0 + y - y_offset, x0 - xi);
            ctx.horz_bound(x0 + x - x_offset, y0 - y, x0 + xi - x_offset);
            ctx.horz_bound(x0 + x - x_offset, y0 + y - y_offset, x0 + xi - x_offset);
        }
    };

    while dx < dy {
        emit(ctx, line, x, y, x_inner);
        if d1 < 0.0 {
            x += 1;
            dx += 2 * height * height;
            d1 += (dx + height * height) as f64;
        } else {
            x += 1;
            y -= 1;
            dx += 2 * height * height;
            dy -= 2 * width * width;
            d1 += (dx - dy + height * height) as f64;
            if line && y < height - t {
                line = false;
            }
            if !line && dx_inner < dy_inner {
                while d1_inner < 0.0 {
                    x_inner += 1;
                    dx_inner += 2 * hi * hi;
                    d1_inner += (dx_inner + hi * hi) as f64;
                }
                x_inner += 1;
                y_inner -= 1;
                dx_inner += 2 * hi * hi;
                dy_inner -= 2 * wi * wi;
                d1_inner += (dx_inner - dy_inner + hi * hi) as f64;
            }
        }
    }
    let mut d2: f64 = ((height * height) as f64 * ((x as f64 + 0.5) * (x as f64 + 0.5)))
        + ((width * width) as f64 * ((y - 1) * (y - 1)) as f64)
        - (width * width * height * height) as f64;
    while y >= 0 {
        emit(ctx, line, x, y, x_inner);
        if d2 > 0.0 {
            y -= 1;
            dy -= 2 * width * width;
            d2 += (width * width - dy) as f64;
        } else {
            y -= 1;
            x += 1;
            dx += 2 * height * height;
            dy -= 2 * width * width;
            d2 += (dx - dy + width * width) as f64;
        }
        if line && y < height - t {
            line = false;
        }
        if !line {
            if dx_inner < dy_inner {
                while d1_inner < 0.0 {
                    x_inner += 1;
                    dx_inner += 2 * hi * hi;
                    d1_inner += (dx_inner + hi * hi) as f64;
                }
                x_inner += 1;
                y_inner -= 1;
                dx_inner += 2 * hi * hi;
                dy_inner -= 2 * wi * wi;
                d1_inner += (dx_inner - dy_inner + hi * hi) as f64;
            } else if y_inner >= 0 {
                if d2_inner == 0.0 {
                    d2_inner = ((hi as f64) * hi as f64) * ((x_inner as f64 + 0.5) * (x_inner as f64 + 0.5))
                        + ((wi as f64) * wi as f64) * (((y_inner - 1) * (y_inner - 1)) as f64)
                        - ((wi as f64) * wi as f64 * hi as f64 * hi as f64);
                }
                if d2_inner > 0.0 {
                    y_inner -= 1;
                    dy_inner -= 2 * wi * wi;
                    d2_inner += (wi * wi - dy_inner) as f64;
                } else {
                    y_inner -= 1;
                    x_inner += 1;
                    dx_inner += 2 * hi * hi;
                    dy_inner -= 2 * wi * wi;
                    d2_inner += (dx_inner - dy_inner + wi * wi) as f64;
                }
            }
        }
    }
}

fn check_pixel_in_arc(x: i32, y: i32, min_dot: f64, ir1: f64, ir2: f64, iir1: f64, iir2: f64, xm: f64, ym: f64) -> bool {
    let (xf, yf) = (x as f64, y as f64);
    if (x * x) as f64 * ir1 + (y * y) as f64 * ir2 > 1.0 {
        return false;
    }
    if (x * x) as f64 * iir1 + (y * y) as f64 * iir2 < 1.0 {
        return false;
    }
    xf * xm + yf * ym >= min_dot * ((x * x + y * y) as f64).sqrt()
}

fn draw_arc(ctx: &mut Ctx, xc: i32, yc: i32, r1: i32, r2: i32, width: i32, a_start: f64, mut a_stop: f64) {
    if width <= 0 {
        return;
    }
    if a_stop < a_start {
        a_stop += 2.0 * std::f64::consts::PI;
    }
    if a_stop <= a_start {
        return;
    }
    let a_mid = 0.5 * (a_start + a_stop);
    let a_dist = a_mid - a_start;
    let xm = a_mid.cos();
    let ym = -a_mid.sin();
    let ir1_i = r1 - width;
    let ir2_i = r2 - width;
    let ir1 = 1.0 / (r1 * r1) as f64;
    let ir2 = 1.0 / (r2 * r2) as f64;
    let iir1 = 1.0 / (ir1_i * ir1_i) as f64;
    let iir2 = 1.0 / (ir2_i * ir2_i) as f64;
    let min_dot = if a_dist < std::f64::consts::PI { (a_mid - a_start).cos() } else { -1.0 };

    // bounds (calc_arc_bounds)
    let clip = ctx.clip;
    let xs = a_start.cos();
    let ys = -a_start.sin();
    let xe = a_stop.cos();
    let ye = -a_stop.sin();
    let xsi = (xs * ir1_i as f64 + 0.5) as i32;
    let ysi = (ys * ir2_i as f64 + 0.5) as i32;
    let xei = (xe * ir1_i as f64 + 0.5) as i32;
    let yei = (ye * ir2_i as f64 + 0.5) as i32;
    let xso = (xs * r1 as f64 + 0.5) as i32;
    let yso = (ys * r2 as f64 + 0.5) as i32;
    let xeo = (xe * r1 as f64 + 0.5) as i32;
    let yeo = (ye * r2 as f64 + 0.5) as i32;
    let mut minx = -r1;
    if -xm < min_dot {
        minx = xsi.min(xei).min(xso.min(xeo));
    }
    minx = minx.max(clip.x - xc);
    let mut miny = -r2;
    if -ym < min_dot {
        miny = ysi.min(yei).min(yso.min(yeo));
    }
    miny = miny.max(clip.y - yc);
    let mut maxx = r1;
    if xm < min_dot {
        maxx = xsi.max(xei).max(xso.max(xeo));
    }
    maxx = maxx.min(clip.x + clip.w - xc - 1);
    let mut maxy = r2;
    if ym < min_dot {
        maxy = ysi.max(yei).max(yso.max(yeo));
    }
    maxy = maxy.min(clip.y + clip.h - yc - 1);
    if minx >= maxx || miny >= maxy {
        return;
    }
    let chk = |x: i32, y: i32| check_pixel_in_arc(x, y, min_dot, ir1, ir2, iir1, iir2, xm, ym);
    loop {
        if miny >= maxy {
            return;
        }
        if (minx..=maxx).any(|x| chk(x, miny)) {
            break;
        }
        miny += 1;
    }
    loop {
        if maxy <= miny {
            return;
        }
        if (minx..=maxx).any(|x| chk(x, maxy)) {
            break;
        }
        maxy -= 1;
    }
    loop {
        if minx >= maxx {
            return;
        }
        if (miny..=maxy).any(|y| chk(minx, y)) {
            break;
        }
        minx += 1;
    }
    loop {
        if minx >= maxx {
            return;
        }
        if (miny..=maxy).any(|y| chk(maxx, y)) {
            break;
        }
        maxx -= 1;
    }
    if minx >= maxx || miny >= maxy {
        return;
    }
    let max_required_y = maxy.max(-miny);
    for y in 0..=max_required_y {
        let pos_y = y >= miny && y <= maxy;
        let neg_y = -y >= miny && -y <= maxy;
        let y2 = y * y;
        let x_outer = (r1 as f64 * (1.0 - y2 as f64 * ir2).sqrt()) as i32;
        let mut x_inner = 0;
        if y < ir2_i {
            x_inner = (ir1_i as f64 * (1.0 - y2 as f64 * iir2).sqrt()) as i32;
        }
        let py = yc + y;
        let ny = yc - y;
        let y_dot = y as f64 * ym;
        for x in x_inner..=x_outer {
            let pos_x = x >= minx && x <= maxx;
            let neg_x = -x >= minx && -x <= maxx;
            if !(pos_x || neg_x) {
                continue;
            }
            let px = xc + x;
            let nx = xc - x;
            let cmp = min_dot * ((x * x + y2) as f64).sqrt();
            let x_dot = x as f64 * xm;
            if pos_y && pos_x && (x_dot + y_dot >= cmp) {
                ctx.unsafe_set_at(px, py);
            }
            if pos_y && neg_x && (-x_dot + y_dot >= cmp) {
                ctx.unsafe_set_at(nx, py);
            }
            if neg_y && pos_x && (x_dot - y_dot >= cmp) {
                ctx.unsafe_set_at(px, ny);
            }
            if neg_y && neg_x && (-x_dot - y_dot >= cmp) {
                ctx.unsafe_set_at(nx, ny);
            }
        }
    }
}

fn ctx(s: &mut Surface, c: Color) -> Ctx<'_> {
    let color = s.map(c);
    let clip = s.clip;
    Ctx { s, clip, color }
}

// ------------------------------------------------------------------ public API (pygame.draw.*)

/// pygame.draw.rect(surface, color, rect, width=0, border_radius=0)
pub fn rect(s: &mut Surface, c: Color, r: Rect, width: i32, radius: i32) {
    if width < 0 {
        return;
    }
    let mut ctx = ctx(s, c);
    if radius <= 0 || r.w.abs() < 2 || r.h.abs() < 2 {
        let Some(clipped) = r.sdl_intersect(&ctx.clip) else {
            return;
        };
        if width > 0 && width * 2 < clipped.w && width * 2 < clipped.h {
            draw_rect_outline(&mut ctx, r.x, r.y, r.x + r.w - 1, r.y + r.h - 1, width);
        } else {
            let color = ctx.color;
            let w = ctx.s.w;
            for y in clipped.y..clipped.y + clipped.h {
                let row = (y * w) as usize;
                ctx.s.px[row + clipped.x as usize..row + (clipped.x + clipped.w) as usize].fill(color);
            }
        }
        return;
    }
    let mut r = r;
    if r.w < 0 {
        r.x += r.w;
        r.w = -r.w;
    }
    if r.h < 0 {
        r.y += r.h;
        r.h = -r.h;
    }
    let mut width = width;
    if width > r.w / 2 || width > r.h / 2 {
        width = (r.w / 2).max(r.h / 2);
    }
    draw_round_rect(&mut ctx, r.x, r.y, r.x + r.w - 1, r.y + r.h - 1, radius, width);
}

/// pygame.draw.line(surface, color, start, end, width=1)
pub fn line(s: &mut Surface, c: Color, start: (i32, i32), end: (i32, i32), width: i32) {
    if width < 1 {
        return;
    }
    let mut ctx = ctx(s, c);
    draw_line_width(&mut ctx, start.0, start.1, end.0, end.1, width);
}

/// pygame.draw.lines(surface, color, closed, points, width=1)
pub fn lines(s: &mut Surface, c: Color, closed: bool, pts: &[(i32, i32)], width: i32) {
    if pts.len() < 2 || width < 1 {
        return;
    }
    let mut ctx = ctx(s, c);
    for i in 1..pts.len() {
        draw_line_width(&mut ctx, pts[i - 1].0, pts[i - 1].1, pts[i].0, pts[i].1, width);
    }
    if closed && pts.len() > 2 {
        let l = pts.len() - 1;
        draw_line_width(&mut ctx, pts[l].0, pts[l].1, pts[0].0, pts[0].1, width);
    }
}

/// pygame.draw.circle(surface, color, center, radius, width=0)
pub fn circle(s: &mut Surface, c: Color, center: (i32, i32), radius: i32, width: i32) {
    if radius < 1 || width < 0 {
        return;
    }
    let width = width.min(radius);
    let (px, py) = center;
    let clip = s.clip;
    if px > clip.x + clip.w + radius || px < clip.x - radius || py > clip.y + clip.h + radius || py < clip.y - radius {
        return;
    }
    let mut ctx = ctx(s, c);
    if width == 0 || width == radius {
        draw_circle_filled(&mut ctx, px, py, radius);
    } else if width == 1 {
        draw_circle_bresenham_thin(&mut ctx, px, py, radius);
    } else {
        draw_circle_bresenham(&mut ctx, px, py, radius, width);
    }
}

/// pygame.draw.polygon(surface, color, points, width=0)
pub fn polygon(s: &mut Surface, c: Color, pts: &[(i32, i32)], width: i32) {
    if width != 0 {
        lines(s, c, true, pts, width);
        return;
    }
    if pts.len() < 3 {
        return;
    }
    let xs: Vec<i32> = pts.iter().map(|p| p.0).collect();
    let ys: Vec<i32> = pts.iter().map(|p| p.1).collect();
    let mut ctx = ctx(s, c);
    if pts.len() != 3 {
        draw_fillpoly(&mut ctx, &xs, &ys);
    } else {
        draw_filltri(&mut ctx, &xs, &ys);
    }
}

/// pygame.draw.ellipse(surface, color, rect, width=0)
pub fn ellipse(s: &mut Surface, c: Color, r: Rect, width: i32) {
    if width < 0 {
        return;
    }
    let mut ctx = ctx(s, c);
    if width == 0 || width >= (r.w / 2 + r.w % 2).min(r.h / 2 + r.h % 2) {
        draw_ellipse_filled(&mut ctx, r.x, r.y, r.w, r.h);
    } else {
        draw_ellipse_thickness(&mut ctx, r.x, r.y, r.w, r.h, width - 1);
    }
}

/// pygame.draw.arc(surface, color, rect, start_angle, stop_angle, width=1)
pub fn arc(s: &mut Surface, c: Color, r: Rect, a_start: f64, a_stop: f64, width: i32) {
    if width < 0 {
        return;
    }
    let mut width = width;
    if width > r.w / 2 || width > r.h / 2 {
        width = (r.w / 2).max(r.h / 2);
    }
    let mut a_stop = a_stop;
    if a_stop < a_start {
        a_stop += 2.0 * std::f64::consts::PI;
    }
    width = width.min(r.w.min(r.h) / 2);
    let mut ctx = ctx(s, c);
    draw_arc(&mut ctx, r.x + r.w / 2, r.y + r.h / 2, r.w / 2, r.h / 2, width, a_start, a_stop);
}
