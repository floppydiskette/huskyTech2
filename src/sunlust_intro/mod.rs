// array of 2d points showing the path that the cool looking rainbow line will take
// to draw the HT2 logo


use crate::animation::Animation2D;
use crate::helpers::gen_rainbow;
use crate::renderer::loc;
use crate::renderer::ht_renderer;

static points: [loc; 21] = [
    loc{x: 640, y: 104},
    loc{x: 640-445, y: 104}, // 445 = 320 (centre screen) + 13 (half of the T bar) + 30 (length of one T arm) + 26 (one H bar) + 30 (length of one H arm) + 26 (one H bar)
    loc{x: 640-445, y: 208}, // go down to the bottom of the H
    loc{x: 640-419, y: 208}, // to the right side of the H bar
    loc{x: 640-419, y: 208-39}, // go to the bottom side of the H arm
    loc{x: 640-389, y: 208-39}, // go to the right side of the H arm
    loc{x: 640-389, y: 208}, // go to the bottom side of the right H arm
    loc{x: 640-415, y: 208}, // go to the right side of the right H arm
    loc{x: 640-415, y: 104+26}, // go just below the connector of the T to the H
    loc{x: 640-385, y: 104+26}, // go to the side of the T bar
    loc{x: 640-385, y: 208}, // go to the bottom of the T bar
    loc{x: 640-359, y: 208}, // go to the right side of the T bar
    loc{x: 640-359, y: 104+26}, // go to the bottom side of the T arm
    loc{x: 640-221, y: 104+26}, // WE NEED TO ARC FROM THE LAST POINT TO THIS ONE, THIS IS THE 2
    loc{x: 640-359, y: 208-26}, // go to the slight bend in the bottom left of the 2
    loc{x: 640-359, y: 208}, // go to the bottom of the 2
    loc{x: 640-221, y: 208}, // go to the right side of the 2
    loc{x: 640-221, y: 208-26}, // go to the top part of the right side of the 2
    loc{x: 640-333, y: 208-26}, // go to the little crack in the left side of the 2
    loc{x: 640-221, y: 104+26}, // go to the top right of the 2
    loc{x: 640-359, y: 104}, // DO AN ARC ON THIS ONE TOO
];

pub fn animate(mut renderer: ht_renderer) {

    let mut points_on_screen: Vec<loc> = Vec::new();
    let mut pos_i = 0;

    // time for the rainbow outline animation
    let rainbow_length = 1122.0; // in milliseconds

    let mut time = 0.0; // animation time from 0 onwards
    let mut delta_time = 0.0; // time since last frame
    let mut last_time = 0.0; // unix time of last frame

    let mut greater_i = 0;
    while greater_i < 21 {
        let mut i = 0;

        // time to animate the rainbow outline
        let tta = rainbow_length / 21.0;
        while (i as f32) < tta {
            if i > 20 {
                break;
            }
            let current_animation = Animation2D::new(points[i], points[i + 1], tta);
            delta_time += (time - last_time); // delta time in milliseconds
            last_time = time;
            time += delta_time;

            pos_i = 0;
            while pos_i < points_on_screen.len() { // draw all the previous points
                let color = gen_rainbow(time + (pos_i * 100) as f64);
                renderer.put_pixel(renderer.to_gl_coord(points_on_screen[pos_i]), color);
                pos_i += 1;
                renderer.swap_buffers();
            }

            let point = current_animation.get_point_at_time(time as f64);
            points_on_screen.push(point);
            // if length is greater than 40, remove the first point
            if points_on_screen.len() > 40 {
                points_on_screen.remove(0);
            }
            // draw the current point
            let color = gen_rainbow(time + pos_i as f64 + 100.0);
            renderer.put_pixel(renderer.to_gl_coord(point), color);
            renderer.swap_buffers();
            i += 1;
        }
        greater_i += 2;
        println!("done");
    }
}