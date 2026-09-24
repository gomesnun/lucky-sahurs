//! Integer rectangle with pygame.Rect semantics (C integer division, truncating float inputs).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// pygame converts float arguments with a plain C cast (truncation toward zero).
#[inline]
pub fn ti(v: f64) -> i32 {
    if v.is_nan() {
        0
    } else if v >= i32::MAX as f64 {
        i32::MAX
    } else if v <= i32::MIN as f64 {
        i32::MIN
    } else {
        v as i32
    }
}

impl Rect {
    pub const ZERO: Rect = Rect { x: 0, y: 0, w: 0, h: 0 };

    #[inline]
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    /// Rect built from float values (pygame truncates each one).
    #[inline]
    pub fn f(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(ti(x), ti(y), ti(w), ti(h))
    }
    #[inline]
    pub fn left(&self) -> i32 {
        self.x
    }
    #[inline]
    pub fn top(&self) -> i32 {
        self.y
    }
    #[inline]
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    #[inline]
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    #[inline]
    pub fn centerx(&self) -> i32 {
        self.x + self.w / 2
    }
    #[inline]
    pub fn centery(&self) -> i32 {
        self.y + self.h / 2
    }
    #[inline]
    pub fn center(&self) -> (i32, i32) {
        (self.centerx(), self.centery())
    }
    #[inline]
    pub fn topleft(&self) -> (i32, i32) {
        (self.x, self.y)
    }
    #[inline]
    pub fn topright(&self) -> (i32, i32) {
        (self.right(), self.y)
    }
    #[inline]
    pub fn bottomleft(&self) -> (i32, i32) {
        (self.x, self.bottom())
    }
    #[inline]
    pub fn bottomright(&self) -> (i32, i32) {
        (self.right(), self.bottom())
    }
    #[inline]
    pub fn midtop(&self) -> (i32, i32) {
        (self.centerx(), self.y)
    }
    #[inline]
    pub fn midbottom(&self) -> (i32, i32) {
        (self.centerx(), self.bottom())
    }
    #[inline]
    pub fn midleft(&self) -> (i32, i32) {
        (self.x, self.centery())
    }
    #[inline]
    pub fn midright(&self) -> (i32, i32) {
        (self.right(), self.centery())
    }
    #[inline]
    pub fn size(&self) -> (i32, i32) {
        (self.w, self.h)
    }

    pub fn set_center(&mut self, c: (i32, i32)) {
        self.x = c.0 - self.w / 2;
        self.y = c.1 - self.h / 2;
    }
    pub fn set_centerx(&mut self, cx: i32) {
        self.x = cx - self.w / 2;
    }
    pub fn set_centery(&mut self, cy: i32) {
        self.y = cy - self.h / 2;
    }
    pub fn set_midtop(&mut self, p: (i32, i32)) {
        self.x = p.0 - self.w / 2;
        self.y = p.1;
    }
    pub fn set_midbottom(&mut self, p: (i32, i32)) {
        self.x = p.0 - self.w / 2;
        self.y = p.1 - self.h;
    }
    pub fn set_midleft(&mut self, p: (i32, i32)) {
        self.x = p.0;
        self.y = p.1 - self.h / 2;
    }
    pub fn set_midright(&mut self, p: (i32, i32)) {
        self.x = p.0 - self.w;
        self.y = p.1 - self.h / 2;
    }
    pub fn set_topright(&mut self, p: (i32, i32)) {
        self.x = p.0 - self.w;
        self.y = p.1;
    }
    pub fn set_bottomleft(&mut self, p: (i32, i32)) {
        self.x = p.0;
        self.y = p.1 - self.h;
    }
    pub fn set_bottomright(&mut self, p: (i32, i32)) {
        self.x = p.0 - self.w;
        self.y = p.1 - self.h;
    }
    pub fn set_right(&mut self, r: i32) {
        self.x = r - self.w;
    }
    pub fn set_bottom(&mut self, b: i32) {
        self.y = b - self.h;
    }

    // ---- constructors mirroring surf.get_rect(<anchor>=...) ----
    pub fn sized(w: i32, h: i32) -> Rect {
        Rect::new(0, 0, w, h)
    }
    pub fn with_center(w: i32, h: i32, c: (i32, i32)) -> Rect {
        let mut r = Rect::sized(w, h);
        r.set_center(c);
        r
    }
    pub fn with_midtop(w: i32, h: i32, p: (i32, i32)) -> Rect {
        let mut r = Rect::sized(w, h);
        r.set_midtop(p);
        r
    }
    pub fn with_midbottom(w: i32, h: i32, p: (i32, i32)) -> Rect {
        let mut r = Rect::sized(w, h);
        r.set_midbottom(p);
        r
    }
    pub fn with_midleft(w: i32, h: i32, p: (i32, i32)) -> Rect {
        let mut r = Rect::sized(w, h);
        r.set_midleft(p);
        r
    }
    pub fn with_midright(w: i32, h: i32, p: (i32, i32)) -> Rect {
        let mut r = Rect::sized(w, h);
        r.set_midright(p);
        r
    }
    pub fn with_topright(w: i32, h: i32, p: (i32, i32)) -> Rect {
        let mut r = Rect::sized(w, h);
        r.set_topright(p);
        r
    }
    pub fn with_bottomright(w: i32, h: i32, p: (i32, i32)) -> Rect {
        let mut r = Rect::sized(w, h);
        r.set_bottomright(p);
        r
    }
    pub fn with_bottomleft(w: i32, h: i32, p: (i32, i32)) -> Rect {
        let mut r = Rect::sized(w, h);
        r.set_bottomleft(p);
        r
    }

    /// pygame: Rect(x - dx/2, y - dy/2, w + dx, h + dy) with C division.
    pub fn inflate(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x - dx / 2, self.y - dy / 2, self.w + dx, self.h + dy)
    }
    pub fn move_by(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// collidepoint with float coordinates (pygame truncates them to int first).
    pub fn collidepoint(&self, p: (f64, f64)) -> bool {
        let px = ti(p.0);
        let py = ti(p.1);
        self.collidepoint_i(px, py)
    }
    pub fn collidepoint_i(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    pub fn colliderect(&self, o: &Rect) -> bool {
        // pygame-ce: zero-size rects never collide
        if self.w == 0 || self.h == 0 || o.w == 0 || o.h == 0 {
            return false;
        }
        let (ax, ay, aw, ah) = self.normalized_parts();
        let (bx, by, bw, bh) = o.normalized_parts();
        ax.min(ax + aw) < bx.max(bx + bw)
            && ay.min(ay + ah) < by.max(by + bh)
            && ax.max(ax + aw) > bx.min(bx + bw)
            && ay.max(ay + ah) > by.min(by + bh)
    }

    fn normalized_parts(&self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.w, self.h)
    }

    /// Rect.contains: `o` fully inside self.
    pub fn contains(&self, o: &Rect) -> bool {
        self.x <= o.x
            && self.y <= o.y
            && self.x + self.w >= o.x + o.w
            && self.y + self.h >= o.y + o.h
            && self.x + self.w > o.x
            && self.y + self.h > o.y
    }

    /// Rect.clip: intersection (zero-size at self's topleft when disjoint, like pygame).
    pub fn clip(&self, o: &Rect) -> Rect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = (self.x + self.w).min(o.x + o.w);
        let b = (self.y + self.h).min(o.y + o.h);
        if r <= x || b <= y {
            return Rect::new(self.x, self.y, 0, 0);
        }
        Rect::new(x, y, r - x, b - y)
    }

    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        let r = (self.x + self.w).max(o.x + o.w);
        let b = (self.y + self.h).max(o.y + o.h);
        Rect::new(x, y, r - x, b - y)
    }

    /// SDL_IntersectRect (returns None when empty).
    pub fn sdl_intersect(&self, o: &Rect) -> Option<Rect> {
        if self.w <= 0 || self.h <= 0 || o.w <= 0 || o.h <= 0 {
            return None;
        }
        let x = self.x.max(o.x);
        let r = (self.x + self.w).min(o.x + o.w);
        let y = self.y.max(o.y);
        let b = (self.y + self.h).min(o.y + o.h);
        if r <= x || b <= y {
            None
        } else {
            Some(Rect::new(x, y, r - x, b - y))
        }
    }
}
