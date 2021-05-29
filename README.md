# stockton

[![Build Status](https://travis-ci.org/tcmal/stockton.svg?branch=master)](https://travis-ci.org/tcmal/stockton)

A WIP Quake engine using Vulkan and Rust.

## State

Currently, it can render a BSP file with textures on the filesystem using however many texture arrays are needed. It doesn't properly cull/sort the faces of the BSP file though.

## License

Code & Assets (including from `rust-bsp`) are licensed under the GNU GPL v3.0, all contributions automatically come under this. See LICENSE.

Exceptions:

  - `rendy-memory` and `rendy-descriptor` are both modified from [here](https://github.com/amethyst/rendy) and are licensed under MIT.
  - `examples/render-quad/data/test1.png` - [Photo by Lisa Fotios from Pexels](https://www.pexels.com/photo/white-petaled-flowers-painting-2224220/)
  - `examples/render-quad/data/test2.png` - [Photo by Elina Sazonova from Pexels](https://www.pexels.com/photo/brown-tabby-cat-on-pink-textile-3971972/)