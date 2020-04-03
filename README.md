# stockton

[![Build Status](https://travis-ci.org/tcmal/stockton.svg?branch=master)](https://travis-ci.org/tcmal/stockton)

A 3D engine.

## TODOs

Render Optimisations:
  - Make StagedBuffers resizable
  - Share the same Memory across multiple Buffers
  - Use the same descriptorpool for all descriptorsets
  - Handle textures spread across multiple descriptorsets/draw calls
  - Instanced drawing
  - Model translation matrices
  - Use a different command pool for memcpy operations
  - Sync memcpy operations with semaphores
  - Add timing/profiling
  - Fix shadermodules not being destroyed on shutdown
  - Handle window resize properly

Features:
  - Moving Camera/Positionable Trait
  - Entity drawing

## License

Code & Assets (including from `rust-bsp`) are licensed under the GNU GPL v3.0, all contributions automatically come under this. See LICENSE.

Exceptions:

  - `examples/render-quad/data/test1.png` - [Photo by Lisa Fotios from Pexels](https://www.pexels.com/photo/white-petaled-flowers-painting-2224220/)
  - `examples/render-quad/data/test2.png` - [Photo by Elina Sazonova from Pexels](https://www.pexels.com/photo/brown-tabby-cat-on-pink-textile-3971972/)