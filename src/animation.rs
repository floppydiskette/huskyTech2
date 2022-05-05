use crate::renderer::loc;

pub struct Animation2D {
    pub loc1: loc,
    pub loc2: loc,
    pub time_to_animate: f32, // in milliseconds
}

impl Animation2D {
    pub fn new(loc1: loc, loc2: loc, time_to_animate: f32) -> Animation2D {
        Animation2D {
            loc1,
            loc2,
            time_to_animate,
        }
    }

    pub fn get_point_at_time(&self, time: f32) -> loc {
        let time_ratio = time / self.time_to_animate;
        let x = (self.loc1.x + (self.loc2.x - self.loc1.x)) as f32 * time_ratio;
        let y = (self.loc1.y + (self.loc2.y - self.loc1.y)) as f32 * time_ratio;
        loc { x: x as i32, y: y as i32 }
    }
}