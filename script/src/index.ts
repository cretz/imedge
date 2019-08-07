const cratePromise = import('../../crate/pkg/index').then(c => {
  c.init()
  return c
})

const showStackOnErr = true

addEventListener('fetch', ((event: FetchEvent) => {
  event.respondWith(handleRequest(event.request))
}) as EventListener)

async function handleRequest(request: Request) {
  try {
    // Get URL
    const crate = await cratePromise
    const urlParams = new URL(request.url).searchParams
    const imageUrl = urlParams.get('url')
    if (!imageUrl) return new Response('Missing url parameter', { status: 400 })
    // Create the image with the given format or null
    let image = new crate.WorkingImage(imageUrl)
    // Now go over every param to apply opts in order
    urlParams.forEach((value, key) => {
      // Handle opts, ignore unrecognized
      switch (key) {
        case 'resize':
          const flags: { exact?: string; filter?: string } = {}
          const [w, h] = parseOpt(value, 2, flags)
          if (!w || !h) throw new Error('Missing width and/or height')
          image = image.resize(
            Number.parseFloat(w), w.includes('.'),
            Number.parseFloat(h), h.includes('.'),
            'exact' in flags,
            flags.filter ? crate.filter_type_from_string(flags.filter) : undefined
          )
          break
      }
    })
    // Build the image with the given format or same as input
    const formatStr = urlParams.get('format')
    const format = formatStr ? crate.image_format_from_string(formatStr) : undefined
    return await (image.build(format) as Promise<Response>)
  } catch (e) {
    if (e instanceof Response) return e;
    const msg = !showStackOnErr ? e : e.stack || e
    return new Response('Error: ' + msg, { status: 500 })
  }
}

function parseOpt(value: string, maxArgs: number, flags?: any): string[] {
  // Format is a,b,c(d=e,f)
  const commaIndex = value.indexOf('(')
  if (commaIndex >= 0) {
    if (!value.endsWith(')')) throw new Error('Missing end parens')
    if (flags) {
      value.substring(commaIndex + 1, value.length - 1).split(',').forEach(flag => {
        const [key, val] = flag.split('=', 2)
        flags[key] = val
      })
    }
    value = value.substring(0, commaIndex)
  }
  return value.split(',', maxArgs)
}