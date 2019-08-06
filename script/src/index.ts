
const cratePromise = import('../../crate/pkg/index').then(c => {
  c.init()
  return c
})

addEventListener('fetch', ((event: FetchEvent) => {
  event.respondWith(handleRequest(event.request))
}) as EventListener)

async function handleRequest(request: Request) {
  try {
    const crate = await cratePromise
    const imageUrl = new URL(request.url).searchParams.get('url')
    if (!imageUrl) return new Response('Missing url parameter', { status: 400 })
    return await (crate.rotate(imageUrl) as Promise<Response>)
  } catch (e) {
    return new Response('Error: ' + (e.stack || e), { status: 500 })
  }
}