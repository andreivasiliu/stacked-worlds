# Stacked Worlds

A game prototype written in Rust.

This project's focus is both as a playground for me to learn Rust, and to implement a game and level-editor for an idea described to me by [lemon24](https://github.com/lemon24).

What it looks like: [game gif](https://media.giphy.com/media/1jkUVLAMs9hLudoD8P/giphy.gif). 

## Running

Download the sources, get [Rust](https://www.rust-lang.org/en-US/), and run `cargo run --release`.

## Controls

Mouse:
* `LMB` *(hold)* - Drag to create rooms, drag inside rooms to draw rectangles
* `RMB` *(hold)* - Hold to create a chain between you and the target, if in range
* `MMB` *(hold)* - Enable edge-panning (will be changed to better panning later)

Keyboard:
* `a` and `d` - Move left or right
* `Space` - Jump (must be touching a surface)
* `z` *(hold)* - Press to peek into the next room, release to teleport there
* `c` - Change camera mode (toggles between following the player or static)
* `r` - Reset the world (delete all rooms)
* `Esc` - Quit
