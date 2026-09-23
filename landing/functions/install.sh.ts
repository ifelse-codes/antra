/**
 * Cloudflare Pages Function — serve `/install.sh` with `Content-Type: text/plain`.
 *
 * The one-liner `curl -fsSL https://antra.iifelse.com/install.sh | bash` is
 * piped into `bash`, so an explicit `text/plain` (not `application/octet-stream`
 * or a guessed shell MIME) keeps the asset predictable across proxies. We
 * re-wrap the static `install.sh` from the ASSETS binding to pin the header.
 */
export const onRequestGet: PagesFunction = async (context) => {
  const asset = await context.env.ASSETS.fetch(
    new URL("/install.sh", context.request.url),
  );
  if (!asset.ok) {
    return new Response("install.sh not found", { status: 404 });
  }
  return new Response(asset.body, {
    status: 200,
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
      "Cache-Control": "max-age=3600",
    },
  });
};
