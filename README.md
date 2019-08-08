# Imedge

This is a service worker script to manipulate images using [image-rs](https://github.com/image-rs/image) compiled to
WebAssembly. It is built as an example to show image manipulation, not as a full-fledged image manipulation/optimization
solution.

### Usage

When built and running (see later sections), this operates at a URL. It accepts the following general URL query
parameters:

* `url` - Required path to the image to fetch and manipulate.
* `format` - Optional value of `JPEG` or `PNG` to set as output format. If unset, uses same format as was read
  originally.

All other URL query parameters are image operations, or "ops". They are applied to the image in the order they are seen
and can be duplicated. In addition to standard values, some accept additional parenthesized flags. Ops are in the format
of `op1=val1,val2(flag1=flagval1,flag2)`. Ops:

* `blur=sigma` - Blur by the `sigma` float.
* `border=a,b,c,d(color=hexcolor)` - Add a border around the image. Only `a` is required. `a`, `b`, `c`, and `d` are
  either integer values or decimal values as percents of the width or height. The values are applied similar to how
  `border-width` in CSS is applied, i.e. either `border=all`, `border=top_and_bottom,right_and_left`,
  `border=top,right_and_left,bottom` or `border=top,right,bottom,left`. The `color` flag is an optional hex RGB value,
  otherwise black is used.
* `brighten=v` - Brighten by the `v` integer. `v` can be negative to darken.
* `contrast=c` - Adjust the contrast by the `c` float. `c` can be negative.
* `crop=w,h` or `crop=x,y,w,h` - Crop the image by the given `w` integer width and `h` integer height. The four-value
  version can set the offset `x` and `y` integers.
* `flip=h|v` - Flip either horizontally (the default or if `h` is set) or vertically (if `v` is set).
* `grayscale` - Convert to gray scale.
* `resize=w,h(exact,filter=filtertype)` - Resize to `w` width and `h` height. `w` and `h` can be fixed integers or
  floats that are applied as percentages of the current image. If `h` is not present, it is assumed to be the same as
  `w`. By default this resizes to the largest of the sizes while maintaining the aspect ratio. If the optional `exact`
  flag is set, the image will be resized to exactly that amount without maintaining the aspect ratio. `filter` is an
  advanced flag to choose the filter type.
* `sharpen=sigma,threshold` - Applies an unsharpen mask with the given blur `sigma` float and `threshold` integer.
* `thumbnail=w,h(exact)` - This is the same as `resize` but uses faster algorithm.

### Building

Prerequisites:

* Rust (latest stable)
* Nodejs (latest stable)

Navigate to script directory:

    cd script

To build:

    npm run build

This will build an optimized script for use at `dist/index.js`.

### Development

Instead of doing optimized build, this builds a development version:

    npm run build-dev

Or to have it watch or changes and automatically rebuild:

    npm run dev

To run a local development service worker handler that will reload on changes:

    npm run cloudworker
