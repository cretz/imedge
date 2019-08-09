# Imedge

This is a service worker script to manipulate images using [image-rs](https://github.com/image-rs/image) compiled to
WebAssembly. It is built as an example to show image manipulation, not as a full-fledged image manipulation/optimization
solution.

### Usage

When built and running (see later sections), this operates at a URL and is based on query parameters. Unless otherwise
stated, query parameters are in the `op1=val1,val2(flag1=flagval1,flag2)`. Sometimes values may be optional, but the
parenthesized flags always are. There are fixed parameters and image operations. The fixed parameters are:

* `url=url` - Path to the image to fetch and manipulate. Either this or `empty` is required.
* `empty=w,h(color=hexcolor)` - Creates an image of `w` integer width and `h` integer height. The optional `color` flag
  can be provided to give a fill color, otherwise it is transparent.
* `format=format` - Optional value of `JPEG` or `PNG` to set as output format. If unset, uses same format as was read
  originally (or `PNG` if `empty` was used).

All other URL query parameters are image operations, or "ops". They are applied to the image in the order they are seen
and can be duplicated. The ops are:

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
* `overlay=(x=left|center|right|#,y=top|middle|bottom|#,hrepeat,vrepeat)url` - Overlays an image onto the current one.
  Unlike the other ops, this one has the flags first. `x` can be set to a horizontal alignment or a pixel offset or a
  width percentage offset. Similarly `y` can be set to a vertical alignment or a pixel offset or a height percentage
  offset. The default for `x` is `center` and the default for `y` is `middle`. `hrepeat` can be set to repeat the
  overlay to the right until the end of the image. `vrepeat` repeats beneath the overlay. After the flags is the URL of
  the image to use.
* `resize=w,h(exact,filter=filtertype)` - Resize to `w` width and `h` height. `w` and `h` can be fixed integers or
  floats that are applied as percentages of the current image. If `h` is not present, it is assumed to be the same as
  `w`. By default this resizes to the largest of the sizes while maintaining the aspect ratio. If the optional `exact`
  flag is set, the image will be resized to exactly that amount without maintaining the aspect ratio. `filter` is an
  advanced flag to choose the filter type.
* `sharpen=sigma,threshold` - Applies an unsharpen mask with the given blur `sigma` float and `threshold` integer.
* `thumbnail=w,h(exact)` - This is the same as `resize` but uses faster algorithm.

Note, care is taken to internally handle recursion on `url` or `overlay` when they point back to the same script. While
not very useful for `url`, this is very helpful for `overlay` because it essentially provides a way to stitch several
images together each with their own operations.

The script has a constant maximum number of ops that can be applied. This applies to recursive URLs such as those seen
in `overlay`. It also has a setting to restrict to only loading images from the same origin.

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
