# stockton

[![Build Status](https://travis-ci.org/tcmal/stockton.svg?branch=master)](https://travis-ci.org/tcmal/stockton)

A 3D engine inspired by quake.

Most of what is described below isn't fully done, or even started.

## Developing games

Maps currently use the regular Q3 `.bsp` format, with each type of entity needing to be defined as a type implementing the `Entity` trait, through which it recieves events. You'll also need some sort of a `TextureStore` which finds the textures needed and converts them into a usable format. A lot of this is helped by `stockton-glue`

## Internal Structure

`bsp` is a library for parsing `.bsp` files to nice data structures. It can be found [here](https://github.com/tcmal/rust-bsp)

`stockton-types` contains shared types & macros used by all the other crates, for example the world, entities, and other important things.

`stockton-simulate` makes the world living, including collision detection, propagating events to entities and game state.

`stockton-render` renders the world to a given surface, using `gfx` and `nalgebra`.

`stockton-glue` helps you glue these together into an actual executable game.

## License

Code & Assets (including from `rust-bsp`) are licensed under the GNU GPL v3.0, all contributions automatically come under this. See LICENSE.
