use crate::renderer::Loc;

pub struct Animation2D {
    pub loc1: Loc,
    pub loc2: Loc,
    pub time_to_animate: f32, // in milliseconds
}

impl Animation2D {
    pub fn new(loc1: Loc, loc2: Loc, time_to_animate: f32) -> Animation2D {
        Animation2D {
            loc1,
            loc2,
            time_to_animate,
        }
    }

    pub fn get_point_at_time(&self, time: f64) -> Loc {
        // time is in milliseconds
        let time_ratio = time / self.time_to_animate as f64;
        let x = self.loc1.x as f64 + ((self.loc2.x - self.loc1.x) as f64 * time_ratio);
        let y = self.loc1.y as f64 + ((self.loc2.y - self.loc1.y) as f64 * time_ratio);
        Loc { x: x as i32, y: y as i32 }
    }
}