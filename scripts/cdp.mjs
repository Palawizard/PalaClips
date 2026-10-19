import { pathToFileURL } from "node:url";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function debuggerUrl(port) {
  for (let attempt = 0; attempt < 120; attempt++) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const page = targets.find((target) => target.type === "page" && !target.url.startsWith("devtools"));
      if (page) return page.webSocketDebuggerUrl;
    } catch {}
    await sleep(500);
  }
  throw new Error(`WebView2 n'ouvre pas le port de débogage ${port}`);
}

export async function connect(port) {
  const socket = new WebSocket(await debuggerUrl(port));
  await new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = reject;
  });
  const pending = new Map();
  let nextId = 0;
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);
    pending.get(message.id)?.(message);
    pending.delete(message.id);
  };
  const evaluate = async (expression) => {
    const id = ++nextId;
    const reply = new Promise((resolve) => pending.set(id, resolve));
    socket.send(
      JSON.stringify({ id, method: "Runtime.evaluate", params: { expression, awaitPromise: true, returnByValue: true } }),
    );
    const { result, error } = await reply;
    if (error) throw new Error(error.message);
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    return result.result.value;
  };
  return {
    evaluate,
    page: (fn, ...args) => evaluate(`(${fn})(...${JSON.stringify(args)})`),
    close: () => socket.close(),
  };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [port, expression] = process.argv.slice(2);
  const cdp = await connect(port);
  console.log(JSON.stringify(await cdp.evaluate(expression)));
  cdp.close();
}
