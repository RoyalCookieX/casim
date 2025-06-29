# casim
This is my attempt at a 2D sandbox simulator written in Rust.

## Features:
- Different element interactions:
    - Stone stays in place
    - Sand falls down while spreading out
- GPU-driven simulation using compute shaders
- Simulation can be either in real-time or step-by-step

## Controls:
- 1: Draw Stone
- 2: Draw Sand
- Left Click: Draws at mouse cursor
- Right Click: Erases at mouse cursor
- Scroll Wheel: Changes draw radius
- Left Shift: Pause/Unpause simulation
- Space: Advances simulation by 1 step

## How to Build & Run (VSCode):
- Set up Rust with VSCode: https://code.visualstudio.com/docs/languages/rust
- In the project directory, run ``cargo run --release``, which will eventually generate ``target/release/casim.exe``, followed by executing it as well.

![](https://github.com/RoyalCookieX/casim/blob/main/screenshots/screenshot_0.png?raw=true)

## TODO:
- [ ] Water
    - [x] Draw with '3' key
    - [ ] Interaction with Stone
    - [ ] Interaction with Sand
