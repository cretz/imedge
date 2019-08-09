import * as crate from '../../crate/pkg/index'

const config = {
  sameOrigin: false,
  maxOps: 10,
  showStackOnErr: false,
}

addEventListener('fetch', ((event: FetchEvent) => {
  event.respondWith(handleRequest(event.request))
}) as EventListener)

async function handleRequest(request: Request) {
  let image: WorkingImage|null = null
  try {
    // Init the crate (idempotent)
    crate.init()
    // Create the image
    let origReq = new OrigRequest(new URL(request.url))
    image = WorkingImage.fromQueryParams(origReq, origReq.url.searchParams)
    // Apply the ops
    image.applyOps(origReq.url.searchParams)
    // Build the image with the given format or same as input
    const formatStr = origReq.url.searchParams.get('format')
    const format = formatStr ? crate.image_format_from_string(formatStr) : undefined
    return await image.build(format)
  } catch (e) {
    if (e instanceof Response) return e
    if (e instanceof ValidationError) return new Response(e.message, { status: 400 })
    const msg = !config.showStackOnErr ? e : e.stack || e
    return new Response('Error: ' + msg, { status: 500 })
  } finally {
    if (image) try { (image as any).image.free() } catch (e) { }
  }
}

class OrigRequest {
  opCount: number = 0
  constructor(public url: URL) { }
}

class WorkingImage {
  static fromQueryParams(request: OrigRequest, params: URLSearchParams) {
    const imageUrl = params.get('url')
    const empty = params.get('empty')
    // Use empty image if requested, otherwise load
    if (empty !== null) {
      if (imageUrl !== null) throw new ValidationError('Cannot give "empty" and "url" parameters')
      const flags: { color?: string } = {}
      let [w, h] = parseOpInts('empty', empty, 2, flags)
      if (w === undefined || h === undefined) throw new ValidationError('Missing "empty" width and/or height')
      return new WorkingImage(request, crate.WorkingImage.empty(w, h, flags.color || '00000000'))
    }
    if (!imageUrl) throw new ValidationError('Must have "empty" or "url" parameter')
    if (config.sameOrigin && new URL(imageUrl).origin !== request.url.origin) {
      throw new ValidationError('Invalid image URL')
    }
    return this.fromUrl(request, imageUrl)
  }

  static fromUrl(request: OrigRequest, url: string) {
    const parsedUrl = new URL(url)
    // If this is at the same origin+path as request, just work recursively.
    // Infinite recursion is not a concern (nor is billion laughs since script
    // runtime is limited).
    if (parsedUrl.origin === request.url.origin && parsedUrl.pathname === request.url.pathname) {
      const image = WorkingImage.fromQueryParams(request, parsedUrl.searchParams)
      image.applyOps(parsedUrl.searchParams)
    }
    // Otherwise, just load it
    return new WorkingImage(request, new crate.WorkingImage(url))
  }

  constructor(private request: OrigRequest, private image: crate.WorkingImage) { }

  applyOps(params: URLSearchParams) {
    // Go over every param to apply ops in order
    params.forEach((value, key) => {
      // If key is a method here w/ "op" prefixed, it's an op. Ignore otherwise.
      if (WorkingImage.prototype.hasOwnProperty('op' + key)) {
        if (++this.request.opCount > config.maxOps) throw new ValidationError('Too many ops')
        ;(this as any)['op' + key](value)
      }
    })
  }

  build(format?: crate.ImageFormat) {
    return this.image.build(format) as Promise<Response>
  }

  opblur(strOp: string) {
    const [s] = parseOp('blur', strOp, 1)
    if (!s) throw new ValidationError('Missing blur sigma')
    this.image = this.image.blur(Number.parseFloat(s))
  }

  opborder(strOp: string) {
    const flags: { color?: string } = {}
    let [top, right, bottom, left] = parseOpNumOrPcts('border', strOp, 4, flags)
    if (!top) throw new ValidationError('Missing value for border op')
    else if (!right) [right, bottom, left] = [top, top, top]
    else if (!bottom) [bottom, left] = [top, right]
    else if (!left) left = right
    this.image = this.image.border(
      top.v, top.pct, right.v, right.pct,
      bottom.v, bottom.pct, left.v, left.pct,
      flags.color || '00000000')
  }

  opbrighten(strOp: string) {
    const [v] = parseOp('brighten', strOp, 1)
    if (!v) throw new ValidationError('Missing brighten value')
    this.image = this.image.brighten(Number.parseInt(v))
  }

  opcontrast(strOp: string) {
    const [c] = parseOp('contrast', strOp, 1)
    if (!c) throw new ValidationError('Missing contrast value')
    this.image = this.image.contrast(Number.parseFloat(c))
  }

  opcrop(strOp: string) {
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

  opflip(strOp: string) {
    let [dir] = parseOp('flip', strOp, 1)
    if (!dir) dir = 'h'
    if (dir !== 'h' && dir !== 'v') throw new ValidationError('Invalid flip direction')
    this.image = this.image.flip(dir === 'h')
  }

  opgrayscale(strOp: string) {
    parseOp('grayscale', strOp, 0)
    this.image = this.image.grayscale()
  }

  opoverlay(strOp: string) {
    // Special format, flags first: (x=left|center|right|#,y=top|middle|bottom|#,hrepeat,vrepeat)http(s)://...
    const flags: { x?: string; y?: string; hrepeat?: string; vrepeat?: string } = {}
    let url = strOp
    const parenIndex = strOp.indexOf('(')
    if (parenIndex >= 0) {
      const parenEndIndex = strOp.indexOf(')')
      if (parenEndIndex === -1) throw new ValidationError('Invalid overlay flags')
      parseFlags(strOp.substring(parenIndex + 1, parenEndIndex), flags)
      url = strOp.substring(parenEndIndex + 1)
    }
    if (!url.startsWith('http://') && !url.startsWith('https://')) {
      throw new ValidationError('Overlay URL must be absolute')
    }
    // Get x and y
    const x_pct = !!(flags.x && flags.x.includes('.'))
    let x = 0
    let halign: number|undefined
    if (flags.x === 'left') halign = -1
    else if (flags.x === 'center') halign = 0
    else if (flags.x === 'right') halign = 1
    else {
      x = Number.parseFloat(flags.x || '')
      if (Number.isNaN(x)) halign = 0
    }
    const y_pct = !!(flags.y && flags.y.includes('.'))
    let y = 0
    let valign: number|undefined
    if (flags.y === 'top') valign = -1
    else if (flags.y === 'middle') valign = 0
    else if (flags.y === 'bottom') valign = 1
    else {
      y = Number.parseFloat(flags.y || '')
      if (Number.isNaN(y)) valign = 0
    }
    // If the URL is the same origin and path, we're just gonna work recursively in here
    const parsedUrl = new URL(url)
    let overlay: WorkingImage
    if (parsedUrl.origin === this.request.url.origin && parsedUrl.pathname === this.request.url.pathname) {
      overlay = WorkingImage.fromQueryParams(this.request, parsedUrl.searchParams)
      overlay.applyOps(parsedUrl.searchParams)
    } else {
      // Otherwise, just load it
      overlay = new WorkingImage(this.request, new crate.WorkingImage(url))
    }
    // Do overlay
    this.image = this.image.overlay(overlay.image,
      x, x_pct, halign, 'hrepeat' in flags,
      y, y_pct, valign, 'vrepeat' in flags)
  }

  opresize(strOp: string) {
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

  oprotate(strOp: string) {
    let [deg] = parseOpInts('rotate', strOp, 1)
    if (deg !== 90 && deg !== 180 && deg !== 270) throw new ValidationError('Rotation arg must be 90, 180, or 270')
    this.image = this.image.rotate(deg)
  }

  opsharpen(strOp: string) {
    let [s, t] = parseOp('sharpen', strOp, 2)
    const sNum = Number.parseFloat(s)
    const tNum = Number.parseInt(t)
    if (Number.isNaN(sNum) || Number.isNaN(tNum)) throw new ValidationError('Invalid sharpen sigma and/or threshold')
    this.image = this.image.sharpen(sNum, tNum)
  }

  opthumbnail(strOp: string) {
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
  const parenIndex = value.indexOf('(')
  if (parenIndex >= 0) {
    if (!value.endsWith(')')) throw new ValidationError('Missing flag end param on op ' + key)
    if (!flags) throw new ValidationError('Flags not supported on op ' + key)
    parseFlags(value.substring(parenIndex + 1, value.length - 1), flags)
    value = value.substring(0, parenIndex)
  }
  const args = !value ? [] : value.split(',', maxArgs + 1)
  if (args.length > maxArgs) throw new ValidationError('Too many args for op ' + key)
  return args
}

function parseFlags(str: string, flags: any) {
  str.split(',').forEach(flag => {
    const [key, val] = flag.split('=', 2)
    flags[key] = val
  })
}