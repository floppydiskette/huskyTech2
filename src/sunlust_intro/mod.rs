// array of 2d points showing the path that the cool looking rainbow line will take
// to draw the HT2 logo


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