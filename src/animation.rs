use gfx_maths::*;

pub struct Animation2D {
    pub loc1: Vec2,
    pub loc2: Vec2,
    pub time_to_animate: f32, // in milliseconds
}

impl Animation2D {
    pub fn new(loc1: Vec2, loc2: Vec2, time_to_animate: f32) -> Animation2D {
        Animation2D {
            loc1,
            loc2,
            time_to_animate,
        }
    }

    pub fn get_point_at_time(&self, time: f64) -> Vec2 {
        // time is in milliseconds
        let time_ratio = time / self.time_to_animate as f64;
        let x = self.loc1.x as f64 + ((self.loc2.x - self.loc1.x) as f64 * time_ratio);
        let y = self.loc1.y as f64 + ((self.loc2.y - self.loc1.y) as f64 * time_ratio);
        Vec2::new(x as f32, y as f32)
    }
}