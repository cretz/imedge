import * as crate from '../../crate/pkg/index'

const config = {
  sameOrigin: false,
  maxOps: 5,
  showStackOnErr: false,
}

addEventListener('fetch', ((event: FetchEvent) => {
  event.respondWith(handleRequest(event.request))
}) as EventListener)

async function handleRequest(request: Request) {
  try {
    // Init the crate (idempotent)
    crate.init()
    // Get URL
    const requestUrl = new URL(request.url)
    const imageUrl = requestUrl.searchParams.get('url')
    if (!imageUrl) throw new ValidationError('Missing url parameter')
    if (config.sameOrigin && new URL(imageUrl, request.url).origin !== requestUrl.origin) {
      throw new ValidationError('Invalid image URL')
    }
    // Create the image
    let image = new WorkingImage(imageUrl)
    // Now go over every param to apply ops in order
    let opCount = 0
    requestUrl.searchParams.forEach((value, key) => {
      // If key is a method on the image that's not "build", it's an op
      const anyImage = image as any
      if (WorkingImage.prototype.hasOwnProperty(key) && key !== 'build' && typeof anyImage[key] === 'function') {
        if (++opCount > config.maxOps) throw new ValidationError('Too many ops')
        anyImage[key](value)
      }
    })
    // Build the image with the given format or same as input
    const formatStr = requestUrl.searchParams.get('format')
    const format = formatStr ? crate.image_format_from_string(formatStr) : undefined
    return await image.build(format)
  } catch (e) {
    if (e instanceof Response) return e;
    if (e instanceof ValidationError) return new Response(e.message, { status: 400 })
    const msg = !config.showStackOnErr ? e : e.stack || e
    return new Response('Error: ' + msg, { status: 500 })
  }
}

class WorkingImage {
  image: crate.WorkingImage
  constructor(imageUrl: string) {
    this.image = new crate.WorkingImage(imageUrl)
  }

  build(format?: crate.ImageFormat) {
    return this.image.build(format) as Promise<Response>
  }

  blur(strOp: string) {
    const [s] = parseOp('blur', strOp, 1)
    if (!s) throw new ValidationError('Missing blur sigma')
    this.image = this.image.blur(Number.parseFloat(s))
  }

  border(strOp: string) {
    const flags: { color?: string } = {}
    let [top, right, bottom, left] = parseOpNumOrPcts('border', strOp, 4, flags)
    if (!top) throw new ValidationError('Missing value for border op')
    else if (!right) [right, bottom, left] = [top, top, top]
    else if (!bottom) [bottom, left] = [top, right]
    else if (!left) left = right
    this.image = this.image.border(top.v, top.pct, right.v, right.pct, bottom.v, bottom.pct, left.v, left.pct, flags.color)
  }

  brighten(strOp: string) {
    const [v] = parseOp('brighten', strOp, 1)
    if (!v) throw new ValidationError('Missing brighten value')
    this.image = this.image.brighten(Number.parseInt(v))
  }

  contrast(strOp: string) {
    const [c] = parseOp('contrast', strOp, 1)
    if (!c) throw new ValidationError('Missing contrast value')
    this.image = this.image.contrast(Number.parseFloat(c))
  }

  crop(strOp: string) {
    // Either w,h or x,y,w,h
    let [x, y, width, height] = parseOpInts('crop', strOp, 4)
    if (y === undefined) throw new ValidationError('Need two or four values for crop')
    if (width === undefined) {
      width = x
      height = y
      x = 0
      y = 0
    } else if (height === undefined) throw new ValidationError('Need two or four values for crop')
    this.image = this.image.crop(x, y, width, height)
  }

  flip(strOp: string) {
    let [dir] = parseOp('flip', strOp, 1)
    if (!dir) dir = 'h'
    if (dir !== 'h' && dir !== 'v') throw new ValidationError('Invalid flip direction')
    this.image = this.image.flip(dir === 'h')
  }

  grayscale(strOp: string) {
    parseOp('grayscale', strOp, 0)
    this.image = this.image.grayscale()
  }

  resize(strOp: string) {
    const flags: { exact?: string; filter?: string } = {}
    let [w, h] = parseOpNumOrPcts('resize', strOp, 2, flags)
    if (!w) throw new ValidationError('Need at least one value for resize')
    if (!h) h = w
    this.image = this.image.resize(
      w.v, w.pct, h.v, h.pct,
      'exact' in flags,
      flags.filter ? crate.filter_type_from_string(flags.filter) : undefined
    )
  }

  rotate(strOp: string) {
    let [deg] = parseOpInts('rotate', strOp, 1)
    if (deg !== 90 && deg !== 180 && deg !== 270) throw new ValidationError('Rotation arg must be 90, 180, or 270')
    this.image = this.image.rotate(deg)
  }

  sharpen(strOp: string) {
    let [s, t] = parseOp('sharpen', strOp, 2)
    const sNum = Number.parseFloat(s)
    const tNum = Number.parseInt(t)
    if (Number.isNaN(sNum) || Number.isNaN(tNum)) throw new ValidationError('Invalid sharpen sigma and/or threshold')
    this.image = this.image.sharpen(sNum, tNum)
  }

  thumbnail(strOp: string) {
    const flags: { exact?: string } = {}
    let [w, h] = parseOpNumOrPcts('thumbnail', strOp, 2, flags)
    if (!w) throw new ValidationError('Need at least one value for thumbnail')
    if (!h) h = w
    this.image = this.image.thumbnail(w.v, w.pct, h.v, h.pct, 'exact' in flags)
  }
}

class ValidationError extends Error {
  constructor(msg: string) {
    super(msg)
  }
}

function parseOpNumOrPcts(key: string, value: string, maxArgs: number, flags?: any): { v: number, pct: boolean }[] {
  const ret = new Array<{ v: number, pct: boolean }>()
  parseOp(key, value, maxArgs, flags).forEach(arg => {
    const v = Number.parseFloat(arg)
    if (!Number.isNaN(v)) ret.push({ v, pct: arg.includes('.') })
  })
  return ret
}

function parseOpInts(key: string, value: string, maxArgs: number, flags?: any): number[] {
  const ret = new Array<number>()
  parseOp(key, value, maxArgs, flags).forEach(arg => {
    const v = Number.parseInt(arg)
    if (!Number.isNaN(v)) ret.push(v)
  })
  return ret
}

function parseOp(key: string, value: string, maxArgs: number, flags?: any): string[] {
  // Format is a,b,c(d=e,f)
  const commaIndex = value.indexOf('(')
  if (commaIndex >= 0) {
    if (!value.endsWith(')')) throw new ValidationError('Missing flag end param on op ' + key)
    if (!flags) throw new ValidationError('Flags not supported on op ' + key)
    value.substring(commaIndex + 1, value.length - 1).split(',').forEach(flag => {
      const [key, val] = flag.split('=', 2)
      flags[key] = val
    })
    value = value.substring(0, commaIndex)
  }
  const args = !value ? [] : value.split(',', maxArgs + 1)
  if (args.length > maxArgs) throw new ValidationError('Too many args for op ' + key)
  return args
}