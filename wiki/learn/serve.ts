// Tiny static server for the field guide. `bun serve.ts`, then open :3020.
const root = import.meta.dir;
Bun.serve({
  port: 3020,
  async fetch(req) {
    let path = new URL(req.url).pathname;
    if (path === "/" || path === "/index") path = "/index.html";
    const file = Bun.file(root + path);
    if (await file.exists()) return new Response(file);
    const html = Bun.file(root + path + ".html");
    if (await html.exists()) return new Response(html);
    return new Response("not found", { status: 404 });
  },
});
console.log("Learn Omega → http://localhost:3020");
